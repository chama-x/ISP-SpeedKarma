use isp_speedkarma::core::intelligence::*;
use isp_speedkarma::data::models::*;
use isp_speedkarma::data::repository::Repository;
use isp_speedkarma::data::migrations::MigrationManager;
use isp_speedkarma::network::monitor::BackgroundMonitor;
use chrono::{DateTime, Utc, Duration, Weekday, Datelike};
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::time::{sleep, Duration as TokioDuration};

/// Setup comprehensive test environment with realistic data patterns
async fn setup_comprehensive_test_environment() -> (Arc<Repository>, DefaultIntelligenceCore, BackgroundMonitor) {
    let database_url = ":memory:";
    let pool = SqlitePool::connect(database_url).await.unwrap();
    
    let migration_manager = MigrationManager::new(database_url.to_string());
    migration_manager.run_migrations(&pool).await.unwrap();
    
    let repository = Arc::new(Repository::new(pool));
    let intelligence = DefaultIntelligenceCore::new(Arc::clone(&repository));
    let monitor = BackgroundMonitor::new(Arc::clone(&repository));
    
    // Create realistic ISP profile
    let isp_profile = ISPProfile::new(
        "Hutch".to_string(),
        "Sri Lanka".to_string(),
        "DNS Analysis + Public IP".to_string(),
    );
    let isp_id = repository.save_isp_profile(&isp_profile).await.unwrap();
    
    // Add realistic throttling patterns
    let evening_pattern = ThrottlingPattern::new(
        isp_id,
        19, 0,  // 7 PM
        22, 0,  // 10 PM
        vec![Weekday::Mon, Weekday::Tue, Weekday::Wed, Weekday::Thu, Weekday::Fri],
        0.8,    // High severity
    );
    repository.save_throttling_pattern(&evening_pattern).await.unwrap();
    
    let weekend_pattern = ThrottlingPattern::new(
        isp_id,
        14, 0,  // 2 PM
        23, 0,  // 11 PM
        vec![Weekday::Sat, Weekday::Sun],
        0.6,    // Medium severity
    );
    repository.save_throttling_pattern(&weekend_pattern).await.unwrap();
    
    // Generate comprehensive measurement data over 30 days
    let base_time = Utc::now() - Duration::days(30);
    
    for day in 0..30 {
        let current_date = base_time + Duration::days(day);
        let weekday = current_date.weekday();
        
        for hour in 0..24 {
            let timestamp = current_date + Duration::hours(hour);
            
            // Determine if this is a throttling period
            let is_throttled = match weekday {
                Weekday::Sat | Weekday::Sun => hour >= 14 && hour <= 23,
                _ => hour >= 19 && hour <= 22,
            };
            
            // Base speeds with realistic variation
            let base_speed = if is_throttled { 
                20.0 + (hour as f64 * 0.3) // Throttled speed
            } else { 
                60.0 + (hour as f64 * 0.5) // Normal speed
            };
            
            // Add some random variation (±20%)
            let variation = (day as f64 % 7.0) * 0.1 - 0.3;
            let actual_speed = base_speed * (1.0 + variation);
            
            // Baseline measurement (no optimization)
            let baseline_measurement = SpeedMeasurement {
                id: None,
                timestamp,
                download_mbps: actual_speed.max(5.0), // Minimum 5 Mbps
                upload_mbps: actual_speed * 0.15,
                latency_ms: if is_throttled { 80 + (hour as u32 * 2) } else { 30 + (hour as u32) },
                optimization_active: false,
                confidence: 0.8 + (day as f64 % 10.0) * 0.02,
            };
            repository.save_speed_measurement(&baseline_measurement).await.unwrap();
            
            // Add optimized measurements for the last 15 days during throttling periods
            if day >= 15 && is_throttled {
                let optimized_speed = actual_speed * 2.2; // Significant improvement
                let optimized_measurement = SpeedMeasurement {
                    id: None,
                    timestamp: timestamp + Duration::minutes(30), // Slightly offset
                    download_mbps: optimized_speed.min(150.0), // Cap at 150 Mbps
                    upload_mbps: optimized_speed * 0.2,
                    latency_ms: 35 + (hour as u32),
                    optimization_active: true,
                    confidence: 0.9,
                };
                repository.save_speed_measurement(&optimized_measurement).await.unwrap();
            }
        }
    }
    
    // Add multiple optimization strategies with different effectiveness
    let strategies = vec![
        OptimizationStrategy {
            id: None,
            name: "Conservative".to_string(),
            server_rotation_interval_minutes: 15,
            packet_timing_min_seconds: 45.0,
            packet_timing_max_seconds: 90.0,
            connection_count: 2,
            traffic_intensity: 0.3,
            stealth_level: StealthLevel::High,
            effectiveness_score: Some(0.7),
            created_at: Utc::now(),
        },
        OptimizationStrategy {
            id: None,
            name: "Aggressive".to_string(),
            server_rotation_interval_minutes: 5,
            packet_timing_min_seconds: 20.0,
            packet_timing_max_seconds: 40.0,
            connection_count: 5,
            traffic_intensity: 0.8,
            stealth_level: StealthLevel::Medium,
            effectiveness_score: Some(0.9),
            created_at: Utc::now(),
        },
        OptimizationStrategy {
            id: None,
            name: "Balanced".to_string(),
            server_rotation_interval_minutes: 10,
            packet_timing_min_seconds: 30.0,
            packet_timing_max_seconds: 60.0,
            connection_count: 3,
            traffic_intensity: 0.5,
            stealth_level: StealthLevel::Medium,
            effectiveness_score: Some(0.85),
            created_at: Utc::now(),
        },
    ];
    
    for strategy in strategies {
        repository.save_optimization_strategy(&strategy).await.unwrap();
    }
    
    (repository, intelligence, monitor)
}

