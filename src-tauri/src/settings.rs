//! User settings, persisted as JSON in the app config dir.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

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
    /// Onboarding answers (role, where you type...). Stays on this computer.
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
    /// Keep settings, dictionary and snippets in step across this person's
    /// computers through their Toolbay account (only when signed in). On by default.
    /// "light" (default, like the original), "dark" or "system".
    pub appearance: String,
    pub sync_enabled: bool,
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
            sync_enabled: true,
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

pub fn load(dir: &PathBuf) -> Settings {
    std::fs::read_to_string(path(dir))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(dir: &PathBuf, s: &Settings) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join("settings.json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(tmp, path(dir))?;
    crate::sync::poke();
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
