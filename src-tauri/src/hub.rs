//! Living under the Toolbay hub: show/hide/quit requests, running state, the
//! signed-in Toolbay account, and the desktop shortcut.
//!
//! Control: the hub (or anyone) runs `toolbay-flow.exe --show | --hide | --quit`.
//! Flow is single-instance, so that second process forwards its arguments to the
//! running one and exits. `--hide` minimizes the window; nothing runs in the
//! background, so closing the main window quits Flow.
//!
//! State: `<app data>/running.json` = {pid, version, visible, window, updatedAt, hub}, rewritten on
//! every show/hide and removed on quit.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

pub fn handle_args(app: &AppHandle, args: &[String]) {
    if args.iter().any(|a| a == "--quit") {
        quit(app);
    } else if args.iter().any(|a| a == "--hide") {
        // Minimize, never hide: a hidden window would leave Flow running unseen.
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.minimize();
        }
        write_state(app);
    } else {
        crate::show_hub(app, None);
    }
}

static QUITTING: AtomicBool = AtomicBool::new(false);
static QUIT_DONE: AtomicBool = AtomicBool::new(false);

/// Quits Flow. The main window first gets "flush-before-quit" so it can save
/// edits still waiting on a timer (the Scratchpad, settings fields); it answers
/// with `quit_flushed`. If it doesn't within 2 seconds, Flow quits anyway.
pub fn quit(app: &AppHandle) {
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    let ready = crate::UI_READY.load(Ordering::SeqCst);
    if !ready || app.get_webview_window("main").is_none() || app.emit_to("main", "flush-before-quit", ()).is_err() {
        finish_quit(app);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        finish_quit(&app);
    });
}

/// The window saved what it had (or the 2 seconds ran out): install a downloaded
/// update if there is one, then exit.
pub fn finish_quit(app: &AppHandle) {
    if QUIT_DONE.swap(true, Ordering::SeqCst) {
        return;
    }
    clear_state(app);
    crate::updates::install_pending();
    app.exit(0);
}

/// Quitting has started (the window close is turned into a flush, then a quit).
pub fn quitting() -> bool {
    QUITTING.load(Ordering::SeqCst)
}

/// Removes running.json (the app is no longer running).
pub fn clear_state(app: &AppHandle) {
    let _ = std::fs::remove_file(state_path(app));
}

fn state_path(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("running.json")
}

pub fn write_state(app: &AppHandle) {
    let visible = app.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false);
    let v = serde_json::json!({
        "pid": std::process::id(),
        "version": app.package_info().version.to_string(),
        "visible": visible,
        "window": if visible { "shown" } else { "hidden" },
        "updatedAt": chrono::Utc::now().to_rfc3339(),
        "hub": from_hub(),
    });
    let _ = std::fs::write(state_path(app), v.to_string());
}

/// Started by the Toolbay hub (it sets TOOLBAY_HUB=1 and passes --from-toolbay).
pub fn from_hub() -> bool {
    std::env::var("TOOLBAY_HUB").map(|v| v == "1").unwrap_or(false) || std::env::args().any(|a| a == "--from-toolbay")
}

#[derive(Serialize, Clone, Default)]
pub struct Account {
    pub name: String,
    pub handle: String,
    pub avatar_url: Option<String>,
}

/// The hub passes the user's toolbay.ai token in TOOLBAY_TOKEN. It's only held in
/// memory and only sent to toolbay.ai.
pub async fn account() -> Option<Account> {
    let token = std::env::var("TOOLBAY_TOKEN").ok().filter(|t| !t.is_empty())?;
    let r = reqwest::Client::new()
        .get("https://toolbay.ai/api/v1/me")
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .ok()?;
    let v: serde_json::Value = r.error_for_status().ok()?.json().await.ok()?;
    let u = v.get("user").unwrap_or(&v);
    Some(Account {
        name: u["name"].as_str().unwrap_or_default().to_string(),
        handle: u["handle"].as_str().unwrap_or_default().to_string(),
        avatar_url: u["avatarUrl"].as_str().map(|s| s.to_string()),
    })
}

/// Creates or removes the "Toolbay Flow" shortcut on the desktop.
pub fn set_desktop_shortcut(on: bool) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    #[cfg(windows)]
    {
        let desktop = desktop_dir().ok_or_else(|| anyhow::anyhow!("no desktop folder"))?;
        let lnk = desktop.join("Toolbay Flow.lnk");
        if on {
            let l = mslnk::ShellLink::new(&exe)?;
            l.create_lnk(&lnk)?;
        } else if lnk.exists() {
            std::fs::remove_file(lnk)?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        // exe is .../Toolbay Flow.app/Contents/MacOS/toolbay-flow
        let app_bundle = exe.ancestors().nth(3).map(|p| p.to_path_buf()).ok_or_else(|| anyhow::anyhow!("no app bundle"))?;
        let link = desktop_dir().ok_or_else(|| anyhow::anyhow!("no desktop folder"))?.join("Toolbay Flow");
        if on {
            if !link.exists() {
                std::os::unix::fs::symlink(&app_bundle, &link)?;
            }
        } else if link.exists() {
            std::fs::remove_file(link)?;
        }
    }
    let _ = exe;
    Ok(())
}

/// Is there a Toolbay Flow shortcut on the desktop? (The installer's checkbox makes one.)
pub fn has_desktop_shortcut() -> bool {
    desktop_dir().map(|d| d.join(if cfg!(windows) { "Toolbay Flow.lnk" } else { "Toolbay Flow" }).exists()).unwrap_or(false)
}

fn desktop_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Desktop")).filter(|p| p.exists()).or_else(|| {
            std::env::var_os("OneDrive").map(|o| PathBuf::from(o).join("Desktop")).filter(|p| p.exists())
        })
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Desktop"))
    }
}
