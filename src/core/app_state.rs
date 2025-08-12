use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct AppControlState {
    pub optimization_mode: OptimizationMode,
}

impl Default for AppControlState {
    fn default() -> Self { Self { optimization_mode: OptimizationMode::Disabled } }
}

pub type SharedAppState = Arc<RwLock<AppControlState>>;


