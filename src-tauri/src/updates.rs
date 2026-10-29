//! Silent updates for standalone installs. Flow checks GitHub Releases at start and
//! every 6 hours and downloads a signed update in the background. It never restarts
//! while it's open: the update is installed when you quit, and the next start is
//! the new version.
//! When launched from the Toolbay hub this is off: the hub updates Flow itself.

use crate::dictation::St;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::time::Duration;
use tauri_plugin_updater::{Update, UpdaterExt};

/// A downloaded update waiting for the next quit.
static PENDING: Lazy<Mutex<Option<(Update, Vec<u8>)>>> = Lazy::new(|| Mutex::new(None));

pub fn start(st: &St) {
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            if let Err(e) = check_and_download(&st).await {
                log::info!("update check: {e:#}");
            }
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

async fn check_and_download(st: &St) -> anyhow::Result<()> {
    let Some(update) = st.app.updater()?.check().await? else { return Ok(()) };
    if PENDING.lock().as_ref().is_some_and(|(u, _)| u.version == update.version) {
        return Ok(());
    }
    log::info!("update {} available", update.version);
    let bytes = update.download(|_, _| {}, || {}).await?;
    log::info!("update {} downloaded; installs when Flow quits", update.version);
    *PENDING.lock() = Some((update, bytes));
    Ok(())
}

/// Called on quit. On Windows the installer takes over and this process exits here.
pub fn install_pending() {
    let Some((update, bytes)) = PENDING.lock().take() else { return };
    log::info!("installing update {}", update.version);
    if let Err(e) = update.install(bytes) {
        log::warn!("update install failed: {e:#}");
    }
}
