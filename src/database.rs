use sqlx::{Pool, Row, Sqlite, SqlitePool};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::collections::HashMap;
use tracing::{info, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: String,
    pub sender: String,
    pub content: String,
    pub channel: Option<String>,
    pub message_type: String,
    pub metadata: Option<String>, // JSON string
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredUser {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub is_active: bool,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChannel {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_private: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    /// Create a new database connection
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        info!("Connecting to database: {}", database_url);
        
        let pool = SqlitePool::connect(database_url).await?;
        
        let db = Self { pool };
        db.run_migrations().await?;
        
        info!("Database connection established");
        Ok(db)
    }

    /// Run database migrations
    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        info!("Running database migrations");

        // Create users table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT UNIQUE NOT NULL,
                email TEXT,
                display_name TEXT,
                is_active BOOLEAN NOT NULL DEFAULT TRUE,
                last_seen DATETIME NOT NULL,
                created_at DATETIME NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create channels table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                description TEXT,
                is_private BOOLEAN NOT NULL DEFAULT FALSE,
                created_by TEXT NOT NULL,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (created_by) REFERENCES users (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create messages table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                sender TEXT NOT NULL,
                content TEXT NOT NULL,
                channel TEXT,
                message_type TEXT NOT NULL,
                metadata TEXT,
                created_at DATETIME NOT NULL,
                FOREIGN KEY (sender) REFERENCES users (username),
                FOREIGN KEY (channel) REFERENCES channels (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages (created_at)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_channel ON messages (channel)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_messages_sender ON messages (sender)")
            .execute(&self.pool)
            .await?;

        info!("Database migrations completed");
        Ok(())
    }

    /// Store a new message
    pub async fn store_message(
        &self,
        sender: &str,
        content: &str,
        channel: Option<&str>,
        message_type: &str,
        metadata: Option<&HashMap<String, String>>,
    ) -> Result<String, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let metadata_json = metadata.map(|m| serde_json::to_string(m).unwrap_or_default());
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO messages (id, sender, content, channel, message_type, metadata, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(sender)
        .bind(content)
        .bind(channel)
        .bind(message_type)
        .bind(metadata_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        debug!("Stored message {} from {} in channel {:?}", id, sender, channel);
        Ok(id)
    }

    /// Get recent messages from a channel
    pub async fn get_recent_messages(
        &self,
        channel: Option<&str>,
        limit: i32,
    ) -> Result<Vec<StoredMessage>, sqlx::Error> {
        let rows = if let Some(channel) = channel {
            sqlx::query(
                r#"
                SELECT id, sender, content, channel, message_type, metadata, created_at
                FROM messages
                WHERE channel = ?
                ORDER BY created_at DESC
                LIMIT ?
                "#,
            )
            .bind(channel)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, sender, content, channel, message_type, metadata, created_at
                FROM messages
                WHERE channel IS NULL
                ORDER BY created_at DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let mut messages = Vec::new();
        for row in rows {
            messages.push(StoredMessage {
                id: row.get("id"),
                sender: row.get("sender"),
                content: row.get("content"),
                channel: row.get("channel"),
                message_type: row.get("message_type"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
            });
        }

        // Reverse to get chronological order
        messages.reverse();
        Ok(messages)
    }

    /// Create or update a user
    pub async fn upsert_user(&self, username: &str) -> Result<String, sqlx::Error> {
        let user_id = Uuid::new_v4().to_string();
        let now = Utc::now();

        // Try to update existing user
        let result = sqlx::query(
            "UPDATE users SET last_seen = ?, is_active = TRUE WHERE username = ?"
        )
        .bind(now)
        .bind(username)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            // Create new user
            sqlx::query(
                r#"
                INSERT INTO users (id, username, is_active, last_seen, created_at)
                VALUES (?, ?, TRUE, ?, ?)
                "#,
            )
            .bind(&user_id)
            .bind(username)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;

            debug!("Created new user: {}", username);
        } else {
            debug!("Updated existing user: {}", username);
        }

        Ok(user_id)
    }

    /// Mark user as offline
    pub async fn mark_user_offline(&self, username: &str) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        
        sqlx::query(
            "UPDATE users SET is_active = FALSE, last_seen = ? WHERE username = ?"
        )
        .bind(now)
        .bind(username)
        .execute(&self.pool)
        .await?;

        debug!("Marked user {} as offline", username);
        Ok(())
    }

    /// Get all active users
    pub async fn get_active_users(&self) -> Result<Vec<StoredUser>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, username, email, display_name, is_active, last_seen, created_at
            FROM users
            WHERE is_active = TRUE
            ORDER BY username
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut users = Vec::new();
        for row in rows {
            users.push(StoredUser {
                id: row.get("id"),
                username: row.get("username"),
                email: row.get("email"),
                display_name: row.get("display_name"),
                is_active: row.get("is_active"),
                last_seen: row.get("last_seen"),
                created_at: row.get("created_at"),
            });
        }

        Ok(users)
    }

    /// Create a new channel
    pub async fn create_channel(
        &self,
        name: &str,
        description: Option<&str>,
        is_private: bool,
        created_by: &str,
    ) -> Result<String, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO channels (id, name, description, is_private, created_by, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(is_private)
        .bind(created_by)
        .bind(now)
        .execute(&self.pool)
        .await?;

        info!("Created channel '{}' (id: {}) by {}", name, id, created_by);
        Ok(id)
    }

    /// Get all channels (public only for most users)
    pub async fn get_channels(&self, include_private: bool) -> Result<Vec<StoredChannel>, sqlx::Error> {
        let rows = if include_private {
            sqlx::query(
                r#"
                SELECT id, name, description, is_private, created_by, created_at
                FROM channels
                ORDER BY name
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, name, description, is_private, created_by, created_at
                FROM channels
                WHERE is_private = FALSE
                ORDER BY name
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        let mut channels = Vec::new();
        for row in rows {
            channels.push(StoredChannel {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                is_private: row.get("is_private"),
                created_by: row.get("created_by"),
                created_at: row.get("created_at"),
            });
        }

        Ok(channels)
    }

    /// Clean up old messages (for data retention)
    pub async fn cleanup_old_messages(&self, retention_days: u32) -> Result<u64, sqlx::Error> {
        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);
        
        let result = sqlx::query(
            "DELETE FROM messages WHERE created_at < ?"
        )
        .bind(cutoff_date)
        .execute(&self.pool)
        .await?;

        let deleted_count = result.rows_affected();
        if deleted_count > 0 {
            info!("Cleaned up {} old messages (older than {} days)", deleted_count, retention_days);
        }

        Ok(deleted_count)
    }

    /// Get database statistics
    pub async fn get_stats(&self) -> Result<HashMap<String, i64>, sqlx::Error> {
        let mut stats = HashMap::new();

        // Count messages
        let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM messages")
            .fetch_one(&self.pool)
            .await?;
        stats.insert("total_messages".to_string(), message_count);

        // Count active users
        let active_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_active = TRUE")
            .fetch_one(&self.pool)
            .await?;
        stats.insert("active_users".to_string(), active_users);

        // Count total users
        let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        stats.insert("total_users".to_string(), total_users);

        // Count channels
        let channel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM channels")
            .fetch_one(&self.pool)
            .await?;
        stats.insert("total_channels".to_string(), channel_count);

        Ok(stats)
    }
}
