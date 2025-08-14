use crate::core::app_state::{OptimizationMode, SharedAppState};
use crate::core::error::Result;
use crate::data::repository::Repository;
use crate::ui::tray::SystemTray;
use chrono::{Utc, Duration as ChronoDuration};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressPhase {
    Resolving,
    Connecting,
    Tuning,
    Optimizing,
    Inactive,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerInfo {
    pub host: String,
    pub region: String,
    pub ip: Option<String>,
    pub stealth_level: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveMetrics {
    pub down_mbps: f64,
    pub up_mbps: f64,
    pub latency_ms: u32,
    pub improvement: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationProgressPayload {
    pub phase: ProgressPhase,
    pub phase_percent: u8,
    pub server: ServerInfo,
    pub metrics: LiveMetrics,
    pub next_rotation_s: u32,
    pub confidence: f64,
    pub timestamp: String,
}

/// Starts a background task that emits `optimization_progress` events to the UI
pub fn start_progress_broadcaster(
    app_handle: AppHandle,
    repository: Arc<Repository>,
    shared_state: SharedAppState,
) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        let mut since_enabled: Option<Instant> = None;
        // Determine rotation interval from the best strategy (fallback to default 10 min)
        let default_rotation_s = 10 * 60u32;
        let mut rotation_seconds: u32 = match repository.get_best_optimization_strategy().await {
            Ok(Some(s)) => s.server_rotation_interval_minutes.saturating_mul(60),
            _ => default_rotation_s,
        };
        if rotation_seconds == 0 { rotation_seconds = default_rotation_s; }
        let mut next_rotation_s: u32 = rotation_seconds;
        loop {
            interval.tick().await;

            // Check if optimization is enabled
            let enabled = {
                let guard = shared_state.read().await;
                matches!(guard.optimization_mode, OptimizationMode::Enabled)
            };

            if !enabled {
                since_enabled = None;
            // Emit inactive state
            let payload = OptimizationProgressPayload {
                phase: ProgressPhase::Inactive,
                phase_percent: 0,
                server: ServerInfo { host: "".into(), region: "".into(), ip: None, stealth_level: None },
                metrics: LiveMetrics { down_mbps: 0.0, up_mbps: 0.0, latency_ms: 0, improvement: 1.0 },
                next_rotation_s: 0,
                confidence: 0.0,
                timestamp: Utc::now().to_rfc3339(),
            };
            let _ = app_handle.emit_all("optimization_progress", payload.clone());
            // Native tray metrics row removed; HTML popover shows live metrics
                continue;
            }

            // Phase machine for first seconds after enabling
            if since_enabled.is_none() { since_enabled = Some(Instant::now()); }
            let elapsed = since_enabled.unwrap().elapsed().as_secs_f64();
            let (phase, percent) = if elapsed < 3.0 {
                (ProgressPhase::Resolving, ((elapsed / 3.0) * 30.0) as u8)
            } else if elapsed < 7.0 {
                (ProgressPhase::Connecting, (30.0 + ((elapsed - 3.0) / 4.0) * 40.0) as u8)
            } else if elapsed < 10.0 {
                (ProgressPhase::Tuning, (70.0 + ((elapsed - 7.0) / 3.0) * 25.0) as u8)
            } else {
                (ProgressPhase::Optimizing, 100)
            };

            // Compute lightweight live metrics from last 2 minutes
            let since = Utc::now() - ChronoDuration::minutes(2);
            let measurements = repository.get_speed_measurements_since(since).await.unwrap_or_default();
            let mut down_sum = 0.0;
            let mut up_sum = 0.0;
            let mut count = 0.0;
            for m in &measurements {
                down_sum += m.download_mbps;
                up_sum += m.upload_mbps;
                count += 1.0;
            }
            let (down_mbps, up_mbps) = if count > 0.0 { (down_sum / count, up_sum / count) } else { (0.0, 0.0) };

            // Estimate improvement vs a simple baseline (avg of last 30 minutes unoptimized)
            let since_baseline = Utc::now() - ChronoDuration::minutes(30);
            let baseline_meas = repository.get_speed_measurements_since(since_baseline).await.unwrap_or_default();
            let mut base_sum = 0.0; let mut base_count = 0.0;
            for m in &baseline_meas { if !m.optimization_active { base_sum += m.download_mbps; base_count += 1.0; } }
            let baseline = if base_count > 0.0 { base_sum / base_count } else { 0.0 };
            let improvement = if baseline > 0.0 && down_mbps > 0.0 { (down_mbps / baseline).max(0.5).min(5.0) } else { 1.0 };

            // Confidence proxy: number of samples last 2 minutes
            let confidence = (count / 10.0).min(1.0);

            // Decrease rotation countdown in steady state
            if matches!(phase, ProgressPhase::Optimizing) && next_rotation_s > 0 { next_rotation_s = next_rotation_s.saturating_sub(2); }
            if next_rotation_s == 0 { next_rotation_s = rotation_seconds; }

            let payload = OptimizationProgressPayload {
                phase,
                phase_percent: percent.min(100),
                server: ServerInfo { host: "Auto route".into(), region: "".into(), ip: None, stealth_level: Some("High".into()) },
                metrics: LiveMetrics { down_mbps, up_mbps, latency_ms: 0, improvement },
                next_rotation_s,
                confidence,
                timestamp: Utc::now().to_rfc3339(),
            };

            let _ = app_handle.emit_all("optimization_progress", payload);

            // Native tray metrics row removed; HTML popover shows live metrics
        }
    });
}