#[tokio::test]
async fn test_comprehensive_effectiveness_analysis() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model with comprehensive data
    intelligence.train_model().await.unwrap();
    
    // Analyze patterns
    let analysis = intelligence.analyze_patterns().await.unwrap();
    
    // Should detect significant patterns with 30 days of data
    assert!(analysis.confidence_level > 0.7, "Should have high confidence with 30 days of data");
    assert!(analysis.baseline_speed > 0.0, "Should calculate baseline speed");
    assert!(analysis.data_collection_days >= 25, "Should recognize substantial data collection period");
    
    // Should detect throttling periods
    assert!(!analysis.throttling_periods.is_empty(), "Should detect throttling periods");
    
    // Verify ISP detection
    assert_eq!(analysis.isp_profile, Some("Hutch".to_string()), "Should identify ISP correctly");
}

#[tokio::test]
async fn test_strategy_effectiveness_comparison() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Test different strategies
    let conservative = OptimizationStrategy {
        id: None,
        name: "Conservative".to_string(),
        server_rotation_interval_minutes: 15,
        packet_timing_min_seconds: 45.0,
        packet_timing_max_seconds: 90.0,
        connection_count: 2,
        traffic_intensity: 0.3,
        stealth_level: StealthLevel::High,
        effectiveness_score: None,
        created_at: Utc::now(),
    };
    
    let aggressive = OptimizationStrategy {
        id: None,
        name: "Aggressive".to_string(),
        server_rotation_interval_minutes: 5,
        packet_timing_min_seconds: 20.0,
        packet_timing_max_seconds: 40.0,
        connection_count: 5,
        traffic_intensity: 0.8,
        stealth_level: StealthLevel::Medium,
        effectiveness_score: None,
        created_at: Utc::now(),
    };
    
    let conservative_effectiveness = intelligence.calculate_strategy_effectiveness(&conservative).await.unwrap();
    let aggressive_effectiveness = intelligence.calculate_strategy_effectiveness(&aggressive).await.unwrap();
    
    // Both should show improvement over baseline
    assert!(conservative_effectiveness.avg_improvement > 1.0, "Conservative strategy should show improvement");
    assert!(aggressive_effectiveness.avg_improvement > 1.0, "Aggressive strategy should show improvement");
    
    // Should have reasonable confidence levels
    assert!(conservative_effectiveness.confidence > 0.5, "Should have confidence in conservative strategy");
    assert!(aggressive_effectiveness.confidence > 0.5, "Should have confidence in aggressive strategy");
    
    // Should have sample data
    assert!(conservative_effectiveness.sample_count > 0, "Should have samples for conservative strategy");
    assert!(aggressive_effectiveness.sample_count > 0, "Should have samples for aggressive strategy");
}

