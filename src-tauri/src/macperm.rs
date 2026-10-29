//! macOS privacy permissions Flow needs: the microphone and Input Monitoring (the listen-only
//! shortcut listener). Flow NEVER asks for Accessibility: an app holding it can sit in front of
//! every key press, and a stall there froze a person's whole Mac.
//!
//! An app only appears in System Settings > Privacy > Input Monitoring after it asks for access
//! itself, so request_input_monitoring registers Flow there (and shows macOS's own prompt)
//! before the page opens.

#[cfg(target_os = "macos")]
mod imp {
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
    pub fn microphone_blocked() -> bool { false }
    pub fn input_monitoring() -> bool { true }
    pub fn request_input_monitoring() -> bool { true }
}

pub use imp::*;
