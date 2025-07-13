use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::{info, warn, error};
use std::time::Instant;
use serde_json::json;

use crate::onewheel::{
    models::*,
    analysis::RideAnalyzer,
    database::OneWheelDatabase,
    auth::AuthManager,
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub analyzer: Arc<RideAnalyzer>,
    pub database: Arc<Mutex<OneWheelDatabase>>,
    pub auth: Arc<Mutex<AuthManager>>,
}

/// OneWheel REST API server
pub struct OneWheelApi {
    state: AppState,
}

impl OneWheelApi {
    pub fn new() -> Self {
        let analyzer = Arc::new(RideAnalyzer::new());
        let database = Arc::new(Mutex::new(OneWheelDatabase::new()));
        let mut auth_manager = AuthManager::new();
        
        // Add a demo API key for testing
        let demo_key = auth_manager.add_demo_key();
        info!("Demo API key created: {}", demo_key);
        
        let auth = Arc::new(Mutex::new(auth_manager));

        Self {
            state: AppState {
                analyzer,
                database,
                auth,
            },
        }
    }

    /// Initialize database connection
    pub async fn init_database(&self, database_url: &str) -> Result<(), OneWheelError> {
        let mut db = self.state.database.lock().await;
        db.connect(database_url).await
    }

    /// Create the Axum router with all endpoints
    pub fn create_router(&self) -> Router {
        let db = self.state.database.clone();
        
        Router::new()
            .route("/health", get(|| async { 
                Json(serde_json::json!({
                    "status": "healthy",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "service": "onewheel-api",
                    "version": "1.0.0"
                }))
            }))
            .route("/api/rides", post({
                let db = db.clone();
                move |headers: HeaderMap, Json(payload): Json<RideUploadRequest>| async move {
                    tracing::info!("Received ride upload request");
                    
                    // Extract user ID from auth header
                    let user_id = match extract_user_from_headers(&headers) {
                        Ok(user_id) => user_id,
                        Err(e) => {
                            tracing::warn!("Authentication failed: {}", e);
                            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Authentication required"}))).into_response();
                        }
                    };

                    // Call the database operation directly 
                    let guard = db.lock().await;
                    let result = guard.store_ride(&payload.ride_data, &user_id).await;

                    match result {
                        Ok(_) => {
                            tracing::info!("Ride stored successfully with ID: {}", payload.ride_data.id);
                            (StatusCode::CREATED, Json(json!({"rideId": payload.ride_data.id}))).into_response()
                        }
                        Err(e) => {
                            tracing::error!("Failed to store ride: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to store ride"}))).into_response()
                        }
                    }
                }
            }))
            .route("/api/rides", get({
                let db = db.clone();
                move |headers: HeaderMap, Query(params): Query<std::collections::HashMap<String, String>>| async move {
                    tracing::info!("Received rides list request");
                    
                    // Extract user ID from auth header
                    let user_id = match extract_user_from_headers(&headers) {
                        Ok(user_id) => user_id,
                        Err(e) => {
                            tracing::warn!("Authentication failed: {}", e);
                            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Authentication required"}))).into_response();
                        }
                    };
                    
                    // Parse query parameters
                    let page: u32 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1);
                    let page_size: u32 = params.get("pageSize").and_then(|p| p.parse().ok()).unwrap_or(20);
                    let offset = (page - 1) * page_size;

                    // Clone variables and perform database call
                    let guard = db.lock().await;
                    let result = guard.get_user_rides(&user_id, Some(page_size), Some(offset)).await;

                    match result {
                        Ok(rides) => {
                            use crate::onewheel::models::{RideListResponse, RideListItem};
                            
                            let ride_items: Vec<RideListItem> = rides.into_iter().map(|ride| {
                                RideListItem {
                                    id: ride.id,
                                    start_time: ride.start_time.to_rfc3339(),
                                    end_time: ride.end_time.map(|t| t.to_rfc3339()).unwrap_or_default(),
                                    distance: ride.distance.unwrap_or(0.0),
                                    duration: ride.duration.unwrap_or(0),
                                    max_speed: ride.max_speed.unwrap_or(0.0),
                                    avg_speed: ride.avg_speed.unwrap_or(0.0),
                                    top_speed: ride.max_speed.unwrap_or(0.0), // Use max_speed as top_speed
                                }
                            }).collect();

                            let response = RideListResponse {
                                rides: ride_items.clone(),
                                total: ride_items.len() as u32,
                                page,
                                page_size,
                            };

                            (StatusCode::OK, Json(response)).into_response()
                        }
                        Err(e) => {
                            tracing::error!("Failed to get user rides: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to retrieve rides"}))).into_response()
                        }
                    }
                }
            }))
            .route("/api/rides/:ride_id", get({
                let db = db.clone();
                move |headers: HeaderMap, Path(ride_id): Path<String>| async move {
                    tracing::info!("Received single ride request for ID: {}", ride_id);
                    
                    // Extract user ID from auth header
                    let user_id = match extract_user_from_headers(&headers) {
                        Ok(user_id) => user_id,
                        Err(e) => {
                            tracing::warn!("Authentication failed: {}", e);
                            return (StatusCode::UNAUTHORIZED, Json(json!({"error": "Authentication required"}))).into_response();
                        }
                    };

                    // Clone variables and perform database call
                    let guard = db.lock().await;
                    let result = guard.get_ride(&ride_id).await;

                    match result {
                        Ok(Some(ride)) => {
                            // Verify the ride belongs to the requesting user
                            if ride.user_id.as_deref() == Some(&user_id) {
                                (StatusCode::OK, Json(ride)).into_response()
                            } else {
                                (StatusCode::FORBIDDEN, Json(json!({"error": "Access denied"}))).into_response()
                            }
                        }
                        Ok(None) => {
                            (StatusCode::NOT_FOUND, Json(json!({"error": "Ride not found"}))).into_response()
                        }
                        Err(e) => {
                            tracing::error!("Failed to get ride: {}", e);
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to retrieve ride"}))).into_response()
                        }
                    }
                }
            }))
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive())
                    .into_inner()
            )
    }

    /// Start the API server
    pub async fn start(&self, address: &str) -> Result<(), OneWheelError> {
        let app = self.create_router();
        
        info!("Starting OneWheel API server on {}", address);
        
        let listener = tokio::net::TcpListener::bind(address).await
            .map_err(|e| OneWheelError::Database(format!("Failed to bind to {}: {}", address, e)))?;
        
        axum::serve(listener, app).await
            .map_err(|e| OneWheelError::Database(format!("Server error: {}", e)))?;
        
        Ok(())
    }
}

