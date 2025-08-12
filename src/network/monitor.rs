use crate::core::error::{Result, SpeedKarmaError};
use crate::data::models::{SpeedMeasurement, ISPProfile, ThrottlingPattern};
use crate::data::repository::Repository;
use chrono::{DateTime, Utc, Duration, Weekday, Timelike, Datelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, info, warn, error};

/// Network interface statistics for bandwidth calculation
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub packets_received: u64,
    pub packets_sent: u64,
    pub timestamp: Instant,
}

impl Default for NetworkStats {
    fn default() -> Self {
        Self {
            bytes_received: 0,
            bytes_sent: 0,
            packets_received: 0,
            packets_sent: 0,
            timestamp: Instant::now(),
        }
    }
}

/// Passive speed measurement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassiveSpeedResult {
    pub timestamp: DateTime<Utc>,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub confidence: f64,
    pub measurement_duration_seconds: f64,
}

/// Configuration for passive monitoring
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub measurement_interval_seconds: u64,
    pub measurement_window_seconds: u64,
    pub min_confidence_threshold: f64,
    pub max_measurements_per_hour: u32,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            measurement_interval_seconds: 60, // Measure every minute
            measurement_window_seconds: 30,   // 30-second measurement windows
            min_confidence_threshold: 0.3,    // Minimum confidence to store measurement
            max_measurements_per_hour: 60,    // Rate limiting
        }
    }
}

/// Background network monitoring service
/// Operates like macOS system processes - always running, never intrusive
pub struct BackgroundMonitor {
    config: MonitoringConfig,
    repository: Arc<Repository>,
    is_running: Arc<RwLock<bool>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    network_interfaces: Arc<RwLock<HashMap<String, NetworkStats>>>,
    measurement_count: Arc<RwLock<u32>>,
    last_hour_reset: Arc<RwLock<DateTime<Utc>>>,
}

