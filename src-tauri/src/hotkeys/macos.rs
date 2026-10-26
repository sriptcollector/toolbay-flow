//! macOS key listener: a listen-only CGEventTap. Needs Input Monitoring permission. Keys are
//! observed, not swallowed.
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
}

/// The flag that is set while a modifier key is held.
fn modifier_flag(code: u32) -> Option<CGEventFlags> {
    Some(match code {
        55 | 54 => CGEventFlags::CGEventFlagCommand,
        56 | 60 => CGEventFlags::CGEventFlagShift,
        58 | 61 => CGEventFlags::CGEventFlagAlternate,
        59 | 62 => CGEventFlags::CGEventFlagControl,
        63 => CGEventFlags::CGEventFlagSecondaryFn,
        57 => CGEventFlags::CGEventFlagAlphaShift,
        _ => return None,
    })
}

/// Shortcuts store left Shift for either Shift key; every other key is its virtual key code.
fn normalize(code: u32) -> u32 {
    if code == 60 {
        SHIFT
    } else {
        code
    }
}

pub fn start(shared: Arc<Shared>, sink: Sink) {
    let m = Arc::new(Mutex::new(Matcher::new(shared, sink)));
    std::thread::Builder::new()
        .name("hotkeys".into())
        .spawn(move || {
            // Without permission the tap can't be created. Keep trying instead of giving up, so
            // the shortcut starts working as soon as it's switched on, without a restart.
            let mut logged = false;
            loop {
                let m2 = m.clone();
                let tap = CGEventTap::new(
                    CGEventTapLocation::HID,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::ListenOnly,
                    vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged],
                    move |_proxy, ty, ev| {
                        match ty {
                            CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                                let t = TAP.load(Ordering::SeqCst);
                                if !t.is_null() {
                                    unsafe { CGEventTapEnable(t, true) };
                                }
                            }
                            CGEventType::KeyDown | CGEventType::KeyUp => {
                                if ev.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0 {
                                    return None;
                                }
                                let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                                m2.lock().key(normalize(code), matches!(ty, CGEventType::KeyDown));
                            }
                            CGEventType::FlagsChanged => {
                                let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u32;
                                if let Some(flag) = modifier_flag(code) {
                                    let down = ev.get_flags().contains(flag);
                                    m2.lock().key(normalize(code), down);
                                }
                            }
                            _ => {}
                        }
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
