use crate::config::DEBUG_ENV;
use std::env;
use std::sync::OnceLock;

static DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

fn enabled() -> bool {
    *DEBUG_ENABLED.get_or_init(|| {
        env::var(DEBUG_ENV)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    })
}

pub fn debug(message: impl AsRef<str>) {
    if enabled() {
        eprintln!("[codex-warden][debug] {}", message.as_ref());
    }
}

pub fn warn(message: impl AsRef<str>) {
    eprintln!("[codex-warden][warn] {}", message.as_ref());
}
