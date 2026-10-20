//! The dictation state machine: hotkeys in, text out.

use crate::audio::{self, Recorder};
use crate::context::{self, FocusContext};
use crate::db::{count_words, Db, DictEntry, HistoryItem};
use crate::engines::{self, Engines, Paths};
use crate::hotkeys::{self, HotkeyEvent};
use crate::llm::{self, CleanupInput, Llm};
use crate::paste;
use crate::settings::{self, Action, CleanupLevel, DataPolicy, LlmProvider, Settings, Style};
use crate::sounds::{self, Sound};
use crate::textproc;
use anyhow::{anyhow, Result};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Ptt,
    HandsFree,
    Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Recording,
    Processing,
}

struct Session {
    phase: Phase,
    mode: Mode,
    started: Instant,
    ctx: Arc<Mutex<Option<FocusContext>>>,
    last_tap_up: Option<Instant>,
    stream: Arc<Mutex<StreamState>>,
}

pub struct State {
    pub app: AppHandle,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub settings: RwLock<Settings>,
    pub db: Db,
    pub paths: Paths,
    pub engines: Engines,
    pub engine_lock: tokio::sync::Mutex<()>,
    rec: Mutex<Recorder>,
    session: Mutex<Session>,
    pub hk: Arc<hotkeys::Shared>,
    /// Incremented on cancel so in-flight work knows to drop its result.
    generation: AtomicU64,
    pub mic_test: AtomicBool,
    pub last_learned: Mutex<Vec<i64>>,
    last_activity: Mutex<Instant>,
}

#[derive(Clone, Serialize)]
pub struct BarState {
    pub phase: &'static str, // idle | recording | processing | done | error | notice
    pub mode: Option<Mode>,
    pub message: Option<String>,
}

pub type St = Arc<State>;

impl State {
    pub fn new(app: AppHandle, config_dir: PathBuf, data_dir: PathBuf) -> Result<St> {
        std::fs::create_dir_all(&config_dir)?;
        std::fs::create_dir_all(&data_dir)?;
        let s = settings::load(&config_dir);
        let db = Db::open(&data_dir.join("flow.sqlite"))?;
        let _ = db.apply_retention(&s.data_policy);
        let hk = Arc::new(hotkeys::Shared {
            shortcuts: RwLock::new(s.shortcuts.clone()),
            dismiss_armed: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            enabled: AtomicBool::new(true),
        });
        Ok(Arc::new(State {
            app,
            paths: Paths::new(&data_dir),
            config_dir,
            data_dir,
            settings: RwLock::new(s),
            db,
            engines: Engines::default(),
            engine_lock: tokio::sync::Mutex::new(()),
            rec: Mutex::new(Recorder::new()),
            session: Mutex::new(Session {
                phase: Phase::Idle,
                mode: Mode::Ptt,
                started: Instant::now(),
                ctx: Arc::new(Mutex::new(None)),
                last_tap_up: None,
                stream: Arc::new(Mutex::new(StreamState::default())),
            }),
            hk,
            generation: AtomicU64::new(0),
            mic_test: AtomicBool::new(false),
            last_learned: Mutex::new(vec![]),
            last_activity: Mutex::new(Instant::now()),
        }))
    }

    pub fn bar(&self, phase: &'static str, mode: Option<Mode>, message: Option<String>) {
        let _ = self.app.emit("bar-state", BarState { phase, mode, message });
    }

    pub fn notice(&self, msg: impl Into<String>) {
        self.bar("notice", None, Some(msg.into()));
    }
}

// ---------------- engines ----------------

