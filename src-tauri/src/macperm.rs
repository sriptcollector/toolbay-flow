//! macOS privacy permissions Flow needs for the microphone, typing and the shortcut.
//!
//! An app only appears in System Settings > Privacy > Accessibility / Input Monitoring after it
//! asks for access itself. Opening the settings page alone leaves the list empty, so the
//! request_* calls register Flow there (and show macOS's own prompt) before the page opens.

#[cfg(target_os = "macos")]
mod imp {
    use accessibility_sys::{kAXTrustedCheckOptionPrompt, AXIsProcessTrusted, AXIsProcessTrustedWithOptions};
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    // IOKit/hid/IOHIDLib.h
    const LISTEN_EVENT: u32 = 1; // kIOHIDRequestTypeListenEvent

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOHIDRequestAccess(request: u32) -> bool;
    }

    // CoreGraphics: the check that matches what the shortcut's event tap needs.
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightListenEventAccess() -> bool;
        fn CGRequestListenEventAccess() -> bool;
    }

    // AVFoundation: the microphone permission (Privacy & Security > Microphone).
    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {
        static AVMediaTypeAudio: &'static objc2::runtime::AnyObject;
    }

    pub fn accessibility() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// macOS has refused Flow the microphone (switched off in Privacy & Security, or blocked
    /// by a profile). Then recordings come back as pure silence with no error. Not asked yet
    /// counts as allowed: opening the microphone shows macOS's own prompt.
    pub fn microphone_blocked() -> bool {
        use objc2::{class, msg_send};
        // AVAuthorizationStatus: 0 not asked yet, 1 restricted, 2 denied, 3 allowed.
        let status: isize = unsafe { msg_send![class!(AVCaptureDevice), authorizationStatusForMediaType: AVMediaTypeAudio] };
        matches!(status, 1 | 2)
    }

    /// Input Monitoring is on if macOS says so, or if the shortcut's listener is already running
    /// (the real test: it can only start with the permission).
    pub fn input_monitoring() -> bool {
        crate::hotkeys::mac_listening() || unsafe { CGPreflightListenEventAccess() }
    }

    pub fn request_accessibility() -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let opts = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
            AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef())
        }
    }

    /// Clear Toolbay Flow's own entry for a privacy service (once per run). An older build's
    /// entry can show as switched on in System Settings while macOS ignores it for this build;
    /// after the reset, the request below adds a fresh entry that does apply.
    fn reset_once(service: &str, done: &std::sync::atomic::AtomicBool) {
        if done.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let _ = std::process::Command::new("/usr/bin/tccutil")
            .args(["reset", service, "ai.toolbay.flow"])
            .status();
    }

    static AX_RESET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    static INPUT_RESET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// Allow button: the permission isn't working, so start it over cleanly and ask again.
    pub fn fresh_request_accessibility() -> bool {
        if accessibility() {
            return true;
        }
        reset_once("Accessibility", &AX_RESET);
        request_accessibility()
    }

    pub fn fresh_request_input_monitoring() -> bool {
        if input_monitoring() {
            return true;
        }
        reset_once("ListenEvent", &INPUT_RESET);
        request_input_monitoring()
    }

    pub fn request_input_monitoring() -> bool {
        if input_monitoring() {
            return true;
        }
        // Either call adds Toolbay Flow to the Input Monitoring list; ask both so it's there.
        unsafe {
            let cg = CGRequestListenEventAccess();
            let hid = IOHIDRequestAccess(LISTEN_EVENT);
            cg || hid
        }
    }
}

/// System Settings > Keyboard > "Press 🌐 key to": 0 Do Nothing, 1 Change Input Source,
/// 2 Show Emoji & Symbols, 3 Start Dictation. Anything but 0 makes macOS act on Fn too.
/// None when unknown (not a Mac, or never changed: newer macOS then defaults to emoji).
#[cfg(target_os = "macos")]
pub fn fn_key_usage() -> Option<i64> {
    let out = std::process::Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.HIToolbox", "AppleFnUsageType"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(not(target_os = "macos"))]
pub fn fn_key_usage() -> Option<i64> {
    Some(0)
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
mod imp {
    pub fn accessibility() -> bool { true }
    pub fn microphone_blocked() -> bool { false }
    pub fn input_monitoring() -> bool { true }
    pub fn request_accessibility() -> bool { true }
    pub fn request_input_monitoring() -> bool { true }
    pub fn fresh_request_accessibility() -> bool { true }
    pub fn fresh_request_input_monitoring() -> bool { true }
}

pub use imp::*;
