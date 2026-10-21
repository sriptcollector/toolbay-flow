//! Commands the React UI calls.

use crate::db::{DictEntry, HistoryItem, Note, Stats};
use crate::dictation::{self, St};
use crate::engines;
use crate::settings::Settings;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};
#[allow(unused_imports)]
use chrono;

type R<T> = Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

#[tauri::command]
pub fn get_settings(st: State<St>) -> Settings {
    st.settings.read().clone()
}

#[tauri::command]
pub fn save_settings(st: State<St>, settings: Settings) -> R<()> {
    dictation::apply_settings(&st, settings).map_err(e)?;
    crate::refresh_tray(&st.app);
    Ok(())
}

#[tauri::command]
pub fn default_shortcuts() -> Vec<crate::settings::Shortcut> {
    crate::settings::default_shortcuts()
}

#[derive(Serialize)]
pub struct ModelRow {
    #[serde(flatten)]
    info: engines::ModelInfo,
    downloaded: bool,
}

#[tauri::command]
pub fn list_models(st: State<St>) -> Vec<ModelRow> {
    engines::MODELS.iter().map(|m| ModelRow { info: m.clone(), downloaded: st.paths.model_path(m).exists() }).collect()
}

#[derive(Serialize)]
pub struct Hardware {
    nvidia: bool,
    threads: usize,
    platform: &'static str,
    speech_engine: Option<String>,
    cleanup_engine: Option<String>,
    speech_engine_ready: bool,
    cleanup_engine_ready: bool,
    whisper_running: bool,
    llama_running: bool,
    data_dir: String,
}

#[tauri::command]
pub fn hardware(st: State<St>) -> Hardware {
    let gpu = st.settings.read().use_gpu;
    let w = engines::pick_engine("whisper", gpu);
    let l = engines::pick_engine("llama", gpu);
    Hardware {
        nvidia: engines::has_nvidia(),
        threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
        platform: std::env::consts::OS,
        speech_engine: w.map(|e| e.label.to_string()),
        cleanup_engine: l.map(|e| e.label.to_string()),
        speech_engine_ready: w.and_then(|e| st.paths.engine_exe(e)).is_some(),
        cleanup_engine_ready: l.and_then(|e| st.paths.engine_exe(e)).is_some(),
        whisper_running: st.engines.whisper.lock().as_mut().map(|s| s.alive()).unwrap_or(false),
        llama_running: st.engines.llama.lock().as_mut().map(|s| s.alive()).unwrap_or(false),
        data_dir: st.data_dir.to_string_lossy().to_string(),
    }
}

#[derive(Serialize)]
pub struct AutoInfo {
    hardware: crate::models::Hardware,
    choice: crate::models::Choice,
    mode: String,
    speech_in_use: Option<String>,
}

#[tauri::command]
pub fn auto_info(st: State<St>) -> AutoInfo {
    let hw = crate::models::detect();
    let lang = st.settings.read().language.clone();
    AutoInfo {
        choice: crate::models::recommend(&hw, &lang),
        hardware: hw,
        mode: st.settings.read().model_mode.clone(),
        speech_in_use: crate::models::current_speech_path(&st).map(|x| x.0),
    }
}

#[tauri::command]
pub fn rerun_auto(st: State<St>) {
    crate::models::auto_setup(&st);
}

#[tauri::command]
pub async fn download_model(st: State<'_, St>, id: String) -> R<()> {
    engines::ensure_model(&st.app, &st.paths, &id).await.map_err(e)?;
    Ok(())
}

