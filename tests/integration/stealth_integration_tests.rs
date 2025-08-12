use isp_speedkarma::network::stealth::StealthEngine;
use isp_speedkarma::network::servers::ServerPool;
use isp_speedkarma::data::models::{SpeedtestServer, StealthLevel};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_stealth_engine_with_mock_servers() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Add test servers that point to httpbin.org for testing HTTP requests
    let test_servers = vec![
        SpeedtestServer::new(
            "httpbin1".to_string(),
            "httpbin.org".to_string(),
            80,
            "HTTPBin Test Server".to_string(),
            "Test Country".to_string(),
            "Test Sponsor".to_string(),
        ),
    ];
    
    server_pool.set_servers(test_servers);
    let server_pool = Arc::new(server_pool);
    
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Low);
    
    // Test that the stealth engine can be created and configured
    let stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(stats.stealth_level, StealthLevel::Low);
    assert_eq!(stats.active_connections, 0);
    
    // Test traffic generation (should not panic even if requests fail)
    let result = timeout(
        Duration::from_secs(10),
        stealth_engine.generate_mimicry_traffic()
    ).await;
    
    assert!(result.is_ok());
    let traffic_result = result.unwrap();
    assert!(traffic_result.is_ok());
}

#[tokio::test]
async fn test_stealth_client_creation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);
    
    // Test that authentic speedtest client can be created
    let client_result = stealth_engine.create_authentic_speedtest_client().await;
    assert!(client_result.is_ok());
    
    let client = client_result.unwrap();
    
    // Verify the client has proper headers by making a test request
    let test_result = timeout(
        Duration::from_secs(5),
        client.get("https://httpbin.org/headers").send()
    ).await;
    
    if let Ok(Ok(response)) = test_result {
        assert!(response.status().is_success());
        
        // Check that the response contains our User-Agent
        if let Ok(text) = response.text().await {
            assert!(text.contains("Mozilla/5.0"));
        }
    }
    // If the request fails (network issues), that's okay for this test
}

#[tokio::test]
async fn test_stealth_payload_authenticity() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);
    
    // Test payload generation with various sizes
    for size in [100, 500, 1000, 1500] {
        let payload = stealth_engine.generate_speedtest_payload(size);
        
        // Verify payload structure
        assert!(payload.starts_with("content1="));
        assert!(payload.len() <= size);
        
        // Verify payload contains only valid characters
        let content_part = &payload[9..]; // Skip "content1="
        assert!(content_part.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}

#[tokio::test]
async fn test_stealth_randomization() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Maximum);
    
    // Test that random string generation produces different results
    let mut random_strings = Vec::new();
    for _ in 0..10 {
        let random_str = stealth_engine.generate_random_string(16);
        assert_eq!(random_str.len(), 16);
        assert!(random_str.chars().all(|c| c.is_ascii_hexdigit()));
        random_strings.push(random_str);
    }
    
    // Verify that we got different strings (very high probability)
    let unique_count = random_strings.iter().collect::<std::collections::HashSet<_>>().len();
    assert!(unique_count > 5); // Should have at least some variety
    
    // Test timing randomization
    let mut delays = Vec::new();
    for _ in 0..5 {
        let delay = stealth_engine.calculate_next_cycle_delay().await;
        delays.push(delay);
    }
    
    // Verify delays are within expected range for Maximum stealth level
    for delay in &delays {
        assert!(*delay >= Duration::from_secs(15));
        assert!(*delay <= Duration::from_secs(180));
    }
    
    // Verify some variation in delays
    let unique_delays = delays.iter().collect::<std::collections::HashSet<_>>().len();
    assert!(unique_delays > 1); // Should have some variation
}

#[tokio::test]
async fn test_stealth_level_adaptation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let mut stealth_engine = StealthEngine::new(server_pool, StealthLevel::Low);
    
    // Test initial level
    let initial_stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(initial_stats.stealth_level, StealthLevel::Low);
    
    // Test level updates
    for level in [StealthLevel::Medium, StealthLevel::High, StealthLevel::Maximum] {
        let result = stealth_engine.update_stealth_level(level.clone()).await;
        assert!(result.is_ok());
        
        let stats = stealth_engine.get_stealth_stats().await;
        assert_eq!(stats.stealth_level, level);
    }
}

#[tokio::test]
async fn test_server_rotation_with_multiple_servers() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Add multiple test servers
    let test_servers = vec![
        SpeedtestServer::new(
            "test1".to_string(),
            "httpbin.org".to_string(),
            80,
            "Test Server 1".to_string(),
            "Sri Lanka".to_string(),
            "Test Sponsor".to_string(),
        ),
        SpeedtestServer::new(
            "test2".to_string(),
            "httpbin.org".to_string(),
            80,
            "Test Server 2".to_string(),
            "Singapore".to_string(),
            "Test Sponsor".to_string(),
        ),
        SpeedtestServer::new(
            "test3".to_string(),
            "httpbin.org".to_string(),
            80,
            "Test Server 3".to_string(),
            "India".to_string(),
            "Test Sponsor".to_string(),
        ),
    ];
    
    server_pool.set_servers(test_servers);
    let server_pool = Arc::new(server_pool);
    
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);
    
    // Test multiple rotations
    for _ in 0..5 {
        let result = stealth_engine.rotate_servers().await;
        assert!(result.is_ok());
        
        // Small delay between rotations
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    // Verify stats are updated
    let stats = stealth_engine.get_stealth_stats().await;
    assert_eq!(stats.stealth_level, StealthLevel::High);
}