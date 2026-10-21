//! Microphone capture with cpal. Records mono f32, resamples to 16 kHz for whisper.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

pub const SAMPLE_RATE: u32 = 16_000;

type LevelFn = Arc<dyn Fn(f32) + Send + Sync>;

/// How long the microphone stays open after a dictation, so the next one starts instantly.
const WARM_SECS: u64 = 120;
/// Audio kept from just before the hotkey (people often start talking as they press).
const PREROLL_MS: u32 = 300;

struct Capture {
    raw: Vec<f32>,
    rate: u32,
    recording: bool,
    on_level: Option<LevelFn>,
    level: f32,
    last_emit: Instant,
}

pub struct Recorder {
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
    cap: Arc<Mutex<Capture>>,
    device: Option<Option<String>>,
    /// 16 kHz copy of the recording, extended incrementally for streaming.
    conv: Vec<f32>,
    conv_pos: usize,
    pub started: Option<Instant>,
    last_used: Instant,
}

pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

impl Recorder {
    pub fn new() -> Self {
        Recorder {
            stop_tx: None,
            handle: None,
            cap: Arc::new(Mutex::new(Capture { raw: vec![], rate: 48_000, recording: false, on_level: None, level: 0.0, last_emit: Instant::now() })),
            device: None,
            conv: vec![],
            conv_pos: 0,
            started: None,
            last_used: Instant::now(),
        }
    }

    pub fn is_recording(&self) -> bool {
        self.cap.lock().recording
    }

    fn is_open(&self) -> bool {
        self.stop_tx.is_some() && self.handle.as_ref().map(|h| !h.is_finished()).unwrap_or(false)
    }

    /// Opens the input stream on its own thread (cpal streams are not Send everywhere).
    /// Shared-mode capture on the default "console" role, so Windows does not duck
    /// other apps' audio the way it does for communication streams.
    fn open(&mut self, device: Option<String>) -> Result<()> {
        self.close();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        let cap = self.cap.clone();
        let dev_name = device.clone();
        let handle = std::thread::Builder::new().name("mic".into()).spawn(move || {
            let res = (|| -> Result<cpal::Stream> {
                let host = cpal::default_host();
                let dev = match &dev_name {
                    Some(name) => host
                        .input_devices()?
                        .find(|d| d.name().map(|n| &n == name).unwrap_or(false))
                        .or_else(|| host.default_input_device()),
                    None => host.default_input_device(),
                }
                .ok_or_else(|| anyhow!("No microphone found. Plug one in, or pick another one in Settings."))?;
                let cfg = dev.default_input_config()?;
                let channels = cfg.channels() as usize;
                let rate = cfg.sample_rate().0;
                cap.lock().rate = rate;
                let preroll = (rate * PREROLL_MS / 1000) as usize;
                let err_fn = |e| log::error!("mic stream error: {e}");
                let cap2 = cap.clone();
                let mut push = move |data: &[f32]| {
                    let mut c = cap2.lock();
                    let mut sum = 0f32;
                    let mut n = 0usize;
                    for frame in data.chunks(channels) {
                        let v = frame.iter().sum::<f32>() / channels as f32;
                        sum += v * v;
                        n += 1;
                        c.raw.push(v);
                    }
                    if !c.recording {
                        // Warm and idle: keep only the pre-roll window in memory.
                        let len = c.raw.len();
                        if len > preroll * 2 {
                            c.raw.drain(..len - preroll);
                        }
                        return;
                    }
                    let rms = (sum / n.max(1) as f32).sqrt();
                    c.level = c.level * 0.6 + rms * 0.4;
                    if c.last_emit.elapsed().as_millis() >= 33 {
                        c.last_emit = Instant::now();
                        if let Some(f) = c.on_level.clone() {
                            let l = (c.level * 8.0).min(1.0);
                            drop(c);
                            f(l);
                        }
                    }
                };
                let stream = match cfg.sample_format() {
                    cpal::SampleFormat::F32 => dev.build_input_stream(&cfg.into(), move |d: &[f32], _| push(d), err_fn, None)?,
                    cpal::SampleFormat::I16 => dev.build_input_stream(
                        &cfg.into(),
                        move |d: &[i16], _| {
                            let v: Vec<f32> = d.iter().map(|s| *s as f32 / 32768.0).collect();
                            push(&v)
                        },
                        err_fn,
                        None,
                    )?,
                    cpal::SampleFormat::U16 => dev.build_input_stream(
                        &cfg.into(),
                        move |d: &[u16], _| {
                            let v: Vec<f32> = d.iter().map(|s| (*s as f32 - 32768.0) / 32768.0).collect();
                            push(&v)
                        },
                        err_fn,
                        None,
                    )?,
                    cpal::SampleFormat::I32 => dev.build_input_stream(
                        &cfg.into(),
                        move |d: &[i32], _| {
                            let v: Vec<f32> = d.iter().map(|s| *s as f32 / 2147483648.0).collect();
                            push(&v)
                        },
                        err_fn,
                        None,
                    )?,
                    f => return Err(anyhow!("This microphone uses a format Toolbay Flow can't read ({f:?}). Pick another one in Settings.")),
                };
                stream.play()?;
                Ok(stream)
            })();
            match res {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })?;
        match ready_rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => {
                self.stop_tx = Some(stop_tx);
                self.handle = Some(handle);
                self.device = Some(device);
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("The microphone didn't respond. Check it's plugged in and not in use by another app.")),
        }
    }

