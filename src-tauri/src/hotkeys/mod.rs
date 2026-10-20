//! Global hotkeys. A platform hook feeds raw key up/down events into `Matcher`,
//! which turns chords (including modifier-only chords like Ctrl+Win or Fn) into
//! Pressed/Released events for actions.

use crate::settings::{Action, Shortcut};
use parking_lot::RwLock;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Pressed(Action),
    Released(Action),
    /// Raw keys while the settings UI records a new shortcut.
    Recording { keys: Vec<u32>, done: bool },
}

pub struct Shared {
    pub shortcuts: RwLock<Vec<Shortcut>>,
    /// Esc is only swallowed while a dictation is in progress.
    pub dismiss_armed: AtomicBool,
    /// When true, all key events go to the shortcut recorder instead.
    pub recording: AtomicBool,
    pub enabled: AtomicBool,
}

/// dwExtraInfo marker on keystrokes Flow synthesizes, so the hook can skip them.
#[allow(dead_code)]
pub const OWN_INPUT_TAG: usize = 0x5442_464C;

pub type Sink = Arc<dyn Fn(HotkeyEvent) + Send + Sync>;

pub struct Matcher {
    pub down: BTreeSet<u32>,
    active: Vec<Action>,
    rec_max: BTreeSet<u32>,
    shared: Arc<Shared>,
    sink: Sink,
}

impl Matcher {
    pub fn new(shared: Arc<Shared>, sink: Sink) -> Self {
        Matcher { down: BTreeSet::new(), active: vec![], rec_max: BTreeSet::new(), shared, sink }
    }

    /// Returns true if the key event should be swallowed (not passed to the app).
    pub fn key(&mut self, code: u32, is_down: bool) -> bool {
        if self.shared.recording.load(Ordering::Relaxed) {
            if is_down {
                self.down.insert(code);
                self.rec_max = self.rec_max.union(&self.down).cloned().collect();
                (self.sink)(HotkeyEvent::Recording { keys: self.rec_max.iter().cloned().collect(), done: false });
            } else {
                self.down.remove(&code);
                if self.down.is_empty() {
                    let keys: Vec<u32> = std::mem::take(&mut self.rec_max).into_iter().collect();
                    (self.sink)(HotkeyEvent::Recording { keys, done: true });
                }
            }
            return true;
        }
        if !self.shared.enabled.load(Ordering::Relaxed) {
            self.down.clear();
            return false;
        }
        let was_down = self.down.contains(&code);
        if is_down {
            if was_down {
                // Auto-repeat: swallow if it belongs to an active non-modifier chord.
                return self.active.iter().any(|a| self.completes_with(a, code));
            }
            self.down.insert(code);
        } else {
            self.down.remove(&code);
        }

        // Release actions whose keys are no longer all held.
        let shortcuts = self.shared.shortcuts.read().clone();
        let mut released = vec![];
        self.active.retain(|a| {
            let keys = shortcuts.iter().find(|s| &s.action == a).map(|s| s.keys.clone()).unwrap_or_default();
            let held = keys.iter().all(|k| self.down.contains(k));
            if !held {
                released.push(a.clone());
            }
            held
        });
        for a in released {
            (self.sink)(HotkeyEvent::Released(a));
        }

        if is_down {
            let set: Vec<u32> = self.down.iter().cloned().collect();
            if let Some(s) = shortcuts.iter().find(|s| s.keys == set) {
                if s.action == Action::Dismiss && !self.shared.dismiss_armed.load(Ordering::Relaxed) {
                    return false;
                }
                if !self.active.contains(&s.action) {
                    self.active.push(s.action.clone());
                    (self.sink)(HotkeyEvent::Pressed(s.action.clone()));
                }
                // Swallow the non-modifier key that completed a chord (Space, Z, 1...).
                return !is_modifier(code);
            }
        }
        false
    }

    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    fn completes_with(&self, a: &Action, code: u32) -> bool {
        !is_modifier(code)
            && self.shared.shortcuts.read().iter().any(|s| &s.action == a && s.keys.contains(&code))
    }
}

#[cfg(windows)]
pub fn is_modifier(code: u32) -> bool {
    matches!(code, 0x10 | 0x11 | 0x12 | 0x5B)
}

#[cfg(not(windows))]
pub fn is_modifier(code: u32) -> bool {
    use mac_codes::*;
    matches!(code, FN | CTRL | ALT | CMD | SHIFT)
}

/// macOS virtual key codes used in default shortcuts.
#[allow(dead_code)]
pub mod mac_codes {
    pub const FN: u32 = 63;
    pub const CTRL: u32 = 59;
    pub const ALT: u32 = 58;
    pub const CMD: u32 = 55;
    pub const SHIFT: u32 = 56;
    pub const SPACE: u32 = 49;
    pub const ESC: u32 = 53;
    pub const KEY_V: u32 = 9;
    pub const KEY_C: u32 = 8;
    pub const KEY_1: u32 = 18;
    pub const KEY_2: u32 = 19;
    pub const KEY_3: u32 = 20;
}

pub fn start(shared: Arc<Shared>, sink: Sink) {
    #[cfg(windows)]
    windows::start(shared, sink);
    #[cfg(target_os = "macos")]
    macos::start(shared, sink);
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (shared, sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;

    fn setup() -> (Matcher, Arc<Mutex<Vec<String>>>) {
        let shared = Arc::new(Shared {
            shortcuts: RwLock::new(crate::settings::default_shortcuts()),
            dismiss_armed: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            enabled: AtomicBool::new(true),
        });
        let log = Arc::new(Mutex::new(vec![]));
        let l2 = log.clone();
        let m = Matcher::new(shared, Arc::new(move |e| l2.lock().push(format!("{e:?}"))));
        (m, log)
    }

    #[cfg(windows)]
    #[test]
    fn ptt_and_handsfree() {
        let (mut m, log) = setup();
        m.key(0x11, true);
        m.key(0x5B, true);
        assert!(m.key(0x20, true)); // space swallowed
        m.key(0x20, false);
        m.key(0x5B, false);
        m.key(0x11, false);
        let l = log.lock().join(",");
        assert_eq!(l, "Pressed(PushToTalk),Pressed(HandsFree),Released(HandsFree),Released(PushToTalk)");
    }

    #[cfg(windows)]
    #[test]
    fn esc_only_when_armed() {
        let (mut m, log) = setup();
        assert!(!m.key(0x1B, true));
        m.key(0x1B, false);
        assert!(log.lock().is_empty());
        m.shared.dismiss_armed.store(true, Ordering::Relaxed);
        m.key(0x1B, true);
        assert_eq!(log.lock()[0], "Pressed(Dismiss)");
    }
}