#[tauri::command]
pub async fn download_engines(st: State<'_, St>, kinds: Vec<String>) -> R<()> {
    let gpu = st.settings.read().use_gpu;
    for k in kinds {
        if let Some(eng) = engines::pick_engine(&k, gpu) {
            engines::ensure_engine(&st.app, &st.paths, eng).await.map_err(e)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn delete_model(st: State<St>, id: String) -> R<()> {
    let m = engines::model(&id).ok_or("unknown model")?;
    if m.kind == "speech" {
        *st.engines.whisper.lock() = None;
    } else {
        *st.engines.llama.lock() = None;
    }
    std::fs::remove_file(st.paths.model_path(m)).map_err(e)
}

#[tauri::command]
pub async fn start_engines(st: State<'_, St>) -> R<()> {
    let st = st.inner().clone();
    dictation::ensure_whisper(&st).await.map_err(e)?;
    let level = st.settings.read().cleanup_level.clone();
    let provider = st.settings.read().llm_provider.clone();
    if matches!(level, crate::settings::CleanupLevel::Medium | crate::settings::CleanupLevel::High) && provider == crate::settings::LlmProvider::Local {
        dictation::ensure_llama(&st).await.map_err(e)?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_mics() -> Vec<String> {
    crate::audio::list_devices()
}

#[tauri::command]
pub fn mic_test(st: State<St>, on: bool) -> R<()> {
    dictation::mic_test(&st, on).map_err(e)
}

#[tauri::command]
pub fn history_list(st: State<St>, query: String, limit: i64, offset: i64) -> R<Vec<HistoryItem>> {
    st.db.list_history(&query, limit, offset).map_err(e)
}

#[tauri::command]
pub fn history_delete(st: State<St>, id: i64) -> R<()> {
    st.db.delete_history(id).map_err(e)
}

#[tauri::command]
pub fn history_clear(st: State<St>) -> R<()> {
    st.db.clear_history().map_err(e)
}

#[tauri::command]
pub fn history_audio(st: State<St>, id: i64) -> R<Vec<u8>> {
    st.db.get_audio(id).map_err(e)?.ok_or_else(|| "No audio saved for this dictation".to_string())
}

#[tauri::command]
pub async fn history_retry(st: State<'_, St>, id: i64) -> R<String> {
    let st = st.inner().clone();
    dictation::retry(&st, id).await.map_err(e)
}

#[tauri::command]
pub fn copy_text(text: String) -> R<()> {
    crate::paste::copy(&text).map_err(e)
}

#[tauri::command]
pub fn dict_list(st: State<St>, snippets: bool) -> R<Vec<DictEntry>> {
    st.db.list_dict(snippets).map_err(e)
}

#[tauri::command]
pub fn dict_save(st: State<St>, entry: DictEntry) -> R<i64> {
    st.db.upsert_dict(&entry).map_err(e)
}

/// Reads a dictionary (CSV) or snippets (JSON) file and says what would be added.
#[tauri::command]
pub fn dict_import_preview(st: State<St>, text: String, snippets: bool) -> R<crate::import::ImportPreview> {
    let existing: Vec<String> = st.db.list_dict(snippets).map_err(e)?.into_iter().map(|d| d.phrase).collect();
    Ok(if snippets { crate::import::parse_snippets_json(&text, &existing) } else { crate::import::parse_dictionary_csv(&text, &existing) })
}

#[tauri::command]
pub fn dict_import(st: State<St>, items: Vec<crate::import::ImportItem>, snippets: bool) -> R<usize> {
    for it in &items {
        let d = DictEntry { phrase: it.phrase.clone(), replacement: it.replacement.clone(), is_snippet: snippets, source: "import".into(), ..Default::default() };
        st.db.upsert_dict(&d).map_err(e)?;
    }
    let _ = st.app.emit("dictionary-changed", ());
    Ok(items.len())
}

#[tauri::command]
pub fn dict_delete(st: State<St>, id: i64) -> R<()> {
    st.db.delete_dict(id).map_err(e)
}

#[tauri::command]
pub fn undo_learned(st: State<St>) -> R<()> {
    let ids = std::mem::take(&mut *st.last_learned.lock());
    for id in ids {
        st.db.delete_dict(id).map_err(e)?;
    }
    let _ = st.app.emit("dictionary-changed", ());
    Ok(())
}

#[tauri::command]
pub fn notes_list(st: State<St>) -> R<Vec<Note>> {
    st.db.list_notes().map_err(e)
}

#[tauri::command]
pub fn note_save(st: State<St>, note: Note) -> R<i64> {
    st.db.save_note(&note).map_err(e)
}

#[tauri::command]
pub fn note_delete(st: State<St>, id: i64) -> R<()> {
    st.db.delete_note(id).map_err(e)
}

#[tauri::command]
pub fn stats(st: State<St>) -> R<Stats> {
    st.db.stats().map_err(e)
}

#[tauri::command]
pub fn record_shortcut(st: State<St>, on: bool) {
    st.hk.recording.store(on, Ordering::Relaxed);
}

/// Runs the cleanup pass on typed text (for previews and the try-it screen).
#[tauri::command]
pub async fn preview_cleanup(st: State<'_, St>, text: String, app_name: String) -> R<String> {
    let st = st.inner().clone();
    let ctx = crate::context::FocusContext { app: app_name.clone(), exe: app_name, ..Default::default() };
    dictation::format_text(&st, &text, &ctx).await.map_err(e)
}

#[tauri::command]
pub fn bar_action(st: State<St>, action: String) {
    let st = st.inner().clone();
    match action.as_str() {
        "start_handsfree" => dictation::start(&st, dictation::Mode::HandsFree),
        "stop" => dictation::stop(&st),
        "cancel" => dictation::cancel(&st),
        "open" => crate::show_hub(&st.app, None),
        "settings" => crate::show_hub(&st.app, Some("settings")),
        // The bar's menu: give focus back to the app under it before pasting.
        "paste_last" => {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(150));
                dictation::paste_last(&st);
            });
        }
        "undo_cancel" => dictation::recover(&st),
        "open_settings" => crate::show_hub(&st.app, Some("settings")),
        "hide_1h" => {
            let mut s = st.settings.read().clone();
            s.bar_snooze_until = chrono::Utc::now().timestamp() + 3600;
            let _ = dictation::apply_settings(&st, s);
            st.notice_with("The bar is hidden for an hour", "Undo", "show_bar", 6000);
        }
        "show_bar" => {
            let mut s = st.settings.read().clone();
            s.bar_snooze_until = 0;
            let _ = dictation::apply_settings(&st, s);
        }
        "history" => crate::show_hub(&st.app, Some("home")),
        _ => {}
    }
}

#[tauri::command]
pub fn bar_hit(rects: Vec<[f64; 4]>) {
    *crate::BAR_HIT.lock() = rects;
}

/// Keystrokes from Flow's own windows, so hotkeys work while Flow is focused.
#[tauri::command]
pub fn own_key(vk: u32, down: bool) {
    crate::hotkeys::feed(vk, down);
}

#[tauri::command]
pub fn ui_ready(app: AppHandle) {
    crate::ui_ready(&app);
}

#[tauri::command]
pub fn open_hub(app: AppHandle, page: Option<String>) {
    crate::show_hub(&app, page.as_deref());
}

#[tauri::command]
pub fn hotkeys_enabled(st: State<St>, on: bool) {
    dictation::hk_enabled(&st, on);
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> serde_json::Value {
    serde_json::json!({
        "version": app.package_info().version.to_string(),
        "fromToolbay": crate::hub::from_hub(),
        "os": std::env::consts::OS,
    })
}

#[tauri::command]
pub fn open_data_dir(st: State<St>) {
    use tauri_plugin_opener::OpenerExt;
    let _ = st.app.opener().open_path(st.data_dir.to_string_lossy().to_string(), None::<&str>);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    crate::hub::quit(&app);
}

#[tauri::command]
pub async fn toolbay_account() -> Option<crate::hub::Account> {
    crate::hub::account().await
}

#[tauri::command]
pub fn sync_status() -> crate::sync::Status {
    crate::sync::flow::get().map(|s| s.status()).unwrap_or_default()
}

#[tauri::command]
pub async fn sync_now() -> crate::sync::Status {
    let Some(s) = crate::sync::flow::get() else { return Default::default() };
    s.sync_once().await;
    s.status()
}

/// Sign-in belongs to the Toolbay app: toolbay://signin asks it to run its own
/// browser sign-in (or just comes forward if already signed in). Flow then picks
/// up that sign-in from the keychain.
#[tauri::command]
pub async fn sync_sign_in(app: AppHandle) -> R<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url("toolbay://signin", None::<&str>)
        .map_err(|_| "Sign-in happens in the Toolbay app. Install it from toolbay.ai, sign in there, and Toolbay Flow picks it up.".to_string())?;
    tauri::async_runtime::spawn(async {
        // Watch for the sign-in for a few minutes.
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_secs(4)).await;
            if crate::sync::signed_in_here() {
                if let Some(s) = crate::sync::flow::get() {
                    s.sync_once().await;
                    if s.status().account.is_some() {
                        break;
                    }
                }
            }
        }
    });
    Ok(())
}

