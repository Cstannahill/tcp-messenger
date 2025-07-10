use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex, mpsc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use std::fmt;

// Custom error type for thread safety
#[derive(Debug, Clone)]
pub enum ClientError {
    ConnectionError(String),
    IoError(String),
    SerializationError(String),
    NetworkTimeout,
    InvalidHandshake,
    ServerDisconnected,
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            ClientError::IoError(msg) => write!(f, "IO error: {}", msg),
            ClientError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ClientError::NetworkTimeout => write!(f, "Network timeout"),
            ClientError::InvalidHandshake => write!(f, "Invalid handshake"),
            ClientError::ServerDisconnected => write!(f, "Server disconnected"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::TimedOut => ClientError::NetworkTimeout,
            io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset => {
                ClientError::ServerDisconnected
            }
            _ => ClientError::IoError(error.to_string()),
        }
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(error: serde_json::Error) -> Self {
        ClientError::SerializationError(error.to_string())
    }
}

// Enhanced message protocol with validation
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum MessageType {
    Chat { content: String },
    SystemNotification { content: String },
    UserJoined { username: String },
    UserLeft { username: String },
    Heartbeat,
    Error { code: u32, message: String },
    CommandResponse { command: String, response: String },
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
    pub fn new_chat(sender: String, content: String) -> Self {
        Self::validate_content(&content).expect("Invalid message content");
        
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        sender.hash(&mut hasher);
        content.hash(&mut hasher);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        
        Self {
            id: format!("msg_{:x}", hasher.finish()),
            sender,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: 1,
            message_type: MessageType::Chat { content },
            channel: Some("general".to_string()),
            metadata: HashMap::new(),
        }
    }

    pub fn new_heartbeat(sender: String) -> Self {
        Self {
            id: format!("heartbeat_{}", fastrand::u32(0..1_000_000)
),
            sender,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: 1,
            message_type: MessageType::Heartbeat,
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

    fn validate_content(content: &str) -> Result<(), ClientError> {
        if content.is_empty() {
            return Err(ClientError::SerializationError("Empty content".to_string()));
        }
        if content.len() > 4096 {
            return Err(ClientError::SerializationError("Content too long".to_string()));
        }
        Ok(())
    }
}

// Enhanced random number generator
mod fastrand {
    use std::cell::RefCell;
    use std::time::{SystemTime, UNIX_EPOCH};

    thread_local! {
        static RNG_STATE: RefCell<u64> = RefCell::new(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64
        );
    }

    pub fn u32(range: std::ops::Range<u32>) -> u32 {
        RNG_STATE.with(|state| {
            let mut s = state.borrow_mut();
            *s = s.wrapping_mul(1103515245).wrapping_add(12345);
            ((*s >> 16) as u32) % (range.end - range.start) + range.start
        })
    }
}

// Connection manager for robust networking
struct ConnectionManager {
    address: String,
    username: String,
    stream: Option<TcpStream>,
    retry_count: u32,
    max_retries: u32,
}

impl ConnectionManager {
    fn new(address: String, username: String) -> Self {
        Self {
            address,
            username,
            stream: None,
            retry_count: 0,
            max_retries: 3,
        }
    }

    fn connect(&mut self) -> Result<TcpStream, ClientError> {
        for attempt in 0..=self.max_retries {
            match TcpStream::connect(&self.address) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
                    stream.set_nodelay(true)?; // Disable Nagle's algorithm for lower latency
                    
                    // Perform handshake
                    let mut handshake_stream = stream.try_clone()?;
                    writeln!(handshake_stream, "{}", self.username)?;
                    handshake_stream.flush()?;
                    
                    self.stream = Some(stream.try_clone()?);
                    self.retry_count = 0;
                    
                    return Ok(stream);
                }
                Err(e) => {
                    if attempt < self.max_retries {
                        eprintln!("Connection attempt {} failed: {}. Retrying in 2s...", attempt + 1, e);
                        thread::sleep(Duration::from_secs(2));
                    } else {
                        return Err(ClientError::ConnectionError(format!("Failed after {} attempts: {}", self.max_retries + 1, e)));
                    }
                }
            }
        }
        
        Err(ClientError::ConnectionError("Max retries exceeded".to_string()))
    }

    fn is_connected(&self) -> bool {
        self.stream.is_some()
    }
}

