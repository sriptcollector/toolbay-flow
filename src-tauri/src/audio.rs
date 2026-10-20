//! Microphone capture with cpal. Records mono f32, resamples to 16 kHz for whisper.

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

pub const SAMPLE_RATE: u32 = 16_000;

type LevelFn = Arc<dyn Fn(f32) + Send + Sync>;

pub struct Recorder {
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
    buf: Arc<Mutex<Vec<f32>>>,
    rate: Arc<Mutex<u32>>,
    pub started: Option<Instant>,
}

pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.input_devices()
        .map(|it| it.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

pub fn default_device_name() -> Option<String> {
    cpal::default_host().default_input_device().and_then(|d| d.name().ok())
}

impl Recorder {
    pub fn new() -> Self {
        Recorder { stop_tx: None, handle: None, buf: Arc::new(Mutex::new(Vec::new())), rate: Arc::new(Mutex::new(48_000)), started: None }
    }

    pub fn is_recording(&self) -> bool {
        self.stop_tx.is_some()
    }

    /// Starts capture on a dedicated thread (cpal streams are not Send on every platform).
    pub fn start(&mut self, device: Option<String>, on_level: LevelFn) -> Result<()> {
        if self.is_recording() {
            return Ok(());
        }
        self.buf.lock().clear();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        let buf = self.buf.clone();
        let rate = self.rate.clone();
        let handle = std::thread::Builder::new().name("mic".into()).spawn(move || {
            let res = (|| -> Result<cpal::Stream> {
                let host = cpal::default_host();
                let dev = match &device {
                    Some(name) => host
                        .input_devices()?
                        .find(|d| d.name().map(|n| &n == name).unwrap_or(false))
                        .or_else(|| host.default_input_device()),
                    None => host.default_input_device(),
                }
                .ok_or_else(|| anyhow!("No microphone found"))?;
                let cfg = dev.default_input_config()?;
                let channels = cfg.channels() as usize;
                *rate.lock() = cfg.sample_rate().0;
                let err_fn = |e| log::error!("mic stream error: {e}");
                let mut last_emit = Instant::now();
                let mut level = 0f32;
                let mut push = move |data: &[f32]| {
                    let mut b = buf.lock();
                    let mut sum = 0f32;
                    for frame in data.chunks(channels) {
                        let s = frame.iter().sum::<f32>() / channels as f32;
                        sum += s * s;
                        b.push(s);
                    }
                    drop(b);
                    let rms = (sum / (data.len().max(1) / channels.max(1)).max(1) as f32).sqrt();
                    level = level * 0.6 + rms * 0.4;
                    if last_emit.elapsed().as_millis() >= 33 {
                        last_emit = Instant::now();
                        on_level((level * 8.0).min(1.0));
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
                    f => return Err(anyhow!("Unsupported sample format {f:?}")),
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
                self.started = Some(Instant::now());
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("Microphone did not start")),
        }
    }

    /// Stops capture and returns 16 kHz mono samples.
    pub fn stop(&mut self) -> Vec<f32> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.started = None;
        let raw = std::mem::take(&mut *self.buf.lock());
        resample(&raw, *self.rate.lock(), SAMPLE_RATE)
    }
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

/// Simple energy-based speech check so silent presses do not paste hallucinations.
pub fn has_speech(samples: &[f32]) -> bool {
    if samples.len() < (SAMPLE_RATE as usize) / 5 {
        return false;
    }
    let frame = 320; // 20 ms
    let mut loud = 0;
    for ch in samples.chunks(frame) {
        let e = (ch.iter().map(|s| s * s).sum::<f32>() / ch.len() as f32).sqrt();
        if e > 0.012 {
            loud += 1;
        }
    }
    loud >= 6
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
