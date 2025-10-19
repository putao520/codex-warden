use crate::config::{
    LEGACY_WAIT_INTERVAL_ENV, MAX_WAIT_DURATION, WAIT_INTERVAL_DEFAULT, WAIT_INTERVAL_ENV,
};
use crate::logging::warn;
use crate::platform;
use crate::registry::{CleanupReason, RegistryEntry, RegistryError, TaskRegistry};
use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WaitError {
    #[error("registry error: {0}")]
    Registry(#[from] RegistryError),
}

pub fn run() -> Result<(), WaitError> {
    let registry = TaskRegistry::connect()?;
    let interval = read_interval();
    let mut previous: HashMap<u32, String> = HashMap::new();
    let mut completed: Vec<String> = Vec::new();
    let mut completed_set: HashSet<String> = HashSet::new();
    let start = Instant::now();

    loop {
        let now = chrono::Utc::now();
        let cleanups = registry.sweep_stale_entries(
            now,
            platform::process_alive,
            &platform::terminate_process,
        )?;
        for event in cleanups {
            if event.reason == CleanupReason::Timeout {
                continue;
            }
            if completed_set.insert(event.record.log_path.clone()) {
                completed.push(event.record.log_path.clone());
            }
        }

        let entries = registry.entries()?;
        let current_map: HashMap<u32, String> = entries
            .iter()
            .map(|entry| (entry.pid, entry.record.log_path.clone()))
            .collect();

        for (pid, log_path) in previous.iter() {
            if !current_map.contains_key(pid) && completed_set.insert(log_path.clone()) {
                completed.push(log_path.clone());
            }
        }

        if current_map.is_empty() {
            print_completed(&completed);
            return Ok(());
        }

        if start.elapsed() >= MAX_WAIT_DURATION {
            print_timeout(&entries);
            return Ok(());
        }

        previous = current_map;
        thread::sleep(interval);
    }
}

fn read_interval() -> Duration {
    read_env_interval(WAIT_INTERVAL_ENV)
        .or_else(|| read_env_interval(LEGACY_WAIT_INTERVAL_ENV))
        .unwrap_or(WAIT_INTERVAL_DEFAULT)
}

fn read_env_interval(var: &str) -> Option<Duration> {
    match std::env::var(var) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(seconds) if seconds > 0 => Some(Duration::from_secs(seconds)),
            _ => {
                warn(format!(
                    "environment variable {var} invalid, using default 30s"
                ));
                None
            }
        },
        Err(_) => None,
    }
}

fn print_completed(paths: &[String]) {
    println!("{} tasks finished during this wait:", paths.len());
    for (idx, path) in paths.iter().enumerate() {
        println!("{}. {}", idx + 1, path);
    }
    println!("Review logs above and continue with the next steps.");
}

fn print_timeout(entries: &[RegistryEntry]) {
    println!("Reached wait timeout; unfinished tasks:");
    for entry in entries {
        println!("- PID {} -> {}", entry.pid, entry.record.log_path);
    }
}