// Enhanced chat client with improved architecture
pub struct ChatClient {
    connection_manager: ConnectionManager,
    running: Arc<AtomicBool>,
    message_tx: Option<mpsc::Sender<String>>,
    shutdown_rx: Option<mpsc::Receiver<()>>,
    stats: Arc<Mutex<ClientStats>>,
}

#[derive(Debug, Default)]
struct ClientStats {
    messages_sent: u64,
    messages_received: u64,
    heartbeats_sent: u64,
    connection_errors: u64,
    uptime_start: Option<std::time::Instant>,
}

impl ClientStats {
    fn new() -> Self {
        Self {
            uptime_start: Some(std::time::Instant::now()),
            ..Default::default()
        }
    }

    fn uptime(&self) -> Duration {
        self.uptime_start.map_or(Duration::from_secs(0), |start| start.elapsed())
    }

    fn print_stats(&self) {
        println!("\n=== Client Statistics ===");
        println!("Uptime: {:?}", self.uptime());
        println!("Messages sent: {}", self.messages_sent);
        println!("Messages received: {}", self.messages_received);
        println!("Heartbeats sent: {}", self.heartbeats_sent);
        println!("Connection errors: {}", self.connection_errors);
    }
}

impl ChatClient {
    pub fn new(username: String, server_address: String) -> Self {
        let connection_manager = ConnectionManager::new(server_address, username);
        
        Self {
            connection_manager,
            running: Arc::new(AtomicBool::new(false)),
            message_tx: None,
            shutdown_rx: None,
            stats: Arc::new(Mutex::new(ClientStats::new())),
        }
    }

    pub fn connect(&mut self) -> Result<(), ClientError> {
        println!("Connecting to server at {}...", self.connection_manager.address);
        
        let _stream = self.connection_manager.connect()?;
        self.running.store(true, Ordering::SeqCst);

        println!("Connected to server! You can now start chatting.");
        println!("Commands: /help, /users, /channels, /stats, /quit");
        println!("Type your message and press Enter to send.");

        Ok(())
    }

    pub fn start_interactive_session(&mut self) -> Result<(), ClientError> {
        let stream = self.connection_manager.stream.take()
            .ok_or(ClientError::ConnectionError("Not connected to server".to_string()))?;
        
        // Create channels for coordinated shutdown
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let (message_tx, message_rx) = mpsc::channel();
        
        self.message_tx = Some(message_tx);
        self.shutdown_rx = Some(shutdown_rx);

        // Start message receiving thread
        let reader_stream = stream.try_clone()?;
        let running_clone = Arc::clone(&self.running);
        let username_clone = self.connection_manager.username.clone();
        let stats_clone = Arc::clone(&self.stats);
        let shutdown_tx_clone = shutdown_tx.clone();
        
        let receiver_handle = thread::spawn(move || -> Result<(), ClientError> {
            Self::message_receiver(reader_stream, running_clone, username_clone, stats_clone, shutdown_tx_clone)
        });

        // Start heartbeat thread
        let heartbeat_stream = stream.try_clone()?;
        let running_clone = Arc::clone(&self.running);
        let username_clone = self.connection_manager.username.clone();
        let stats_clone = Arc::clone(&self.stats);
        
        let heartbeat_handle = thread::spawn(move || -> Result<(), ClientError> {
            Self::heartbeat_sender(heartbeat_stream, running_clone, username_clone, stats_clone)
        });

        // Start message sending thread
        let writer_stream = stream;
        let running_clone = Arc::clone(&self.running);
        let username_clone = self.connection_manager.username.clone();
        let stats_clone = Arc::clone(&self.stats);
        
        let sender_handle = thread::spawn(move || -> Result<(), ClientError> {
            Self::message_sender(writer_stream, running_clone, username_clone, stats_clone, message_rx)
        });

        // Main input loop
        self.input_loop()?;

        // Graceful shutdown
        self.running.store(false, Ordering::SeqCst);
        drop(self.message_tx.take()); // Close the channel
        
        // Wait for threads to finish with timeout
        let handles = vec![receiver_handle, heartbeat_handle, sender_handle];
        for handle in handles {
            if handle.join().is_err() {
                eprintln!("Warning: Thread failed to join cleanly");
            }
        }

        // Print final statistics
        self.stats.lock().unwrap().print_stats();
        println!("Disconnected from server.");
        Ok(())
    }

