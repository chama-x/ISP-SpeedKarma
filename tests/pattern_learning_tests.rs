use isp_speedkarma::core::intelligence::*;
use isp_speedkarma::data::models::*;
use isp_speedkarma::data::repository::Repository;
use isp_speedkarma::data::migrations::MigrationManager;
use chrono::{DateTime, Utc, Duration, Weekday};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Setup test database with sample data
async fn setup_test_db_with_data() -> (Arc<Repository>, DefaultIntelligenceCore) {
    let database_url = ":memory:";
    let pool = SqlitePool::connect(database_url).await.unwrap();
    
    let migration_manager = MigrationManager::new(database_url.to_string());
    migration_manager.run_migrations(&pool).await.unwrap();
    
    let repository = Arc::new(Repository::new(pool));
    let intelligence = DefaultIntelligenceCore::new(Arc::clone(&repository));
    
    // Add sample speed measurements for testing
    let base_time = Utc::now() - Duration::days(14);
    
    // Add baseline measurements (no optimization)
    for day in 0..14 {
        for hour in 0..24 {
            let timestamp = base_time + Duration::days(day) + Duration::hours(hour);
            
            // Simulate throttling during peak hours (19:00-22:00)
            let is_peak_hour = hour >= 19 && hour <= 22;
            let base_speed = if is_peak_hour { 25.0 } else { 50.0 };
            
            let measurement = SpeedMeasurement {
                id: None,
                timestamp,
                download_mbps: base_speed + (hour as f64 * 0.5), // Add some variation
                upload_mbps: base_speed * 0.2,
                latency_ms: 30 + (hour as u32 * 2),
                optimization_active: false,
                confidence: 0.8,
            };
            
            repository.save_speed_measurement(&measurement).await.unwrap();
        }
    }
    
    // Add optimized measurements for comparison
    for day in 7..14 {
        for hour in [19, 20, 21, 22] { // Only during peak hours
            let timestamp = base_time + Duration::days(day) + Duration::hours(hour);
            
            let measurement = SpeedMeasurement {
                id: None,
                timestamp,
                download_mbps: 75.0 + (hour as f64 * 0.5), // Better speed with optimization
                upload_mbps: 15.0,
                latency_ms: 25,
                optimization_active: true,
                confidence: 0.9,
            };
            
            repository.save_speed_measurement(&measurement).await.unwrap();
        }
    }
    
    (repository, intelligence)
}

#[tokio::test]
async fn test_pattern_learning_model_initialization() {
    let model = PatternLearningModel::default();
    
    assert_eq!(model.strategy_effectiveness.len(), 0);
    assert_eq!(model.temporal_weights.len(), 0);
    assert_eq!(model.weekly_weights.len(), 0);
    assert_eq!(model.model_confidence, 0.0);
    assert_eq!(model.training_samples, 0);
}

#[tokio::test]
async fn test_strategy_effectiveness_calculation() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    let strategy = OptimizationStrategy::default_strategy();
    let effectiveness = intelligence.calculate_strategy_effectiveness(&strategy).await.unwrap();
    
    // Should detect improvement from optimization
    assert!(effectiveness.avg_improvement > 1.0, "Expected improvement > 1.0, got {}", effectiveness.avg_improvement);
    assert!(effectiveness.confidence > 0.0, "Expected confidence > 0.0, got {}", effectiveness.confidence);
    assert!(effectiveness.sample_count > 0, "Expected samples > 0, got {}", effectiveness.sample_count);
}

#[tokio::test]
async fn test_temporal_pattern_learning() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model with sample data
    intelligence.train_model().await.unwrap();
    
    // Check that temporal patterns were learned
    assert!(!intelligence.learning_model.temporal_weights.is_empty(), "Temporal weights should not be empty");
    
    // Peak hours (19-22) should have lower performance scores
    let peak_hour_weights: Vec<f64> = (19..=22)
        .filter_map(|hour| intelligence.learning_model.temporal_weights.get(&hour))
        .copied()
        .collect();
    
    let non_peak_hour_weights: Vec<f64> = (9..=17)
        .filter_map(|hour| intelligence.learning_model.temporal_weights.get(&hour))
        .copied()
        .collect();
    
    if !peak_hour_weights.is_empty() && !non_peak_hour_weights.is_empty() {
        let avg_peak = peak_hour_weights.iter().sum::<f64>() / peak_hour_weights.len() as f64;
        let avg_non_peak = non_peak_hour_weights.iter().sum::<f64>() / non_peak_hour_weights.len() as f64;
        
        assert!(avg_peak < avg_non_peak, "Peak hours should have lower performance than non-peak hours");
    }
}

