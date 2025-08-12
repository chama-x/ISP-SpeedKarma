use crate::core::error::Result;
use crate::data::models::*;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};

/// Repository pattern implementation for database operations
pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Speed measurement operations
    pub async fn save_speed_measurement(&self, measurement: &SpeedMeasurement) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO speed_measurements (timestamp, download_mbps, upload_mbps, latency_ms, optimization_active, confidence)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&measurement.timestamp)
        .bind(measurement.download_mbps)
        .bind(measurement.upload_mbps)
        .bind(measurement.latency_ms)
        .bind(measurement.optimization_active)
        .bind(measurement.confidence)
        .execute(&self.pool)
        .await?;
        
        Ok(result.last_insert_rowid())
    }
    
    pub async fn get_speed_measurements_since(&self, since: DateTime<Utc>) -> Result<Vec<SpeedMeasurement>> {
        let rows = sqlx::query(
            r#"
            SELECT id, timestamp, download_mbps, upload_mbps, latency_ms, optimization_active, confidence
            FROM speed_measurements
            WHERE timestamp >= ?
            ORDER BY timestamp DESC
            "#
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        
        let measurements = rows.into_iter().map(|row| {
            SpeedMeasurement {
                id: row.get("id"),
                timestamp: row.get("timestamp"),
                download_mbps: row.get("download_mbps"),
                upload_mbps: row.get("upload_mbps"),
                latency_ms: row.get("latency_ms"),
                optimization_active: row.get("optimization_active"),
                confidence: row.get("confidence"),
            }
        }).collect();
        
        Ok(measurements)
    }
    
    /// ISP profile operations
    pub async fn save_isp_profile(&self, profile: &ISPProfile) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO isp_profiles (name, region, detection_method, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(&profile.name)
        .bind(&profile.region)
        .bind(&profile.detection_method)
        .bind(&profile.created_at)
        .bind(&profile.updated_at)
        .execute(&self.pool)
        .await?;
        
        Ok(result.last_insert_rowid())
    }
    
    pub async fn get_current_isp_profile(&self) -> Result<Option<ISPProfile>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, region, detection_method, created_at, updated_at
            FROM isp_profiles
            ORDER BY updated_at DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;
        
        let profile = row.map(|r| ISPProfile {
            id: r.get("id"),
            name: r.get("name"),
            region: r.get("region"),
            detection_method: r.get("detection_method"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        });
        
        Ok(profile)
    }
    
    /// Throttling pattern operations
    pub async fn save_throttling_pattern(&self, pattern: &ThrottlingPattern) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO throttling_patterns (isp_profile_id, start_hour, start_minute, end_hour, end_minute, days_of_week, severity, confidence)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(pattern.isp_profile_id)
        .bind(pattern.start_hour)
        .bind(pattern.start_minute)
        .bind(pattern.end_hour)
        .bind(pattern.end_minute)
        .bind(&pattern.days_of_week_json())
        .bind(pattern.severity)
        .bind(pattern.confidence)
        .execute(&self.pool)
        .await?;
        
        Ok(result.last_insert_rowid())
    }
    
    pub async fn get_throttling_patterns_for_isp(&self, isp_profile_id: i64) -> Result<Vec<ThrottlingPattern>> {
        let rows = sqlx::query(
            r#"
            SELECT id, isp_profile_id, start_hour, start_minute, end_hour, end_minute, days_of_week, severity, confidence
            FROM throttling_patterns
            WHERE isp_profile_id = ?
            ORDER BY confidence DESC
            "#
        )
        .bind(isp_profile_id)
        .fetch_all(&self.pool)
        .await?;
        
        let patterns = rows.into_iter().map(|row| {
            ThrottlingPattern::from_db_row(
                row.get("id"),
                row.get("isp_profile_id"),
                row.get("start_hour"),
                row.get("start_minute"),
                row.get("end_hour"),
                row.get("end_minute"),
                &row.get::<String, _>("days_of_week"),
                row.get("severity"),
                row.get("confidence"),
            )
        }).collect();
        
        Ok(patterns)
    }
    
    /// Optimization strategy operations
    pub async fn save_optimization_strategy(&self, strategy: &OptimizationStrategy) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO optimization_strategies (name, server_rotation_interval_minutes, packet_timing_min_seconds, packet_timing_max_seconds, connection_count, traffic_intensity, stealth_level, effectiveness_score, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&strategy.name)
        .bind(strategy.server_rotation_interval_minutes)
        .bind(strategy.packet_timing_min_seconds)
        .bind(strategy.packet_timing_max_seconds)
        .bind(strategy.connection_count)
        .bind(strategy.traffic_intensity)
        .bind(&strategy.stealth_level.to_string())
        .bind(strategy.effectiveness_score)
        .bind(&strategy.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(result.last_insert_rowid())
    }
    
    pub async fn get_best_optimization_strategy(&self) -> Result<Option<OptimizationStrategy>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, server_rotation_interval_minutes, packet_timing_min_seconds, packet_timing_max_seconds, connection_count, traffic_intensity, stealth_level, effectiveness_score, created_at
            FROM optimization_strategies
            WHERE effectiveness_score IS NOT NULL
            ORDER BY effectiveness_score DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await?;
        
        let strategy = row.map(|r| OptimizationStrategy {
            id: r.get("id"),
            name: r.get("name"),
            server_rotation_interval_minutes: r.get("server_rotation_interval_minutes"),
            packet_timing_min_seconds: r.get("packet_timing_min_seconds"),
            packet_timing_max_seconds: r.get("packet_timing_max_seconds"),
            connection_count: r.get("connection_count"),
            traffic_intensity: r.get("traffic_intensity"),
            stealth_level: StealthLevel::from_string(&r.get::<String, _>("stealth_level")),
            effectiveness_score: r.get("effectiveness_score"),
            created_at: r.get("created_at"),
        });
        
        Ok(strategy)
    }
    
    /// Cleanup old data (privacy-focused approach)
    pub async fn cleanup_old_data(&self, days_to_keep: u32) -> Result<()> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days_to_keep as i64);
        
        sqlx::query("DELETE FROM speed_measurements WHERE timestamp < ?")
            .bind(cutoff_date)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }

    // Speedtest Server operations
    pub async fn save_speedtest_server(&self, server: &SpeedtestServer) -> Result<i64> {
        let id = sqlx::query(
            r#"
            INSERT INTO speedtest_servers (
                server_id, host, port, name, country, sponsor, 
                distance, latency, is_active, last_used
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&server.server_id)
        .bind(&server.host)
        .bind(server.port)
        .bind(&server.name)
        .bind(&server.country)
        .bind(&server.sponsor)
        .bind(server.distance)
        .bind(server.latency)
        .bind(server.is_active)
        .bind(server.last_used)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        
        Ok(id)
    }

    pub async fn get_active_speedtest_servers(&self) -> Result<Vec<SpeedtestServer>> {
        let rows = sqlx::query(
            "SELECT * FROM speedtest_servers WHERE is_active = 1 ORDER BY country, name"
        )
        .fetch_all(&self.pool)
        .await?;
        
        let servers = rows.into_iter().map(|row| {
            SpeedtestServer {
                id: row.get("id"),
                server_id: row.get("server_id"),
                host: row.get("host"),
                port: row.get("port"),
                name: row.get("name"),
                country: row.get("country"),
                sponsor: row.get("sponsor"),
                distance: row.get("distance"),
                latency: row.get("latency"),
                is_active: row.get("is_active"),
                last_used: row.get("last_used"),
            }
        }).collect();
        
        Ok(servers)
    }

    pub async fn get_servers_by_country(&self, country: &str) -> Result<Vec<SpeedtestServer>> {
        let rows = sqlx::query(
            "SELECT * FROM speedtest_servers WHERE country = ? AND is_active = 1 ORDER BY name"
        )
        .bind(country)
        .fetch_all(&self.pool)
        .await?;
        
        let servers = rows.into_iter().map(|row| {
            SpeedtestServer {
                id: row.get("id"),
                server_id: row.get("server_id"),
                host: row.get("host"),
                port: row.get("port"),
                name: row.get("name"),
                country: row.get("country"),
                sponsor: row.get("sponsor"),
                distance: row.get("distance"),
                latency: row.get("latency"),
                is_active: row.get("is_active"),
                last_used: row.get("last_used"),
            }
        }).collect();
        
        Ok(servers)
    }

    pub async fn update_server_last_used(&self, server_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE speedtest_servers SET last_used = ? WHERE server_id = ?"
        )
        .bind(Utc::now())
        .bind(server_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }

    // App Configuration operations
    pub async fn save_app_config(&self, config: &AppConfig) -> Result<i64> {
        // First, check if config already exists
        let existing = sqlx::query("SELECT id FROM app_config LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        
        if let Some(row) = existing {
            // Update existing config
            let id: i64 = row.get("id");
            sqlx::query(
                r#"
                UPDATE app_config SET
                    auto_start = ?,
                    monitoring_enabled = ?,
                    optimization_enabled = ?,
                    data_retention_days = ?,
                    bandwidth_limit_mb_per_hour = ?,
                    notification_level = ?,
                    updated_at = ?
                WHERE id = ?
                "#
            )
            .bind(config.auto_start)
            .bind(config.monitoring_enabled)
            .bind(config.optimization_enabled)
            .bind(config.data_retention_days)
            .bind(config.bandwidth_limit_mb_per_hour)
            .bind(&config.notification_level)
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
            
            Ok(id)
        } else {
            // Insert new config
            let id = sqlx::query(
                r#"
                INSERT INTO app_config (
                    auto_start, monitoring_enabled, optimization_enabled,
                    data_retention_days, bandwidth_limit_mb_per_hour,
                    notification_level, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#
            )
            .bind(config.auto_start)
            .bind(config.monitoring_enabled)
            .bind(config.optimization_enabled)
            .bind(config.data_retention_days)
            .bind(config.bandwidth_limit_mb_per_hour)
            .bind(&config.notification_level)
            .bind(config.created_at)
            .bind(config.updated_at)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();
            
            Ok(id)
        }
    }

    pub async fn get_app_config(&self) -> Result<Option<AppConfig>> {
        let row = sqlx::query("SELECT * FROM app_config LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;
        
        let config = row.map(|r| AppConfig {
            id: r.get("id"),
            auto_start: r.get("auto_start"),
            monitoring_enabled: r.get("monitoring_enabled"),
            optimization_enabled: r.get("optimization_enabled"),
            data_retention_days: r.get("data_retention_days"),
            bandwidth_limit_mb_per_hour: r.get("bandwidth_limit_mb_per_hour"),
            notification_level: r.get("notification_level"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        });
        
        Ok(config)
    }

    // Analytics and reporting methods
    pub async fn get_speed_statistics(&self, days: u32) -> Result<SpeedStatistics> {
        let since = Utc::now() - chrono::Duration::days(days as i64);
        
        let stats = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_measurements,
                AVG(download_mbps) as avg_download,
                MAX(download_mbps) as max_download,
                MIN(download_mbps) as min_download,
                AVG(upload_mbps) as avg_upload,
                AVG(latency_ms) as avg_latency,
                AVG(CASE WHEN optimization_active THEN download_mbps END) as avg_optimized_download,
                AVG(CASE WHEN NOT optimization_active THEN download_mbps END) as avg_baseline_download
            FROM speed_measurements 
            WHERE timestamp >= ?
            "#
        )
        .bind(since)
        .fetch_one(&self.pool)
        .await?;
        
        Ok(SpeedStatistics {
            total_measurements: stats.get("total_measurements"),
            avg_download_mbps: stats.get::<Option<f64>, _>("avg_download").unwrap_or(0.0),
            max_download_mbps: stats.get::<Option<f64>, _>("max_download").unwrap_or(0.0),
            min_download_mbps: stats.get::<Option<f64>, _>("min_download").unwrap_or(0.0),
            avg_upload_mbps: stats.get::<Option<f64>, _>("avg_upload").unwrap_or(0.0),
            avg_latency_ms: stats.get::<Option<f64>, _>("avg_latency").unwrap_or(0.0),
            avg_optimized_download_mbps: stats.get::<Option<f64>, _>("avg_optimized_download"),
            avg_baseline_download_mbps: stats.get::<Option<f64>, _>("avg_baseline_download"),
            improvement_factor: None, // Will be calculated
        })
    }

    pub async fn get_throttling_effectiveness(&self, isp_profile_id: i64, days: u32) -> Result<f64> {
        let since = Utc::now() - chrono::Duration::days(days as i64);
        
        let result = sqlx::query(
            r#"
            SELECT 
                AVG(CASE WHEN optimization_active THEN download_mbps END) as optimized_avg,
                AVG(CASE WHEN NOT optimization_active THEN download_mbps END) as baseline_avg
            FROM speed_measurements sm
            JOIN throttling_patterns tp ON tp.isp_profile_id = ?
            WHERE sm.timestamp >= ?
            "#
        )
        .bind(isp_profile_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;
        
        let optimized_avg: Option<f64> = result.get("optimized_avg");
        let baseline_avg: Option<f64> = result.get("baseline_avg");
        
        match (optimized_avg, baseline_avg) {
            (Some(opt), Some(base)) if base > 0.0 => Ok(opt / base),
            _ => Ok(1.0), // No improvement detected
        }
    }

    /// Permanently deletes all user data from the database
    /// Use with caution. Intended for legal compliance (opt-out / data deletion)
    pub async fn delete_all_user_data(&self) -> Result<()> {
        // Order matters due to foreign keys
        sqlx::query("DELETE FROM speed_measurements").execute(&self.pool).await?;
        sqlx::query("DELETE FROM throttling_patterns").execute(&self.pool).await?;
        sqlx::query("DELETE FROM optimization_strategies").execute(&self.pool).await?;
        sqlx::query("DELETE FROM speedtest_servers").execute(&self.pool).await?;
        sqlx::query("DELETE FROM isp_profiles").execute(&self.pool).await?;
        // Keep app_config so app can retain preferences; do not delete schema_migrations
        Ok(())
    }
}

/// Speed statistics for analytics
#[derive(Debug, Clone)]
pub struct SpeedStatistics {
    pub total_measurements: i64,
    pub avg_download_mbps: f64,
    pub max_download_mbps: f64,
    pub min_download_mbps: f64,
    pub avg_upload_mbps: f64,
    pub avg_latency_ms: f64,
    pub avg_optimized_download_mbps: Option<f64>,
    pub avg_baseline_download_mbps: Option<f64>,
    pub improvement_factor: Option<f64>,
}

impl SpeedStatistics {
    /// Calculate the improvement factor if both optimized and baseline data exist
    pub fn calculate_improvement_factor(&mut self) {
        if let (Some(optimized), Some(baseline)) = (self.avg_optimized_download_mbps, self.avg_baseline_download_mbps) {
            if baseline > 0.0 {
                self.improvement_factor = Some(optimized / baseline);
            }
        }
    }
}#[cfg(test)
]
mod tests {
    use super::*;
    use crate::data::migrations::MigrationManager;
    use chrono::Weekday;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> SqlitePool {
        let database_url = ":memory:";
        let pool = SqlitePool::connect(database_url).await.unwrap();
        
        let migration_manager = MigrationManager::new(database_url.to_string());
        migration_manager.run_migrations(&pool).await.unwrap();
        
        pool
    }

    #[tokio::test]
    async fn test_speed_measurement_crud() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        let measurement = SpeedMeasurement::new(50.0, 10.0, 25, false);
        
        // Test save
        let id = repo.save_speed_measurement(&measurement).await.unwrap();
        assert!(id > 0);
        
        // Test retrieve
        let since = Utc::now() - chrono::Duration::hours(1);
        let measurements = repo.get_speed_measurements_since(since).await.unwrap();
        assert_eq!(measurements.len(), 1);
        assert_eq!(measurements[0].download_mbps, 50.0);
    }

    #[tokio::test]
    async fn test_isp_profile_operations() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        let profile = ISPProfile::new(
            "Hutch".to_string(),
            "Sri Lanka".to_string(),
            "DNS Analysis".to_string(),
        );
        
        // Test save
        let id = repo.save_isp_profile(&profile).await.unwrap();
        assert!(id > 0);
        
        // Test retrieve
        let retrieved = repo.get_current_isp_profile().await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Hutch");
    }

    #[tokio::test]
    async fn test_throttling_pattern_operations() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        // First create an ISP profile
        let profile = ISPProfile::new(
            "Test ISP".to_string(),
            "Test Region".to_string(),
            "Test Method".to_string(),
        );
        let isp_id = repo.save_isp_profile(&profile).await.unwrap();
        
        // Create throttling pattern
        let pattern = ThrottlingPattern::new(
            isp_id,
            19, 0,  // 7 PM
            22, 0,  // 10 PM
            vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
            0.8,
        );
        
        // Test save
        let pattern_id = repo.save_throttling_pattern(&pattern).await.unwrap();
        assert!(pattern_id > 0);
        
        // Test retrieve
        let patterns = repo.get_throttling_patterns_for_isp(isp_id).await.unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].severity, 0.8);
    }

    #[tokio::test]
    async fn test_optimization_strategy_operations() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        let mut strategy = OptimizationStrategy::default_strategy();
        strategy.effectiveness_score = Some(0.8); // Set effectiveness score for testing
        
        // Test save
        let id = repo.save_optimization_strategy(&strategy).await.unwrap();
        assert!(id > 0);
        
        // Test retrieve
        let retrieved = repo.get_best_optimization_strategy().await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Default");
    }

    #[tokio::test]
    async fn test_speedtest_server_operations() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        let server = SpeedtestServer::new(
            "12345".to_string(),
            "speedtest.example.com".to_string(),
            8080,
            "Test Server".to_string(),
            "Singapore".to_string(),
            "Test Sponsor".to_string(),
        );
        
        // Test save
        let id = repo.save_speedtest_server(&server).await.unwrap();
        assert!(id > 0);
        
        // Test retrieve active servers
        let servers = repo.get_active_speedtest_servers().await.unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].server_id, "12345");
        
        // Test retrieve by country
        let sg_servers = repo.get_servers_by_country("Singapore").await.unwrap();
        assert_eq!(sg_servers.len(), 1);
        
        // Test update last used
        repo.update_server_last_used("12345").await.unwrap();
    }

    #[tokio::test]
    async fn test_app_config_operations() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        let config = AppConfig::default();
        
        // Test save (insert)
        let id = repo.save_app_config(&config).await.unwrap();
        assert!(id > 0);
        
        // Test retrieve
        let retrieved = repo.get_app_config().await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.as_ref().unwrap().auto_start, true);
        
        // Test save (update)
        let mut updated_config = retrieved.unwrap();
        updated_config.auto_start = false;
        let updated_id = repo.save_app_config(&updated_config).await.unwrap();
        assert_eq!(updated_id, id); // Should be same ID (update, not insert)
        
        // Verify update
        let final_config = repo.get_app_config().await.unwrap();
        assert_eq!(final_config.unwrap().auto_start, false);
    }

    #[tokio::test]
    async fn test_speed_statistics() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        // Add some test measurements
        let measurements = vec![
            SpeedMeasurement::new(50.0, 10.0, 25, false),
            SpeedMeasurement::new(100.0, 20.0, 30, true),
            SpeedMeasurement::new(75.0, 15.0, 20, false),
        ];
        
        for measurement in measurements {
            repo.save_speed_measurement(&measurement).await.unwrap();
        }
        
        // Get statistics
        let mut stats = repo.get_speed_statistics(7).await.unwrap();
        assert_eq!(stats.total_measurements, 3);
        assert_eq!(stats.avg_download_mbps, 75.0); // (50 + 100 + 75) / 3
        assert_eq!(stats.max_download_mbps, 100.0);
        assert_eq!(stats.min_download_mbps, 50.0);
        
        // Test improvement factor calculation
        stats.calculate_improvement_factor();
        assert!(stats.improvement_factor.is_some());
        
        let improvement = stats.improvement_factor.unwrap();
        // Optimized: 100.0, Baseline: (50.0 + 75.0) / 2 = 62.5
        // Improvement: 100.0 / 62.5 = 1.6
        assert!((improvement - 1.6).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_cleanup_old_data() {
        let pool = setup_test_db().await;
        let repo = Repository::new(pool);
        
        // Add some measurements
        repo.save_speed_measurement(&SpeedMeasurement::new(50.0, 10.0, 25, false)).await.unwrap();
        
        // Test cleanup (should not fail)
        repo.cleanup_old_data(30).await.unwrap();
        
        // Verify data still exists (since it's recent)
        let since = Utc::now() - chrono::Duration::hours(1);
        let measurements = repo.get_speed_measurements_since(since).await.unwrap();
        assert_eq!(measurements.len(), 1);
    }
}