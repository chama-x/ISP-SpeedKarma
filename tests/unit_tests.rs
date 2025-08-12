use isp_speedkarma::core::error::*;
use isp_speedkarma::core::config::*;
use isp_speedkarma::core::intelligence::*;
use isp_speedkarma::network::stealth::{StealthEngine, StealthStats, DetectionRisk};
use isp_speedkarma::network::servers::ServerPool;
use isp_speedkarma::data::models::{SpeedtestServer, StealthLevel};
use std::sync::Arc;
use std::time::Duration;

// Include DPI bypass tests
#[path = "unit/dpi_bypass_tests.rs"]
mod dpi_bypass_tests;
mod pattern_learning_tests;

#[test]
fn test_error_user_notification() {
    let insufficient_data = SpeedKarmaError::InsufficientData { required: 3 };
    assert!(insufficient_data.should_notify_user());
    
    let network_error = SpeedKarmaError::NetworkUnavailable("Connection lost".to_string());
    assert!(!network_error.should_notify_user());
}

#[test]
fn test_error_severity() {
    let permissions_error = SpeedKarmaError::PermissionsRequired;
    assert_eq!(permissions_error.severity(), ErrorSeverity::High);
    
    let network_error = SpeedKarmaError::NetworkUnavailable("Temporary".to_string());
    assert_eq!(network_error.severity(), ErrorSeverity::Low);
}

#[test]
fn test_user_friendly_messages() {
    let insufficient_data = SpeedKarmaError::InsufficientData { required: 5 };
    let message = insufficient_data.user_message();
    assert!(message.contains("5 more days"));
    
    let permissions_error = SpeedKarmaError::PermissionsRequired;
    let message = permissions_error.user_message();
    assert!(message.contains("permissions"));
}

#[test]
fn test_default_config_validation() {
    let config = AppConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_invalid_confidence_level() {
    let mut config = AppConfig::default();
    config.auto_optimization.min_confidence = 1.5; // Invalid: > 1.0
    
    assert!(config.validate().is_err());
}

#[test]
fn test_invalid_improvement_factor() {
    let mut config = AppConfig::default();
    config.auto_optimization.min_improvement_factor = 0.5; // Invalid: < 1.0
    
    assert!(config.validate().is_err());
}

#[test]
fn test_time_range_creation() {
    let range = TimeRange::new("19:00", "22:00");
    assert_eq!(range.start_hour, 19);
    assert_eq!(range.start_minute, 0);
    assert_eq!(range.end_hour, 22);
    assert_eq!(range.end_minute, 0);
}

#[test]
fn test_system_status_learning() {
    let status = SystemStatus::learning(3, 7);
    
    match status.state {
        SystemState::Learning => {},
        _ => panic!("Expected Learning state"),
    }
    
    assert!(status.data_collection_progress.is_some());
    let progress = status.data_collection_progress.unwrap();
    assert_eq!(progress.days_collected, 3);
    assert_eq!(progress.days_needed, 7);
}

// Stealth Engine Tests
#[tokio::test]
async fn test_stealth_engine_creation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);
    
    let stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(stats.stealth_level, StealthLevel::Medium);
    assert_eq!(stats.active_connections, 0);
}

#[tokio::test]
async fn test_traffic_pattern_creation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    
    // Test different stealth levels create appropriate patterns
    let low_stealth = StealthEngine::new(server_pool.clone(), StealthLevel::Low);
    let high_stealth = StealthEngine::new(server_pool.clone(), StealthLevel::Maximum);
    
    let low_stats = low_stealth.get_stealth_stats().await;
    let high_stats = high_stealth.get_stealth_stats().await;
    
    assert_eq!(low_stats.stealth_level, StealthLevel::Low);
    assert_eq!(high_stats.stealth_level, StealthLevel::Maximum);
    
    // Maximum stealth should have shorter rotation intervals
    assert!(high_stats.next_rotation_in < Duration::from_secs(300));
}

#[tokio::test]
async fn test_server_rotation_logic() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Add test servers
    let test_servers = vec![
        SpeedtestServer::new(
            "test1".to_string(),
            "test1.example.com".to_string(),
            8080,
            "Test Server 1".to_string(),
            "Sri Lanka".to_string(),
            "Test Sponsor".to_string(),
        ),
        SpeedtestServer::new(
            "test2".to_string(),
            "test2.example.com".to_string(),
            8080,
            "Test Server 2".to_string(),
            "Singapore".to_string(),
            "Test Sponsor".to_string(),
        ),
    ];
    
    server_pool.set_servers(test_servers);
    let server_pool = Arc::new(server_pool);
    
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);
    
    // Test server rotation
    let result = stealth_engine.rotate_servers().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_speedtest_payload_generation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);
    
    // Test payload generation with different sizes
    let small_payload = stealth_engine.generate_speedtest_payload(100);
    let large_payload = stealth_engine.generate_speedtest_payload(1000);
    
    assert!(small_payload.len() <= 100);
    assert!(large_payload.len() <= 1000);
    assert!(small_payload.starts_with("content1="));
    assert!(large_payload.starts_with("content1="));
}

#[tokio::test]
async fn test_random_string_generation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);
    
    let random_str1 = stealth_engine.generate_random_string(8);
    let random_str2 = stealth_engine.generate_random_string(8);
    
    assert_eq!(random_str1.len(), 8);
    assert_eq!(random_str2.len(), 8);
    assert_ne!(random_str1, random_str2); // Should be different (very high probability)
    
    // Should only contain hex characters
    assert!(random_str1.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn test_stealth_level_update() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let mut stealth_engine = StealthEngine::new(server_pool, StealthLevel::Low);
    
    let initial_stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(initial_stats.stealth_level, StealthLevel::Low);
    
    // Update stealth level
    let result = stealth_engine.update_stealth_level(StealthLevel::Maximum).await;
    assert!(result.is_ok());
    
    let updated_stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(updated_stats.stealth_level, StealthLevel::Maximum);
}

#[tokio::test]
async fn test_stealth_engine_start_stop() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Add at least one test server
    let test_servers = vec![
        SpeedtestServer::new(
            "test1".to_string(),
            "test1.example.com".to_string(),
            8080,
            "Test Server 1".to_string(),
            "Sri Lanka".to_string(),
            "Test Sponsor".to_string(),
        ),
    ];
    
    server_pool.set_servers(test_servers);
    let server_pool = Arc::new(server_pool);
    
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);
    
    // Test stop (should always succeed)
    let stop_result = stealth_engine.stop().await;
    assert!(stop_result.is_ok());
}

#[tokio::test]
async fn test_connection_stats_tracking() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);
    
    let test_server = SpeedtestServer::new(
        "test1".to_string(),
        "test1.example.com".to_string(),
        8080,
        "Test Server 1".to_string(),
        "Sri Lanka".to_string(),
        "Test Sponsor".to_string(),
    );
    
    // Update connection stats
    stealth_engine.update_connection_stats(&test_server, 1024).await;
    
    let stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(stats.active_connections, 1);
    assert_eq!(stats.total_bytes_sent, 1024);
    assert_eq!(stats.total_packets_sent, 1);
}