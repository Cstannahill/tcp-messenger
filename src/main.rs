use std::env;
use std::process;

mod server;
mod client;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  {} server", args[0]);
        eprintln!("  {} client [username] [address]", args[0]);
        process::exit(1);
    }

    match args[1].as_str() {
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

            let address = args.get(3).cloned().unwrap_or_else(|| "192.168.0.112:80".to_string());

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

fn run_server() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config = server::ServerConfig::default();
    let chat_server = server::ChatServer::new(config);

    chat_server.run()
}

async fn run_client(username: String, address: String) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = client::ChatClient::new(username, address);
    client.connect().await?;
    client.start_interactive_session().await?;
    Ok(())
}