pub async fn ensure_whisper(st: &St) -> Result<String> {
    let _g = st.engine_lock.lock().await;
    let use_gpu = st.settings.read().use_gpu;
    let (model_id, model_path) = crate::models::current_speech_path(st).ok_or_else(|| anyhow!("Flow is still getting its speech model. Try again in a moment."))?;
    // Prefer the GPU engine once it is downloaded; until then use the bundled CPU one.
    let mut eng = engines::pick_engine("whisper", use_gpu).ok_or_else(|| anyhow!("No speech engine for this platform"))?;
    let mut exe = st.paths.engine_exe(eng).or_else(|| crate::models::bundled_engine_exe(st, eng));
    if exe.is_none() {
        if let Some(cpu) = engines::pick_engine("whisper", false) {
            if let Some(b) = crate::models::bundled_engine_exe(st, cpu) {
                eng = cpu;
                exe = Some(b);
            }
        }
    }
    let key = format!("{}|{}", eng.id, model_id);
    {
        let mut w = st.engines.whisper.lock();
        if let Some(srv) = w.as_mut() {
            if srv.key == key && srv.alive() {
                return Ok(srv.base());
            }
        }
        *w = None;
    }
    let exe = match exe {
        Some(e) => e,
        None => engines::ensure_engine(&st.app, &st.paths, eng).await?,
    };
    let mut args = vec![
        "-m".to_string(),
        model_path.to_string_lossy().to_string(),
        "-t".into(),
        engines::threads().to_string(),
        "-nt".into(),
        "-sns".into(),
    ];
    if !eng.gpu {
        args.push("-ng".into());
    }
    let srv = engines::spawn_server(&exe, &args, key, &st.data_dir.join("whisper-server.log"))?;
    let base = srv.base();
    *st.engines.whisper.lock() = Some(srv);
    engines::wait_ready(&base, "/", Duration::from_secs(90)).await?;
    // Warm-up: the first inference initializes GPU kernels.
    let _ = llm::transcribe(&base, audio::to_wav(&vec![0f32; 16000]), "en", "").await;
    Ok(base)
}

pub async fn ensure_llama(st: &St) -> Result<String> {
    let _g = st.engine_lock.lock().await;
    let (model_id, use_gpu) = {
        let s = st.settings.read();
        (s.local_llm_model.clone(), s.use_gpu)
    };
    let m = engines::model(&model_id).ok_or_else(|| anyhow!("Unknown cleanup model"))?;
    let model_path = crate::models::model_file(st, m).ok_or_else(|| anyhow!("The AI cleanup model is still downloading"))?;
    let eng = engines::pick_engine("llama", use_gpu).ok_or_else(|| anyhow!("No cleanup engine for this platform"))?;
    let key = format!("{}|{}", eng.id, model_id);
    {
        let mut l = st.engines.llama.lock();
        if let Some(srv) = l.as_mut() {
            if srv.key == key && srv.alive() {
                return Ok(srv.base());
            }
        }
        *l = None;
    }
    let exe = engines::ensure_engine(&st.app, &st.paths, eng).await?;
    let mut args = vec![
        "-m".to_string(),
        model_path.to_string_lossy().to_string(),
        "-c".into(),
        "8192".into(),
        "-np".into(),
        "1".into(),
        "-t".into(),
        engines::threads().to_string(),
        "--no-webui".into(),
        "--jinja".into(),
    ];
    if eng.gpu {
        args.extend(["-ngl".into(), "99".into()]);
    } else {
        args.extend(["-ngl".into(), "0".into()]);
    }
    let srv = engines::spawn_server(&exe, &args, key, &st.data_dir.join("llama-server.log"))?;
    let base = srv.base();
    *st.engines.llama.lock() = Some(srv);
    engines::wait_ready(&base, "/health", Duration::from_secs(120)).await?;
    // Warm-up: compiles GPU shaders and caches the cleanup prompt prefix.
    let warm = Llm {
        provider: LlmProvider::Local,
        local_base: Some(base.clone()),
        cloud: st.settings.read().cloud_llm.clone(),
        anthropic: st.settings.read().anthropic.clone(),
    };
    let style = st.settings.read().styles.other.clone();
    let input = CleanupInput {
        transcript: "hello world",
        level: &CleanupLevel::Medium,
        style: &style,
        category: "other",
        app: "",
        window_title: "",
        before: "",
        after: "",
        vocabulary: "",
        language: "en",
        name: "",
    };
    let (sys, msgs) = llm::cleanup_messages(&input);
    let _ = warm.chat(&sys, &msgs, 16, Duration::from_secs(60)).await;
    Ok(base)
}

pub fn prewarm(st: &St) {
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = ensure_whisper(&st).await {
            log::info!("speech engine not started: {e}");
        }
        let (level, provider) = {
            let s = st.settings.read();
            (s.cleanup_level.clone(), s.llm_provider.clone())
        };
        if matches!(level, CleanupLevel::Medium | CleanupLevel::High) && provider == LlmProvider::Local {
            if let Err(e) = ensure_llama(&st).await {
                log::info!("cleanup engine not started: {e}");
            }
        }
    });
}

async fn llm_for(st: &St) -> Result<Llm> {
    let (provider, cloud, anthropic) = {
        let s = st.settings.read();
        (s.llm_provider.clone(), s.cloud_llm.clone(), s.anthropic.clone())
    };
    let local_base = if provider == LlmProvider::Local { Some(ensure_llama(st).await?) } else { None };
    Ok(Llm { provider, local_base, cloud, anthropic })
}

// ---------------- hotkeys ----------------

