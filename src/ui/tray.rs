use crate::core::error::{Result, SpeedKarmaError};
use crate::core::intelligence::{SystemStatus, SystemState};
use crate::core::app_state::{OptimizationMode, SharedAppState};
use crate::core::config::AppConfig;
use crate::data::repository::Repository;
use crate::network::speedtest_runner::SpeedtestRunner;
// (no direct dependency on event payload types; tray receives distilled values)
use crate::ui::panel::PanelInterface;
use crate::ui::status::{format_menu_item_text, format_tooltip_text};
use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray as TauriSystemTray, 
    SystemTrayEvent, SystemTrayMenu, SystemTrayMenuItem,
    api::notification::Notification,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// System tray interface following Apple's menu bar design principles
pub struct SystemTray {
    app_handle: Option<AppHandle>,
    current_status: Arc<RwLock<SystemStatus>>,
    menu_items: SystemTrayMenuItems,
}

/// Menu item identifiers for system tray (minimal native menu)
#[derive(Debug)]
struct SystemTrayMenuItems {
    status_item: String,
    separator1: String,
    toggle_optimization: String,
    run_speedtest: String,
    separator2: String,
    quit: String,
}

impl Default for SystemTrayMenuItems {
    fn default() -> Self {
        Self {
            status_item: "status".to_string(),
            separator1: "sep1".to_string(),
            toggle_optimization: "toggle_opt".to_string(),
            run_speedtest: "run_speedtest".to_string(),
            separator2: "sep2".to_string(),
            quit: "quit".to_string(),
        }
    }
}

impl SystemTray {
    pub fn new() -> Self {
        Self {
            app_handle: None,
            current_status: Arc::new(RwLock::new(SystemStatus {
                state: SystemState::Learning,
                message: "Initializing...".to_string(),
                data_collection_progress: None,
                effectiveness: None,
            })),
            menu_items: SystemTrayMenuItems::default(),
        }
    }
    
    /// Creates the initial system tray menu following Apple's design principles
    pub fn create_tray_menu() -> TauriSystemTray {
        let menu_items = SystemTrayMenuItems::default();
        
        // Create menu items (minimal)
        let status_item = CustomMenuItem::new(&menu_items.status_item, "◉ SpeedKarma")
            .disabled(); // Status item is non-clickable
        
        let toggle_optimization = CustomMenuItem::new(&menu_items.toggle_optimization, "Enable Optimization");
        let run_speedtest = CustomMenuItem::new(&menu_items.run_speedtest, "Run Speedtest Now");
        let quit = CustomMenuItem::new(&menu_items.quit, "Quit SpeedKarma");
        
        // Build minimal menu
        let tray_menu = SystemTrayMenu::new()
            .add_item(status_item)
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(toggle_optimization)
            .add_item(run_speedtest)
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(quit);
        
        TauriSystemTray::new().with_menu(tray_menu)
    }
    
    /// Initializes the system tray with minimal interface
    pub async fn initialize(&mut self, app_handle: AppHandle) -> Result<()> {
        info!("Initializing system tray interface");
        self.app_handle = Some(app_handle);
        
        // Set initial tray tooltip
        self.update_tray_tooltip("SpeedKarma - Learning network patterns").await?;
        
        debug!("System tray initialized successfully");
        Ok(())
    }
    
    /// Updates the tray status display following Apple's clear communication style
    pub async fn update_status(&self, status: SystemStatus) -> Result<()> {
        debug!("Updating tray status: {:?}", status.state);

        // Capture previous status for transition notifications
        let previous_status = self.current_status.read().await.clone();

        // Update internal status
        *self.current_status.write().await = status.clone();
        
        // Update tray tooltip
        let tooltip = format_tooltip_text(&status);
        self.update_tray_tooltip(&tooltip).await?;
        
        // Update menu items based on status
        self.update_menu_items(&status).await?;

        // Notify on important transitions
        self.maybe_notify_transition(&previous_status, &status).await?;
        
        Ok(())
    }
    