#[tokio::test]
async fn test_temporal_pattern_accuracy() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Check temporal patterns
    let temporal_weights = &intelligence.learning_model.temporal_weights;
    
    // Should have learned patterns for most hours
    assert!(temporal_weights.len() >= 20, "Should learn patterns for most hours of the day");
    
    // Peak throttling hours should have lower weights
    let evening_weights: Vec<f64> = (19..=22)
        .filter_map(|hour| temporal_weights.get(&hour))
        .copied()
        .collect();
    
    let morning_weights: Vec<f64> = (8..=11)
        .filter_map(|hour| temporal_weights.get(&hour))
        .copied()
        .collect();
    
    if !evening_weights.is_empty() && !morning_weights.is_empty() {
        let avg_evening = evening_weights.iter().sum::<f64>() / evening_weights.len() as f64;
        let avg_morning = morning_weights.iter().sum::<f64>() / morning_weights.len() as f64;
        
        assert!(avg_evening < avg_morning, 
                "Evening hours (throttled) should have lower performance than morning hours");
    }
}

#[tokio::test]
async fn test_optimization_decision_accuracy() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Get optimization decision
    let decision = intelligence.should_optimize().await.unwrap();
    
    // With clear throttling patterns and sufficient data, should recommend optimization
    assert!(decision.should_activate, "Should recommend optimization with clear throttling patterns");
    assert!(decision.confidence > 0.7, "Should have high confidence in decision");
    assert!(decision.estimated_improvement.is_some(), "Should provide estimated improvement");
    
    let improvement = decision.estimated_improvement.unwrap();
    assert!(improvement > 1.5, "Should estimate significant improvement (>1.5x)");
    
    assert!(!decision.reason.is_empty(), "Should provide clear reason for decision");
}

#[tokio::test]
async fn test_recommendation_quality() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Generate recommendations
    let recommendations = intelligence.generate_recommendations().await.unwrap();
    
    // Should generate meaningful recommendations
    assert!(!recommendations.is_empty(), "Should generate recommendations with sufficient data");
    
    // Check recommendation quality
    for recommendation in &recommendations {
        assert!(!recommendation.description.is_empty(), "Each recommendation should have description");
        assert!(recommendation.confidence > 0.5, "Recommendations should have reasonable confidence");
        assert!(recommendation.expected_improvement > 1.0, "Should expect positive improvement");
        assert!(recommendation.priority >= 1 && recommendation.priority <= 5, "Priority should be valid");
        
        // High priority recommendations should have high confidence
        if recommendation.priority <= 2 {
            assert!(recommendation.confidence > 0.7, "High priority recommendations should have high confidence");
        }
    }
    
    // Should include timing optimization for detected throttling periods
    let has_timing_rec = recommendations.iter()
        .any(|r| matches!(r.recommendation_type, RecommendationType::TimingOptimization));
    assert!(has_timing_rec, "Should recommend timing optimization for detected throttling");
}

#[tokio::test]
async fn test_isp_specific_learning() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Check ISP-specific parameters
    let isp_params = intelligence.learning_model.isp_parameters.get("Hutch");
    assert!(isp_params.is_some(), "Should learn parameters for Hutch ISP");
    
    let params = isp_params.unwrap();
    
    // Hutch is known for aggressive throttling, so should use high stealth
    assert_eq!(params.optimal_stealth_level, StealthLevel::High, "Should recommend high stealth for Hutch");
    assert!(params.detection_risk > 0.5, "Should recognize high detection risk for Hutch");
    assert!(params.confidence > 0.5, "Should have confidence in ISP parameters");
    
    // Should have reasonable optimization parameters
    assert!(params.optimal_rotation_interval > 0, "Should have positive rotation interval");
    assert!(params.optimal_traffic_intensity > 0.0 && params.optimal_traffic_intensity <= 1.0, 
            "Traffic intensity should be valid");
}