pub fn start_hotkeys(st: &St) {
    let (tx, rx) = std::sync::mpsc::channel::<HotkeyEvent>();
    hotkeys::start(st.hk.clone(), Arc::new(move |e| {
        let _ = tx.send(e);
    }));
    let st = st.clone();
    std::thread::Builder::new()
        .name("dictation".into())
        .spawn(move || {
            while let Ok(ev) = rx.recv() {
                on_hotkey(&st, ev);
            }
        })
        .ok();
}

fn on_hotkey(st: &St, ev: HotkeyEvent) {
    let (phase, mode, elapsed, last_tap) = {
        let s = st.session.lock();
        (s.phase, s.mode, s.started.elapsed(), s.last_tap_up)
    };
    match ev {
        HotkeyEvent::Recording { keys, done } => {
            let _ = st.app.emit("shortcut-recorded", serde_json::json!({ "keys": keys, "done": done }));
            if done {
                st.hk.recording.store(false, Ordering::Relaxed);
            }
        }
        HotkeyEvent::Pressed(a) => match (a, phase) {
            (Action::PushToTalk, Phase::Idle) => {
                if last_tap.map(|t| t.elapsed() < Duration::from_millis(450)).unwrap_or(false) {
                    start(st, Mode::HandsFree);
                } else {
                    start(st, Mode::Ptt);
                }
            }
            (Action::PushToTalk, Phase::Recording) if mode == Mode::HandsFree && elapsed > Duration::from_millis(400) => stop(st),
            (Action::HandsFree, Phase::Idle) => start(st, Mode::HandsFree),
            (Action::HandsFree, Phase::Recording) => {
                if mode == Mode::Ptt {
                    set_mode(st, Mode::HandsFree);
                } else {
                    stop(st);
                }
            }
            (Action::Command, Phase::Idle) if st.settings.read().command_mode => start(st, Mode::Command),
            (Action::Command, Phase::Recording) if mode == Mode::Ptt && st.settings.read().command_mode => set_mode(st, Mode::Command),
            (Action::Dismiss, Phase::Recording) | (Action::Dismiss, Phase::Processing) => cancel(st),
            (Action::PasteLast, _) => {
                if let Ok(Some(t)) = st.db.last_text() {
                    std::thread::sleep(Duration::from_millis(120));
                    let restore = st.settings.read().restore_clipboard;
                    let _ = paste::paste(&t, restore);
                }
            }
            (Action::CopyLast, _) => {
                if let Ok(Some(t)) = st.db.last_text() {
                    let _ = paste::copy(&t);
                    st.notice("Copied last dictation");
                }
            }
            (Action::Transform1, Phase::Idle) => transform(st, 0),
            (Action::Transform2, Phase::Idle) => transform(st, 1),
            (Action::Transform3, Phase::Idle) => transform(st, 2),
            _ => {}
        },
        HotkeyEvent::Released(a) => match (a, phase, mode) {
            (Action::PushToTalk, Phase::Recording, Mode::Ptt) => {
                if elapsed < Duration::from_millis(300) {
                    // A tap, not a hold: the first half of a double-tap.
                    discard(st, false);
                    st.session.lock().last_tap_up = Some(Instant::now());
                } else {
                    stop(st);
                }
            }
            (Action::PushToTalk, Phase::Recording, Mode::Command) | (Action::Command, Phase::Recording, Mode::Command) => stop(st),
            _ => {}
        },
    }
}

fn set_mode(st: &St, mode: Mode) {
    st.session.lock().mode = mode;
    if st.settings.read().sounds && mode == Mode::HandsFree {
        sounds::play(Sound::Lock);
    }
    st.bar("recording", Some(mode), None);
}

