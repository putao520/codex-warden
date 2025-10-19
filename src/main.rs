mod config;
mod logging;
mod platform;
mod registry;
mod shared_map;
mod signal;
mod supervisor;
mod task_record;
mod wait_mode;

use crate::config::CODEX_BIN;
use crate::registry::TaskRegistry;
use crate::supervisor::ProcessError;
use crate::wait_mode::WaitError;
use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::process::{Command, ExitCode};
use thiserror::Error;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from((code & 0xFF) as u8),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<i32, WorkerError> {
    let mut args_iter = env::args_os();
    args_iter.next(); // skip program name
    let args: Vec<OsString> = args_iter.collect();

    if args.is_empty() {
        return verify_codex();
    }

    if args.len() == 1
        && args[0]
            .to_str()
            .is_some_and(|cmd| cmd.eq_ignore_ascii_case("wait"))
    {
        wait_mode::run()?;
        return Ok(0);
    }

    let registry = TaskRegistry::connect()?;
    let exit_code = supervisor::execute_codex(&registry, &args)?;
    Ok(exit_code)
}

fn verify_codex() -> Result<i32, WorkerError> {
    let output = Command::new(CODEX_BIN).arg("--version").output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorkerError::VersionCheck(format!(
            "Codex version check failed: {}",
            stderr.trim()
        )));
    }
    io::stdout().write_all(&output.stdout)?;
    Ok(0)
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("{0}")]
    Message(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Task registry error: {0}")]
    Registry(#[from] registry::RegistryError),
    #[error("Codex execution failed: {0}")]
    Process(#[from] ProcessError),
    #[error("Wait mode failed: {0}")]
    Wait(#[from] WaitError),
    #[error("{0}")]
    VersionCheck(String),
}