#[tokio::test]
async fn test_model_confidence_progression() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Initial confidence should be low
    let initial_confidence = intelligence.learning_model.model_confidence;
    assert_eq!(initial_confidence, 0.0, "Initial model confidence should be 0.0");
    
    // Train with comprehensive data
    intelligence.train_model().await.unwrap();
    
    // Confidence should increase significantly
    let trained_confidence = intelligence.learning_model.model_confidence;
    assert!(trained_confidence > 0.7, "Model confidence should be high after training with comprehensive data");
    assert!(trained_confidence <= 1.0, "Model confidence should not exceed 1.0");
    
    // Training samples should be recorded
    assert!(intelligence.learning_model.training_samples > 500, "Should have substantial training samples");
}

#[tokio::test]
async fn test_effectiveness_measurement_accuracy() {
    let (repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Get speed statistics to verify our test data
    let stats = repository.get_speed_statistics(30).await.unwrap();
    
    // Verify test data setup
    assert!(stats.total_measurements > 1000, "Should have substantial measurement data");
    assert!(stats.avg_baseline_download_mbps.is_some(), "Should have baseline measurements");
    assert!(stats.avg_optimized_download_mbps.is_some(), "Should have optimized measurements");
    
    // Calculate improvement factor
    let baseline = stats.avg_baseline_download_mbps.unwrap();
    let optimized = stats.avg_optimized_download_mbps.unwrap();
    let improvement = optimized / baseline;
    
    // Should show significant improvement (our test data has 2.2x improvement)
    assert!(improvement > 1.8, "Should detect significant improvement in test data");
    assert!(improvement < 3.0, "Improvement should be realistic");
    
    // Test strategy effectiveness calculation
    let default_strategy = OptimizationStrategy::default_strategy();
    let effectiveness = intelligence.calculate_strategy_effectiveness(&default_strategy).await.unwrap();
    
    // Should match the improvement we see in raw data
    assert!((effectiveness.avg_improvement - improvement).abs() < 0.5, 
            "Strategy effectiveness should match raw data improvement");
}

#[tokio::test]
async fn test_learning_model_serialization() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Test that the model can be serialized (important for persistence)
    let model = &intelligence.learning_model;
    let serialized = serde_json::to_string(model);
    assert!(serialized.is_ok(), "Learning model should be serializable");
    
    // Test deserialization
    let serialized_data = serialized.unwrap();
    let deserialized: Result<PatternLearningModel, _> = serde_json::from_str(&serialized_data);
    assert!(deserialized.is_ok(), "Learning model should be deserializable");
    
    let restored_model = deserialized.unwrap();
    assert_eq!(restored_model.model_confidence, model.model_confidence, "Model confidence should be preserved");
    assert_eq!(restored_model.training_samples, model.training_samples, "Training samples should be preserved");
}

#[tokio::test]
async fn test_real_time_adaptation() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    let initial_confidence = intelligence.learning_model.model_confidence;
    
    // Simulate real-time effectiveness feedback
    intelligence.adapt_strategy(2.5).await.unwrap(); // Good effectiveness
    intelligence.adapt_strategy(1.8).await.unwrap(); // Moderate effectiveness
    intelligence.adapt_strategy(3.2).await.unwrap(); // Excellent effectiveness
    
    // Model should maintain stability (adaptation is logged but doesn't change core model yet)
    assert_eq!(intelligence.learning_model.model_confidence, initial_confidence, 
               "Model should maintain stability during adaptation");
}

#[tokio::test]
async fn test_system_status_accuracy() {
    let (_repository, mut intelligence, _monitor) = setup_comprehensive_test_environment().await;
    
    // Before training - should be in learning mode
    let initial_status = intelligence.get_status().await.unwrap();
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // After training - should show optimizing or monitoring status
    let trained_status = intelligence.get_status().await.unwrap();
    
    match trained_status.state {
        SystemState::Optimizing => {
            assert!(trained_status.effectiveness.is_some(), "Optimizing state should have effectiveness metrics");
            let effectiveness = trained_status.effectiveness.unwrap();
            assert!(effectiveness.improvement_factor > 1.5, "Should show significant improvement factor");
            assert!(effectiveness.confidence > 0.7, "Should have high confidence in effectiveness");
        }
        SystemState::Monitoring => {
            // Monitoring state is also valid with sufficient data
        }
        _ => {
            panic!("With comprehensive data, should be in Optimizing or Monitoring state");
        }
    }
    
    assert!(!trained_status.message.is_empty(), "Status should have informative message");
}