pub fn start(st: &St, mode: Mode) {
    if st.mic_test.load(Ordering::Relaxed) {
        return;
    }
    let s = st.settings.read().clone();
    if crate::models::current_speech_path(st).is_none() {
        st.bar("notice", None, Some("Flow is still getting ready. Try again in a moment.".into()));
        return;
    }
    let ctx_slot: Arc<Mutex<Option<FocusContext>>> = Arc::new(Mutex::new(None));
    {
        let slot = ctx_slot.clone();
        let read_text = s.context_awareness;
        std::thread::spawn(move || {
            *slot.lock() = Some(context::capture(read_text));
        });
    }
    let app = st.app.clone();
    let res = st.rec.lock().start(s.microphone.clone(), Arc::new(move |lvl| {
        let _ = app.emit("bar-level", lvl);
    }));
    if let Err(e) = res {
        sounds::play(Sound::Error);
        st.bar("error", None, Some(format!("{e}")));
        return;
    }
    *st.last_activity.lock() = Instant::now();
    let stream = Arc::new(Mutex::new(StreamState::default()));
    {
        let mut ss = st.session.lock();
        ss.phase = Phase::Recording;
        ss.mode = mode;
        ss.started = Instant::now();
        ss.ctx = ctx_slot;
        ss.last_tap_up = None;
        ss.stream = stream.clone();
    }
    st.hk.dismiss_armed.store(true, Ordering::Relaxed);
    if s.sounds {
        sounds::play(if mode == Mode::HandsFree { Sound::Lock } else { Sound::Start });
    }
    st.bar("recording", Some(mode), None);
    let gen = st.generation.load(Ordering::SeqCst);
    // Transcribe finished sentences while the user is still talking, so only the
    // last few seconds are left when they let go.
    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        let _ = ensure_whisper(&st2).await;
        stream_loop(st2, stream, gen).await;
    });
    // Safety stop for forgotten hands-free sessions.
    let st3 = st.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20 * 60));
        if st3.generation.load(Ordering::SeqCst) == gen && st3.session.lock().phase == Phase::Recording {
            stop(&st3);
        }
    });
}

/// Progress of streaming transcription for the current recording.
#[derive(Default)]
pub struct StreamState {
    /// 16 kHz samples already transcribed.
    committed: usize,
    parts: Vec<String>,
    busy: bool,
}

async fn stream_loop(st: St, stream: Arc<Mutex<StreamState>>, gen: u64) {
    const MIN_CHUNK: usize = 16_000 * 4;
    loop {
        tokio::time::sleep(Duration::from_millis(350)).await;
        if st.generation.load(Ordering::SeqCst) != gen || st.session.lock().phase != Phase::Recording {
            return;
        }
        let snap = st.rec.lock().snapshot();
        let from = stream.lock().committed;
        if snap.len() < from + MIN_CHUNK {
            continue;
        }
        let Some(cut) = audio::find_cut(&snap, from + MIN_CHUNK - 16_000) else { continue };
        if cut <= from {
            continue;
        }
        let prev = stream.lock().parts.join(" ");
        stream.lock().busy = true;
        let text = transcribe_with(&st, &snap[from..cut], &prev).await;
        let mut s = stream.lock();
        s.busy = false;
        match text {
            Ok(t) => {
                if !t.trim().is_empty() {
                    s.parts.push(t);
                }
                s.committed = cut;
            }
            Err(e) => {
                log::warn!("streaming chunk failed: {e:#}");
                return;
            }
        }
    }
}

fn discard(st: &St, with_sound: bool) {
    let _ = st.rec.lock().stop();
    st.session.lock().phase = Phase::Idle;
    st.hk.dismiss_armed.store(false, Ordering::Relaxed);
    if with_sound && st.settings.read().sounds {
        sounds::play(Sound::Cancel);
    }
    st.bar("idle", None, None);
}

pub fn cancel(st: &St) {
    st.generation.fetch_add(1, Ordering::SeqCst);
    discard(st, true);
}

