//! The Mac's own brightness keys, aimed at external displays.
//!
//! Apple keyboards deliver F1/F2 in two shapes depending on System Settings › Keyboard:
//! 1. Default (media keys): `kCGEventSystemDefined` (14) through the HID event tap, carrying
//!    `NX_SUBTYPE_AUX_CONTROL_BUTTONS` (8) in `NSEvent.subtype` and the key type + press/release
//!    state packed in `NSEvent.data1` (`NX_KEYTYPE_BRIGHTNESS_UP` = 2, `NX_KEYTYPE_BRIGHTNESS_DOWN`
//!    = 3; `0x0A` = key down, `0x0B` = key up). CoreGraphics' `kCGKeyboardEventKeycode` field is 0
//!    on these events, so `NSEvent` decoding is required.
//!    Approach adapted from Lunar (`alin23/Lunar`) and `alin23/MediaKeyTap` (MIT License).
//! 2. Standard function keys ("Use F1, F2, etc. keys as standard function keys" ticked): ordinary
//!    `kCGEventKeyDown` (10) / `kCGEventKeyUp` (11) events with virtual key codes 122 (F1) and
//!    120 (F2).
//!
//! Both shapes target the display under the pointer by default (or every / selected external
//! display if chosen in Settings). A built-in display uses its own macOS path (pass-through for
//! native media keys, DisplayServices for plain F1/F2); external displays always use DDC/CI.
//!
//! Needs Accessibility to install the active event tap. Without it the tap cannot be created, the
//! app reports `"Brightness keys need Accessibility"` with a button to open the pane, and logs
//! that state once per launch.

use std::{
    ffi::c_void,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};
use objc2::{msg_send, runtime::AnyObject};
use tauri::AppHandle;

use crate::hardware;

pub const NEED_ACCESSIBILITY: &str = "Brightness keys need Accessibility";