    fn input_loop(&mut self) -> Result<(), ClientError> {
        let stdin = io::stdin();
        let message_tx = self.message_tx.as_ref().unwrap();
        
        for line in stdin.lock().lines() {
            let input = line.map_err(|e| ClientError::IoError(e.to_string()))?;
            
            if input.trim() == "/quit" {
                break;
            }

            if input.trim().is_empty() {
                continue;
            }

            // Handle local commands
            if input.starts_with('/') {
                match input.trim() {
                    "/help" => {
                        self.print_help();
                        continue;
                    }
                    "/stats" => {
                        self.stats.lock().unwrap().print_stats();
                        continue;
                    }
                    _ => {
                        // Send command to server
                        if message_tx.send(input.trim().to_string()).is_err() {
                            break; // Channel closed
                        }
                    }
                }
            } else {
                // Send chat message
                if message_tx.send(input.trim().to_string()).is_err() {
                    break; // Channel closed
                }
            }
        }

        Ok(())
    }

    fn print_help(&self) {
        println!("Available commands:");
        println!("  /help     - Show this help");
        println!("  /users    - List connected users");
        println!("  /channels - List available channels");
        println!("  /stats    - Show client statistics");
        println!("  /quit     - Exit the chat");
    }

    fn message_receiver(
        stream: TcpStream,
        running: Arc<AtomicBool>,
        _username: String,
        stats: Arc<Mutex<ClientStats>>,
        shutdown_tx: mpsc::Sender<()>,
    ) -> Result<(), ClientError> {
        let mut reader = BufReader::new(stream);
        let mut buffer = String::new();

        while running.load(Ordering::SeqCst) {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    println!("\nServer disconnected.");
                    running.store(false, Ordering::SeqCst);
                    let _ = shutdown_tx.send(());
                    break;
                }
                Ok(_) => {
                    let received = buffer.trim();
                    if received.is_empty() {
                        continue;
                    }

                    // Parse incoming message
                    match serde_json::from_str::<Message>(received) {
                        Ok(message) => {
                            Self::display_message(&message);
                            stats.lock().unwrap().messages_received += 1;
                        }
                        Err(_) => {
                            // Handle non-JSON responses (like command responses)
                            println!("Server: {}", received);
                        }
                    }
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::TimedOut {
                        continue;
                    }
                    eprintln!("Error reading from server: {}", e);
                    stats.lock().unwrap().connection_errors += 1;
                    running.store(false, Ordering::SeqCst);
                    let _ = shutdown_tx.send(());
                    break;
                }
            }
        }

        Ok(())
    }

    fn message_sender(
        mut stream: TcpStream,
        running: Arc<AtomicBool>,
        username: String,
        stats: Arc<Mutex<ClientStats>>,
        message_rx: mpsc::Receiver<String>,
    ) -> Result<(), ClientError> {
        while running.load(Ordering::SeqCst) {
            match message_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(input) => {
                    let result = if input.starts_with('/') {
                        // Send command to server
                        writeln!(stream, "{}", input).map_err(ClientError::from)
                    } else {
                        // Send chat message
                        let message = Message::new_chat(username.clone(), input);
                        let json = serde_json::to_string(&message)?;
                        writeln!(stream, "{}", json).map_err(ClientError::from)
                    };

                    match result {
                        Ok(()) => {
                            stream.flush()?;
                            stats.lock().unwrap().messages_sent += 1;
                        }
                        Err(e) => {
                            eprintln!("Failed to send message: {}", e);
                            stats.lock().unwrap().connection_errors += 1;
                            running.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    continue; // Check running flag
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break; // Channel closed
                }
            }
        }

        Ok(())
    }

    fn heartbeat_sender(
        mut stream: TcpStream,
        running: Arc<AtomicBool>,
        username: String,
        stats: Arc<Mutex<ClientStats>>,
    ) -> Result<(), ClientError> {
        while running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(25));
            
            if !running.load(Ordering::SeqCst) {
                break;
            }

            let heartbeat = Message::new_heartbeat(username.clone());
            let json = serde_json::to_string(&heartbeat)?;
            
            match writeln!(stream, "{}", json) {
                Ok(()) => {
                    if let Err(e) = stream.flush() {
                        eprintln!("Failed to flush heartbeat: {}", e);
                        stats.lock().unwrap().connection_errors += 1;
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                    stats.lock().unwrap().heartbeats_sent += 1;
                }
                Err(e) => {
                    eprintln!("Failed to send heartbeat: {}", e);
                    stats.lock().unwrap().connection_errors += 1;
                    running.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }

        Ok(())
    }

    fn display_message(message: &Message) {
        match &message.message_type {
            MessageType::Chat { content } => {
                let timestamp = Self::format_timestamp(message.timestamp);
                let channel = message.channel.as_deref().unwrap_or("general");
                println!("[{}] #{} {}: {}", timestamp, channel, message.sender, content);
            }
            MessageType::SystemNotification { content } => {
                println!("*** {}", content);
            }
            MessageType::UserJoined { username } => {
                println!("*** {} joined the chat", username);
            }
            MessageType::UserLeft { username } => {
                println!("*** {} left the chat", username);
            }
            MessageType::Error { code, message } => {
                println!("ERROR {}: {}", code, message);
            }
            MessageType::CommandResponse { command, response } => {
                println!("/{}: {}", command, response);
            }
            MessageType::Heartbeat => {
                // Don't display heartbeat messages
            }
        }
    }

    fn format_timestamp(timestamp: u64) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let diff = now.saturating_sub(timestamp);
        
        if diff < 60 {
            "now".to_string()
        } else if diff < 3600 {
            format!("{}m ago", diff / 60)
        } else if diff < 86400 {
            format!("{}h ago", diff / 3600)
        } else {
            format!("{}d ago", diff / 86400)
        }
    }
}

