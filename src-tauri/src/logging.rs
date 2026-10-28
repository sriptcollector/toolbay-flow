//! A small local log file (flow.log in the app data folder, kept under ~2 MB) and
//! crash recovery: panics are logged, and on Windows the app asks to be restarted
//! after a crash. Logs never leave the computer.

use parking_lot::Mutex;
use std::io::Write;
use std::path::{Path, PathBuf};

struct FileLog {
    path: PathBuf,
    lock: Mutex<()>,
}

impl log::Log for FileLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let _g = self.lock.lock();
        if std::fs::metadata(&self.path).map(|m| m.len() > 2_000_000).unwrap_or(false) {
            let _ = std::fs::rename(&self.path, self.path.with_extension("old.log"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = writeln!(f, "{} [{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), r.level(), r.args());
        }
    }
    fn flush(&self) {}
}

pub fn init(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    let logger = Box::leak(Box::new(FileLog { path: dir.join("flow.log"), lock: Mutex::new(()) }));
    let _ = log::set_logger(logger).map(|()| log::set_max_level(log::LevelFilter::Info));
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("crash: {info}");
        prev(info);
    }));
    #[cfg(windows)]
    unsafe {
        // Windows reopens Flow (window shown) if it crashes or hangs (after it has run for a minute).
        use windows::Win32::System::Recovery::{RegisterApplicationRestart, REGISTER_APPLICATION_RESTART_FLAGS};
        let _ = RegisterApplicationRestart(windows::core::PCWSTR::null(), REGISTER_APPLICATION_RESTART_FLAGS(0));
    }
    log::info!("Toolbay Flow {} starting", env!("CARGO_PKG_VERSION"));
}