pub fn stop(st: &St) {
    let released = Instant::now();
    let mut samples = st.rec.lock().stop();
    let (mode, ctx_slot, dur, stream) = {
        let mut ss = st.session.lock();
        ss.phase = Phase::Processing;
        (ss.mode, ss.ctx.clone(), ss.started.elapsed(), ss.stream.clone())
    };
    // Developer hook for automated end-to-end tests: replace the microphone
    // recording with a 16 kHz mono WAV file.
    if let Ok(p) = std::env::var("TOOLBAY_FLOW_TEST_AUDIO") {
        if let Ok(s) = std::fs::read(&p).map_err(anyhow::Error::from).and_then(|b| audio::from_wav(&b)) {
            samples = s;
            *stream.lock() = StreamState::default();
        }
    }
    if st.settings.read().sounds {
        sounds::play(Sound::Stop);
    }
    st.bar("processing", Some(mode), None);
    let gen = st.generation.load(Ordering::SeqCst);
    let st2 = st.clone();
    tauri::async_runtime::spawn(async move {
        let res = async {
            if !audio::has_speech(&samples) {
                return Ok(None);
            }
            // Finish any chunk in flight, then transcribe only the tail.
            for _ in 0..500 {
                if !stream.lock().busy {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            let (committed, parts) = {
                let s = stream.lock();
                (s.committed.min(samples.len()), s.parts.clone())
            };
            let prev = parts.join(" ");
            let tail = &samples[committed..];
            let tail_text = if audio::has_speech(tail) || committed == 0 { transcribe_with(&st2, tail, &prev).await? } else { String::new() };
            let raw = [prev, tail_text].iter().filter(|t| !t.trim().is_empty()).cloned().collect::<Vec<_>>().join(" ");
            let mut ctx = None;
            for _ in 0..40 {
                if let Some(c) = ctx_slot.lock().clone() {
                    ctx = Some(c);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            process(&st2, raw, samples, mode, ctx.unwrap_or_default(), dur, gen, released).await
        }
        .await;
        let current = st2.generation.load(Ordering::SeqCst) == gen;
        st2.session.lock().phase = Phase::Idle;
        st2.hk.dismiss_armed.store(false, Ordering::Relaxed);
        if !current {
            return;
        }
        match res {
            Ok(Some(msg)) => st2.bar("done", Some(mode), Some(msg)),
            Ok(None) => st2.bar("idle", None, None),
            Err(e) => {
                log::error!("dictation failed: {e:#}");
                if st2.settings.read().sounds {
                    sounds::play(Sound::Error);
                }
                st2.bar("error", None, Some(friendly_err(&e)));
            }
        }
    });
}

/// Error text for people, not developers: what happened and what to do.
fn friendly_err(e: &anyhow::Error) -> String {
    let s = format!("{e:#}").to_lowercase();
    if s.contains("connect") || s.contains("engine did not become ready") || s.contains("timed out") {
        "Flow's speech engine is restarting. Try again in a few seconds.".into()
    } else if s.contains("microphone") {
        format!("{e}")
    } else if s.contains("api key") || s.contains("401") || s.contains("403") {
        "Your cloud AI key was refused. Check it in Settings > Cleanup, or switch back to On this computer.".into()
    } else {
        let t = format!("{e}");
        t.chars().take(110).collect()
    }
}

/// Transcribe samples with the local speech engine.
pub async fn transcribe(st: &St, samples: &[f32]) -> Result<String> {
    transcribe_with(st, samples, "").await
}

/// Transcribe, giving the speech model the user's vocabulary and the text so far.
pub async fn transcribe_with(st: &St, samples: &[f32], previous: &str) -> Result<String> {
    let base = ensure_whisper(st).await?;
    let (lang, vocab) = {
        let s = st.settings.read();
        let dict = st.db.list_dict(false).unwrap_or_default();
        (s.language.clone(), textproc::vocabulary_prompt(&dict, 400))
    };
    let mut s = audio::compact_silence(samples);
    audio::normalize(&mut s);
    // Whisper is more reliable with a little silence on both sides.
    let mut padded = vec![0f32; 3200];
    padded.extend_from_slice(&s);
    padded.extend(std::iter::repeat(0f32).take(3200));
    let wav = audio::to_wav(&padded);
    let mut prompt = vocab;
    if !previous.is_empty() {
        if !prompt.is_empty() {
            prompt.push_str(". ");
        }
        prompt.push_str(&llm::tail(previous, 300));
    }
    let raw = llm::transcribe(&base, wav, &lang, &prompt).await?;
    Ok(textproc::strip_artifacts(&raw))
}

fn style_for(s: &Settings, category: &str) -> Style {
    match category {
        "personal" => s.styles.personal.clone(),
        "work" => s.styles.work.clone(),
        "email" => s.styles.email.clone(),
        _ => s.styles.other.clone(),
    }
}

/// Turns a raw transcript into final text for the given context.
pub async fn format_text(st: &St, raw: &str, ctx: &FocusContext) -> Result<String> {
    let s = st.settings.read().clone();
    let dict: Vec<DictEntry> = [st.db.list_dict(false).unwrap_or_default(), st.db.list_dict(true).unwrap_or_default()].concat();
    if let Some(sn) = textproc::exact_snippet(raw, &dict) {
        st.db.bump_dict_use(sn.id);
        return Ok(sn.replacement.clone().unwrap_or_default());
    }
    let category = textproc::app_category(&ctx.exe, &ctx.window_title, &ctx.url, &s.app_categories);
    let style = style_for(&s, &category);
    let mut used_llm = false;
    let mut text = match s.cleanup_level {
        CleanupLevel::None => raw.trim().to_string(),
        CleanupLevel::Light => textproc::rule_cleanup(raw, &style),
        // One to three words: rules are instant and an LLM adds nothing.
        CleanupLevel::Medium | CleanupLevel::High if raw.split_whitespace().count() <= 3 => textproc::rule_cleanup(raw, &style),
        CleanupLevel::Medium | CleanupLevel::High => {
            let vocab = textproc::vocabulary_prompt(&dict, 400);
            let input = CleanupInput {
                transcript: raw,
                level: &s.cleanup_level,
                style: &style,
                category: &category,
                app: &ctx.app,
                window_title: &ctx.window_title,
                before: if s.context_awareness { &ctx.before } else { "" },
                after: if s.context_awareness { &ctx.after } else { "" },
                vocabulary: &vocab,
                language: &s.language,
                name: &s.name,
            };
            let (system, msgs) = llm::cleanup_messages(&input);
            let max_tokens = (raw.split_whitespace().count() as u32 * 2 + 48).min(1500);
            let out = match llm_for(st).await {
                Ok(l) => l.chat(&system, &msgs, max_tokens, Duration::from_secs(20)).await,
                Err(e) => Err(e),
            };
            match out {
                Ok(o) if llm::plausible(raw, &o) => {
                    used_llm = true;
                    textproc::apply_spoken_punctuation(&strip_echo(&o, &ctx.before))
                }
                Ok(o) => {
                    log::warn!("cleanup output rejected: {o:?}");
                    textproc::rule_cleanup(raw, &style)
                }
                Err(e) => {
                    log::warn!("cleanup failed, using rules: {e:#}");
                    textproc::rule_cleanup(raw, &style)
                }
            }
        }
    };
    if used_llm {
        text = textproc::apply_style(&text, &style);
    }
    let (t, ids) = textproc::apply_replacements(&text, &dict);
    let (t, ids2) = textproc::expand_snippets(&t, &dict);
    for id in ids.into_iter().chain(ids2) {
        st.db.bump_dict_use(id);
    }
    Ok(join_with_context(&t, &ctx.before, used_llm))
}

/// Small models sometimes repeat the context text before the answer; drop it.
fn strip_echo(out: &str, before: &str) -> String {
    let b = before.trim();
    if b.len() >= 8 {
        let tail: String = llm::tail(b, 60);
        if let Some(i) = out.find(tail.trim()) {
            if i < 4 {
                return out[i + tail.trim().len()..].trim_start().to_string();
            }
        }
    }
    out.trim().to_string()
}

/// Adds a leading space when continuing text, and lowercases the first word
/// when continuing mid-sentence (rules mode only; the LLM already sees the context).
fn join_with_context(text: &str, before: &str, used_llm: bool) -> String {
    let mut t = text.to_string();
    let last = before.chars().last();
    if let Some(c) = last {
        if !used_llm && (c.is_alphanumeric() || c == ',') {
            let mut chars = t.chars();
            if let Some(f) = chars.next() {
                let rest: String = chars.collect();
                let second_lower = rest.chars().next().map(|c| c.is_lowercase()).unwrap_or(true);
                if f.is_uppercase() && second_lower && f != 'I' {
                    t = f.to_lowercase().collect::<String>() + &rest;
                }
            }
        }
        let starts_punct = t.starts_with([',', '.', '!', '?', ';', ':', ')']);
        if !c.is_whitespace() && !"([{\"'/@#".contains(c) && !starts_punct {
            t = format!(" {t}");
        }
    }
    t
}

#[allow(clippy::too_many_arguments)]
async fn process(st: &St, raw: String, samples: Vec<f32>, mode: Mode, ctx: FocusContext, dur: Duration, gen: u64, released: Instant) -> Result<Option<String>> {
    let speech_ms = released.elapsed().as_millis() as u64;
    let keep_audio = st.settings.read().keep_audio;
    let audio_bytes = if keep_audio { Some(audio::to_wav(&samples)) } else { None };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    if st.generation.load(Ordering::SeqCst) != gen {
        return Ok(None);
    }
    // "…press enter": paste, then send the message.
    let mut raw = raw;
    let mut press_enter = false;
    if mode != Mode::Command && st.settings.read().press_enter_command {
        if let Some(stripped) = textproc::strip_press_enter(&raw) {
            raw = stripped;
            press_enter = true;
        }
    }
    let (text, mode_name) = if mode == Mode::Command {
        let mut selected = ctx.selected.clone();
        if selected.trim().is_empty() && !is_terminal(&ctx.exe) {
            selected = tokio::task::spawn_blocking(paste::copy_selection).await.ok().flatten().unwrap_or_default();
        }
        let l = llm_for(st).await?;
        let (sys, msgs) = llm::command_messages(&raw, &selected, &ctx.before, &ctx.after, &ctx.app);
        let out = l.chat(&sys, &msgs, 2048, Duration::from_secs(60)).await?;
        if out.trim().is_empty() {
            return Err(anyhow!("No result from the model"));
        }
        (out, "command")
    } else {
        (format_text(st, &raw, &ctx).await?, "dictation")
    };
    if st.generation.load(Ordering::SeqCst) != gen {
        return Ok(None);
    }
    let restore = st.settings.read().restore_clipboard;
    let t2 = text.clone();
    tokio::task::spawn_blocking(move || paste::paste(&t2, restore)).await??;
    if press_enter {
        tokio::time::sleep(Duration::from_millis(120)).await;
        let _ = tokio::task::spawn_blocking(paste::press_enter).await;
    }
    // Release-to-paste latency, the number that makes dictation feel instant or not.
    let latency = released.elapsed().as_millis() as i64;
    let cleanup_ms = (latency as u64).saturating_sub(speech_ms);
    crate::models::record_latency(st, (samples.len() / 16) as u64, speech_ms, if mode == Mode::Command { None } else { Some(cleanup_ms) });

    let policy = st.settings.read().data_policy.clone();
    let mut hid = 0;
    if policy != DataPolicy::NeverStore {
        let item = HistoryItem {
            mode: mode_name.into(),
            raw_text: raw.clone(),
            text: text.trim().to_string(),
            app: ctx.app.clone(),
            window_title: ctx.window_title.clone(),
            url: ctx.url.clone(),
            language: st.settings.read().language.clone(),
            duration_ms: dur.as_millis() as i64,
            latency_ms: latency,
            words: count_words(&text) as i64,
            status: "pasted".into(),
            ..Default::default()
        };
        hid = st.db.add_history(&item, audio_bytes.as_deref()).unwrap_or(0);
        let _ = st.app.emit("history-changed", ());
    }
    if mode_name == "dictation" && st.settings.read().auto_learn_words && !ctx.is_password {
        watch_edits(st.clone(), text.trim().to_string(), ctx, hid);
    }
    let words = count_words(&text);
    Ok(Some(format!("{words} word{}", if words == 1 { "" } else { "s" })))
}

fn is_terminal(exe: &str) -> bool {
    let e = exe.to_lowercase();
    ["windowsterminal", "cmd.exe", "powershell", "pwsh", "conhost", "wezterm", "alacritty", "terminal", "iterm"].iter().any(|t| e.contains(t))
}

/// After a paste, watch the field for a little while and learn names/jargon the
/// user corrects by hand.
fn watch_edits(st: St, pasted: String, ctx: FocusContext, history_id: i64) {
    std::thread::spawn(move || {
        let anchor: String = llm::tail(ctx.before.trim_end(), 40);
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(2500));
            let now = context::capture(false);
            if now.pid != ctx.pid || now.window_title != ctx.window_title {
                return;
            }
            let Some(doc) = context::focused_text() else { return };
            if doc.contains(&pasted) {
                continue;
            }
            let start = if anchor.is_empty() { Some(0) } else { doc.find(&anchor).map(|i| i + anchor.len()) };
            let Some(start) = start else { continue };
            let region: String = doc[start..].chars().take(pasted.chars().count() + 60).collect();
            let wc = pasted.split_whitespace().count() + 2;
            let region = region.split_whitespace().take(wc).collect::<Vec<_>>().join(" ");
            let fixes = textproc::learned_corrections(&pasted, &region);
            if fixes.is_empty() {
                continue;
            }
            let mut ids = vec![];
            let mut words = vec![];
            for (_old, new) in fixes {
                let e = DictEntry { phrase: new.clone(), source: "user_edits".into(), ..Default::default() };
                if let Ok(id) = st.db.upsert_dict(&e) {
                    ids.push(id);
                    words.push(new);
                }
            }
            if history_id > 0 {
                let _ = st.db.set_edited(history_id, &region);
            }
            if !words.is_empty() {
                *st.last_learned.lock() = ids;
                let _ = st.app.emit("dictionary-learned", words.clone());
                st.bar("learned", None, Some(format!("Added {} to your dictionary", words.join(", "))));
            }
            return;
        }
    });
}

/// Rewrites the selected text with transform #i.
pub fn transform(st: &St, i: usize) {
    let tf = st.settings.read().transforms.get(i).cloned();
    let Some(tf) = tf else { return };
    let st = st.clone();
    st.session.lock().phase = Phase::Processing;
    st.bar("processing", None, Some(tf.name.clone()));
    tauri::async_runtime::spawn(async move {
        let res: Result<String> = async {
            std::thread::sleep(Duration::from_millis(150));
            let ctx = tokio::task::spawn_blocking(|| context::capture(true)).await?;
            let mut sel = ctx.selected.clone();
            if sel.trim().is_empty() {
                sel = tokio::task::spawn_blocking(paste::copy_selection).await?.unwrap_or_default();
            }
            if sel.trim().is_empty() {
                return Err(anyhow!("Select some text first"));
            }
            let l = llm_for(&st).await?;
            let (sys, msgs) = llm::transform_messages(&tf.instructions, &sel);
            let out = l.chat(&sys, &msgs, 3000, Duration::from_secs(90)).await?;
            let restore = st.settings.read().restore_clipboard;
            let o2 = out.clone();
            tokio::task::spawn_blocking(move || paste::paste(&o2, restore)).await??;
            if st.settings.read().data_policy != DataPolicy::NeverStore {
                let item = HistoryItem {
                    mode: "transform".into(),
                    raw_text: sel,
                    text: out.clone(),
                    app: ctx.app,
                    window_title: ctx.window_title,
                    words: count_words(&out) as i64,
                    status: "pasted".into(),
                    ..Default::default()
                };
                let _ = st.db.add_history(&item, None);
                let _ = st.app.emit("history-changed", ());
            }
            Ok(tf.name.clone())
        }
        .await;
        st.session.lock().phase = Phase::Idle;
        match res {
            Ok(name) => st.bar("done", None, Some(name)),
            Err(e) => st.bar("error", None, Some(friendly_err(&e))),
        }
    });
}

/// Re-runs a history item from its saved audio.
pub async fn retry(st: &St, id: i64) -> Result<String> {
    let wav = st.db.get_audio(id)?.ok_or_else(|| anyhow!("No audio saved for this dictation"))?;
    let samples = audio::from_wav(&wav)?;
    let raw = transcribe(st, &samples).await?;
    let text = format_text(st, &raw, &FocusContext::default()).await?;
    st.db.update_history_text(id, &raw, text.trim(), "pasted")?;
    Ok(text.trim().to_string())
}

/// Microphone test for onboarding/settings: streams levels as "mic-level".
pub fn mic_test(st: &St, on: bool) -> Result<()> {
    if on {
        if st.session.lock().phase != Phase::Idle {
            return Ok(());
        }
        st.mic_test.store(true, Ordering::Relaxed);
        let app = st.app.clone();
        let dev = st.settings.read().microphone.clone();
        st.rec.lock().start(dev, Arc::new(move |l| {
            let _ = app.emit("mic-level", l);
        }))?;
    } else if st.mic_test.swap(false, Ordering::Relaxed) {
        let _ = st.rec.lock().stop();
    }
    Ok(())
}

pub fn apply_settings(st: &St, new: Settings) -> Result<()> {
    let old = st.settings.read().clone();
    settings::save(&st.config_dir, &new)?;
    *st.hk.shortcuts.write() = new.shortcuts.clone();
    if old.speech_model != new.speech_model || old.use_gpu != new.use_gpu {
        *st.engines.whisper.lock() = None;
    }
    if old.local_llm_model != new.local_llm_model || old.use_gpu != new.use_gpu {
        *st.engines.llama.lock() = None;
    }
    if old.data_policy != new.data_policy {
        let _ = st.db.apply_retention(&new.data_policy);
    }
    {
        use tauri_plugin_autostart::ManagerExt;
        let al = st.app.autolaunch();
        if new.launch_at_login {
            let _ = al.enable();
        } else {
            let _ = al.disable();
        }
    }
    if let Some(bar) = st.app.get_webview_window("bar") {
        if new.show_bar && new.bar_snooze_until <= chrono::Utc::now().timestamp() {
            let _ = bar.show();
        } else {
            let _ = bar.hide();
        }
    }
    *st.settings.write() = new;
    Ok(())
}

/// Time since the last dictation started (and nothing is in progress now).
pub fn idle_for(st: &St) -> Duration {
    if st.session.lock().phase != Phase::Idle {
        return Duration::ZERO;
    }
    st.last_activity.lock().elapsed()
}

pub fn mic_idle_tick(st: &St) {
    if !st.mic_test.load(Ordering::Relaxed) {
        st.rec.lock().close_if_idle();
    }
}

pub fn hk_enabled(st: &St, on: bool) {
    st.hk.enabled.store(on, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joins() {
        assert_eq!(join_with_context("Hello there.", "I said", false), " hello there.");
        assert_eq!(join_with_context("Hello.", "Done. ", false), "Hello.");
        assert_eq!(join_with_context("Hello.", "", false), "Hello.");
        assert_eq!(join_with_context("Hello.", "end.", true), " Hello.");
    }
}

#[allow(dead_code)]
fn _unused(_: AtomicU64) {}
