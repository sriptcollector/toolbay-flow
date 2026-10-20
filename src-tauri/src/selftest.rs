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
