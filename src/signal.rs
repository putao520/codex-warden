use crate::platform;
use std::io;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

static CHILD_PID: AtomicU32 = AtomicU32::new(0);
static INIT: Once = Once::new();

pub struct SignalGuard;

impl Drop for SignalGuard {
    fn drop(&mut self) {
        CHILD_PID.store(0, Ordering::SeqCst);
    }
}

pub fn install(child_pid: u32) -> io::Result<SignalGuard> {
    INIT.call_once(|| {
        #[cfg(unix)]
        unsafe {
            install_unix_handlers();
        }
        #[cfg(windows)]
        unsafe {
            install_windows_handler();
        }
    });
    CHILD_PID.store(child_pid, Ordering::SeqCst);
    Ok(SignalGuard)
}

#[cfg(unix)]
unsafe fn install_unix_handlers() {
    extern "C" fn handler(signum: libc::c_int) {
        match signum {
            libc::SIGINT | libc::SIGTERM => {
                let pid = CHILD_PID.load(Ordering::SeqCst);
                if pid != 0 {
                    platform::terminate_process(pid);
                }
            }
            _ => {}
        }
    }

    libc::signal(libc::SIGINT, handler as usize);
    libc::signal(libc::SIGTERM, handler as usize);
}

#[cfg(windows)]
unsafe fn install_windows_handler() {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::System::Console::{
        CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
    };

    unsafe extern "system" fn handler(ctrl_type: u32) -> BOOL {
        match ctrl_type {
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
                let pid = CHILD_PID.load(Ordering::SeqCst);
                if pid != 0 {
                    platform::terminate_process(pid);
                }
                BOOL(1)
            }
            _ => BOOL(0),
        }
    }

    let _ = unsafe { SetConsoleCtrlHandler(Some(handler), true) };
}
