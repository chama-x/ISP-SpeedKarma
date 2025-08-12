use chrono::{DateTime, Utc, Weekday, Datelike, Timelike};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Validation errors for data models
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Speed value must be non-negative: {value}")]
    InvalidSpeed { value: f64 },
    #[error("Confidence must be between 0.0 and 1.0: {value}")]
    InvalidConfidence { value: f64 },
    #[error("Latency must be reasonable (0-10000ms): {value}")]
    InvalidLatency { value: u32 },
    #[error("Hour must be between 0-23: {value}")]
    InvalidHour { value: u8 },
    #[error("Minute must be between 0-59: {value}")]
    InvalidMinute { value: u8 },
    #[error("Severity must be between 0.0 and 1.0: {value}")]
    InvalidSeverity { value: f64 },
    #[error("Traffic intensity must be between 0.0 and 1.0: {value}")]
    InvalidTrafficIntensity { value: f64 },
    #[error("Connection count must be between 1-10: {value}")]
    InvalidConnectionCount { value: u8 },
    #[error("Name cannot be empty")]
    EmptyName,
    #[error("Region cannot be empty")]
    EmptyRegion,
}

/// Speed measurement data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedMeasurement {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub latency_ms: u32,
    pub optimization_active: bool,
    pub confidence: f64,
}

/// ISP profile information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISPProfile {
    pub id: Option<i64>,
    pub name: String,
    pub region: String,
    pub detection_method: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Throttling pattern data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottlingPattern {
    pub id: Option<i64>,
    pub isp_profile_id: i64,
    pub start_hour: u8,
    pub start_minute: u8,
    pub end_hour: u8,
    pub end_minute: u8,
    #[serde(serialize_with = "serialize_weekdays", deserialize_with = "deserialize_weekdays")]
    pub days_of_week: Vec<Weekday>,
    pub severity: f64,
    pub confidence: f64,
}

// Helper functions for serializing weekdays to/from JSON strings
fn serialize_weekdays<S>(weekdays: &Vec<Weekday>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let weekday_numbers: Vec<u8> = weekdays.iter().map(|w| w.number_from_monday() as u8).collect();
    serde_json::to_string(&weekday_numbers).unwrap().serialize(serializer)
}

fn deserialize_weekdays<'de, D>(deserializer: D) -> Result<Vec<Weekday>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let json_string = String::deserialize(deserializer)?;
    let weekday_numbers: Vec<u8> = serde_json::from_str(&json_string)
        .map_err(serde::de::Error::custom)?;
    
    let weekdays = weekday_numbers.iter()
        .filter_map(|&n| match n {
            1 => Some(Weekday::Mon),
            2 => Some(Weekday::Tue),
            3 => Some(Weekday::Wed),
            4 => Some(Weekday::Thu),
            5 => Some(Weekday::Fri),
            6 => Some(Weekday::Sat),
            7 => Some(Weekday::Sun),
            _ => None,
        })
        .collect();
    
    Ok(weekdays)
}

/// Stealth level enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StealthLevel {
    Low,
    Medium,
    High,
    Maximum,
}

impl StealthLevel {
    /// Convert to string for database storage
    pub fn to_string(&self) -> String {
        match self {
            StealthLevel::Low => "Low".to_string(),
            StealthLevel::Medium => "Medium".to_string(),
            StealthLevel::High => "High".to_string(),
            StealthLevel::Maximum => "Maximum".to_string(),
        }
    }

    /// Create from string (for database retrieval)
    pub fn from_string(s: &str) -> Self {
        match s {
            "Low" => StealthLevel::Low,
            "High" => StealthLevel::High,
            "Maximum" => StealthLevel::Maximum,
            _ => StealthLevel::Medium, // Default
        }
    }
}

/// Optimization strategy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationStrategy {
    pub id: Option<i64>,
    pub name: String,
    pub server_rotation_interval_minutes: u32,
    pub packet_timing_min_seconds: f64,
    pub packet_timing_max_seconds: f64,
    pub connection_count: u8,
    pub traffic_intensity: f64,
    pub stealth_level: StealthLevel,
    pub effectiveness_score: Option<f64>,
    pub created_at: DateTime<Utc>,
}

impl SpeedMeasurement {
    pub fn new(download_mbps: f64, upload_mbps: f64, latency_ms: u32, optimization_active: bool) -> Self {
        Self {
            id: None,
            timestamp: Utc::now(),
            download_mbps,
            upload_mbps,
            latency_ms,
            optimization_active,
            confidence: 1.0, // Default confidence
        }
    }

