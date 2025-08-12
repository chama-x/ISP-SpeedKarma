use crate::core::app_state::{OptimizationMode, SharedAppState};
use crate::core::config::DisguiseModeConfig;
use crate::data::repository::Repository;
use crate::data::models::StealthLevel;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tracing::{info, warn, debug};

/// Global disguise proxy: best-effort approach that periodically warms up and can be wired to an HTTP proxy later.
pub struct DisguiseProxy {
    app: AppHandle,
    repository: Arc<Repository>,
    shared: SharedAppState,
    config: DisguiseModeConfig,
}

impl DisguiseProxy {
    pub fn new(app: AppHandle, repository: Arc<Repository>, shared: SharedAppState, config: DisguiseModeConfig) -> Self {
        Self { app, repository, shared, config }
    }

    /// Placeholder: future hook to route app HTTP requests through a header-masquerading client.
    pub async fn is_enabled(&self) -> bool { self.config.enabled }

    /// Background pulse that mimics speedtest headers to keep cache/paths primed for general traffic
    pub fn start(self: Arc<Self>) {
        tauri::async_runtime::spawn(async move {
            loop {
                if !self.config.enabled { tokio::time::sleep(Duration::from_secs(10)).await; continue; }
                let enabled = { let s = self.shared.read().await; matches!(s.optimization_mode, OptimizationMode::Enabled) };
                if !enabled { tokio::time::sleep(Duration::from_secs(5)).await; continue; }
                // Warm path using stealth server selection
                let stealth_level = match self.repository.get_best_optimization_strategy().await {
                    Ok(Some(s)) => s.stealth_level,
                    _ => StealthLevel::Medium,
                };
                let _ = stealth_level; // reserved for future use
                // For now this is a no-op; real routing hook would be here.
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });
    }
}


