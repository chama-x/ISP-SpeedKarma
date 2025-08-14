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

/// Formats a concise status string suitable for the native tray menu item.
pub fn format_menu_item_text(status: &SystemStatus) -> String {
    match status.state {
        SystemState::Learning => {
            if let Some(DataCollectionProgress { days_collected, days_needed, .. }) = &status.data_collection_progress {
                format!(
                    "◉ SpeedKarma — Learning patterns ({} of {} days)",
                    days_collected, days_needed
                )
            } else {
                "◉ SpeedKarma — Learning patterns".to_string()
            }
        }
        SystemState::Optimizing => {
            if let Some(EffectivenessMetrics { improvement_factor, .. }) = &status.effectiveness {
                format!("◉ SpeedKarma — Optimizing ({}x)", improvement_factor)
            } else {
                "◉ SpeedKarma — Optimizing".to_string()
            }
        }
        SystemState::Monitoring => "◉ SpeedKarma — Monitoring".to_string(),
        SystemState::Inactive => "◉ SpeedKarma — Inactive".to_string(),
        SystemState::Error(ref err) => format!("◉ SpeedKarma — Error: {}", err),
    }
}

/// Formats a tooltip string for the tray icon.
pub fn format_tooltip_text(status: &SystemStatus) -> String {
    match status.state {
        SystemState::Learning => {
            if let Some(DataCollectionProgress { progress_percentage, .. }) = &status.data_collection_progress {
                format!("SpeedKarma - Learning patterns ({:.0}% complete)", progress_percentage)
            } else {
                "SpeedKarma - Learning network patterns".to_string()
            }
        }
        SystemState::Optimizing => {
            if let Some(EffectivenessMetrics { improvement_factor, .. }) = &status.effectiveness {
                format!("SpeedKarma - Optimizing ({}x improvement)", improvement_factor)
            } else {
                "SpeedKarma - Optimizing network".to_string()
            }
        }
        SystemState::Monitoring => "SpeedKarma - Monitoring network".to_string(),
        SystemState::Inactive => "SpeedKarma - Inactive".to_string(),
        SystemState::Error(ref err) => format!("SpeedKarma - Error: {}", err),
    }
}