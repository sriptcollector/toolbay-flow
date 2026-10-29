//! Reads what the user is typing into: focused app, window title, and the text
//! around the cursor (UI Automation on Windows, Accessibility API on macOS).
//! Nothing is read from password fields.

use serde::Serialize;
use std::time::Duration;

#[derive(Clone, Debug, Default, Serialize)]
pub struct FocusContext {
    pub app: String,
    pub exe: String,
    pub window_title: String,
    pub url: String,
    pub before: String,
    pub after: String,
    pub selected: String,
    pub is_password: bool,
    pub has_text_field: bool,
    pub pid: u32,
}

/// Captures context with a hard timeout so a hung app never blocks dictation.
pub fn capture(read_text: bool) -> FocusContext {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(platform::capture(read_text));
    });
    let mut c = rx.recv_timeout(Duration::from_millis(700)).unwrap_or_else(|_| platform::window_only());
    c.before = limit_before(&c.before);
    c.after = limit_after(&c.after);
    c
}

/// Text kept on each side of the cursor (same as the original app).
pub const AROUND_CURSOR: usize = 500;

/// The last AROUND_CURSOR characters before the cursor, starting on a whole word.
pub fn limit_before(s: &str) -> String {
    let rev: String = s.chars().rev().collect();
    limit_after(&rev).chars().rev().collect::<String>().trim_start().to_string()
}

/// The first AROUND_CURSOR characters after the cursor, ending on a whole word.
pub fn limit_after(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= AROUND_CURSOR {
        return s.trim_end().to_string();
    }
    // Look one character further so a word that ends exactly at the limit stays.
    let head = &chars[..=AROUND_CURSOR];
    match head.iter().rposition(|c| c.is_whitespace()) {
        Some(i) => head[..i].iter().collect::<String>().trim_end().to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn around_cursor_limits() {
        let long = "word ".repeat(200); // 1000 chars
        let a = limit_after(&long);
        assert!(a.chars().count() <= AROUND_CURSOR && a.ends_with("word"));
        let b = limit_before(&format!("{long}end"));
        assert!(b.chars().count() <= AROUND_CURSOR && b.starts_with("word") && b.ends_with("end"));
        assert_eq!(limit_after("short text  "), "short text");
        assert_eq!(limit_after(&"x".repeat(600)), "");
    }
}

/// Full text of the focused field (for learning from the user's edits).
pub fn focused_text() -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(platform::focused_text());
    });
    rx.recv_timeout(Duration::from_millis(700)).ok().flatten()
}

fn friendly(exe: &str) -> String {
    let stem = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    let stem = stem.strip_suffix(".exe").or_else(|| stem.strip_suffix(".EXE")).unwrap_or(stem);
    let known = [
        ("chrome", "Google Chrome"), ("msedge", "Microsoft Edge"), ("firefox", "Firefox"), ("brave", "Brave"),
        ("notepad", "Notepad"), ("winword", "Word"), ("outlook", "Outlook"), ("olk", "Outlook"), ("slack", "Slack"),
        ("discord", "Discord"), ("code", "VS Code"), ("cursor", "Cursor"), ("windowsterminal", "Terminal"),
        ("ms-teams", "Teams"), ("teams", "Teams"), ("whatsapp", "WhatsApp"), ("telegram", "Telegram"),
        ("notion", "Notion"), ("obsidian", "Obsidian"), ("excel", "Excel"), ("powerpnt", "PowerPoint"),
        ("claude", "Claude"), ("chatgpt", "ChatGPT"), ("arc", "Arc"), ("opera", "Opera"),
    ];
    let l = stem.to_lowercase();
    known.iter().find(|(k, _)| *k == l).map(|(_, v)| v.to_string()).unwrap_or_else(|| {
        let mut c = stem.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    })
}

