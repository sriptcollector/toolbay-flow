//! Windows low-level keyboard hook (WH_KEYBOARD_LL) on its own message-loop thread.

use super::{Matcher, Shared, Sink};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::Arc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static MATCHER: OnceCell<Mutex<Matcher>> = OnceCell::new();

/// Left modifiers become the generic code; right Ctrl/Alt keep their own so they
/// can be bound alone (see `generic`).
fn normalize(vk: u32) -> u32 {
    match vk {
        0xA0 | 0xA1 => 0x10, // shift (either)
        0xA2 => 0x11,        // left ctrl
        0xA4 => 0x12,        // left alt
        0x5C => 0x5B,        // right win -> win
        v => v,
    }
}

/// Keys Flow swallowed on key-down: Windows never sees them, so GetAsyncKeyState
/// reports them up. Without this set the stale-key check below dropped them,
/// which cut shortcut recording short after the first key.
static SWALLOWED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static LAST_EVENT: Mutex<Option<(u32, u32, u32, usize)>> = Mutex::new(None);

fn physically_down(code: u32) -> bool {
    unsafe {
        if SWALLOWED.lock().contains(&code) {
            // Swallowed keys never reach Windows' key state; trust our own record.
            return true;
        }
        if code == 0x5B {
            return (GetAsyncKeyState(0x5B) as u16 & 0x8000) != 0 || (GetAsyncKeyState(0x5C) as u16 & 0x8000) != 0;
        }
        (GetAsyncKeyState(code as i32) as u16 & 0x8000) != 0
    }
}

/// Sends an unassigned key so releasing Win after a chord does not open Start.
fn mask_win() {
    let mk = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0xE8), wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: super::OWN_INPUT_TAG } },
    };
    let inputs = [mk(KEYBD_EVENT_FLAGS(0)), mk(KEYEVENTF_KEYUP)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        // Ignore only keystrokes Flow itself sends (paste, Start-menu masking);
        // other injected input (AutoHotkey, Stream Deck, remote desktop) still counts.
        let ours = (kb.flags.0 & LLKHF_INJECTED.0) != 0 && kb.dwExtraInfo == super::OWN_INPUT_TAG;
        // Same event delivered to both hooks while one replaces the other.
        {
            let key = (kb.vkCode, kb.flags.0, kb.time, wparam.0);
            let mut last = LAST_EVENT.lock();
            if *last == Some(key) {
                return CallNextHookEx(None, code, wparam, lparam);
            }
            *last = Some(key);
        }
        if std::env::var_os("TOOLBAY_FLOW_DEBUG_KEYS").is_some() {
            log::info!("key vk={:#x} msg={:#x} flags={:#x} extra={:#x} ours={}", kb.vkCode, wparam.0, kb.flags.0, kb.dwExtraInfo, ours);
        }
        if !ours {
            let msg = wparam.0 as u32;
            let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;
            if is_down || is_up {
                let vk = normalize(kb.vkCode);
                if let Some(m) = MATCHER.get() {
                    let mut m = m.lock();
                    if is_down {
                        m.prune_stale(vk, physically_down);
                    }
                    let was_active = m.has_active();
                    let mut swallow = m.key(vk, is_down);
                    {
                        let mut sw = SWALLOWED.lock();
                        if is_down && swallow && !sw.contains(&vk) {
                            sw.push(vk);
                        } else if !is_down {
                            if let Some(i) = sw.iter().position(|k| *k == vk) {
                                // Swallow the matching key-up too, so apps never see half a keystroke.
                                sw.remove(i);
                                swallow = true;
                            }
                        }
                    }
                    // A Flow shortcut that includes Alt or Win just fired. Tap a harmless
                    // key now, while they're still held, so releasing them later doesn't
                    // open the Start menu or put the app into menu mode (which would
                    // swallow the paste).
                    if is_down && !was_active && m.has_active() && m.down.iter().any(|k| matches!(*k, 0x12 | 0xA5 | 0x5B)) {
                        drop(m);
                        mask_win();
                    }
                    if swallow {
                        return LRESULT(1);
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// Keys typed while one of Flow's own windows is focused. Windows does not pass
/// those to Flow's own low-level hook, so the page forwards them here instead.
pub fn feed(vk: u32, down: bool) {
    if let Some(m) = MATCHER.get() {
        let vk = normalize(vk);
        let mut m = m.lock();
        let was_active = m.has_active();
        m.key(vk, down);
        if down && !was_active && m.has_active() && m.down.iter().any(|k| matches!(*k, 0x12 | 0xA5 | 0x5B)) {
            drop(m);
            mask_win();
        }
    }
}

/// Windows silently drops a low-level hook whose callback is ever slow (for
/// example while the computer is busy running the speech model). Re-installing it
/// regularly keeps hotkeys alive; events seen by both hooks during the swap are
/// recognised by their timestamp and handled once.
const REINSTALL_MS: u32 = 30_000;

pub fn start(shared: Arc<Shared>, sink: Sink) {
    let _ = MATCHER.set(Mutex::new(Matcher::new(shared, sink)));
    std::thread::Builder::new()
        .name("hotkeys".into())
        .spawn(|| unsafe {
            let install = || SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0);
            let mut h = match install() {
                Ok(h) => {
                    log::info!("keyboard hook installed");
                    h
                }
                Err(e) => {
                    log::error!("keyboard hook failed: {e}");
                    return;
                }
            };
            let timer = SetTimer(None, 0, REINSTALL_MS, None);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_TIMER && msg.wParam.0 == timer {
                    if let Ok(new) = install() {
                        let _ = UnhookWindowsHookEx(h);
                        h = new;
                    }
                    continue;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(h);
        })
        .ok();
}
