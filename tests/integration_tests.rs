use isp_speedkarma::data::repository::Repository;
use isp_speedkarma::data::models::*;
use isp_speedkarma::data::migrations::MigrationManager;
use sqlx::SqlitePool;
use chrono::Utc;

// Include server connection integration tests
mod integration {
    pub mod server_connection_tests;
    pub mod stealth_integration_tests;
}

#[path = "integration/dpi_evasion_tests.rs"]
mod dpi_evasion_tests;

#[path = "integration/effectiveness_analysis_tests.rs"]
mod effectiveness_analysis_tests;

async fn setup_test_db() -> SqlitePool {
    let database_url = ":memory:"; // In-memory database for tests
    let migration_manager = MigrationManager::new(database_url.to_string());
    
    migration_manager.create_database_if_not_exists().await.unwrap();
    let pool = SqlitePool::connect(database_url).await.unwrap();
    migration_manager.run_migrations(&pool).await.unwrap();
    
    pool
}

#[tokio::test]
async fn test_speed_measurement_crud() {
    let pool = setup_test_db().await;
    let repo = Repository::new(pool);
    
    let measurement = SpeedMeasurement::new(50.0, 10.0, 20, false);
    let id = repo.save_speed_measurement(&measurement).await.unwrap();
    
    assert!(id > 0);
    
    let measurements = repo.get_speed_measurements_since(Utc::now() - chrono::Duration::hours(1)).await.unwrap();
    assert_eq!(measurements.len(), 1);
    assert_eq!(measurements[0].download_mbps, 50.0);
}

#[tokio::test]
async fn test_isp_profile_operations() {
    let pool = setup_test_db().await;
    let repo = Repository::new(pool);
    
    let profile = ISPProfile::new("Hutch".to_string(), "Sri Lanka".to_string(), "DNS Analysis".to_string());
    let id = repo.save_isp_profile(&profile).await.unwrap();
    
    assert!(id > 0);
    
    let current_profile = repo.get_current_isp_profile().await.unwrap();
    assert!(current_profile.is_some());
    assert_eq!(current_profile.unwrap().name, "Hutch");
}