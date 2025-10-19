use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Running,
    CompletedButUnread,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub started_at: DateTime<Utc>,
    pub log_id: String,
    pub log_path: String,
    #[serde(default)]
    pub manager_pid: Option<u32>,
    #[serde(default)]
    pub cleanup_reason: Option<String>,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub exit_code: Option<i32>,
}

impl TaskRecord {
    pub fn new(
        started_at: DateTime<Utc>,
        log_id: String,
        log_path: String,
        manager_pid: Option<u32>,
    ) -> Self {
        Self {
            started_at,
            log_id,
            log_path,
            manager_pid,
            cleanup_reason: None,
            status: TaskStatus::Running,
            result: None,
            completed_at: None,
            exit_code: None,
        }
    }

    pub fn mark_completed(
        mut self,
        result: Option<String>,
        exit_code: Option<i32>,
        completed_at: DateTime<Utc>,
    ) -> Self {
        self.status = TaskStatus::CompletedButUnread;
        self.result = result;
        self.exit_code = exit_code;
        self.completed_at = Some(completed_at);
        self
    }

    pub fn with_cleanup_reason(mut self, reason: &str) -> Self {
        let result = self.result.clone();
        let exit_code = self.exit_code;
        let completed_at = self.completed_at.unwrap_or_else(Utc::now);
        self = self.mark_completed(result, exit_code, completed_at);
        self.cleanup_reason = Some(reason.to_owned());
        self
    }
}
