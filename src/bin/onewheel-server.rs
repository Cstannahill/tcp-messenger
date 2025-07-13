use tcp_messaging::onewheel::OneWheelApi;
use tracing_subscriber;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    
    // Initialize OneWheel API and database by default
    let api = OneWheelApi::new();
    
    // Use environment variable or default database URL
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:password@localhost/onewheel".to_string());
    
    // Always try to initialize database (unless explicitly disabled)
    if !database_url.is_empty() && database_url != "none" {
        match api.init_database(&database_url).await {
            Ok(_) => {
                println!("✅ Database initialized successfully");
            }
            Err(e) => {
                eprintln!("⚠️  Warning: Database initialization failed: {}", e);
                eprintln!("   API will continue without database persistence");
                eprintln!("   To fix this, ensure PostgreSQL is running and DATABASE_URL is correct");
            }
        }
    } else {
        println!("ℹ️  Database disabled (DATABASE_URL=none)");
    }
    
    match args.get(1).map(|s| s.as_str()) {
        Some("--help") | Some("-h") | Some("help") => {
            println!("OneWheel API Server");
            println!();
            println!("USAGE:");
            println!("  {}                        - Start OneWheel API server (default)", args[0]);
            println!("  {} server                 - Start TCP chat server", args[0]);
            println!("  {} client [user] [addr]   - Start TCP chat client", args[0]);
            println!("  {} --help                 - Show this help", args[0]);
            println!();
            println!("ONEWHEEL API (Default Mode):");
            println!("  🛹 REST API server on port 8080");
            println!("  🔑 Demo API key: ow_demo_12345678901234567890123456789012");
            println!("  📖 See API_USAGE_GUIDE.md for detailed documentation");
            println!();
            println!("ENVIRONMENT VARIABLES:");
            println!("  DATABASE_URL              - PostgreSQL connection string");
            println!("                              Default: postgresql://postgres:password@localhost/onewheel");
            println!("                              Set to 'none' to disable database");
            println!();
            println!("EXAMPLES:");
            println!("  {}                        # Start OneWheel API (default)", args[0]);
            println!("  DATABASE_URL=none {}      # Start without database", args[0]);
            println!("  {} server                 # Start TCP chat server instead", args[0]);
            return Ok(());
        }
        Some("server") => {
            // Start regular TCP chat server (now on port 10420)
            let mut config = tcp_messaging::server::ServerConfig::default();
            config.bind_address = "0.0.0.0:10420".to_string();
            let server = tcp_messaging::server::ChatServer::new(config);
            server.run()?;
        }
        Some("client") => {
            // Start TCP chat client (default to port 10420)
            let username = args.get(2).unwrap_or(&"user".to_string()).clone();
            let address = args.get(3).unwrap_or(&"192.168.0.112:10420".to_string()).clone();
            
            let mut client = tcp_messaging::client::ChatClient::new(username, address);
            client.run_with_auto_reconnect().await;
        }
        // Default behavior: Start OneWheel API server
        _ => {
            // Start server on port 80
            let address = "0.0.0.0:80";
            println!();
            println!("🛹 Starting OneWheel API server on {}", address);
            println!("🔑 Demo API key: ow_demo_12345678901234567890123456789012");
            println!();
            println!("📡 Available endpoints:");
            println!("  POST /api/rides           - Upload ride data");
            println!("  GET  /api/rides/:ride_id  - Get specific ride");
            println!("  GET  /api/rides           - List user rides");
            println!("  POST /api/analyze-ride    - Analyze ride");
            println!("  GET  /api/users/:id/stats - Get user statistics");
            println!("  GET  /health              - Health check");
            println!();
            println!("📖 For detailed API documentation, see API_USAGE_GUIDE.md");
            println!();
            
            api.start(address).await?;
        }
    }

    Ok(())
}
