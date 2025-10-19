use crate::logging::debug;
use libc::{c_int, pid_t};
use std::io;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub fn prepare_command(cmd: &mut Command) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            #[cfg(target_os = "linux")]
            {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(())
}

pub fn process_alive(pid: u32) -> bool {
    let pid = pid as pid_t;
    unsafe { libc::kill(pid, 0) == 0 }
    || last_errno() == libc::EPERM
}

pub fn terminate_process(pid: u32) {
    fn send(pid: pid_t, signal: c_int) -> bool {
        unsafe { libc::kill(pid, signal) == 0 }
    }

    let pid = pid as pid_t;
    if !process_alive(pid as u32) {
        return;
    }

    if send(pid, libc::SIGTERM) {
        thread::sleep(Duration::from_millis(500));
        if !process_alive(pid as u32) {
            return;
        }
    }

    if send(pid, libc::SIGKILL) {
        debug(format!("å?pid={} å‘é€?SIGKILL", pid));
    }
}

#[inline]
fn last_errno() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe {
        *libc::__errno_location()
    }

    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    unsafe {
        *libc::__error()
    }
}
