//! Silent updates for standalone installs. Flow checks GitHub Releases at start and
//! every 6 hours, downloads a signed update in the background, and installs it only
//! when you haven't dictated for a while, so it never interrupts you.
//! When launched from the Toolbay hub this is off: the hub updates Flow itself.

use crate::dictation::St;
use std::time::Duration;
use tauri_plugin_updater::UpdaterExt;

pub fn start(st: &St) {
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            if let Err(e) = check_and_install(&st).await {
                log::info!("update check: {e:#}");
            }
            tokio::time::sleep(Duration::from_secs(6 * 3600)).await;
        }
    });
}

async fn check_and_install(st: &St) -> anyhow::Result<()> {
    let Some(update) = st.app.updater()?.check().await? else { return Ok(()) };
    log::info!("update {} available", update.version);
    let bytes = update.download(|_, _| {}, || {}).await?;
    // Wait until Flow has been idle for 10 minutes.
    loop {
        if crate::dictation::idle_for(st) > Duration::from_secs(600) {
            break;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
    log::info!("installing update {}", update.version);
    update.install(bytes)?;
    st.app.restart();
}
