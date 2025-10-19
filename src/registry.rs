use crate::config::{MAX_RECORD_AGE, SHARED_MEMORY_SIZE, SHARED_NAMESPACE};
use crate::logging::{debug, warn};
use crate::shared_map::{SharedMapError, open_or_create};
use crate::task_record::TaskRecord;
use chrono::{DateTime, Duration, Utc};
use shared_hashmap::SharedMemoryHashMap;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug)]
pub struct TaskRegistry {
    map: Mutex<SharedMemoryHashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub pid: u32,
    pub key: String,
    pub record: TaskRecord,
}

#[derive(Debug)]
pub struct CleanupEvent {
    pub _pid: u32,
    pub record: TaskRecord,
    pub reason: CleanupReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupReason {
    ProcessExited,
    Timeout,
    ManagerMissing,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("shared task map init failed: {0}")]
    Shared(#[from] SharedMapError),
    #[error("shared hashmap operation failed: {0}")]
    Map(String),
    #[error("registry mutex poisoned")]
    Poison,
    #[error("record serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

impl From<shared_hashmap::Error> for RegistryError {
    fn from(value: shared_hashmap::Error) -> Self {
        RegistryError::Map(value.to_string())
    }
}

impl TaskRegistry {
    pub fn connect() -> Result<Self, RegistryError> {
        let map = open_or_create(SHARED_NAMESPACE, SHARED_MEMORY_SIZE)?;
        Ok(Self {
            map: Mutex::new(map),
        })
    }

    pub fn register(&self, pid: u32, record: &TaskRecord) -> Result<(), RegistryError> {
        let key = pid.to_string();
        let value = serde_json::to_string(record)?;
        self.with_map(|map| {
            map.try_insert(key.clone(), value)?;
            Ok(())
        })
    }

    pub fn remove(&self, pid: u32) -> Result<Option<TaskRecord>, RegistryError> {
        let key = pid.to_string();
        let removed = self.with_map(|map| Ok(map.remove(&key)))?;
        match removed {
            Some(text) => Ok(Some(serde_json::from_str(&text)?)),
            None => Ok(None),
        }
    }

    pub fn entries(&self) -> Result<Vec<RegistryEntry>, RegistryError> {
        let snapshot: Vec<(String, String)> = {
            let guard = self.map.lock().map_err(|_| RegistryError::Poison)?;
            guard.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        };

        let mut entries = Vec::new();
        let mut invalid_keys = Vec::new();

        for (key, value) in snapshot {
            match key.parse::<u32>() {
                Ok(pid) => match serde_json::from_str::<TaskRecord>(&value) {
                    Ok(record) => entries.push(RegistryEntry { pid, key, record }),
                    Err(err) => {
                        warn(format!("failed to parse task record pid={key}: {err}"));
                        invalid_keys.push(key);
                    }
                },
                Err(_) => {
                    warn(format!("detected invalid pid key: {key}"));
                    invalid_keys.push(key);
                }
            }
        }

        if !invalid_keys.is_empty() {
            self.remove_keys(&invalid_keys)?;
        }

        Ok(entries)
    }

    pub fn sweep_stale_entries<F>(
        &self,
        now: DateTime<Utc>,
        process_alive: F,
        terminate: &dyn Fn(u32),
    ) -> Result<Vec<CleanupEvent>, RegistryError>
    where
        F: Fn(u32) -> bool,
    {
        let entries = self.entries()?;
        let mut removals = Vec::new();
        let mut events = Vec::new();

        for entry in entries {
            let mut reason = None;
            if !process_alive(entry.pid) {
                reason = Some(CleanupReason::ProcessExited);
            } else {
                if let Some(manager_pid) = entry
                    .record
                    .manager_pid
                    .filter(|&manager_pid| manager_pid != entry.pid && !process_alive(manager_pid))
                {
                    debug(format!(
                        "manager pid={manager_pid} missing, terminating Codex child pid={}",
                        entry.pid
                    ));
                    terminate(entry.pid);
                    reason = Some(CleanupReason::ManagerMissing);
                }
                if reason.is_none() {
                    let age = now.signed_duration_since(entry.record.started_at);
                    if age > Duration::from_std(MAX_RECORD_AGE).unwrap_or(Duration::zero()) {
                        debug(format!(
                            "pid={} exceeded age {:.1}h, performing timeout cleanup",
                            entry.pid,
                            age.num_minutes() as f64 / 60.0
                        ));
                        terminate(entry.pid);
                        reason = Some(CleanupReason::Timeout);
                    }
                }
            }

            if let Some(reason) = reason {
                removals.push(entry.key.clone());
                events.push(CleanupEvent {
                    _pid: entry.pid,
                    record: entry.record.with_cleanup_reason(match reason {
                        CleanupReason::ProcessExited => "process_exited",
                        CleanupReason::Timeout => "timeout_cleanup",
                        CleanupReason::ManagerMissing => "manager_missing",
                    }),
                    reason,
                });
            }
        }

        if !removals.is_empty() {
            self.remove_keys(&removals)?;
        }

        Ok(events)
    }

    fn remove_keys(&self, keys: &[String]) -> Result<(), RegistryError> {
        if keys.is_empty() {
            return Ok(());
        }
        self.with_map(|map| {
            for key in keys {
                map.remove(key);
            }
            Ok(())
        })
    }

    fn with_map<T>(
        &self,
        f: impl FnOnce(&mut SharedMemoryHashMap<String, String>) -> Result<T, RegistryError>,
    ) -> Result<T, RegistryError> {
        let mut guard = self.map.lock().map_err(|_| RegistryError::Poison)?;
        f(&mut guard)
    }
}
