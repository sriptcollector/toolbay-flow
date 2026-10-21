//! What Toolbay Flow keeps in the Toolbay account, as sync items:
//!
//! | key | what |
//! |---|---|
//! | `settings` | preferences, engine choice (never API keys) |
//! | `hotkeys` | shortcuts per platform: `{windows: [...], macos: [...]}` |
//! | `styles` | the style per app category, custom app rules |
//! | `transforms` | the three transforms |
//! | `profile` | onboarding answers, so a new computer skips onboarding |
//! | `dictionary/<id>`, `snippets/<id>` | one item per word or snippet |
//! | `history/<id>` | last 50 dictations, text only, when the person turns it on |
//!
//! Never synced: audio, API keys, the microphone, model choices (they depend on
//! the computer's hardware), window and debug state.

use super::Store;
use crate::db::{DictEntry, HistoryItem};
use crate::dictation::St;
use crate::settings::Settings;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use tauri::Emitter;

const HISTORY_LIMIT: i64 = 50;

/// Settings fields that follow the person to every computer.
const SETTINGS: &[&str] = &[
    "launch_at_login",
    "show_bar",
    "sounds",
    "language",
    "cleanup_level",
    "llm_provider",
    "context_awareness",
    "auto_learn_words",
    "command_mode",
    "data_policy",
    "keep_audio",
    "restore_clipboard",
    "press_enter_command",
    "engine",
    "sync_history",
    "appearance",
];
/// Cloud engine choices: provider address and model sync, the key stays here.
const CLOUD: &[&str] = &["cloud_asr", "cloud_llm", "anthropic"];

const OS: &str = if cfg!(windows) { "windows" } else { "macos" };
const OTHER_OS: &str = if cfg!(windows) { "macos" } else { "windows" };

pub struct FlowStore(pub St);

fn dict_value(e: &DictEntry) -> Value {
    json!({ "phrase": e.phrase, "replacement": e.replacement, "starred": e.starred, "source": e.source, "created_at": e.created_at })
}

fn history_value(h: &HistoryItem) -> Value {
    json!({
        "created_at": h.created_at, "mode": h.mode, "raw_text": h.raw_text, "text": h.text, "edited_text": h.edited_text,
        "app": h.app, "window_title": h.window_title, "language": h.language, "duration_ms": h.duration_ms,
        "latency_ms": h.latency_ms, "words": h.words, "status": h.status,
    })
}

/// The synced part of the settings.
pub fn settings_item(s: &Settings) -> Value {
    let all = serde_json::to_value(s).unwrap_or_default();
    let mut out = Map::new();
    for k in SETTINGS {
        out.insert(k.to_string(), all[*k].clone());
    }
    for k in CLOUD {
        out.insert(k.to_string(), json!({ "base_url": all[*k]["base_url"], "model": all[*k]["model"] }));
    }
    Value::Object(out)
}

/// Settings with a synced `settings` item merged in (keys and local-only fields kept).
pub fn merge_settings(s: &Settings, item: &Value) -> Settings {
    let mut all = serde_json::to_value(s).unwrap_or_default();
    for k in SETTINGS {
        if let Some(v) = item.get(*k) {
            all[*k] = v.clone();
        }
    }
    for k in CLOUD {
        for f in ["base_url", "model"] {
            if let Some(v) = item.get(*k).and_then(|c| c.get(f)).filter(|v| v.is_string()) {
                all[*k][f] = v.clone();
            }
        }
    }
    serde_json::from_value(all).unwrap_or_else(|_| s.clone())
}

impl Store for FlowStore {
    fn enabled(&self) -> bool {
        self.0.settings.read().sync_enabled
    }

    fn ignored(&self, key: &str) -> bool {
        key.starts_with("history/") && !self.0.settings.read().sync_history
    }

    fn retains(&self, key: &str) -> bool {
        key == "profile" ||
        key.strip_prefix("history/").is_some_and(|sid| self.0.db.sync_has("history", sid))
    }

    fn snapshot(&self) -> BTreeMap<String, Value> {
        let s = self.0.settings.read().clone();
        let mut m = BTreeMap::new();
        m.insert("settings".into(), settings_item(&s));
        let mut hk = Map::new();
        if s.hotkeys_other_os.is_array() {
            hk.insert(OTHER_OS.into(), s.hotkeys_other_os.clone());
        }
        hk.insert(OS.into(), serde_json::to_value(&s.shortcuts).unwrap_or_default());
        m.insert("hotkeys".into(), Value::Object(hk));
        m.insert("styles".into(), json!({ "styles": s.styles, "app_categories": s.app_categories }));
        m.insert("transforms".into(), serde_json::to_value(&s.transforms).unwrap_or_default());
        // Only once set up: re-running setup here never sends other computers back into it.
        if s.onboarded {
            m.insert("profile".into(), json!({ "name": s.name, "answers": s.profile }));
        }
        for (sid, e) in self.0.db.sync_dict().unwrap_or_default() {
            let prefix = if e.is_snippet { "snippets" } else { "dictionary" };
            m.insert(format!("{prefix}/{sid}"), dict_value(&e));
        }
        if s.sync_history && s.data_policy != crate::settings::DataPolicy::NeverStore {
            for (sid, h) in self.0.db.sync_history(HISTORY_LIMIT).unwrap_or_default() {
                m.insert(format!("history/{sid}"), history_value(&h));
            }
        }
        m
    }

