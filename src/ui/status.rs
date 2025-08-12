use crate::core::intelligence::{SystemStatus, SystemState, EffectivenessMetrics, DataCollectionProgress};
use crate::core::error::Result;

/// Status display component for showing system state to users
pub struct StatusDisplay {
    // Placeholder for future UI binding/state
}

impl StatusDisplay {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Updates the displayed status following Apple's clear communication style
    pub async fn update(&self, status: SystemStatus) -> Result<()> {
        // In a full UI, this would update bound components. For now, noop.
        Ok(())
    }
    
    /// Formats status message for user display
    pub fn format_status_message(&self, status: &SystemStatus) -> String {
        match status.state {
            SystemState::Learning => {
                if let Some(DataCollectionProgress { days_collected, days_needed, progress_percentage }) = &status.data_collection_progress {
                    format!(
                        "Learning your network patterns — {}/{} days ({:.0}%)",
                        days_collected, days_needed, progress_percentage
                    )
                } else {
                    "Learning your network patterns".to_string()
                }
            }
            SystemState::Optimizing => {
                if let Some(EffectivenessMetrics { improvement_factor, .. }) = &status.effectiveness {
                    format!("Optimizing — {}x improvement", improvement_factor)
                } else {
                    "Optimizing network".to_string()
                }
            }
            SystemState::Monitoring => "Monitoring network".to_string(),
            SystemState::Inactive => "Inactive".to_string(),
            SystemState::Error(ref err) => format!("Error: {}", err),
        }
    }
}