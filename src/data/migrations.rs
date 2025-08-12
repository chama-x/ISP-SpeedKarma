use crate::core::error::Result;
use sqlx::{SqlitePool, migrate::MigrateDatabase, Row};
use chrono::{DateTime, Utc};

/// Database migration management with versioning
pub struct MigrationManager {
    database_url: String,
}

/// Migration information
#[derive(Debug)]
pub struct Migration {
    pub version: i32,
    pub name: String,
    pub sql: String,
    pub applied_at: Option<DateTime<Utc>>,
}

impl MigrationManager {
    pub fn new(database_url: String) -> Self {
        Self { database_url }
    }
    
    /// Creates the database if it doesn't exist
    pub async fn create_database_if_not_exists(&self) -> Result<()> {
        if !sqlx::Sqlite::database_exists(&self.database_url).await? {
            sqlx::Sqlite::create_database(&self.database_url).await?;
        }
        Ok(())
    }
    
    /// Runs all pending migrations
    pub async fn run_migrations(&self, pool: &SqlitePool) -> Result<()> {
        // First, create the migrations table to track applied migrations
        self.create_migrations_table(pool).await?;
        
        // Get all available migrations
        let migrations = self.get_all_migrations();
        
        // Get applied migrations
        let applied_versions = self.get_applied_migration_versions(pool).await?;
        
        // Apply pending migrations
        for migration in migrations {
            if !applied_versions.contains(&migration.version) {
                println!("Applying migration {}: {}", migration.version, migration.name);
                
                // Execute the migration
                sqlx::query(&migration.sql)
                    .execute(pool)
                    .await?;
                
                // Record the migration as applied
                self.record_migration_applied(pool, &migration).await?;
                
                println!("Migration {} applied successfully", migration.version);
            }
        }
        
        Ok(())
    }

    /// Creates the migrations tracking table
    async fn create_migrations_table(&self, pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(pool)
        .await?;
        
        Ok(())
    }

    /// Gets all available migrations in order
    fn get_all_migrations(&self) -> Vec<Migration> {
        vec![
            Migration {
                version: 1,
                name: "create_speed_measurements_table".to_string(),
                sql: self.get_speed_measurements_table_sql(),
                applied_at: None,
            },
            Migration {
                version: 2,
                name: "create_isp_profiles_table".to_string(),
                sql: self.get_isp_profiles_table_sql(),
                applied_at: None,
            },
            Migration {
                version: 3,
                name: "create_throttling_patterns_table".to_string(),
                sql: self.get_throttling_patterns_table_sql(),
                applied_at: None,
            },
            Migration {
                version: 4,
                name: "create_optimization_strategies_table".to_string(),
                sql: self.get_optimization_strategies_table_sql(),
                applied_at: None,
            },
            Migration {
                version: 5,
                name: "create_speedtest_servers_table".to_string(),
                sql: self.get_speedtest_servers_table_sql(),
                applied_at: None,
            },
            Migration {
                version: 6,
                name: "create_app_config_table".to_string(),
                sql: self.get_app_config_table_sql(),
                applied_at: None,
            },
            Migration {
                version: 7,
                name: "add_indexes_for_performance".to_string(),
                sql: self.get_performance_indexes_sql(),
                applied_at: None,
            },
        ]
    }

    /// Gets applied migration versions
    async fn get_applied_migration_versions(&self, pool: &SqlitePool) -> Result<Vec<i32>> {
        let rows = sqlx::query("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;
        
        let versions = rows.into_iter()
            .map(|row| row.get::<i32, _>("version"))
            .collect();
        
        Ok(versions)
    }

    /// Records a migration as applied
    async fn record_migration_applied(&self, pool: &SqlitePool, migration: &Migration) -> Result<()> {
        sqlx::query(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, ?)"
        )
        .bind(migration.version)
        .bind(&migration.name)
        .bind(Utc::now())
        .execute(pool)
        .await?;
        
        Ok(())
    }

    /// Reverts the last applied migration (for development)
    pub async fn revert_last_migration(&self, pool: &SqlitePool) -> Result<()> {
        let last_version = sqlx::query("SELECT version FROM schema_migrations ORDER BY version DESC LIMIT 1")
            .fetch_optional(pool)
            .await?;
        
        if let Some(row) = last_version {
            let version: i32 = row.get("version");
            
            // Note: In a production system, you'd want to store rollback SQL
            // For now, we'll just remove the migration record
            sqlx::query("DELETE FROM schema_migrations WHERE version = ?")
                .bind(version)
                .execute(pool)
                .await?;
            
            println!("Migration {} reverted (table structure remains)", version);
        }
        
        Ok(())
    }
    
    fn get_speed_measurements_table_sql(&self) -> String {
        r#"
        CREATE TABLE IF NOT EXISTS speed_measurements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp DATETIME NOT NULL,
            download_mbps REAL NOT NULL,
            upload_mbps REAL NOT NULL,
            latency_ms INTEGER NOT NULL,
            optimization_active BOOLEAN NOT NULL,
            confidence REAL NOT NULL DEFAULT 1.0,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#.to_string()
    }
    
