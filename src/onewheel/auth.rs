use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{Utc, Duration};
use crate::onewheel::models::OneWheelError;

/// Simple API key authentication
#[derive(Debug, Clone)]
pub struct ApiKey {
    pub key: String,
    pub user_id: String,
    pub created_at: chrono::DateTime<Utc>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
    pub is_active: bool,
}

/// Rate limiting tracker
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub requests: u32,
    pub window_start: chrono::DateTime<Utc>,
    pub max_requests: u32,
    pub window_duration: Duration,
}

/// Authentication manager
pub struct AuthManager {
    api_keys: HashMap<String, ApiKey>,
    rate_limits: HashMap<String, RateLimit>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            api_keys: HashMap::new(),
            rate_limits: HashMap::new(),
        }
    }

    /// Generate a new API key for a user
    pub fn generate_api_key(&mut self, user_id: String) -> String {
        let key = format!("ow_{}", Uuid::new_v4().to_string().replace("-", ""));
        let api_key = ApiKey {
            key: key.clone(),
            user_id,
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + Duration::days(365)), // 1 year expiry
            is_active: true,
        };
        
        self.api_keys.insert(key.clone(), api_key);
        key
    }

    /// Validate API key and return user ID
    pub fn validate_api_key(&self, key: &str) -> Result<String, OneWheelError> {
        if let Some(api_key) = self.api_keys.get(key) {
            if !api_key.is_active {
                return Err(OneWheelError::Auth("API key is disabled".to_string()));
            }

            if let Some(expires_at) = api_key.expires_at {
                if Utc::now() > expires_at {
                    return Err(OneWheelError::Auth("API key has expired".to_string()));
                }
            }

            Ok(api_key.user_id.clone())
        } else {
            Err(OneWheelError::Auth("Invalid API key".to_string()))
        }
    }

    /// Check rate limiting for a user
    pub fn check_rate_limit(&mut self, user_id: &str, max_requests: u32, window_minutes: i64) -> Result<(), OneWheelError> {
        let now = Utc::now();
        let window_duration = Duration::minutes(window_minutes);
        
        let rate_limit = self.rate_limits.entry(user_id.to_string()).or_insert_with(|| {
            RateLimit {
                requests: 0,
                window_start: now,
                max_requests,
                window_duration,
            }
        });

        // Reset window if it has expired
        if now - rate_limit.window_start > rate_limit.window_duration {
            rate_limit.requests = 0;
            rate_limit.window_start = now;
        }

        // Check if limit exceeded
        if rate_limit.requests >= rate_limit.max_requests {
            return Err(OneWheelError::Auth(format!(
                "Rate limit exceeded: {} requests per {} minutes", 
                max_requests, window_minutes
            )));
        }

        // Increment request count
        rate_limit.requests += 1;
        Ok(())
    }

    /// Add a demo API key for testing
    pub fn add_demo_key(&mut self) -> String {
        let demo_key = "ow_demo_12345678901234567890123456789012".to_string();
        let api_key = ApiKey {
            key: demo_key.clone(),
            user_id: "demo_user".to_string(),
            created_at: Utc::now(),
            expires_at: None, // Never expires for demo
            is_active: true,
        };
        
        self.api_keys.insert(demo_key.clone(), api_key);
        demo_key
    }
}

/// JWT token structure for more advanced authentication
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String, // User ID
    pub exp: usize,  // Expiry timestamp
    pub iat: usize,  // Issued at timestamp
    pub iss: String, // Issuer
}

impl Default for AuthManager {
    fn default() -> Self {
        Self::new()
    }
}
