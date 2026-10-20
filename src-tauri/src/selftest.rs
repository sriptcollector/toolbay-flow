//! `toolbay-flow --selftest <whisper-url> <llama-url> <wav>...`
//! Runs recordings through the same speech + cleanup code the app uses and
//! prints the results as JSON lines. Used for automated checks; no UI.

use crate::llm::{self, CleanupInput, Llm};
use crate::settings::{CleanupLevel, LlmProvider, Settings, Style};
use crate::{audio, textproc};
use std::time::{Duration, Instant};

pub fn run(args: &[String]) -> i32 {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        let (Some(whisper), Some(llama)) = (args.first(), args.get(1)) else {
            eprintln!("usage: --selftest <whisper-url> <llama-url> <wav>...");
            return 2;
        };
        let s = Settings::default();
        let l = Llm { provider: LlmProvider::Local, local_base: Some(llama.clone()), cloud: s.cloud_llm.clone(), anthropic: s.anthropic.clone() };
        for wav in &args[2..] {
            let parts: Vec<&str> = wav.splitn(2, '@').collect();
            let (path, app) = (parts[0], parts.get(1).copied().unwrap_or("Notepad"));
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    println!("{}", serde_json::json!({"file": path, "error": e.to_string()}));
                    continue;
                }
            };
            let samples = audio::from_wav(&bytes).unwrap_or_default();
            let t0 = Instant::now();
            let mut padded = vec![0f32; 4800];
            padded.extend_from_slice(&samples);
            padded.extend(std::iter::repeat(0f32).take(4800));
            let raw = llm::transcribe(whisper, audio::to_wav(&padded), "en", "Toolbay").await.unwrap_or_default();
            let raw = textproc::strip_artifacts(&raw);
            let asr_ms = t0.elapsed().as_millis();
            let category = textproc::app_category(app, app, "", &Default::default());
            let style: Style = match category.as_str() {
                "personal" => s.styles.personal.clone(),
                "work" => s.styles.work.clone(),
                "email" => s.styles.email.clone(),
                _ => s.styles.other.clone(),
            };
            let input = CleanupInput {
                transcript: &raw,
                level: &CleanupLevel::Medium,
                style: &style,
                category: &category,
                app,
                window_title: "",
                before: "",
                after: "",
                vocabulary: "Toolbay",
                language: "en",
                name: "Orion",
            };
            let (sys, msgs) = llm::cleanup_messages(&input);
            let t1 = Instant::now();
            let out = l.chat(&sys, &msgs, 400, Duration::from_secs(30)).await.unwrap_or_default();
            let ok = llm::plausible(&raw, &out);
            let text = if ok { textproc::apply_style(&textproc::apply_spoken_punctuation(&out), &style) } else { textproc::rule_cleanup(&raw, &style) };
            println!(
                "{}",
                serde_json::json!({"file": path, "app": app, "category": category, "raw": raw, "text": text, "llm_used": ok,
                    "speech_ms": asr_ms, "cleanup_ms": t1.elapsed().as_millis()})
            );
        }
        0
    })
}

/// `toolbay-flow --bench <whisper-url> <llama-url> <runs> <wav>...`
/// Release-to-text latency with streaming: chunks cut at pauses are transcribed
/// "while talking" (not timed); after release only the tail plus cleanup are timed.
pub fn bench(args: &[String]) -> i32 {
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    rt.block_on(async {
        let (Some(whisper), Some(llama), Some(runs)) = (args.first(), args.get(1), args.get(2).and_then(|r| r.parse::<usize>().ok())) else {
            eprintln!("usage: --bench <whisper-url> <llama-url> <runs> <wav>...");
            return 2;
        };
        let s = Settings::default();
        let l = Llm { provider: LlmProvider::Local, local_base: Some(llama.clone()), cloud: s.cloud_llm.clone(), anthropic: s.anthropic.clone() };
        let style = Style::Formal;
        for path in &args[3..] {
            let samples = audio::from_wav(&std::fs::read(path).unwrap_or_default()).unwrap_or_default();
            // Emulate the stream loop: look for a cut every 350 ms of "recording".
            let mut committed = 0usize;
            let mut t = 16_000 * 4;
            let mut parts: Vec<String> = vec![];
            while t <= samples.len() {
                if t >= committed + 16_000 * 4 {
                    if let Some(cut) = audio::find_cut(&samples[..t], committed + 16_000 * 3) {
                        if cut > committed {
                            let txt = tx(whisper, &samples[committed..cut], &parts.join(" ")).await;
                            parts.push(txt);
                            committed = cut;
                        }
                    }
                }
                t += 5600;
            }
            let mut times = vec![];
            let mut last = String::new();
            for _ in 0..runs {
                let t0 = Instant::now();
                let tail = tx(whisper, &samples[committed..], &parts.join(" ")).await;
                let raw = [parts.join(" "), tail].join(" ").trim().to_string();
                let text = if raw.split_whitespace().count() <= 3 {
                    textproc::rule_cleanup(&raw, &style)
                } else {
                    let input = CleanupInput {
                        transcript: &raw, level: &CleanupLevel::Medium, style: &style, category: "other", app: "Notepad",
                        window_title: "", before: "", after: "", vocabulary: "", language: "en", name: "",
                    };
                    let (sys, msgs) = llm::cleanup_messages(&input);
                    let max = (raw.split_whitespace().count() as u32 * 2 + 48).min(1500);
                    l.chat(&sys, &msgs, max, Duration::from_secs(30)).await.unwrap_or_default()
                };
                times.push(t0.elapsed().as_millis() as u64);
                last = text;
            }
            times.sort();
            let pct = |p: f64| times[((times.len() as f64 - 1.0) * p).round() as usize];
            println!(
                "{}",
                serde_json::json!({"file": path, "audio_s": samples.len() as f64 / 16000.0, "streamed_chunks": parts.len(),
                    "tail_s": (samples.len() - committed) as f64 / 16000.0, "p50_ms": pct(0.5), "p95_ms": pct(0.95), "text": last})
            );
        }
        0
    })
}

async fn tx(whisper: &str, s: &[f32], prev: &str) -> String {
    let mut c = audio::compact_silence(s);
    audio::normalize(&mut c);
    let mut padded = vec![0f32; 3200];
    padded.extend_from_slice(&c);
    padded.extend(std::iter::repeat(0f32).take(3200));
    let raw = llm::transcribe(whisper, audio::to_wav(&padded), "en", &llm::tail(prev, 300)).await.unwrap_or_default();
    textproc::strip_artifacts(&raw)
}
