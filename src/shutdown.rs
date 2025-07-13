use signal_hook_tokio::Signals;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, error};
use futures::stream::StreamExt;

#[derive(Clone)]
pub struct ShutdownHandler {
    shutdown_tx: broadcast::Sender<()>,
    is_shutting_down: Arc<AtomicBool>,
}

impl ShutdownHandler {
    pub fn new() -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);
        let is_shutting_down = Arc::new(AtomicBool::new(false));
        
        Self {
            shutdown_tx,
            is_shutting_down,
        }
    }

    /// Start listening for shutdown signals
    pub async fn listen_for_signals(&self) {
        let shutdown_tx = self.shutdown_tx.clone();
        let is_shutting_down = Arc::clone(&self.is_shutting_down);

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use signal_hook::consts::{SIGINT, SIGTERM};
                
                let signals = Signals::new(&[SIGINT, SIGTERM]);
                match signals {
                    Ok(mut signals) => {
                        info!("Shutdown signal handler initialized (SIGINT, SIGTERM)");
                        
                        if let Some(signal) = signals.next().await {
                            match signal {
                                SIGINT => info!("Received SIGINT, initiating graceful shutdown..."),
                                SIGTERM => info!("Received SIGTERM, initiating graceful shutdown..."),
                                _ => info!("Received signal {}, initiating graceful shutdown...", signal),
                            }
                            
                            is_shutting_down.store(true, Ordering::SeqCst);
                            
                            if let Err(e) = shutdown_tx.send(()) {
                                error!("Failed to send shutdown signal: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to set up signal handler: {}", e);
                    }
                }
            }

            #[cfg(windows)]
            {
                use tokio::signal;
                
                info!("Shutdown signal handler initialized (Ctrl+C)");
                if let Err(e) = signal::ctrl_c().await {
                    error!("Failed to listen for Ctrl+C: {}", e);
                } else {
                    info!("Received Ctrl+C, initiating graceful shutdown...");
                    is_shutting_down.store(true, Ordering::SeqCst);
                    
                    if let Err(e) = shutdown_tx.send(()) {
                        error!("Failed to send shutdown signal: {}", e);
                    }
                }
            }
        });
    }

    /// Get a receiver for shutdown notifications
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Trigger shutdown manually
    pub fn trigger_shutdown(&self) {
        info!("Manual shutdown triggered");
        self.is_shutting_down.store(true, Ordering::SeqCst);
        let _ = self.shutdown_tx.send(());
    }

    /// Check if shutdown is in progress
    pub fn is_shutting_down(&self) -> bool {
        self.is_shutting_down.load(Ordering::SeqCst)
    }

    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&self) {
        let mut receiver = self.subscribe();
        let _ = receiver.recv().await;
    }
}

impl Default for ShutdownHandler {
    fn default() -> Self {
        Self::new()
    }
}
