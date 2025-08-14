use crate::core::app_state::{OptimizationMode, SharedAppState};
use crate::core::config::SpeedtestRunnerConfig;
use crate::core::error::Result;
use crate::data::repository::Repository;
use crate::data::models::{StealthLevel, SpeedMeasurement};
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE, ACCEPT_ENCODING, CACHE_CONTROL, CONNECTION};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, timeout};
use tracing::{info, warn, debug};

#[derive(Debug, Clone, Serialize)]
pub struct SpeedtestProgressPayload {
    pub phase: String,
    pub down_mbps: f64,
    pub up_mbps: f64,
    pub elapsed_s: u32,
}

pub struct SpeedtestRunner {
    app: AppHandle,
    repository: Arc<Repository>,
    shared: SharedAppState,
    config: SpeedtestRunnerConfig,
}

impl SpeedtestRunner {
    pub fn new(app: AppHandle, repository: Arc<Repository>, shared: SharedAppState, config: SpeedtestRunnerConfig) -> Self {
        Self { app, repository, shared, config }
    }

    fn build_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15"));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        headers
    }

    async fn pick_server(&self, stealth_level: &StealthLevel) -> Option<String> {
        if let Ok(servers) = self.repository.get_active_speedtest_servers().await {
            if let Some(s) = servers.first() {
                let scheme = if matches!(stealth_level, StealthLevel::Maximum) { "https" } else { "http" };
                return Some(format!("{}://{}:{}/", scheme, s.host, s.port));
            }
        }
        Some("https://speed.cloudflare.com/".to_string())
    }

    pub async fn run_once(&self) -> Result<()> {
        if !self.config.enabled { return Ok(()); }
        // Allow manual speedtest to run regardless of optimization mode

        // Choose server and client
        let stealth_level = match self.repository.get_best_optimization_strategy().await {
            Ok(Some(s)) => s.stealth_level,
            _ => StealthLevel::Medium,
        };
        let base = match self.pick_server(&stealth_level).await { Some(b)=>b, None=>return Ok(()) };
        let client = reqwest::Client::builder().default_headers(Self::build_headers()).pool_idle_timeout(Duration::from_secs(30)).build()?;

        // Download phase: open parallel streams and fully read bodies until time expires
        let dl_secs = self.config.download_duration_s.max(1);
        let dl_start = std::time::Instant::now();
        let end_time = dl_start + Duration::from_secs(dl_secs as u64);
        let is_cloudflare = base.contains("speed.cloudflare.com");
        let mut tasks = Vec::new();
        let dl_bytes = Arc::new(AtomicU64::new(0));
        for i in 0..self.config.parallel_connections.max(1) as usize {
            let client_cl = client.clone();
            let base_cl = base.clone();
            let dl_bytes_cl = Arc::clone(&dl_bytes);
            tasks.push(tokio::spawn(async move {
                let mut seed: u64 = i as u64 + 1;
                while std::time::Instant::now() < end_time {
                    let url = if is_cloudflare {
                        format!("{}__down?bytes=16777216&seed={}", base_cl, seed)
                    } else {
                        format!("{}speedtest/random4000x4000.jpg?r={}", base_cl, seed)
                    };
                    if let Ok(resp) = client_cl.get(&url).send().await {
                        if let Ok(bytes) = resp.bytes().await {
                            dl_bytes_cl.fetch_add(bytes.len() as u64, Ordering::Relaxed);
                        }
                    }
                    seed = seed.wrapping_add(1);
                }
            }));
        }
        // Emit progress ticks during download
        loop {
            let now = std::time::Instant::now();
            if now >= end_time { break; }
            let elapsed = (dl_secs as u64).saturating_sub((end_time - now).as_secs());
            let _ = self.app.emit_all("speedtest_progress", SpeedtestProgressPayload { phase: "download".into(), down_mbps: 0.0, up_mbps: 0.0, elapsed_s: elapsed as u32 });
            sleep(Duration::from_millis(300)).await;
        }
        for t in tasks { let _ = t.await; }
        let dl_elapsed_s = dl_start.elapsed().as_secs_f64().max(0.001);
        let down_mbps = (dl_bytes.load(Ordering::Relaxed) as f64 * 8.0) / (dl_elapsed_s * 1_000_000.0);

        // Upload phase: push random data to upload endpoints
        let ul_secs = self.config.upload_duration_s.max(1);
        let start_ul = std::time::Instant::now();
        let mut tasks_ul = Vec::new();
        let ul_bytes = Arc::new(AtomicU64::new(0));
        for _i in 0..self.config.parallel_connections.max(1) as usize {
            let url = if is_cloudflare { format!("{}__up", base) } else { format!("{}speedtest/upload.php", base) };
            let body = vec![0u8; 2_000_000]; // ~2MB per request, repeated
            let client_cl = client.clone();
            let ul_bytes_cl = Arc::clone(&ul_bytes);
            let body_len = body.len();
            tasks_ul.push(tokio::spawn(async move {
                let _ = timeout(Duration::from_secs(ul_secs as u64), async {
                    loop {
                        if client_cl.post(&url).body(body.clone()).send().await.is_ok() {
                            ul_bytes_cl.fetch_add(body_len as u64, Ordering::Relaxed);
                        }
                    }
                }).await;
            }));
        }
        loop {
            let elapsed = start_ul.elapsed().as_secs();
            if elapsed >= ul_secs as u64 { break; }
            let _ = self.app.emit_all("speedtest_progress", SpeedtestProgressPayload { phase: "upload".into(), down_mbps: 0.0, up_mbps: 0.0, elapsed_s: elapsed as u32 });
            sleep(Duration::from_millis(300)).await;
        }
        for t in tasks_ul { let _ = t.await; }
        let ul_elapsed_s = start_ul.elapsed().as_secs_f64().max(0.001);
        let up_mbps = (ul_bytes.load(Ordering::Relaxed) as f64 * 8.0) / (ul_elapsed_s * 1_000_000.0);

        // Persist measurement for intelligence engine
        let measurement = SpeedMeasurement::new(down_mbps, up_mbps, 0, true);
        let _ = self.repository.save_speed_measurement(&measurement).await;

        let _ = self.app.emit_all("speedtest_progress", SpeedtestProgressPayload { phase: "done".into(), down_mbps: down_mbps, up_mbps: up_mbps, elapsed_s: (dl_secs+ul_secs) });
        Ok(())
    }
}


