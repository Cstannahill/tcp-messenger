use std::collections::HashMap;
use std::io::{self, Write};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::file_transfer::{FileTransferManager, utils};
use uuid;
use chrono;

// Custom error type for thread safety
#[derive(Debug, Clone)]
pub enum ClientError {
    ConnectionError { 
        address: String, 
        reason: String, 
        context: Option<String> 
    },
    IoError { 
        operation: String, 
        reason: String, 
        context: Option<String> 
    },
    SerializationError { 
        message_type: String, 
        reason: String 
    },
    NetworkTimeout { 
        operation: String, 
        duration_ms: u64 
    },
    InvalidHandshake { 
        expected: String, 
        received: String 
    },
    ServerDisconnected { 
        reason: String, 
        last_activity: String 
    },
    ConfigurationError { 
        field: String, 
        value: String, 
        reason: String 
    },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::ConnectionError { address, reason, context } => {
                match context {
                    Some(ctx) => write!(f, "Failed to connect to {}: {} (Context: {})", address, reason, ctx),
                    None => write!(f, "Failed to connect to {}: {}", address, reason),
                }
            },
            ClientError::IoError { operation, reason, context } => {
                match context {
                    Some(ctx) => write!(f, "IO error during {}: {} (Context: {})", operation, reason, ctx),
                    None => write!(f, "IO error during {}: {}", operation, reason),
                }
            },
            ClientError::SerializationError { message_type, reason } => {
                write!(f, "Failed to serialize/deserialize {}: {}", message_type, reason)
            },
            ClientError::NetworkTimeout { operation, duration_ms } => {
                write!(f, "Network timeout during {} after {}ms", operation, duration_ms)
            },
            ClientError::InvalidHandshake { expected, received } => {
                write!(f, "Handshake failed - expected: '{}', received: '{}'", expected, received)
            },
            ClientError::ServerDisconnected { reason, last_activity } => {
                write!(f, "Server disconnected: {} (Last activity: {})", reason, last_activity)
            },
            ClientError::ConfigurationError { field, value, reason } => {
                write!(f, "Configuration error in field '{}' with value '{}': {}", field, value, reason)
            },
        }
    }
}

impl std::error::Error for ClientError {}

