use tracing::{info, error};
use tracing_subscriber;

mod core;
mod network;
mod ui;
mod data;

use crate::core::error::Result;
use crate::core::intelligence::{DecisionEngine, DefaultIntelligenceCore};
use crate::core::intelligence::IntelligenceCore;
use crate::core::config::AppConfig;
use crate::data::migrations::MigrationManager;
use crate::data::models::OptimizationStrategy;
use crate::data::repository::Repository;
use crate::ui::tray::SystemTray;
use crate::network::monitor::BackgroundMonitor;
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    tauri::Builder::default()
        .system_tray(SystemTray::create_tray_menu())
        .on_system_tray_event(|app, event| {
            // Handle system tray events asynchronously
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let tray_state = app_handle.state::<Arc<RwLock<SystemTray>>>();
                let tray = tray_state.read().await;
                if let Err(e) = tray.handle_tray_event(event).await {
                    tracing::warn!("Failed to handle tray event: {}", e);
                }
            });
        })
        .setup(|app| {
            let app_handle = app.handle();
            
            // Initialize application asynchronously
            tauri::async_runtime::spawn(async move {
                if let Err(e) = initialize_application(app_handle).await {
                    error!("Failed to initialize application: {}", e);
                }
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn initialize_application(app_handle: tauri::AppHandle) -> Result<()> {
    info!("Starting ISP-SpeedKarma application");
    
    // Initialize database (file-based in user config dir)
    let db_path = std::env::temp_dir().join("speedkarma.db");
    let database_url = format!("sqlite://{}", db_path.display());
    let migration_manager = MigrationManager::new(database_url.clone());
    migration_manager.create_database_if_not_exists().await?;
    let pool = SqlitePool::connect(&database_url).await?;
    migration_manager.run_migrations(&pool).await?;

    let repository = Arc::new(Repository::new(pool));

    // Load app configuration (JSON-based intelligent defaults)
    let app_config = AppConfig::load().await?;
    app_config.validate()?;

    // Initialize system tray
    let mut system_tray = SystemTray::new();
    system_tray.initialize(app_handle.clone()).await?;
    
    // Store repository and system tray in app state for access from event handlers
    app_handle.manage(Arc::clone(&repository));
    app_handle.manage(Arc::new(RwLock::new(system_tray)));

    // Start passive background monitoring if enabled
    {
        let repo_for_monitor = Arc::clone(&repository);
        tokio::spawn(async move {
            let mut monitor = BackgroundMonitor::new(repo_for_monitor);
            if let Err(e) = monitor.start_monitoring().await {
                tracing::warn!("Failed to start background monitoring: {}", e);
            }
        });
    }

    // Perform ISP detection on startup (non-blocking) and save profile
    {
        let repo_for_detection = Arc::clone(&repository);
        tokio::spawn(async move {
            let monitor = BackgroundMonitor::new(Arc::clone(&repo_for_detection));
            match monitor.detect_isp().await {
                Ok(result) => {
                    if let Err(e) = monitor.save_isp_profile(&result).await {
                        tracing::warn!("Failed to save ISP profile: {}", e);
                    } else {
                        // Apply a sensible default optimization strategy based on detected ISP
                        // Only if no strategy exists yet
                        match repo_for_detection.get_best_optimization_strategy().await {
                            Ok(Some(_)) => {
                                // Strategy already exists; do nothing
                            }
                            Ok(None) | Err(_) => {
                                // Choose a default strategy depending on whether ISP is known to throttle
                                let isp_name_lower = result.isp_name.to_lowercase();
                                let throttling_isps = ["hutch", "dialog", "mobitel", "airtel"]; 
                                let mut strategy = if throttling_isps
                                    .iter()
                                    .any(|name| isp_name_lower.contains(name))
                                {
                                    OptimizationStrategy::high_stealth_strategy()
                                } else {
                                    OptimizationStrategy::default_strategy()
                                };

                                // Seed an initial effectiveness score to help selection later
                                if strategy.effectiveness_score.is_none() {
                                    strategy.effectiveness_score = Some(0.6);
                                }

                                if let Err(e) = repo_for_detection.save_optimization_strategy(&strategy).await {
                                    tracing::warn!("Failed to save initial optimization strategy: {}", e);
                                } else {
                                    tracing::info!(
                                        "Applied initial optimization strategy: {} (stealth: {:?})",
                                        strategy.name, strategy.stealth_level
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => tracing::warn!("ISP detection failed: {}", e),
            }
        });
    }

    // Start decision engine in background
    let repo_for_task = Arc::clone(&repository);
    let app_handle_for_task = app_handle.clone();
    tokio::spawn(async move {
        let mut engine = DecisionEngine::new(repo_for_task);
        
        // Status update loop
        let status_app_handle = app_handle_for_task.clone();
        let repo_for_status = Arc::clone(&engine.intelligence().repository);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Get status from intelligence core and update tray
                let tray_state = status_app_handle.state::<Arc<RwLock<SystemTray>>>();
                let tray = tray_state.read().await;
                // Compute current status from intelligence core
                let intelligence = DefaultIntelligenceCore::new(Arc::clone(&repo_for_status));
                let status = match intelligence.get_status().await {
                    Ok(s) => s,
                    Err(e) => crate::core::intelligence::SystemStatus {
                        state: crate::core::intelligence::SystemState::Error(e.to_string()),
                        message: "Error obtaining status".to_string(),
                        data_collection_progress: None,
                        effectiveness: None,
                    },
                };
                
                if let Err(e) = tray.update_status(status).await {
                    tracing::warn!("Failed to update tray status: {}", e);
                }
            }
        });
        
        // Decision engine loop
        if let Err(e) = engine.run().await {
            tracing::warn!("Decision engine stopped: {}", e);
        }
    });

    info!("ISP-SpeedKarma initialized successfully");
    Ok(())
}

// Entry point for non-mobile builds
#[tokio::main]
async fn main() {
    run();
}