/// Extract and validate API key from headers
fn extract_api_key(headers: &HeaderMap) -> Result<String, OneWheelError> {
    let auth_header = headers.get("authorization")
        .ok_or_else(|| OneWheelError::Auth("No authorization header".to_string()))?;
    
    let auth_str = auth_header.to_str()
        .map_err(|_| OneWheelError::Auth("Invalid authorization header".to_string()))?;
    
    if let Some(token) = auth_str.strip_prefix("Bearer ") {
        Ok(token.to_string())
    } else {
        Err(OneWheelError::Auth("Invalid authorization format".to_string()))
    }
}

/// Extract user ID from headers (simplified for now)
fn extract_user_from_headers(headers: &HeaderMap) -> Result<String, OneWheelError> {
    let auth_header = headers.get("authorization")
        .ok_or_else(|| OneWheelError::Auth("No authorization header".to_string()))?;
    
    let auth_str = auth_header.to_str()
        .map_err(|_| OneWheelError::Auth("Invalid authorization header".to_string()))?;
    
    if let Some(token) = auth_str.strip_prefix("Bearer ") {
        // For now, just use the token as user_id
        // In production, you'd validate the JWT and extract user_id
        Ok(token.to_string())
    } else {
        Err(OneWheelError::Auth("Invalid authorization format".to_string()))
    }
}

impl Default for OneWheelApi {
    fn default() -> Self {
        Self::new()
    }
}
