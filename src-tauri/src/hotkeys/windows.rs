//! Windows low-level keyboard hook (WH_KEYBOARD_LL) on its own message-loop thread.

use super::{Matcher, Shared, Sink};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::sync::Arc;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static MATCHER: OnceCell<Mutex<Matcher>> = OnceCell::new();
static WIN_CHORD: Mutex<bool> = Mutex::new(false);

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
                    if is_down && m.down.contains(&0x5B) && m.has_active() {
                        // A chord involving Win is being used by Flow.
                        *WIN_CHORD.lock() = true;
                    }
                    if is_up && vk == 0x5B {
                        let mut wc = WIN_CHORD.lock();
                        if *wc {
                            *wc = false;
                            drop(m);
                            mask_win();
                        }
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

pub fn start(shared: Arc<Shared>, sink: Sink) {
    let _ = MATCHER.set(Mutex::new(Matcher::new(shared, sink)));
    std::thread::Builder::new()
        .name("hotkeys".into())
        .spawn(|| unsafe {
            let h = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0) {
                Ok(h) => h,
                Err(e) => {
                    log::error!("keyboard hook failed: {e}");
                    return;
                }
            };
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWindowsHookEx(h);
        })
        .ok();
}
