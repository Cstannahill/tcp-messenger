use std::env;
use std::process;

mod server;
mod client;
mod config;
mod database;
mod shutdown;
mod file_transfer;
mod onewheel;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  {} server", args[0]);
        eprintln!("  {} client [username] [address]", args[0]);
        eprintln!("  {} onewheel", args[0]);
        process::exit(1);
    }

    match args[1].as_str() {
        "onewheel" => {
            if let Err(e) = run_onewheel_api().await {
                eprintln!("OneWheel API server failed: {}", e);
                process::exit(1);
            }
        }
        "server" => {
            if let Err(e) = run_server() {
                eprintln!("Server failed: {}", e);
                process::exit(1);
            }
        }
        "client" => {
            let username = args.get(2).cloned().unwrap_or_else(|| {
                print!("Enter your username: ");
                use std::io::{stdin, stdout, Write};
                stdout().flush().unwrap();
                let mut input = String::new();
                stdin().read_line(&mut input).unwrap();
                input.trim().to_string()
            });

            let address = args.get(3).cloned();

            let rt = tokio::runtime::Runtime::new().unwrap();
            if let Err(e) = rt.block_on(run_client(username, address)) {
                eprintln!("Client failed: {}", e);
                process::exit(1);
            }
        }
        _ => {
            eprintln!("Invalid mode: {}", args[1]);
            process::exit(1);
        }
    }
}

async fn run_onewheel_api() -> Result<(), Box<dyn std::error::Error>> {
    use onewheel::OneWheelApi;

    // Initialize logging
    tracing_subscriber::fmt::init();

    let api = OneWheelApi::new();

    // Use environment variable or skip database for now
    let database_url = env::var("DATABASE_URL").ok();

    if let Some(url) = database_url {
        if let Err(e) = api.init_database(&url).await {
            eprintln!("Warning: Database initialization failed: {}", e);
            eprintln!("API will continue without database persistence");
        }
    } else {
        println!("No DATABASE_URL provided, running without persistence");
    }

    // Start server on port 3001 (different from TCP chat server)
    let address = "0.0.0.0:3001";
    println!("Starting OneWheel API server on {}", address);
    println!("Demo API key: ow_demo_12345678901234567890123456789012");
    println!();
    println!("Available endpoints:");
    println!("  POST /api/rides           - Upload ride data");
    println!("  POST /api/analyze-ride    - Analyze ride");
    println!("  GET  /api/users/:id/stats - Get user statistics");
    println!("  GET  /health              - Health check");
    println!();

    api.start(address).await?;
    Ok(())
}

fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Starting TCP messaging server...");
    println!("Note: Using default configuration for now");

    // Use the existing server implementation
    let config = server::ServerConfig {
        bind_address: "0.0.0.0:10420".to_string(),
        max_clients: 100,
        heartbeat_interval: std::time::Duration::from_secs(30),
        client_timeout: std::time::Duration::from_secs(300),
        message_rate_limit: 10,
        rate_limit_window: std::time::Duration::from_secs(60),
        max_message_size: 4096,
    };

    let chat_server = server::ChatServer::new(config);
    chat_server.run()
}

async fn run_client(username: String, address: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to server...");

    let addr = address.unwrap_or_else(|| "192.168.0.112:80".to_string());

    let mut client = client::ChatClient::new(username, addr);

    client.connect().await?;
    client.start_interactive_session().await?;
    Ok(())
}
