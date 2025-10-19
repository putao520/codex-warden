use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub started_at: DateTime<Utc>,
    pub log_id: String,
    pub log_path: String,
    #[serde(default)]
    pub manager_pid: Option<u32>,
    #[serde(default)]
    pub cleanup_reason: Option<String>,
}

impl TaskRecord {
    pub fn with_cleanup_reason(mut self, reason: &str) -> Self {
        self.cleanup_reason = Some(reason.to_owned());
        self
    }
}
