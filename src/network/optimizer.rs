use crate::core::app_state::{OptimizationMode, SharedAppState};
use crate::core::error::Result;
use crate::data::repository::Repository;
use crate::network::keeper::ThroughputKeeper;
use crate::network::stealth::StealthEngine;
use chrono::Utc;
use std::sync::Arc;

/// Network optimization engine interface
pub trait NetworkOptimizer {
    /// Starts network optimization with current strategy
    async fn start_optimization(&self) -> Result<()>;
    
    /// Stops all optimization activities
    async fn stop_optimization(&self) -> Result<()>;
    
    /// Gets current effectiveness metrics
    async fn get_effectiveness(&self) -> Result<EffectivenessMetrics>;
    
    /// Adapts to changing network conditions
    async fn adapt_to_conditions(&self, conditions: NetworkConditions) -> Result<()>;
}

/// Metrics showing optimization effectiveness
#[derive(Debug, Clone)]
pub struct EffectivenessMetrics {
    pub improvement_factor: f64,
    pub baseline_speed: f64,
    pub optimized_speed: f64,
    pub confidence: f64,
}

/// Current network conditions for adaptation
#[derive(Debug, Clone)]
pub struct NetworkConditions {
    pub current_speed: f64,
    pub latency: u32,
    pub packet_loss: f64,
    pub congestion_level: f64,
}

/// Default network optimizer implementation
pub struct DefaultNetworkOptimizer {
    repository: Arc<Repository>,
    shared_state: SharedAppState,
    keeper: Arc<ThroughputKeeper>,
    stealth: Arc<StealthEngine>,
}

impl DefaultNetworkOptimizer {
    pub fn new(
        repository: Arc<Repository>,
        shared_state: SharedAppState,
        keeper: Arc<ThroughputKeeper>,
        stealth: Arc<StealthEngine>,
    ) -> Self {
        Self { repository, shared_state, keeper, stealth }
    }
}

impl NetworkOptimizer for DefaultNetworkOptimizer {
    async fn start_optimization(&self) -> Result<()> {
        {
            let mut s = self.shared_state.write().await;
            s.optimization_mode = OptimizationMode::Enabled;
        }
        // Start background keeper loop
        self.keeper.clone().start();
        // Start stealth and loop
        let stealth = self.stealth.clone();
        stealth.start().await?;
        tokio::spawn(async move { stealth.run_stealth_loop().await; });
        Ok(())
    }
    
    async fn stop_optimization(&self) -> Result<()> {
        {
            let mut s = self.shared_state.write().await;
            s.optimization_mode = OptimizationMode::Disabled;
        }
        self.keeper.stop().await;
        let _ = self.stealth.stop().await;
        Ok(())
    }
    
    async fn get_effectiveness(&self) -> Result<EffectivenessMetrics> {
        let since = Utc::now() - chrono::Duration::hours(6);
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        let (mut base_sum, mut base_n, mut opt_sum, mut opt_n) = (0.0, 0u32, 0.0, 0u32);
        for m in measurements {
            if m.optimization_active { opt_sum += m.download_mbps; opt_n += 1; }
            else { base_sum += m.download_mbps; base_n += 1; }
        }
        let baseline = if base_n > 0 { base_sum / base_n as f64 } else { 0.0 };
        let optimized = if opt_n > 0 { opt_sum / opt_n as f64 } else { 0.0 };
        let improvement = if baseline > 0.0 && optimized > 0.0 { optimized / baseline } else { 1.0 };
        let confidence = ((opt_n + base_n) as f64 / 100.0).min(1.0);
        Ok(EffectivenessMetrics {
            improvement_factor: improvement,
            baseline_speed: baseline,
            optimized_speed: optimized,
            confidence,
        })
    }
    
    async fn adapt_to_conditions(&self, _conditions: NetworkConditions) -> Result<()> {
        // Placeholder: hook for dynamic adjustments
        Ok(())
    }
}