    pub fn close(&mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.device = None;
        let mut c = self.cap.lock();
        c.raw.clear();
        c.recording = false;
    }

    /// Opens the microphone without recording, keeping the last 300 ms in memory,
    /// so a dictation that starts right after includes the first syllable.
    pub fn warm(&mut self, device: Option<String>) -> Result<()> {
        if self.is_recording() || (self.is_open() && self.device.as_ref() == Some(&device)) {
            self.last_used = Instant::now();
            return Ok(());
        }
        self.open(device)?;
        self.last_used = Instant::now();
        Ok(())
    }

    /// Starts recording. Reuses the warm stream when possible (instant start, with pre-roll).
    pub fn start(&mut self, device: Option<String>, on_level: LevelFn) -> Result<()> {
        if self.is_recording() {
            return Ok(());
        }
        let reuse = self.is_open() && self.device.as_ref() == Some(&device);
        if !reuse {
            self.open(device)?;
        }
        {
            let mut c = self.cap.lock();
            c.on_level = Some(on_level);
            c.recording = true;
            c.level = 0.0;
            if !reuse {
                c.raw.clear();
            }
        }
        self.conv.clear();
        self.conv_pos = 0;
        self.started = Some(Instant::now());
        self.last_used = Instant::now();
        Ok(())
    }

    /// 16 kHz samples recorded so far (for transcribing while the user still talks).
    pub fn snapshot(&mut self) -> Vec<f32> {
        let (chunk, rate) = {
            let c = self.cap.lock();
            if self.conv_pos > c.raw.len() {
                self.conv_pos = 0;
            }
            (c.raw[self.conv_pos..].to_vec(), c.rate)
        };
        self.conv_pos += chunk.len();
        self.conv.extend(resample(&chunk, rate, SAMPLE_RATE));
        self.conv.clone()
    }

    /// Stops recording and returns 16 kHz mono samples. The stream stays warm for a while.
    pub fn stop(&mut self) -> Vec<f32> {
        if !self.is_recording() {
            return vec![];
        }
        let _ = self.snapshot();
        {
            let mut c = self.cap.lock();
            c.recording = false;
            c.on_level = None;
            c.raw.clear();
        }
        self.conv_pos = 0;
        self.started = None;
        self.last_used = Instant::now();
        std::mem::take(&mut self.conv)
    }

    /// Closes the microphone once it has been idle for a while (called periodically).
    pub fn close_if_idle(&mut self) {
        if self.is_open() && !self.is_recording() && self.last_used.elapsed().as_secs() >= WARM_SECS {
            self.close();
        }
    }
}

/// Per-recording loudness levels. Thresholds come from the recording itself, so a
/// quiet microphone or a soft voice is not mistaken for silence. (A fixed 0.008
/// threshold cut real words out of quiet dictations.)
pub struct Levels {
    pub frames: Vec<f32>,
    pub threshold: f32,
}

pub const FRAME: usize = 320; // 20 ms at 16 kHz

pub fn levels(samples: &[f32]) -> Levels {
    let frames: Vec<f32> = samples.chunks(FRAME).map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len().max(1) as f32).sqrt()).collect();
    if frames.is_empty() {
        return Levels { frames, threshold: 0.0015 };
    }
    let mut sorted = frames.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let floor = sorted[sorted.len() / 10]; // background noise
    let loud = sorted[(sorted.len() * 95) / 100]; // speech peaks
    // Silence = close to the noise floor. Never above 3% of the loud level, so
    // quiet speech always counts as speech.
    let threshold = (floor * 2.5).max(0.0012).min((loud * 0.03).max(0.0012));
    Levels { frames, threshold }
}

