use crate::core::error::Result;
use crate::data::models::{SpeedMeasurement, OptimizationStrategy, ThrottlingPattern, StealthLevel};
use crate::data::repository::Repository;
use chrono::{DateTime, Utc, Duration, Weekday, Timelike, Datelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Core intelligence engine interface - the heart of SpeedKarma's decision making
/// Following Apple's approach to AI: powerful but invisible
pub trait IntelligenceCore {
    /// Analyzes network patterns to identify throttling periods and ISP behavior
    async fn analyze_patterns(&self) -> Result<PatternAnalysis>;
    
    /// Determines if optimization should be active based on current conditions
    async fn should_optimize(&self) -> Result<OptimizationDecision>;
    
    /// Adapts strategy based on effectiveness feedback
    async fn adapt_strategy(&self, effectiveness: f64) -> Result<()>;
    
    /// Gets current system status for UI display
    async fn get_status(&self) -> Result<SystemStatus>;
}

/// Analysis results from pattern recognition and machine learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternAnalysis {
    /// Identified periods when ISP throttling occurs
    pub throttling_periods: Vec<TimeRange>,
    
    /// Baseline speed without optimization (Mbps)
    pub baseline_speed: f64,
    
    /// Confidence level in the analysis (0.0 to 1.0)
    pub confidence_level: f64,
    
    /// Number of days of data collected
    pub data_collection_days: u32,
    
    /// Detected ISP information
    pub isp_profile: Option<String>,
}

/// Time range for throttling periods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_hour: u8,
    pub start_minute: u8,
    pub end_hour: u8,
    pub end_minute: u8,
    pub days_of_week: Vec<u8>, // 0 = Sunday, 1 = Monday, etc.
}

/// Decision about whether to activate optimization
#[derive(Debug, Clone)]
pub struct OptimizationDecision {
    pub should_activate: bool,
    pub reason: String,
    pub confidence: f64,
    pub estimated_improvement: Option<f64>, // Expected speed multiplier
}

/// Current system status for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub state: SystemState,
    pub message: String,
    pub data_collection_progress: Option<DataCollectionProgress>,
    pub effectiveness: Option<EffectivenessMetrics>,
}

/// System operational states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemState {
    /// Initial state - collecting baseline data
    Learning,
    /// Actively optimizing network performance
    Optimizing,
    /// Monitoring but not optimizing
    Monitoring,
    /// Temporarily inactive due to conditions
    Inactive,
    /// Error state requiring attention
    Error(String),
}

/// Progress of data collection phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataCollectionProgress {
    pub days_collected: u32,
    pub days_needed: u32,
    pub progress_percentage: f64,
}

/// Metrics showing optimization effectiveness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessMetrics {
    /// Speed improvement multiplier (e.g., 2.5 = 2.5x faster)
    pub improvement_factor: f64,
    
    /// Average speed before optimization (Mbps)
    pub baseline_speed: f64,
    
    /// Average speed with optimization (Mbps)
    pub optimized_speed: f64,
    
    /// Confidence in these metrics (0.0 to 1.0)
    pub confidence: f64,
    
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: &str, end: &str) -> Self {
        // Parse time strings like "19:00" and "22:00"
        let start_parts: Vec<&str> = start.split(':').collect();
        let end_parts: Vec<&str> = end.split(':').collect();
        
        Self {
            start_hour: start_parts[0].parse().unwrap_or(0),
            start_minute: start_parts[1].parse().unwrap_or(0),
            end_hour: end_parts[0].parse().unwrap_or(23),
            end_minute: end_parts[1].parse().unwrap_or(59),
            days_of_week: vec![0, 1, 2, 3, 4, 5, 6], // All days by default
        }
    }
}

impl SystemStatus {
    /// Creates a learning status for initial data collection
    pub fn learning(days_collected: u32, days_needed: u32) -> Self {
        let progress = DataCollectionProgress {
            days_collected,
            days_needed,
            progress_percentage: (days_collected as f64 / days_needed as f64) * 100.0,
        };
        
        Self {
            state: SystemState::Learning,
            message: format!("Learning your network patterns ({} of {} days)", days_collected, days_needed),
            data_collection_progress: Some(progress),
            effectiveness: None,
        }
    }
    
    /// Creates an optimizing status with effectiveness metrics
    pub fn optimizing(effectiveness: EffectivenessMetrics) -> Self {
        Self {
            state: SystemState::Optimizing,
            message: format!("Optimizing ({}x improvement)", effectiveness.improvement_factor),
            data_collection_progress: None,
            effectiveness: Some(effectiveness),
        }
    }
}

/// Machine learning model for pattern optimization
/// Uses simple yet effective statistical learning approaches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternLearningModel {
    /// Historical effectiveness scores for different strategies
    pub strategy_effectiveness: HashMap<String, StrategyEffectiveness>,
    
    /// Time-based pattern weights (hour of day -> effectiveness multiplier)
    pub temporal_weights: HashMap<u8, f64>,
    
    /// Day-of-week pattern weights
    pub weekly_weights: HashMap<Weekday, f64>,
    
    /// ISP-specific learning parameters
    pub isp_parameters: HashMap<String, ISPLearningParams>,
    
    /// Model confidence and learning progress
    pub model_confidence: f64,
    
    /// Number of training samples used
    pub training_samples: u32,
    
    /// Last model update timestamp
    pub last_updated: DateTime<Utc>,
}