    /// Validate the speed measurement data
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.download_mbps < 0.0 {
            return Err(ValidationError::InvalidSpeed { value: self.download_mbps });
        }
        if self.upload_mbps < 0.0 {
            return Err(ValidationError::InvalidSpeed { value: self.upload_mbps });
        }
        if self.latency_ms > 10000 {
            return Err(ValidationError::InvalidLatency { value: self.latency_ms });
        }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            return Err(ValidationError::InvalidConfidence { value: self.confidence });
        }
        Ok(())
    }

    /// Check if this measurement indicates good performance
    pub fn is_good_performance(&self) -> bool {
        self.download_mbps > 10.0 && self.upload_mbps > 1.0 && self.latency_ms < 100
    }

    /// Calculate a performance score (0.0 to 1.0)
    pub fn performance_score(&self) -> f64 {
        let download_score = (self.download_mbps / 100.0).min(1.0);
        let upload_score = (self.upload_mbps / 20.0).min(1.0);
        let latency_score = (1.0 - (self.latency_ms as f64 / 1000.0)).max(0.0);
        
        (download_score * 0.5 + upload_score * 0.3 + latency_score * 0.2) * self.confidence
    }
}

impl ISPProfile {
    pub fn new(name: String, region: String, detection_method: String) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            name,
            region,
            detection_method,
            created_at: now,
            updated_at: now,
        }
    }

    /// Validate the ISP profile data
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if self.region.trim().is_empty() {
            return Err(ValidationError::EmptyRegion);
        }
        Ok(())
    }

    /// Update the profile's updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Check if this profile is for a known throttling ISP
    pub fn is_known_throttling_isp(&self) -> bool {
        let throttling_isps = ["hutch", "dialog", "mobitel", "airtel"];
        throttling_isps.iter().any(|&isp| self.name.to_lowercase().contains(isp))
    }
}

impl ThrottlingPattern {
    pub fn new(
        isp_profile_id: i64,
        start_hour: u8,
        start_minute: u8,
        end_hour: u8,
        end_minute: u8,
        days_of_week: Vec<Weekday>,
        severity: f64,
    ) -> Self {
        Self {
            id: None,
            isp_profile_id,
            start_hour,
            start_minute,
            end_hour,
            end_minute,
            days_of_week,
            severity,
            confidence: 0.5, // Default confidence
        }
    }

    /// Convert weekdays to JSON string for database storage
    pub fn days_of_week_json(&self) -> String {
        let weekday_numbers: Vec<u8> = self.days_of_week.iter()
            .map(|w| w.number_from_monday() as u8)
            .collect();
        serde_json::to_string(&weekday_numbers).unwrap_or_default()
    }

    /// Create from database row with JSON string for days
    pub fn from_db_row(
        id: Option<i64>,
        isp_profile_id: i64,
        start_hour: u8,
        start_minute: u8,
        end_hour: u8,
        end_minute: u8,
        days_json: &str,
        severity: f64,
        confidence: f64,
    ) -> Self {
        let weekday_numbers: Vec<u8> = serde_json::from_str(days_json).unwrap_or_default();
        let days_of_week = weekday_numbers.iter()
            .filter_map(|&n| match n {
                1 => Some(Weekday::Mon),
                2 => Some(Weekday::Tue),
                3 => Some(Weekday::Wed),
                4 => Some(Weekday::Thu),
                5 => Some(Weekday::Fri),
                6 => Some(Weekday::Sat),
                7 => Some(Weekday::Sun),
                _ => None,
            })
            .collect();

        Self {
            id,
            isp_profile_id,
            start_hour,
            start_minute,
            end_hour,
            end_minute,
            days_of_week,
            severity,
            confidence,
        }
    }

