use tokio_postgres::{Client, NoTls, types::ToSql};
use crate::onewheel::models::*;
use std::collections::HashMap;
use tracing::{info, error};
use serde_json;

/// Database interface for OneWheel ride data
pub struct OneWheelDatabase {
    client: Option<Client>,
}

impl OneWheelDatabase {
    /// List all users (stub implementation)
    pub async fn list_users(&self) -> Result<Vec<String>, OneWheelError> {
        let client = match &self.client {
            Some(c) => c,
            None => return Err(OneWheelError::Database("No database connection".to_string())),
        };
        let query = "SELECT id FROM users LIMIT 100";
        let rows = client.query(query, &[]).await.map_err(|e| OneWheelError::Database(e.to_string()))?;
        Ok(rows.iter().map(|row| row.get::<_, String>(0)).collect())
    }
    pub fn new() -> Self {
        Self { client: None }
    }

    /// Initialize database connection
    pub async fn connect(&mut self, database_url: &str) -> Result<(), OneWheelError> {
        match tokio_postgres::connect(database_url, NoTls).await {
            Ok((client, connection)) => {
                // Spawn the connection in the background
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        error!("Database connection error: {}", e);
                    }
                });
                
                self.client = Some(client);
                info!("Connected to OneWheel database");
                
                // Initialize schema if needed
                self.init_schema().await?;
                
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to database: {}", e);
                Err(OneWheelError::Database(e.to_string()))
            }
        }
    }

    /// Initialize database schema
    async fn init_schema(&self) -> Result<(), OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        // Create rides table
        let create_rides_table = r#"
            CREATE TABLE IF NOT EXISTS rides (
                id VARCHAR(255) PRIMARY KEY,
                user_id VARCHAR(255) NOT NULL,
                start_time BIGINT NOT NULL,
                end_time BIGINT,
                distance DOUBLE PRECISION,
                max_speed DOUBLE PRECISION,
                avg_speed DOUBLE PRECISION,
                duration INTEGER,
                route JSONB,
                start_battery INTEGER,
                end_battery INTEGER,
                notes TEXT,
                metadata JSONB,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        "#;

        // Create ride insights table
        let create_insights_table = r#"
            CREATE TABLE IF NOT EXISTS ride_insights (
                id SERIAL PRIMARY KEY,
                ride_id VARCHAR(255) REFERENCES rides(id) ON DELETE CASCADE,
                overall_score DOUBLE PRECISION,
                speed_efficiency DOUBLE PRECISION,
                energy_efficiency DOUBLE PRECISION,
                route_quality DOUBLE PRECISION,
                ride_style VARCHAR(100),
                insights JSONB,
                suggestions JSONB,
                comparisons JSONB,
                ai_metrics JSONB,
                mood_analysis TEXT,
                safety_tips JSONB,
                skill_progression JSONB,
                analysis_type VARCHAR(20) DEFAULT 'ai',
                generated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                processing_time_ms INTEGER
            )
        "#;

        // Create users table
        let create_users_table = r#"
            CREATE TABLE IF NOT EXISTS users (
                id VARCHAR(255) PRIMARY KEY,
                email VARCHAR(255) UNIQUE,
                api_key VARCHAR(255) UNIQUE,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                last_active TIMESTAMP,
                settings JSONB
            )
        "#;

        // Create indexes
        let create_indexes = vec![
            "CREATE INDEX IF NOT EXISTS idx_rides_user_id ON rides(user_id)",
            "CREATE INDEX IF NOT EXISTS idx_rides_start_time ON rides(start_time)",
            "CREATE INDEX IF NOT EXISTS idx_rides_created_at ON rides(created_at)",
            "CREATE INDEX IF NOT EXISTS idx_insights_ride_id ON ride_insights(ride_id)",
            "CREATE INDEX IF NOT EXISTS idx_insights_generated_at ON ride_insights(generated_at)",
        ];

        // Execute schema creation
        client.execute(create_rides_table, &[]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        
        client.execute(create_insights_table, &[]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        
        client.execute(create_users_table, &[]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;

        for index_sql in create_indexes {
            client.execute(index_sql, &[]).await
                .map_err(|e| OneWheelError::Database(e.to_string()))?;
        }

        // Test the connection with a simple query
        match client.query_one("SELECT 1 as test", &[]).await {
            Ok(_) => info!("Database connection test successful"),
            Err(e) => {
                error!("Database connection test failed: {}", e);
                return Err(OneWheelError::Database(format!("Connection test failed: {}", e)));
            }
        }

        info!("Database schema initialized successfully");
        Ok(())
    }

    /// Store ride data
    pub async fn store_ride(&self, ride_data: &RideData, user_id: &str) -> Result<(), OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        // Convert Option types to JSON strings for JSONB fields
        let route_json_str = ride_data.route.as_ref()
            .map(|r| serde_json::to_string(r).unwrap_or_else(|_| "null".to_string()));

        let metadata_json_str = ride_data.metadata.as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "null".to_string()));

        let insert_sql = r#"
            INSERT INTO rides (
                id, user_id, start_time, end_time, distance, max_speed, avg_speed,
                duration, route, start_battery, end_battery, notes, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10, $11, $12, $13::jsonb)
            ON CONFLICT (id) DO UPDATE SET
                end_time = EXCLUDED.end_time,
                distance = EXCLUDED.distance,
                max_speed = EXCLUDED.max_speed,
                avg_speed = EXCLUDED.avg_speed,
                duration = EXCLUDED.duration,
                route = EXCLUDED.route,
                start_battery = EXCLUDED.start_battery,
                end_battery = EXCLUDED.end_battery,
                notes = EXCLUDED.notes,
                metadata = EXCLUDED.metadata,
                updated_at = CURRENT_TIMESTAMP
        "#;

        client.execute(insert_sql, &[
            &ride_data.id,
            &user_id,
            &ride_data.start_time.timestamp(),
            &ride_data.end_time.map(|dt| dt.timestamp()),
            &ride_data.distance,
            &ride_data.max_speed,
            &ride_data.avg_speed,
            &ride_data.duration.map(|d| d as i32),
            &route_json_str.as_deref().unwrap_or("null"),
            &ride_data.start_battery.map(|b| b as i32),
            &ride_data.end_battery.map(|b| b as i32),
            &ride_data.notes,
            &metadata_json_str.as_deref().unwrap_or("null"),
        ]).await.map_err(|e| OneWheelError::Database(e.to_string()))?;

        info!("Stored ride data for ride ID: {}", ride_data.id);
        Ok(())
    }

    /// Store ride analysis
    pub async fn store_analysis(&self, analysis: &RideAnalysis, processing_time_ms: u32) -> Result<(), OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        let insights_json = serde_json::to_string(&analysis.insights)
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        let suggestions_json = serde_json::to_string(&analysis.suggestions)
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        let comparisons_json = serde_json::to_string(&analysis.comparisons)
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        let ai_metrics_json = serde_json::to_string(&analysis.ai_metrics)
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        let safety_tips_json = serde_json::to_string(&analysis.safety_tips)
            .map_err(|e| OneWheelError::Database(e.to_string()))?;
        let skill_progression_json = serde_json::to_string(&analysis.skill_progression)
            .map_err(|e| OneWheelError::Database(e.to_string()))?;

        let insert_sql = r#"
            INSERT INTO ride_insights (
                ride_id, overall_score, speed_efficiency, energy_efficiency, route_quality,
                ride_style, insights, suggestions, comparisons, ai_metrics, mood_analysis,
                safety_tips, skill_progression, generated_at, processing_time_ms
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            ON CONFLICT (ride_id) DO UPDATE SET
                overall_score = EXCLUDED.overall_score,
                speed_efficiency = EXCLUDED.speed_efficiency,
                energy_efficiency = EXCLUDED.energy_efficiency,
                route_quality = EXCLUDED.route_quality,
                ride_style = EXCLUDED.ride_style,
                insights = EXCLUDED.insights,
                suggestions = EXCLUDED.suggestions,
                comparisons = EXCLUDED.comparisons,
                ai_metrics = EXCLUDED.ai_metrics,
                mood_analysis = EXCLUDED.mood_analysis,
                safety_tips = EXCLUDED.safety_tips,
                skill_progression = EXCLUDED.skill_progression,
                generated_at = EXCLUDED.generated_at,
                processing_time_ms = EXCLUDED.processing_time_ms
        "#;

        client.execute(insert_sql, &[
            &analysis.ride_id,
            &analysis.overall_score,
            &analysis.speed_efficiency,
            &analysis.energy_efficiency,
            &analysis.route_quality,
            &analysis.ride_style,
            &insights_json,
            &suggestions_json,
            &comparisons_json,
            &ai_metrics_json,
            &analysis.mood_analysis,
            &safety_tips_json,
            &skill_progression_json,
            &analysis.generated_at.timestamp(),
            &(processing_time_ms as i32),
        ]).await.map_err(|e| OneWheelError::Database(e.to_string()))?;

        info!("Stored analysis for ride ID: {}", analysis.ride_id);
        Ok(())
    }

    /// Get ride by ID
    pub async fn get_ride(&self, ride_id: &str) -> Result<Option<RideData>, OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        let query = "SELECT * FROM rides WHERE id = $1";
        let rows = client.query(query, &[&ride_id]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;

        if let Some(row) = rows.first() {
            let route_str: Option<String> = row.get("route");
            let metadata_str: Option<String> = row.get("metadata");
            
            let start_timestamp: i64 = row.get("start_time");
            let end_timestamp: Option<i64> = row.get("end_time");

            let ride_route = route_str.and_then(|s| serde_json::from_str(&s).ok());
            let ride_metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

            use chrono::{DateTime, Utc, TimeZone};
            let start_time = Utc.timestamp_opt(start_timestamp, 0).single()
                .ok_or_else(|| OneWheelError::Database("Invalid start_time timestamp".to_string()))?;
            let end_time = end_timestamp.and_then(|ts| Utc.timestamp_opt(ts, 0).single());

            Ok(Some(RideData {
                id: row.get("id"),
                user_id: Some(row.get("user_id")),
                start_time,
                end_time,
                distance: row.get("distance"),
                max_speed: row.get("max_speed"),
                avg_speed: row.get("avg_speed"),
                duration: row.get::<_, Option<i32>>("duration").map(|d| d as u32),
                route: ride_route,
                start_battery: row.get::<_, Option<i32>>("start_battery").map(|b| b as u8),
                end_battery: row.get::<_, Option<i32>>("end_battery").map(|b| b as u8),
                notes: row.get("notes"),
                metadata: ride_metadata,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get analysis by ride ID
    pub async fn get_analysis(&self, ride_id: &str) -> Result<Option<RideAnalysis>, OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        let query = "SELECT * FROM ride_insights WHERE ride_id = $1 ORDER BY generated_at DESC LIMIT 1";
        let rows = client.query(query, &[&ride_id]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;

        if let Some(row) = rows.first() {
            let insights_str: String = row.get("insights");
            let suggestions_str: String = row.get("suggestions");
            let comparisons_str: String = row.get("comparisons");
            let ai_metrics_str: String = row.get("ai_metrics");
            let safety_tips_str: String = row.get("safety_tips");
            let skill_progression_str: String = row.get("skill_progression");
            let generated_timestamp: i64 = row.get("generated_at");
            
            use chrono::{DateTime, Utc, TimeZone};
            let generated_at = Utc.timestamp_opt(generated_timestamp, 0).single()
                .ok_or_else(|| OneWheelError::Database("Invalid generated_at timestamp".to_string()))?;

            Ok(Some(RideAnalysis {
                ride_id: row.get("ride_id"),
                overall_score: row.get("overall_score"),
                speed_efficiency: row.get("speed_efficiency"),
                energy_efficiency: row.get("energy_efficiency"),
                route_quality: row.get("route_quality"),
                ride_style: row.get("ride_style"),
                insights: serde_json::from_str(&insights_str).unwrap_or_default(),
                suggestions: serde_json::from_str(&suggestions_str).unwrap_or_default(),
                comparisons: serde_json::from_str(&comparisons_str).unwrap_or_default(),
                generated_at,
                ai_metrics: serde_json::from_str(&ai_metrics_str).unwrap_or_default(),
                mood_analysis: row.get("mood_analysis"),
                safety_tips: serde_json::from_str(&safety_tips_str).unwrap_or_default(),
                skill_progression: serde_json::from_str(&skill_progression_str).unwrap_or_default(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get user statistics
    pub async fn get_user_stats(&self, user_id: &str) -> Result<UserStats, OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        // Get basic stats
        let stats_query = r#"
            SELECT 
                COUNT(*) as total_rides,
                COALESCE(SUM(distance), 0) as total_distance,
                COALESCE(SUM(duration), 0) as total_time,
                COALESCE(AVG(avg_speed), 0) as average_speed
            FROM rides 
            WHERE user_id = $1 AND distance IS NOT NULL
        "#;

        let stats_row = client.query_one(stats_query, &[&user_id]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;

        // Get best ride
        let best_ride_query = r#"
            SELECT r.id, ri.overall_score, r.start_time
            FROM rides r
            JOIN ride_insights ri ON r.id = ri.ride_id
            WHERE r.user_id = $1
            ORDER BY ri.overall_score DESC
            LIMIT 1
        "#;

        let best_ride = match client.query_opt(best_ride_query, &[&user_id]).await {
            Ok(Some(row)) => {
                let timestamp: i64 = row.get("start_time");
                use chrono::{DateTime, Utc, TimeZone};
                let date = Utc.timestamp_opt(timestamp, 0).single()
                    .ok_or_else(|| OneWheelError::Database("Invalid start_time timestamp".to_string()))?;
                Some(BestRide {
                    ride_id: row.get("id"),
                    score: row.get("overall_score"),
                    date,
                })
            },
            _ => None,
        };

        // For now, return basic stats with placeholder skill trends
        let mut skill_trends = HashMap::new();
        skill_trends.insert("balance".to_string(), SkillTrend {
            current: 75.0,
            trend: "+2.1".to_string(),
        });
        skill_trends.insert("efficiency".to_string(), SkillTrend {
            current: 82.0,
            trend: "+5.3".to_string(),
        });

        Ok(UserStats {
            total_rides: stats_row.get::<_, i64>("total_rides") as u32,
            total_distance: stats_row.get("total_distance"),
            total_time: stats_row.get::<_, i64>("total_time") as u32,
            average_speed: stats_row.get("average_speed"),
            best_ride,
            skill_trends,
        })
    }

    /// Get all rides for a user with pagination
    pub async fn get_user_rides(&self, user_id: &str, limit: Option<u32>, offset: Option<u32>) -> Result<Vec<RideData>, OneWheelError> {
        let client = self.client.as_ref().ok_or_else(|| {
            OneWheelError::Database("No database connection".to_string())
        })?;

        let limit = limit.unwrap_or(50).min(100) as i64; // Max 100 rides per request
        let offset = offset.unwrap_or(0) as i64;

        let query = r#"
            SELECT * FROM rides 
            WHERE user_id = $1 
            ORDER BY start_time DESC 
            LIMIT $2 OFFSET $3
        "#;
        
        let rows = client.query(query, &[&user_id, &limit, &offset]).await
            .map_err(|e| OneWheelError::Database(e.to_string()))?;

        let mut rides = Vec::new();
        for row in rows {
            let route_str: Option<String> = row.get("route");
            let metadata_str: Option<String> = row.get("metadata");
            
            let start_timestamp: i64 = row.get("start_time");
            let end_timestamp: Option<i64> = row.get("end_time");

            let ride_route = route_str.and_then(|s| serde_json::from_str(&s).ok());
            let ride_metadata = metadata_str.and_then(|s| serde_json::from_str(&s).ok());

            use chrono::{DateTime, Utc, TimeZone};
            let start_time = Utc.timestamp_opt(start_timestamp, 0).single()
                .ok_or_else(|| OneWheelError::Database("Invalid start_time timestamp".to_string()))?;
            let end_time = end_timestamp.and_then(|ts| Utc.timestamp_opt(ts, 0).single());

            rides.push(RideData {
                id: row.get("id"),
                user_id: Some(row.get("user_id")),
                start_time,
                end_time,
                distance: row.get("distance"),
                max_speed: row.get("max_speed"),
                avg_speed: row.get("avg_speed"),
                duration: row.get::<_, Option<i32>>("duration").map(|d| d as u32),
                route: ride_route,
                start_battery: row.get::<_, Option<i32>>("start_battery").map(|b| b as u8),
                end_battery: row.get::<_, Option<i32>>("end_battery").map(|b| b as u8),
                notes: row.get("notes"),
                metadata: ride_metadata,
            });
        }

        Ok(rides)
    }

}

impl Default for AiMetrics {
    fn default() -> Self {
        Self {
            acceleration_pattern: "unknown".to_string(),
            braking_score: 50.0,
            terrain_difficulty: "unknown".to_string(),
            weather_impact: "unknown".to_string(),
        }
    }
}