#[cfg(windows)]
mod platform {
    use super::{friendly, FocusContext};
    use uiautomation::patterns::{UITextPattern, UIValuePattern};
    use uiautomation::types::{TextPatternRangeEndpoint as Ep, TextUnit};
    use uiautomation::UIAutomation;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};

    pub fn window_only() -> FocusContext {
        let mut c = FocusContext::default();
        unsafe {
            let hwnd = GetForegroundWindow();
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(hwnd, &mut buf);
            c.window_title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            c.pid = pid;
            if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                let mut path = [0u16; 1024];
                let mut len = path.len() as u32;
                if QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut len).is_ok() {
                    let full = String::from_utf16_lossy(&path[..len as usize]);
                    c.exe = full.rsplit('\\').next().unwrap_or(&full).to_string();
                }
                let _ = CloseHandle(h);
            }
        }
        c.app = friendly(&c.exe);
        c
    }

    pub fn capture(read_text: bool) -> FocusContext {
        let mut c = window_only();
        let Ok(auto) = UIAutomation::new() else { return c };
        // Flow's own windows (the hub and the bar): no context needed.
        let read_text = read_text && c.pid != std::process::id();
        let Ok(el) = auto.get_focused_element() else { return c };
        c.is_password = el.is_password().unwrap_or(false);
        if c.is_password || !read_text {
            c.has_text_field = c.is_password;
            return c;
        }
        if let Ok(tp) = el.get_pattern::<UITextPattern>() {
            c.has_text_field = true;
            let sel = tp.get_selection().ok().and_then(|v| v.into_iter().next());
            if let Some(sel) = sel {
                c.selected = sel.get_text(8000).unwrap_or_default();
                // Fresh range objects for before/after (selection ranges are new each call).
                if let Some(b) = tp.get_selection().ok().and_then(|v| v.into_iter().next()) {
                    let _ = b.move_endpoint_by_unit(Ep::Start, TextUnit::Character, -600);
                    let _ = b.move_endpoint_by_range(Ep::End, &sel, Ep::Start);
                    c.before = b.get_text(1200).unwrap_or_default();
                }
                if let Some(a) = tp.get_selection().ok().and_then(|v| v.into_iter().next()) {
                    let _ = a.move_endpoint_by_range(Ep::Start, &sel, Ep::End);
                    let _ = a.move_endpoint_by_unit(Ep::End, TextUnit::Character, 600);
                    c.after = a.get_text(1200).unwrap_or_default();
                }
                return c;
            }
        }
        // Only real edit boxes: documents (web pages) expose their URL as "value".
        let is_edit = matches!(el.get_control_type(), Ok(uiautomation::controls::ControlType::Edit) | Ok(uiautomation::controls::ControlType::ComboBox));
        if !is_edit {
            return c;
        }
        if let Ok(vp) = el.get_pattern::<UIValuePattern>() {
            c.has_text_field = true;
            let v = vp.get_value().unwrap_or_default();
            let chars: Vec<char> = v.chars().collect();
            c.before = chars[chars.len().saturating_sub(600)..].iter().collect();
        }
        c
    }

    pub fn focused_text() -> Option<String> {
        let auto = UIAutomation::new().ok()?;
        let el = auto.get_focused_element().ok()?;
        if el.is_password().unwrap_or(false) {
            return None;
        }
        if let Ok(tp) = el.get_pattern::<UITextPattern>() {
            if let Ok(r) = tp.get_document_range() {
                return r.get_text(20000).ok();
            }
        }
        el.get_pattern::<UIValuePattern>().ok()?.get_value().ok()
    }
}

#[cfg(target_os = "macos")]
mod platform {
    //! macOS: only the app in front (NSWorkspace, no permission). Flow never uses Accessibility
    //! on a Mac, so it doesn't read text fields, selections or window titles there.
    use super::{friendly, FocusContext};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    pub fn window_only() -> FocusContext {
        let mut c = FocusContext::default();
        unsafe {
            let ws: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
            if ws.is_null() {
                return c;
            }
            let app: Option<Retained<AnyObject>> = msg_send![ws, frontmostApplication];
            if let Some(app) = app {
                let pid: i32 = msg_send![&*app, processIdentifier];
                c.pid = pid.max(0) as u32;
                let name: *mut AnyObject = msg_send![&*app, localizedName];
                if !name.is_null() {
                    let utf8: *const std::ffi::c_char = msg_send![name, UTF8String];
                    if !utf8.is_null() {
                        c.app = std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned();
                    }
                }
                c.exe = c.app.clone();
            }
        }
        if c.app.is_empty() {
            c.app = friendly(&c.exe);
        }
        c
    }

    pub fn capture(_read_text: bool) -> FocusContext {
        window_only()
    }

    pub fn focused_text() -> Option<String> {
        None
    }
}
#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::FocusContext;
    pub fn window_only() -> FocusContext {
        FocusContext::default()
    }
    pub fn capture(_: bool) -> FocusContext {
        FocusContext::default()
    }
    pub fn focused_text() -> Option<String> {
        None
    }
}

/// Is the foreground app this process (Toolbay Flow's own windows)?
pub fn is_self(c: &FocusContext) -> bool {
    c.pid == std::process::id()
}
