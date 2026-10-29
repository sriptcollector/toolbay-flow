//! Automatic model choice. Flow looks at the computer (GPU, video memory, RAM,
//! CPU cores) and picks the speech and cleanup models that keep dictation feeling
//! instant. A small starter model ships inside the installer, so dictation works
//! right after install; better models download quietly in the background.

use crate::dictation::St;
use crate::engines::{self, ModelInfo};
use crate::settings::CleanupLevel;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

/// Speech model bundled in the installer (works on any CPU, fully offline).
pub const STARTER_SPEECH: &str = "base.en-q5";

#[derive(Clone, Debug, Serialize, Default)]
pub struct Hardware {
    pub gpu_vendor: String, // "nvidia" | "amd" | "intel" | "apple" | ""
    pub gpu_name: String,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub threads: usize,
}

impl Hardware {
    pub fn fingerprint(&self) -> String {
        format!("{}|{}|{}|{}", self.gpu_vendor, self.vram_mb / 1024, self.ram_mb / 1024, self.threads)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Choice {
    pub tier: &'static str,
    pub speech_model: String,
    pub llm_model: String,
    pub cleanup: CleanupLevel,
    pub why: String,
}

pub fn detect() -> Hardware {
    let mut h = Hardware { threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4), ..Default::default() };
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Graphics::Dxgi::*;
        use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        if GlobalMemoryStatusEx(&mut m).is_ok() {
            h.ram_mb = m.ullTotalPhys / (1024 * 1024);
        }
        if let Ok(f) = CreateDXGIFactory1::<IDXGIFactory1>() {
            let mut i = 0;
            while let Ok(a) = f.EnumAdapters1(i) {
                i += 1;
                let Ok(d) = a.GetDesc1() else { continue };
                if (d.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
                    continue;
                }
                let vram = (d.DedicatedVideoMemory / (1024 * 1024)) as u64;
                let vendor = match d.VendorId {
                    0x10DE => "nvidia",
                    0x1002 => "amd",
                    0x8086 => "intel",
                    _ => "",
                };
                // Keep the adapter with the most dedicated memory.
                if vram > h.vram_mb {
                    h.vram_mb = vram;
                    h.gpu_vendor = vendor.into();
                    let n = d.Description.iter().position(|c| *c == 0).unwrap_or(d.Description.len());
                    h.gpu_name = String::from_utf16_lossy(&d.Description[..n]);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output() {
            h.ram_mb = String::from_utf8_lossy(&o.stdout).trim().parse::<u64>().unwrap_or(0) / (1024 * 1024);
        }
        if cfg!(target_arch = "aarch64") {
            h.gpu_vendor = "apple".into();
            h.gpu_name = "Apple silicon".into();
            h.vram_mb = h.ram_mb; // unified memory
        }
    }
    h
}

/// The best models for this computer that still feel instant.
pub fn recommend(h: &Hardware, language: &str) -> Choice {
    let english = language == "en";
    let ml = |en: &str, multi: &str| if english { en.to_string() } else { multi.to_string() };
    let nvidia = h.gpu_vendor == "nvidia";
    let apple = h.gpu_vendor == "apple";
    let other_gpu = matches!(h.gpu_vendor.as_str(), "amd" | "intel") && h.vram_mb >= 3500;
    // Apple silicon shares its memory between the models and everything else: the 4B cleanup
    // model only from 24 GB, and under 12 GB a small speech model with rule cleanup.
    if (nvidia && h.vram_mb >= 6000) || (apple && h.ram_mb >= 24_000) {
        Choice {
            tier: "fast-gpu",
            speech_model: "large-v3-turbo-q5".into(),
            llm_model: "qwen3-4b-instruct".into(),
            cleanup: CleanupLevel::Medium,
            why: format!("{} with {} GB", h.gpu_name, h.vram_mb / 1024),
        }
    } else if nvidia && h.vram_mb >= 3000 || apple && h.ram_mb >= 12_000 {
        Choice {
            tier: "gpu",
            speech_model: "large-v3-turbo-q5".into(),
            llm_model: "qwen3-1.7b".into(),
            cleanup: CleanupLevel::Medium,
            why: format!("{} with {} GB", h.gpu_name, h.vram_mb / 1024),
        }
    } else if apple {
        Choice {
            tier: "apple-light",
            speech_model: ml("small.en-q5", "small-q5"),
            llm_model: "qwen3-1.7b".into(),
            cleanup: CleanupLevel::Light,
            why: format!("{} with {} GB memory", h.gpu_name, h.ram_mb / 1024),
        }
    } else if other_gpu {
        Choice {
            tier: "gpu-cleanup",
            speech_model: ml("small.en-q5", "small-q5"),
            llm_model: "qwen3-1.7b".into(),
            cleanup: CleanupLevel::Medium,
            why: format!("{} (cleanup on GPU, speech on CPU)", h.gpu_name),
        }
    } else if h.threads >= 8 && h.ram_mb >= 12000 {
        Choice {
            tier: "cpu",
            speech_model: ml("small.en-q5", "small-q5"),
            llm_model: "qwen3-1.7b".into(),
            cleanup: CleanupLevel::Light,
            why: format!("{}-thread CPU, no fast GPU", h.threads),
        }
    } else {
        Choice {
            tier: "light",
            speech_model: ml("base.en-q5", "base-q5"),
            llm_model: "qwen3-1.7b".into(),
            cleanup: CleanupLevel::Light,
            why: format!("{}-thread CPU, {} GB memory", h.threads, h.ram_mb / 1024),
        }
    }
}

/// Where a model file can be found: downloaded copy first, then the installer's bundle.
pub fn model_file(st: &St, m: &ModelInfo) -> Option<PathBuf> {
    let p = st.paths.model_path(m);
    if p.exists() {
        return Some(p);
    }
    let b = st.app.path().resource_dir().ok()?.join("bundled").join("models").join(m.file);
    b.exists().then_some(b)
}

/// The speech model to use right now: the chosen one if present, else the starter.
pub fn current_speech_path(st: &St) -> Option<(String, PathBuf)> {
    let want = st.settings.read().speech_model.clone();
    if let Some(m) = engines::model(&want) {
        if let Some(p) = model_file(st, m) {
            return Some((want, p));
        }
    }
    let lang = st.settings.read().language.clone();
    let starters: &[&str] = if lang == "en" { &[STARTER_SPEECH, "base-q5"] } else { &["base-q5", STARTER_SPEECH] };
    for id in starters {
        if let Some(m) = engines::model(id) {
            if let Some(p) = model_file(st, m) {
                return Some((id.to_string(), p));
            }
        }
    }
    None
}

/// Median cleanup request time above which Auto steps the cleanup model down.
pub const SLOW_CLEANUP_MS: u64 = 700;

static UPGRADING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
struct Status {
    busy: bool,
    message: String,
}

/// In Auto mode: re-check the hardware, update the choice, and download what is
/// missing in the background. Safe to call often.
pub fn auto_setup(st: &St) {
    if st.settings.read().model_mode != "auto" || UPGRADING.swap(true, Ordering::SeqCst) {
        return;
    }
    let st = st.clone();
    tauri::async_runtime::spawn(async move {
        let hw = detect();
        let s = st.settings.read().clone();
        let mut choice = recommend(&hw, &s.language);
        // Respect slowdowns measured on this computer.
        if s.auto_slow_cleanup && matches!(choice.cleanup, CleanupLevel::Medium) {
            if choice.llm_model == "qwen3-4b-instruct" {
                choice.llm_model = "qwen3-1.7b".into();
            } else {
                choice.cleanup = CleanupLevel::Light;
            }
        }
        if s.auto_slow_speech {
            // Step down one size, keeping the multilingual model for other languages.
            let english = s.language == "en";
            choice.speech_model = match choice.speech_model.as_str() {
                "large-v3-turbo-q5" => (if english { "small.en-q5" } else { "small-q5" }).into(),
                "small.en-q5" => "base.en-q5".into(),
                "small-q5" => "base-q5".into(),
                other => other.into(),
            };
        }
        if s.cleanup_chosen {
            // The person picked the cleanup level: Auto only picks the models.
            choice.cleanup = s.cleanup_level.clone();
        }
        let changed = s.speech_model != choice.speech_model || s.local_llm_model != choice.llm_model || s.cleanup_level != choice.cleanup || s.hw_fingerprint != hw.fingerprint();
        if changed {
            let _ = crate::dictation::update_settings(&st, |s| {
                s.speech_model = choice.speech_model.clone();
                s.local_llm_model = choice.llm_model.clone();
                if !s.cleanup_chosen {
                    s.cleanup_level = choice.cleanup.clone();
                }
                s.hw_fingerprint = hw.fingerprint();
                s.auto_reason = choice.why.clone();
            });
        }
        let need_llm = matches!(choice.cleanup, CleanupLevel::Medium | CleanupLevel::High);
        let emit = |busy: bool, message: String| {
            let _ = st.app.emit("model-status", Status { busy, message });
        };
        let res: anyhow::Result<()> = async {
            let speech_missing = engines::model(&choice.speech_model).map(|m| model_file(&st, m).is_none()).unwrap_or(false);
            if speech_missing || engines::pick_engine("whisper", s.use_gpu).map(|e| st.paths.engine_exe(e).is_none() && !bundled_engine(&st, e)).unwrap_or(false) {
                emit(true, "Getting a faster speech model…".into());
                if let Some(e) = engines::pick_engine("whisper", s.use_gpu) {
                    if !bundled_engine(&st, e) {
                        engines::ensure_engine(&st.app, &st.paths, e).await?;
                    }
                }
                engines::ensure_model(&st.app, &st.paths, &choice.speech_model).await?;
                *st.engines.whisper.lock() = None;
            }
            if need_llm && engines::model(&choice.llm_model).map(|m| model_file(&st, m).is_none()).unwrap_or(false) {
                emit(true, "Getting the AI cleanup model…".into());
                if let Some(e) = engines::pick_engine("llama", s.use_gpu) {
                    engines::ensure_engine(&st.app, &st.paths, e).await?;
                }
                engines::ensure_model(&st.app, &st.paths, &choice.llm_model).await?;
            }
            Ok(())
        }
        .await;
        UPGRADING.store(false, Ordering::SeqCst);
        match res {
            Ok(()) => {
                emit(false, String::new());
                crate::dictation::prewarm(&st);
            }
            Err(e) => {
                log::warn!("model upgrade failed: {e:#}");
                emit(false, "Couldn't download the better models yet. Toolbay Flow keeps working and will retry when you're online.".into());
            }
        }
    });
}

/// Is this engine shipped inside the installer?
pub fn bundled_engine(st: &St, e: &engines::EngineInfo) -> bool {
    bundled_engine_exe(st, e).is_some()
}

pub fn bundled_engine_exe(st: &St, e: &engines::EngineInfo) -> Option<PathBuf> {
    let dir = st.app.path().resource_dir().ok()?.join("bundled").join("engines").join(e.id);
    let exe = dir.join(e.exe);
    exe.exists().then_some(exe)
}

/// Rolling latency check: if this computer is too slow for the chosen models,
/// step down once and let Auto pick again. `speech_ms` is the speech engine's own request time
/// for `audio_ms` of audio, `cleanup_ms` the cleanup model's request time (no waits, no
/// engine start-up, no pasting).
pub fn record_latency(st: &St, audio_ms: u64, speech_ms: u64, cleanup_ms: Option<u64>) {
    use parking_lot::Mutex;
    use once_cell::sync::Lazy;
    static SAMPLES: Lazy<Mutex<Vec<(f32, Option<u64>)>>> = Lazy::new(|| Mutex::new(vec![]));
    if st.settings.read().model_mode != "auto" || audio_ms < 1500 {
        return;
    }
    let rtf = speech_ms as f32 / audio_ms as f32;
    let mut v = SAMPLES.lock();
    v.push((rtf, cleanup_ms));
    if v.len() < 5 {
        return;
    }
    let mut rtfs: Vec<f32> = v.iter().map(|x| x.0).collect();
    rtfs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut cl: Vec<u64> = v.iter().filter_map(|x| x.1).collect();
    cl.sort();
    v.clear();
    drop(v);
    let slow_speech = rtfs[rtfs.len() / 2] > 0.35;
    let slow_cleanup = !cl.is_empty() && cl[cl.len() / 2] > SLOW_CLEANUP_MS;
    let changed = crate::dictation::update_settings(st, |s| {
        s.auto_slow_speech |= slow_speech;
        s.auto_slow_cleanup |= slow_cleanup;
    });
    if matches!(changed, Ok(true)) {
        auto_setup(st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(ram_gb: u64) -> Hardware {
        let ram_mb = ram_gb * 1024;
        Hardware { gpu_vendor: "apple".into(), gpu_name: "Apple silicon".into(), vram_mb: ram_mb, ram_mb, threads: 8 }
    }

    #[test]
    fn apple_silicon_tiers() {
        let c = recommend(&mac(8), "en");
        assert_eq!((c.speech_model.as_str(), &c.cleanup), ("small.en-q5", &CleanupLevel::Light));
        assert_eq!(recommend(&mac(8), "de").speech_model, "small-q5");
        let c = recommend(&mac(16), "en");
        assert_eq!((c.speech_model.as_str(), c.llm_model.as_str(), &c.cleanup), ("large-v3-turbo-q5", "qwen3-1.7b", &CleanupLevel::Medium));
        assert_eq!(recommend(&mac(18), "en").llm_model, "qwen3-1.7b");
        assert_eq!(recommend(&mac(24), "en").llm_model, "qwen3-4b-instruct");
        assert_eq!(recommend(&mac(36), "fr").speech_model, "large-v3-turbo-q5");
    }
}