/// Removes Flow's synced copy from the Toolbay account and stops syncing on this computer.
#[tauri::command]
pub async fn sync_delete_remote(st: State<'_, St>) -> R<()> {
    let st = st.inner().clone();
    let mut s = st.settings.read().clone();
    s.sync_enabled = false;
    dictation::apply_settings(&st, s).map_err(e)?;
    let _ = st.app.emit("settings-changed", ());
    let sync = crate::sync::flow::get().ok_or("Sync isn't running.")?;
    sync.delete_remote().await.map_err(|err| err.to_string())?;
    sync.sync_once().await;
    Ok(())
}

/// Sends feedback to the Toolbay Flow space on toolbay.ai. Returns the post's
/// address. Debug info (versions, models, recent warnings; never text, audio or
/// keys) is only added when the person ticks the box, and only the maker sees it.
#[tauri::command]
pub async fn send_feedback(st: State<'_, St>, kind: String, text: String, debug: bool, public: bool) -> R<String> {
    let st = st.inner().clone();
    let s = st.settings.read().clone();
    let mut body = serde_json::json!({
        "type": kind,
        "text": text.trim(),
        "appVersion": st.app.package_info().version.to_string(),
        "os": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        "visibility": if public { "public" } else { "maker" },
    });
    if debug {
        let log = std::fs::read_to_string(st.data_dir.join("flow.log")).unwrap_or_default();
        let warnings: Vec<&str> = log.lines().filter(|l| l.contains("WARN") || l.contains("ERROR")).collect();
        let tail: Vec<String> = warnings.iter().rev().take(30).rev().map(|l| l.chars().take(300).collect()).collect();
        body["debug"] = serde_json::json!({
            "engine": s.engine,
            "modelMode": s.model_mode,
            "speechModel": s.speech_model,
            "cleanupModel": s.local_llm_model,
            "cleanup": s.cleanup_level,
            "gpu": s.use_gpu,
            "chosenFor": s.auto_reason,
            "language": s.language,
            "fromToolbayApp": crate::hub::from_hub(),
            "recentWarnings": tail,
        });
    }
    let res = crate::sync::post_signed_in("/api/v1/tools/toolbay-flow/feedback", &body)
        .await
        .map_err(|_| "Couldn't reach toolbay.ai. Check your internet connection and try again.".to_string())?;
    let Some((code, v)) = res else {
        return Err("signin".into());
    };
    match code {
        200 | 201 => Ok(format!("https://toolbay.ai{}", v["url"].as_str().unwrap_or("/tools/toolbay-flow/community"))),
        403 | 401 => Err("signin".into()),
        _ => Err(v["error"].as_str().map(String::from).unwrap_or_else(|| "That didn't send. Please try again in a minute.".into())),
    }
}