#[tokio::test]
async fn test_model_confidence_calculation() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Model confidence should be calculated
    assert!(intelligence.learning_model.model_confidence > 0.0, "Model confidence should be > 0.0");
    assert!(intelligence.learning_model.model_confidence <= 1.0, "Model confidence should be <= 1.0");
    
    // Training samples should be recorded
    assert!(intelligence.learning_model.training_samples > 0, "Training samples should be > 0");
}

#[tokio::test]
async fn test_pattern_analysis() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model first
    intelligence.train_model().await.unwrap();
    
    // Analyze patterns
    let analysis = intelligence.analyze_patterns().await.unwrap();
    
    assert!(analysis.baseline_speed > 0.0, "Baseline speed should be > 0.0");
    assert!(analysis.data_collection_days > 0, "Data collection days should be > 0");
    
    // Should detect some throttling periods during peak hours
    if analysis.confidence_level > 0.5 {
        assert!(!analysis.throttling_periods.is_empty(), "Should detect throttling periods with sufficient confidence");
    }
}

#[tokio::test]
async fn test_optimization_decision_logic() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Get optimization decision
    let decision = intelligence.should_optimize().await.unwrap();
    
    assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0, "Confidence should be between 0.0 and 1.0");
    assert!(!decision.reason.is_empty(), "Decision should have a reason");
    
    // If optimization is recommended, there should be an estimated improvement
    if decision.should_activate {
        assert!(decision.estimated_improvement.is_some(), "Should have estimated improvement when optimization is recommended");
        assert!(decision.estimated_improvement.unwrap() > 1.0, "Estimated improvement should be > 1.0");
    }
}

#[tokio::test]
async fn test_system_status_generation() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Get system status
    let status = intelligence.get_status().await.unwrap();
    
    match status.state {
        SystemState::Learning => {
            assert!(status.data_collection_progress.is_some(), "Learning state should have progress data");
            let progress = status.data_collection_progress.unwrap();
            assert!(progress.days_collected < progress.days_needed, "Should need more days when learning");
        }
        SystemState::Optimizing => {
            assert!(status.effectiveness.is_some(), "Optimizing state should have effectiveness metrics");
            let effectiveness = status.effectiveness.unwrap();
            assert!(effectiveness.improvement_factor > 1.0, "Should show improvement when optimizing");
        }
        SystemState::Monitoring => {
            // Monitoring state is valid
        }
        _ => {
            // Other states are also valid
        }
    }
    
    assert!(!status.message.is_empty(), "Status should have a message");
}

#[tokio::test]
async fn test_recommendation_generation() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Generate recommendations
    let recommendations = intelligence.generate_recommendations().await.unwrap();
    
    // Should generate at least one recommendation with sufficient data
    if intelligence.learning_model.model_confidence > 0.5 {
        assert!(!recommendations.is_empty(), "Should generate recommendations with sufficient confidence");
        
        for recommendation in &recommendations {
            assert!(!recommendation.description.is_empty(), "Recommendation should have description");
            assert!(recommendation.confidence >= 0.0 && recommendation.confidence <= 1.0, "Confidence should be valid");
            assert!(recommendation.expected_improvement > 0.0, "Expected improvement should be positive");
            assert!(recommendation.priority >= 1 && recommendation.priority <= 5, "Priority should be between 1-5");
        }
    }
}

#[tokio::test]
async fn test_effectiveness_analysis_accuracy() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Test strategy effectiveness calculation
    let default_strategy = OptimizationStrategy::default_strategy();
    let effectiveness = intelligence.calculate_strategy_effectiveness(&default_strategy).await.unwrap();
    
    // With our test data, optimization should show improvement during peak hours
    if effectiveness.sample_count > 5 {
        assert!(effectiveness.avg_improvement > 1.0, "Should detect improvement from optimization");
        assert!(effectiveness.success_rate >= 0.0, "Success rate should be non-negative");
        assert!(effectiveness.confidence > 0.0, "Should have confidence in effectiveness measurement");
    }
}

