//! Commands the React UI calls.

use crate::db::{DictEntry, HistoryItem, Note, Stats};
use crate::dictation::{self, St};
use crate::engines;
use crate::settings::Settings;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

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
        _ => {}
    }
}

#[tauri::command]
pub fn bar_hit(rects: Vec<[f64; 4]>) {
    *crate::BAR_HIT.lock() = rects;
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
        "fromToolbay": std::env::args().any(|a| a == "--from-toolbay"),
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
    app.exit(0);
}

#[allow(dead_code)]
fn _m(app: &AppHandle) {
    let _ = app.get_webview_window("main");
}
