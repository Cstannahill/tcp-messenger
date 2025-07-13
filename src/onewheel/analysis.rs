use crate::onewheel::models::*;
use std::collections::HashMap;
use chrono::Utc;
use tracing::{info, warn, error};

/// Core analysis engine for OneWheel ride data
pub struct RideAnalyzer {
    // Configuration and state
}

impl RideAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    /// Perform comprehensive analysis of a ride
    pub async fn analyze_ride(&self, ride_data: &RideData) -> Result<RideAnalysis, OneWheelError> {
        info!("Starting analysis for ride: {}", ride_data.id);

        // 1. Basic performance metrics
        let speed_efficiency = self.calculate_speed_efficiency(ride_data);
        let energy_efficiency = self.calculate_energy_efficiency(ride_data);
        let route_quality = self.assess_route_quality(ride_data);

        // 2. Advanced AI metrics
        let ai_metrics = self.generate_ai_metrics(ride_data).await?;

        // 3. Generate insights and suggestions
        let insights = self.generate_insights(ride_data, speed_efficiency, energy_efficiency);
        let suggestions = self.generate_suggestions(ride_data, speed_efficiency, energy_efficiency);
        let safety_tips = self.generate_safety_tips(ride_data);

        // 4. Analyze riding style and mood
        let ride_style = self.classify_ride_style(ride_data, &ai_metrics);
        let mood_analysis = self.analyze_riding_mood(ride_data, &ai_metrics);

        // 5. Calculate skill progression
        let skill_progression = self.calculate_skill_progression(ride_data, &ai_metrics);

        // 6. Generate comparisons (placeholder - would need historical data)
        let comparisons = self.generate_comparisons(ride_data);

        // 7. Calculate overall score
        let overall_score = self.calculate_overall_score(
            speed_efficiency,
            energy_efficiency,
            route_quality,
            &ai_metrics
        );

        Ok(RideAnalysis {
            ride_id: ride_data.id.clone(),
            overall_score,
            speed_efficiency,
            energy_efficiency,
            route_quality,
            ride_style,
            insights,
            suggestions,
            comparisons,
            generated_at: Utc::now(),
            ai_metrics,
            mood_analysis,
            safety_tips,
            skill_progression,
        })
    }

    /// Calculate speed efficiency based on consistency and patterns
    fn calculate_speed_efficiency(&self, ride_data: &RideData) -> f64 {
        if let (Some(avg_speed), Some(max_speed)) = (ride_data.avg_speed, ride_data.max_speed) {
            if max_speed > 0.0 {
                // Higher ratio of avg to max speed indicates better consistency
                let consistency_ratio = avg_speed / max_speed;
                
                // Scale to 0-100 with bonus for good speeds
                let base_score = consistency_ratio * 100.0;
                
                // Bonus for maintaining reasonable speeds (8-15 mph is ideal)
                let speed_bonus = if avg_speed >= 8.0 && avg_speed <= 15.0 {
                    10.0
                } else if avg_speed >= 6.0 && avg_speed <= 18.0 {
                    5.0
                } else {
                    0.0
                };
                
                (base_score + speed_bonus).min(100.0).max(0.0)
            } else {
                50.0 // Default for invalid data
            }
        } else {
            50.0 // Default when speed data unavailable
        }
    }

    /// Calculate energy efficiency based on distance per battery percentage
    fn calculate_energy_efficiency(&self, ride_data: &RideData) -> f64 {
        if let (Some(distance), Some(start_battery), Some(end_battery)) = 
            (ride_data.distance, ride_data.start_battery, ride_data.end_battery) {
            
            let battery_used = start_battery.saturating_sub(end_battery) as f64;
            
            if battery_used > 0.0 {
                // Miles per battery percentage
                let efficiency = distance / battery_used;
                
                // Scale based on typical OneWheel range (0.15-0.25 miles per %)
                let normalized_efficiency = (efficiency / 0.2) * 100.0;
                
                normalized_efficiency.min(100.0).max(0.0)
            } else {
                75.0 // Default for minimal battery usage
            }
        } else {
            50.0 // Default when battery data unavailable
        }
    }

    /// Assess route quality based on GPS data completeness and accuracy
    fn assess_route_quality(&self, ride_data: &RideData) -> f64 {
        if let Some(route) = &ride_data.route {
            if route.is_empty() {
                return 0.0;
            }

            let mut quality_score = 0.0;
            let mut factors = 0;

            // Factor 1: Point density (more points = better tracking)
            if let Some(duration) = ride_data.duration {
                let points_per_minute = (route.len() as f64) / (duration as f64 / 60.0);
                let density_score = (points_per_minute / 10.0 * 25.0).min(25.0); // Up to 25 points
                quality_score += density_score;
                factors += 1;
            }

            // Factor 2: GPS coordinate validity
            let valid_points = route.iter().filter(|p| {
                p.lat >= -90.0 && p.lat <= 90.0 && p.lng >= -180.0 && p.lng <= 180.0
            }).count();
            let validity_score = (valid_points as f64 / route.len() as f64) * 25.0;
            quality_score += validity_score;
            factors += 1;

            // Factor 3: Route smoothness (check for unrealistic jumps)
            let smoothness_score = self.calculate_route_smoothness(route);
            quality_score += smoothness_score;
            factors += 1;

            // Factor 4: Data completeness
            let completeness_score = self.calculate_data_completeness(route);
            quality_score += completeness_score;
            factors += 1;

            if factors > 0 {
                quality_score
            } else {
                50.0
            }
        } else {
            0.0 // No route data
        }
    }

    /// Calculate route smoothness based on coordinate changes
    fn calculate_route_smoothness(&self, route: &[GpsPoint]) -> f64 {
        if route.len() < 2 {
            return 25.0; // Max score for insufficient data
        }

        let mut total_distance = 0.0;
        let mut unrealistic_jumps = 0;

        for window in route.windows(2) {
            let distance = self.haversine_distance(
                window[0].lat, window[0].lng,
                window[1].lat, window[1].lng
            );
            
            total_distance += distance;
            
            // Flag unrealistic jumps (>100m between points assuming ~1 second intervals)
            if distance > 0.1 { // 100 meters
                unrealistic_jumps += 1;
            }
        }

        let jump_ratio = unrealistic_jumps as f64 / (route.len() - 1) as f64;
        let smoothness = (1.0 - jump_ratio) * 25.0; // Up to 25 points
        
        smoothness.max(0.0)
    }

    /// Calculate data completeness score
    fn calculate_data_completeness(&self, route: &[GpsPoint]) -> f64 {
        if route.is_empty() {
            return 0.0;
        }

        let mut completeness_factors = 0;
        let mut total_factors = 0;

        // Check for timestamps
        total_factors += 1;
        if route.iter().any(|p| p.timestamp.is_some()) {
            completeness_factors += 1;
        }

        // Check for speed data
        total_factors += 1;
        if route.iter().any(|p| p.speed.is_some()) {
            completeness_factors += 1;
        }

        // Check for battery data
        total_factors += 1;
        if route.iter().any(|p| p.battery.is_some()) {
            completeness_factors += 1;
        }

        (completeness_factors as f64 / total_factors as f64) * 25.0
    }

    /// Calculate distance between two GPS points using Haversine formula
    fn haversine_distance(&self, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0; // Earth's radius in kilometers
        let d_lat = (lat2 - lat1).to_radians();
        let d_lon = (lon2 - lon1).to_radians();
        let a = (d_lat / 2.0).sin().powi(2) +
                lat1.to_radians().cos() * lat2.to_radians().cos() *
                (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }

    /// Generate AI metrics (simplified version - would integrate with actual AI)
    async fn generate_ai_metrics(&self, ride_data: &RideData) -> Result<AiMetrics, OneWheelError> {
        // This would integrate with OpenAI or custom ML models
        // For now, generate based on available data
        
        let acceleration_pattern = if let Some(avg_speed) = ride_data.avg_speed {
            if avg_speed < 8.0 { "conservative" }
            else if avg_speed > 15.0 { "aggressive" }
            else { "smooth" }
        } else { "unknown" };

        let braking_score = if let (Some(max_speed), Some(avg_speed)) = 
            (ride_data.max_speed, ride_data.avg_speed) {
            // Higher avg/max ratio suggests good speed control
            (avg_speed / max_speed * 100.0).min(100.0)
        } else { 75.0 };

        let terrain_difficulty = if let Some(route) = &ride_data.route {
            if route.len() > 100 { "moderate" } else { "easy" }
        } else { "unknown" };

        Ok(AiMetrics {
            acceleration_pattern: acceleration_pattern.to_string(),
            braking_score,
            terrain_difficulty: terrain_difficulty.to_string(),
            weather_impact: "minimal".to_string(), // Would need weather data
        })
    }

    /// Generate insights based on ride analysis
    fn generate_insights(&self, ride_data: &RideData, speed_eff: f64, energy_eff: f64) -> Vec<String> {
        let mut insights = Vec::new();

        if speed_eff > 80.0 {
            insights.push("🚀 Excellent speed consistency throughout the ride".to_string());
        } else if speed_eff > 60.0 {
            insights.push("⚡ Good speed control with room for improvement".to_string());
        } else {
            insights.push("🎯 Focus on maintaining steady speeds for better efficiency".to_string());
        }

        if energy_eff > 85.0 {
            insights.push("🔋 Outstanding battery efficiency - you're maximizing range".to_string());
        } else if energy_eff > 70.0 {
            insights.push("🔋 Good energy management with solid efficiency".to_string());
        } else {
            insights.push("⚡ Consider techniques to improve battery efficiency".to_string());
        }

        if let Some(route) = &ride_data.route {
            if route.len() > 50 {
                insights.push("🗺️ Great route tracking with detailed GPS data".to_string());
            }
        }

        insights
    }

    /// Generate improvement suggestions
    fn generate_suggestions(&self, ride_data: &RideData, speed_eff: f64, energy_eff: f64) -> Vec<String> {
        let mut suggestions = Vec::new();

        if speed_eff < 70.0 {
            suggestions.push("Try maintaining steady acceleration for better speed efficiency".to_string());
        }

        if energy_eff < 75.0 {
            suggestions.push("Consider smoother acceleration and braking to improve battery life".to_string());
        }

        if let Some(distance) = ride_data.distance {
            if distance < 5.0 {
                suggestions.push("Consider exploring longer routes to build endurance".to_string());
            }
        }

        if suggestions.is_empty() {
            suggestions.push("Great riding! Keep up the excellent technique".to_string());
        }

        suggestions
    }

    /// Generate safety tips based on ride data
    fn generate_safety_tips(&self, ride_data: &RideData) -> Vec<String> {
        let mut tips = Vec::new();

        if let Some(max_speed) = ride_data.max_speed {
            if max_speed > 20.0 {
                tips.push("Consider wearing additional protective gear for high-speed riding".to_string());
            } else {
                tips.push("Great job maintaining safe speeds throughout the ride".to_string());
            }
        }

        if let Some(distance) = ride_data.distance {
            if distance > 10.0 {
                tips.push("For longer rides, take breaks to stay alert and hydrated".to_string());
            }
        }

        tips.push("Always wear a helmet and appropriate protective gear".to_string());

        tips
    }

    /// Classify riding style based on patterns
    fn classify_ride_style(&self, ride_data: &RideData, ai_metrics: &AiMetrics) -> String {
        if let Some(avg_speed) = ride_data.avg_speed {
            match ai_metrics.acceleration_pattern.as_str() {
                "aggressive" => "Sport Rider".to_string(),
                "conservative" => "Cruiser".to_string(),
                "smooth" => "Balanced Rider".to_string(),
                _ => "Adaptive Rider".to_string(),
            }
        } else {
            "Casual Rider".to_string()
        }
    }

    /// Analyze riding mood based on patterns
    fn analyze_riding_mood(&self, ride_data: &RideData, ai_metrics: &AiMetrics) -> String {
        let mood = if ai_metrics.braking_score > 80.0 {
            "Confident and controlled"
        } else if ai_metrics.braking_score > 60.0 {
            "Relaxed and steady"
        } else {
            "Adventurous and exploring"
        };

        format!("{} riding style with good awareness", mood)
    }

    /// Calculate skill progression metrics
    fn calculate_skill_progression(&self, ride_data: &RideData, ai_metrics: &AiMetrics) -> SkillProgression {
        SkillProgression {
            balance: ai_metrics.braking_score, // Use braking score as balance proxy
            speed_control: if let Some(avg_speed) = ride_data.avg_speed {
                (avg_speed / 15.0 * 100.0).min(100.0)
            } else { 50.0 },
            efficiency: self.calculate_energy_efficiency(ride_data),
            route_planning: self.assess_route_quality(ride_data),
        }
    }

    /// Generate ride comparisons (placeholder)
    fn generate_comparisons(&self, _ride_data: &RideData) -> Vec<String> {
        // Would need historical data from database
        vec![
            "This analysis is based on current ride data".to_string(),
            "Historical comparisons available with more ride data".to_string(),
        ]
    }

    /// Calculate overall score from individual metrics
    fn calculate_overall_score(&self, speed_eff: f64, energy_eff: f64, route_quality: f64, ai_metrics: &AiMetrics) -> f64 {
        let weights = [0.3, 0.3, 0.2, 0.2]; // Speed, Energy, Route, AI
        let scores = [speed_eff, energy_eff, route_quality, ai_metrics.braking_score];
        
        weights.iter().zip(scores.iter()).map(|(w, s)| w * s).sum()
    }
}