impl BackgroundMonitor {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self {
            config: MonitoringConfig::default(),
            repository,
            is_running: Arc::new(RwLock::new(false)),
            shutdown_tx: None,
            network_interfaces: Arc::new(RwLock::new(HashMap::new())),
            measurement_count: Arc::new(RwLock::new(0)),
            last_hour_reset: Arc::new(RwLock::new(Utc::now())),
        }
    }

    pub fn with_config(repository: Arc<Repository>, config: MonitoringConfig) -> Self {
        Self {
            config,
            repository,
            is_running: Arc::new(RwLock::new(false)),
            shutdown_tx: None,
            network_interfaces: Arc::new(RwLock::new(HashMap::new())),
            measurement_count: Arc::new(RwLock::new(0)),
            last_hour_reset: Arc::new(RwLock::new(Utc::now())),
        }
    }
    
    /// Starts passive speed monitoring without running speed tests
    pub async fn start_monitoring(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            debug!("Background monitoring is already running");
            return Ok(());
        }

        info!("Starting passive speed monitoring");
        *is_running = true;
        drop(is_running);

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Clone necessary data for the monitoring task
        let config = self.config.clone();
        let repository = Arc::clone(&self.repository);
        let is_running_clone = Arc::clone(&self.is_running);
        let network_interfaces = Arc::clone(&self.network_interfaces);
        let measurement_count = Arc::clone(&self.measurement_count);
        let last_hour_reset = Arc::clone(&self.last_hour_reset);

        // Spawn the monitoring task
        tokio::spawn(async move {
            let mut interval = interval(StdDuration::from_secs(config.measurement_interval_seconds));
            
            // Initialize network interface baseline
            if let Err(e) = Self::initialize_network_interfaces(&network_interfaces).await {
                error!("Failed to initialize network interfaces: {}", e);
                return;
            }

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check if we should continue running
                        if !*is_running_clone.read().await {
                            break;
                        }

                        // Reset hourly measurement count if needed
                        Self::reset_hourly_count_if_needed(&measurement_count, &last_hour_reset).await;

                        // Check rate limiting
                        if *measurement_count.read().await >= config.max_measurements_per_hour {
                            debug!("Rate limit reached, skipping measurement");
                            continue;
                        }

                        // Perform passive speed measurement
                        match Self::perform_passive_measurement(&config, &network_interfaces).await {
                            Ok(Some(result)) => {
                                // Store the measurement if confidence is sufficient
                                if result.confidence >= config.min_confidence_threshold {
                                    let measurement = SpeedMeasurement {
                                        id: None,
                                        timestamp: result.timestamp,
                                        download_mbps: result.download_mbps,
                                        upload_mbps: result.upload_mbps,
                                        latency_ms: 0, // Passive monitoring doesn't measure latency
                                        optimization_active: false, // This is baseline monitoring
                                        confidence: result.confidence,
                                    };

                                    if let Err(e) = repository.save_speed_measurement(&measurement).await {
                                        warn!("Failed to save speed measurement: {}", e);
                                    } else {
                                        debug!("Saved passive measurement: {:.2} Mbps down, {:.2} Mbps up (confidence: {:.2})", 
                                               result.download_mbps, result.upload_mbps, result.confidence);
                                        
                                        // Increment measurement count
                                        *measurement_count.write().await += 1;
                                    }
                                } else {
                                    debug!("Measurement confidence too low ({:.2}), skipping", result.confidence);
                                }
                            }
                            Ok(None) => {
                                debug!("No valid measurement available");
                            }
                            Err(e) => {
                                warn!("Passive measurement failed: {}", e);
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Received shutdown signal for background monitoring");
                        break;
                    }
                }
            }

            info!("Background monitoring stopped");
            *is_running_clone.write().await = false;
        });

        Ok(())
    }
    
    /// Stops all monitoring activities
    pub async fn stop_monitoring(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            debug!("Background monitoring is not running");
            return Ok(());
        }

        info!("Stopping passive speed monitoring");
        *is_running = false;
        drop(is_running);

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        Ok(())
    }

    /// Check if monitoring is currently active
    pub async fn is_monitoring(&self) -> bool {
        *self.is_running.read().await
    }

    /// Get current monitoring statistics
    pub async fn get_monitoring_stats(&self) -> MonitoringStats {
        MonitoringStats {
            is_running: *self.is_running.read().await,
            measurements_this_hour: *self.measurement_count.read().await,
            max_measurements_per_hour: self.config.max_measurements_per_hour,
            measurement_interval_seconds: self.config.measurement_interval_seconds,
        }
    }

    /// Initialize network interface monitoring
    async fn initialize_network_interfaces(
        network_interfaces: &Arc<RwLock<HashMap<String, NetworkStats>>>
    ) -> Result<()> {
        let interfaces = Self::get_network_interface_stats().await?;
        let mut interfaces_map = network_interfaces.write().await;
        *interfaces_map = interfaces;
        debug!("Initialized {} network interfaces for monitoring", interfaces_map.len());
        Ok(())
    }

    /// Reset hourly measurement count if an hour has passed
    async fn reset_hourly_count_if_needed(
        measurement_count: &Arc<RwLock<u32>>,
        last_hour_reset: &Arc<RwLock<DateTime<Utc>>>
    ) {
        let now = Utc::now();
        let mut last_reset = last_hour_reset.write().await;
        
        if now.signed_duration_since(*last_reset) >= Duration::hours(1) {
            *measurement_count.write().await = 0;
            *last_reset = now;
            debug!("Reset hourly measurement count");
        }
    }

    /// Perform a passive speed measurement by analyzing network interface statistics
    async fn perform_passive_measurement(
        _config: &MonitoringConfig,
        network_interfaces: &Arc<RwLock<HashMap<String, NetworkStats>>>
    ) -> Result<Option<PassiveSpeedResult>> {
        let measurement_start = Instant::now();
        
        // Get current network stats
        let current_stats = Self::get_network_interface_stats().await?;
        
        // Calculate bandwidth usage over the measurement window
        let mut interfaces_guard = network_interfaces.write().await;
        let mut total_download_bytes = 0u64;
        let mut total_upload_bytes = 0u64;
        let mut valid_measurements = 0;
        let mut total_time_diff = 0.0;

        for (interface_name, current_stat) in &current_stats {
            if let Some(previous_stat) = interfaces_guard.get(interface_name) {
                let time_diff = measurement_start.duration_since(previous_stat.timestamp).as_secs_f64();
                
                // Only consider measurements with reasonable time differences
                if time_diff >= 10.0 && time_diff <= 300.0 { // Between 10 seconds and 5 minutes
                    let bytes_received_diff = current_stat.bytes_received.saturating_sub(previous_stat.bytes_received);
                    let bytes_sent_diff = current_stat.bytes_sent.saturating_sub(previous_stat.bytes_sent);
                    
                    // Filter out unrealistic spikes (more than 1 Gbps)
                    let max_bytes_per_second = 125_000_000; // 1 Gbps in bytes
                    let max_bytes_in_window = (max_bytes_per_second as f64 * time_diff) as u64;
                    
                    if bytes_received_diff <= max_bytes_in_window && bytes_sent_diff <= max_bytes_in_window {
                        total_download_bytes += bytes_received_diff;
                        total_upload_bytes += bytes_sent_diff;
                        total_time_diff += time_diff;
                        valid_measurements += 1;
                    }
                }
            }
        }

        // Update stored stats for next measurement
        *interfaces_guard = current_stats;
        drop(interfaces_guard);

        // Calculate speeds if we have valid measurements
        if valid_measurements > 0 && total_time_diff > 0.0 {
            let avg_time_diff = total_time_diff / valid_measurements as f64;
            
            // Convert bytes to megabits per second
            let download_mbps = (total_download_bytes as f64 * 8.0) / (avg_time_diff * 1_000_000.0);
            let upload_mbps = (total_upload_bytes as f64 * 8.0) / (avg_time_diff * 1_000_000.0);
            
            // Calculate confidence based on measurement quality
            let confidence = Self::calculate_measurement_confidence(
                valid_measurements,
                avg_time_diff,
                download_mbps,
                upload_mbps
            );

            Ok(Some(PassiveSpeedResult {
                timestamp: Utc::now(),
                download_mbps,
                upload_mbps,
                confidence,
                measurement_duration_seconds: avg_time_diff,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get network interface statistics using system APIs
    async fn get_network_interface_stats() -> Result<HashMap<String, NetworkStats>> {
        use sysinfo::{System, Networks};
        
        let _system = System::new();
        let mut networks = Networks::new_with_refreshed_list();
        networks.refresh();
        
        let mut stats = HashMap::new();
        let timestamp = Instant::now();
        
        for (interface_name, network) in &networks {
            // Skip loopback and inactive interfaces
            if interface_name.contains("lo") || interface_name.contains("loopback") {
                continue;
            }
            
            let network_stat = NetworkStats {
                bytes_received: network.total_received(),
                bytes_sent: network.total_transmitted(),
                packets_received: network.total_packets_received(),
                packets_sent: network.total_packets_transmitted(),
                timestamp,
            };
            
            stats.insert(interface_name.clone(), network_stat);
        }
        
        if stats.is_empty() {
            return Err(SpeedKarmaError::SystemError(
                "No active network interfaces found".to_string()
            ));
        }
        
        Ok(stats)
    }

    /// Calculate confidence score for a passive measurement
    fn calculate_measurement_confidence(
        valid_measurements: usize,
        avg_time_diff: f64,
        download_mbps: f64,
        upload_mbps: f64
    ) -> f64 {
        let mut confidence = 0.0;
        
        // Base confidence from number of interfaces measured
        confidence += (valid_measurements as f64 / 5.0).min(0.3);
        
        // Time window quality (prefer 30-120 second windows)
        let time_quality = if avg_time_diff >= 30.0 && avg_time_diff <= 120.0 {
            0.3
        } else if avg_time_diff >= 15.0 && avg_time_diff <= 300.0 {
            0.2
        } else {
            0.1
        };
        confidence += time_quality;
        
        // Speed reasonableness (prefer speeds between 1-1000 Mbps)
        let speed_quality = if download_mbps >= 1.0 && download_mbps <= 1000.0 && upload_mbps >= 0.1 && upload_mbps <= 1000.0 {
            0.3
        } else if download_mbps >= 0.1 && download_mbps <= 2000.0 {
            0.2
        } else {
            0.1
        };
        confidence += speed_quality;
        
        // Ratio reasonableness (download usually higher than upload)
        let ratio_quality = if download_mbps > 0.0 && upload_mbps > 0.0 {
            let ratio = download_mbps / upload_mbps;
            if ratio >= 1.0 && ratio <= 50.0 {
                0.1
            } else {
                0.05
            }
        } else {
            0.0
        };
        confidence += ratio_quality;
        
        confidence.min(1.0)
    }

    /// Detect ISP using various methods
    pub async fn detect_isp(&self) -> Result<ISPDetectionResult> {
        info!("Starting ISP detection");
        
        // Try multiple detection methods and combine results
        let mut detection_results = Vec::new();
        
        // Method 1: DNS Analysis
        if let Ok(result) = self.detect_isp_via_dns().await {
            detection_results.push(result);
        }
        
        // Method 2: Public IP lookup (simplified for demo)
        if let Ok(result) = self.detect_isp_via_public_ip().await {
            detection_results.push(result);
        }
        
        // Method 3: Network routing analysis (simplified)
        if let Ok(result) = self.detect_isp_via_routing().await {
            detection_results.push(result);
        }
        
        // Combine results and select the most confident one
        let best_result = detection_results
            .into_iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or_else(|| ISPDetectionResult {
                isp_name: "Unknown ISP".to_string(),
                region: "Unknown".to_string(),
                detection_method: ISPDetectionMethod::Combined.as_str().to_string(),
                confidence: 0.1,
                detected_at: Utc::now(),
            });
        
        info!("ISP detected: {} (confidence: {:.2})", best_result.isp_name, best_result.confidence);
        Ok(best_result)
    }

    /// Analyze speed patterns to detect throttling
    pub async fn analyze_throttling_patterns(&self, days: u32) -> Result<PatternAnalysisResult> {
        info!("Analyzing throttling patterns over {} days", days);
        
        let since = if days == 0 {
            Utc::now() - Duration::hours(1) // Look back 1 hour for current day
        } else {
            Utc::now() - Duration::days(days as i64)
        };
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        
        if measurements.len() < 10 {
            return Ok(PatternAnalysisResult {
                throttling_detected: false,
                patterns: Vec::new(),
                confidence: 0.0,
                analysis_period_days: days,
                baseline_speed_mbps: 0.0,
                throttled_speed_mbps: 0.0,
                improvement_potential: 0.0,
            });
        }
        
        // Group measurements by hour and day of week
        let mut hourly_speeds: HashMap<(Weekday, u8), Vec<f64>> = HashMap::new();
        let mut all_speeds = Vec::new();
        
        for measurement in &measurements {
            let weekday = measurement.timestamp.weekday();
            let hour = measurement.timestamp.hour() as u8;
            
            hourly_speeds
                .entry((weekday, hour))
                .or_insert_with(Vec::new)
                .push(measurement.download_mbps);
            
            all_speeds.push(measurement.download_mbps);
        }
        
        // Calculate baseline speed (median of all measurements)
        all_speeds.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let baseline_speed = if all_speeds.is_empty() {
            0.0
        } else {
            all_speeds[all_speeds.len() / 2]
        };
        
        // Detect throttling patterns
        let patterns = self.detect_throttling_patterns(&hourly_speeds, baseline_speed).await?;
        
        // Calculate overall throttling metrics
        let throttling_detected = !patterns.is_empty();
        let confidence = if throttling_detected {
            patterns.iter().map(|p| p.confidence).sum::<f64>() / patterns.len() as f64
        } else {
            0.0
        };
        
        // Calculate throttled speed (average of speeds during detected throttling periods)
        let throttled_speed = if throttling_detected {
            let mut throttled_speeds = Vec::new();
            for pattern in &patterns {
                for ((weekday, hour), speeds) in &hourly_speeds {
                    if pattern.days_of_week.contains(weekday) && 
                       Self::is_hour_in_pattern(*hour, pattern.start_hour, pattern.end_hour) {
                        throttled_speeds.extend(speeds);
                    }
                }
            }
            if !throttled_speeds.is_empty() {
                throttled_speeds.iter().sum::<f64>() / throttled_speeds.len() as f64
            } else {
                baseline_speed
            }
        } else {
            baseline_speed
        };
        
        let improvement_potential = if throttled_speed > 0.0 {
            baseline_speed / throttled_speed
        } else {
            1.0
        };
        
        info!("Throttling analysis complete: {} patterns detected (confidence: {:.2})", 
              patterns.len(), confidence);
        
        Ok(PatternAnalysisResult {
            throttling_detected,
            patterns,
            confidence,
            analysis_period_days: days,
            baseline_speed_mbps: baseline_speed,
            throttled_speed_mbps: throttled_speed,
            improvement_potential,
        })
    }

    /// Save detected ISP profile to database
    pub async fn save_isp_profile(&self, detection_result: &ISPDetectionResult) -> Result<i64> {
        let profile = ISPProfile::new(
            detection_result.isp_name.clone(),
            detection_result.region.clone(),
            detection_result.detection_method.clone(),
        );
        
        self.repository.save_isp_profile(&profile).await
    }

    /// Save detected throttling patterns to database
    pub async fn save_throttling_patterns(&self, isp_profile_id: i64, patterns: &[DetectedThrottlingPattern]) -> Result<Vec<i64>> {
        let mut pattern_ids = Vec::new();
        
        for pattern in patterns {
            let throttling_pattern = ThrottlingPattern::new(
                isp_profile_id,
                pattern.start_hour,
                pattern.start_minute,
                pattern.end_hour,
                pattern.end_minute,
                pattern.days_of_week.clone(),
                pattern.severity,
            );
            
            let id = self.repository.save_throttling_pattern(&throttling_pattern).await?;
            pattern_ids.push(id);
        }
        
        Ok(pattern_ids)
    }

    /// Detect ISP via DNS analysis
    async fn detect_isp_via_dns(&self) -> Result<ISPDetectionResult> {
        // Simplified DNS-based ISP detection
        // In a real implementation, this would analyze DNS servers and routing
        
        // For demo purposes, we'll simulate detection based on common patterns
        let known_isps = vec![
            ("Hutch", "Sri Lanka", 0.8),
            ("Dialog", "Sri Lanka", 0.7),
            ("Mobitel", "Sri Lanka", 0.6),
            ("SLT", "Sri Lanka", 0.7),
            ("Airtel", "Sri Lanka", 0.6),
        ];
        
        // Simulate DNS lookup delay
        tokio::time::sleep(StdDuration::from_millis(100)).await;
        
        // For demo, randomly select an ISP (in real implementation, this would be actual DNS analysis)
        let (isp_name, region, confidence) = known_isps[0]; // Default to Hutch for demo
        
        Ok(ISPDetectionResult {
            isp_name: isp_name.to_string(),
            region: region.to_string(),
            detection_method: ISPDetectionMethod::DnsAnalysis.as_str().to_string(),
            confidence,
            detected_at: Utc::now(),
        })
    }

    /// Detect ISP via public IP lookup
    async fn detect_isp_via_public_ip(&self) -> Result<ISPDetectionResult> {
        // Simplified public IP-based ISP detection
        // In a real implementation, this would query IP geolocation services
        
        tokio::time::sleep(StdDuration::from_millis(200)).await;
        
        // For demo purposes, simulate a detection result
        Ok(ISPDetectionResult {
            isp_name: "Hutch".to_string(),
            region: "Sri Lanka".to_string(),
            detection_method: ISPDetectionMethod::PublicIPLookup.as_str().to_string(),
            confidence: 0.9,
            detected_at: Utc::now(),
        })
    }

    /// Detect ISP via network routing analysis
    async fn detect_isp_via_routing(&self) -> Result<ISPDetectionResult> {
        // Simplified routing-based ISP detection
        // In a real implementation, this would analyze traceroute data
        
        tokio::time::sleep(StdDuration::from_millis(150)).await;
        
        Ok(ISPDetectionResult {
            isp_name: "Hutch".to_string(),
            region: "Sri Lanka".to_string(),
            detection_method: ISPDetectionMethod::NetworkRouting.as_str().to_string(),
            confidence: 0.7,
            detected_at: Utc::now(),
        })
    }

    /// Detect throttling patterns from hourly speed data
    async fn detect_throttling_patterns(
        &self,
        hourly_speeds: &HashMap<(Weekday, u8), Vec<f64>>,
        baseline_speed: f64,
    ) -> Result<Vec<DetectedThrottlingPattern>> {
        let mut patterns = Vec::new();
        let throttling_threshold = baseline_speed * 0.7; // 30% reduction indicates throttling
        
        // Group consecutive hours with similar throttling behavior
        let mut current_pattern: Option<DetectedThrottlingPattern> = None;
        
        // Analyze each day of the week
        for weekday in [Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri, Weekday::Sat, Weekday::Sun] {
            for hour in 0..24 {
                if let Some(speeds) = hourly_speeds.get(&(weekday, hour)) {
                    if speeds.len() < 3 {
                        continue; // Need at least 3 measurements for confidence
                    }
                    
                    let avg_speed = speeds.iter().sum::<f64>() / speeds.len() as f64;
                    let is_throttled = avg_speed < throttling_threshold;
                    
                    if is_throttled {
                        let severity = 1.0 - (avg_speed / baseline_speed);
                        let confidence = Self::calculate_pattern_confidence(speeds.len(), severity);
                        
                        match &mut current_pattern {
                            Some(pattern) if pattern.end_hour == hour.saturating_sub(1) && 
                                           pattern.days_of_week.contains(&weekday) => {
                                // Extend current pattern
                                pattern.end_hour = hour;
                                pattern.severity = (pattern.severity + severity) / 2.0;
                                pattern.confidence = (pattern.confidence + confidence) / 2.0;
                                pattern.sample_count += speeds.len() as u32;
                            }
                            _ => {
                                // Start new pattern or save current one
                                if let Some(pattern) = current_pattern.take() {
                                    if pattern.confidence > 0.5 {
                                        patterns.push(pattern);
                                    }
                                }
                                
                                current_pattern = Some(DetectedThrottlingPattern {
                                    start_hour: hour,
                                    start_minute: 0,
                                    end_hour: hour,
                                    end_minute: 59,
                                    days_of_week: vec![weekday],
                                    severity,
                                    confidence,
                                    sample_count: speeds.len() as u32,
                                });
                            }
                        }
                    } else if let Some(pattern) = current_pattern.take() {
                        // End current pattern
                        if pattern.confidence > 0.5 {
                            patterns.push(pattern);
                        }
                    }
                }
            }
        }
        
        // Don't forget the last pattern
        if let Some(pattern) = current_pattern {
            if pattern.confidence > 0.5 {
                patterns.push(pattern);
            }
        }
        
        // Merge similar patterns across days
        patterns = Self::merge_similar_patterns(patterns);
        
        Ok(patterns)
    }

    /// Calculate confidence for a throttling pattern
    fn calculate_pattern_confidence(sample_count: usize, severity: f64) -> f64 {
        let sample_confidence = (sample_count as f64 / 10.0).min(1.0); // More samples = higher confidence
        let severity_confidence = severity.min(1.0); // Higher severity = higher confidence
        
        (sample_confidence * 0.6 + severity_confidence * 0.4).min(1.0)
    }

    /// Check if an hour falls within a pattern's time range
    fn is_hour_in_pattern(hour: u8, start_hour: u8, end_hour: u8) -> bool {
        if start_hour <= end_hour {
            hour >= start_hour && hour <= end_hour
        } else {
            // Pattern crosses midnight
            hour >= start_hour || hour <= end_hour
        }
    }

    /// Merge similar throttling patterns across different days
    fn merge_similar_patterns(patterns: Vec<DetectedThrottlingPattern>) -> Vec<DetectedThrottlingPattern> {
        let mut merged: Vec<DetectedThrottlingPattern> = Vec::new();
        
        for pattern in patterns {
            let mut found_similar = false;
            
            for existing in &mut merged {
                // Check if patterns have similar time ranges (within 1 hour)
                if (pattern.start_hour as i16 - existing.start_hour as i16).abs() <= 1 &&
                   (pattern.end_hour as i16 - existing.end_hour as i16).abs() <= 1 &&
                   (pattern.severity - existing.severity).abs() < 0.3 {
                    
                    // Merge the patterns
                    existing.days_of_week.extend(pattern.days_of_week.clone());
                    existing.days_of_week.sort_by_key(|w| w.number_from_monday());
                    existing.days_of_week.dedup();
                    
                    existing.severity = (existing.severity + pattern.severity) / 2.0;
                    existing.confidence = (existing.confidence + pattern.confidence) / 2.0;
                    existing.sample_count += pattern.sample_count;
                    
                    found_similar = true;
                    break;
                }
            }
            
            if !found_similar {
                merged.push(pattern);
            }
        }
        
        merged
    }
}

/// Monitoring statistics for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringStats {
    pub is_running: bool,
    pub measurements_this_hour: u32,
    pub max_measurements_per_hour: u32,
    pub measurement_interval_seconds: u64,
}

/// ISP detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISPDetectionResult {
    pub isp_name: String,
    pub region: String,
    pub detection_method: String,
    pub confidence: f64,
    pub detected_at: DateTime<Utc>,
}

/// Pattern analysis result for throttling detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysisResult {
    pub throttling_detected: bool,
    pub patterns: Vec<DetectedThrottlingPattern>,
    pub confidence: f64,
    pub analysis_period_days: u32,
    pub baseline_speed_mbps: f64,
    pub throttled_speed_mbps: f64,
    pub improvement_potential: f64,
}

/// A detected throttling pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedThrottlingPattern {
    pub start_hour: u8,
    pub start_minute: u8,
    pub end_hour: u8,
    pub end_minute: u8,
    pub days_of_week: Vec<Weekday>,
    pub severity: f64,
    pub confidence: f64,
    pub sample_count: u32,
}