    /// Updates the tray tooltip
    async fn update_tray_tooltip(&self, tooltip: &str) -> Result<()> {
        if let Some(app_handle) = &self.app_handle {
            app_handle.tray_handle()
                .set_tooltip(tooltip)
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to update tray tooltip: {}", e)))?;
        }
        Ok(())
    }

    /// Show notifications when state changes meaningfully
    async fn maybe_notify_transition(&self, previous: &SystemStatus, current: &SystemStatus) -> Result<()> {
        match (&previous.state, &current.state) {
            (SystemState::Learning, SystemState::Optimizing) => {
                self.show_notification("SpeedKarma", "Optimization started (enough data collected)").await?;
            }
            (SystemState::Monitoring, SystemState::Optimizing) => {
                self.show_notification("SpeedKarma", "Optimization enabled").await?;
            }
            (SystemState::Optimizing, SystemState::Monitoring) | (SystemState::Optimizing, SystemState::Inactive) => {
                self.show_notification("SpeedKarma", "Optimization paused").await?;
            }
            (_, SystemState::Error(err)) if !matches!(previous.state, SystemState::Error(_)) => {
                self.show_notification("SpeedKarma", &format!("Error detected: {}", err)).await?;
            }
            _ => {}
        }
        Ok(())
    }
    
    /// Updates menu items based on current status
    async fn update_menu_items(&self, status: &SystemStatus) -> Result<()> {
        if let Some(app_handle) = &self.app_handle {
            let tray_handle = app_handle.tray_handle();
            
            // Update status item text
            let status_text = format_menu_item_text(status);
            tray_handle.get_item(&self.menu_items.status_item)
                .set_title(&status_text)
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to update status item: {}", e)))?;
            
            // Update optimization toggle based on state
            let (toggle_text, toggle_enabled, toggle_selected) = match status.state {
                SystemState::Learning => {
                    if let Some(progress) = &status.data_collection_progress {
                        let _remaining = progress.days_needed - progress.days_collected;
                        ("Enable Optimization".to_string(), false, false)
                    } else {
                        ("Enable Optimization".to_string(), false, false)
                    }
                }
                SystemState::Optimizing => ("Disable Optimization".to_string(), true, true),
                SystemState::Monitoring => ("Enable Optimization".to_string(), true, false),
                SystemState::Inactive => ("Enable Optimization".to_string(), true, false),
                SystemState::Error(_) => ("Enable Optimization".to_string(), false, false),
            };
            
            let toggle_item = tray_handle.get_item(&self.menu_items.toggle_optimization);
            toggle_item.set_title(&toggle_text)
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to update toggle text: {}", e)))?;
            
            if toggle_enabled {
                toggle_item.set_enabled(true)
                    .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to enable toggle: {}", e)))?;
            } else {
                toggle_item.set_enabled(false)
                    .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to disable toggle: {}", e)))?;
            }

            // Reflect selected state where supported (macOS checkmark)
            let _ = toggle_item.set_selected(toggle_selected);
        }
        
        Ok(())
    }
    
    // formatting helpers moved to `ui::status`
    
    /// Shows notification to user following Apple's notification guidelines
    pub async fn show_notification(&self, title: &str, message: &str) -> Result<()> {
        if let Some(app_handle) = &self.app_handle {
            debug!("Showing notification: {} - {}", title, message);
            
            Notification::new(&app_handle.config().tauri.bundle.identifier)
                .title(title)
                .body(message)
                .icon("icons/icon.png")
                .show()
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to show notification: {}", e)))?;
        }
        Ok(())
    }
    
    /// Gets the current status
    pub async fn get_current_status(&self) -> SystemStatus {
        self.current_status.read().await.clone()
    }
    
