//! macOS key listener: a LISTEN-ONLY CGEventTap (Input Monitoring permission, never
//! Accessibility). It only gets copies of key events after the system has delivered them, so it
//! can't hold, change or delay a key press: if Flow stalls, the Mac's keyboard doesn't.
//! (Flow used to install an active tap in front of every key, which needs Accessibility; a slow
//! callback there froze typing on the whole Mac.) The callback does nothing but queue the key;
//! the matching runs on another thread.
//!
//! Because the tap can't hold Fn back, macOS still acts on Fn too; Settings explains "Press 🌐
//! key to: Do Nothing" (see macperm::fn_key_usage).
//!
//! Flow's own tap rather than rdev: rdev looks up the keyboard layout (TISCopyCurrentKeyboard
//! InputSource) on its listener thread for every key, which recent macOS aborts the app for
//! off the main thread. We only need the virtual key code and modifier flags.

use super::mac_codes::*;
use super::{Matcher, Shared, Sink};
use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, EventField,
};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

/// True while the tap is installed, i.e. the shortcut can be heard.
pub static LISTENING: AtomicBool = AtomicBool::new(false);
static TAP: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

extern "C" {
    fn CGEventTapEnable(tap: *mut std::ffi::c_void, enable: bool);
    fn CGEventTapIsEnabled(tap: *mut std::ffi::c_void) -> bool;
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
    fn CGEventSourceFlagsState(state: i32) -> u64;
}

/// kCGEventSourceStateHIDSystemState: what the keyboard hardware says right now.
const HID_STATE: i32 = 1;

/// Set when macOS switched the tap off and we switched it back on: only then can a key release
/// have been missed, so only then is the stale-key check run.
static TAP_RECOVERED: AtomicBool = AtomicBool::new(false);

/// Keys Flow believes are held (raw macOS codes), for the stale-key check.
static HELD: Mutex<Vec<u32>> = parking_lot::const_mutex(Vec::new());

fn really_down(code: u32) -> bool {
    unsafe {
        match modifier_flag(code) {
            Some(flag) => CGEventSourceFlagsState(HID_STATE) & flag.bits() != 0 || CGEventSourceKeyState(HID_STATE, code as u16),
            None => CGEventSourceKeyState(HID_STATE, code as u16),
        }
    }
}

fn note(code: u32, down: bool) {
    let mut h = HELD.lock();
    h.retain(|&c| c != code);
    if down {
        h.push(code);
    }
}

/// Wispr Flow's two safety nets, every 250 ms:
/// - tap watchdog: macOS silently switches a tap off (timeouts, Secure Input); switch it back on.
/// - stale keys: a release that never arrived while the tap was off leaves Fn "held" and the next
///   press is ignored. Right after a recovery, ask the keyboard what's really down and release the
///   rest. Never otherwise: Fn presses we hold back from macOS don't show in its key state, so a
///   routine check would "release" Fn mid-dictation.
fn start_watchdog(m: Arc<Mutex<Matcher>>) {
    std::thread::Builder::new()
        .name("hotkeys-watchdog".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let t = TAP.load(Ordering::SeqCst);
            if !t.is_null() && unsafe { !CGEventTapIsEnabled(t) } {
                unsafe { CGEventTapEnable(t, true) };
                TAP_RECOVERED.store(true, Ordering::SeqCst);
                log::warn!("macOS key listener was switched off; re-enabled");
                continue; // let the keyboard state settle before checking keys
            }
            if !TAP_RECOVERED.swap(false, Ordering::SeqCst) {
                continue;
            }
            let stale: Vec<u32> = HELD.lock().iter().copied().filter(|&c| !really_down(c)).collect();
            for code in stale {
                note(code, false);
                m.lock().key(normalize(code), false);
            }
        })
        .ok();
}

/// The flag that is set while a modifier key is held.
fn modifier_flag(code: u32) -> Option<CGEventFlags> {
    Some(match code {
        55 | 54 => CGEventFlags::CGEventFlagCommand,
        56 | 60 => CGEventFlags::CGEventFlagShift,
        58 | 61 => CGEventFlags::CGEventFlagAlternate,
        59 | 62 => CGEventFlags::CGEventFlagControl,
        63 | 179 => CGEventFlags::CGEventFlagSecondaryFn,
        57 => CGEventFlags::CGEventFlagAlphaShift,
        _ => return None,
    })
}

/// Shortcuts store left Shift for either Shift key, and Fn for the globe key newer Apple
/// keyboards report as 179; every other key is its virtual key code.
fn normalize(code: u32) -> u32 {
    match code {
        60 => SHIFT,
        179 => FN,
        c => c,
    }
}

pub fn start(shared: Arc<Shared>, sink: Sink) {
    let m = Arc::new(Mutex::new(Matcher::new(shared, sink)));
    start_watchdog(m.clone());

    // Keys go through a channel: the tap's callback never waits on a lock or on Flow's work.
    let (tx, rx) = std::sync::mpsc::channel::<(u32, bool)>();
    let worker = m.clone();
    std::thread::Builder::new()
        .name("hotkeys-match".into())
        .spawn(move || {
            for (code, down) in rx {
                note(code, down);
                worker.lock().key(normalize(code), down);
            }
        })
        .ok();

    std::thread::Builder::new()
        .name("hotkeys".into())
        .spawn(move || {
            // Without Input Monitoring the tap can't be created. Keep trying instead of giving up,
            // so the shortcut starts working as soon as it's switched on, without a restart.
            let mut logged = false;
            loop {
                let tx = tx.clone();
                let tap = CGEventTap::new(
                    CGEventTapLocation::Session,
                    CGEventTapPlacement::TailAppendEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged],
                    move |_proxy, ty, ev| {
                        match ty {
                            CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                                let t = TAP.load(Ordering::SeqCst);
                                if !t.is_null() {
                                    unsafe { CGEventTapEnable(t, true) };
                                    TAP_RECOVERED.store(true, Ordering::SeqCst);
                                }
                            }
                            CGEventType::KeyDown | CGEventType::KeyUp => {
                                if ev.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) == 0 {
                                    let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                                    let _ = tx.send((code, matches!(ty, CGEventType::KeyDown)));
                                }
                            }
                            CGEventType::FlagsChanged => {
                                let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                                if let Some(flag) = modifier_flag(code) {
                                    let _ = tx.send((code, ev.get_flags().contains(flag)));
                                }
                            }
                            _ => {}
                        }
                        // A listen-only tap can't change anything; the event has already gone on.
                        None
                    },
                );
                match tap {
                    Ok(tap) => unsafe {
                        let Ok(source) = tap.mach_port.create_runloop_source(0) else {
                            log::error!("macOS key listener: couldn't create a run loop source");
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            continue;
                        };
                        CFRunLoop::get_current().add_source(&source, kCFRunLoopCommonModes);
                        TAP.store(tap.mach_port.as_concrete_TypeRef() as *mut _, Ordering::SeqCst);
                        tap.enable();
                        LISTENING.store(true, Ordering::SeqCst);
                        log::info!("macOS key listener on");
                        CFRunLoop::run_current();
                        LISTENING.store(false, Ordering::SeqCst);
                        TAP.store(std::ptr::null_mut(), Ordering::SeqCst);
                        log::warn!("macOS key listener stopped; restarting");
                    },
                    Err(()) => {
                        if !logged {
                            log::warn!("macOS key listener waiting for Input Monitoring permission");
                            logged = true;
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        })
        .ok();
}
