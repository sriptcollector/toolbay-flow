//! User settings, persisted as JSON in the app config dir.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    PushToTalk,
    HandsFree,
    Command,
    PasteLast,
    CopyLast,
    Dismiss,
    Transform1,
    Transform2,
    Transform3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shortcut {
    pub action: Action,
    /// Platform key codes (Windows virtual-key codes, macOS normalized codes), sorted.
    pub keys: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CleanupLevel {
    /// Paste exactly what the speech model heard.
    None,
    /// Rule-based only: fillers, spoken punctuation, capitalization. No LLM.
    Light,
    /// LLM pass that keeps your words but fixes punctuation, fillers, corrections, lists.
    Medium,
    /// LLM pass that also tightens wording.
    High,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Style {
    Formal,
    Casual,
    VeryCasual,
    Excited,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Styles {
    pub personal: Style,
    pub work: Style,
    pub email: Style,
    pub other: Style,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DataPolicy {
    StoreNormally,
    DeleteAfter24h,
    NeverStore,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    Local,
    /// Any OpenAI-compatible endpoint (OpenAI, Groq, OpenRouter, LM Studio, Ollama...).
    OpenAiCompatible,
    Anthropic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudLlm {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub onboarded: bool,
    /// Ignored: Flow never launches at login. Kept so saved settings still load.
    pub launch_at_login: bool,
    pub show_bar: bool,
    pub sounds: bool,
    pub microphone: Option<String>,
    /// "auto" or a whisper language code ("en", "es", ...).
    pub language: String,
    pub speech_model: String,
    pub use_gpu: bool,
    pub cleanup_level: CleanupLevel,
    /// The person picked the cleanup level themselves: Auto never changes it again.
    pub cleanup_chosen: bool,
    pub llm_provider: LlmProvider,
    pub local_llm_model: String,
    pub cloud_llm: CloudLlm,
    pub anthropic: CloudLlm,
    pub styles: Styles,
    pub context_awareness: bool,
    pub auto_learn_words: bool,
    pub command_mode: bool,
    pub data_policy: DataPolicy,
    pub keep_audio: bool,
    pub shortcuts: Vec<Shortcut>,
    pub transforms: Vec<Transform>,
    /// Apps (exe or bundle id, lowercase) mapped to a style category.
    pub app_categories: BTreeMap<String, String>,
    pub restore_clipboard: bool,
    pub name: String,
    /// "auto" (Flow picks models for this computer) or "manual".
    pub model_mode: String,
    pub hw_fingerprint: String,
    pub auto_reason: String,
    pub auto_slow_speech: bool,
    pub auto_slow_cleanup: bool,
    pub desktop_shortcut: bool,
    /// Onboarding answers (role, where you type...). Kept on this computer; sent
    /// to the Toolbay account only when the person turns sync on.
    pub profile: serde_json::Value,
    /// Say "press enter" at the end to send the message after pasting.
    pub press_enter_command: bool,
    /// Flow bar hidden until this time (unix seconds), from "Hide for 1 hour".
    pub bar_snooze_until: i64,
    /// Keep the audio, raw transcript and final text of the last 20 dictations in
    /// a "debug" folder, to trace where words went missing. Off by default.
    pub debug_recordings: bool,
    /// "local" (default: private, free, on this computer) or "cloud" (hosted
    /// speech recognition with the user's own API key, OpenAI-compatible).
    pub engine: String,
    pub cloud_asr: CloudLlm,
    /// "light" (default, like the original), "dark" or "system".
    pub appearance: String,
    /// Keep settings, profile, dictionary and snippets in step across this
    /// person's computers through their Toolbay account (toolbay.ai can read
    /// this data). Off by default: only the person turns it on.
    pub sync_enabled: bool,
    /// The person answered the sync question (or turned sync on themselves).
    /// Until then sync stays off, whatever an older version saved.
    pub sync_asked: bool,
    /// Also sync recent dictations (text only, never audio). Off by default.
    pub sync_history: bool,
    /// Shortcuts saved on the user's other platform (Mac keys on Windows and the
    /// other way round), kept so syncing never overwrites them.
    pub hotkeys_other_os: serde_json::Value,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            onboarded: false,
            launch_at_login: false,
            show_bar: true,
            sounds: true,
            microphone: None,
            language: "en".into(),
            speech_model: "base.en-q5".into(),
            use_gpu: true,
            cleanup_level: CleanupLevel::Light,
            cleanup_chosen: false,
            llm_provider: LlmProvider::Local,
            local_llm_model: "qwen3-4b-instruct".into(),
            cloud_llm: CloudLlm {
                base_url: "https://api.openai.com/v1".into(),
                api_key: String::new(),
                model: "gpt-4o-mini".into(),
            },
            anthropic: CloudLlm {
                base_url: "https://api.anthropic.com".into(),
                api_key: String::new(),
                model: "claude-haiku-4-5".into(),
            },
            styles: Styles {
                personal: Style::Casual,
                work: Style::Formal,
                email: Style::Formal,
                other: Style::Formal,
            },
            context_awareness: true,
            auto_learn_words: true,
            command_mode: true,
            data_policy: DataPolicy::StoreNormally,
            keep_audio: true,
            shortcuts: default_shortcuts(),
            transforms: default_transforms(),
            app_categories: BTreeMap::new(),
            restore_clipboard: true,
            name: String::new(),
            model_mode: "auto".into(),
            hw_fingerprint: String::new(),
            auto_reason: String::new(),
            auto_slow_speech: false,
            auto_slow_cleanup: false,
            desktop_shortcut: true,
            profile: serde_json::Value::Null,
            press_enter_command: true,
            bar_snooze_until: 0,
            debug_recordings: false,
            engine: "local".into(),
            cloud_asr: CloudLlm {
                base_url: "https://api.groq.com/openai/v1".into(),
                api_key: String::new(),
                model: "whisper-large-v3-turbo".into(),
            },
            appearance: "light".into(),
            sync_enabled: false,
            sync_asked: false,
            sync_history: false,
            hotkeys_other_os: serde_json::Value::Null,
        }
    }
}

pub fn default_transforms() -> Vec<Transform> {
    vec![
        Transform {
            id: "polish".into(),
            name: "Polish".into(),
            instructions: "Rewrite the text so it reads clearly and naturally. Fix grammar, remove repetition and filler, keep the author's voice, meaning and language. Do not add new information.".into(),
        },
        Transform {
            id: "prompt".into(),
            name: "Prompt engineer".into(),
            instructions: "Turn the text into a clear, well-structured prompt for an AI assistant: state the goal, the context, constraints and the desired output format. Keep every requirement the author mentioned.".into(),
        },
        Transform {
            id: "concise".into(),
            name: "Make it shorter".into(),
            instructions: "Make the text about half as long while keeping every important point and the author's tone.".into(),
        },
    ]
}

#[cfg(windows)]
pub fn default_shortcuts() -> Vec<Shortcut> {
    // Windows virtual-key codes. Left/right variants are normalized to the generic code.
    const CTRL: u32 = 0x11;
    const ALT: u32 = 0x12;
    const SHIFT: u32 = 0x10;
    const WIN: u32 = 0x5B;
    const SPACE: u32 = 0x20;
    const ESC: u32 = 0x1B;
    let s = |action, mut keys: Vec<u32>| {
        keys.sort();
        Shortcut { action, keys }
    };
    vec![
        s(Action::PushToTalk, vec![CTRL, WIN]),
        s(Action::HandsFree, vec![CTRL, WIN, SPACE]),
        s(Action::Command, vec![CTRL, WIN, ALT]),
        s(Action::PasteLast, vec![ALT, SHIFT, 0x5A]),
        s(Action::CopyLast, vec![ALT, SHIFT, 0x58]),
        s(Action::Dismiss, vec![ESC]),
        s(Action::Transform1, vec![WIN, ALT, 0x31]),
        s(Action::Transform2, vec![WIN, ALT, 0x32]),
        s(Action::Transform3, vec![WIN, ALT, 0x33]),
    ]
}

#[cfg(not(windows))]
pub fn default_shortcuts() -> Vec<Shortcut> {
    use crate::hotkeys::mac_codes::*;
    let s = |action, mut keys: Vec<u32>| {
        keys.sort();
        Shortcut { action, keys }
    };
    vec![
        s(Action::PushToTalk, vec![FN]),
        s(Action::HandsFree, vec![FN, SPACE]),
        s(Action::Command, vec![FN, CTRL]),
        s(Action::PasteLast, vec![CMD, CTRL, KEY_V]),
        s(Action::CopyLast, vec![CMD, CTRL, KEY_C]),
        s(Action::Dismiss, vec![ESC]),
        s(Action::Transform1, vec![ALT, KEY_1]),
        s(Action::Transform2, vec![ALT, KEY_2]),
        s(Action::Transform3, vec![ALT, KEY_3]),
    ]
}

pub fn path(dir: &PathBuf) -> PathBuf {
    dir.join("settings.json")
}

/// Set when settings.json exists but its contents couldn't be kept safe (unreadable,
/// or damaged and the copy failed). Saving then would replace the person's real
/// settings with defaults, so `save` refuses until the next start.
static SAVE_BLOCKED: AtomicBool = AtomicBool::new(false);
/// Each write gets its own temp file, so two writes never share one.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn load(dir: &PathBuf) -> Settings {
    let (mut s, safe) = read(dir);
    SAVE_BLOCKED.store(!safe, Ordering::SeqCst);
    if safe && migrate_sync(&mut s) {
        // Write it back right away (no sync poke: sync is off now anyway).
        let _ = write_file(dir, &s);
    }
    s
}

/// Reads settings.json. The flag says whether saving over the file is safe: a
/// missing file is (defaults), a damaged one is once a copy is kept as
/// `settings.json.bad-<time>` and the fields that still parse are merged over the
/// defaults; an unreadable file is not.
fn read(dir: &PathBuf) -> (Settings, bool) {
    let p = path(dir);
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), true),
        Err(e) => {
            log::error!("couldn't read {}: {e}", p.display());
            return (Settings::default(), false);
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(s) => (s, true),
        Err(e) => {
            let bad = dir.join(format!("settings.json.bad-{}", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
            let kept = std::fs::write(&bad, &text).is_ok();
            log::error!("settings.json is damaged ({e}); copy kept: {kept}");
            (recover(&text).unwrap_or_default(), kept)
        }
    }
}

/// The fields of a damaged settings file that still parse, merged over the
/// defaults one by one. None when the file isn't a JSON object at all.
pub fn recover(text: &str) -> Option<Settings> {
    let Ok(serde_json::Value::Object(saved)) = serde_json::from_str::<serde_json::Value>(text) else { return None };
    let mut merged = serde_json::to_value(Settings::default()).ok()?;
    for (k, v) in saved {
        let mut next = merged.clone();
        next[k.as_str()] = v;
        if serde_json::from_value::<Settings>(next.clone()).is_ok() {
            merged = next;
        }
    }
    serde_json::from_value(merged).ok()
}

/// One-time privacy migration: versions up to 0.4.18 turned sync on by default,
/// so a saved `sync_enabled: true` doesn't mean the person chose it. Sync stays
/// off until they answer the sync question or turn it on in Settings.
/// Returns true when it changed something.
pub fn migrate_sync(s: &mut Settings) -> bool {
    if !s.sync_asked && s.sync_enabled {
        s.sync_enabled = false;
        return true;
    }
    false
}

/// Saves settings. Callers go through `dictation::update_settings`, which holds
/// one lock across read, change and save.
pub fn save(dir: &PathBuf, s: &Settings) -> anyhow::Result<()> {
    if SAVE_BLOCKED.load(Ordering::SeqCst) {
        anyhow::bail!(
            "Toolbay Flow couldn't read your settings file, so it won't save over it. Quit and reopen Toolbay Flow to try again (the file is settings.json in {}).",
            dir.display()
        );
    }
    write_file(dir, s)?;
    crate::sync::poke();
    Ok(())
}

fn write_file(dir: &PathBuf, s: &Settings) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("settings.json.tmp-{}-{}", std::process::id(), TMP_SEQ.fetch_add(1, Ordering::SeqCst)));
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    if let Err(e) = std::fs::rename(&tmp, path(dir)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// "Win + Alt + 1" style label for messages (the hub has its own fuller table).
pub fn keys_label(keys: &[u32]) -> String {
    #[cfg(not(target_os = "macos"))]
    let (order, name): (&[u32], fn(u32) -> String) = (&[0x11, 0xA3, 0x5B, 0x12, 0xA5, 0x10], |c| match c {
        0x10 => "Shift".into(),
        0x11 => "Ctrl".into(),
        0x12 => "Alt".into(),
        0x5B => "Win".into(),
        0xA3 => "Right Ctrl".into(),
        0xA5 => "Right Alt".into(),
        0x20 => "Space".into(),
        0x1B => "Esc".into(),
        0x30..=0x5A => char::from_u32(c).map(|x| x.to_string()).unwrap_or_default(),
        0x70..=0x87 => format!("F{}", c - 0x6F),
        _ => format!("Key {c}"),
    });
    #[cfg(target_os = "macos")]
    let (order, name): (&[u32], fn(u32) -> String) = (&[63, 59, 62, 58, 61, 55, 54, 56], |c| match c {
        63 => "Fn".into(),
        59 => "Control".into(),
        58 => "Option".into(),
        55 => "\u{2318}".into(),
        56 => "Shift".into(),
        49 => "Space".into(),
        18 => "1".into(),
        19 => "2".into(),
        20 => "3".into(),
        _ => format!("Key {c}"),
    });
    let mut k = keys.to_vec();
    k.sort_by_key(|c| (order.iter().position(|o| o == c).unwrap_or(99), *c));
    k.into_iter().map(name).collect::<Vec<_>>().join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_is_off_by_default() {
        let s = Settings::default();
        assert!(!s.sync_enabled);
        assert!(!s.sync_asked);
    }

    #[test]
    fn old_settings_with_sync_on_are_turned_off_once() {
        // Saved by 0.4.18 or older: sync on by default, no sync_asked field.
        let mut s: Settings = serde_json::from_str(r#"{"onboarded": true, "sync_enabled": true}"#).unwrap();
        assert!(migrate_sync(&mut s));
        assert!(!s.sync_enabled);
        assert!(!migrate_sync(&mut s));
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("flow-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_damaged_file_keeps_the_fields_that_still_parse() {
        let s = recover(r#"{"onboarded": true, "name": "Ada", "sounds": "loud", "cleanup_level": "nonsense"}"#).unwrap();
        assert!(s.onboarded);
        assert_eq!(s.name, "Ada");
        assert!(s.sounds);
        assert_eq!(s.cleanup_level, CleanupLevel::Light);
        assert!(recover("{ not json").is_none());
    }

    #[test]
    fn a_damaged_file_is_copied_before_anything_saves_over_it() {
        let d = temp_dir("damaged");
        std::fs::write(path(&d), r#"{"onboarded": true, "sounds": 3}"#).unwrap();
        let (s, safe) = read(&d);
        assert!(s.onboarded);
        assert!(safe);
        let copies: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("settings.json.bad-")).collect();
        assert_eq!(copies.len(), 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_file_is_defaults() {
        let d = temp_dir("missing");
        let (s, safe) = read(&d);
        assert!(safe);
        assert!(!s.onboarded);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_chosen_sync_stays_on() {
        let mut s: Settings = serde_json::from_str(r#"{"sync_enabled": true, "sync_asked": true}"#).unwrap();
        assert!(!migrate_sync(&mut s));
        assert!(s.sync_enabled);
    }
}
