use isp_speedkarma::network::stealth::{StealthEngine, DetectionRisk};
use isp_speedkarma::network::servers::ServerPool;
use isp_speedkarma::data::models::{SpeedtestServer, StealthLevel};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Test DPI evasion effectiveness by simulating various network conditions
#[tokio::test]
async fn test_dpi_evasion_under_throttling() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);

    // Add test servers
    let test_servers = vec![
        create_test_server("sg1", "speedtest.singapore.com", "Singapore"),
        create_test_server("in1", "speedtest.mumbai.com", "India"),
        create_test_server("lk1", "speedtest.colombo.com", "Sri Lanka"),
    ];

    // Initialize with test servers
    let mut pool = Arc::into_inner(stealth_engine.server_pool.clone())
        .unwrap_or_else(|| ServerPool::new().expect("Failed to create pool"));
    pool.set_servers(test_servers);
    let pool = Arc::new(pool);
    let stealth_engine = StealthEngine::new(pool, StealthLevel::High);
    stealth_engine.start().await.expect("Failed to start stealth engine");

    // Simulate DPI detection scenario
    let mut detection_attempts = 0;
    let max_attempts = 10;

    while detection_attempts < max_attempts {
        // Generate stealth traffic
        let result = stealth_engine.generate_mimicry_traffic().await;
        
        match result {
            Ok(_) => {
                // Success - record positive result
                stealth_engine.record_connection_result(true, Some(0.8)).await;
                println!("Stealth traffic generated successfully (attempt {})", detection_attempts + 1);
            }
            Err(e) => {
                // Failure - record negative result and adapt
                stealth_engine.record_connection_result(false, None).await;
                println!("Stealth traffic failed (attempt {}): {}", detection_attempts + 1, e);
                
                // Trigger adaptation
                stealth_engine.adapt_stealth_strategy().await.expect("Failed to adapt strategy");
            }
        }

        detection_attempts += 1;
        
        // Check if we've adapted to high detection risk
        let current_risk = stealth_engine.assess_detection_risk().await;
        if current_risk == DetectionRisk::High {
            println!("High detection risk detected - testing advanced evasion");
            break;
        }

        // Wait between attempts
        sleep(Duration::from_millis(100)).await;
    }

    // Verify that stealth engine adapted appropriately
    let final_stats = stealth_engine.get_stealth_stats().await;
    let dpi_stats = &final_stats.dpi_bypass_stats;

    // Should have enabled advanced features under high detection risk
    if dpi_stats.detection_risk == DetectionRisk::High {
        assert!(dpi_stats.packet_fragmentation_enabled);
        assert!(dpi_stats.header_obfuscation_enabled);
        assert!(dpi_stats.dns_pattern_replication_enabled);
    }

    stealth_engine.stop().await.expect("Failed to stop stealth engine");
}

/// Test packet fragmentation effectiveness
#[tokio::test]
async fn test_packet_fragmentation_patterns() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Maximum);

    let test_server = create_test_server("test", "test.speedtest.net", "Test");

    // Test different payload sizes with fragmentation
    let payload_sizes = vec![500, 1000, 1500, 2000];
    
    for size in payload_sizes {
        let payload = stealth_engine.generate_speedtest_payload(size);
        assert!(payload.len() <= size);
        
        // Verify payload structure
        assert!(payload.starts_with("content1="));
        
        // Test that payload contains varied data (not all same character)
        let data_part = &payload[9..]; // Skip "content1=" prefix
        let unique_chars: std::collections::HashSet<char> = data_part.chars().collect();
        assert!(unique_chars.len() > 10, "Payload should contain varied characters");
    }
}

/// Test header obfuscation patterns
#[tokio::test]
async fn test_header_obfuscation_variety() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);

    let mut user_agents = std::collections::HashSet::new();
    let mut accept_encodings = std::collections::HashSet::new();
    let mut connections = std::collections::HashSet::new();

    // Generate multiple header sets to test randomization
    for _ in 0..20 {
        let headers = stealth_engine.create_obfuscated_headers().await;
        
        if let Some(ua) = headers.get("user-agent") {
            user_agents.insert(ua.to_str().unwrap().to_string());
        }
        
        if let Some(ae) = headers.get("accept-encoding") {
            accept_encodings.insert(ae.to_str().unwrap().to_string());
        }
        
        if let Some(conn) = headers.get("connection") {
            connections.insert(conn.to_str().unwrap().to_string());
        }
    }

    // Should have multiple variations due to randomization
    assert!(user_agents.len() > 1, "Should have multiple User-Agent variations");
    assert!(accept_encodings.len() > 1, "Should have multiple Accept-Encoding variations");
    assert!(connections.len() > 1, "Should have multiple Connection variations");

    println!("User-Agent variations: {}", user_agents.len());
    println!("Accept-Encoding variations: {}", accept_encodings.len());
    println!("Connection variations: {}", connections.len());
}

