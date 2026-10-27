//! Keeps your music sounding right while you dictate on a Mac.
//!
//! Bluetooth headphones (AirPods and the like) switch into call mode the moment their microphone
//! opens: playback drops to low-quality mono and gets quieter until the mic closes. Wispr Flow
//! avoids this by recording from the Mac's own microphone whenever the default input is a
//! Bluetooth headset, and so do we (only when no microphone was picked in Settings). If someone
//! did pick their headset's mic, we close it right after each dictation instead of keeping it warm.

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use std::ffi::c_void;

    #[repr(C)]
    struct Address {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyDataSize(id: u32, addr: *const Address, qsize: u32, q: *const c_void, size: *mut u32) -> i32;
        fn AudioObjectGetPropertyData(
            id: u32,
            addr: *const Address,
            qsize: u32,
            q: *const c_void,
            size: *mut u32,
            data: *mut c_void,
        ) -> i32;
    }

    const fn code(s: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*s)
    }
    const SYSTEM_OBJECT: u32 = 1;
    const SCOPE_GLOBAL: u32 = code(b"glob");
    const SCOPE_INPUT: u32 = code(b"inpt");
    const DEVICES: u32 = code(b"dev#");
    const DEFAULT_INPUT: u32 = code(b"dIn ");
    const DEVICE_NAME: u32 = code(b"name"); // the name cpal reports
    const TRANSPORT: u32 = code(b"tran");
    const STREAMS: u32 = code(b"stm#");
    const BLUETOOTH: u32 = code(b"blue");
    const BLUETOOTH_LE: u32 = code(b"blea");
    const BUILT_IN: u32 = code(b"bltn");

    fn addr(selector: u32, scope: u32) -> Address {
        Address { selector, scope, element: 0 }
    }

    fn get_u32(id: u32, selector: u32, scope: u32) -> Option<u32> {
        let a = addr(selector, scope);
        let mut v: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let st = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut v as *mut u32 as *mut c_void) };
        (st == 0).then_some(v)
    }

    fn devices() -> Vec<u32> {
        let a = addr(DEVICES, SCOPE_GLOBAL);
        let mut size = 0u32;
        if unsafe { AudioObjectGetPropertyDataSize(SYSTEM_OBJECT, &a, 0, std::ptr::null(), &mut size) } != 0 {
            return vec![];
        }
        let mut ids = vec![0u32; size as usize / std::mem::size_of::<u32>()];
        let st = unsafe { AudioObjectGetPropertyData(SYSTEM_OBJECT, &a, 0, std::ptr::null(), &mut size, ids.as_mut_ptr() as *mut c_void) };
        if st != 0 {
            return vec![];
        }
        ids
    }

    fn name(id: u32) -> Option<String> {
        let a = addr(DEVICE_NAME, SCOPE_GLOBAL);
        let mut s: CFStringRef = std::ptr::null();
        let mut size = std::mem::size_of::<CFStringRef>() as u32;
        let st = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut s as *mut CFStringRef as *mut c_void) };
        if st != 0 || s.is_null() {
            return None;
        }
        Some(unsafe { CFString::wrap_under_create_rule(s) }.to_string())
    }

    fn has_input(id: u32) -> bool {
        let a = addr(STREAMS, SCOPE_INPUT);
        let mut size = 0u32;
        unsafe { AudioObjectGetPropertyDataSize(id, &a, 0, std::ptr::null(), &mut size) == 0 && size > 0 }
    }

    fn is_bluetooth_id(id: u32) -> bool {
        matches!(get_u32(id, TRANSPORT, SCOPE_GLOBAL), Some(BLUETOOTH | BLUETOOTH_LE))
    }

    /// Whether the microphone Flow would use (a picked name, or the system default) is Bluetooth.
    pub fn is_bluetooth(device: Option<&str>) -> bool {
        match device {
            Some(n) => devices().into_iter().any(|id| name(id).as_deref() == Some(n) && is_bluetooth_id(id)),
            None => get_u32(SYSTEM_OBJECT, DEFAULT_INPUT, SCOPE_GLOBAL).is_some_and(is_bluetooth_id),
        }
    }

    /// The Mac's own microphone, if it has one.
    pub fn builtin_input() -> Option<String> {
        devices()
            .into_iter()
            .find(|&id| get_u32(id, TRANSPORT, SCOPE_GLOBAL) == Some(BUILT_IN) && has_input(id))
            .and_then(name)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn is_bluetooth(_device: Option<&str>) -> bool {
        false
    }
    pub fn builtin_input() -> Option<String> {
        None
    }
}

/// The microphone to open: the one picked in Settings; otherwise the default, unless that's a
/// Bluetooth headset and the Mac has its own mic (then the Mac's mic, so music keeps playing
/// normally in the headphones).
pub fn effective_input(picked: Option<String>) -> Option<String> {
    if picked.is_some() {
        return picked;
    }
    if imp::is_bluetooth(None) {
        if let Some(builtin) = imp::builtin_input() {
            log::info!("default mic is a Bluetooth headset; recording from {builtin} so your audio isn't lowered");
            return Some(builtin);
        }
    }
    None
}

/// Keep this mic open between dictations? Not a Bluetooth one: an open headset mic keeps the
/// headphones in call mode.
pub fn keep_warm(device: Option<&str>) -> bool {
    !imp::is_bluetooth(device)
}