    /// Validate the throttling pattern data
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.start_hour > 23 {
            return Err(ValidationError::InvalidHour { value: self.start_hour });
        }
        if self.end_hour > 23 {
            return Err(ValidationError::InvalidHour { value: self.end_hour });
        }
        if self.start_minute > 59 {
            return Err(ValidationError::InvalidMinute { value: self.start_minute });
        }
        if self.end_minute > 59 {
            return Err(ValidationError::InvalidMinute { value: self.end_minute });
        }
        if self.severity < 0.0 || self.severity > 1.0 {
            return Err(ValidationError::InvalidSeverity { value: self.severity });
        }
        if self.confidence < 0.0 || self.confidence > 1.0 {
            return Err(ValidationError::InvalidConfidence { value: self.confidence });
        }
        Ok(())
    }

    /// Check if the pattern is currently active
    pub fn is_active_now(&self) -> bool {
        let now = Utc::now();
        let current_weekday = now.weekday();
        let current_hour = now.hour() as u8;
        let current_minute = now.minute() as u8;

        // Check if today is in the pattern's days
        if !self.days_of_week.contains(&current_weekday) {
            return false;
        }

        // Convert times to minutes for easier comparison
        let start_minutes = self.start_hour as u32 * 60 + self.start_minute as u32;
        let end_minutes = self.end_hour as u32 * 60 + self.end_minute as u32;
        let current_minutes = current_hour as u32 * 60 + current_minute as u32;

        // Handle patterns that cross midnight
        if start_minutes > end_minutes {
            current_minutes >= start_minutes || current_minutes <= end_minutes
        } else {
            current_minutes >= start_minutes && current_minutes <= end_minutes
        }
    }

    /// Get a human-readable description of the pattern
    pub fn description(&self) -> String {
        let days: Vec<String> = self.days_of_week.iter()
            .map(|d| format!("{:?}", d))
            .collect();
        
        format!(
            "{:02}:{:02}-{:02}:{:02} on {} (severity: {:.1}%)",
            self.start_hour, self.start_minute,
            self.end_hour, self.end_minute,
            days.join(", "),
            self.severity * 100.0
        )
    }
}

impl OptimizationStrategy {
    pub fn default_strategy() -> Self {
        Self {
            id: None,
            name: "Default".to_string(),
            server_rotation_interval_minutes: 10,
            packet_timing_min_seconds: 30.0,
            packet_timing_max_seconds: 60.0,
            connection_count: 3,
            traffic_intensity: 0.5,
            stealth_level: StealthLevel::Medium,
            effectiveness_score: None,
            created_at: Utc::now(),
        }
    }

    /// Create a high stealth strategy for aggressive ISPs
    pub fn high_stealth_strategy() -> Self {
        Self {
            id: None,
            name: "High Stealth".to_string(),
            server_rotation_interval_minutes: 5,
            packet_timing_min_seconds: 45.0,
            packet_timing_max_seconds: 90.0,
            connection_count: 2,
            traffic_intensity: 0.3,
            stealth_level: StealthLevel::High,
            effectiveness_score: None,
            created_at: Utc::now(),
        }
    }

    /// Validate the optimization strategy data
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.name.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if self.connection_count == 0 || self.connection_count > 10 {
            return Err(ValidationError::InvalidConnectionCount { value: self.connection_count });
        }
        if self.traffic_intensity < 0.0 || self.traffic_intensity > 1.0 {
            return Err(ValidationError::InvalidTrafficIntensity { value: self.traffic_intensity });
        }
        if self.packet_timing_min_seconds >= self.packet_timing_max_seconds {
            return Err(ValidationError::InvalidSpeed { value: self.packet_timing_min_seconds });
        }
        if let Some(score) = self.effectiveness_score {
            if score < 0.0 || score > 1.0 {
                return Err(ValidationError::InvalidConfidence { value: score });
            }
        }
        Ok(())
    }

    /// Check if this strategy is considered effective
    pub fn is_effective(&self) -> bool {
        self.effectiveness_score.map_or(false, |score| score > 0.6)
    }

    /// Get the stealth level as a numeric value (0-3)
    pub fn stealth_level_numeric(&self) -> u8 {
        match self.stealth_level {
            StealthLevel::Low => 0,
            StealthLevel::Medium => 1,
            StealthLevel::High => 2,
            StealthLevel::Maximum => 3,
        }
    }
}

/// Speedtest server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedtestServer {
    pub id: Option<i64>,
    pub server_id: String,
    pub host: String,
    pub port: u16,
    pub name: String,
    pub country: String,
    pub sponsor: String,
    pub distance: Option<f64>,
    pub latency: Option<f64>,
    pub is_active: bool,
    pub last_used: Option<DateTime<Utc>>,
}

impl SpeedtestServer {
    pub fn new(server_id: String, host: String, port: u16, name: String, country: String, sponsor: String) -> Self {
        Self {
            id: None,
            server_id,
            host,
            port,
            name,
            country,
            sponsor,
            distance: None,
            latency: None,
            is_active: true,
            last_used: None,
        }
    }