/// Test DNS pattern replication timing
#[tokio::test]
async fn test_dns_pattern_timing() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);

    let test_server = create_test_server("dns_test", "speedtest.example.com", "Test");

    // Measure DNS pattern replication timing
    let start_time = Instant::now();
    stealth_engine.replicate_dns_patterns(&test_server).await.expect("DNS pattern replication failed");
    let elapsed = start_time.elapsed();

    // Should take reasonable time for multiple DNS lookups
    assert!(elapsed >= Duration::from_millis(40), "DNS replication should take time for multiple queries");
    assert!(elapsed <= Duration::from_secs(2), "DNS replication should not take too long");

    println!("DNS pattern replication took: {:?}", elapsed);
}

/// Test adaptive stealth under simulated DPI detection
#[tokio::test]
async fn test_adaptive_stealth_under_detection() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Medium);

    // Initial state should be low risk
    assert_eq!(stealth_engine.assess_detection_risk().await, DetectionRisk::Low);

    // Simulate gradual detection increase
    let detection_scenarios = vec![
        (2, DetectionRisk::Low),      // 2 failures -> still low
        (3, DetectionRisk::Medium),   // 3 failures -> medium
        (6, DetectionRisk::High),     // 6 failures -> high
        (12, DetectionRisk::Critical), // 12 failures -> critical
    ];

    for (failure_count, expected_risk) in detection_scenarios {
        // Reset and simulate failures
        for _ in 0..failure_count {
            stealth_engine.record_connection_result(false, None).await;
        }

        let current_risk = stealth_engine.assess_detection_risk().await;
        assert_eq!(current_risk, expected_risk, "Risk assessment failed for {} failures", failure_count);

        // Test adaptation
        stealth_engine.adapt_stealth_strategy().await.expect("Failed to adapt strategy");

        let stats = stealth_engine.get_dpi_bypass_stats().await;
        assert!(stats.adaptation_count > 0, "Should have adapted strategy");

        println!("After {} failures: Risk={:?}, Adaptations={}", 
                failure_count, current_risk, stats.adaptation_count);

        // Reset for next scenario
        // Note: In a real scenario, we'd need a way to reset the adaptive state
        // For testing, we'll continue with accumulated state
    }
}

/// Test effectiveness measurement and learning
#[tokio::test]
async fn test_effectiveness_learning() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);

    // Initial effectiveness should be 1.0
    let initial_stats = stealth_engine.get_dpi_bypass_stats().await;
    assert_eq!(initial_stats.effectiveness_score, 1.0);

    // Simulate mixed results with effectiveness feedback
    let test_scenarios = vec![
        (true, Some(0.9)),  // High effectiveness success
        (true, Some(0.7)),  // Medium effectiveness success
        (false, None),      // Failure
        (true, Some(0.8)),  // Good effectiveness success
        (false, None),      // Another failure
        (true, Some(0.6)),  // Lower effectiveness success
    ];

    for (success, effectiveness) in test_scenarios {
        stealth_engine.record_connection_result(success, effectiveness).await;
        
        let stats = stealth_engine.get_dpi_bypass_stats().await;
        println!("Success: {}, Effectiveness: {:?}, Score: {:.3}, Failures: {}", 
                success, effectiveness, stats.effectiveness_score, stats.consecutive_failures);
    }

    let final_stats = stealth_engine.get_dpi_bypass_stats().await;
    
    // Effectiveness should be updated based on feedback
    assert!(final_stats.effectiveness_score < 1.0, "Effectiveness should decrease with mixed results");
    assert!(final_stats.effectiveness_score > 0.0, "Effectiveness should not go to zero");
    
    // Should have some failures recorded
    assert!(final_stats.consecutive_failures > 0, "Should have recorded consecutive failures");
}

