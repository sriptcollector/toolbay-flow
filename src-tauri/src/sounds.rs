//! Short synthesized UI sounds (no audio files shipped) and muting other audio
//! while dictating.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

#[derive(Clone, Copy)]
pub enum Sound {
    Start,
    Stop,
    Lock,
    Cancel,
    Error,
}

/// (frequency Hz, start s, duration s, gain)
fn notes(s: Sound) -> Vec<(f32, f32, f32, f32)> {
    match s {
        Sound::Start => vec![(660.0, 0.0, 0.07, 0.22), (990.0, 0.06, 0.09, 0.2)],
        Sound::Stop => vec![(880.0, 0.0, 0.07, 0.2), (587.0, 0.06, 0.1, 0.18)],
        Sound::Lock => vec![(784.0, 0.0, 0.05, 0.2), (1047.0, 0.05, 0.05, 0.2), (1319.0, 0.1, 0.08, 0.18)],
        Sound::Cancel => vec![(440.0, 0.0, 0.09, 0.16)],
        Sound::Error => vec![(330.0, 0.0, 0.12, 0.2), (247.0, 0.12, 0.16, 0.2)],
    }
}

pub fn play(s: Sound) {
    std::thread::spawn(move || {
        let _ = (|| -> anyhow::Result<()> {
            let host = cpal::default_host();
            let dev = host.default_output_device().ok_or_else(|| anyhow::anyhow!("no output"))?;
            let cfg = dev.default_output_config()?;
            let rate = cfg.sample_rate().0 as f32;
            let ch = cfg.channels() as usize;
            let ns = notes(s);
            let total = ns.iter().map(|n| n.1 + n.2).fold(0.0, f32::max) + 0.05;
            let len = (total * rate) as usize;
            let mut buf = vec![0f32; len];
            for (f, st, d, g) in ns {
                let a = (st * rate) as usize;
                let n = (d * rate) as usize;
                for i in 0..n {
                    let t = i as f32 / rate;
                    // Soft attack and exponential decay.
                    let env = (t / 0.004).min(1.0) * (-t * 28.0).exp();
                    let v = (2.0 * std::f32::consts::PI * f * t).sin() * 0.8 + (4.0 * std::f32::consts::PI * f * t).sin() * 0.12;
                    if a + i < len {
                        buf[a + i] += v * env * g;
                    }
                }
            }
            let mut pos = 0usize;
            let stream = match cfg.sample_format() {
                cpal::SampleFormat::F32 => dev.build_output_stream(
                    &cfg.into(),
                    move |out: &mut [f32], _| {
                        for frame in out.chunks_mut(ch) {
                            let v = if pos < buf.len() { buf[pos] } else { 0.0 };
                            pos += 1;
                            for x in frame.iter_mut() {
                                *x = v;
                            }
                        }
                    },
                    |e| log::warn!("sound: {e}"),
                    None,
                )?,
                cpal::SampleFormat::I16 => dev.build_output_stream(
                    &cfg.into(),
                    move |out: &mut [i16], _| {
                        for frame in out.chunks_mut(ch) {
                            let v = if pos < buf.len() { buf[pos] } else { 0.0 };
                            pos += 1;
                            for x in frame.iter_mut() {
                                *x = (v * 32767.0) as i16;
                            }
                        }
                    },
                    |e| log::warn!("sound: {e}"),
                    None,
                )?,
                _ => return Ok(()),
            };
            stream.play()?;
            std::thread::sleep(std::time::Duration::from_secs_f32(total + 0.1));
            Ok(())
        })();
    });
}

/// Mutes the default speakers while dictating and restores the previous state.
pub mod media {
    #[cfg(windows)]
    mod imp {
        use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
        use windows::Win32::Media::Audio::{eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

        fn endpoint() -> Option<IAudioEndpointVolume> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
                let dev = en.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
                dev.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None).ok()
            }
        }

        /// Returns true if we muted (so we must unmute later).
        pub fn mute() -> bool {
            let Some(ep) = endpoint() else { return false };
            unsafe {
                let was = ep.GetMute().map(|b| b.as_bool()).unwrap_or(true);
                if !was {
                    return ep.SetMute(true, std::ptr::null()).is_ok();
                }
            }
            false
        }

        pub fn unmute() {
            if let Some(ep) = endpoint() {
                unsafe {
                    let _ = ep.SetMute(false, std::ptr::null());
                }
            }
        }
    }

    #[cfg(not(windows))]
    mod imp {
        pub fn mute() -> bool {
            false
        }
        pub fn unmute() {}
    }

    pub use imp::{mute, unmute};
}
