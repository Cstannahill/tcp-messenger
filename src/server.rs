use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};
use std::sync::mpsc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use tracing::{info, warn, error, debug};

// Enhanced message types with versioning and metadata
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum MessageType {
    Chat { content: String },
    SystemNotification { content: String },
    UserJoined { username: String },
    UserLeft { username: String },
    Heartbeat,
    Error { code: u32, message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub id: String,
    pub sender: String,
    pub timestamp: u64,
    pub version: u8,
    pub message_type: MessageType,
    pub channel: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Message {
    pub fn new(sender: String, message_type: MessageType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            sender,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: 1,
            message_type,
            channel: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_channel(mut self, channel: String) -> Self {
        self.channel = Some(channel);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

// Enhanced client representation with connection metadata
#[derive(Debug)]
pub struct Client {
    pub id: String,
    pub username: String,
    pub stream: TcpStream,
    pub writer: BufWriter<TcpStream>,
    pub connected_at: Instant,
    pub last_heartbeat: Instant,
    pub channels: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl Client {
    pub fn new(stream: TcpStream, username: String) -> Result<Self, std::io::Error> {
        let writer = BufWriter::new(stream.try_clone()?);
        let now = Instant::now();
        
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            username,
            stream,
            writer,
            connected_at: now,
            last_heartbeat: now,
            channels: vec!["general".to_string()], // Default channel
            metadata: HashMap::new(),
        })
    }

    pub fn send_message(&mut self, message: &Message) -> Result<(), std::io::Error> {
        let json = serde_json::to_string(message)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        
        writeln!(self.writer, "{}", json)?;
        self.writer.flush()?;
        Ok(())
    }

    pub fn is_stale(&self, timeout: Duration) -> bool {
        self.last_heartbeat.elapsed() > timeout
    }

    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = Instant::now();
    }
}

// Channel-based message routing system
#[derive(Debug)]
pub struct Channel {
    pub name: String,
    pub members: Vec<String>, // Client IDs
    pub created_at: Instant,
    pub message_history: Vec<Message>,
    pub max_history: usize,
}

impl Channel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            members: Vec::new(),
            created_at: Instant::now(),
            message_history: Vec::new(),
            max_history: 100,
        }
    }

    pub fn add_member(&mut self, client_id: String) {
        if !self.members.contains(&client_id) {
            self.members.push(client_id);
        }
    }

    pub fn remove_member(&mut self, client_id: &str) {
        self.members.retain(|id| id != client_id);
    }

    pub fn add_message(&mut self, message: Message) {
        self.message_history.push(message);
        if self.message_history.len() > self.max_history {
            self.message_history.remove(0);
        }
    }
}

// Enhanced server with comprehensive state management
pub struct ChatServer {
    clients: Arc<RwLock<HashMap<String, Client>>>,
    channels: Arc<RwLock<HashMap<String, Channel>>>,
    message_rate_limiter: Arc<RwLock<HashMap<String, (Instant, u32)>>>,
    config: ServerConfig,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_address: String,
    pub max_clients: usize,
    pub heartbeat_interval: Duration,
    pub client_timeout: Duration,
    pub message_rate_limit: u32,
    pub rate_limit_window: Duration,
    pub max_message_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:10210".to_string(),
            max_clients: 100,
            heartbeat_interval: Duration::from_secs(30),
            client_timeout: Duration::from_secs(90),
            message_rate_limit: 10,
            rate_limit_window: Duration::from_secs(60),
            max_message_size: 8192,
        }
    }
}

impl ChatServer {
    pub fn new(config: ServerConfig) -> Self {
        let mut channels = HashMap::new();
        channels.insert("general".to_string(), Channel::new("general".to_string()));
        
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            channels: Arc::new(RwLock::new(channels)),
            message_rate_limiter: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.config.bind_address)?;
        info!("Chat server listening on {}", self.config.bind_address);

        // Start background tasks
        self.start_heartbeat_monitor();
        self.start_cleanup_task();

