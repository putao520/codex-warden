use std::time::Duration;

pub const CODEX_BIN: &str = "codex";
pub const SHARED_NAMESPACE: &str = "codex-task";
pub const SHARED_MEMORY_SIZE: usize = 4 * 1024 * 1024;
pub const WAIT_INTERVAL_ENV: &str = "CODEX_WORKER_WAIT_INTERVAL_SEC";
pub const DEBUG_ENV: &str = "CODEX_WORKER_DEBUG";

pub const MAX_RECORD_AGE: Duration = Duration::from_secs(12 * 60 * 60);
pub const WAIT_INTERVAL_DEFAULT: Duration = Duration::from_secs(30);
pub const MAX_WAIT_DURATION: Duration = Duration::from_secs(24 * 60 * 60);