/// Trims long leading/trailing silence and shortens only very long pauses.
/// Conservative on purpose: whisper copes with silence, but not with missing words.
pub fn compact_silence(samples: &[f32]) -> Vec<f32> {
    let lv = levels(samples);
    let voiced: Vec<bool> = lv.frames.iter().map(|e| *e > lv.threshold).collect();
    let first = voiced.iter().position(|v| *v);
    let last = voiced.iter().rposition(|v| *v);
    let (Some(first), Some(last)) = (first, last) else { return samples.to_vec() };
    let pad = 25; // 500 ms around speech
    let start = first.saturating_sub(pad);
    let end = (last + pad + 1).min(voiced.len());
    let max_gap = 50; // pauses longer than 1 s are shortened to 1 s
    let mut out = Vec::with_capacity(samples.len());
    let mut gap = 0;
    for i in start..end {
        if voiced[i] {
            gap = 0;
        } else {
            gap += 1;
            if gap > max_gap {
                continue;
            }
        }
        let a = i * FRAME;
        let b = ((i + 1) * FRAME).min(samples.len());
        out.extend_from_slice(&samples[a..b]);
    }
    out
}

/// Where to cut for streaming: the middle of the last clear pause (>= 600 ms)
/// that starts after `from` and ends at least 1 s before the end. Cutting in the
/// middle of a long pause means no word sits on a chunk boundary.
pub fn find_cut(samples: &[f32], from: usize) -> Option<usize> {
    let lv = levels(samples);
    let need = 30; // 600 ms
    let first = from / FRAME;
    let last = (samples.len().saturating_sub(16_000)) / FRAME;
    let mut run = 0;
    let mut best = None;
    for i in first..last.min(lv.frames.len()) {
        if lv.frames[i] <= lv.threshold {
            run += 1;
            if run >= need {
                best = Some((i + 1 - run / 2) * FRAME);
            }
        } else {
            run = 0;
        }
    }
    best
}

/// Is there any speech at all? (Skips silent key presses.)
pub fn has_speech(samples: &[f32]) -> bool {
    if samples.len() < (SAMPLE_RATE as usize) / 5 {
        return false;
    }
    let lv = levels(samples);
    // At least 200 ms clearly above the noise floor, and not just hiss.
    let t = lv.threshold.max(0.003);
    lv.frames.iter().filter(|e| **e > t).count() >= 10
}

/// Seconds of speech in a recording (for hallucination checks).
pub fn speech_seconds(samples: &[f32]) -> f32 {
    let lv = levels(samples);
    lv.frames.iter().filter(|e| **e > lv.threshold.max(0.003)).count() as f32 * FRAME as f32 / SAMPLE_RATE as f32
}

/// Band-limited-enough resampler: box low-pass then linear interpolation.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let filtered: Vec<f32> = if ratio > 1.0 {
        let w = ratio.round().max(1.0) as usize;
        let mut out = Vec::with_capacity(input.len());
        let mut acc = 0f32;
        for i in 0..input.len() {
            acc += input[i];
            if i >= w {
                acc -= input[i - w];
            }
            out.push(acc / w.min(i + 1) as f32);
        }
        out
    } else {
        input.to_vec()
    };
    let n = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = filtered[idx.min(filtered.len() - 1)];
        let b = filtered[(idx + 1).min(filtered.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

pub fn to_wav(samples: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec { channels: 1, sample_rate: SAMPLE_RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut cur = std::io::Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cur, spec).unwrap();
        for s in samples {
            let _ = w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
        let _ = w.finalize();
    }
    cur.into_inner()
}

pub fn from_wav(bytes: &[u8]) -> Result<Vec<f32>> {
    let mut r = hound::WavReader::new(std::io::Cursor::new(bytes))?;
    Ok(r.samples::<i16>().filter_map(|s| s.ok()).map(|s| s as f32 / 32768.0).collect())
}

/// Normalizes quiet recordings so whisper hears them well.
pub fn normalize(samples: &mut [f32]) {
    let peak = samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    if peak > 0.0 && peak < 0.5 {
        let g = (0.7 / peak).min(8.0);
        for s in samples.iter_mut() {
            *s *= g;
        }
    }
}