/// Effectiveness data for a specific optimization strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEffectiveness {
    /// Average improvement factor achieved
    pub avg_improvement: f64,
    
    /// Number of times this strategy was tested
    pub sample_count: u32,
    
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    
    /// Confidence in this effectiveness measurement
    pub confidence: f64,
    
    /// Recent performance trend (positive = improving, negative = declining)
    pub trend: f64,
    
    /// Last time this strategy was used
    pub last_used: DateTime<Utc>,
}

/// ISP-specific learning parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ISPLearningParams {
    /// Optimal stealth level for this ISP
    pub optimal_stealth_level: StealthLevel,
    
    /// Best server rotation interval (minutes)
    pub optimal_rotation_interval: u32,
    
    /// Preferred traffic intensity
    pub optimal_traffic_intensity: f64,
    
    /// Detection risk score (0.0 = low risk, 1.0 = high risk)
    pub detection_risk: f64,
    
    /// Learning confidence for this ISP
    pub confidence: f64,
}

/// Effectiveness analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessAnalysis {
    /// Current effectiveness metrics
    pub current_effectiveness: EffectivenessMetrics,
    
    /// Comparison with baseline (no optimization)
    pub baseline_comparison: BaselineComparison,
    
    /// Strategy performance rankings
    pub strategy_rankings: Vec<StrategyRanking>,
    
    /// Recommendations for improvement
    pub recommendations: Vec<OptimizationRecommendation>,
    
    /// Analysis confidence level
    pub confidence: f64,
}

/// Baseline comparison metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    /// Average speed without optimization
    pub baseline_speed: f64,
    
    /// Average speed with optimization
    pub optimized_speed: f64,
    
    /// Improvement factor
    pub improvement_factor: f64,
    
    /// Statistical significance of improvement
    pub significance: f64,
}

/// Strategy performance ranking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRanking {
    /// Strategy name
    pub strategy_name: String,
    
    /// Effectiveness score (0.0 to 1.0)
    pub effectiveness_score: f64,
    
    /// Confidence in this ranking
    pub confidence: f64,
    
    /// Number of samples used for ranking
    pub sample_count: u32,
}

/// Optimization recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    /// Type of recommendation
    pub recommendation_type: RecommendationType,
    
    /// Human-readable description
    pub description: String,
    
    /// Expected improvement if followed
    pub expected_improvement: f64,
    
    /// Confidence in this recommendation
    pub confidence: f64,
    
    /// Priority level (1 = highest, 5 = lowest)
    pub priority: u8,
}

/// Types of optimization recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecommendationType {
    /// Adjust stealth level
    StealthAdjustment,
    
    /// Change server rotation frequency
    RotationAdjustment,
    
    /// Modify traffic intensity
    TrafficAdjustment,
    
    /// Switch to different strategy
    StrategySwitch,
    
    /// Timing optimization
    TimingOptimization,
    
    /// ISP-specific optimization
    ISPOptimization,
}

/// Default intelligence core implementation with machine learning
pub struct DefaultIntelligenceCore {
    pub repository: Arc<Repository>,
    pub learning_model: PatternLearningModel,
    min_learning_days: u32,
}

impl Default for PatternLearningModel {
    fn default() -> Self {
        Self {
            strategy_effectiveness: HashMap::new(),
            temporal_weights: HashMap::new(),
            weekly_weights: HashMap::new(),
            isp_parameters: HashMap::new(),
            model_confidence: 0.0,
            training_samples: 0,
            last_updated: Utc::now(),
        }
    }
}

impl Default for StrategyEffectiveness {
    fn default() -> Self {
        Self {
            avg_improvement: 1.0,
            sample_count: 0,
            success_rate: 0.0,
            confidence: 0.0,
            trend: 0.0,
            last_used: Utc::now(),
        }
    }
}

impl DefaultIntelligenceCore {
    pub fn new(repository: Arc<Repository>) -> Self {
        Self {
            repository,
            learning_model: PatternLearningModel::default(),
            min_learning_days: 7, // Minimum 7 days of data before making recommendations
        }
    }

    /// Creates a core with a custom minimum learning days value
    pub fn with_min_learning_days(repository: Arc<Repository>, min_learning_days: u32) -> Self {
        Self {
            repository,
            learning_model: PatternLearningModel::default(),
            min_learning_days,
        }
    }

    /// Updates the minimum learning days at runtime
    pub fn set_min_learning_days(&mut self, days: u32) {
        self.min_learning_days = days;
    }

