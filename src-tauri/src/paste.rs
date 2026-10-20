//! Inserting text into the focused app: put it on the clipboard, send Ctrl/Cmd+V,
//! then restore whatever was on the clipboard before.

use anyhow::Result;
use std::time::Duration;

pub fn paste(text: &str, restore: bool) -> Result<()> {
    platform::paste(text, restore)
}

pub fn copy(text: &str) -> Result<()> {
    platform::set_text(text)
}

/// Reads the current selection by sending Ctrl/Cmd+C (used when UI Automation
/// cannot see the selection). The clipboard is restored afterwards.
pub fn copy_selection() -> Option<String> {
    platform::copy_selection()
}

#[cfg(windows)]
mod platform {
    use super::*;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    const CF_UNICODETEXT: u32 = 13;
    // GDI handle formats that cannot be copied as bytes (CF_DIB/CF_DIBV5 cover images).
    const SKIP: &[u32] = &[2, 3, 9, 14, 0x0080, 0x0082, 0x0083, 0x008E];

    struct Clip;
    impl Clip {
        fn open() -> Option<Clip> {
            for _ in 0..20 {
                if unsafe { OpenClipboard(None) }.is_ok() {
                    return Some(Clip);
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            None
        }
    }
    impl Drop for Clip {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    fn snapshot() -> Vec<(u32, Vec<u8>)> {
        let mut out = vec![];
        let Some(_c) = Clip::open() else { return out };
        unsafe {
            let mut f = EnumClipboardFormats(0);
            while f != 0 {
                if !SKIP.contains(&f) {
                    if let Ok(h) = GetClipboardData(f) {
                        let g = HGLOBAL(h.0);
                        let size = GlobalSize(g);
                        let p = GlobalLock(g) as *const u8;
                        if !p.is_null() && size > 0 && size < 64 * 1024 * 1024 {
                            out.push((f, std::slice::from_raw_parts(p, size).to_vec()));
                        }
                        if !p.is_null() {
                            let _ = GlobalUnlock(g);
                        }
                    }
                }
                f = EnumClipboardFormats(f);
            }
        }
        out
    }

    fn put(format: u32, bytes: &[u8]) {
        unsafe {
            if let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) {
                let p = GlobalLock(g) as *mut u8;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
                    let _ = GlobalUnlock(g);
                    let _ = SetClipboardData(format, HANDLE(g.0));
                }
            }
        }
    }

    fn register(name: PCWSTR) -> u32 {
        unsafe { RegisterClipboardFormatW(name) }
    }

    fn write_text(text: &str, private: bool) -> Result<()> {
        let _c = Clip::open().ok_or_else(|| anyhow::anyhow!("clipboard busy"))?;
        unsafe {
            EmptyClipboard()?;
        }
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bytes = unsafe { std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2) };
        put(CF_UNICODETEXT, bytes);
        if private {
            // Keep dictation out of Windows clipboard history and cloud clipboard.
            put(register(w!("ExcludeClipboardContentFromMonitorProcessing")), &[0]);
            put(register(w!("CanIncludeInClipboardHistory")), &0u32.to_le_bytes());
            put(register(w!("CanUploadToCloudClipboard")), &0u32.to_le_bytes());
        }
        Ok(())
    }

    fn restore(snap: Vec<(u32, Vec<u8>)>) {
        let Some(_c) = Clip::open() else { return };
        unsafe {
            let _ = EmptyClipboard();
        }
        for (f, b) in snap {
            put(f, &b);
        }
    }

    pub fn set_text(text: &str) -> Result<()> {
        write_text(text, false)
    }

    fn key(vk: u16, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, time: 0, dwExtraInfo: 0 },
            },
        }
    }

    fn down(vk: i32) -> bool {
        unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
    }

    /// Sends Ctrl+<vk>, releasing modifiers the user may still be holding.
    fn ctrl_combo(vk: u16) {
        let mut inputs = vec![];
        for m in [0x5Bu16, 0x5C, 0x12, 0x10] {
            if down(m as i32) {
                inputs.push(key(m, true));
            }
        }
        // Releasing Alt alone can focus a menu bar; tap Ctrl first to cancel that.
        inputs.push(key(0x11, false));
        inputs.push(key(vk, false));
        inputs.push(key(vk, true));
        inputs.push(key(0x11, true));
        unsafe {
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        }
    }

    pub fn paste(text: &str, restore_after: bool) -> Result<()> {
        let snap = if restore_after { snapshot() } else { vec![] };
        write_text(text, true)?;
        let seq = unsafe { GetClipboardSequenceNumber() };
        std::thread::sleep(Duration::from_millis(30));
        ctrl_combo(0x56); // V
        if restore_after {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(700));
                // Only restore if nobody else changed the clipboard meanwhile.
                if unsafe { GetClipboardSequenceNumber() } == seq {
                    restore(snap);
                }
            });
        }
        Ok(())
    }

    pub fn copy_selection() -> Option<String> {
        let snap = snapshot();
        let before = unsafe { GetClipboardSequenceNumber() };
        ctrl_combo(0x43); // C
        let mut got = None;
        for _ in 0..25 {
            std::thread::sleep(Duration::from_millis(20));
            if unsafe { GetClipboardSequenceNumber() } != before {
                std::thread::sleep(Duration::from_millis(30));
                got = read_text();
                break;
            }
        }
        restore(snap);
        got.filter(|s| !s.is_empty())
    }

    fn read_text() -> Option<String> {
        let _c = Clip::open()?;
        unsafe {
            let h = GetClipboardData(CF_UNICODETEXT).ok()?;
            let g = HGLOBAL(h.0);
            let p = GlobalLock(g) as *const u16;
            if p.is_null() {
                return None;
            }
            let mut len = 0;
            while *p.add(len) != 0 {
                len += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, len));
            let _ = GlobalUnlock(g);
            Some(s)
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    fn cmd_key(keycode: u16) {
        let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else { return };
        if let (Ok(d), Ok(u)) = (CGEvent::new_keyboard_event(src.clone(), keycode, true), CGEvent::new_keyboard_event(src, keycode, false)) {
            d.set_flags(CGEventFlags::CGEventFlagCommand);
            u.set_flags(CGEventFlags::CGEventFlagCommand);
            d.post(CGEventTapLocation::HID);
            u.post(CGEventTapLocation::HID);
        }
    }

    pub fn set_text(text: &str) -> Result<()> {
        arboard::Clipboard::new()?.set_text(text.to_string())?;
        Ok(())
    }

    pub fn paste(text: &str, restore_after: bool) -> Result<()> {
        let mut cb = arboard::Clipboard::new()?;
        let prev = if restore_after { cb.get_text().ok() } else { None };
        cb.set_text(text.to_string())?;
        std::thread::sleep(Duration::from_millis(30));
        cmd_key(9); // V
        if let Some(prev) = prev {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(700));
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(prev);
                }
            });
        }
        Ok(())
    }

    pub fn copy_selection() -> Option<String> {
        let mut cb = arboard::Clipboard::new().ok()?;
        let prev = cb.get_text().ok();
        let _ = cb.set_text(String::new());
        cmd_key(8); // C
        std::thread::sleep(Duration::from_millis(150));
        let got = cb.get_text().ok().filter(|s| !s.is_empty());
        if let Some(p) = prev {
            let _ = cb.set_text(p);
        }
        got
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::*;
    pub fn set_text(_: &str) -> Result<()> {
        Ok(())
    }
    pub fn paste(_: &str, _: bool) -> Result<()> {
        Ok(())
    }
    pub fn copy_selection() -> Option<String> {
        None
    }
}