/// ISP detection methods
#[derive(Debug, Clone)]
pub enum ISPDetectionMethod {
    DnsAnalysis,
    NetworkRouting,
    PublicIPLookup,
    UserAgent,
    Combined,
}

impl ISPDetectionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            ISPDetectionMethod::DnsAnalysis => "DNS Analysis",
            ISPDetectionMethod::NetworkRouting => "Network Routing",
            ISPDetectionMethod::PublicIPLookup => "Public IP Lookup",
            ISPDetectionMethod::UserAgent => "User Agent",
            ISPDetectionMethod::Combined => "Combined Methods",
        }
    }
}
#[cfg(
test)]
mod tests {
    use super::*;
    use crate::data::migrations::MigrationManager;
    use sqlx::SqlitePool;
    use std::time::Duration as StdDuration;
    use tokio::time::sleep;

    async fn setup_test_repository() -> Arc<Repository> {
        let database_url = ":memory:";
        let pool = SqlitePool::connect(database_url).await.unwrap();
        
        let migration_manager = MigrationManager::new(database_url.to_string());
        migration_manager.run_migrations(&pool).await.unwrap();
        
        Arc::new(Repository::new(pool))
    }

    #[tokio::test]
    async fn test_background_monitor_creation() {
        let repository = setup_test_repository().await;
        let monitor = BackgroundMonitor::new(repository);
        
        assert!(!monitor.is_monitoring().await);
        
        let stats = monitor.get_monitoring_stats().await;
        assert!(!stats.is_running);
        assert_eq!(stats.measurements_this_hour, 0);
        assert_eq!(stats.max_measurements_per_hour, 60);
    }

