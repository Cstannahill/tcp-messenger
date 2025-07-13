use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Ride data structure matching the API specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideData {
    pub id: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub user_id: Option<String>,
    #[serde(rename = "startTime")]
    pub start_time: DateTime<Utc>,
    #[serde(rename = "endTime")]
    pub end_time: Option<DateTime<Utc>>,
    pub distance: Option<f64>,
    #[serde(rename = "maxSpeed")]
    pub max_speed: Option<f64>,
    #[serde(rename = "avgSpeed")]
    pub avg_speed: Option<f64>,
    pub duration: Option<u32>,
    pub route: Option<Vec<GpsPoint>>,
    #[serde(rename = "startBattery")]
    pub start_battery: Option<u8>,
    #[serde(rename = "endBattery")]
    pub end_battery: Option<u8>,
    pub notes: Option<String>,
    pub metadata: Option<RideMetadata>,
}

/// GPS coordinate point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpsPoint {
    pub lat: f64,
    pub lng: f64,
    pub timestamp: Option<DateTime<Utc>>,
    pub speed: Option<f64>,
    pub battery: Option<u8>,
}

/// Ride metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideMetadata {
    #[serde(rename = "appVersion")]
    pub app_version: Option<String>,
    #[serde(rename = "deviceType")]
    pub device_type: Option<String>,
    #[serde(rename = "uploadTime")]
    pub upload_time: Option<DateTime<Utc>>,
}

/// AI Analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideAnalysis {
    #[serde(rename = "rideId")]
    pub ride_id: String,
    #[serde(rename = "overallScore")]
    pub overall_score: f64,
    #[serde(rename = "speedEfficiency")]
    pub speed_efficiency: f64,
    #[serde(rename = "energyEfficiency")]
    pub energy_efficiency: f64,
    #[serde(rename = "routeQuality")]
    pub route_quality: f64,
    #[serde(rename = "rideStyle")]
    pub ride_style: String,
    pub insights: Vec<String>,
    pub suggestions: Vec<String>,
    pub comparisons: Vec<String>,
    #[serde(rename = "generatedAt")]
    pub generated_at: DateTime<Utc>,
    #[serde(rename = "aiMetrics")]
    pub ai_metrics: AiMetrics,
    #[serde(rename = "moodAnalysis")]
    pub mood_analysis: String,
    #[serde(rename = "safetyTips")]
    pub safety_tips: Vec<String>,
    #[serde(rename = "skillProgression")]
    pub skill_progression: SkillProgression,
}

/// AI-generated metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMetrics {
    #[serde(rename = "accelerationPattern")]
    pub acceleration_pattern: String,
    #[serde(rename = "brakingScore")]
    pub braking_score: f64,
    #[serde(rename = "terrainDifficulty")]
    pub terrain_difficulty: String,
    #[serde(rename = "weatherImpact")]
    pub weather_impact: String,
}

/// Skill progression tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillProgression {
    pub balance: f64,
    pub speed_control: f64,
    pub efficiency: f64,
    pub route_planning: f64,
}

/// API Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideUploadRequest {
    #[serde(flatten)]
    pub ride_data: RideData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RideUploadResponse {
    pub success: bool,
    #[serde(rename = "rideId")]
    pub ride_id: String,
    pub message: String,
    #[serde(rename = "analysisQueued")]
    pub analysis_queued: bool,
    #[serde(rename = "estimatedAnalysisTime")]
    pub estimated_analysis_time: String,
}

/// Analysis request payload
#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisRequest {
    #[serde(rename = "rideId")]
    pub ride_id: String,
    #[serde(rename = "analysisType")]
    pub analysis_type: String,
    #[serde(rename = "includeComparisons")]
    pub include_comparisons: bool,
    #[serde(rename = "includeSuggestions")]
    pub include_suggestions: bool,
}

/// Response for GET /api/rides
#[derive(Debug, Serialize, Deserialize)]
pub struct RideListResponse {
    pub rides: Vec<RideListItem>,
    pub total: u32,
    pub page: u32,
    #[serde(rename = "pageSize")]
    pub page_size: u32,
}

/// Simplified ride item for list view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RideListItem {
    pub id: String,
    #[serde(rename = "startTime")]
    pub start_time: String,
    #[serde(rename = "endTime")]
    pub end_time: String,
    pub distance: f64,
    pub duration: u32,
    #[serde(rename = "maxSpeed")]
    pub max_speed: f64,
    #[serde(rename = "avgSpeed")]
    pub avg_speed: f64,
    #[serde(rename = "topSpeed")]
    pub top_speed: f64,
}

/// User statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct UserStats {
    #[serde(rename = "totalRides")]
    pub total_rides: u32,
    #[serde(rename = "totalDistance")]
    pub total_distance: f64,
    #[serde(rename = "totalTime")]
    pub total_time: u32,
    #[serde(rename = "averageSpeed")]
    pub average_speed: f64,
    #[serde(rename = "bestRide")]
    pub best_ride: Option<BestRide>,
    #[serde(rename = "skillTrends")]
    pub skill_trends: HashMap<String, SkillTrend>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BestRide {
    #[serde(rename = "rideId")]
    pub ride_id: String,
    pub score: f64,
    pub date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SkillTrend {
    pub current: f64,
    pub trend: String,
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum OneWheelError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Analysis error: {0}")]
    Analysis(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Not found: {0}")]
    NotFound(String),
}
