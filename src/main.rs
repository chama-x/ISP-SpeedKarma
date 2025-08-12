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
use crate::core::app_state::{AppControlState, SharedAppState, OptimizationMode};
use crate::data::migrations::MigrationManager;
use crate::data::models::OptimizationStrategy;
use crate::data::repository::Repository;
use crate::ui::tray::SystemTray;
use crate::ui::panel::PanelInterface;
use crate::ui::progress::start_progress_broadcaster;
use crate::network::monitor::BackgroundMonitor;
use crate::network::{ThroughputKeeper, SpeedtestRunner, DisguiseProxy};
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
        .invoke_handler(tauri::generate_handler![
            toggle_optimization,
            get_optimization_state,
            get_system_status,
            open_advanced,
            quit_app,
            get_config,
            set_min_data_days,
            set_custom_servers,
            export_config,
            import_config,
            set_throughput_keeper,
            run_speedtest_once,
            set_disguise_mode,
        ])
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

#[tauri::command]
async fn toggle_optimization(app: tauri::AppHandle) -> std::result::Result<(), String> {
    let state = app.state::<crate::core::app_state::SharedAppState>();
    let mut guard = state.write().await;
    guard.optimization_mode = match guard.optimization_mode { OptimizationMode::Enabled => OptimizationMode::Disabled, OptimizationMode::Disabled => OptimizationMode::Enabled };
    // Start/stop throughput keeper for clarity, although it self-suspends when disabled
    if let Some(keeper) = app.try_state::<std::sync::Arc<ThroughputKeeper>>() {
        match guard.optimization_mode {
            OptimizationMode::Enabled => {
                // Restart loop if not running
                let k = std::sync::Arc::clone(&keeper);
                k.start();
            }
            OptimizationMode::Disabled => {
                keeper.stop().await;
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn get_optimization_state(app: tauri::AppHandle) -> std::result::Result<serde_json::Value, String> {
    let state = app.state::<crate::core::app_state::SharedAppState>();
    let guard = state.read().await;
    let mode = match guard.optimization_mode { OptimizationMode::Enabled => "Enabled", OptimizationMode::Disabled => "Disabled" };
    Ok(serde_json::json!({"mode": mode, "text": "Learning patterns"}))
}

#[tauri::command]
async fn get_system_status(app: tauri::AppHandle) -> std::result::Result<crate::core::intelligence::SystemStatus, String> {
    let tray_state = app.state::<Arc<RwLock<SystemTray>>>();
    let tray = tray_state.read().await;
    Ok(tray.get_current_status().await)
}

#[tauri::command]
async fn open_advanced(app: tauri::AppHandle) -> std::result::Result<(), String> {
    let tray = app.state::<Arc<RwLock<SystemTray>>>();
    let tray = tray.read().await;
    tray.show_advanced_interface().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn quit_app(app: tauri::AppHandle) -> std::result::Result<(), String> { app.exit(0); Ok(()) }

#[tauri::command]
async fn get_config(_app: tauri::AppHandle) -> std::result::Result<AppConfig, String> { AppConfig::load().await.map_err(|e| e.to_string()) }

#[tauri::command]
async fn set_min_data_days(_app: tauri::AppHandle, days: u32) -> std::result::Result<(), String> {
    let mut cfg = AppConfig::load().await.map_err(|e| e.to_string())?;
    cfg.auto_optimization.min_data_days = days;
    cfg.save().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_custom_servers(_app: tauri::AppHandle, servers: Vec<String>) -> std::result::Result<(), String> {
    let mut cfg = AppConfig::load().await.map_err(|e| e.to_string())?;
    cfg.advanced.custom_servers = servers;
    cfg.save().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn export_config(_app: tauri::AppHandle) -> std::result::Result<String, String> {
    let cfg = AppConfig::load().await.map_err(|e| e.to_string())?;
    serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_config(_app: tauri::AppHandle, json: String) -> std::result::Result<(), String> {
    let cfg: AppConfig = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    cfg.validate().map_err(|e| e.to_string())?;
    cfg.save().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_throughput_keeper(app: tauri::AppHandle, cfg: crate::core::config::ThroughputKeeperConfig) -> std::result::Result<(), String> {
    // Save to config file
    let mut full = AppConfig::load().await.map_err(|e| e.to_string())?;
    full.advanced.throughput_keeper = cfg.clone();
    full.save().await.map_err(|e| e.to_string())?;

    // Notify running keeper if present
    if let Some(keeper) = app.try_state::<std::sync::Arc<ThroughputKeeper>>() {
        keeper.update_config(cfg).await;
    }
    Ok(())
}

#[tauri::command]
async fn run_speedtest_once(app: tauri::AppHandle) -> std::result::Result<(), String> {
    let repo = app.state::<Arc<Repository>>();
    let shared = app.state::<SharedAppState>();
    let cfg = AppConfig::load().await.map_err(|e| e.to_string())?.advanced.speedtest_runner;
    let runner = SpeedtestRunner::new(app.clone(), Arc::clone(&repo), Arc::clone(&shared), cfg);
    tokio::spawn(async move { let _ = runner.run_once().await; });
    Ok(())
}

#[tauri::command]
async fn set_disguise_mode(app: tauri::AppHandle, enabled: bool) -> std::result::Result<(), String> {
    let mut cfg = AppConfig::load().await.map_err(|e| e.to_string())?;
    cfg.advanced.disguise_mode.enabled = enabled;
    cfg.save().await.map_err(|e| e.to_string())?;
    // Start/stop background disguise task
    if enabled {
        if let (Some(repo), Some(shared)) = (app.try_state::<Arc<Repository>>(), app.try_state::<SharedAppState>()) {
            let proxy = std::sync::Arc::new(DisguiseProxy::new(app.clone(), Arc::clone(&repo), Arc::clone(&shared), cfg.advanced.disguise_mode.clone()));
            proxy.clone().start();
            app.manage(proxy);
        }
    }
    Ok(())
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
    
    // Store repository, system tray and app control state in app state for access from event handlers
    app_handle.manage(Arc::clone(&repository));
    app_handle.manage(Arc::new(RwLock::new(system_tray)));
    let shared_state: SharedAppState = Arc::new(RwLock::new(AppControlState::default()));
    app_handle.manage(shared_state.clone());

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
        // Respect configurable data-days requirement
        engine.set_min_learning_days(app_config.auto_optimization.min_data_days);
        
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
                let intelligence = DefaultIntelligenceCore::with_min_learning_days(
                    Arc::clone(&repo_for_status),
                    app_config.auto_optimization.min_data_days,
                );
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

    // Start UI progress broadcaster (pushes optimization_progress events)
    {
        let repo_for_progress = Arc::clone(&repository);
        let app_for_progress = app_handle.clone();
        let shared_for_progress = shared_state.clone();
        start_progress_broadcaster(app_for_progress, repo_for_progress, shared_for_progress);
    }

    // Start ThroughputKeeper background task with safe defaults and live config
    {
        let cfg = app_config.advanced.throughput_keeper.clone();
        let keeper = std::sync::Arc::new(ThroughputKeeper::new(app_handle.clone(), Arc::clone(&repository), shared_state.clone(), cfg));
        keeper.clone().start();
        // Manage so we can update config later
        app_handle.manage(std::sync::Arc::clone(&keeper));
    }

    // Start disguise mode background if enabled
    if app_config.advanced.disguise_mode.enabled {
        let proxy = std::sync::Arc::new(DisguiseProxy::new(app_handle.clone(), Arc::clone(&repository), shared_state.clone(), app_config.advanced.disguise_mode.clone()));
        proxy.clone().start();
        app_handle.manage(proxy);
    }

    info!("ISP-SpeedKarma initialized successfully");
    Ok(())
}

// Entry point for non-mobile builds
#[tokio::main]
async fn main() {
    run();
}