#[tokio::test]
async fn test_isp_parameter_learning() {
    let (repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Add an ISP profile
    let isp_profile = ISPProfile::new(
        "Hutch".to_string(),
        "Sri Lanka".to_string(),
        "Test Detection".to_string(),
    );
    repository.save_isp_profile(&isp_profile).await.unwrap();
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Check ISP parameters were learned
    assert!(!intelligence.learning_model.isp_parameters.is_empty(), "Should learn ISP parameters");
    
    if let Some(params) = intelligence.learning_model.isp_parameters.get("Hutch") {
        // Hutch is a known throttling ISP, so should use high stealth
        assert_eq!(params.optimal_stealth_level, StealthLevel::High, "Known throttling ISP should use high stealth");
        assert!(params.detection_risk > 0.5, "Known throttling ISP should have high detection risk");
        assert!(params.confidence > 0.0, "Should have confidence in ISP parameters");
    }
}

#[tokio::test]
async fn test_learning_model_persistence() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    let initial_confidence = intelligence.learning_model.model_confidence;
    let initial_samples = intelligence.learning_model.training_samples;
    
    // Simulate model adaptation
    intelligence.adapt_strategy(2.5).await.unwrap();
    
    // Model should maintain its learned state
    assert_eq!(intelligence.learning_model.model_confidence, initial_confidence, "Model confidence should persist");
    assert_eq!(intelligence.learning_model.training_samples, initial_samples, "Training samples should persist");
}

#[tokio::test]
async fn test_confidence_scoring_accuracy() {
    let (_repository, intelligence) = setup_test_db_with_data().await;
    
    // Test confidence calculation with different scenarios
    let high_confidence = intelligence.calculate_effectiveness_confidence(100, 100, 2.0);
    let medium_confidence = intelligence.calculate_effectiveness_confidence(50, 50, 1.5);
    let low_confidence = intelligence.calculate_effectiveness_confidence(10, 10, 1.1);
    
    assert!(high_confidence > medium_confidence, "High sample count and improvement should have higher confidence");
    assert!(medium_confidence > low_confidence, "Medium scenario should have higher confidence than low");
    
    // All confidence scores should be valid
    assert!(high_confidence >= 0.0 && high_confidence <= 1.0, "High confidence should be valid");
    assert!(medium_confidence >= 0.0 && medium_confidence <= 1.0, "Medium confidence should be valid");
    assert!(low_confidence >= 0.0 && low_confidence <= 1.0, "Low confidence should be valid");
}

#[tokio::test]
async fn test_recommendation_prioritization() {
    let (_repository, mut intelligence) = setup_test_db_with_data().await;
    
    // Add ISP profile to trigger ISP-specific recommendations
    let repository = &intelligence.repository;
    let isp_profile = ISPProfile::new(
        "Hutch".to_string(),
        "Sri Lanka".to_string(),
        "Test Detection".to_string(),
    );
    repository.save_isp_profile(&isp_profile).await.unwrap();
    
    // Train the model
    intelligence.train_model().await.unwrap();
    
    // Generate recommendations
    let recommendations = intelligence.generate_recommendations().await.unwrap();
    
    if !recommendations.is_empty() {
        // Check that recommendations are properly prioritized
        let priorities: Vec<u8> = recommendations.iter().map(|r| r.priority).collect();
        let has_high_priority = priorities.iter().any(|&p| p <= 2);
        
        if has_high_priority {
            // High priority recommendations should have good expected improvements
            let high_priority_recs: Vec<_> = recommendations.iter()
                .filter(|r| r.priority <= 2)
                .collect();
            
            for rec in high_priority_recs {
                assert!(rec.expected_improvement > 1.0, "High priority recommendations should have meaningful improvements");
                assert!(rec.confidence > 0.5, "High priority recommendations should have reasonable confidence");
            }
        }
    }
}