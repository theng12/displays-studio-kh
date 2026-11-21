//! The Mac's own brightness keys, aimed at external displays.
//!
//! The F1/F2 keys do not send F1/F2: they post `kCGEventSystemDefined` events whose key code is
//! `NX_KEYTYPE_BRIGHTNESS_DOWN` (3) or `NX_KEYTYPE_BRIGHTNESS_UP` (2). macOS answers those for the
//! built-in and Apple displays only, so a third-party monitor ignores them. This module taps those
//! two events, swallows them, and steps the external displays instead.
//!
//! Needs Accessibility. Without it the tap cannot be created, nothing is swallowed, and the panel
//! says so in plain words.

use std::{ffi::c_void, process::Stdio, sync::{Mutex, OnceLock}};
use tauri::{AppHandle, Emitter};

use crate::hardware;

#[derive(Clone, Default, serde::Serialize)]
pub struct Status {
    pub enabled: bool,
    /// Whether macOS has granted Accessibility — the switch cannot work without it.
    pub trusted: bool,
    pub error: Option<String>,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: i64,
        place: i64,
        options: i64,
        events_of_interest: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        refcon: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: u8);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: i64) -> i64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(allocator: *mut c_void, port: *mut c_void, order: i64) -> *mut c_void;
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRemoveSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRelease(object: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
}

struct Tap {
    port: *mut c_void,
    source: *mut c_void,
}

// The tap stays on the main run loop; only apply() reads or writes these pointers.
unsafe impl Send for Tap {}

static APP: OnceLock<AppHandle> = OnceLock::new();
static TAP: Mutex<Option<Tap>> = Mutex::new(None);

const K_CG_EVENT_SYSTEM_DEFINED: u32 = 14;
const K_CG_KEYBOARD_EVENT_KEYCODE: i64 = 9;

fn brightness_key(code: i64) -> Option<bool> {
    match code {
        2 => Some(true),
        3 => Some(false),
        _ => None,
    }
}

/// True when this app may post and filter key events.
pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// Start or stop the tap. Returns what is true afterwards, never what was asked for.
pub fn apply(app: &AppHandle, enabled: bool) -> Status {
    let _ = APP.set(app.clone());
    let mut tap = TAP.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    if !enabled {
        if let Some(old) = tap.take() {
            unsafe {
                CGEventTapEnable(old.port, 0);
                CFRunLoopRemoveSource(CFRunLoopGetMain(), old.source, kCFRunLoopCommonModes);
                CFRelease(old.source);
                CFRelease(old.port);
            }
        }
        return Status { enabled: false, trusted: accessibility_trusted(), error: None };
    }

    if tap.is_some() {
        return Status { enabled: true, trusted: accessibility_trusted(), error: None };
    }
    if !accessibility_trusted() {
        return Status {
            trusted: accessibility_trusted(),
            enabled: false,
            error: Some("Allow Displays Studio KH in System Settings › Privacy & Security › Accessibility, then switch this on again.".into()),
        };
    }

    unsafe {
        let port = CGEventTapCreate(1, 0, 0, 1 << 14, tap_callback, std::ptr::null_mut());
        if port.is_null() {
            return Status {
                trusted: accessibility_trusted(),
                enabled: false,
                error: Some("macOS did not allow the brightness key tap. Check Accessibility permission and try again.".into()),
            };
        }
        let source = CFMachPortCreateRunLoopSource(std::ptr::null_mut(), port, 0);
        if source.is_null() {
            CFRelease(port);
            return Status {
                trusted: accessibility_trusted(),
                enabled: false,
                error: Some("Could not install the brightness key listener.".into()),
            };
        }
        CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
        CGEventTapEnable(port, 1);
        *tap = Some(Tap { port, source });
    }
    Status { enabled: true, trusted: accessibility_trusted(), error: None }
}

extern "C" fn tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    _refcon: *mut c_void,
) -> *mut c_void {
    if event_type != K_CG_EVENT_SYSTEM_DEFINED || event.is_null() {
        return event;
    }
    let code = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) };
    let Some(brighter) = brightness_key(code) else { return event };

    if let Some(app) = APP.get() {
        let app = app.clone();
        std::thread::spawn(move || {
            let _ = app.emit("brightness-step", hardware::step(brighter));
        });
    }
    std::ptr::null_mut()
}

/// Opens System Settings at the Accessibility pane.
pub fn open_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::brightness_key;

    #[test]
    fn brightness_up_key_is_brighter() {
        assert_eq!(brightness_key(2), Some(true));
    }

    #[test]
    fn brightness_down_key_is_dimmer() {
        assert_eq!(brightness_key(3), Some(false));
    }

    #[test]
    fn other_keys_are_ignored() {
        for code in [0, 1, 4, 14, -1, i64::MIN, i64::MAX] {
            assert_eq!(brightness_key(code), None);
        }
    }
}