/// Test raw HTTP request generation for maximum stealth
#[tokio::test]
async fn test_raw_http_request_generation() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::Maximum);

    let test_server = create_test_server("raw_test", "raw.speedtest.net", "Test");

    let request_data = stealth_engine.create_raw_http_request(&test_server).await
        .expect("Failed to create raw HTTP request");

    let request_str = String::from_utf8(request_data).expect("Invalid UTF-8 in request");

    // Validate HTTP request structure
    assert!(request_str.starts_with("GET /speedtest/latency.txt"), "Should start with GET request");
    assert!(request_str.contains("HTTP/1.1"), "Should specify HTTP/1.1");
    assert!(request_str.contains("Host: raw.speedtest.net:8080"), "Should include correct Host header");
    assert!(request_str.contains("User-Agent:"), "Should include User-Agent header");
    assert!(request_str.ends_with("\r\n\r\n"), "Should end with proper HTTP termination");

    // Should include query parameter for cache busting
    assert!(request_str.contains("?r="), "Should include random query parameter");

    println!("Generated raw HTTP request:\n{}", request_str);
}

/// Test complete DPI bypass workflow integration
#[tokio::test]
async fn test_complete_dpi_bypass_workflow() {
    let server_pool = Arc::new(ServerPool::new().expect("Failed to create server pool"));
    let stealth_engine = StealthEngine::new(server_pool, StealthLevel::High);

    // Add test servers
    let test_servers = vec![
        create_test_server("workflow1", "w1.speedtest.net", "Singapore"),
        create_test_server("workflow2", "w2.speedtest.net", "India"),
    ];

    let mut pool = Arc::into_inner(stealth_engine.server_pool.clone())
        .unwrap_or_else(|| ServerPool::new().expect("Failed to create pool"));
    pool.set_servers(test_servers);
    let pool = Arc::new(pool);
    let stealth_engine = StealthEngine::new(pool, StealthLevel::High);

    // Start stealth engine
    stealth_engine.start().await.expect("Failed to start stealth engine");

    // Run complete workflow multiple times
    for iteration in 1..=5 {
        println!("DPI bypass workflow iteration {}", iteration);

        // Generate traffic with full DPI bypass
        let traffic_result = stealth_engine.generate_mimicry_traffic().await;
        
        // Record result and adapt
        match traffic_result {
            Ok(_) => {
                stealth_engine.record_connection_result(true, Some(0.75)).await;
                println!("  Traffic generation successful");
            }
            Err(e) => {
                stealth_engine.record_connection_result(false, None).await;
                println!("  Traffic generation failed: {}", e);
            }
        }

        // Adapt strategy based on results
        stealth_engine.adapt_stealth_strategy().await.expect("Failed to adapt strategy");

        // Check current state
        let stats = stealth_engine.get_stealth_stats().await;
        let dpi_stats = &stats.dpi_bypass_stats;

        println!("  Detection risk: {:?}", dpi_stats.detection_risk);
        println!("  Effectiveness: {:.3}", dpi_stats.effectiveness_score);
        println!("  Adaptations: {}", dpi_stats.adaptation_count);

        // Rotate servers if needed
        if stealth_engine.should_rotate_servers().await {
            stealth_engine.rotate_servers().await.expect("Failed to rotate servers");
            println!("  Rotated servers");
        }

        // Small delay between iterations
        sleep(Duration::from_millis(50)).await;
    }

    // Final verification
    let final_stats = stealth_engine.get_stealth_stats().await;
    let final_dpi_stats = &final_stats.dpi_bypass_stats;

    // Should have some activity recorded
    assert!(final_dpi_stats.adaptation_count > 0, "Should have performed adaptations");
    
    // DPI bypass features should be enabled for high stealth level
    assert!(final_dpi_stats.packet_fragmentation_enabled, "Packet fragmentation should be enabled");
    assert!(final_dpi_stats.header_obfuscation_enabled, "Header obfuscation should be enabled");
    assert!(final_dpi_stats.dns_pattern_replication_enabled, "DNS pattern replication should be enabled");

    stealth_engine.stop().await.expect("Failed to stop stealth engine");
    println!("DPI bypass workflow test completed successfully");
}

// Helper function to create test servers
fn create_test_server(id: &str, host: &str, country: &str) -> SpeedtestServer {
    SpeedtestServer::new(
        id.to_string(),
        host.to_string(),
        8080,
        format!("{} Test Server", country),
        country.to_string(),
        "Test Sponsor".to_string(),
    )
}