// Enhanced batch message sender with connection pooling
pub struct MessageBatch {
    messages: Vec<Message>,
    server_address: String,
    batch_size: usize,
    delay_between_batches: Duration,
}

impl MessageBatch {
    pub fn new(server_address: String) -> Self {
        Self {
            messages: Vec::new(),
            server_address,
            batch_size: 10,
            delay_between_batches: Duration::from_millis(100),
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay_between_batches = delay;
        self
    }

    pub fn add_message(&mut self, sender: String, content: String) -> &mut Self {
        self.messages.push(Message::new_chat(sender, content));
        self
    }

    pub fn add_message_to_channel(&mut self, sender: String, content: String, channel: String) -> &mut Self {
        self.messages.push(Message::new_chat(sender, content).with_channel(channel));
        self
    }

    pub fn send_batch(&self, username: String) -> Result<(), ClientError> {
        let mut connection_manager = ConnectionManager::new(self.server_address.clone(), username);
        let mut stream = connection_manager.connect()?;

        let chunks: Vec<_> = self.messages.chunks(self.batch_size).collect();
        
        for (i, chunk) in chunks.iter().enumerate() {
            if i > 0 {
                thread::sleep(self.delay_between_batches);
            }

            for message in chunk.iter() {
                let json = serde_json::to_string(message)?;
                writeln!(stream, "{}", json)?;
                stream.flush()?;
            }
        }

        println!("Sent {} messages in {} batches", self.messages.len(), chunks.len());
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("TCP Messaging Client v2.0");
    println!("==========================");
    
    // Enhanced command line argument parsing
    let args: Vec<String> = std::env::args().collect();
    let username = if args.len() > 1 {
        args[1].clone()
    } else {
        print!("Enter your username: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    if username.is_empty() || username.len() > 32 {
        eprintln!("Username must be between 1 and 32 characters!");
        std::process::exit(1);
    }

    let server_address = args.get(2).cloned().unwrap_or_else(|| "127.0.0.1:10210".to_string());
    let mut client = ChatClient::new(username, server_address);

    match client.connect() {
        Ok(()) => {
            if let Err(e) = client.start_interactive_session() {
                eprintln!("Session error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to server: {}", e);
            eprintln!("Make sure the server is running on the specified address");
            std::process::exit(1);
        }
    }

    Ok(())
}

// Enhanced example module with better error handling
pub mod examples {
    use super::*;

    pub fn send_single_message(username: &str, content: &str) -> Result<(), ClientError> {
        let mut connection_manager = ConnectionManager::new("127.0.0.1:10210".to_string(), username.to_string());
        let mut stream = connection_manager.connect()?;
        
        let message = Message::new_chat(username.to_string(), content.to_string());
        let json = serde_json::to_string(&message)?;
        writeln!(stream, "{}", json)?;
        stream.flush()?;
        
        println!("Message sent successfully!");
        Ok(())
    }

    pub fn send_message_sequence(username: &str, messages: Vec<&str>) -> Result<(), ClientError> {
        let mut batch = MessageBatch::new("127.0.0.1:10210".to_string())
            .with_batch_size(5)
            .with_delay(Duration::from_millis(200));
        
        for msg in messages {
            batch.add_message(username.to_string(), msg.to_string());
        }
        
        batch.send_batch(username.to_string())?;
        Ok(())
    }

    pub fn run_chat_bot(bot_name: &str, responses: Vec<&str>) -> Result<(), ClientError> {
        let mut connection_manager = ConnectionManager::new("127.0.0.1:10210".to_string(), bot_name.to_string());
        let mut stream = connection_manager.connect()?;

        for response in &responses {
            let message = Message::new_chat(bot_name.to_string(), response.to_string());
            let json = serde_json::to_string(&message)?;
            writeln!(stream, "{}", json)?;
            stream.flush()?;
            
            thread::sleep(Duration::from_secs(2));
        }

        println!("Chat bot completed {} responses", responses.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let message = Message::new_chat("test_user".to_string(), "Hello, world!".to_string());
        
        assert_eq!(message.sender, "test_user");
        assert_eq!(message.version, 1);
        
        match message.message_type {
            MessageType::Chat { content } => {
                assert_eq!(content, "Hello, world!");
            }
            _ => panic!("Expected Chat message type"),
        }
    }

    #[test]
    fn test_message_validation() {
        let result = std::panic::catch_unwind(|| {
            Message::new_chat("test".to_string(), "".to_string())
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_message_batch() {
        let mut batch = MessageBatch::new("127.0.0.1:10210".to_string())
            .with_batch_size(2)
            .with_delay(Duration::from_millis(50));
        
        batch.add_message("user1".to_string(), "First message".to_string())
             .add_message_to_channel("user1".to_string(), "Channel message".to_string(), "test".to_string());
        
        assert_eq!(batch.messages.len(), 2);
        assert_eq!(batch.messages[1].channel, Some("test".to_string()));
        assert_eq!(batch.batch_size, 2);
    }

    #[test]
    fn test_client_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::TimedOut, "timeout");
        let client_error = ClientError::from(io_error);
        
        match client_error {
            ClientError::NetworkTimeout => {},
            _ => panic!("Expected NetworkTimeout"),
        }
    }
}