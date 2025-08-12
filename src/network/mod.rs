pub mod monitor;
pub mod optimizer;
pub mod stealth;
pub mod servers;
pub mod keeper;
pub mod speedtest_runner;
pub mod disguise;

// Re-export commonly used types
pub use monitor::BackgroundMonitor;
pub use optimizer::NetworkOptimizer;
pub use stealth::StealthEngine;
pub use servers::ServerPool;
pub use keeper::ThroughputKeeper;
pub use speedtest_runner::SpeedtestRunner;
pub use disguise::DisguiseProxy;