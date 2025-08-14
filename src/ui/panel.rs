use crate::core::error::{Result, SpeedKarmaError};
use tauri::{AppHandle, Manager, Window, WindowBuilder, WindowUrl, PhysicalPosition, Position, WindowEvent};

/// Lightweight controller for the Control Center-like popover panel
pub struct PanelInterface;

impl PanelInterface {
    pub fn new() -> Self { Self }

    /// Ensure the panel window exists; create if missing
    fn ensure_panel(app_handle: &AppHandle) -> Result<Window> {
        if let Some(window) = app_handle.get_window("panel") {
            return Ok(window);
        }

        let window = WindowBuilder::new(
            app_handle,
            "panel",
            WindowUrl::App("panel.html".into()),
        )
        .title("SpeedKarma")
        .decorations(false)
        .resizable(false)
        .visible(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .inner_size(420.0, 420.0)
        .build()
        .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to create panel window: {}", e)))?;

        // Hide on blur to emulate Control Center behavior
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                let _ = window_clone.hide();
            }
        });

        Ok(window)
    }

    /// Show the panel near a given tray click position (OS pixels)
    pub fn show_at_tray_position(app_handle: &AppHandle, position_x: f64, position_y: f64) -> Result<()> {
        let window = Self::ensure_panel(app_handle)?;

        // Offset so the panel appears below and slightly left of the tray icon
        let panel_width: f64 = 420.0;
        let x = (position_x - panel_width + 24.0).max(8.0);
        let y = (position_y + 8.0).max(8.0);

        window
            .set_position(Position::Physical(PhysicalPosition { x: x as i32, y: y as i32 }))
            .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to position panel: {}", e)))?;

        window
            .show()
            .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to show panel: {}", e)))?;
        window
            .set_focus()
            .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to focus panel: {}", e)))?;

        Ok(())
    }

    /// Show the panel (without repositioning) and focus it
    pub fn show(app_handle: &AppHandle) -> Result<()> {
        let window = Self::ensure_panel(app_handle)?;
        window
            .show()
            .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to show panel: {}", e)))?;
        window
            .set_focus()
            .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to focus panel: {}", e)))?;
        Ok(())
    }

    /// Toggle visibility; if visible hide, else show near a position
    pub fn toggle_at_tray_position(app_handle: &AppHandle, position_x: f64, position_y: f64) -> Result<()> {
        let window = Self::ensure_panel(app_handle)?;
        match window.is_visible() {
            Ok(true) => {
                window
                    .hide()
                    .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to hide panel: {}", e)))?
            }
            _ => {
                Self::show_at_tray_position(app_handle, position_x, position_y)?;
            }
        }
        Ok(())
    }

    /// Hide if open
    pub fn hide(app_handle: &AppHandle) -> Result<()> {
        if let Some(window) = app_handle.get_window("panel") {
            window
                .hide()
                .map_err(|e| SpeedKarmaError::SystemError(format!("Failed to hide panel: {}", e)))?;
        }
        Ok(())
    }
}