    /// Perform comprehensive effectiveness analysis
    pub async fn analyze_effectiveness(&self) -> Result<EffectivenessAnalysis> {
        let since = Utc::now() - Duration::days(30);
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        
        if measurements.len() < 50 {
            return Ok(EffectivenessAnalysis {
                current_effectiveness: EffectivenessMetrics {
                    improvement_factor: 1.0,
                    baseline_speed: 0.0,
                    optimized_speed: 0.0,
                    confidence: 0.0,
                    last_updated: Utc::now(),
                },
                baseline_comparison: BaselineComparison {
                    baseline_speed: 0.0,
                    optimized_speed: 0.0,
                    improvement_factor: 1.0,
                    significance: 0.0,
                },
                strategy_rankings: Vec::new(),
                recommendations: Vec::new(),
                confidence: 0.0,
            });
        }

        // Separate optimized and baseline measurements
        let optimized_measurements: Vec<_> = measurements.iter()
            .filter(|m| m.optimization_active)
            .collect();
        
        let baseline_measurements: Vec<_> = measurements.iter()
            .filter(|m| !m.optimization_active)
            .collect();

        // Calculate baseline comparison
        let baseline_comparison = self.calculate_baseline_comparison(&baseline_measurements, &optimized_measurements)?;
        
        // Calculate current effectiveness metrics
        let current_effectiveness = EffectivenessMetrics {
            improvement_factor: baseline_comparison.improvement_factor,
            baseline_speed: baseline_comparison.baseline_speed,
            optimized_speed: baseline_comparison.optimized_speed,
            confidence: self.calculate_effectiveness_confidence(
                optimized_measurements.len(),
                baseline_measurements.len(),
                baseline_comparison.improvement_factor,
            ),
            last_updated: Utc::now(),
        };

        // Generate strategy rankings
        let strategy_rankings = self.generate_strategy_rankings().await?;
        
        // Generate recommendations
        let recommendations = self.generate_recommendations().await?;
        
        // Calculate overall analysis confidence
        let confidence = self.calculate_analysis_confidence(&measurements, &current_effectiveness);

        Ok(EffectivenessAnalysis {
            current_effectiveness,
            baseline_comparison,
            strategy_rankings,
            recommendations,
            confidence,
        })
    }

    /// Calculate baseline comparison metrics
    fn calculate_baseline_comparison(
        &self,
        baseline_measurements: &[&SpeedMeasurement],
        optimized_measurements: &[&SpeedMeasurement],
    ) -> Result<BaselineComparison> {
        let baseline_speed = if !baseline_measurements.is_empty() {
            baseline_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / baseline_measurements.len() as f64
        } else {
            0.0
        };

        let optimized_speed = if !optimized_measurements.is_empty() {
            optimized_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / optimized_measurements.len() as f64
        } else {
            0.0
        };

        let improvement_factor = if baseline_speed > 0.0 {
            optimized_speed / baseline_speed
        } else {
            1.0
        };

        // Calculate statistical significance using simple variance analysis
        let significance = self.calculate_statistical_significance(baseline_measurements, optimized_measurements);

        Ok(BaselineComparison {
            baseline_speed,
            optimized_speed,
            improvement_factor,
            significance,
        })
    }

    /// Calculate statistical significance of improvement
    fn calculate_statistical_significance(
        &self,
        baseline_measurements: &[&SpeedMeasurement],
        optimized_measurements: &[&SpeedMeasurement],
    ) -> f64 {
        if baseline_measurements.len() < 5 || optimized_measurements.len() < 5 {
            return 0.0;
        }

        // Calculate means
        let baseline_mean = baseline_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / baseline_measurements.len() as f64;
        let optimized_mean = optimized_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / optimized_measurements.len() as f64;

        // Calculate variances
        let baseline_variance = baseline_measurements.iter()
            .map(|m| (m.download_mbps - baseline_mean).powi(2))
            .sum::<f64>() / (baseline_measurements.len() - 1) as f64;

        let optimized_variance = optimized_measurements.iter()
            .map(|m| (m.download_mbps - optimized_mean).powi(2))
            .sum::<f64>() / (optimized_measurements.len() - 1) as f64;

        // Simple t-test approximation
        let pooled_variance = (baseline_variance + optimized_variance) / 2.0;
        if pooled_variance <= 0.0 {
            return 0.0;
        }

        let standard_error = (pooled_variance * (1.0 / baseline_measurements.len() as f64 + 1.0 / optimized_measurements.len() as f64)).sqrt();
        if standard_error <= 0.0 {
            return 0.0;
        }

        let t_statistic = (optimized_mean - baseline_mean) / standard_error;
        
        // Convert t-statistic to significance (0.0 to 1.0)
        (t_statistic.abs() / 3.0).min(1.0)
    }

