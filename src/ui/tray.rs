use crate::core::error::{Result, SpeedKarmaError};
use crate::core::intelligence::{SystemStatus, SystemState};
use crate::ui::advanced::AdvancedInterface;
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

/// Menu item identifiers for system tray
#[derive(Debug)]
struct SystemTrayMenuItems {
    status_item: String,
    separator1: String,
    toggle_optimization: String,
    advanced: String,
    separator2: String,
    quit: String,
}

impl Default for SystemTrayMenuItems {
    fn default() -> Self {
        Self {
            status_item: "status".to_string(),
            separator1: "sep1".to_string(),
            toggle_optimization: "toggle_opt".to_string(),
            advanced: "advanced".to_string(),
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
        
        // Create menu items with Apple-inspired design
        let status_item = CustomMenuItem::new(&menu_items.status_item, "◉ SpeedKarma")
            .disabled(); // Status item is non-clickable
        
        let toggle_optimization = CustomMenuItem::new(&menu_items.toggle_optimization, "○ Enable optimization");
        let advanced = CustomMenuItem::new(&menu_items.advanced, "Advanced...");
        let quit = CustomMenuItem::new(&menu_items.quit, "Quit SpeedKarma");
        
        // Build menu with progressive disclosure
        let tray_menu = SystemTrayMenu::new()
            .add_item(status_item)
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(toggle_optimization)
            .add_native_item(SystemTrayMenuItem::Separator)
            .add_item(advanced)
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
        let tooltip = self.format_tooltip(&status);
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
            let status_text = self.format_status_menu_item(status);
            tray_handle.get_item(&self.menu_items.status_item)
                .set_title(&status_text)
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to update status item: {}", e)))?;
            
            // Update optimization toggle based on state
            let (toggle_text, toggle_enabled) = match status.state {
                SystemState::Learning => {
                    if let Some(progress) = &status.data_collection_progress {
                        let remaining = progress.days_needed - progress.days_collected;
                        (format!("○ Enable optimization\n   (Available in {} days)", remaining), false)
                    } else {
                        ("○ Enable optimization\n   (Learning patterns...)".to_string(), false)
                    }
                }
                SystemState::Optimizing => ("● Disable optimization".to_string(), true),
                SystemState::Monitoring => ("○ Enable optimization".to_string(), true),
                SystemState::Inactive => ("○ Enable optimization".to_string(), true),
                SystemState::Error(_) => ("○ Enable optimization\n   (Error - check logs)".to_string(), false),
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
        }
        
        Ok(())
    }
    
    /// Formats the status for the menu item following Apple's design language
    fn format_status_menu_item(&self, status: &SystemStatus) -> String {
        match status.state {
            SystemState::Learning => {
                if let Some(progress) = &status.data_collection_progress {
                    format!("◉ SpeedKarma\nLearning patterns ({} of {} days)", 
                           progress.days_collected, progress.days_needed)
                } else {
                    "◉ SpeedKarma\nLearning patterns".to_string()
                }
            }
            SystemState::Optimizing => {
                if let Some(effectiveness) = &status.effectiveness {
                    format!("◉ SpeedKarma\nOptimizing ({}x improvement)", 
                           effectiveness.improvement_factor)
                } else {
                    "◉ SpeedKarma\nOptimizing".to_string()
                }
            }
            SystemState::Monitoring => "◉ SpeedKarma\nMonitoring".to_string(),
            SystemState::Inactive => "◉ SpeedKarma\nInactive".to_string(),
            SystemState::Error(ref err) => format!("◉ SpeedKarma\nError: {}", err),
        }
    }
    
    /// Formats the tooltip text
    fn format_tooltip(&self, status: &SystemStatus) -> String {
        match status.state {
            SystemState::Learning => {
                if let Some(progress) = &status.data_collection_progress {
                    format!("SpeedKarma - Learning patterns ({:.0}% complete)", 
                           progress.progress_percentage)
                } else {
                    "SpeedKarma - Learning network patterns".to_string()
                }
            }
            SystemState::Optimizing => {
                if let Some(effectiveness) = &status.effectiveness {
                    format!("SpeedKarma - Optimizing ({}x improvement)", 
                           effectiveness.improvement_factor)
                } else {
                    "SpeedKarma - Optimizing network".to_string()
                }
            }
            SystemState::Monitoring => "SpeedKarma - Monitoring network".to_string(),
            SystemState::Inactive => "SpeedKarma - Inactive".to_string(),
            SystemState::Error(ref err) => format!("SpeedKarma - Error: {}", err),
        }
    }
    
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
            SystemTrayEvent::LeftClick { position: _, size: _, .. } => {
                debug!("Tray left-clicked - showing menu");
                // On macOS, left click typically shows the menu
                // This is handled automatically by Tauri
            }
            SystemTrayEvent::RightClick { position: _, size: _, .. } => {
                debug!("Tray right-clicked - showing context menu");
                // Right click shows context menu (handled automatically)
            }
            SystemTrayEvent::DoubleClick { position: _, size: _, .. } => {
                debug!("Tray double-clicked - toggling main window");
                self.toggle_main_window().await?;
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
    
    /// Toggles the main window visibility
    async fn toggle_main_window(&self) -> Result<()> {
        if let Some(app_handle) = &self.app_handle {
            if let Some(window) = app_handle.get_window("main") {
                if window.is_visible().unwrap_or(false) {
                    window.hide()
                        .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to hide window: {}", e)))?;
                } else {
                    window.show()
                        .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to show window: {}", e)))?;
                    window.set_focus()
                        .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to focus window: {}", e)))?;
                }
            }
        }
        Ok(())
    }
    
    /// Handles menu item clicks
    async fn handle_menu_click(&self, item_id: &str) -> Result<()> {
        match item_id {
            id if id == self.menu_items.toggle_optimization => {
                info!("Optimization toggle clicked");
                self.handle_optimization_toggle().await?;
            }
            id if id == self.menu_items.advanced => {
                info!("Advanced settings clicked");
                self.show_advanced_interface().await?;
            }
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
    
    /// Handles optimization toggle
    async fn handle_optimization_toggle(&self) -> Result<()> {
        let current_status = self.current_status.read().await.clone();
        
        match current_status.state {
            SystemState::Optimizing => {
                info!("Disabling optimization");
                self.show_notification("SpeedKarma", "Optimization disabled").await?;
                // TODO: Send disable command to optimization engine
            }
            SystemState::Monitoring | SystemState::Inactive => {
                info!("Enabling optimization");
                self.show_notification("SpeedKarma", "Optimization enabled").await?;
                // TODO: Send enable command to optimization engine
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
    
    /// Shows the advanced interface
    async fn show_advanced_interface(&self) -> Result<()> {
        info!("Showing advanced interface");
        
        if let Some(app_handle) = &self.app_handle {
            let advanced = AdvancedInterface::new();
            // Ignore errors beyond notification; we already log them
            if let Err(e) = advanced.show(app_handle).await {
                warn!("Failed to open advanced interface: {}", e);
                self.show_notification("SpeedKarma", "Unable to open Advanced settings").await?;
            }
        }
        
        Ok(())
    }
    
    /// Handles application quit
    async fn handle_quit(&self) -> Result<()> {
        info!("Application quit requested");
        
        if let Some(app_handle) = &self.app_handle {
            app_handle.exit(0);
        }
        
        Ok(())
    }
}