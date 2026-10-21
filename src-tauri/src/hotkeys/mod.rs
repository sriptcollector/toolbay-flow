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
    /// The keys held are the start of a dictation shortcut: get the mic ready.
    Prefix,
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
    rec_was: bool,
    shared: Arc<Shared>,
    sink: Sink,
}

impl Matcher {
    pub fn new(shared: Arc<Shared>, sink: Sink) -> Self {
        Matcher { down: BTreeSet::new(), active: vec![], rec_max: BTreeSet::new(), rec_was: false, shared, sink }
    }

    /// Returns true if the key event should be swallowed (not passed to the app).
    pub fn key(&mut self, code: u32, is_down: bool) -> bool {
        let rec = self.shared.recording.load(Ordering::Relaxed);
        if rec != self.rec_was {
            // Entering or leaving recording: start from a clean slate.
            self.rec_was = rec;
            self.rec_max.clear();
            self.down.clear();
            self.active.clear();
            if rec && !is_down {
                return true;
            }
        }
        if rec {
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
            let held = keys.iter().all(|k| holds(&self.down, *k));
            if !held {
                released.push(a.clone());
            }
            held
        });
        for a in released {
            (self.sink)(HotkeyEvent::Released(a));
        }

        if is_down && self.active.is_empty() {
            let dictation = [Action::PushToTalk, Action::HandsFree, Action::Command];
            let prefix = shortcuts.iter().any(|s| {
                dictation.contains(&s.action) && s.keys.len() > self.down.len() && self.down.iter().all(|d| s.keys.contains(d) || s.keys.contains(&generic(*d)))
            });
            if prefix {
                (self.sink)(HotkeyEvent::Prefix);
            }
        }
        if is_down {
            if let Some(s) = shortcuts.iter().find(|s| combo_matches(&s.keys, &self.down)) {
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

    pub fn is_recording(&self) -> bool {
        self.shared.recording.load(Ordering::Relaxed)
    }

    /// Drops keys whose release we never saw (lock screen, UAC prompt...).
    /// Never while recording a shortcut: keys swallowed for the recorder look
    /// "up" to the OS, and pruning them cut recordings after the first key.
    pub fn prune_stale(&mut self, current: u32, physically_down: impl Fn(u32) -> bool) {
        if self.is_recording() {
            return;
        }
        let stale: Vec<u32> = self.down.iter().cloned().filter(|k| *k != current && !physically_down(*k)).collect();
        for k in stale {
            self.key(k, false);
        }
    }

    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    fn completes_with(&self, a: &Action, code: u32) -> bool {
        !is_modifier(code)
            && self.shared.shortcuts.read().iter().any(|s| &s.action == a && s.keys.contains(&code))
    }
}

/// Right-hand modifiers keep their own code so "Right Alt" alone can be a shortcut;
/// a shortcut saved with the generic modifier accepts either side.
#[cfg(windows)]
pub fn generic(code: u32) -> u32 {
    match code {
        0xA3 => 0x11, // right Ctrl
        0xA5 => 0x12, // right Alt
        c => c,
    }
}

#[cfg(not(windows))]
pub fn generic(code: u32) -> u32 {
    match code {
        62 => 59, // right Control
        61 => 58, // right Option
        54 => 55, // right Command
        c => c,
    }
}

/// Is key `k` of a shortcut satisfied by the keys held down?
pub fn holds(down: &BTreeSet<u32>, k: u32) -> bool {
    down.contains(&k) || down.iter().any(|d| generic(*d) == k && *d != k)
}

/// Exact match: the held keys are this shortcut, nothing more, nothing less.
pub fn combo_matches(keys: &[u32], down: &BTreeSet<u32>) -> bool {
    if keys.is_empty() || keys.len() != down.len() {
        return false;
    }
    let raw: Vec<u32> = down.iter().cloned().collect();
    if keys == raw.as_slice() {
        return true;
    }
    let mut g: Vec<u32> = down.iter().map(|d| generic(*d)).collect();
    g.sort();
    g.dedup();
    keys == g.as_slice()
}

#[cfg(windows)]
pub fn is_modifier(code: u32) -> bool {
    matches!(code, 0x10 | 0x11 | 0x12 | 0x5B | 0xA3 | 0xA5)
}

#[cfg(not(windows))]
pub fn is_modifier(code: u32) -> bool {
    use mac_codes::*;
    matches!(code, FN | CTRL | ALT | CMD | SHIFT | 62 | 61 | 54)
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

/// Key events from Flow's own windows (see `windows::feed`).
pub fn feed(vk: u32, down: bool) {
    #[cfg(windows)]
    windows::feed(vk, down);
    #[cfg(not(windows))]
    let _ = (vk, down);
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
        let m = Matcher::new(
            shared,
            Arc::new(move |e| {
                if !matches!(e, HotkeyEvent::Prefix) {
                    l2.lock().push(format!("{e:?}"))
                }
            }),
        );
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
    fn right_alt_alone_and_either_side() {
        let (mut m, log) = setup();
        m.shared.shortcuts.write()[0].keys = vec![0xA5];
        m.key(0xA5, true);
        m.key(0xA5, false);
        // Default Ctrl+Win still works with the right Ctrl.
        m.shared.shortcuts.write()[0].keys = vec![0x11, 0x5B];
        m.key(0xA3, true);
        m.key(0x5B, true);
        m.key(0x5B, false);
        m.key(0xA3, false);
        assert_eq!(log.lock().join(","), "Pressed(PushToTalk),Released(PushToTalk),Pressed(PushToTalk),Released(PushToTalk)");
    }

    #[cfg(windows)]
    #[test]
    fn recording_collects_full_combo() {
        let (mut m, log) = setup();
        m.shared.recording.store(true, Ordering::Relaxed);
        m.key(0x11, true);
        m.key(0x5B, true);
        m.key(0x20, true);
        m.key(0x20, false);
        m.key(0x5B, false);
        m.key(0x11, false);
        let l = log.lock();
        assert_eq!(l.last().unwrap(), "Recording { keys: [17, 32, 91], done: true }");
    }

    /// Regression: recording Ctrl+Win while the OS reports both keys as up
    /// (because the recorder swallows them) must still capture the whole combo.
    #[cfg(windows)]
    #[test]
    fn recording_survives_stale_check() {
        let (mut m, log) = setup();
        m.shared.recording.store(true, Ordering::Relaxed);
        for (k, d) in [(0x11, true), (0x5B, true), (0x5B, false), (0x11, false)] {
            if d {
                m.prune_stale(k, |_| false);
            }
            m.key(k, d);
        }
        assert_eq!(log.lock().last().unwrap(), "Recording { keys: [17, 91], done: true }");
        // And the saved shortcut then works.
        m.shared.recording.store(false, Ordering::Relaxed);
        m.shared.shortcuts.write()[0].keys = vec![0x11, 0x5B];
        log.lock().clear();
        m.key(0x11, true);
        m.key(0x5B, true);
        m.key(0x5B, false);
        assert_eq!(log.lock().join(","), "Pressed(PushToTalk),Released(PushToTalk)");
    }

    #[cfg(windows)]
    #[test]
    fn prefix_warms_mic() {
        let shared = Arc::new(Shared {
            shortcuts: RwLock::new(crate::settings::default_shortcuts()),
            dismiss_armed: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            enabled: AtomicBool::new(true),
        });
        let log = Arc::new(Mutex::new(vec![]));
        let l2 = log.clone();
        let mut m = Matcher::new(shared, Arc::new(move |e| l2.lock().push(format!("{e:?}"))));
        m.key(0x11, true); // Ctrl: start of Ctrl+Win
        assert_eq!(log.lock()[0], "Prefix");
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