    /// Generate strategy performance rankings
    async fn generate_strategy_rankings(&self) -> Result<Vec<StrategyRanking>> {
        let mut rankings = Vec::new();

        for (strategy_name, effectiveness) in &self.learning_model.strategy_effectiveness {
            rankings.push(StrategyRanking {
                strategy_name: strategy_name.clone(),
                effectiveness_score: effectiveness.avg_improvement - 1.0, // Convert to 0-based score
                confidence: effectiveness.confidence,
                sample_count: effectiveness.sample_count,
            });
        }

        // Sort by effectiveness score (descending)
        rankings.sort_by(|a, b| b.effectiveness_score.partial_cmp(&a.effectiveness_score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(rankings)
    }

    /// Calculate overall analysis confidence
    fn calculate_analysis_confidence(&self, measurements: &[SpeedMeasurement], effectiveness: &EffectivenessMetrics) -> f64 {
        let data_confidence = (measurements.len() as f64 / 500.0).min(1.0) * 0.4;
        let effectiveness_confidence = effectiveness.confidence * 0.4;
        let model_confidence = self.learning_model.model_confidence * 0.2;

        data_confidence + effectiveness_confidence + model_confidence
    }

    /// Advanced pattern learning with statistical analysis
    pub async fn learn_advanced_patterns(&mut self) -> Result<()> {
        let since = Utc::now() - Duration::days(60); // Use 60 days for advanced learning
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        
        if measurements.len() < 100 {
            return Ok(()); // Not enough data for advanced learning
        }

        // Learn temporal patterns with statistical significance
        self.learn_temporal_patterns_advanced(&measurements).await?;
        
        // Learn ISP-specific patterns
        self.learn_isp_patterns(&measurements).await?;
        
        // Learn effectiveness patterns
        self.learn_effectiveness_patterns(&measurements).await?;
        
        // Update model confidence
        self.calculate_model_confidence();
        
        Ok(())
    }

    /// Learn temporal patterns with advanced statistical analysis
    async fn learn_temporal_patterns_advanced(&mut self, measurements: &[SpeedMeasurement]) -> Result<()> {
        let mut hourly_data: std::collections::HashMap<u8, Vec<f64>> = std::collections::HashMap::new();
        let mut weekly_data: std::collections::HashMap<Weekday, Vec<f64>> = std::collections::HashMap::new();

        // Group measurements by time patterns
        for measurement in measurements {
            let hour = measurement.timestamp.hour() as u8;
            let weekday = measurement.timestamp.weekday();
            let performance = measurement.performance_score();

            hourly_data.entry(hour).or_default().push(performance);
            weekly_data.entry(weekday).or_default().push(performance);
        }

        // Calculate statistical weights for hourly patterns
        for (hour, scores) in hourly_data {
            if scores.len() >= 5 {
                let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
                
                // Weight combines mean performance and consistency (low variance)
                let consistency_factor = 1.0 / (1.0 + variance);
                let weight = mean * consistency_factor;
                
                self.learning_model.temporal_weights.insert(hour, weight);
            }
        }

        // Calculate statistical weights for weekly patterns
        for (weekday, scores) in weekly_data {
            if scores.len() >= 5 {
                let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / scores.len() as f64;
                
                let consistency_factor = 1.0 / (1.0 + variance);
                let weight = mean * consistency_factor;
                
                self.learning_model.weekly_weights.insert(weekday, weight);
            }
        }

        Ok(())
    }

    /// Learn ISP-specific optimization patterns
    async fn learn_isp_patterns(&mut self, measurements: &[SpeedMeasurement]) -> Result<()> {
        if let Some(isp_profile) = self.repository.get_current_isp_profile().await? {
            let optimized_measurements: Vec<_> = measurements.iter()
                .filter(|m| m.optimization_active)
                .collect();

            if optimized_measurements.len() >= 20 {
                // Analyze effectiveness by time of day
                let mut hourly_effectiveness: std::collections::HashMap<u8, Vec<f64>> = std::collections::HashMap::new();
                
                for measurement in &optimized_measurements {
                    let hour = measurement.timestamp.hour() as u8;
                    hourly_effectiveness.entry(hour).or_default().push(measurement.performance_score());
                }

                // Find optimal parameters based on effectiveness patterns
                let mut best_hours = Vec::new();
                let mut avg_effectiveness = 0.0;
                let mut total_samples = 0;

                for (hour, scores) in hourly_effectiveness {
                    if scores.len() >= 3 {
                        let hour_avg = scores.iter().sum::<f64>() / scores.len() as f64;
                        avg_effectiveness += hour_avg * scores.len() as f64;
                        total_samples += scores.len();
                        
                        if hour_avg > 0.7 {
                            best_hours.push(hour);
                        }
                    }
                }

                if total_samples > 0 {
                    avg_effectiveness /= total_samples as f64;
                }

                // Determine optimal stealth level based on ISP characteristics and effectiveness
                let optimal_stealth = if isp_profile.is_known_throttling_isp() {
                    if avg_effectiveness > 0.8 {
                        StealthLevel::High
                    } else {
                        StealthLevel::Maximum
                    }
                } else {
                    StealthLevel::Medium
                };

                // Calculate detection risk based on effectiveness patterns
                let detection_risk = if avg_effectiveness < 0.5 {
                    0.9 // High risk if effectiveness is dropping
                } else if isp_profile.is_known_throttling_isp() {
                    0.6
                } else {
                    0.3
                };

                let params = ISPLearningParams {
                    optimal_stealth_level: optimal_stealth,
                    optimal_rotation_interval: if detection_risk > 0.7 { 5 } else { 10 },
                    optimal_traffic_intensity: if detection_risk > 0.7 { 0.3 } else { 0.5 },
                    detection_risk,
                    confidence: (total_samples as f64 / 50.0).min(1.0),
                };

                self.learning_model.isp_parameters.insert(isp_profile.name, params);
            }
        }

        Ok(())
    }

    /// Learn effectiveness patterns for different strategies
    async fn learn_effectiveness_patterns(&mut self, measurements: &[SpeedMeasurement]) -> Result<()> {
        // Group measurements by optimization periods to infer strategy effectiveness
        let optimized_measurements: Vec<_> = measurements.iter()
            .filter(|m| m.optimization_active)
            .collect();

        let baseline_measurements: Vec<_> = measurements.iter()
            .filter(|m| !m.optimization_active)
            .collect();

        if optimized_measurements.len() >= 10 && baseline_measurements.len() >= 10 {
            // Calculate overall effectiveness
            let baseline_avg = baseline_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / baseline_measurements.len() as f64;
            let optimized_avg = optimized_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / optimized_measurements.len() as f64;
            
            let improvement = if baseline_avg > 0.0 { optimized_avg / baseline_avg } else { 1.0 };
            let success_rate = if improvement > 1.2 { 1.0 } else { (improvement - 1.0).max(0.0) };

            // Calculate trend by comparing recent vs older measurements
            let recent_cutoff = Utc::now() - Duration::days(7);
            let recent_optimized: Vec<_> = optimized_measurements.iter()
                .filter(|m| m.timestamp > recent_cutoff)
                .collect();
            
            let older_optimized: Vec<_> = optimized_measurements.iter()
                .filter(|m| m.timestamp <= recent_cutoff)
                .collect();

            let trend = if !recent_optimized.is_empty() && !older_optimized.is_empty() {
                let recent_avg = recent_optimized.iter().map(|m| m.download_mbps).sum::<f64>() / recent_optimized.len() as f64;
                let older_avg = older_optimized.iter().map(|m| m.download_mbps).sum::<f64>() / older_optimized.len() as f64;
                
                if older_avg > 0.0 {
                    (recent_avg - older_avg) / older_avg
                } else {
                    0.0
                }
            } else {
                0.0
            };

            // Update default strategy effectiveness
            let effectiveness = StrategyEffectiveness {
                avg_improvement: improvement,
                sample_count: optimized_measurements.len() as u32,
                success_rate,
                confidence: self.calculate_effectiveness_confidence(
                    optimized_measurements.len(),
                    baseline_measurements.len(),
                    improvement,
                ),
                trend,
                last_used: Utc::now(),
            };

            self.learning_model.strategy_effectiveness.insert("Default".to_string(), effectiveness);
        }

        Ok(())
    }

    /// Train the machine learning model with historical data
    pub async fn train_model(&mut self) -> Result<()> {
        let since = Utc::now() - Duration::days(30); // Use last 30 days for training
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        
        if measurements.len() < 50 {
            // Not enough data for meaningful training
            return Ok(());
        }

        // Use advanced pattern learning for better accuracy
        self.learn_advanced_patterns().await?;
        
        // Fallback to basic learning if advanced learning didn't work
        if self.learning_model.temporal_weights.is_empty() {
            self.update_temporal_patterns(&measurements).await?;
        }
        
        if self.learning_model.strategy_effectiveness.is_empty() {
            self.update_strategy_effectiveness().await?;
        }
        
        if self.learning_model.isp_parameters.is_empty() {
            self.update_isp_parameters().await?;
        }
        
        // Calculate overall model confidence
        self.calculate_model_confidence();
        
        self.learning_model.training_samples = measurements.len() as u32;
        self.learning_model.last_updated = Utc::now();
        
        Ok(())
    }

    /// Update temporal pattern weights based on historical data
    async fn update_temporal_patterns(&mut self, measurements: &[SpeedMeasurement]) -> Result<()> {
        let mut hourly_performance: HashMap<u8, Vec<f64>> = HashMap::new();
        let mut weekly_performance: HashMap<Weekday, Vec<f64>> = HashMap::new();
        
        // Group measurements by time patterns
        for measurement in measurements {
            let hour = measurement.timestamp.hour() as u8;
            let weekday = measurement.timestamp.weekday();
            let performance_score = measurement.performance_score();
            
            hourly_performance.entry(hour).or_default().push(performance_score);
            weekly_performance.entry(weekday).or_default().push(performance_score);
        }
        
        // Calculate temporal weights
        for (hour, scores) in hourly_performance {
            if scores.len() >= 3 {
                let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
                self.learning_model.temporal_weights.insert(hour, avg_score);
            }
        }
        
        for (weekday, scores) in weekly_performance {
            if scores.len() >= 3 {
                let avg_score = scores.iter().sum::<f64>() / scores.len() as f64;
                self.learning_model.weekly_weights.insert(weekday, avg_score);
            }
        }
        
        Ok(())
    }

    /// Update strategy effectiveness based on historical performance
    async fn update_strategy_effectiveness(&mut self) -> Result<()> {
        // Get all optimization strategies from database
        let strategies = vec![
            OptimizationStrategy::default_strategy(),
            OptimizationStrategy::high_stealth_strategy(),
        ];
        
        for strategy in strategies {
            let effectiveness = self.calculate_strategy_effectiveness(&strategy).await?;
            self.learning_model.strategy_effectiveness.insert(
                strategy.name.clone(),
                effectiveness,
            );
        }
        
        Ok(())
    }

    /// Calculate effectiveness for a specific strategy
    pub async fn calculate_strategy_effectiveness(&self, _strategy: &OptimizationStrategy) -> Result<StrategyEffectiveness> {
        // This would analyze historical data for this specific strategy
        // For now, we'll use a simplified calculation
        
        let since = Utc::now() - Duration::days(14);
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        
        let optimized_measurements: Vec<_> = measurements.iter()
            .filter(|m| m.optimization_active)
            .collect();
        
        let baseline_measurements: Vec<_> = measurements.iter()
            .filter(|m| !m.optimization_active)
            .collect();
        
        if optimized_measurements.len() < 5 || baseline_measurements.len() < 5 {
            return Ok(StrategyEffectiveness::default());
        }
        
        let avg_optimized = optimized_measurements.iter()
            .map(|m| m.download_mbps)
            .sum::<f64>() / optimized_measurements.len() as f64;
        
        let avg_baseline = baseline_measurements.iter()
            .map(|m| m.download_mbps)
            .sum::<f64>() / baseline_measurements.len() as f64;
        
        let improvement = if avg_baseline > 0.0 { avg_optimized / avg_baseline } else { 1.0 };
        let success_rate = if improvement > 1.2 { 1.0 } else { improvement - 1.0 };
        let confidence = self.calculate_effectiveness_confidence(
            optimized_measurements.len(),
            baseline_measurements.len(),
            improvement,
        );
        
        Ok(StrategyEffectiveness {
            avg_improvement: improvement,
            sample_count: optimized_measurements.len() as u32,
            success_rate: success_rate.max(0.0).min(1.0),
            confidence,
            trend: 0.0, // Would be calculated from recent vs older data
            last_used: Utc::now(),
        })
    }

    /// Update ISP-specific learning parameters
    async fn update_isp_parameters(&mut self) -> Result<()> {
        if let Some(isp_profile) = self.repository.get_current_isp_profile().await? {
            let params = ISPLearningParams {
                optimal_stealth_level: if isp_profile.is_known_throttling_isp() {
                    StealthLevel::High
                } else {
                    StealthLevel::Medium
                },
                optimal_rotation_interval: 10,
                optimal_traffic_intensity: 0.5,
                detection_risk: if isp_profile.is_known_throttling_isp() { 0.7 } else { 0.3 },
                confidence: 0.6,
            };
            
            self.learning_model.isp_parameters.insert(isp_profile.name, params);
        }
        
        Ok(())
    }

    /// Calculate overall model confidence
    fn calculate_model_confidence(&mut self) {
        let mut confidence_factors = Vec::new();
        
        // Factor 1: Amount of training data
        let data_confidence = (self.learning_model.training_samples as f64 / 1000.0).min(1.0);
        confidence_factors.push(data_confidence * 0.3);
        
        // Factor 2: Strategy effectiveness confidence
        let strategy_confidence = self.learning_model.strategy_effectiveness.values()
            .map(|s| s.confidence)
            .sum::<f64>() / self.learning_model.strategy_effectiveness.len().max(1) as f64;
        confidence_factors.push(strategy_confidence * 0.4);
        
        // Factor 3: Temporal pattern confidence
        let temporal_confidence = if self.learning_model.temporal_weights.len() >= 12 { 0.8 } else { 0.4 };
        confidence_factors.push(temporal_confidence * 0.3);
        
        self.learning_model.model_confidence = confidence_factors.iter().sum::<f64>().min(1.0);
    }

    /// Calculate confidence for effectiveness measurements
    pub fn calculate_effectiveness_confidence(&self, optimized_samples: usize, baseline_samples: usize, improvement: f64) -> f64 {
        let sample_confidence = ((optimized_samples + baseline_samples) as f64 / 100.0).min(1.0);
        let improvement_confidence = if improvement > 1.5 { 1.0 } else if improvement > 1.2 { 0.8 } else { 0.5 };
        
        (sample_confidence * 0.6 + improvement_confidence * 0.4).min(1.0)
    }

    /// Predict optimal optimization times based on learned patterns
    pub fn predict_optimal_times(&self) -> Vec<(u8, f64)> {
        let mut optimal_times = Vec::new();
        
        for (&hour, &weight) in &self.learning_model.temporal_weights {
            // Lower weights indicate throttling periods (better for optimization)
            if weight < 0.6 {
                let optimization_score = 1.0 - weight; // Invert weight for optimization score
                optimal_times.push((hour, optimization_score));
            }
        }
        
        // Sort by optimization score (descending)
        optimal_times.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        
        optimal_times
    }

    /// Get confidence score for a specific time period
    pub fn get_time_confidence(&self, hour: u8, weekday: Weekday) -> f64 {
        let hourly_confidence = self.learning_model.temporal_weights.get(&hour).copied().unwrap_or(0.5);
        let weekly_confidence = self.learning_model.weekly_weights.get(&weekday).copied().unwrap_or(0.5);
        
        // Combine hourly and weekly confidence
        (hourly_confidence * 0.7 + weekly_confidence * 0.3).min(1.0)
    }

    /// Evaluate if current conditions are favorable for optimization
    pub fn is_favorable_time(&self) -> bool {
        let now = Utc::now();
        let current_hour = now.hour() as u8;
        let current_weekday = now.weekday();
        
        let confidence = self.get_time_confidence(current_hour, current_weekday);
        
        // Consider it favorable if confidence is low (indicating throttling)
        // and we have sufficient model confidence
        confidence < 0.6 && self.learning_model.model_confidence > 0.5
    }

    /// Get the best strategy for current ISP and conditions
    pub async fn get_optimal_strategy(&self) -> Result<Option<OptimizationStrategy>> {
        // Try to get the best strategy from database first
        if let Some(strategy) = self.repository.get_best_optimization_strategy().await? {
            return Ok(Some(strategy));
        }

        // If no stored strategy, create one based on learned parameters
        if let Some(isp_profile) = self.repository.get_current_isp_profile().await? {
            if let Some(isp_params) = self.learning_model.isp_parameters.get(&isp_profile.name) {
                let strategy = OptimizationStrategy {
                    id: None,
                    name: format!("Learned-{}", isp_profile.name),
                    server_rotation_interval_minutes: isp_params.optimal_rotation_interval,
                    packet_timing_min_seconds: if isp_params.detection_risk > 0.7 { 45.0 } else { 30.0 },
                    packet_timing_max_seconds: if isp_params.detection_risk > 0.7 { 90.0 } else { 60.0 },
                    connection_count: if isp_params.detection_risk > 0.7 { 2 } else { 3 },
                    traffic_intensity: isp_params.optimal_traffic_intensity,
                    stealth_level: isp_params.optimal_stealth_level.clone(),
                    effectiveness_score: Some(isp_params.confidence),
                    created_at: Utc::now(),
                };
                
                return Ok(Some(strategy));
            }
        }

        // Fallback to default strategy
        Ok(Some(OptimizationStrategy::default_strategy()))
    }

    /// Generate optimization recommendations based on learned patterns
    pub async fn generate_recommendations(&self) -> Result<Vec<OptimizationRecommendation>> {
        let mut recommendations = Vec::new();
        
        // Recommendation 1: Stealth level adjustment
        if let Some(isp_params) = self.learning_model.isp_parameters.values().next() {
            if isp_params.detection_risk > 0.6 {
                recommendations.push(OptimizationRecommendation {
                    recommendation_type: RecommendationType::StealthAdjustment,
                    description: "Increase stealth level to avoid ISP detection".to_string(),
                    expected_improvement: 1.2,
                    confidence: isp_params.confidence,
                    priority: 1,
                });
            }
        }
        
        // Recommendation 2: Strategy switching
        let best_strategy = self.learning_model.strategy_effectiveness.iter()
            .max_by(|a, b| a.1.avg_improvement.partial_cmp(&b.1.avg_improvement).unwrap_or(std::cmp::Ordering::Equal));
        
        if let Some((strategy_name, effectiveness)) = best_strategy {
            if effectiveness.avg_improvement > 1.5 && effectiveness.confidence > 0.7 {
                recommendations.push(OptimizationRecommendation {
                    recommendation_type: RecommendationType::StrategySwitch,
                    description: format!("Switch to '{}' strategy for better performance", strategy_name),
                    expected_improvement: effectiveness.avg_improvement,
                    confidence: effectiveness.confidence,
                    priority: 2,
                });
            }
        }
        
        // Recommendation 3: Timing optimization
        let peak_hours = self.learning_model.temporal_weights.iter()
            .filter(|(_, &weight)| weight < 0.5)
            .map(|(&hour, _)| hour)
            .collect::<Vec<_>>();
        
        if !peak_hours.is_empty() {
            recommendations.push(OptimizationRecommendation {
                recommendation_type: RecommendationType::TimingOptimization,
                description: format!("Focus optimization during peak throttling hours: {:?}", peak_hours),
                expected_improvement: 2.0,
                confidence: 0.8,
                priority: 1,
            });
        }
        
        Ok(recommendations)
    }
}

impl IntelligenceCore for DefaultIntelligenceCore {
    async fn analyze_patterns(&self) -> Result<PatternAnalysis> {
        let since = Utc::now() - Duration::days(self.min_learning_days as i64);
        let measurements = self.repository.get_speed_measurements_since(since).await?;
        
        if measurements.len() < 20 {
            return Ok(PatternAnalysis {
                throttling_periods: Vec::new(),
                baseline_speed: 0.0,
                confidence_level: 0.0,
                data_collection_days: 0,
                isp_profile: None,
            });
        }
        
        // Calculate baseline speed
        let baseline_measurements: Vec<_> = measurements.iter()
            .filter(|m| !m.optimization_active)
            .collect();
        
        let baseline_speed = if !baseline_measurements.is_empty() {
            baseline_measurements.iter().map(|m| m.download_mbps).sum::<f64>() / baseline_measurements.len() as f64
        } else {
            0.0
        };
        
        // Detect throttling periods using temporal weights
        let mut throttling_periods = Vec::new();
        for (&hour, &weight) in &self.learning_model.temporal_weights {
            if weight < 0.6 { // Low performance indicates throttling
                throttling_periods.push(TimeRange {
                    start_hour: hour,
                    start_minute: 0,
                    end_hour: hour,
                    end_minute: 59,
                    days_of_week: vec![0, 1, 2, 3, 4, 5, 6], // All days
                });
            }
        }
        
        let confidence_level = self.learning_model.model_confidence;
        let data_collection_days = measurements.len() as u32 / 24; // Rough estimate
        
        let isp_profile = self.repository.get_current_isp_profile().await?
            .map(|p| p.name);
        
        Ok(PatternAnalysis {
            throttling_periods,
            baseline_speed,
            confidence_level,
            data_collection_days,
            isp_profile,
        })
    }

    async fn should_optimize(&self) -> Result<OptimizationDecision> {
        let analysis = self.analyze_patterns().await?;
        
        // Decision logic based on learned patterns
        let should_activate = analysis.confidence_level > 0.6 && 
                             analysis.baseline_speed > 0.0 &&
                             !analysis.throttling_periods.is_empty();
        
        let reason = if should_activate {
            "Throttling patterns detected with sufficient confidence".to_string()
        } else if analysis.confidence_level <= 0.6 {
            "Insufficient data confidence for optimization".to_string()
        } else {
            "No significant throttling patterns detected".to_string()
        };
        
        let estimated_improvement = if should_activate {
            Some(self.learning_model.strategy_effectiveness.values()
                .map(|s| s.avg_improvement)
                .fold(1.0, f64::max))
        } else {
            None
        };
        
        Ok(OptimizationDecision {
            should_activate,
            reason,
            confidence: analysis.confidence_level,
            estimated_improvement,
        })
    }

    async fn adapt_strategy(&self, effectiveness: f64) -> Result<()> {
        // Real-time strategy adaptation based on effectiveness feedback
        
        // Log the adaptation for monitoring
        println!("Adapting strategy based on effectiveness: {:.2}", effectiveness);
        
        // If effectiveness is very low, we might need to switch strategies
        if effectiveness < 1.2 {
            // Poor effectiveness - consider increasing stealth or changing approach
            if let Some(isp_profile) = self.repository.get_current_isp_profile().await? {
                println!("Low effectiveness detected for ISP: {}. Consider increasing stealth level.", isp_profile.name);
            }
        } else if effectiveness > 2.5 {
            // Excellent effectiveness - current strategy is working well
            println!("Excellent effectiveness detected. Current strategy is optimal.");
        }
        
        // In a full implementation, this would:
        // 1. Update strategy effectiveness in the learning model
        // 2. Adjust ISP parameters based on real-time feedback
        // 3. Trigger strategy switching if needed
        // 4. Update confidence scores
        
        // For now, we maintain the existing approach of logging and monitoring
        // Real-time adaptation will be implemented in the network optimizer
        
        Ok(())
    }

    async fn get_status(&self) -> Result<SystemStatus> {
        let analysis = self.analyze_patterns().await?;
        
        if analysis.data_collection_days < self.min_learning_days {
            Ok(SystemStatus::learning(analysis.data_collection_days, self.min_learning_days))
        } else if analysis.confidence_level > 0.6 {
            let effectiveness = EffectivenessMetrics {
                improvement_factor: self.learning_model.strategy_effectiveness.values()
                    .map(|s| s.avg_improvement)
                    .fold(1.0, f64::max),
                baseline_speed: analysis.baseline_speed,
                optimized_speed: analysis.baseline_speed * 1.5, // Estimated
                confidence: analysis.confidence_level,
                last_updated: Utc::now(),
            };
            Ok(SystemStatus::optimizing(effectiveness))
        } else {
            Ok(SystemStatus {
                state: SystemState::Monitoring,
                message: "Monitoring network patterns".to_string(),
                data_collection_progress: None,
                effectiveness: None,
            })
        }
    }
}

/// Periodic decision engine that trains the model and evaluates optimization decisions
pub struct DecisionEngine {
    repository: Arc<Repository>,
    intelligence: DefaultIntelligenceCore,
    min_training_interval_minutes: u64,
}

impl DecisionEngine {
    pub fn new(repository: Arc<Repository>) -> Self {
        let intelligence = DefaultIntelligenceCore::new(Arc::clone(&repository));
        Self {
            repository,
            intelligence,
            min_training_interval_minutes: 15,
        }
    }

    /// Allows configuring minimum learning days used by the intelligence core
    pub fn set_min_learning_days(&mut self, days: u32) {
        self.intelligence.set_min_learning_days(days);
    }

    /// Runs periodically: trains model and logs decision outcome
    pub async fn run(&mut self) -> Result<()> {
        use tokio::time::{sleep, Duration as TokioDuration};

        loop {
            // Cleanup old data based on privacy policy (30 days)
            let _ = self.repository.cleanup_old_data(30).await;

            // Train and analyze
            if let Err(e) = self.intelligence.train_model().await {
                tracing::warn!("Model training failed: {}", e);
            }

            match self.intelligence.should_optimize().await {
                Ok(decision) => {
                    tracing::info!(
                        should_activate = decision.should_activate,
                        confidence = decision.confidence,
                        reason = %decision.reason,
                        est_impr = ?decision.estimated_improvement,
                        "Optimization decision evaluated"
                    );
                }
                Err(e) => tracing::warn!("Decision evaluation failed: {}", e),
            }

            sleep(TokioDuration::from_secs(self.min_training_interval_minutes * 60)).await;
        }
    }

    pub fn intelligence(&self) -> &DefaultIntelligenceCore {
        &self.intelligence
    }
}