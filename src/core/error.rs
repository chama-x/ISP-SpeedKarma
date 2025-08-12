use thiserror::Error;

/// Result type alias for SpeedKarma operations
pub type Result<T> = std::result::Result<T, SpeedKarmaError>;

/// Core error types for ISP-SpeedKarma application
/// Following Apple's error philosophy: errors should be rare, graceful, and actionable
#[derive(Debug, Error)]
pub enum SpeedKarmaError {
    /// Network-related errors that can be recovered automatically
    #[error("Network optimization temporarily unavailable: {0}")]
    NetworkUnavailable(String),
    
    /// Database operation errors
    #[error("Database operation failed: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    /// HTTP client errors for speedtest server connections
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    
    /// Configuration and setup errors
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    /// Insufficient data for analysis - gentle notification to user
    #[error("Insufficient data for analysis (need {required} more days)")]
    InsufficientData { required: u32 },
    
    /// System permissions required - user action needed
    #[error("System permissions required for network monitoring")]
    PermissionsRequired,
    
    /// General system errors
    #[error("System error: {0}")]
    SystemError(String),
    
    /// Serialization/deserialization errors
    #[error("Data serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    /// IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl SpeedKarmaError {
    /// Determines if this error should be shown to the user or handled silently
    pub fn should_notify_user(&self) -> bool {
        match self {
            SpeedKarmaError::NetworkUnavailable(_) => false, // Silent recovery
            SpeedKarmaError::InsufficientData { .. } => true, // Gentle notification
            SpeedKarmaError::PermissionsRequired => true, // User action required
            SpeedKarmaError::ConfigurationError(_) => true, // User action may be needed
            _ => false, // Most errors handled silently with automatic recovery
        }
    }
    
    /// Gets user-friendly error message following Apple's clear communication style
    pub fn user_message(&self) -> String {
        match self {
            SpeedKarmaError::InsufficientData { required } => {
                format!("Still learning your network patterns. {} more days needed for optimization.", required)
            }
            SpeedKarmaError::PermissionsRequired => {
                "SpeedKarma needs network monitoring permissions to optimize your connection.".to_string()
            }
            SpeedKarmaError::ConfigurationError(msg) => {
                format!("Configuration needs attention: {}", msg)
            }
            _ => "SpeedKarma encountered an issue but will continue working.".to_string()
        }
    }
    
    /// Determines the severity level for logging and UI indication
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            SpeedKarmaError::NetworkUnavailable(_) => ErrorSeverity::Low,
            SpeedKarmaError::InsufficientData { .. } => ErrorSeverity::Info,
            SpeedKarmaError::PermissionsRequired => ErrorSeverity::High,
            SpeedKarmaError::ConfigurationError(_) => ErrorSeverity::Medium,
            SpeedKarmaError::DatabaseError(_) => ErrorSeverity::Medium,
            _ => ErrorSeverity::Low,
        }
    }
}

/// Error severity levels for appropriate handling and user communication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Informational - no action needed, just status update
    Info,
    /// Low severity - automatic recovery, minimal logging
    Low,
    /// Medium severity - log for debugging, may affect functionality
    Medium,
    /// High severity - requires user attention or action
    High,
}

/// Extension trait for Result types to add SpeedKarma-specific error handling
pub trait ResultExt<T> {
    /// Converts errors to SpeedKarmaError with context
    fn with_context(self, context: &str) -> Result<T>;
    
    /// Logs error and continues with default value (for non-critical operations)
    fn log_and_continue(self, default: T) -> T;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: std::fmt::Display,
{
    fn with_context(self, context: &str) -> Result<T> {
        self.map_err(|e| SpeedKarmaError::SystemError(format!("{}: {}", context, e)))
    }
    
    fn log_and_continue(self, default: T) -> T {
        match self {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!("Non-critical operation failed: {}", e);
                default
            }
        }
    }
}