    #[tokio::test]
    async fn test_background_monitor_with_custom_config() {
        let repository = setup_test_repository().await;
        let config = MonitoringConfig {
            measurement_interval_seconds: 30,
            measurement_window_seconds: 15,
            min_confidence_threshold: 0.5,
            max_measurements_per_hour: 120,
        };
        
        let monitor = BackgroundMonitor::with_config(repository, config);
        let stats = monitor.get_monitoring_stats().await;
        
        assert_eq!(stats.measurement_interval_seconds, 30);
        assert_eq!(stats.max_measurements_per_hour, 120);
    }

    #[tokio::test]
    async fn test_start_stop_monitoring() {
        let repository = setup_test_repository().await;
        let mut monitor = BackgroundMonitor::new(repository);
        
        // Start monitoring
        monitor.start_monitoring().await.unwrap();
        assert!(monitor.is_monitoring().await);
        
        // Try to start again (should not fail)
        monitor.start_monitoring().await.unwrap();
        assert!(monitor.is_monitoring().await);
        
        // Stop monitoring
        monitor.stop_monitoring().await.unwrap();
        
        // Give it a moment to stop
        sleep(StdDuration::from_millis(100)).await;
        assert!(!monitor.is_monitoring().await);
        
        // Try to stop again (should not fail)
        monitor.stop_monitoring().await.unwrap();
    }

