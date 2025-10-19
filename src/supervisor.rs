use crate::config::CODEX_BIN;
use crate::logging::debug;
use crate::platform::{self, ChildResources};
use crate::registry::{RegistryError, TaskRegistry};
use crate::signal;
use crate::task_record::TaskRecord;
use chrono::Utc;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::{self, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Registry error: {0}")]
    Registry(#[from] RegistryError),
}

pub fn execute_codex(registry: &TaskRegistry, args: &[OsString]) -> Result<i32, ProcessError> {
    platform::init_platform();

    registry.sweep_stale_entries(
        Utc::now(),
        platform::process_alive,
        &platform::terminate_process,
    )?;

    let should_register = args
        .first()
        .and_then(|arg| arg.to_str())
        .map(|s| s.eq_ignore_ascii_case("exec"))
        .unwrap_or(false);

    let log_id = Uuid::new_v4().to_string();
    let log_path = generate_log_path(&log_id)?;

    let log_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&log_path)?;

    let mut command = Command::new(CODEX_BIN);
    command.args(args);
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    platform::prepare_command(&mut command)?;

    let mut child = command.spawn()?;
    let child_pid = child.id();
    debug(format!(
        "Started Codex process pid={} log={}",
        child_pid,
        log_path.display()
    ));

    let _resources: ChildResources = platform::after_spawn(&child)?;
    let signal_guard = signal::install(child_pid)?;

    let log_writer = Arc::new(Mutex::new(BufWriter::new(log_file)));
    let mut copy_handles = Vec::new();

    if let Some(stdout) = child.stdout.take() {
        copy_handles.push(spawn_copy(stdout, log_writer.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        copy_handles.push(spawn_copy(stderr, log_writer.clone()));
    }

    let registration_guard = if should_register {
        let record = TaskRecord {
            started_at: Utc::now(),
            log_id: log_id.clone(),
            log_path: log_path.to_string_lossy().into_owned(),
            manager_pid: Some(platform::current_pid()),
            cleanup_reason: None,
        };
        if let Err(err) = registry.register(child_pid, &record) {
            platform::terminate_process(child_pid);
            let _ = child.wait();
            return Err(err.into());
        }
        Some(RegistrationGuard::new(registry, child_pid))
    } else {
        None
    };

    let status = child.wait()?;
    drop(signal_guard);

    for handle in copy_handles {
        match handle.join() {
            Ok(result) => result?,
            Err(_) => {
                return Err(io::Error::other("Log writer thread failed").into());
            }
        }
    }

    {
        let mut writer = log_writer
            .lock()
            .map_err(|_| io::Error::other("Log writer lock poisoned"))?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
    }

    if let Some(guard) = registration_guard {
        let _ = guard.complete();
    }

    Ok(extract_exit_code(status))
}

fn generate_log_path(log_id: &str) -> io::Result<PathBuf> {
    let tmp = std::env::temp_dir();
    Ok(tmp.join(format!("{log_id}.txt")))
}

fn spawn_copy<R>(
    mut reader: R,
    writer: Arc<Mutex<BufWriter<std::fs::File>>>,
) -> thread::JoinHandle<io::Result<()>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            let mut guard = writer
                .lock()
                .map_err(|_| io::Error::other("Log writer lock poisoned"))?;
            guard.write_all(&buffer[..read])?;
            guard.flush()?;
        }
        Ok(())
    })
}

fn extract_exit_code(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

struct RegistrationGuard<'a> {
    registry: &'a TaskRegistry,
    pid: u32,
    active: bool,
}

impl<'a> RegistrationGuard<'a> {
    fn new(registry: &'a TaskRegistry, pid: u32) -> Self {
        Self {
            registry,
            pid,
            active: true,
        }
    }

    fn complete(mut self) -> Result<(), RegistryError> {
        if self.active {
            let _ = self.registry.remove(self.pid)?;
            self.active = false;
        }
        Ok(())
    }
}

impl Drop for RegistrationGuard<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.registry.remove(self.pid);
        }
    }
}