#[derive(Clone, Default, serde::Serialize)]
pub struct Status {
    pub enabled: bool,
    /// Whether macOS has granted Accessibility — the switch cannot work without it.
    pub trusted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyShape {
    Media,
    StandardFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BrightnessEvent {
    pub shape: KeyShape,
    pub brighter: bool,
    pub down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetPath {
    PassThrough,
    BuiltIn(u32),
    ExternalDdc(u32),
    AllExternalDdc,
    SelectedExternalDdc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TapAction {
    PassThrough,
    SwallowOnly,
    Step { target: TargetPath, brighter: bool },
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
    fn CGEventGetFlags(event: *mut c_void) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(allocator: *mut c_void, port: *mut c_void, order: i64) -> *mut c_void;
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRemoveSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRelease(object: *const c_void);
    fn CFDictionaryCreate(
        allocator: *mut c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        count: i64,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *mut c_void;
    static kCFRunLoopCommonModes: *const c_void;
    static kCFBooleanTrue: *const c_void;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

struct Tap {
    port: *mut c_void,
    source: *mut c_void,
}

// The tap stays on the main run loop; only apply() and timeout recovery touch these pointers.
unsafe impl Send for Tap {}

static APP: OnceLock<AppHandle> = OnceLock::new();
static TAP: Mutex<Option<Tap>> = Mutex::new(None);
static LOGGED_UNTRUSTED: AtomicBool = AtomicBool::new(false);

const K_CG_HID_EVENT_TAP: i64 = 0;
const K_CG_SESSION_EVENT_TAP: i64 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: i64 = 0;
const K_CG_EVENT_TAP_OPTION_DEFAULT: i64 = 0;

const K_CG_EVENT_KEY_DOWN: u32 = 10;
const K_CG_EVENT_KEY_UP: u32 = 11;
const K_CG_EVENT_SYSTEM_DEFINED: u32 = 14;
const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xffff_fffe;
const K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xffff_ffff;
const K_CG_KEYBOARD_EVENT_KEYCODE: i64 = 9;

const K_CG_EVENT_FLAG_MASK_CONTROL: u64 = 1 << 18;
const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 1 << 19;
const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 1 << 20;

const NX_SUBTYPE_AUX_CONTROL_BUTTONS: i16 = 8;
const NX_KEYTYPE_BRIGHTNESS_UP: usize = 2;
const NX_KEYTYPE_BRIGHTNESS_DOWN: usize = 3;
const NX_KEY_STATE_DOWN: usize = 0x0A;
const NX_KEY_STATE_UP: usize = 0x0B;

const K_VK_F2: i64 = 120;
const K_VK_F1: i64 = 122;
// Alternate macOS brightness keyDown/keyUp virtual keycodes (per Lunar Hotkeys.swift / MediaKeyTap).
const K_VK_BRIGHTNESS_UP: i64 = 144;
const K_VK_BRIGHTNESS_DOWN: i64 = 145;

fn should_reenable_tap(event_type: u32) -> bool {
    event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT
        || event_type == K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT
}

fn has_shortcut_modifier(flags: u64) -> bool {
    flags & (K_CG_EVENT_FLAG_MASK_COMMAND | K_CG_EVENT_FLAG_MASK_CONTROL | K_CG_EVENT_FLAG_MASK_ALTERNATE) != 0
}

/// Decodes the `subtype` and `data1` fields of an `NX_SYSDEFINED` `NSEvent`.
pub(crate) fn decode_media_data1(subtype: i16, data1: isize, flags: u64) -> Option<BrightnessEvent> {
    if subtype != NX_SUBTYPE_AUX_CONTROL_BUTTONS || has_shortcut_modifier(flags) {
        return None;
    }
    let bits = data1 as usize;
    let key_code = (bits & 0xFFFF_0000) >> 16;
    let key_state = (bits & 0x0000_FF00) >> 8;
    let brighter = match key_code {
        NX_KEYTYPE_BRIGHTNESS_UP => true,
        NX_KEYTYPE_BRIGHTNESS_DOWN => false,
        _ => return None,
    };
    let down = match key_state {
        NX_KEY_STATE_DOWN => true,
        NX_KEY_STATE_UP => false,
        _ => return None,
    };
    Some(BrightnessEvent { shape: KeyShape::Media, brighter, down })
}

/// Decodes plain F1/F2 (`122`/`120`) and hardware brightness (`145`/`144`) `kCGEventKeyDown`/`kCGEventKeyUp`.
pub(crate) fn decode_function_key(event_type: u32, keycode: i64, flags: u64) -> Option<BrightnessEvent> {
    if has_shortcut_modifier(flags) {
        return None;
    }
    let down = match event_type {
        K_CG_EVENT_KEY_DOWN => true,
        K_CG_EVENT_KEY_UP => false,
        _ => return None,
    };
    let (shape, brighter) = match keycode {
        K_VK_F2 => (KeyShape::StandardFunction, true),
        K_VK_F1 => (KeyShape::StandardFunction, false),
        K_VK_BRIGHTNESS_UP => (KeyShape::Media, true),
        K_VK_BRIGHTNESS_DOWN => (KeyShape::Media, false),
        _ => return None,
    };
    Some(BrightnessEvent { shape, brighter, down })
}

/// Extracts `(subtype, data1)` from a `kCGEventSystemDefined` `CGEventRef` via `NSEvent`.
unsafe fn read_system_defined(event: *mut c_void) -> Option<(i16, isize)> {
    objc2::rc::autoreleasepool(|_| {
        let cls = objc2::class!(NSEvent);
        let ns_event: *mut AnyObject = msg_send![cls, eventWithCGEvent: event];
        if ns_event.is_null() {
            return None;
        }
        let subtype: i16 = msg_send![ns_event, subtype];
        let data1: isize = msg_send![ns_event, data1];
        Some((subtype, data1))
    })
}

/// Decodes either supported brightness-key shape from a live `CGEventRef`.
pub(crate) unsafe fn decode_cgevent(event_type: u32, event: *mut c_void) -> Option<BrightnessEvent> {
    if event.is_null() {
        return None;
    }
    let flags = CGEventGetFlags(event);
    match event_type {
        K_CG_EVENT_SYSTEM_DEFINED => {
            let (subtype, data1) = read_system_defined(event)?;
            decode_media_data1(subtype, data1, flags)
        }
        K_CG_EVENT_KEY_DOWN | K_CG_EVENT_KEY_UP => {
            let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);
            decode_function_key(event_type, keycode, flags)
        }
        _ => None,
    }
}

/// Pure routing decision: chooses built-in vs DDC/CI vs pass-through from the decoded key and mode.
pub(crate) fn decide_action(
    event: BrightnessEvent,
    mode: crate::KeysMove,
    pointer: Option<(u32, bool)>,
) -> TapAction {
    let target = match mode {
        crate::KeysMove::Pointer => match pointer {
            None => TargetPath::PassThrough,
            Some((_, true)) if event.shape == KeyShape::Media => TargetPath::PassThrough,
            Some((id, true)) => TargetPath::BuiltIn(id),
            Some((id, false)) => TargetPath::ExternalDdc(id),
        },
        crate::KeysMove::All => TargetPath::AllExternalDdc,
        crate::KeysMove::Selected => TargetPath::SelectedExternalDdc,
    };
    match target {
        TargetPath::PassThrough => TapAction::PassThrough,
        _ if event.down => TapAction::Step { target, brighter: event.brighter },
        _ => TapAction::SwallowOnly,
    }
}

/// Complete tap decision for a `CGEventRef`: decode + route. Tested directly with real `CGEventRef`s.
pub(crate) unsafe fn handle_cgevent(
    event_type: u32,
    event: *mut c_void,
    mode: crate::KeysMove,
    pointer: impl FnOnce() -> Option<(u32, bool)>,
) -> TapAction {
    let Some(decoded) = decode_cgevent(event_type, event) else {
        return TapAction::PassThrough;
    };
    decide_action(decoded, mode, pointer())
}

fn mark_untrusted_logged(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::SeqCst)
}

fn untrusted_status(detail: &str) -> Status {
    Status {
        enabled: false,
        trusted: false,
        error: Some(format!("{NEED_ACCESSIBILITY}. {detail}")),
    }
}

fn note_untrusted_once(app: &AppHandle) {
    if mark_untrusted_logged(&LOGGED_UNTRUSTED) {
        let message = format!(
            "{NEED_ACCESSIBILITY} — allow Displays Studio KH in System Settings › Privacy & Security › Accessibility."
        );
        eprintln!("{message}");
        crate::log_line(app, &message);
    }
}

/// True when this app may post and filter key events.
pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// `{kAXTrustedCheckOptionPrompt: true}` — the option that makes macOS show its own grant dialog.
fn prompt_options() -> *mut c_void {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        CFDictionaryCreate(
            std::ptr::null_mut(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        )
    }
}

/// Ask macOS for Accessibility. The system shows its own dialog and the answer is given in
/// System Settings, so this returns what is true now, never what was requested.
pub fn request_accessibility() -> bool {
    unsafe {
        let options = prompt_options();
        if options.is_null() {
            return accessibility_trusted();
        }
        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        CFRelease(options);
        trusted
    }
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
        note_untrusted_once(app);
        return untrusted_status(
            "Allow Displays Studio KH in System Settings › Privacy & Security › Accessibility, then switch this on again.",
        );
    }

    unsafe {
        let events = (1u64 << K_CG_EVENT_SYSTEM_DEFINED)
            | (1u64 << K_CG_EVENT_KEY_DOWN)
            | (1u64 << K_CG_EVENT_KEY_UP);
        // Prefer the HID tap so media-key brightness events arrive before BezelServices consumes
        // them; fall back to the session tap if HID tap creation is refused.
        let mut port = CGEventTapCreate(
            K_CG_HID_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_DEFAULT,
            events,
            tap_callback,
            std::ptr::null_mut(),
        );
        if port.is_null() {
            port = CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_DEFAULT,
                events,
                tap_callback,
                std::ptr::null_mut(),
            );
        }
        if port.is_null() {
            note_untrusted_once(app);
            return untrusted_status(
                "macOS did not allow the brightness key tap. Check Accessibility permission and try again.",
            );
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

static LAST_STEP: Mutex<Option<(u32, bool, std::time::Instant)>> = Mutex::new(None);

/// Suppresses the duplicate event macOS can emit within ~30 ms when a single physical brightness
/// key press arrives as both `kCGEventKeyDown` (144/145) and `kCGEventSystemDefined` (2/3).
fn is_cross_shape_duplicate(event_type: u32, brighter: bool, now: std::time::Instant) -> bool {
    let mut last = LAST_STEP.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let duplicate = matches!(
        *last,
        Some((prev_type, prev_brighter, prev_at))
            if prev_type != event_type
                && prev_brighter == brighter
                && now.duration_since(prev_at) < std::time::Duration::from_millis(30)
    );
    if !duplicate {
        *last = Some((event_type, brighter, now));
    }
    duplicate
}

extern "C" fn tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    _refcon: *mut c_void,
) -> *mut c_void {
    if should_reenable_tap(event_type) {
        // macOS disables a tap on timeout or user input so the keyboard cannot lock up. Re-arm it.
        if let Some(tap) = TAP.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref() {
            unsafe { CGEventTapEnable(tap.port, 1); }
        }
        return event;
    }
    if event.is_null() {
        return event;
    }
    let Some(app) = APP.get() else { return event };
    let action = unsafe {
        handle_cgevent(event_type, event, crate::keys_move(app), || {
            hardware::pointer_display().map(|id| (id, hardware::is_builtin(id)))
        })
    };
    match action {
        TapAction::PassThrough => event,
        TapAction::SwallowOnly => std::ptr::null_mut(),
        TapAction::Step { target, brighter } => {
            if !is_cross_shape_duplicate(event_type, brighter, std::time::Instant::now()) {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::run_brightness_step(&app, target, brighter).await;
                });
            }
            std::ptr::null_mut()
        }
    }
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
    use super::*;
    use crate::KeysMove;
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use std::sync::atomic::AtomicBool;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(source: *const c_void, virtual_key: u16, key_down: bool) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRetain(object: *const c_void) -> *const c_void;
    }

    /// Builds a real in-memory `NX_SYSDEFINED` `CGEventRef` (never posted to HID or any tap).
    fn media_cgevent(subtype: i16, data1: isize, flags: u64) -> *mut c_void {
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct NSPoint { x: f64, y: f64 }
        unsafe impl objc2::Encode for NSPoint {
            const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
        }
        objc2::rc::autoreleasepool(|_| unsafe {
            let cls = objc2::class!(NSEvent);
            let ns_event: *mut AnyObject = msg_send![
                cls,
                otherEventWithType: 14usize,
                location: NSPoint { x: 0.0, y: 0.0 },
                modifierFlags: 0usize,
                timestamp: 0.0f64,
                windowNumber: 0isize,
                context: std::ptr::null::<AnyObject>(),
                subtype: subtype,
                data1: data1,
                data2: -1isize
            ];
            assert!(!ns_event.is_null(), "NSEvent otherEventWithType returned nil");
            let cg: *mut c_void = msg_send![ns_event, CGEvent];
            assert!(!cg.is_null(), "NSEvent CGEvent returned NULL");
            CFRetain(cg);
            if flags != 0 {
                CGEventSetFlags(cg, flags);
            }
            cg
        })
    }

    /// Builds a real in-memory keyboard `CGEventRef` (never posted to HID or any tap).
    fn keyboard_cgevent(virtual_key: u16, key_down: bool, flags: u64) -> *mut c_void {
        unsafe {
            let cg = CGEventCreateKeyboardEvent(std::ptr::null(), virtual_key, key_down);
            assert!(!cg.is_null(), "CGEventCreateKeyboardEvent returned NULL");
            CGEventSetFlags(cg, flags);
            cg
        }
    }

    /// Exact decoder logic shipped in 0.1.13 — kept here so the regression fixture proves 0.1.13
    /// produced no change on both real macOS event shapes.
    unsafe fn v0_1_13_handler(event_type: u32, event: *mut c_void) -> Option<bool> {
        if event_type != K_CG_EVENT_SYSTEM_DEFINED || event.is_null() {
            return None;
        }
        match CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) {
            2 => Some(true),
            3 => Some(false),
            _ => None,
        }
    }

