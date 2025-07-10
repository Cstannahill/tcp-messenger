use std::io::{Read, Write}; // Added Read trait import
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    sender: String,
    content: String,
}

// Helper struct to store client info
#[derive(Debug)]
struct Client {
    id: usize,
    stream: TcpStream,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:10210")?;
    println!("Listening on 127.0.0.1:10210");

    // Shared state for managing clients - changed to store Client structs
    let clients: Arc<Mutex<Vec<Client>>> = Arc::new(Mutex::new(Vec::new()));

    for stream_result in listener.incoming() {
        match stream_result {
            Ok(stream) => {
                println!("New connection: {:?}", stream);
                let clients_clone = Arc::clone(&clients);
                let stream_clone = stream.try_clone().unwrap();
                
                // Get client ID before spawning thread
                let client_id = {
                    let mut clients_lock = clients_clone.lock().unwrap();
                    let id = clients_lock.len();
                    clients_lock.push(Client {
                        id,
                        stream: stream.try_clone().unwrap(),
                    });
                    id
                };

                // Handle client in a separate thread
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream_clone, client_id, clients_clone) {
                        eprintln!("Error handling client {}: {}", client_id, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_client(
    mut stream: TcpStream,
    client_id: usize,
    clients: Arc<Mutex<Vec<Client>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let mut buffer = [0; 1024];
        match stream.read(&mut buffer) {
            Ok(n) => {
                if n == 0 {
                    println!("Client {} disconnected", client_id);
                    // Remove client from the list
                    let mut clients_lock = clients.lock().unwrap();
                    clients_lock.retain(|client| client.id != client_id);
                    break;
                }

                let received = String::from_utf8_lossy(&buffer[..n]);

                // Attempt to parse the JSON message
                let message: Message = match serde_json::from_str(&received) {
                    Ok(msg) => msg,
                    Err(e) => {
                        eprintln!("Error parsing JSON from client {}: {}", client_id, e);
                        continue;
                    }
                };

                println!("Received from client {}: {:?}", client_id, message);

                // Broadcast the message to all other clients
                broadcast_message(&message, client_id, &clients)?;
            }
            Err(e) => {
                eprintln!("Error reading from client {}: {}", client_id, e);
                // Remove client from the list
                let mut clients_lock = clients.lock().unwrap();
                clients_lock.retain(|client| client.id != client_id);
                break;
            }
        }
    }
    Ok(())
}

fn broadcast_message(
    message: &Message,
    sender_id: usize,
    clients: &Arc<Mutex<Vec<Client>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let message_json = serde_json::to_string(message)?;
    let mut clients_lock = clients.lock().unwrap();
    
    // Remove disconnected clients while broadcasting
    clients_lock.retain_mut(|client| {
        if client.id != sender_id {
            match client.stream.write_all(message_json.as_bytes()) {
                Ok(_) => {
                    match client.stream.flush() {
                        Ok(_) => true, // Keep client
                        Err(_) => false, // Remove client
                    }
                }
                Err(_) => false, // Remove client
            }
        } else {
            true // Keep sender
        }
    });
    
    Ok(())
}