    /// Handles system tray events
    pub async fn handle_tray_event(&self, event: SystemTrayEvent) -> Result<()> {
        match event {
            SystemTrayEvent::LeftClick { position, size: _, .. } => {
                debug!("Tray left-clicked - toggling popover panel");
                if let Some(app_handle) = &self.app_handle {
                    let x = position.x as f64;
                    let y = position.y as f64;
                    if let Err(e) = PanelInterface::toggle_at_tray_position(app_handle, x, y) {
                        tracing::warn!("Failed to toggle panel: {}", e);
                    }
                }
            }
            SystemTrayEvent::RightClick { position: _, size: _, .. } => {
                debug!("Tray right-clicked - showing context menu");
                // Right click shows context menu (handled automatically)
            }
            SystemTrayEvent::DoubleClick { position: _, size: _, .. } => {
                debug!("Tray double-clicked");
            }
            SystemTrayEvent::MenuItemClick { id, .. } => {
                debug!("Menu item clicked: {}", id);
                self.handle_menu_click(&id).await?;
            }
            _ => {
                debug!("Unhandled tray event");
            }
        }
        Ok(())
    }
    
    // No main window management in tray-only mode
    
    /// Handles menu item clicks
    async fn handle_menu_click(&self, item_id: &str) -> Result<()> {
        match item_id {
            id if id == self.menu_items.toggle_optimization => {
                info!("Optimization toggle clicked");
                self.handle_optimization_toggle().await?;
            }
            id if id == self.menu_items.run_speedtest => {
                info!("Run Speedtest Now clicked");
                if let Some(app_handle) = &self.app_handle {
                    let repo = app_handle.state::<Arc<Repository>>();
                    let shared = app_handle.state::<SharedAppState>();
                    let cfg = AppConfig::load().await? .advanced.speedtest_runner;
                    let runner = SpeedtestRunner::new(app_handle.clone(), Arc::clone(&repo), Arc::clone(&shared), cfg);
                    tokio::spawn(async move { let _ = runner.run_once().await; });
                }
            }
            // Advanced menu removed
            id if id == self.menu_items.quit => {
                info!("Quit clicked");
                self.handle_quit().await?;
            }
            _ => {
                debug!("Unknown menu item clicked: {}", item_id);
            }
        }
        Ok(())
    }

    // Live metrics and keeper rows removed from native menu
    
    /// Handles optimization toggle
    async fn handle_optimization_toggle(&self) -> Result<()> {
        let current_status = self.current_status.read().await.clone();
        
        match current_status.state {
            SystemState::Optimizing => {
                info!("Disabling optimization");
                self.show_notification("SpeedKarma", "Optimization disabled").await?;
                if let Some(app_handle) = &self.app_handle {
                    let state = app_handle.state::<crate::core::app_state::SharedAppState>();
                    let mut guard = state.write().await;
                    guard.optimization_mode = OptimizationMode::Disabled;
                }
            }
            SystemState::Monitoring | SystemState::Inactive => {
                info!("Enabling optimization");
                self.show_notification("SpeedKarma", "Optimization enabled").await?;
                if let Some(app_handle) = &self.app_handle {
                    let state = app_handle.state::<crate::core::app_state::SharedAppState>();
                    let mut guard = state.write().await;
                    guard.optimization_mode = OptimizationMode::Enabled;
                }
            }
            SystemState::Learning => {
                warn!("Cannot toggle optimization while learning");
                self.show_notification("SpeedKarma", "Still learning network patterns").await?;
            }
            SystemState::Error(_) => {
                warn!("Cannot toggle optimization due to error");
                self.show_notification("SpeedKarma", "Cannot enable optimization - check status").await?;
            }
        }
        
        Ok(())
    }
    
    // Advanced interface removed in tray-only mode
    
    /// Handles application quit
    async fn handle_quit(&self) -> Result<()> {
        info!("Application quit requested");
        
        if let Some(app_handle) = &self.app_handle {
            app_handle.exit(0);
        }
        
        Ok(())
    }
}