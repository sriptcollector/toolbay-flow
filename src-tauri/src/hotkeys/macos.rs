//! macOS key listener (CGEventTap through rdev). Needs Accessibility + Input
//! Monitoring permission. Keys are observed, not swallowed.

use super::mac_codes::*;
use super::{Matcher, Shared, Sink};
use parking_lot::Mutex;
use std::sync::Arc;

fn code(k: rdev::Key) -> Option<u32> {
    use rdev::Key::*;
    Some(match k {
        Function => FN,
        ControlLeft => CTRL,
        ControlRight => 62,
        Alt => ALT,
        AltGr => 61,
        MetaLeft => CMD,
        MetaRight => 54,
        ShiftLeft | ShiftRight => SHIFT,
        Space => SPACE,
        Escape => ESC,
        KeyV => KEY_V,
        KeyC => KEY_C,
        Num1 => KEY_1,
        Num2 => KEY_2,
        Num3 => KEY_3,
        Unknown(c) => c,
        KeyA => 0, KeyS => 1, KeyD => 2, KeyF => 3, KeyH => 4, KeyG => 5, KeyZ => 6, KeyX => 7,
        KeyB => 11, KeyQ => 12, KeyW => 13, KeyE => 14, KeyR => 15, KeyY => 16, KeyT => 17,
        Num4 => 21, Num6 => 22, Num5 => 23, Num9 => 25, Num7 => 26, Num8 => 28, Num0 => 29,
        KeyO => 31, KeyU => 32, KeyI => 34, KeyP => 35, Return => 36, KeyL => 37, KeyJ => 38,
        KeyK => 40, KeyN => 45, KeyM => 46, Tab => 48, Backspace => 51,
        F1 => 122, F2 => 120, F3 => 99, F4 => 118, F5 => 96, F6 => 97, F7 => 98, F8 => 100,
        F9 => 101, F10 => 109, F11 => 103, F12 => 111,
        _ => return None,
    })
}

pub fn start(shared: Arc<Shared>, sink: Sink) {
    let m = Arc::new(Mutex::new(Matcher::new(shared, sink)));
    std::thread::Builder::new()
        .name("hotkeys".into())
        .spawn(move || {
            let res = rdev::listen(move |ev| {
                let (k, down) = match ev.event_type {
                    rdev::EventType::KeyPress(k) => (k, true),
                    rdev::EventType::KeyRelease(k) => (k, false),
                    _ => return,
                };
                if let Some(c) = code(k) {
                    m.lock().key(c, down);
                }
            });
            if let Err(e) = res {
                log::error!("macOS key listener failed (grant Accessibility / Input Monitoring): {e:?}");
            }
        })
        .ok();
}
