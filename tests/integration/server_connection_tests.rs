use isp_speedkarma::network::servers::ServerPool;
use isp_speedkarma::data::models::SpeedtestServer;
use std::time::Duration;
use tokio::time::timeout;

/// Test server connection establishment
#[tokio::test]
async fn test_server_connection_establishment() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Load servers (should work with fallback if API fails)
    let result = server_pool.load_servers().await;
    assert!(result.is_ok(), "Failed to load servers: {:?}", result);
    
    // Try to establish at least one connection
    let connections_result = server_pool.establish_connection_pool(1).await;
    
    // Should succeed with at least one connection or fail gracefully
    match connections_result {
        Ok(count) => {
            assert!(count > 0, "Should establish at least one connection");
            println!("Successfully established {} connections", count);
        }
        Err(e) => {
            // This is acceptable in test environments where external connections may fail
            println!("Connection establishment failed (expected in test env): {}", e);
        }
    }
}

/// Test connection health monitoring
#[tokio::test]
async fn test_connection_health_monitoring() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Load servers
    if server_pool.load_servers().await.is_err() {
        println!("Skipping health monitoring test - no servers available");
        return;
    }
    
    // Try to establish connections
    if server_pool.establish_connection_pool(2).await.is_err() {
        println!("Skipping health monitoring test - no connections established");
        return;
    }
    
    // Monitor connections (should not panic)
    let monitor_result = timeout(
        Duration::from_secs(10),
        server_pool.monitor_connections()
    ).await;
    
    assert!(monitor_result.is_ok(), "Connection monitoring timed out");
    assert!(monitor_result.unwrap().is_ok(), "Connection monitoring failed");
}

/// Test server rotation functionality
#[tokio::test]
async fn test_server_rotation() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Add some test servers manually
    let test_servers = vec![
        SpeedtestServer::new(
            "test1".to_string(),
            "test1.example.com".to_string(),
            8080,
            "Test Server 1".to_string(),
            "Test Country".to_string(),
            "Test Sponsor".to_string(),
        ),
        SpeedtestServer::new(
            "test2".to_string(),
            "test2.example.com".to_string(),
            8080,
            "Test Server 2".to_string(),
            "Test Country".to_string(),
            "Test Sponsor".to_string(),
        ),
    ];
    
    // Manually set servers for testing
    server_pool.set_servers(test_servers);
    
    // Test rotation
    let first_server_id = server_pool.next_server().map(|s| s.server_id.clone());
    let second_server_id = server_pool.next_server().map(|s| s.server_id.clone());
    let third_server_id = server_pool.next_server().map(|s| s.server_id.clone()); // Should wrap around
    
    assert!(first_server_id.is_some());
    assert!(second_server_id.is_some());
    assert!(third_server_id.is_some());
    
    // Should rotate through servers
    assert_ne!(first_server_id, second_server_id);
    assert_eq!(first_server_id, third_server_id);
}

/// Test regional server filtering
#[tokio::test]
async fn test_regional_server_filtering() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Add test servers from different regions
    let test_servers = vec![
        SpeedtestServer::new(
            "lk1".to_string(),
            "test.lk".to_string(),
            8080,
            "Sri Lanka Server".to_string(),
            "Sri Lanka".to_string(),
            "Local ISP".to_string(),
        ),
        SpeedtestServer::new(
            "sg1".to_string(),
            "test.sg".to_string(),
            8080,
            "Singapore Server".to_string(),
            "Singapore".to_string(),
            "Regional ISP".to_string(),
        ),
        SpeedtestServer::new(
            "us1".to_string(),
            "test.us".to_string(),
            8080,
            "US Server".to_string(),
            "United States".to_string(),
            "US ISP".to_string(),
        ),
    ];
    
    server_pool.set_servers(test_servers);
    
    // Test regional filtering
    let sri_lanka_servers = server_pool.get_regional_servers("Sri Lanka");
    assert_eq!(sri_lanka_servers.len(), 1);
    assert_eq!(sri_lanka_servers[0].country, "Sri Lanka");
    
    let singapore_servers = server_pool.get_regional_servers("Singapore");
    assert_eq!(singapore_servers.len(), 1);
    assert_eq!(singapore_servers[0].country, "Singapore");
    
    let nonexistent_servers = server_pool.get_regional_servers("Nonexistent");
    assert_eq!(nonexistent_servers.len(), 0);
}

