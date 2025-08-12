use crate::core::error::{Result, SpeedKarmaError};
use crate::core::config::AppConfig;
use tauri::{AppHandle, Manager, WindowUrl};

/// Advanced configuration interface (hidden by default)
/// Following progressive disclosure principles
pub struct AdvancedInterface {}

impl AdvancedInterface {
    pub fn new() -> Self { Self {} }
    
    /// Shows the advanced configuration panel (creates if missing)
    pub async fn show(&self, app_handle: &AppHandle) -> Result<()> {
        if let Some(window) = app_handle.get_window("advanced") {
            window
                .show()
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to show advanced window: {}", e)))?;
            window
                .set_focus()
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to focus advanced window: {}", e)))?;
            return Ok(());
        }

        tauri::WindowBuilder::new(
            app_handle,
            "advanced",
            WindowUrl::App("index.html".into()),
        )
        .title("SpeedKarma — Advanced Settings")
        .resizable(true)
        .visible(true)
        .inner_size(560.0, 640.0)
        .build()
        .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to create advanced window: {}", e)))?;

        Ok(())
    }
    
    /// Hides the advanced configuration panel
    pub async fn hide(&self, app_handle: &AppHandle) -> Result<()> {
        if let Some(window) = app_handle.get_window("advanced") {
            window
                .hide()
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to hide advanced window: {}", e)))?;
        }
        Ok(())
    }

    /// Export app configuration to JSON string
    pub async fn export_config(&self) -> Result<String> {
        let cfg = AppConfig::load().await?;
        Ok(serde_json::to_string_pretty(&cfg)? )
    }

    /// Import app configuration from JSON string and persist
    pub async fn import_config(&self, json: &str) -> Result<()> {
        let cfg: AppConfig = serde_json::from_str(json)?;
        cfg.validate()?;
        cfg.save().await?;
        Ok(())
    }
}