    fn apply(&self, changes: Vec<(String, Option<Value>)>) {
        let st = &self.0;
        let old = st.settings.read().clone();
        let mut s = old.clone();
        let (mut dict, mut history) = (false, false);
        for (key, v) in changes {
            let (kind, id) = key.split_once('/').unwrap_or((key.as_str(), ""));
            match (kind, v) {
                ("settings", Some(v)) => s = merge_settings(&s, &v),
                ("hotkeys", Some(v)) => {
                    if let Ok(mine) = serde_json::from_value(v[OS].clone()) {
                        s.shortcuts = mine;
                    }
                    s.hotkeys_other_os = v[OTHER_OS].clone();
                }
                ("styles", Some(v)) => {
                    if let Ok(x) = serde_json::from_value(v["styles"].clone()) {
                        s.styles = x;
                    }
                    if let Ok(x) = serde_json::from_value(v["app_categories"].clone()) {
                        s.app_categories = x;
                    }
                }
                ("transforms", Some(v)) => {
                    if let Ok(x) = serde_json::from_value(v) {
                        s.transforms = x;
                    }
                }
                ("profile", Some(v)) => {
                    // Set up on another computer: skip onboarding here too.
                    s.onboarded = true;
                    if let Some(n) = v["name"].as_str() {
                        s.name = n.into();
                    }
                    if !v["answers"].is_null() {
                        s.profile = v["answers"].clone();
                    }
                }
                ("dictionary" | "snippets", Some(v)) => {
                    let e = DictEntry {
                        phrase: v["phrase"].as_str().unwrap_or_default().into(),
                        replacement: v["replacement"].as_str().map(String::from),
                        is_snippet: kind == "snippets",
                        starred: v["starred"].as_bool().unwrap_or(false),
                        source: v["source"].as_str().unwrap_or("manual").into(),
                        created_at: v["created_at"].as_str().unwrap_or_default().into(),
                        ..Default::default()
                    };
                    if !e.phrase.is_empty() {
                        let _ = st.db.sync_put_dict(id, &e);
                        dict = true;
                    }
                }
                ("dictionary" | "snippets", None) => {
                    let _ = st.db.sync_delete("dictionary", id);
                    dict = true;
                }
                ("history", Some(v)) => {
                    if let Ok(h) = serde_json::from_value::<HistoryItem>(json!({
                        "created_at": v["created_at"], "mode": v["mode"], "raw_text": v["raw_text"], "text": v["text"],
                        "edited_text": v["edited_text"], "app": v["app"], "window_title": v["window_title"],
                        "language": v["language"], "duration_ms": v["duration_ms"], "latency_ms": v["latency_ms"],
                        "words": v["words"], "status": v["status"],
                        "id": 0, "url": "", "has_audio": false,
                    })) {
                        let _ = st.db.sync_put_history(id, &h);
                        history = true;
                    }
                }
                ("history", None) => {
                    let _ = st.db.sync_delete("history", id);
                    history = true;
                }
                _ => {}
            }
        }
        if serde_json::to_value(&s).ok() != serde_json::to_value(&old).ok() {
            let _ = crate::dictation::apply_settings(st, s);
            let _ = st.app.emit("settings-changed", ());
            crate::refresh_tray(&st.app);
        }
        if dict {
            let _ = st.app.emit("dictionary-changed", ());
        }
        if history {
            let _ = st.app.emit("history-changed", ());
        }
    }
}

static SYNC: once_cell::sync::OnceCell<std::sync::Arc<super::Syncer>> = once_cell::sync::OnceCell::new();

pub fn get() -> Option<&'static std::sync::Arc<super::Syncer>> {
    SYNC.get()
}

/// Starts syncing in the background (it does nothing until the person is signed in).
pub fn start(st: St) {
    let app = st.app.clone();
    let sync = std::sync::Arc::new(super::Syncer::new(
        "toolbay-flow",
        st.config_dir.clone(),
        Box::new(FlowStore(st.clone())),
        Box::new(move |s| {
            let _ = app.emit("sync-status", s);
        }),
    ));
    if SYNC.set(sync.clone()).is_ok() {
        tauri::async_runtime::spawn(sync.run_forever());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_keys_and_device_settings_never_sync() {
        let mut s = Settings::default();
        s.cloud_asr.api_key = "gsk_secret".into();
        s.cloud_llm.api_key = "sk_secret".into();
        s.microphone = Some("USB mic".into());
        let item = settings_item(&s).to_string();
        assert!(!item.contains("secret"));
        assert!(!item.contains("USB mic"));
        assert!(!item.contains("speech_model"));
    }

    #[test]
    fn merging_keeps_this_computers_keys() {
        let mut here = Settings::default();
        here.cloud_asr.api_key = "mine".into();
        here.speech_model = "large".into();
        let mut there = Settings::default();
        there.engine = "cloud".into();
        there.cloud_asr.model = "whisper-x".into();
        there.sounds = false;
        let merged = merge_settings(&here, &settings_item(&there));
        assert_eq!(merged.engine, "cloud");
        assert_eq!(merged.cloud_asr.model, "whisper-x");
        assert_eq!(merged.cloud_asr.api_key, "mine");
        assert_eq!(merged.speech_model, "large");
        assert!(!merged.sounds);
    }
}
