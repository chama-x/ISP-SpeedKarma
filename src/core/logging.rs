use tracing_subscriber::fmt;

/// Initialize structured logging for tests and app runs
pub fn init_for_tests() { let _ = fmt().with_target(false).try_init(); }

/// Initialize compact logging suitable for CI
pub fn init_for_ci() { let _ = fmt().compact().try_init(); }