    /// Validate the speedtest server data
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.server_id.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if self.host.trim().is_empty() {
            return Err(ValidationError::EmptyName);
        }
        if self.port == 0 {
            return Err(ValidationError::InvalidSpeed { value: self.port as f64 });
        }
        Ok(())
    }

    /// Mark this server as recently used
    pub fn mark_used(&mut self) {
        self.last_used = Some(Utc::now());
    }

    /// Check if this server is suitable for the given region
    pub fn is_suitable_for_region(&self, region: &str) -> bool {
        let region_lower = region.to_lowercase();
        let country_lower = self.country.to_lowercase();
        
        // Direct country match
        if country_lower.contains(&region_lower) {
            return true;
        }
        
        // Regional proximity for Sri Lanka
        if region_lower.contains("sri lanka") || region_lower.contains("lanka") {
            return country_lower.contains("singapore") || 
                   country_lower.contains("india") || 
                   country_lower.contains("sri lanka");
        }
        
        false
    }
}

/// Configuration settings for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub id: Option<i64>,
    pub auto_start: bool,
    pub monitoring_enabled: bool,
    pub optimization_enabled: bool,
    pub data_retention_days: u32,
    pub bandwidth_limit_mb_per_hour: Option<f64>,
    pub notification_level: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            id: None,
            auto_start: true,
            monitoring_enabled: true,
            optimization_enabled: false, // Start with monitoring only
            data_retention_days: 30,
            bandwidth_limit_mb_per_hour: Some(1.0), // 1MB per hour limit
            notification_level: "Normal".to_string(),
            created_at: now,
            updated_at: now,
        }
    }
}

impl AppConfig {
    /// Validate the application configuration
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.data_retention_days == 0 || self.data_retention_days > 365 {
            return Err(ValidationError::InvalidSpeed { value: self.data_retention_days as f64 });
        }
        if let Some(limit) = self.bandwidth_limit_mb_per_hour {
            if limit <= 0.0 || limit > 1000.0 {
                return Err(ValidationError::InvalidSpeed { value: limit });
            }
        }
        Ok(())
    }

    /// Update the configuration's updated_at timestamp
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    #[test]
    fn test_speed_measurement_validation() {
        let mut measurement = SpeedMeasurement::new(50.0, 10.0, 50, false);
        assert!(measurement.validate().is_ok());

        measurement.download_mbps = -1.0;
        assert!(measurement.validate().is_err());

        measurement.download_mbps = 50.0;
        measurement.confidence = 1.5;
        assert!(measurement.validate().is_err());
    }

    #[test]
    fn test_speed_measurement_performance_score() {
        let measurement = SpeedMeasurement::new(100.0, 20.0, 20, false);
        let score = measurement.performance_score();
        assert!(score > 0.8);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_throttling_pattern_validation() {
        let pattern = ThrottlingPattern::new(
            1,
            19, 0,  // 7 PM
            22, 0,  // 10 PM
            vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
            0.8,
        );
        assert!(pattern.validate().is_ok());

        let mut invalid_pattern = pattern.clone();
        invalid_pattern.start_hour = 25;
        assert!(invalid_pattern.validate().is_err());
    }

    #[test]
    fn test_optimization_strategy_validation() {
        let strategy = OptimizationStrategy::default_strategy();
        assert!(strategy.validate().is_ok());

        let mut invalid_strategy = strategy.clone();
        invalid_strategy.connection_count = 0;
        assert!(invalid_strategy.validate().is_err());
    }

    #[test]
    fn test_isp_profile_validation() {
        let profile = ISPProfile::new(
            "Hutch".to_string(),
            "Sri Lanka".to_string(),
            "DNS Analysis".to_string(),
        );
        assert!(profile.validate().is_ok());
        assert!(profile.is_known_throttling_isp());

        let mut invalid_profile = profile.clone();
        invalid_profile.name = "".to_string();
        assert!(invalid_profile.validate().is_err());
    }

    #[test]
    fn test_speedtest_server_region_suitability() {
        let server = SpeedtestServer::new(
            "12345".to_string(),
            "speedtest.singapore.com".to_string(),
            8080,
            "Singapore Server".to_string(),
            "Singapore".to_string(),
            "Test Sponsor".to_string(),
        );

        assert!(server.is_suitable_for_region("Sri Lanka"));
        assert!(server.is_suitable_for_region("singapore"));
        assert!(!server.is_suitable_for_region("United States"));
    }

    #[test]
    fn test_app_config_validation() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());

        let mut invalid_config = config.clone();
        invalid_config.data_retention_days = 0;
        assert!(invalid_config.validate().is_err());
    }
}