use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub network: NetworkConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub limits: LimitsConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub bind_address: String,
    pub port: u16,
    pub max_clients: usize,
    pub tcp_keepalive: bool,
    pub keepalive_interval_secs: u64,
    pub heartbeat_interval_secs: u64,
    pub client_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub enabled: bool,
    pub url: String,
    pub pool_size: u32,
    pub message_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String, // "json" or "pretty"
    pub file_path: Option<String>,
    pub max_file_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    pub max_message_size: usize,
    pub message_rate_limit: u32,
    pub rate_limit_window_secs: u64,
    pub max_channels_per_user: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_tls: bool,
    pub cert_file: Option<String>,
    pub key_file: Option<String>,
    pub require_auth: bool,
    pub jwt_secret: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            network: NetworkConfig {
                bind_address: "0.0.0.0".to_string(),
                port: 80,
                max_clients: 100,
                tcp_keepalive: true,
                keepalive_interval_secs: 30,
                heartbeat_interval_secs: 30,
                client_timeout_secs: 90,
            },
            database: DatabaseConfig {
                enabled: true,
                url: "sqlite:./chat.db".to_string(),
                pool_size: 10,
                message_retention_days: 30,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
                file_path: Some("./logs/tcp-messaging.log".to_string()),
                max_file_size_mb: 100,
            },
            limits: LimitsConfig {
                max_message_size: 8192,
                message_rate_limit: 10,
                rate_limit_window_secs: 60,
                max_channels_per_user: 50,
            },
            security: SecurityConfig {
                enable_tls: false,
                cert_file: None,
                key_file: None,
                require_auth: false,
                jwt_secret: None,
            },
        }
    }
}

impl ServerConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: ServerConfig = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Load configuration from file or create default if file doesn't exist
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if path.as_ref().exists() {
            Self::from_file(path)
        } else {
            let config = Self::default();
            config.to_file(path)?;
            Ok(config)
        }
    }

    /// Validate configuration values
    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.network.port == 0 {
            return Err("Port cannot be 0".into());
        }
        
        if self.network.max_clients == 0 {
            return Err("Max clients must be greater than 0".into());
        }

        if self.limits.max_message_size == 0 {
            return Err("Max message size must be greater than 0".into());
        }

        if self.database.enabled && self.database.url.is_empty() {
            return Err("Database URL is required when database is enabled".into());
        }

        Ok(())
    }

    /// Get full bind address (IP:port)
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.network.bind_address, self.network.port)
    }

    /// Get heartbeat interval as Duration
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_secs(self.network.heartbeat_interval_secs)
    }

    /// Get client timeout as Duration
    pub fn client_timeout(&self) -> Duration {
        Duration::from_secs(self.network.client_timeout_secs)
    }

    /// Get rate limit window as Duration
    pub fn rate_limit_window(&self) -> Duration {
        Duration::from_secs(self.limits.rate_limit_window_secs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server_address: String,
    pub port: u16,
    pub username: Option<String>,
    pub auto_reconnect: bool,
    pub max_retries: u32,
    pub timeout_secs: u64,
    pub logging: LoggingConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_address: "192.168.0.112".to_string(),
            port: 80,
            username: None,
            auto_reconnect: true,
            max_retries: 5,
            timeout_secs: 30,
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "pretty".to_string(),
                file_path: None,
                max_file_size_mb: 10,
            },
        }
    }
}

impl ClientConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: ClientConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        if path.as_ref().exists() {
            Self::from_file(path)
        } else {
            let config = Self::default();
            let content = toml::to_string_pretty(&config)?;
            fs::write(path, content)?;
            Ok(config)
        }
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server_address, self.port)
    }

    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }
}