    /// Minimal fake display rig proving the tap handler moves the targeted display's brightness
    /// through the right path (DDC/CI vs built-in) without touching real hardware.
    struct FakeRig {
        builtin_id: u32,
        builtin_level: f32,
        external_id: u32,
        external_ddc: u16,
        external_max: u16,
    }

    impl FakeRig {
        fn new() -> Self {
            Self { builtin_id: 1, builtin_level: 0.50, external_id: 2, external_ddc: 40, external_max: 100 }
        }

        fn apply(&mut self, action: TapAction) {
            if let TapAction::Step { target, brighter } = action {
                match target {
                    TargetPath::ExternalDdc(id) if id == self.external_id => {
                        self.external_ddc = hardware::step_to(self.external_ddc, self.external_max, brighter);
                    }
                    TargetPath::BuiltIn(id) if id == self.builtin_id => {
                        self.builtin_level = hardware::step_builtin_level(self.builtin_level, brighter);
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn the_prompt_options_build_and_release() {
        // Only the dictionary: passing it to AXIsProcessTrustedWithOptions here would put a system
        // dialog on the owner's screen during a test run.
        let options = prompt_options();
        assert!(!options.is_null(), "the Accessibility prompt option must build");
        unsafe { CFRelease(options) };
    }

    #[test]
    fn real_media_key_cgevent_fails_on_0_1_13_and_moves_external_display_on_0_1_14() {
        let up_down = media_cgevent(8, ((NX_KEYTYPE_BRIGHTNESS_UP << 16) | (NX_KEY_STATE_DOWN << 8)) as isize, 0);
        let up_release = media_cgevent(8, ((NX_KEYTYPE_BRIGHTNESS_UP << 16) | (NX_KEY_STATE_UP << 8)) as isize, 0);
        let down_down = media_cgevent(8, ((NX_KEYTYPE_BRIGHTNESS_DOWN << 16) | (NX_KEY_STATE_DOWN << 8)) as isize, 0);

        // Against 0.1.13: CGEventGetIntegerValueField(event, 9) returns 0, so nothing changes.
        assert_eq!(unsafe { v0_1_13_handler(K_CG_EVENT_SYSTEM_DEFINED, up_down) }, None);
        assert_eq!(unsafe { v0_1_13_handler(K_CG_EVENT_SYSTEM_DEFINED, down_down) }, None);

        // Against 0.1.14 with pointer on external display 2 (starting at 40/100):
        let mut rig = FakeRig::new();
        let a1 = unsafe { handle_cgevent(K_CG_EVENT_SYSTEM_DEFINED, up_down, KeysMove::Pointer, || Some((2, false))) };
        assert_eq!(a1, TapAction::Step { target: TargetPath::ExternalDdc(2), brighter: true });
        rig.apply(a1);
        assert_eq!(rig.external_ddc, 45, "media brightness-up moves 40 -> 45");

        // Key-up is swallowed without a second step.
        let a2 = unsafe { handle_cgevent(K_CG_EVENT_SYSTEM_DEFINED, up_release, KeysMove::Pointer, || Some((2, false))) };
        assert_eq!(a2, TapAction::SwallowOnly);
        rig.apply(a2);
        assert_eq!(rig.external_ddc, 45, "media key-up must not double-step");

        // Brightness-down moves 45 -> 40.
        let a3 = unsafe { handle_cgevent(K_CG_EVENT_SYSTEM_DEFINED, down_down, KeysMove::Pointer, || Some((2, false))) };
        assert_eq!(a3, TapAction::Step { target: TargetPath::ExternalDdc(2), brighter: false });
        rig.apply(a3);
        assert_eq!(rig.external_ddc, 40, "media brightness-down moves 45 -> 40");
        assert!((rig.builtin_level - 0.50).abs() < 1e-6, "built-in display untouched");

        unsafe {
            CFRelease(up_down);
            CFRelease(up_release);
            CFRelease(down_down);
        }
    }

    #[test]
    fn real_plain_f1_f2_cgevent_fails_on_0_1_13_and_moves_external_display_on_0_1_14() {
        let f2_down = keyboard_cgevent(K_VK_F2 as u16, true, 0);
        let f2_up = keyboard_cgevent(K_VK_F2 as u16, false, 0);
        let f1_down = keyboard_cgevent(K_VK_F1 as u16, true, 0);

        // Against 0.1.13: kCGEventKeyDown is never handled, so nothing changes.
        assert_eq!(unsafe { v0_1_13_handler(K_CG_EVENT_KEY_DOWN, f2_down) }, None);
        assert_eq!(unsafe { v0_1_13_handler(K_CG_EVENT_KEY_DOWN, f1_down) }, None);

        // Against 0.1.14 with pointer on external display 2 (starting at 40/100):
        let left = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(1920.0, 1080.0));
        let right = CGRect::new(&CGPoint::new(1920.0, 0.0), &CGSize::new(1280.0, 720.0));
        let under_pointer = hardware::display_at((2200.0, 500.0), &[(1, left), (2, right)]).unwrap();
        assert_eq!(under_pointer, 2);

        let mut rig = FakeRig::new();
        let a1 = unsafe { handle_cgevent(K_CG_EVENT_KEY_DOWN, f2_down, KeysMove::Pointer, || Some((under_pointer, false))) };
        assert_eq!(a1, TapAction::Step { target: TargetPath::ExternalDdc(2), brighter: true });
        rig.apply(a1);
        assert_eq!(rig.external_ddc, 45, "plain F2 moves 40 -> 45");

        let a2 = unsafe { handle_cgevent(K_CG_EVENT_KEY_UP, f2_up, KeysMove::Pointer, || Some((under_pointer, false))) };
        assert_eq!(a2, TapAction::SwallowOnly);
        rig.apply(a2);
        assert_eq!(rig.external_ddc, 45);

        let a3 = unsafe { handle_cgevent(K_CG_EVENT_KEY_DOWN, f1_down, KeysMove::Pointer, || Some((under_pointer, false))) };
        assert_eq!(a3, TapAction::Step { target: TargetPath::ExternalDdc(2), brighter: false });
        rig.apply(a3);
        assert_eq!(rig.external_ddc, 40, "plain F1 moves 45 -> 40");

        unsafe {
            CFRelease(f2_down);
            CFRelease(f2_up);
            CFRelease(f1_down);
        }
    }

    #[test]
    fn builtin_display_uses_its_own_path_never_ddc() {
        let media_up = media_cgevent(8, ((NX_KEYTYPE_BRIGHTNESS_UP << 16) | (NX_KEY_STATE_DOWN << 8)) as isize, 0);
        let f2_down = keyboard_cgevent(K_VK_F2 as u16, true, 0);

        // Media key on built-in display passes through to macOS's own native path + HUD.
        let a_media = unsafe { handle_cgevent(K_CG_EVENT_SYSTEM_DEFINED, media_up, KeysMove::Pointer, || Some((1, true))) };
        assert_eq!(a_media, TapAction::PassThrough);

        // Plain F2 on built-in display uses BuiltIn(1) (DisplayServices), never ExternalDdc.
        let mut rig = FakeRig::new();
        let a_f2 = unsafe { handle_cgevent(K_CG_EVENT_KEY_DOWN, f2_down, KeysMove::Pointer, || Some((1, true))) };
        assert_eq!(a_f2, TapAction::Step { target: TargetPath::BuiltIn(1), brighter: true });
        rig.apply(a_f2);
        assert!((rig.builtin_level - 0.55).abs() < 1e-6, "built-in stepped 0.50 -> 0.55");
        assert_eq!(rig.external_ddc, 40, "external DDC untouched when pointer is on built-in");

        unsafe {
            CFRelease(media_up);
            CFRelease(f2_down);
        }
    }

    #[test]
    fn modifier_chords_and_unrelated_events_pass_through() {
        for flags in [K_CG_EVENT_FLAG_MASK_COMMAND, K_CG_EVENT_FLAG_MASK_CONTROL, K_CG_EVENT_FLAG_MASK_ALTERNATE] {
            assert_eq!(decode_function_key(K_CG_EVENT_KEY_DOWN, K_VK_F2, flags), None);
            assert_eq!(decode_function_key(K_CG_EVENT_KEY_DOWN, K_VK_F1, flags), None);
            assert_eq!(
                decode_media_data1(8, ((NX_KEYTYPE_BRIGHTNESS_UP << 16) | (NX_KEY_STATE_DOWN << 8)) as isize, flags),
                None,
            );
        }
        // Wrong subtype (not 8) or non-brightness media key (e.g. volume = 0) are ignored.
        assert_eq!(decode_media_data1(7, ((NX_KEYTYPE_BRIGHTNESS_UP << 16) | (NX_KEY_STATE_DOWN << 8)) as isize, 0), None);
        assert_eq!(decode_media_data1(8, ((0usize << 16) | (NX_KEY_STATE_DOWN << 8)) as isize, 0), None);
        assert_eq!(decode_function_key(K_CG_EVENT_KEY_DOWN, 0, 0), None);
    }

    #[test]
    fn untrusted_state_names_accessibility_and_logs_once_per_launch() {
        let status = untrusted_status("Open System Settings.");
        assert!(!status.enabled);
        assert!(!status.trusted);
        assert!(status.error.as_deref().unwrap_or("").starts_with(NEED_ACCESSIBILITY));

        let flag = AtomicBool::new(false);
        assert!(mark_untrusted_logged(&flag), "first untrusted encounter logs once");
        assert!(!mark_untrusted_logged(&flag), "second encounter in same launch does not repeat");
        assert!(!mark_untrusted_logged(&flag), "third encounter in same launch does not repeat");
    }

    #[test]
    fn real_brightness_keydown_144_145_fails_on_0_1_13_and_moves_external_display_on_0_1_14() {
        let up_down = keyboard_cgevent(K_VK_BRIGHTNESS_UP as u16, true, 0);
        let up_release = keyboard_cgevent(K_VK_BRIGHTNESS_UP as u16, false, 0);
        let down_down = keyboard_cgevent(K_VK_BRIGHTNESS_DOWN as u16, true, 0);

        // Against 0.1.13: kCGEventKeyDown is never handled, so 144/145 produce no change.
        assert_eq!(unsafe { v0_1_13_handler(K_CG_EVENT_KEY_DOWN, up_down) }, None);
        assert_eq!(unsafe { v0_1_13_handler(K_CG_EVENT_KEY_DOWN, down_down) }, None);

        // Against 0.1.14 with pointer on external display 2 (starting at 40/100):
        let mut rig = FakeRig::new();
        let a1 = unsafe { handle_cgevent(K_CG_EVENT_KEY_DOWN, up_down, KeysMove::Pointer, || Some((2, false))) };
        assert_eq!(a1, TapAction::Step { target: TargetPath::ExternalDdc(2), brighter: true });
        rig.apply(a1);
        assert_eq!(rig.external_ddc, 45, "keycode 144 moves 40 -> 45");

        let a2 = unsafe { handle_cgevent(K_CG_EVENT_KEY_UP, up_release, KeysMove::Pointer, || Some((2, false))) };
        assert_eq!(a2, TapAction::SwallowOnly);
        rig.apply(a2);
        assert_eq!(rig.external_ddc, 45, "keycode 144 key-up must not double-step");

        let a3 = unsafe { handle_cgevent(K_CG_EVENT_KEY_DOWN, down_down, KeysMove::Pointer, || Some((2, false))) };
        assert_eq!(a3, TapAction::Step { target: TargetPath::ExternalDdc(2), brighter: false });
        rig.apply(a3);
        assert_eq!(rig.external_ddc, 40, "keycode 145 moves 45 -> 40");

        // On built-in display, hardware brightness keycode 144 passes through natively.
        let a_builtin = unsafe { handle_cgevent(K_CG_EVENT_KEY_DOWN, up_down, KeysMove::Pointer, || Some((1, true))) };
        assert_eq!(a_builtin, TapAction::PassThrough);

        unsafe {
            CFRelease(up_down);
            CFRelease(up_release);
            CFRelease(down_down);
        }
    }

    #[test]
    fn cross_shape_duplicate_within_30ms_is_suppressed_while_repeats_pass() {
        let t0 = std::time::Instant::now();
        {
            *LAST_STEP.lock().unwrap_or_else(|p| p.into_inner()) = None;
        }
        assert!(!is_cross_shape_duplicate(K_CG_EVENT_KEY_DOWN, true, t0));
        // Mirror NX_SYSDEFINED arriving 5 ms after keyDown 144 for the same direction is suppressed.
        assert!(is_cross_shape_duplicate(
            K_CG_EVENT_SYSTEM_DEFINED,
            true,
            t0 + std::time::Duration::from_millis(5),
        ));
        // Same-shape key repeat is never suppressed.
        assert!(!is_cross_shape_duplicate(
            K_CG_EVENT_KEY_DOWN,
            true,
            t0 + std::time::Duration::from_millis(10),
        ));
        // Different shape after 40 ms is a fresh press.
        assert!(!is_cross_shape_duplicate(
            K_CG_EVENT_SYSTEM_DEFINED,
            true,
            t0 + std::time::Duration::from_millis(50),
        ));
    }

    #[test]
    fn a_timed_out_tap_has_a_reenable_path() {
        assert_eq!(K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT, 0xffff_fffe);
        assert!(should_reenable_tap(K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT));
        assert!(should_reenable_tap(K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT));
        assert!(!should_reenable_tap(K_CG_EVENT_KEY_DOWN));
    }
}