    #[tokio::test]
    async fn test_network_interface_stats() {
        // This test may fail in CI environments without network interfaces
        match BackgroundMonitor::get_network_interface_stats().await {
            Ok(stats) => {
                // Should have at least one interface in most environments
                assert!(!stats.is_empty(), "Should find at least one network interface");
                
                for (name, stat) in &stats {
                    assert!(!name.is_empty(), "Interface name should not be empty");
                    // Basic sanity checks - stats should be reasonable
                    assert!(stat.bytes_received < u64::MAX / 2, "Received bytes should be reasonable");
                    assert!(stat.bytes_sent < u64::MAX / 2, "Sent bytes should be reasonable");
                }
            }
            Err(_) => {
                // In some test environments, network interfaces might not be available
                // This is acceptable for unit tests
                println!("Network interfaces not available in test environment");
            }
        }
    }

    #[test]
    fn test_measurement_confidence_calculation() {
        // Test high confidence scenario
        let confidence = BackgroundMonitor::calculate_measurement_confidence(
            3,      // 3 valid measurements
            60.0,   // 60 second window (ideal)
            50.0,   // 50 Mbps download (reasonable)
            10.0,   // 10 Mbps upload (reasonable)
        );
        assert!(confidence > 0.8, "Should have high confidence for ideal measurement");
        
        // Test medium confidence scenario
        let confidence = BackgroundMonitor::calculate_measurement_confidence(
            1,      // 1 valid measurement
            30.0,   // 30 second window
            100.0,  // 100 Mbps download
            20.0,   // 20 Mbps upload
        );
        assert!(confidence > 0.7, "Should have high confidence for good measurement, got: {}", confidence);
        
        // Test medium confidence scenario (short window)
        let confidence = BackgroundMonitor::calculate_measurement_confidence(
            1,      // 1 valid measurement
            15.0,   // Short but acceptable window
            50.0,   // Reasonable speed
            10.0,   // Reasonable upload
        );
        assert!(confidence > 0.4 && confidence < 0.8, "Should have medium confidence, got: {}", confidence);
        
        // Test low confidence scenario
        let confidence = BackgroundMonitor::calculate_measurement_confidence(
            0,      // No valid measurements
            5.0,    // Very short window
            2000.0, // Unrealistic speed
            1000.0, // Unrealistic upload
        );
        assert!(confidence <= 0.4, "Should have low confidence for poor measurement, got: {}", confidence);
    }

