use crate::core::app_state::{OptimizationMode, SharedAppState};
use crate::core::config::ThroughputKeeperConfig;
use crate::core::error::Result;
use crate::data::repository::Repository;
use crate::data::models::StealthLevel;
use chrono::{DateTime, Utc, Duration as ChronoDuration, Timelike};
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE, ACCEPT_ENCODING, CACHE_CONTROL, CONNECTION, RANGE, PRAGMA};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{info, warn, debug, error};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeeperCadence {
    Warmup,
    Steady,
    Recovery,
    Suspended,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeeperProgressPayload {
    pub next_burst_in_s: u32,
    pub last_burst_kb: u32,
    pub hour_used_mb: f64,
    pub hour_budget_mb: f64,
    pub cadence: String,
}

pub struct ThroughputKeeper {
    repository: Arc<Repository>,
    shared_state: SharedAppState,
    app_handle: AppHandle,
    config: Arc<RwLock<ThroughputKeeperConfig>>,
    is_running: Arc<RwLock<bool>>,
    hourly_budget_used_mb: Arc<RwLock<f64>>, // resets every hour
    last_reset: Arc<RwLock<DateTime<Utc>>>,
}

impl ThroughputKeeper {
    pub fn new(app_handle: AppHandle, repository: Arc<Repository>, shared_state: SharedAppState, config: ThroughputKeeperConfig) -> Self {
        Self {
            repository,
            shared_state,
            app_handle,
            config: Arc::new(RwLock::new(config)),
            is_running: Arc::new(RwLock::new(false)),
            hourly_budget_used_mb: Arc::new(RwLock::new(0.0)),
            last_reset: Arc::new(RwLock::new(Utc::now())),
        }
    }

    pub async fn update_config(&self, cfg: ThroughputKeeperConfig) { *self.config.write().await = cfg; }

    fn jitter_secs(base: u64, jitter_frac: f64) -> u64 {
        // Compute jitter using time-based hash to keep this function non-async and non-blocking
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as u64)
            .unwrap_or(0);
        let jitter = (base as f64 * jitter_frac) as i64;
        let sign = if (nanos & 1) == 0 { -1 } else { 1 };
        let magnitude = (nanos % (jitter.abs() as u64 + 1)) as i64;
        let delta = sign * magnitude.min(jitter.abs());
        let val = base as i64 + delta;
        val.max(1) as u64
    }

    fn user_agent() -> &'static str { "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15" }

    fn build_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(Self::user_agent()));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers.insert(PRAGMA, HeaderValue::from_static("no-cache"));
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        headers
    }

    async fn choose_burst_size_kb(cfg: &ThroughputKeeperConfig, cadence: &KeeperCadence, last_size: u32) -> u32 {
        let mut sizes = cfg.burst_sizes_kb.clone();
        sizes.sort();
        let min_size = *sizes.first().unwrap_or(&64);
        let max_size = *sizes.last().unwrap_or(&256);
        match cadence {
            KeeperCadence::Warmup => min_size.max(64),
            KeeperCadence::Recovery => (last_size * 2).min(max_size),
            KeeperCadence::Steady => last_size,
            KeeperCadence::Suspended => 0,
        }
    }

    fn should_quiet_hour(cfg: &ThroughputKeeperConfig) -> bool {
        if let Some(hours) = &cfg.quiet_hours {
            let now = Utc::now();
            let hour = now.hour() as u8;
            // Quiet hours represent disallowed hours; suspend if current hour is listed
            return hours.contains(&hour);
        }
        false
    }

    async fn reset_budget_if_needed(&self) {
        let mut last_reset = self.last_reset.write().await;
        let now = Utc::now();
        if now.signed_duration_since(*last_reset) >= ChronoDuration::hours(1) {
            *self.hourly_budget_used_mb.write().await = 0.0;
            *last_reset = now;
            debug!("ThroughputKeeper: hourly budget reset");
        }
    }

    async fn get_recent_metrics(&self) -> (f64, f64) {
        // Returns (down_mbps_avg_last_20s, trend_ratio_20s_vs_prev_20s)
        let now = Utc::now();
        let recent = self.repository.get_speed_measurements_since(now - ChronoDuration::seconds(40)).await;
        if let Ok(samples) = recent {
            let mut last20 = Vec::new();
            let mut prev20 = Vec::new();
            for m in samples {
                if m.timestamp > now - ChronoDuration::seconds(20) { last20.push(m.download_mbps); }
                else { prev20.push(m.download_mbps); }
            }
            let avg_last = if last20.is_empty() { 0.0 } else { last20.iter().sum::<f64>() / last20.len() as f64 };
            let avg_prev = if prev20.is_empty() { avg_last } else { prev20.iter().sum::<f64>() / prev20.len() as f64 };
            let trend = if avg_prev <= 0.0001 { 1.0 } else { avg_last / avg_prev };
            return (avg_last, trend);
        }
        (0.0, 1.0)
    }

    async fn pick_target_url(&self, stealth_level: &StealthLevel) -> Option<String> {
        // Prefer active speedtest servers; fallback to a CDN-like path
        if let Ok(servers) = self.repository.get_active_speedtest_servers().await {
            if let Some(s) = servers.first() {
                let scheme = if matches!(stealth_level, StealthLevel::Maximum) { "https" } else { "http" };
                let nonce = (Utc::now().timestamp_millis() as u64) & 0xFFFF_FFFF;
                return Some(format!("{}://{}:{}/download?nocache={}", scheme, s.host, s.port, nonce));
            }
        }
        Some("https://speed.cloudflare.com/__down?bytes=262144".to_string())
    }

    async fn perform_burst(&self, size_kb: u32, stealth_level: &StealthLevel) -> Result<()> {
        let url = match self.pick_target_url(stealth_level).await { Some(u) => u, None => return Ok(()) };
        let mut headers = Self::build_headers();
        // Randomize Range header, mimic partial GET/HEAD
        let size_bytes = (size_kb as u64) * 1024;
        let start = (Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64) % 2048u64;
        let end = start + size_bytes.saturating_sub(1);
        let range_val = format!("bytes={}-{}", start, end);
        headers.insert(RANGE, HeaderValue::from_str(&range_val).unwrap_or(HeaderValue::from_static("bytes=0-1023")));

        let client = reqwest::Client::builder()
            .http2_prior_knowledge()
            .pool_idle_timeout(Duration::from_secs(30))
            .build()?;

        // Randomly pick method (compute randomness in a local scope to avoid non-Send across await)
        let head_first = {
            let mut rng = rand::thread_rng();
            rng.gen_bool(0.4)
        };
        if head_first {
            let _ = client.head(&url).headers(headers.clone()).send().await;
            let pause_ms: u64 = {
                let mut rng = rand::thread_rng();
                rng.gen_range(20..80)
            };
            sleep(Duration::from_millis(pause_ms)).await;
        }
        let _ = client.get(&url).headers(headers).send().await;
        Ok(())
    }

    pub fn start(self: Arc<Self>) {
        let keeper = Arc::clone(&self);
        tauri::async_runtime::spawn(async move { keeper.run_loop().await; });
    }

    pub async fn stop(&self) {
        *self.is_running.write().await = false;
    }

    async fn run_loop(&self) {
        {
            let mut r = self.is_running.write().await;
            if *r { return; }
            *r = true;
        }

        info!("ThroughputKeeper started");
        let mut cadence = KeeperCadence::Warmup;
        let mut last_change = Instant::now();
        let mut last_burst_kb: u32 = 64;

        loop {
            if !*self.is_running.read().await { break; }
            // Check optimization and config enable
            let enabled = {
                let s = self.shared_state.read().await;
                matches!(s.optimization_mode, OptimizationMode::Enabled)
            };
            let cfg = self.config.read().await.clone();
            if !enabled || !cfg.enabled || Self::should_quiet_hour(&cfg) {
                cadence = KeeperCadence::Suspended;
                self.emit_progress(0, 0, *self.hourly_budget_used_mb.read().await, cfg.hourly_budget_mb, &cadence).await;
                sleep(Duration::from_secs(3)).await;
                continue;
            }

            // Budget checks
            self.reset_budget_if_needed().await;
            let used = *self.hourly_budget_used_mb.read().await;
            if used >= cfg.hourly_budget_mb {
                cadence = KeeperCadence::Suspended;
                self.emit_progress(0, 0, used, cfg.hourly_budget_mb, &cadence).await;
                sleep(Duration::from_secs(30)).await;
                continue;
            }

            // Read metrics trend
            let (down_avg, trend) = self.get_recent_metrics().await; // trend ~ 1.0 stable, <1 drop
            let drop_detected = trend <= (1.0 - cfg.tighten_threshold_drop);
            let stable_enough = last_change.elapsed().as_secs() as u32 >= cfg.relax_threshold_stability_s;

            // Cadence state machine
            cadence = match cadence {
                KeeperCadence::Warmup => {
                    if stable_enough { KeeperCadence::Steady } else { KeeperCadence::Warmup }
                }
                KeeperCadence::Steady => {
                    if drop_detected { KeeperCadence::Recovery } else { KeeperCadence::Steady }
                }
                KeeperCadence::Recovery => {
                    if stable_enough { KeeperCadence::Steady } else { KeeperCadence::Recovery }
                }
                KeeperCadence::Suspended => {
                    // when re-enabled, go to Warmup
                    KeeperCadence::Warmup
                }
            };
            if matches!(cadence, KeeperCadence::Recovery) || matches!(cadence, KeeperCadence::Warmup) { last_change = Instant::now(); }

            // Interval and size selection
            let base_interval = match cadence {
                KeeperCadence::Warmup => 5,
                KeeperCadence::Recovery => 5,
                KeeperCadence::Steady => 10,
                KeeperCadence::Suspended => 15,
            } as u64;
            let mut interval_s = base_interval;
            // Budget-aware stretching as we approach cap
            let budget_ratio = used / (cfg.hourly_budget_mb.max(0.001));
            if budget_ratio > 0.8 { interval_s += 5; }
            if budget_ratio > 0.9 { interval_s += 10; }
            // Slight jitter
            interval_s = Self::jitter_secs(interval_s, 0.15);

            let stealth_level = match self.repository.get_best_optimization_strategy().await {
                Ok(Some(s)) => s.stealth_level,
                _ => StealthLevel::Medium,
            };

            let size_kb = Self::choose_burst_size_kb(&cfg, &cadence, last_burst_kb).await;
            if size_kb == 0 { sleep(Duration::from_secs(interval_s)).await; continue; }

            // Perform burst with backoff
            let burst_bytes_mb = (size_kb as f64) / 1024.0;
            let mut attempt = 0u8;
            let mut success = false;
            while attempt < 3 {
                match self.perform_burst(size_kb, &stealth_level).await {
                    Ok(_) => { success = true; break; },
                    Err(e) => { warn!("ThroughputKeeper burst failed: {}", e); sleep(Duration::from_secs(2u64.pow(attempt as u32))).await; }
                }
                attempt += 1;
            }
            if success {
                // account budget
                {
                    let mut used = self.hourly_budget_used_mb.write().await;
                    *used += burst_bytes_mb;
                }
                last_burst_kb = size_kb;
            }

            // Emit UI progress
            let used_mb = *self.hourly_budget_used_mb.read().await;
            self.emit_progress(interval_s as u32, last_burst_kb, used_mb, cfg.hourly_budget_mb, &cadence).await;

            sleep(Duration::from_secs(interval_s)).await;
        }

        info!("ThroughputKeeper stopped");
    }

    async fn emit_progress(&self, next_in_s: u32, last_kb: u32, used_mb: f64, budget_mb: f64, cadence: &KeeperCadence) {
        let payload = KeeperProgressPayload {
            next_burst_in_s: next_in_s,
            last_burst_kb: last_kb,
            hour_used_mb: (used_mb * 100.0).round() / 100.0,
            hour_budget_mb: (budget_mb * 100.0).round() / 100.0,
            cadence: match cadence { KeeperCadence::Warmup => "warmup", KeeperCadence::Steady => "steady", KeeperCadence::Recovery => "recovery", KeeperCadence::Suspended => "suspended" }.to_string(),
        };
        let _ = self.app_handle.emit_all("keeper_progress", payload);
    }
}