        for stream_result in listener.incoming() {
            match stream_result {
                Ok(stream) => {
                    if self.clients.read().unwrap().len() >= self.config.max_clients {
                        warn!("Maximum client limit reached, rejecting connection");
                        continue;
                    }

                    let clients = Arc::clone(&self.clients);
                    let channels = Arc::clone(&self.channels);
                    let rate_limiter = Arc::clone(&self.message_rate_limiter);
                    let config = self.config.clone();

                    thread::spawn(move || {
                        if let Err(e) = Self::handle_client(stream, clients, channels, rate_limiter, config) {
                            error!("Error handling client: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }

        Ok(())
    }

    fn handle_client(
        stream: TcpStream,
        clients: Arc<RwLock<HashMap<String, Client>>>,
        channels: Arc<RwLock<HashMap<String, Channel>>>,
        rate_limiter: Arc<RwLock<HashMap<String, (Instant, u32)>>>,
        config: ServerConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = BufReader::new(stream.try_clone()?);
        
        // Initial handshake to get username
        let mut username_buffer = String::new();
        reader.read_line(&mut username_buffer)?;
        let username = username_buffer.trim().to_string();
        
        if username.is_empty() {
            return Err("Invalid username".into());
        }

        let mut client = Client::new(stream, username.clone())?;
        let client_id = client.id.clone();
        
        // Send welcome message
        let welcome_msg = Message::new(
            "system".to_string(),
            MessageType::SystemNotification {
                content: format!("Welcome to the chat server, {}!", username),
            },
        );
        client.send_message(&welcome_msg)?;

        // Add client to server state
        clients.write().unwrap().insert(client_id.clone(), client);
        
        // Add to general channel
        channels.write().unwrap()
            .get_mut("general")
            .unwrap()
            .add_member(client_id.clone());

        // Broadcast user joined
        let join_msg = Message::new(
            "system".to_string(),
            MessageType::UserJoined { username: username.clone() },
        ).with_channel("general".to_string());
        
        Self::broadcast_to_channel(&join_msg, "general", &clients, &channels)?;

        // Main message loop
        loop {
            let mut buffer = String::new();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    info!("Client {} disconnected", username);
                    break;
                }
                Ok(n) => {
                    if n > config.max_message_size {
                        warn!("Message too large from client {}", username);
                        continue;
                    }

                    // Rate limiting
                    if Self::is_rate_limited(&client_id, &rate_limiter, &config) {
                        warn!("Rate limit exceeded for client {}", username);
                        continue;
                    }

                    let received = buffer.trim();
                    
                    // Parse and handle message
                    let message: Message = match serde_json::from_str(received) {
                        Ok(msg) => msg,
                        Err(e) => {
                            error!("Error parsing JSON from client {}: {}", username, e);
                            continue;
                        }
                    };

                    debug!("Received message from {}: {:?}", username, message);

                    // Update client heartbeat
                    if let Some(client) = clients.write().unwrap().get_mut(&client_id) {
                        client.update_heartbeat();
                    }

                    // Route message based on type
                    match &message.message_type {
                        MessageType::Chat { .. } => {
                            let channel = message.channel.as_deref().unwrap_or("general");
                            Self::broadcast_to_channel(&message, channel, &clients, &channels)?;
                            
                            // Add to channel history
                            if let Some(ch) = channels.write().unwrap().get_mut(channel) {
                                ch.add_message(message.clone());
                            }
                        }
                        MessageType::Heartbeat => {
                            // Heartbeat handled by client update above
                        }
                        _ => {
                            // Handle other message types
                            Self::broadcast_to_channel(&message, "general", &clients, &channels)?;
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading from client {}: {}", username, e);
                    break;
                }
            }
        }

        // Cleanup on disconnect
        clients.write().unwrap().remove(&client_id);
        
        // Remove from all channels
        for channel in channels.write().unwrap().values_mut() {
            channel.remove_member(&client_id);
        }

        // Broadcast user left
        let leave_msg = Message::new(
            "system".to_string(),
            MessageType::UserLeft { username },
        ).with_channel("general".to_string());
        
        Self::broadcast_to_channel(&leave_msg, "general", &clients, &channels)?;

        Ok(())
    }

    fn broadcast_to_channel(
        message: &Message,
        channel_name: &str,
        clients: &Arc<RwLock<HashMap<String, Client>>>,
        channels: &Arc<RwLock<HashMap<String, Channel>>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let channel_members = {
            let channels_lock = channels.read().unwrap();
            channels_lock.get(channel_name)
                .map(|ch| ch.members.clone())
                .unwrap_or_default()
        };

        let mut disconnected_clients = Vec::new();
        
        {
            let mut clients_lock = clients.write().unwrap();
            for client_id in &channel_members {
                if let Some(client) = clients_lock.get_mut(client_id) {
                    if let Err(e) = client.send_message(message) {
                        warn!("Failed to send message to client {}: {}", client.username, e);
                        disconnected_clients.push(client_id.clone());
                    }
                }
            }
            
            // Remove disconnected clients
            for client_id in &disconnected_clients {
                clients_lock.remove(client_id);
            }
        }

        // Remove disconnected clients from channels
        if !disconnected_clients.is_empty() {
            let mut channels_lock = channels.write().unwrap();
            for channel in channels_lock.values_mut() {
                for client_id in &disconnected_clients {
                    channel.remove_member(client_id);
                }
            }
        }

        Ok(())
    }

    fn is_rate_limited(
        client_id: &str,
        rate_limiter: &Arc<RwLock<HashMap<String, (Instant, u32)>>>,
        config: &ServerConfig,
    ) -> bool {
        let now = Instant::now();
        let mut limiter = rate_limiter.write().unwrap();
        
        match limiter.get_mut(client_id) {
            Some((last_reset, count)) => {
                if now.duration_since(*last_reset) > config.rate_limit_window {
                    *last_reset = now;
                    *count = 1;
                    false
                } else {
                    *count += 1;
                    *count > config.message_rate_limit
                }
            }
            None => {
                limiter.insert(client_id.to_string(), (now, 1));
                false
            }
        }
    }

    fn start_heartbeat_monitor(&self) {
        let clients = Arc::clone(&self.clients);
        let channels = Arc::clone(&self.channels);
        let timeout = self.config.client_timeout;
        let interval = self.config.heartbeat_interval;

        thread::spawn(move || {
            loop {
                thread::sleep(interval);
                
                let mut stale_clients = Vec::new();
                
                {
                    let clients_lock = clients.read().unwrap();
                    for (id, client) in clients_lock.iter() {
                        if client.is_stale(timeout) {
                            stale_clients.push((id.clone(), client.username.clone()));
                        }
                    }
                }

                // Remove stale clients
                for (client_id, username) in stale_clients {
                    info!("Removing stale client: {}", username);
                    clients.write().unwrap().remove(&client_id);
                    
                    // Remove from channels
                    for channel in channels.write().unwrap().values_mut() {
                        channel.remove_member(&client_id);
                    }
                }
            }
        });
    }

    fn start_cleanup_task(&self) {
        let rate_limiter = Arc::clone(&self.message_rate_limiter);
        let window = self.config.rate_limit_window;

        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(300)); // Clean up every 5 minutes
                
                let now = Instant::now();
                let mut limiter = rate_limiter.write().unwrap();
                
                limiter.retain(|_, (last_reset, _)| {
                    now.duration_since(*last_reset) <= window * 2
                });
            }
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    let config = ServerConfig::default();
    let server = ChatServer::new(config);
    
    server.run()
}