    #[test]
    fn test_passive_speed_result_serialization() {
        let result = PassiveSpeedResult {
            timestamp: Utc::now(),
            download_mbps: 50.5,
            upload_mbps: 10.2,
            confidence: 0.85,
            measurement_duration_seconds: 60.0,
        };
        
        // Test serialization
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("50.5"));
        assert!(json.contains("10.2"));
        assert!(json.contains("0.85"));
        
        // Test deserialization
        let deserialized: PassiveSpeedResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.download_mbps, 50.5);
        assert_eq!(deserialized.upload_mbps, 10.2);
        assert_eq!(deserialized.confidence, 0.85);
    }

    #[test]
    fn test_monitoring_config_defaults() {
        let config = MonitoringConfig::default();
        
        assert_eq!(config.measurement_interval_seconds, 60);
        assert_eq!(config.measurement_window_seconds, 30);
        assert_eq!(config.min_confidence_threshold, 0.3);
        assert_eq!(config.max_measurements_per_hour, 60);
    }

    #[test]
    fn test_network_stats_default() {
        let stats = NetworkStats::default();
        
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.packets_received, 0);
        assert_eq!(stats.packets_sent, 0);
    }

    #[tokio::test]
    async fn test_monitoring_with_rate_limiting() {
        let repository = setup_test_repository().await;
        let config = MonitoringConfig {
            measurement_interval_seconds: 1, // Very fast for testing
            measurement_window_seconds: 1,
            min_confidence_threshold: 0.0, // Accept all measurements
            max_measurements_per_hour: 2,  // Very low limit for testing
        };
        
        let mut monitor = BackgroundMonitor::with_config(repository, config);
        
        // Start monitoring
        monitor.start_monitoring().await.unwrap();
        
        // Wait a bit for some measurements
        sleep(StdDuration::from_secs(3)).await;
        
        let stats = monitor.get_monitoring_stats().await;
        
        // Should respect rate limiting
        assert!(stats.measurements_this_hour <= stats.max_measurements_per_hour);
        
        monitor.stop_monitoring().await.unwrap();
    }

    #[tokio::test]
    async fn test_monitoring_stats_serialization() {
        let stats = MonitoringStats {
            is_running: true,
            measurements_this_hour: 15,
            max_measurements_per_hour: 60,
            measurement_interval_seconds: 60,
        };
        
        // Test serialization
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("15"));
        assert!(json.contains("60"));
        
        // Test deserialization
        let deserialized: MonitoringStats = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.is_running, true);
        assert_eq!(deserialized.measurements_this_hour, 15);
        assert_eq!(deserialized.max_measurements_per_hour, 60);
    }

    #[tokio::test]
    async fn test_isp_detection() {
        let repository = setup_test_repository().await;
        let monitor = BackgroundMonitor::new(repository);
        
        let result = monitor.detect_isp().await.unwrap();
        
        assert!(!result.isp_name.is_empty());
        assert!(!result.region.is_empty());
        assert!(!result.detection_method.is_empty());
        assert!(result.confidence > 0.0);
        assert!(result.confidence <= 1.0);
    }

    #[tokio::test]
    async fn test_isp_detection_methods() {
        let repository = setup_test_repository().await;
        let monitor = BackgroundMonitor::new(repository);
        
        // Test DNS detection
        let dns_result = monitor.detect_isp_via_dns().await.unwrap();
        assert_eq!(dns_result.detection_method, "DNS Analysis");
        assert!(dns_result.confidence > 0.0);
        
        // Test public IP detection
        let ip_result = monitor.detect_isp_via_public_ip().await.unwrap();
        assert_eq!(ip_result.detection_method, "Public IP Lookup");
        assert!(ip_result.confidence > 0.0);
        
        // Test routing detection
        let routing_result = monitor.detect_isp_via_routing().await.unwrap();
        assert_eq!(routing_result.detection_method, "Network Routing");
        assert!(routing_result.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_save_isp_profile() {
        let repository = setup_test_repository().await;
        let monitor = BackgroundMonitor::new(repository.clone());
        
        let detection_result = ISPDetectionResult {
            isp_name: "Test ISP".to_string(),
            region: "Test Region".to_string(),
            detection_method: "Test Method".to_string(),
            confidence: 0.8,
            detected_at: Utc::now(),
        };
        
        let profile_id = monitor.save_isp_profile(&detection_result).await.unwrap();
        assert!(profile_id > 0);
        
        // Verify the profile was saved
        let saved_profile = repository.get_current_isp_profile().await.unwrap();
        assert!(saved_profile.is_some());
        assert_eq!(saved_profile.unwrap().name, "Test ISP");
    }

    #[tokio::test]
    async fn test_throttling_pattern_analysis_insufficient_data() {
        let repository = setup_test_repository().await;
        let monitor = BackgroundMonitor::new(repository);
        
        // Test with no measurements
        let result = monitor.analyze_throttling_patterns(7).await.unwrap();
        
        assert!(!result.throttling_detected);
        assert_eq!(result.patterns.len(), 0);
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.baseline_speed_mbps, 0.0);
    }

    #[tokio::test]
    async fn test_throttling_pattern_analysis_basic() {
        let repository = setup_test_repository().await;
        let monitor = BackgroundMonitor::new(repository);
        
        // Test with no data - should return empty result without error
        let result = monitor.analyze_throttling_patterns(7).await.unwrap();
        assert!(!result.throttling_detected);
        assert_eq!(result.patterns.len(), 0);
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.analysis_period_days, 7);
        assert_eq!(result.baseline_speed_mbps, 0.0);
        assert_eq!(result.throttled_speed_mbps, 0.0);
        assert_eq!(result.improvement_potential, 0.0);
    }

    #[test]
    fn test_pattern_confidence_calculation() {
        // Test high confidence (many samples, high severity)
        let confidence = BackgroundMonitor::calculate_pattern_confidence(20, 0.8);
        assert!(confidence > 0.8);
        
        // Test medium confidence (few samples, medium severity)
        let confidence = BackgroundMonitor::calculate_pattern_confidence(5, 0.5);
        assert!(confidence > 0.4 && confidence < 0.8);
        
        // Test low confidence (few samples, low severity)
        let confidence = BackgroundMonitor::calculate_pattern_confidence(2, 0.2);
        assert!(confidence < 0.5);
    }

    #[test]
    fn test_hour_in_pattern() {
        // Normal pattern (19:00 - 22:00)
        assert!(BackgroundMonitor::is_hour_in_pattern(19, 19, 22));
        assert!(BackgroundMonitor::is_hour_in_pattern(20, 19, 22));
        assert!(BackgroundMonitor::is_hour_in_pattern(22, 19, 22));
        assert!(!BackgroundMonitor::is_hour_in_pattern(18, 19, 22));
        assert!(!BackgroundMonitor::is_hour_in_pattern(23, 19, 22));
        
        // Pattern crossing midnight (23:00 - 02:00)
        assert!(BackgroundMonitor::is_hour_in_pattern(23, 23, 2));
        assert!(BackgroundMonitor::is_hour_in_pattern(0, 23, 2));
        assert!(BackgroundMonitor::is_hour_in_pattern(1, 23, 2));
        assert!(BackgroundMonitor::is_hour_in_pattern(2, 23, 2));
        assert!(!BackgroundMonitor::is_hour_in_pattern(3, 23, 2));
        assert!(!BackgroundMonitor::is_hour_in_pattern(22, 23, 2));
    }

    #[test]
    fn test_merge_similar_patterns() {
        let patterns = vec![
            DetectedThrottlingPattern {
                start_hour: 19,
                start_minute: 0,
                end_hour: 21,
                end_minute: 59,
                days_of_week: vec![Weekday::Mon],
                severity: 0.7,
                confidence: 0.8,
                sample_count: 10,
            },
            DetectedThrottlingPattern {
                start_hour: 19,
                start_minute: 0,
                end_hour: 22,
                end_minute: 59,
                days_of_week: vec![Weekday::Tue],
                severity: 0.75,
                confidence: 0.85,
                sample_count: 12,
            },
        ];
        
        let merged = BackgroundMonitor::merge_similar_patterns(patterns);
        
        // Should merge into one pattern covering both days
        assert_eq!(merged.len(), 1);
        assert!(merged[0].days_of_week.contains(&Weekday::Mon));
        assert!(merged[0].days_of_week.contains(&Weekday::Tue));
        assert_eq!(merged[0].sample_count, 22);
    }

    #[test]
    fn test_isp_detection_result_serialization() {
        let result = ISPDetectionResult {
            isp_name: "Test ISP".to_string(),
            region: "Test Region".to_string(),
            detection_method: "Test Method".to_string(),
            confidence: 0.85,
            detected_at: Utc::now(),
        };
        
        // Test serialization
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Test ISP"));
        assert!(json.contains("0.85"));
        
        // Test deserialization
        let deserialized: ISPDetectionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.isp_name, "Test ISP");
        assert_eq!(deserialized.confidence, 0.85);
    }

    #[test]
    fn test_pattern_analysis_result_serialization() {
        let result = PatternAnalysisResult {
            throttling_detected: true,
            patterns: vec![],
            confidence: 0.75,
            analysis_period_days: 7,
            baseline_speed_mbps: 100.0,
            throttled_speed_mbps: 30.0,
            improvement_potential: 3.33,
        };
        
        // Test serialization
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("100.0"));
        assert!(json.contains("3.33"));
        
        // Test deserialization
        let deserialized: PatternAnalysisResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.throttling_detected, true);
        assert_eq!(deserialized.baseline_speed_mbps, 100.0);
        assert_eq!(deserialized.improvement_potential, 3.33);
    }

    #[test]
    fn test_isp_detection_method_as_str() {
        assert_eq!(ISPDetectionMethod::DnsAnalysis.as_str(), "DNS Analysis");
        assert_eq!(ISPDetectionMethod::NetworkRouting.as_str(), "Network Routing");
        assert_eq!(ISPDetectionMethod::PublicIPLookup.as_str(), "Public IP Lookup");
        assert_eq!(ISPDetectionMethod::UserAgent.as_str(), "User Agent");
        assert_eq!(ISPDetectionMethod::Combined.as_str(), "Combined Methods");
    }
}