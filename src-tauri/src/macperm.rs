//! macOS privacy permissions Flow needs for typing and the shortcut.
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
    const GRANTED: u32 = 0; // kIOHIDAccessTypeGranted

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOHIDCheckAccess(request: u32) -> u32;
        fn IOHIDRequestAccess(request: u32) -> bool;
    }

    pub fn accessibility() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    pub fn input_monitoring() -> bool {
        unsafe { IOHIDCheckAccess(LISTEN_EVENT) == GRANTED }
    }

    pub fn request_accessibility() -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let opts = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
            AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef())
        }
    }

    pub fn request_input_monitoring() -> bool {
        unsafe { IOHIDRequestAccess(LISTEN_EVENT) }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn accessibility() -> bool { true }
    pub fn input_monitoring() -> bool { true }
    pub fn request_accessibility() -> bool { true }
    pub fn request_input_monitoring() -> bool { true }
}

pub use imp::*;
