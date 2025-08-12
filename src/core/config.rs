use crate::core::error::{Result, SpeedKarmaError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration following Apple's intelligent defaults philosophy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Automatic ISP detection and optimization settings
    pub auto_optimization: AutoOptimizationConfig,
    
    /// Network monitoring configuration
    pub monitoring: MonitoringConfig,
    
    /// UI and notification preferences
    pub ui: UiConfig,
    
    /// Advanced settings (hidden by default)
    pub advanced: AdvancedConfig,

    /// Legal and compliance settings
    pub legal: LegalConfig,
}

/// Automatic optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoOptimizationConfig {
    /// Enable automatic optimization when patterns are detected
    pub enabled: bool,
    
    /// Minimum confidence level required to start optimization (0.0 to 1.0)
    pub min_confidence: f64,
    
    /// Minimum improvement factor required to continue optimization
    pub min_improvement_factor: f64,
    
    /// Days of data required before enabling optimization
    pub min_data_days: u32,
}

/// Network monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Interval between passive speed measurements (seconds)
    pub measurement_interval: u64,
    
    /// Maximum bandwidth usage per hour (MB)
    pub max_bandwidth_per_hour: f64,
    
    /// Enable monitoring during specific hours only
    pub time_restrictions: Option<TimeRestrictions>,
}

/// UI and notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Show notifications for status changes
    pub show_notifications: bool,
    
    /// Minimize to system tray on startup
    pub start_minimized: bool,
    
    /// Theme preference (auto, light, dark)
    pub theme: String,
}

/// Advanced configuration (hidden from main UI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    /// Custom speedtest servers (overrides automatic selection)
    pub custom_servers: Vec<String>,
    
    /// Enable debug logging
    pub debug_logging: bool,
    
    /// Custom ISP profile override
    pub isp_override: Option<String>,
    
    /// Traffic pattern customization
    pub traffic_patterns: TrafficPatternConfig,
}

/// Legal and compliance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalConfig {
    /// Has the user accepted the terms of use
    pub terms_accepted: bool,
}

/// Time-based restrictions for operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRestrictions {
    pub allowed_hours: Vec<u8>, // Hours when operation is allowed (0-23)
    pub allowed_days: Vec<u8>,  // Days when operation is allowed (0-6, 0=Sunday)
}

/// Traffic pattern configuration for stealth operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficPatternConfig {
    /// Server rotation interval (minutes)
    pub rotation_interval_minutes: u32,
    
    /// Packet timing randomization range (seconds)
    pub timing_randomization: (f64, f64),
    
    /// Number of concurrent connections
    pub connection_count: u8,
    
    /// Traffic intensity level (0.0 to 1.0)
    pub intensity: f64,
}

impl Default for AppConfig {
    /// Intelligent defaults following Apple's "it just works" philosophy
    fn default() -> Self {
        Self {
            auto_optimization: AutoOptimizationConfig {
                enabled: true,
                min_confidence: 0.8,
                min_improvement_factor: 1.5, // At least 50% improvement
                min_data_days: 7,
            },
            monitoring: MonitoringConfig {
                measurement_interval: 300, // 5 minutes
                max_bandwidth_per_hour: 1.0, // 1MB per hour
                time_restrictions: None, // No restrictions by default
            },
            ui: UiConfig {
                show_notifications: true,
                start_minimized: true,
                theme: "auto".to_string(),
            },
            advanced: AdvancedConfig {
                custom_servers: Vec::new(),
                debug_logging: false,
                isp_override: None,
                traffic_patterns: TrafficPatternConfig {
                    rotation_interval_minutes: 10,
                    timing_randomization: (30.0, 60.0),
                    connection_count: 3,
                    intensity: 0.5,
                },
            },
            legal: LegalConfig {
                terms_accepted: false,
            },
        }
    }
}

impl AppConfig {
    /// Loads configuration from file or creates default
    pub async fn load() -> Result<Self> {
        let config_path = Self::config_file_path()?;
        
        if config_path.exists() {
            let content = tokio::fs::read_to_string(&config_path).await?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            // Create default configuration
            let config = Self::default();
            config.save().await?;
            Ok(config)
        }
    }
    
    /// Saves configuration to file
    pub async fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path()?;
        
        // Ensure parent directory exists
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&config_path, content).await?;
        
        Ok(())
    }
    
    /// Gets the platform-specific configuration file path
    fn config_file_path() -> Result<PathBuf> {
        let config_dir = if cfg!(target_os = "macos") {
            dirs::config_dir()
                .ok_or_else(|| SpeedKarmaError::ConfigurationError("Cannot find config directory".to_string()))?
                .join("SpeedKarma")
        } else if cfg!(target_os = "windows") {
            dirs::config_dir()
                .ok_or_else(|| SpeedKarmaError::ConfigurationError("Cannot find config directory".to_string()))?
                .join("SpeedKarma")
        } else {
            return Err(SpeedKarmaError::ConfigurationError("Unsupported platform".to_string()));
        };
        
        Ok(config_dir.join("config.json"))
    }
    
    /// Validates configuration values
    pub fn validate(&self) -> Result<()> {
        if self.auto_optimization.min_confidence < 0.0 || self.auto_optimization.min_confidence > 1.0 {
            return Err(SpeedKarmaError::ConfigurationError(
                "Confidence level must be between 0.0 and 1.0".to_string()
            ));
        }
        
        if self.auto_optimization.min_improvement_factor < 1.0 {
            return Err(SpeedKarmaError::ConfigurationError(
                "Improvement factor must be at least 1.0".to_string()
            ));
        }
        
        if self.monitoring.max_bandwidth_per_hour <= 0.0 {
            return Err(SpeedKarmaError::ConfigurationError(
                "Bandwidth limit must be positive".to_string()
            ));
        }
        // Legal: nothing to validate beyond boolean
        
        Ok(())
    }
}

// Add dirs dependency for cross-platform directory handling
// This would be added to Cargo.toml in a real implementation