    fn get_isp_profiles_table_sql(&self) -> String {
        r#"
        CREATE TABLE IF NOT EXISTS isp_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            region TEXT NOT NULL,
            detection_method TEXT NOT NULL,
            created_at DATETIME NOT NULL,
            updated_at DATETIME NOT NULL
        );
        "#.to_string()
    }
    
    fn get_throttling_patterns_table_sql(&self) -> String {
        r#"
        CREATE TABLE IF NOT EXISTS throttling_patterns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            isp_profile_id INTEGER NOT NULL,
            start_hour INTEGER NOT NULL,
            start_minute INTEGER NOT NULL,
            end_hour INTEGER NOT NULL,
            end_minute INTEGER NOT NULL,
            days_of_week TEXT NOT NULL,
            severity REAL NOT NULL,
            confidence REAL NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (isp_profile_id) REFERENCES isp_profiles (id)
        );
        "#.to_string()
    }
    
    fn get_optimization_strategies_table_sql(&self) -> String {
        r#"
        CREATE TABLE IF NOT EXISTS optimization_strategies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            server_rotation_interval_minutes INTEGER NOT NULL,
            packet_timing_min_seconds REAL NOT NULL,
            packet_timing_max_seconds REAL NOT NULL,
            connection_count INTEGER NOT NULL,
            traffic_intensity REAL NOT NULL,
            stealth_level TEXT NOT NULL,
            effectiveness_score REAL,
            created_at DATETIME NOT NULL
        );
        "#.to_string()
    }

    fn get_speedtest_servers_table_sql(&self) -> String {
        r#"
        CREATE TABLE IF NOT EXISTS speedtest_servers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            server_id TEXT NOT NULL UNIQUE,
            host TEXT NOT NULL,
            port INTEGER NOT NULL,
            name TEXT NOT NULL,
            country TEXT NOT NULL,
            sponsor TEXT NOT NULL,
            distance REAL,
            latency REAL,
            is_active BOOLEAN NOT NULL DEFAULT 1,
            last_used DATETIME,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "#.to_string()
    }

    fn get_app_config_table_sql(&self) -> String {
        r#"
        CREATE TABLE IF NOT EXISTS app_config (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            auto_start BOOLEAN NOT NULL DEFAULT 1,
            monitoring_enabled BOOLEAN NOT NULL DEFAULT 1,
            optimization_enabled BOOLEAN NOT NULL DEFAULT 0,
            data_retention_days INTEGER NOT NULL DEFAULT 30,
            bandwidth_limit_mb_per_hour REAL,
            notification_level TEXT NOT NULL DEFAULT 'Normal',
            created_at DATETIME NOT NULL,
            updated_at DATETIME NOT NULL
        );
        "#.to_string()
    }

    fn get_performance_indexes_sql(&self) -> String {
        r#"
        CREATE INDEX IF NOT EXISTS idx_speed_measurements_timestamp ON speed_measurements(timestamp);
        CREATE INDEX IF NOT EXISTS idx_speed_measurements_optimization_active ON speed_measurements(optimization_active);
        CREATE INDEX IF NOT EXISTS idx_throttling_patterns_isp_profile_id ON throttling_patterns(isp_profile_id);
        CREATE INDEX IF NOT EXISTS idx_speedtest_servers_country ON speedtest_servers(country);
        CREATE INDEX IF NOT EXISTS idx_speedtest_servers_is_active ON speedtest_servers(is_active);
        "#.to_string()
    }
}#[cfg
(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[tokio::test]
    async fn test_migration_system() {
        let database_url = ":memory:";
        let pool = SqlitePool::connect(database_url).await.unwrap();
        let migration_manager = MigrationManager::new(database_url.to_string());
        
        // Test running migrations
        migration_manager.run_migrations(&pool).await.unwrap();
        
        // Verify migrations table exists and has records
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert!(count > 0);
        
        // Verify all expected tables exist
        let tables = vec![
            "speed_measurements",
            "isp_profiles", 
            "throttling_patterns",
            "optimization_strategies",
            "speedtest_servers",
            "app_config"
        ];
        
        for table in tables {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?"
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .unwrap();
            
            assert_eq!(exists, 1, "Table {} should exist", table);
        }
    }

    #[tokio::test]
    async fn test_migration_idempotency() {
        let database_url = ":memory:";
        let pool = SqlitePool::connect(database_url).await.unwrap();
        let migration_manager = MigrationManager::new(database_url.to_string());
        
        // Run migrations twice
        migration_manager.run_migrations(&pool).await.unwrap();
        migration_manager.run_migrations(&pool).await.unwrap();
        
        // Should not fail and should not duplicate migration records
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        // Should have exactly the number of migrations we defined
        let expected_migrations = migration_manager.get_all_migrations().len() as i64;
        assert_eq!(count, expected_migrations);
    }

    #[tokio::test]
    async fn test_migration_versioning() {
        let database_url = ":memory:";
        let pool = SqlitePool::connect(database_url).await.unwrap();
        let migration_manager = MigrationManager::new(database_url.to_string());
        
        migration_manager.run_migrations(&pool).await.unwrap();
        
        // Check that migrations are applied in order
        let versions: Vec<i32> = sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
        
        // Verify versions are sequential starting from 1
        for (i, version) in versions.iter().enumerate() {
            assert_eq!(*version, (i + 1) as i32);
        }
    }

    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_speedkarma.db");
        let database_url = format!("sqlite://{}", db_path.display());
        
        // Clean up any existing test database
        let _ = std::fs::remove_file(&db_path);
        
        let migration_manager = MigrationManager::new(database_url.clone());
        
        // Test database creation
        migration_manager.create_database_if_not_exists().await.unwrap();
        
        // Verify database file exists
        assert!(db_path.exists());
        
        // Clean up
        let _ = std::fs::remove_file(&db_path);
    }
}