impl From<io::Error> for ClientError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::TimedOut => ClientError::NetworkTimeout {
                operation: "IO operation".to_string(),
                duration_ms: 0, // Duration not available from io::Error
            },
            io::ErrorKind::ConnectionAborted | io::ErrorKind::ConnectionReset => {
                ClientError::ServerDisconnected {
                    reason: error.to_string(),
                    last_activity: chrono::Utc::now().to_rfc3339(),
                }
            }
            _ => ClientError::IoError {
                operation: "General IO".to_string(),
                reason: error.to_string(),
                context: None,
            },
        }
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(error: serde_json::Error) -> Self {
        ClientError::SerializationError {
            message_type: "JSON".to_string(),
            reason: error.to_string(),
        }
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
    // File transfer message types
    FileUploadRequest { 
        file_id: String, 
        filename: String, 
        file_size: u64, 
        mime_type: String, 
        checksum: String 
    },
    FileUploadResponse { 
        file_id: String, 
        accepted: bool, 
        reason: Option<String>, 
        chunk_size: usize 
    },
    FileChunk { 
        file_id: String, 
        chunk_index: u32, 
        total_chunks: u32, 
        data: String 
    },
    FileChunkAck { 
        file_id: String, 
        chunk_index: u32, 
        received: bool 
    },
    FileUploadComplete { 
        file_id: String, 
        success: bool, 
        message: String 
    },
    FileDownloadRequest { 
        file_id: String 
    },
    FileDownloadResponse { 
        file_id: String, 
        available: bool, 
        filename: Option<String>, 
        file_size: Option<u64>, 
        mime_type: Option<String> 
    },
    FileListRequest,
    FileListResponse { 
        files: Vec<crate::file_transfer::FileInfo> 
    },
    FileProgress { 
        file_id: String, 
        bytes_transferred: u64, 
        total_bytes: u64, 
        percentage: f32 
    },
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
            return Err(ClientError::SerializationError {
                message_type: "Message content".to_string(),
                reason: "Empty content".to_string(),
            });
        }
        if content.len() > 4096 {
            return Err(ClientError::SerializationError {
                message_type: "Message content".to_string(),
                reason: "Content too long".to_string(),
            });
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
    async fn connect(&mut self) -> Result<TcpStream, ClientError> {
        use tokio::io::AsyncWriteExt;
        println!("[Client] Attempting to connect to server at {} as username '{}'", self.address, self.username);
        for attempt in 0..=self.max_retries {
            println!("[Client] Connection attempt {} to {}", attempt + 1, self.address);
            match TcpStream::connect(&self.address).await {
                Ok(mut stream) => {
                    println!("[Client] TCP connection established to {}", self.address);
                    // Perform handshake
                    println!("[Client] Sending handshake with username '{}'", self.username);
                    if let Err(e) = stream.write_all(format!("{}\n", self.username).as_bytes()).await {
                        eprintln!("[Client] Failed to send handshake: {}", e);
                        return Err(ClientError::IoError {
                            operation: "handshake".to_string(),
                            reason: e.to_string(),
                            context: Some(format!("Sending username '{}' to {}", self.username, self.address)),
                        });
                    }
                    self.stream = Some(stream);
                    self.retry_count = 0;
                    println!("[Client] Connection and handshake successful to {} as '{}'", self.address, self.username);
                    // Store the stream and return a clone/reference - but TcpStream doesn't support clone
                    // Instead, store it and take only when needed
                    let stream = self.stream.take().unwrap();
                    return Ok(stream);
                }
                Err(e) => {
                    eprintln!("[Client] Connection attempt {} failed: {}", attempt + 1, e);
                    if attempt < self.max_retries {
                        let delay = 8 + attempt * 4;
                        println!("[Client] Retrying in {} seconds for mobile robustness...", delay);
                        sleep(Duration::from_secs(delay as u64)).await;
                    } else {
                        eprintln!("[Client] All connection attempts failed. Giving up.");
                        return Err(ClientError::ConnectionError {
                            address: self.address.clone(),
                            reason: format!("Failed after {} attempts", self.max_retries + 1),
                            context: Some(format!("Last error: {}", e)),
                        });
                    }
                }
            }
        }
        eprintln!("[Client] Max retries exceeded. Unable to connect to server at {}", self.address);
        Err(ClientError::ConnectionError {
            address: self.address.clone(),
            reason: "Max retries exceeded".to_string(),
            context: None,
        })
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
    file_manager: Option<FileTransferManager>,
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
    async fn input_loop_concurrent(message_tx: mpsc::Sender<String>, mut shutdown_rx: mpsc::Receiver<()>) -> Result<(), ClientError> {
        use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader, stdin as async_stdin};
        let mut stdin = AsyncBufReader::new(async_stdin());
        loop {
            let mut input = String::new();
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    break;
                }
                result = stdin.read_line(&mut input) => {
                    let bytes_read = result?;
                    if bytes_read == 0 {
                        break;
                    }
                    let line = input.trim().to_string();
                    if line == "/quit" {
                        break;
                    }
                    if line.is_empty() {
                        continue;
                    }
                    if message_tx.send(line).await.is_err() {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn new(username: String, server_address: String) -> Self {
        let connection_manager = ConnectionManager::new(server_address, username);
        // Initialize file manager with downloads directory
        let file_manager = FileTransferManager::new("./downloads").ok();
        
        Self {
            connection_manager,
            running: Arc::new(AtomicBool::new(false)),
            message_tx: None,
            shutdown_rx: None,
            stats: Arc::new(Mutex::new(ClientStats::new())),
            file_manager,
        }
    }

    pub async fn connect(&mut self) -> Result<(), ClientError> {
        println!("[Client] Connecting to server at {}...", self.connection_manager.address);
        match self.connection_manager.connect().await {
            Ok(stream) => {
                // Store the stream back in the connection manager
                self.connection_manager.stream = Some(stream);
                self.running.store(true, Ordering::SeqCst);
                println!("[Client] Connected to server! You can now start chatting.");
                println!("[Client] Commands: /help, /users, /channels, /stats, /upload, /download, /files, /quit");
                println!("[Client] Type your message and press Enter to send.");
                Ok(())
            }
            Err(e) => {
                eprintln!("[Client] Failed to connect: {}", e);
                Err(e)
            }
        }
    }

    /// Auto-reconnect loop with exponential backoff
    pub async fn run_with_auto_reconnect(&mut self) {
        let mut backoff = 2;
        let max_backoff = 60;
        loop {
            match self.connect().await {
                Ok(()) => {
                    backoff = 2;
                    let session_result = self.start_interactive_session().await;
                    match session_result {
                        Ok(()) => {
                            println!("Session ended gracefully.");
                            break;
                        }
                        Err(e) => {
                            eprintln!("Session error: {}", e);
                            println!("Attempting to reconnect in {} seconds...", backoff);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Connection error: {}", e);
                    println!("Reconnecting in {} seconds...", backoff);
                }
            }
            sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(max_backoff);
        }
    }

    pub async fn start_interactive_session(&mut self) -> Result<(), ClientError> {
        // Receiver task needs these clones
        // Extract stream and create shutdown/message channels at the top
        let stream = self.connection_manager.stream.take()
            .ok_or(ClientError::ConnectionError {
                address: "unknown".to_string(),
                reason: "Not connected to server".to_string(),
                context: None,
            })?;
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        let (message_tx, message_rx) = mpsc::channel(32);

        self.message_tx = Some(message_tx.clone());
        // Do not assign shutdown_rx to self; it is moved into the input loop task below

        // Split the stream for concurrent tasks
        let (reader_stream, writer_stream) = stream.into_split();

        // Receiver task
        let receiver_handle = tokio::spawn({
            let running_clone = Arc::clone(&self.running);
            let username_clone = self.connection_manager.username.clone();
            let stats_clone = Arc::clone(&self.stats);
            let shutdown_tx_clone = shutdown_tx.clone();
            Self::message_receiver(reader_stream, running_clone, username_clone, stats_clone, shutdown_tx_clone)
        });

        // Unused variables removed
        // Heartbeat and sender both want to write; refactor: heartbeat sends to sender via channel
        // Create a heartbeat channel
        let (heartbeat_tx, heartbeat_rx) = mpsc::channel(8);
        let running_clone = Arc::clone(&self.running);
        let username_clone = self.connection_manager.username.clone();
        let stats_clone = Arc::clone(&self.stats);
        let heartbeat_handle = tokio::spawn(async move {
            while running_clone.load(Ordering::SeqCst) {
                sleep(Duration::from_secs(25)).await;
                if !running_clone.load(Ordering::SeqCst) {
                    break;
                }
                let heartbeat = Message::new_heartbeat(username_clone.clone());
                if heartbeat_tx.send(heartbeat).await.is_err() {
                    break;
                }
                stats_clone.lock().await.heartbeats_sent += 1;
            }
            Ok::<(), ClientError>(())
        });

        // Sender task: receives from both message_rx and heartbeat_rx
        // Unused variables removed
        let sender_handle = tokio::spawn({
            let mut message_rx = message_rx;
            let mut heartbeat_rx = heartbeat_rx;
            let mut writer_stream = writer_stream;
            let running_clone = Arc::clone(&self.running);
            let username_clone = self.connection_manager.username.clone();
            let stats_clone = Arc::clone(&self.stats);
            // Don't clone file_manager, just pass the reference
            async move {
                use tokio::io::AsyncWriteExt;
                loop {
                    tokio::select! {
                        Some(input) = message_rx.recv() => {
                            let result = if input.starts_with('/') {
                                // Handle file transfer commands locally first - no file manager for now
                                match Self::handle_command(input.clone(), None, &mut writer_stream).await {
                                    Ok(handled) => {
                                        if !handled {
                                            // Command not handled locally, send to server
                                            writer_stream.write_all(format!("{}\n", input).as_bytes()).await.map_err(ClientError::from)
                                        } else {
                                            Ok(())
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Command error: {}", e);
                                        Ok(())
                                    }
                                }
                            } else {
                                let message = Message::new_chat(username_clone.clone(), input);
                                let json = serde_json::to_string(&message)?;
                                writer_stream.write_all(format!("{}\n", json).as_bytes()).await.map_err(ClientError::from)
                            };
                            match result {
                                Ok(()) => {
                                    stats_clone.lock().await.messages_sent += 1;
                                }
                                Err(e) => {
                                    eprintln!("Failed to send message: {}", e);
                                    stats_clone.lock().await.connection_errors += 1;
                                    running_clone.store(false, Ordering::SeqCst);
                                    break;
                                }
                            }
                        }
                        Some(heartbeat) = heartbeat_rx.recv() => {
                            let json = serde_json::to_string(&heartbeat)?;
                            if let Err(e) = writer_stream.write_all(format!("{}\n", json).as_bytes()).await {
                                eprintln!("Failed to send heartbeat: {}", e);
                                stats_clone.lock().await.connection_errors += 1;
                                running_clone.store(false, Ordering::SeqCst);
                                break;
                            }
                        }
                        else => { break; }
                    }
                }
                Ok::<(), ClientError>(())
            }
        });

        // Legacy sender task removed; all sending is handled in the new sender task above

        // Main input loop runs concurrently with shutdown signal
        // No need for mut, just move shutdown_rx into the input loop task
        let input_handle = tokio::spawn(async move {
            Self::input_loop_concurrent(message_tx, shutdown_rx).await
        });

        // Wait for tasks to finish
        let handles = vec![receiver_handle, heartbeat_handle, sender_handle, input_handle];
        for handle in handles {
            if let Err(e) = handle.await {
                eprintln!("Warning: Task failed to join cleanly: {:?}", e);
            }
        }

        // Print final statistics
        let stats = self.stats.lock().await;
        stats.print_stats();
        println!("Disconnected from server.");
        Ok(())
    }
    
    // File transfer methods
    
    /// Upload a file to the server
    pub async fn upload_file<P: AsRef<Path>>(&mut self, file_path: P) -> Result<String, ClientError> {
        let file_path = file_path.as_ref();
        
        if let Some(ref mut file_manager) = self.file_manager {
            // Read file
            let mut file = File::open(file_path).await
                .map_err(|e| ClientError::IoError {
                    operation: "open file".to_string(),
                    reason: e.to_string(),
                    context: Some(file_path.display().to_string()),
                })?;
            
            let mut file_data = Vec::new();
            file.read_to_end(&mut file_data).await
                .map_err(|e| ClientError::IoError {
                    operation: "read file".to_string(),
                    reason: e.to_string(),
                    context: Some(file_path.display().to_string()),
                })?;
            
            let filename = file_path.file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| ClientError::IoError {
                    operation: "get filename".to_string(),
                    reason: "Invalid filename".to_string(),
                    context: Some(file_path.display().to_string()),
                })?;
            
            let file_size = file_data.len() as u64;
            let checksum = utils::calculate_checksum(&file_data);
            let mime_type = "application/octet-stream"; // TODO: Better MIME detection
            
            // Start upload session
            let file_id = file_manager.start_upload(
                filename.to_string(),
                file_size,
                mime_type.to_string(),
                checksum.clone(),
            ).map_err(|e| ClientError::SerializationError {
                message_type: "file upload".to_string(),
                reason: e,
            })?;
            
            // Send upload request
            let upload_msg = Message {
                id: uuid::Uuid::new_v4().to_string(),
                sender: self.connection_manager.username.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                version: 1,
                message_type: MessageType::FileUploadRequest {
                    file_id: file_id.clone(),
                    filename: filename.to_string(),
                    file_size,
                    mime_type: mime_type.to_string(),
                    checksum,
                },
                channel: None,
                metadata: HashMap::new(),
            };
            
            self.send_message_direct(upload_msg).await?;
            
            println!("File upload initiated: {} ({})", filename, utils::format_file_size(file_size));
            Ok(file_id)
        } else {
            Err(ClientError::ConfigurationError {
                field: "file_manager".to_string(),
                value: "none".to_string(),
                reason: "File transfer not initialized".to_string(),
            })
        }
    }
    
    /// List files available on the server
    pub async fn list_files(&mut self) -> Result<(), ClientError> {
        let list_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            sender: self.connection_manager.username.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: 1,
            message_type: MessageType::FileListRequest,
            channel: None,
            metadata: HashMap::new(),
        };
        
        self.send_message_direct(list_msg).await?;
        Ok(())
    }
    
    /// Download a file from the server
    pub async fn download_file(&mut self, file_id: String) -> Result<(), ClientError> {
        let download_msg = Message {
            id: uuid::Uuid::new_v4().to_string(),
            sender: self.connection_manager.username.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            version: 1,
            message_type: MessageType::FileDownloadRequest { file_id },
            channel: None,
            metadata: HashMap::new(),
        };
        
        self.send_message_direct(download_msg).await?;
        Ok(())
    }
    
    /// Send a message directly (internal helper)
    async fn send_message_direct(&mut self, message: Message) -> Result<(), ClientError> {
        if let Some(ref mut stream) = self.connection_manager.stream {
            let json = serde_json::to_string(&message)
                .map_err(|e| ClientError::SerializationError {
                    message_type: "message".to_string(),
                    reason: e.to_string(),
                })?;
            
            stream.write_all(format!("{}\n", json).as_bytes()).await
                .map_err(|e| ClientError::IoError {
                    operation: "send message".to_string(),
                    reason: e.to_string(),
                    context: Some("direct message send".to_string()),
                })?;
            
            Ok(())
        } else {
            Err(ClientError::ConnectionError {
                address: self.connection_manager.address.clone(),
                reason: "Not connected".to_string(),
                context: Some("send_message_direct".to_string()),
            })
        }
    }
    
    // End of ChatClient impl

    fn print_help(&self) {
        println!("Available commands:");
        println!("  /help     - Show this help");
        println!("  /users    - List connected users");
        println!("  /channels - List available channels");
        println!("  /stats    - Show client statistics");
        println!("  /upload <path> - Upload a file to the server");
        println!("  /download <filename> - Download a file from the server");
        println!("  /files    - List files available on the server");
        println!("  /quit     - Exit the chat");
    }

    /// Handle client-side commands, returns true if command was handled locally
    async fn handle_command(
        input: String,
        file_manager: Option<FileTransferManager>,
        stream: &mut tokio::net::tcp::OwnedWriteHalf,
    ) -> Result<bool, ClientError> {
        use tokio::io::AsyncWriteExt;
        
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(false);
        }

        match parts[0] {
            "/help" => {
                println!("Available commands:");
                println!("  /help     - Show this help");
                println!("  /users    - List connected users");
                println!("  /channels - List available channels");
                println!("  /stats    - Show client statistics");
                println!("  /upload <path> - Upload a file to the server");
                println!("  /download <filename> - Download a file from the server");
                println!("  /files    - List files available on the server");
                println!("  /quit     - Exit the chat");
                Ok(true)
            }
            "/upload" => {
                if parts.len() < 2 {
                    println!("Usage: /upload <file_path>");
                    return Ok(true);
                }
                
                let file_path = parts[1..].join(" "); // Handle paths with spaces
                println!("Uploading file: {}", file_path);
                
                // Basic file existence check
                if !std::path::Path::new(&file_path).exists() {
                    println!("File not found: {}", file_path);
                    return Ok(true);
                }
                
                // Read file size
                let metadata = std::fs::metadata(&file_path);
                let file_size = match metadata {
                    Ok(meta) => meta.len(),
                    Err(e) => {
                        println!("Error reading file metadata: {}", e);
                        return Ok(true);
                    }
                };
                
                // Generate file ID and get filename
                let file_id = uuid::Uuid::new_v4().to_string();
                let filename = std::path::Path::new(&file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                println!("File validation passed. Size: {} bytes", file_size);
                
                // Create upload request message
                let upload_request = MessageType::FileUploadRequest {
                    file_id,
                    filename,
                    file_size,
                    mime_type: "application/octet-stream".to_string(), // Default MIME type
                    checksum: "".to_string(), // Will be calculated during upload
                };
                
                let message = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: "client".to_string(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    version: 1,
                    message_type: upload_request,
                    channel: None,
                    metadata: HashMap::new(),
                };
                
                let json = serde_json::to_string(&message)
                    .map_err(|e| ClientError::SerializationError {
                        message_type: "file upload request".to_string(),
                        reason: e.to_string(),
                    })?;
                
                stream.write_all(format!("{}\n", json).as_bytes()).await
                    .map_err(|e| ClientError::IoError {
                        operation: "send upload request".to_string(),
                        reason: e.to_string(),
                        context: Some("upload command".to_string()),
                    })?;
                
                println!("Upload request sent to server");
                Ok(true)
            }
            "/download" => {
                if parts.len() < 2 {
                    println!("Usage: /download <filename>");
                    return Ok(true);
                }
                
                let file_id = parts[1..].join(" ");
                println!("Requesting download: {}", file_id);
                
                // Create download request message
                let download_request = MessageType::FileDownloadRequest {
                    file_id: file_id.clone(),
                };
                
                let message = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: "client".to_string(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    version: 1,
                    message_type: download_request,
                    channel: None,
                    metadata: HashMap::new(),
                };
                
                let json = serde_json::to_string(&message)
                    .map_err(|e| ClientError::SerializationError {
                        message_type: "file download request".to_string(),
                        reason: e.to_string(),
                    })?;
                
                stream.write_all(format!("{}\n", json).as_bytes()).await
                    .map_err(|e| ClientError::IoError {
                        operation: "send download request".to_string(),
                        reason: e.to_string(),
                        context: Some("download command".to_string()),
                    })?;
                
                println!("Download request sent to server");
                Ok(true)
            }
            "/files" => {
                println!("Requesting file list from server...");
                
                // Create file list request message
                let list_request = MessageType::FileListRequest;
                
                let message = Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: "client".to_string(),
                    timestamp: chrono::Utc::now().timestamp() as u64,
                    version: 1,
                    message_type: list_request,
                    channel: None,
                    metadata: HashMap::new(),
                };
                
                let json = serde_json::to_string(&message)
                    .map_err(|e| ClientError::SerializationError {
                        message_type: "file list request".to_string(),
                        reason: e.to_string(),
                    })?;
                
                stream.write_all(format!("{}\n", json).as_bytes()).await
                    .map_err(|e| ClientError::IoError {
                        operation: "send file list request".to_string(),
                        reason: e.to_string(),
                        context: Some("files command".to_string()),
                    })?;
                
                Ok(true)
            }
            _ => Ok(false), // Command not handled locally
        }
    }

async fn message_receiver(
stream: tokio::net::tcp::OwnedReadHalf,
    running: Arc<AtomicBool>,
    _username: String,
    stats: Arc<Mutex<ClientStats>>,
    shutdown_tx: mpsc::Sender<()>,
) -> Result<(), ClientError> {
    use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
    let mut reader = AsyncBufReader::new(stream);
    let mut buffer = String::new();

    while running.load(Ordering::SeqCst) {
        buffer.clear();
        let bytes_read = reader.read_line(&mut buffer).await?;
        if bytes_read == 0 {
            println!("\nServer disconnected.");
            running.store(false, Ordering::SeqCst);
            let _ = shutdown_tx.send(());
            break;
        }
        let received = buffer.trim();
        if received.is_empty() {
            continue;
        }
        // Parse incoming message
        match serde_json::from_str::<Message>(received) {
            Ok(message) => {
                Self::display_message(&message);
                stats.lock().await.messages_received += 1;
            }
            Err(_) => {
                // Handle non-JSON responses (like command responses)
                println!("Server: {}", received);
            }
        }
    }
    Ok(())
}

async fn message_sender(
    mut stream: tokio::net::tcp::OwnedWriteHalf,
    running: Arc<AtomicBool>,
    username: String,
    stats: Arc<Mutex<ClientStats>>,
    mut message_rx: mpsc::Receiver<String>,
) -> Result<(), ClientError> {
    use tokio::io::AsyncWriteExt;
    while running.load(Ordering::SeqCst) {
        match message_rx.recv().await {
            Some(input) => {
                let result = if input.starts_with('/') {
                    stream.write_all(format!("{}\n", input).as_bytes()).await.map_err(ClientError::from)
                } else {
                    let message = Message::new_chat(username.clone(), input);
                    let json = serde_json::to_string(&message)?;
                    stream.write_all(format!("{}\n", json).as_bytes()).await.map_err(ClientError::from)
                };
                match result {
                    Ok(()) => {
                    stats.lock().await.messages_sent += 1;
                    }
                    Err(e) => {
                        eprintln!("Failed to send message: {}", e);
                        stats.lock().await.connection_errors += 1;
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
            None => {
                break; // Channel closed
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
            MessageType::FileUploadResponse { file_id, accepted, reason, chunk_size: _ } => {
                if *accepted {
                    println!("*** File upload accepted: {} (ID: {})", reason.as_deref().unwrap_or("Ready"), file_id);
                } else {
                    println!("*** File upload rejected: {}", reason.as_deref().unwrap_or("Unknown error"));
                }
            }
            MessageType::FileDownloadResponse { file_id: _, available, filename, file_size, mime_type: _ } => {
                if *available {
                    println!("*** File download ready: {} ({} bytes)", 
                        filename.as_deref().unwrap_or("unknown"), 
                        file_size.unwrap_or(0));
                } else {
                    println!("*** File not available for download");
                }
            }
            MessageType::FileListResponse { files } => {
                if files.is_empty() {
                    println!("*** No files available on server");
                } else {
                    println!("*** Available files:");
                    for file in files {
                        println!("  - {} ({} bytes, uploaded: {})", 
                            file.filename, 
                            file.file_size,
                            chrono::DateTime::<chrono::Utc>::from_timestamp(file.uploaded_at.timestamp(), 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        );
                    }
                }
            }
            MessageType::FileChunk { .. } => {
                // File chunks are handled internally, not displayed
            }
            _ => {
                // Handle other message types that might be added
                println!("*** Received message: {:?}", message.message_type);
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

    pub async fn send_batch(&self, username: String) -> Result<(), ClientError> {
        use tokio::io::AsyncWriteExt;
        use tokio::time::sleep;
        let mut connection_manager = ConnectionManager::new(self.server_address.clone(), username.clone());
        let stream = connection_manager.connect().await?;
        let mut write_half = stream.into_split().1;
        let chunks: Vec<_> = self.messages.chunks(self.batch_size).collect();
        // Send each batch sequentially (concurrent writing to same stream is not safe)
        for (i, chunk) in chunks.iter().enumerate() {
            let delay = if i > 0 { self.delay_between_batches } else { Duration::from_millis(0) };
            if delay > Duration::from_millis(0) {
                sleep(delay).await;
            }
            for message in chunk.iter() {
                let json = serde_json::to_string(&message)?;
                write_half.write_all(format!("{}\n", json).as_bytes()).await?;
            }
        }
        println!("Sent {} messages in {} batches", self.messages.len(), chunks.len());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    let server_address = args.get(2).cloned().unwrap_or_else(|| "127.0.0.1:80".to_string());
    let mut client = ChatClient::new(username, server_address);

    client.run_with_auto_reconnect().await;

    Ok(())
}

// Enhanced example module with better error handling
pub mod examples {
    use super::*;

    pub async fn send_single_message(username: &str, content: &str) -> Result<(), ClientError> {
        let mut connection_manager = ConnectionManager::new("127.0.0.1:80".to_string(), username.to_string());
        let stream = connection_manager.connect().await?;
        let mut write_half = stream.into_split().1;
        use tokio::io::AsyncWriteExt;
        let message = Message::new_chat(username.to_string(), content.to_string());
        let json = serde_json::to_string(&message)?;
        write_half.write_all(format!("{}\n", json).as_bytes()).await?;
        println!("Message sent successfully!");
        Ok(())
    }

    pub async fn send_message_sequence(username: &str, messages: Vec<&str>) -> Result<(), ClientError> {
        let mut batch = MessageBatch::new("127.0.0.1:80".to_string())
            .with_batch_size(5)
            .with_delay(Duration::from_millis(200));
        for msg in messages {
            batch.add_message(username.to_string(), msg.to_string());
        }
        batch.send_batch(username.to_string()).await?;
        Ok(())
    }

    pub async fn run_chat_bot(bot_name: &str, responses: Vec<&str>) -> Result<(), ClientError> {
        use tokio::io::AsyncWriteExt;
        use tokio::time::sleep;
        let mut connection_manager = ConnectionManager::new("127.0.0.1:80".to_string(), bot_name.to_string());
        let stream = connection_manager.connect().await?;
        let mut write_half = stream.into_split().1;
        for response in &responses {
            let message = Message::new_chat(bot_name.to_string(), response.to_string());
            let json = serde_json::to_string(&message)?;
            write_half.write_all(format!("{}\n", json).as_bytes()).await?;
            sleep(Duration::from_secs(2)).await;
        }
        println!("Chat bot completed {} responses", responses.len());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_creation() {
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

    #[tokio::test]
    async fn test_message_validation() {
        let result = std::panic::catch_unwind(|| {
            Message::new_chat("test".to_string(), "".to_string())
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_message_batch() {
        let mut batch = MessageBatch::new("127.0.0.1:80".to_string())
            .with_batch_size(2)
            .with_delay(Duration::from_millis(50));
        batch.add_message("user1".to_string(), "First message".to_string())
             .add_message_to_channel("user1".to_string(), "Channel message".to_string(), "test".to_string());
        assert_eq!(batch.messages.len(), 2);
        assert_eq!(batch.messages[1].channel, Some("test".to_string()));
        assert_eq!(batch.batch_size, 2);
    }

    #[tokio::test]
    async fn test_client_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::TimedOut, "timeout");
        let client_error = ClientError::from(io_error);
        match client_error {
            ClientError::NetworkTimeout => {},
            _ => panic!("Expected NetworkTimeout"),
        }
    }
}