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
    rx.recv_timeout(Duration::from_millis(700)).unwrap_or_else(|_| platform::window_only())
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
                    let _ = a.move_endpoint_by_unit(Ep::End, TextUnit::Character, 300);
                    c.after = a.get_text(600).unwrap_or_default();
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
    //! macOS Accessibility (AXUIElement). Requires the Accessibility permission.
    use super::{friendly, FocusContext};
    use accessibility_sys::*;
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{CFRange, CFTypeRef};
    use std::ptr;

    unsafe fn attr(el: AXUIElementRef, name: &str) -> Option<CFType> {
        let key = CFString::new(name);
        let mut v: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(el, key.as_concrete_TypeRef(), &mut v);
        if err == kAXErrorSuccess && !v.is_null() {
            Some(CFType::wrap_under_create_rule(v))
        } else {
            None
        }
    }

    unsafe fn attr_string(el: AXUIElementRef, name: &str) -> Option<String> {
        let v = attr(el, name)?;
        v.downcast::<CFString>().map(|s| s.to_string())
    }

    unsafe fn focused() -> (Option<CFType>, Option<CFType>) {
        let sys = AXUIElementCreateSystemWide();
        let app = attr(sys, "AXFocusedApplication");
        let el = app.as_ref().and_then(|a| attr(a.as_CFTypeRef() as AXUIElementRef, "AXFocusedUIElement"));
        core_foundation_sys::base::CFRelease(sys as CFTypeRef);
        (app, el)
    }

    pub fn window_only() -> FocusContext {
        let mut c = FocusContext::default();
        unsafe {
            let (app, _) = focused();
            if let Some(app) = app {
                let a = app.as_CFTypeRef() as AXUIElementRef;
                c.app = attr_string(a, "AXTitle").unwrap_or_default();
                c.exe = c.app.clone();
                let mut pid = 0i32;
                AXUIElementGetPid(a, &mut pid);
                c.pid = pid as u32;
                if let Some(w) = attr(a, "AXFocusedWindow") {
                    c.window_title = attr_string(w.as_CFTypeRef() as AXUIElementRef, "AXTitle").unwrap_or_default();
                }
            }
        }
        if c.app.is_empty() {
            c.app = friendly(&c.exe);
        }
        c
    }

    pub fn capture(read_text: bool) -> FocusContext {
        let mut c = window_only();
        unsafe {
            let (_, el) = focused();
            let Some(el) = el else { return c };
            let e = el.as_CFTypeRef() as AXUIElementRef;
            let sub = attr_string(e, "AXSubrole").unwrap_or_default();
            c.is_password = sub == "AXSecureTextField";
            if c.is_password || !read_text {
                return c;
            }
            let value = attr_string(e, "AXValue");
            c.has_text_field = value.is_some();
            c.selected = attr_string(e, "AXSelectedText").unwrap_or_default();
            if let (Some(value), Some(r)) = (value, attr(e, "AXSelectedTextRange")) {
                let mut range = CFRange { location: 0, length: 0 };
                if AXValueGetValue(r.as_CFTypeRef() as AXValueRef, kAXValueTypeCFRange, &mut range as *mut _ as *mut _) {
                    let chars: Vec<u16> = value.encode_utf16().collect();
                    let start = (range.location as usize).min(chars.len());
                    let end = ((range.location + range.length) as usize).min(chars.len());
                    let b = String::from_utf16_lossy(&chars[start.saturating_sub(600)..start]);
                    let a = String::from_utf16_lossy(&chars[end..(end + 300).min(chars.len())]);
                    c.before = b;
                    c.after = a;
                }
            }
        }
        c
    }

    pub fn focused_text() -> Option<String> {
        unsafe {
            let (_, el) = focused();
            let el = el?;
            let e = el.as_CFTypeRef() as AXUIElementRef;
            if attr_string(e, "AXSubrole").as_deref() == Some("AXSecureTextField") {
                return None;
            }
            attr_string(e, "AXValue")
        }
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