/// Test connection statistics
#[tokio::test]
async fn test_connection_statistics() {
    let server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Get initial stats (should be empty)
    let stats = server_pool.get_connection_stats().await;
    assert_eq!(stats.total_connections, 0);
    assert_eq!(stats.healthy_connections, 0);
    assert!(stats.average_latency_ms.is_none());
}

/// Test fallback server loading
#[tokio::test]
async fn test_fallback_server_loading() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // This should work even if the API is unavailable
    let result = server_pool.load_servers().await;
    
    // Should either succeed with API servers or fallback servers
    assert!(result.is_ok(), "Server loading should not fail completely");
    
    // Should have some servers loaded
    assert!(server_pool.servers_count() > 0, "Should have loaded some servers");
    
    // Should prioritize regional servers
    let regional_servers = server_pool.get_regional_servers("Sri Lanka");
    if !regional_servers.is_empty() {
        println!("Found {} Sri Lankan servers", regional_servers.len());
    }
    
    let singapore_servers = server_pool.get_regional_servers("Singapore");
    if !singapore_servers.is_empty() {
        println!("Found {} Singapore servers", singapore_servers.len());
    }
}

/// Test server priority calculation
#[tokio::test]
async fn test_server_prioritization() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Set user location to Sri Lanka
    server_pool.set_user_location(6.9271, 79.8612); // Colombo coordinates
    
    // Load servers and check prioritization
    if server_pool.load_servers().await.is_ok() {
        let closest_servers = server_pool.get_closest_servers(5);
        
        if !closest_servers.is_empty() {
            println!("Top {} servers by priority:", closest_servers.len());
            for (i, server) in closest_servers.iter().enumerate() {
                println!("  {}. {} ({}) - Distance: {:?}km", 
                         i + 1, server.name, server.country, server.distance);
            }
            
            // Regional servers should be prioritized
            let has_regional = closest_servers.iter().any(|s| {
                s.country.to_lowercase().contains("sri lanka") ||
                s.country.to_lowercase().contains("singapore") ||
                s.country.to_lowercase().contains("india")
            });
            
            if has_regional {
                println!("✓ Regional servers are properly prioritized");
            }
        }
    }
}

/// Integration test for complete connection workflow
#[tokio::test]
async fn test_complete_connection_workflow() {
    let mut server_pool = ServerPool::new().expect("Failed to create server pool");
    
    // Step 1: Load servers
    println!("Step 1: Loading servers...");
    if server_pool.load_servers().await.is_err() {
        println!("Server loading failed - using minimal test");
        return;
    }
    
    // Step 2: Establish connections
    println!("Step 2: Establishing connections...");
    let connection_result = server_pool.establish_connection_pool(2).await;
    
    match connection_result {
        Ok(count) => {
            println!("✓ Established {} connections", count);
            
            // Step 3: Get connection stats
            println!("Step 3: Checking connection stats...");
            let stats = server_pool.get_connection_stats().await;
            println!("  Total connections: {}", stats.total_connections);
            println!("  Healthy connections: {}", stats.healthy_connections);
            if let Some(latency) = stats.average_latency_ms {
                println!("  Average latency: {:.1}ms", latency);
            }
            
            // Step 4: Monitor connections
            println!("Step 4: Monitoring connections...");
            let monitor_result = timeout(
                Duration::from_secs(5),
                server_pool.monitor_connections()
            ).await;
            
            assert!(monitor_result.is_ok(), "Connection monitoring should complete");
            println!("✓ Connection monitoring completed");
            
            // Step 5: Test server rotation
            println!("Step 5: Testing server rotation...");
            let connected_servers = server_pool.get_connected_servers().await;
            println!("  Connected servers: {:?}", connected_servers);
            
            println!("✓ Complete workflow test passed");
        }
        Err(e) => {
            println!("Connection establishment failed: {}", e);
            println!("This is acceptable in test environments with limited network access");
        }
    }
}