#[derive(Serialize)]
pub struct MicCheck {
    ok: bool,
    message: String,
}

/// Can Flow open the microphone? Explains what to do when Windows blocks it.
#[tauri::command]
pub fn mic_check(st: State<St>) -> MicCheck {
    if st.mic_test.load(Ordering::Relaxed) {
        return MicCheck { ok: true, message: String::new() };
    }
    let dev = st.settings.read().microphone.clone();
    let mut r = crate::audio::Recorder::new();
    match r.start(dev, std::sync::Arc::new(|_| {})) {
        Ok(()) => {
            let _ = r.stop();
            r.close();
            MicCheck { ok: true, message: String::new() }
        }
        Err(err) => MicCheck {
            ok: false,
            message: if cfg!(windows) {
                format!("Windows is blocking the microphone. Click Open settings and turn on \"Let desktop apps access your microphone\". ({err})")
            } else {
                format!("macOS hasn't allowed the microphone yet. Click Open settings and switch on Toolbay Flow. ({err})")
            },
        },
    }
}

/// Opens the right page of the OS privacy settings.
#[tauri::command]
pub fn open_system_settings(app: AppHandle, kind: String) {
    use tauri_plugin_opener::OpenerExt;
    let url = if cfg!(windows) {
        match kind.as_str() {
            "microphone" => "ms-settings:privacy-microphone",
            _ => "ms-settings:privacy",
        }
    } else {
        match kind.as_str() {
            "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            _ => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        }
    };
    let _ = app.opener().open_url(url, None::<&str>);
}

#[tauri::command]
pub fn has_desktop_shortcut() -> bool {
    crate::hub::has_desktop_shortcut()
}

#[tauri::command]
pub fn set_desktop_shortcut(st: State<St>, on: bool) -> R<()> {
    crate::hub::set_desktop_shortcut(on).map_err(|_| "Couldn't change the desktop shortcut. Check that your Desktop folder exists.".to_string())?;
    let mut s = st.settings.read().clone();
    s.desktop_shortcut = on;
    dictation::apply_settings(&st, s).map_err(e)
}

#[allow(dead_code)]
fn _m(app: &AppHandle) {
    let _ = app.get_webview_window("main");
}
