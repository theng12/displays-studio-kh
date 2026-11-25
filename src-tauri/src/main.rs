mod brightness_keys;

/// Sparkle through the shared shim: one C call, one state flag, no Tauri types in between.
mod sparkle {
    use std::sync::atomic::{AtomicU8, Ordering};

    extern "C" {
        fn kh_sparkle_start() -> i32;
        fn kh_sparkle_last_error() -> *const std::ffi::c_char;
        fn kh_sparkle_set_state_callback(cb: extern "C" fn(i32));
    }

    static STATE: AtomicU8 = AtomicU8::new(0);

    extern "C" fn on_state(state: i32) {
        STATE.store(state.clamp(0, 2) as u8, Ordering::SeqCst);
    }

    /// True while Sparkle is installing or relaunching: the app must not refuse its own quit then.
    pub fn updating() -> bool {
        STATE.load(Ordering::SeqCst) >= 1
    }

    /// Called once, after the menu-bar item exists. A refusal lands in the log with its reason.
    pub fn start() {
        unsafe {
            kh_sparkle_set_state_callback(on_state);
            let rc = kh_sparkle_start();
            if rc != 0 {
                let reason = std::ffi::CStr::from_ptr(kh_sparkle_last_error()).to_string_lossy();
                eprintln!("kh-sparkle: refused to start ({rc}): {reason}");
            }
        }
    }
}
mod hardware;
mod hold;
mod hud;
mod hotkeys;
#[cfg(all(feature = "webview-check", target_os = "macos"))]
mod snapshot;
mod login;
mod presets;

use std::{path::PathBuf, sync::{Arc, Mutex}};
use tauri::{menu::{Menu, MenuItem, PredefinedMenuItem}, tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent}, ActivationPolicy, AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Default)]
struct Bus(Arc<Mutex<()>>);

#[derive(Default)]
struct Shortcuts(Mutex<ShortcutStatus>);

#[derive(Clone, Default, serde::Serialize)]
struct ShortcutStatus {
    enabled: bool,
    error: Option<String>,
}

async fn ddc<T: Send + 'static>(app: &AppHandle, label: &'static str, run: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    // The update hold refuses new work here, before any hardware is touched.
    let hold = app.state::<hold::Hold>();
    let id = hold.claim(label)?;
    let bus = app.state::<Bus>().0.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let _guard = bus.try_lock().map_err(|_| "Another display action is in progress. Try again in a moment.")?;
        // Test-only: a real DDC write takes milliseconds; this lets the hold be proven with work
        // genuinely in flight. Set only by tests/hold.sh.
        if let Some(ms) = std::env::var("DISPLAYS_STUDIO_TEST_WORK_MS").ok().and_then(|raw| raw.trim().parse::<u64>().ok()) {
            eprintln!("test work in flight for {ms} ms");
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        run()
    }).await.map_err(|_| "The display action stopped unexpectedly.".to_string())?;
    hold.settle(id);
    outcome
}

#[tauri::command]
async fn scan(app: AppHandle) -> Result<Vec<hardware::Display>, String> {
    #[cfg(all(feature = "webview-check", target_os = "macos"))]
    if app.state::<Snapshot>().0.is_some() {
        return Ok(hardware::demo());
    }
    ddc(&app, "Refreshing displays", hardware::scan).await
}

#[tauri::command]
async fn set_level(app: AppHandle, display: String, code: u8, percent: u16) -> Result<hardware::Level, String> {
    ddc(&app, "A display change", move || hardware::set(&display, code, percent)).await
}

fn presets_path(app: &AppHandle) -> Result<PathBuf, String> {
    pref_file(app, "presets.json")
}

fn preset_names(presets: &[presets::Preset]) -> Vec<String> {
    presets.iter().map(|preset| preset.name.clone()).collect()
}

#[tauri::command]
fn presets_list(app: AppHandle) -> Result<Vec<String>, String> {
    Ok(preset_names(&presets::load(&presets_path(&app)?)))
}

/// Remembers what the displays report right now. The read happens through the same hold and lock as
/// any other hardware action, so an update pause refuses it in plain words instead of racing it.
#[tauri::command]
async fn preset_save(app: AppHandle, name: String) -> Result<Vec<String>, String> {
    let name = presets::clean_name(&name)?;
    let entries = ddc(&app, "Saving a preset", hardware::snapshot).await?;
    if entries.is_empty() {
        return Err("Nothing to save: no monitor reported a setting.".into());
    }
    let path = presets_path(&app)?;
    let mut list = presets::load(&path);
    let preset = presets::Preset {
        name,
        entries: entries.into_iter().map(|(display, code, percent)| presets::Entry { display, code, percent }).collect(),
    };
    presets::upsert(&mut list, preset);
    presets::save(&path, &list)?;
    Ok(preset_names(&list))
}

#[tauri::command]
async fn preset_apply(app: AppHandle, name: String) -> Result<hardware::All, String> {
    let preset = presets::load(&presets_path(&app)?).into_iter().find(|preset| preset.name == name)
        .ok_or("That preset has gone. Save it again.")?;
    let items = preset.entries.into_iter().map(|entry| (entry.display, entry.code, entry.percent)).collect();
    ddc(&app, "Applying a preset", move || hardware::set_many(items)).await
}

#[tauri::command]
fn preset_delete(app: AppHandle, name: String) -> Result<Vec<String>, String> {
    let path = presets_path(&app)?;
    let mut list = presets::load(&path);
    presets::remove(&mut list, &name);
    presets::save(&path, &list)?;
    Ok(preset_names(&list))
}

fn keys() -> [Shortcut; 2] {
    let mods = Some(Modifiers::CONTROL | Modifiers::SHIFT);
    [Shortcut::new(mods, Code::BracketRight), Shortcut::new(mods, Code::BracketLeft)]
}

#[derive(Default)]
struct Keys(Mutex<brightness_keys::Status>);

/// True inside the headless check, which must never put a macOS permission dialog on screen.
#[cfg(feature = "webview-check")]
fn hidden_check(app: &AppHandle) -> bool {
    app.state::<Check>().0.is_some()
}
#[cfg(not(feature = "webview-check"))]
fn hidden_check(_: &AppHandle) -> bool {
    false
}

fn pref_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    #[cfg(feature = "webview-check")]
    if let Some(test) = app.state::<Check>().0.as_ref() {
        return Ok(test.join(name));
    }
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join(name))
}

/// A switch is on unless the user turned it off: absent preference means on.
fn pref_on(app: &AppHandle, name: &str) -> bool {
    pref_file(app, name).ok().and_then(|path| std::fs::read_to_string(path).ok()).as_deref() != Some("off")
}

fn save_pref(app: &AppHandle, name: &str, on: bool) -> Result<(), String> {
    pref_file(app, name).and_then(|path| {
        std::fs::create_dir_all(path.parent().expect("preference directory"))
            .and_then(|()| std::fs::write(path, if on { "on" } else { "off" }))
            .map_err(|e| e.to_string())
    })
}

fn hotkeys_path(app: &AppHandle) -> Result<PathBuf, String> {
    pref_file(app, "hotkeys.json")
}

/// A text preference; `fallback` when the file is absent, which is how a default stays a default.
fn pref_text(app: &AppHandle, name: &str, fallback: &str) -> String {
    pref_file(app, name).ok().and_then(|path| std::fs::read_to_string(path).ok())
        .map(|text| text.trim().to_string()).filter(|text| !text.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn save_text(app: &AppHandle, name: &str, value: &str) -> Result<(), String> {
    pref_file(app, name).and_then(|path| {
        std::fs::create_dir_all(path.parent().expect("preference directory"))
            .and_then(|()| std::fs::write(path, value))
            .map_err(|e| e.to_string())
    })
}

/// "pointer" (the display under the mouse), "all" (every external display) or "selected".
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeysMove { Pointer, All, Selected }

fn keys_follow(app: &AppHandle) -> String {
    pref_text(app, "keys-follow", "pointer")
}

pub(crate) fn keys_move(app: &AppHandle) -> KeysMove {
    match keys_follow(app).as_str() {
        "all" => KeysMove::All,
        "selected" => KeysMove::Selected,
        _ => KeysMove::Pointer,
    }
}

fn step_selected(app: &AppHandle, brighter: bool) -> Result<hardware::Step, String> {
    let selected = pref_text(app, "selected-display", "");
    if selected.is_empty() { hardware::step(brighter) }
    else { hardware::step_on(&selected, brighter) }
}

/// The displays the global keys and F1/F2 move: the one under the pointer, every one, or the
/// selected one. Built-in displays use macOS's DisplayServices path; external displays use DDC/CI.
pub(crate) fn step_follow(app: &AppHandle, brighter: bool) -> Result<hardware::Step, String> {
    match keys_move(app) {
        KeysMove::Pointer => match hardware::pointer_display() {
            Some(display) if hardware::is_builtin(display) => hardware::step_builtin(display, brighter),
            Some(display) => hardware::step_id(display, brighter),
            None => Err("Move the pointer over the display you want to change.".into()),
        },
        KeysMove::All => hardware::step(brighter),
        KeysMove::Selected => step_selected(app, brighter),
    }
}

/// Runs a tap-triggered brightness step through the shared DDC bus lock & update hold, updates
/// the tray tooltip, and emits the Result<Step, String> shape the panel expects.
pub(crate) async fn run_brightness_step(
    app: &AppHandle,
    target: brightness_keys::TargetPath,
    brighter: bool,
) {
    let handle = app.clone();
    let outcome = match target {
        brightness_keys::TargetPath::PassThrough => return,
        brightness_keys::TargetPath::BuiltIn(display) => {
            ddc(app, "Brightness keys", move || hardware::step_builtin(display, brighter)).await
        }
        brightness_keys::TargetPath::ExternalDdc(display) => {
            ddc(app, "Brightness keys", move || hardware::step_id(display, brighter)).await
        }
        brightness_keys::TargetPath::AllExternalDdc => {
            ddc(app, "Brightness keys", move || hardware::step(brighter)).await
        }
        brightness_keys::TargetPath::SelectedExternalDdc => {
            ddc(app, "Brightness keys", move || step_selected(&handle, brighter)).await
        }
    };
    if let Some(tray) = app.tray_by_id("displays") {
        let message = match &outcome { Ok(step) => step.message(), Err(reason) => reason };
        let _ = tray.set_tooltip(Some(message));
    }
    // The HUD answers "how far did that move?" where macOS draws no HUD of its own — external
    // displays only, never on the built-in screen.
    if let Ok(step) = &outcome {
        if !hidden_check(app) && pref_on(app, "hud") {
            if let Some((_display, percent)) = hud::pick(&target, &step.brightness_changes(), hardware::pointer_display()) {
                hud::show(app, percent);
            }
        }
    }
    let _ = app.emit_to("main", "brightness-step", outcome);
}

/// Registers the global pair (when it is on) and every per-display chord. One registration point, so
/// changing one setting cannot leave another's shortcut silently unregistered.
fn register_shortcuts(app: &AppHandle, enabled: bool) -> ShortcutStatus {
    let manager = app.global_shortcut();
    let chords = hotkeys::load(&hotkeys_path(app).unwrap_or_default());
    let attempt = (|| -> Result<(), String> {
        manager.unregister_all().map_err(|e| e.to_string())?;
        if enabled {
            manager.register_multiple(keys()).map_err(|e| e.to_string())?;
        }
        for hotkey in &chords {
            let chord = hotkeys::clean_chord(&hotkey.chord)?;
            let shortcut = <Shortcut as std::str::FromStr>::from_str(&chord)
                .map_err(|_| format!("{} is not a shortcut macOS accepts", hotkey.chord))?;
            manager.register(shortcut)
                .map_err(|e| format!("{} belongs to another app or macOS ({e})", hotkey.chord))?;
        }
        Ok(())
    })();
    let error = attempt.err().map(|e| {
        let _ = manager.unregister_all();
        format!("Couldn't register every shortcut: {e}.")
    });
    ShortcutStatus { enabled: enabled && error.is_none(), error }
}

/// If macOS has just granted Accessibility, install the tap without a restart.
fn ensure_keys(app: &AppHandle) {
    if hidden_check(app) { return; }
    let state = app.state::<Keys>();
    let mut status = state.0.lock().unwrap();
    if pref_on(app, "brightness-keys") && !status.enabled {
        *status = brightness_keys::apply(app, true);
    }
}

pub(crate) fn log_line(app: &AppHandle, line: &str) {
    // The hidden check never writes into the owner's Library/Logs.
    #[cfg(feature = "webview-check")]
    if hidden_check(app) { return; }
    let Ok(dir) = app.path().app_log_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() { return; }
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("brightness-keys.log")) else { return; };
    use std::io::Write;
    let _ = writeln!(file, "{line}");
}

/// The panel is a dropdown, not a scroller: it grows to fit the displays it lists (owner bug 2). The
/// hidden check measures the fixed 372×668 size, so it is left alone there.
#[tauri::command]
fn fit_panel(app: AppHandle, window: tauri::WebviewWindow, height: f64) {
    // The release build never reads the handle: only the check feature can suppress the resize.
    #[cfg(not(feature = "webview-check"))]
    let _ = &app;
    #[cfg(feature = "webview-check")]
    if hidden_check(&app) {
        #[cfg(all(feature = "webview-check", target_os = "macos"))]
        if app.state::<Snapshot>().0.is_some() {
            let _ = window.set_size(tauri::LogicalSize::new(372.0, height.clamp(300.0, 1400.0)));
        }
        return;
    }
    let _ = window.set_size(tauri::LogicalSize::new(372.0, height.clamp(300.0, 1400.0)));
}

#[tauri::command]
fn panel_info(app: AppHandle) -> (String, ShortcutStatus, login::Status, brightness_keys::Status, &'static str) {
    ensure_keys(&app);
    (
        app.package_info().version.to_string(),
        app.state::<Shortcuts>().0.lock().unwrap().clone(),
        login::Status { enabled: login::enabled(), error: None },
        app.state::<Keys>().0.lock().unwrap().clone(),
        include_str!("../../ui-shell/whats-new.json"),
    )
}

#[tauri::command]
fn set_shortcuts(app: AppHandle, enabled: bool) -> ShortcutStatus {
    let mut status = register_shortcuts(&app, enabled);
    if status.error.is_none() {
        if let Err(e) = save_pref(&app, "brightness-shortcuts", enabled) {
            status.error = Some(format!("Changed for this run, but couldn't save the shortcut setting: {e}"));
        }
    }
    *app.state::<Shortcuts>().0.lock().unwrap() = status.clone();
    status
}

#[tauri::command]
fn set_login(app: AppHandle, enabled: bool) -> login::Status {
    let mut status = login::apply(enabled);
    if status.error.is_none() {
        if let Err(e) = save_pref(&app, "launch-at-login", enabled) {
            status.error = Some(format!("Changed for this run, but couldn't save the setting: {e}"));
        }
    }
    status
}

#[tauri::command]
fn set_brightness_keys(app: AppHandle, enabled: bool) -> brightness_keys::Status {
    if enabled && !brightness_keys::accessibility_trusted() && !hidden_check(&app) {
        brightness_keys::request_accessibility();
    }
    let mut status = brightness_keys::apply(&app, enabled);
    if status.error.is_none() {
        if let Err(e) = save_pref(&app, "brightness-keys", enabled) {
            status.error = Some(format!("Changed for this run, but couldn't save the setting: {e}"));
        }
    }
    *app.state::<Keys>().0.lock().unwrap() = status.clone();
    status
}

#[tauri::command]
fn open_accessibility_settings() {
    brightness_keys::open_settings();
}

#[tauri::command]
async fn set_all(app: AppHandle, code: u8, percent: u16) -> Result<hardware::All, String> {
    ddc(&app, "Changing all displays", move || hardware::set_all(code, percent)).await
}

#[derive(serde::Serialize)]
struct SettingsInfo {
    version: String,
    shortcuts: ShortcutStatus,
    keys: brightness_keys::Status,
    login: login::Status,
    follow: String,
    selected: String,
    hud: bool,
    hotkeys: Vec<hotkeys::Hotkey>,
}

#[tauri::command]
fn settings_info(app: AppHandle) -> SettingsInfo {
    ensure_keys(&app);
    SettingsInfo {
        version: app.package_info().version.to_string(),
        shortcuts: app.state::<Shortcuts>().0.lock().unwrap().clone(),
        keys: app.state::<Keys>().0.lock().unwrap().clone(),
        login: login::Status { enabled: login::enabled(), error: None },
        follow: keys_follow(&app),
        selected: pref_text(&app, "selected-display", ""),
        hud: pref_on(&app, "hud"),
        hotkeys: hotkeys::load(&hotkeys_path(&app).unwrap_or_default()),
    }
}

/// Every registered shortcut is rebuilt from the saved list, so a change here cannot leave another
/// chord silently unregistered.
fn re_register(app: &AppHandle) {
    let status = register_shortcuts(app, pref_on(app, "brightness-shortcuts"));
    *app.state::<Shortcuts>().0.lock().unwrap() = status;
}

#[tauri::command]
fn set_keys_follow(app: AppHandle, follow: String, display: String) -> Result<SettingsInfo, String> {
    if follow != "pointer" && follow != "all" && follow != "selected" {
        return Err("Choose whether the keys move the display under the pointer, every display, or the selected one.".into());
    }
    save_text(&app, "keys-follow", &follow)?;
    save_text(&app, "selected-display", display.trim())?;
    Ok(settings_info(app))
}

#[tauri::command]
fn set_hud(app: AppHandle, enabled: bool) -> Result<SettingsInfo, String> {
    save_pref(&app, "hud", enabled)?;
    Ok(settings_info(app))
}

#[tauri::command]
fn set_hotkey(app: AppHandle, display: String, brighter: bool, chord: String) -> Result<SettingsInfo, String> {
    let chord = hotkeys::clean_chord(&chord)?;
    let path = hotkeys_path(&app)?;
    let mut list = hotkeys::load(&path);
    hotkeys::upsert(&mut list, hotkeys::Hotkey { display, brighter, chord });
    hotkeys::save(&path, &list)?;
    re_register(&app);
    let info = settings_info(app.clone());
    if let Some(error) = info.shortcuts.error.clone() { return Err(error); }
    Ok(info)
}

#[tauri::command]
fn clear_hotkey(app: AppHandle, display: String, brighter: bool) -> Result<SettingsInfo, String> {
    let path = hotkeys_path(&app)?;
    let mut list = hotkeys::load(&path);
    hotkeys::remove(&mut list, &display, brighter);
    hotkeys::save(&path, &list)?;
    re_register(&app);
    Ok(settings_info(app))
}

#[tauri::command]
fn open_settings_window(app: AppHandle) -> Result<(), String> {
    show_settings(&app);
    Ok(())
}

fn show_settings(app: &AppHandle) {
    open_settings(app, false)
}

/// `hidden` is the screenshot pass (§5d): same page, built without showing it or changing the app's
/// activation policy, so no window appears on anyone's screen.
fn open_settings(app: &AppHandle, hidden: bool) {
    // A menu-bar app has no Dock icon, so a real window must take the app out of accessory mode or it
    // opens behind everything; the close handler puts it back.
    if !hidden { let _ = app.set_activation_policy(ActivationPolicy::Regular); }
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show().and_then(|()| window.set_focus());
        return;
    }
    let handle = app.clone();
    let build = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Displays Studio KH Settings")
        .inner_size(560.0, 640.0)
        .resizable(false).minimizable(false).maximizable(false)
        .center()
        .visible(!hidden)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny);
    #[cfg(all(feature = "webview-check", target_os = "macos"))]
    let build = if hidden {
        build.on_page_load(|_, event| {
            if event.event() == tauri::webview::PageLoadEvent::Finished { snapshot::note_page_loaded(); }
        })
    } else { build };
    let window = match build.build() {
        Ok(window) => window,
        Err(e) => {
            eprintln!("Couldn't open Settings: {e}");
            let _ = handle.set_activation_policy(ActivationPolicy::Accessory);
            return;
        }
    };
    if !hidden {
        if let Err(e) = window.set_focus() {
            eprintln!("Couldn't focus Settings: {e}");
        }
    }
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            let _ = handle.set_activation_policy(ActivationPolicy::Accessory);
        }
    });
}

#[tauri::command]
fn close_panel(window: tauri::WebviewWindow) -> Result<(), String> {
    // Escape/Cmd+W fold the dropdown away; the window stays cached for the next open.
    window.hide().map_err(|e| e.to_string())
}

fn create_panel(app: &AppHandle, hidden: bool) -> tauri::Result<tauri::WebviewWindow> {
    // A menu-bar dropdown, not a window: no title bar, always above other apps, sized so every
    // control fits without scrolling (the owner compared it to Lunar, which drops from the tray).
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Displays Studio KH").inner_size(372.0, 668.0)
        .decorations(false).always_on_top(true).shadow(true).maximizable(false)
        .minimizable(false).resizable(false)
        .visible(!hidden).focused(!hidden)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny);
    #[cfg(all(feature = "webview-check", target_os = "macos"))]
    let capturing = app.state::<Snapshot>().0.is_some();
    #[cfg(all(feature = "webview-check", not(target_os = "macos")))]
    let capturing = false;
    #[cfg(feature = "webview-check")]
    let builder = if let Some(dir) = app.state::<Check>().0.as_ref() {
        if capturing {
            // The headless check must not run during a capture: it closes the window when it finishes.
            builder.data_directory(dir.join("webkit")).incognito(true).on_page_load(|_, event| {
                if event.event() == tauri::webview::PageLoadEvent::Finished {
                    #[cfg(all(feature = "webview-check", target_os = "macos"))]
                    snapshot::note_page_loaded();
                }
            })
        } else {
            builder.data_directory(dir.join("webkit")).incognito(true).on_page_load(|window, event| {
                if event.event() == tauri::webview::PageLoadEvent::Finished {
                    if let Err(e) = window.eval(include_str!("webview-check.js")) { eprintln!("check eval: {e}"); }
                }
            })
        }
    } else { builder };
    let window = builder.build()?;
    #[cfg(not(feature = "webview-check"))]
    {
        // A dropdown closes when the person clicks anywhere else: hide the panel, keep it alive.
        let handle = app.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if let Some(panel) = handle.get_webview_window("main") {
                    let _ = panel.hide();
                }
            }
        });
    }
    Ok(window)
}

fn show_panel(app: &AppHandle) {
    show_panel_at(app, None);
}

/// Opens the panel, optionally anchored just under the tray icon (menu-bar dropdown).
fn show_panel_at(app: &AppHandle, under_icon: Option<(f64, f64)>) {
    ensure_keys(app);
    let window = match app.get_webview_window("main") {
        Some(window) => window,
        None => match create_panel(app, false) {
            Ok(window) => window,
            Err(e) => {
                eprintln!("Couldn't open Displays Studio KH: {e}");
                if let Some(tray) = app.tray_by_id("displays") {
                    let _ = tray.set_tooltip(Some("Couldn't open Displays Studio KH. Quit and reopen it."));
                }
                return;
            }
        },
    };
    if let Some((x, y)) = under_icon {
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
    if let Err(e) = window.show().and_then(|()| window.set_focus()) {
        eprintln!("Couldn't open Displays Studio KH: {e}");
        if let Some(tray) = app.tray_by_id("displays") {
            let _ = tray.set_tooltip(Some("Couldn't open Displays Studio KH. Quit and reopen it."));
        }
    }
}

#[cfg(feature = "webview-check")]
#[derive(Default)]
struct Check(Option<PathBuf>);

/// §5d: the directory screenshots are written to. Set only by `--snapshot <dir>`.
#[cfg(all(feature = "webview-check", target_os = "macos"))]
#[derive(Default)]
struct Snapshot(Option<PathBuf>);

#[cfg(feature = "webview-check")]
#[tauri::command]
fn check_result(app: AppHandle, window: tauri::WebviewWindow, result: String) -> Result<(), String> {
    if app.state::<Check>().0.is_none() { return Err("Verification mode is off.".into()); }
    if window.is_visible().map_err(|e| e.to_string())? {
        app.exit(1);
        return Err("The test window was visible.".into());
    }
    window.close().map_err(|e| e.to_string())?;
    println!("WEBVIEW_CHECK {result}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "webview-check")]
    let test = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => None,
        [flag, name] if flag == "--ddc-check" => {
            let (original, target, max) = hardware::verify_write(name)?;
            println!("DDC_CHECK {name}: {original}/{max} -> {target}/{max} -> {original}/{max} (read back)");
            return Ok(());
        }
        // --test-mode is the same isolated hidden run under the name the studio scanner looks for.
        [flag, dir] if (flag == "--webview-check" || flag == "--test-mode" || flag == "--snapshot") && std::path::Path::new(dir).is_dir() => Some(PathBuf::from(dir)),
        _ => return Err("Expected --webview-check (or --test-mode / --snapshot) <isolated data directory> or --ddc-check <exact monitor name>.".into()),
    };

    #[cfg(all(feature = "webview-check", target_os = "macos"))]
    let snapshot_dir = if std::env::args().any(|a| a == "--snapshot") { test.clone() } else { None };

    #[cfg(all(feature = "webview-check", target_os = "macos"))]
    let builder = tauri::Builder::default().manage(Snapshot(snapshot_dir)).manage(Bus::default());
    #[cfg(not(all(feature = "webview-check", target_os = "macos")))]
    let builder = tauri::Builder::default().manage(Bus::default());
    let builder = builder.manage(Shortcuts::default()).manage(Keys::default()).manage(hold::Hold::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed { return; }
            let app = app.clone();
            let id = shortcut.id();
            tauri::async_runtime::spawn(async move {
                // A per-display chord moves only its display; the global pair follows the Settings
                // choice — every display, or the selected one.
                let own = hotkeys::load(&hotkeys_path(&app).unwrap_or_default()).into_iter()
                    .find(|h| std::str::FromStr::from_str(&h.chord).map(|s: Shortcut| s.id()).ok() == Some(id));
                let outcome = match own {
                    Some(hotkey) => {
                        let (display, brighter) = (hotkey.display.clone(), hotkey.brighter);
                        ddc(&app, "Brightness hotkey", move || hardware::step_on(&display, brighter)).await
                    }
                    None => {
                        let brighter = id == keys()[0].id();
                        let handle = app.clone();
                        ddc(&app, "Brightness keys", move || step_follow(&handle, brighter)).await
                    }
                };
                if let Some(tray) = app.tray_by_id("displays") {
                    let message = match &outcome { Ok(step) => step.message(), Err(reason) => reason };
                    let _ = tray.set_tooltip(Some(message));
                }
                let _ = app.emit_to("main", "brightness-step", outcome);
            });
        }).build());
    #[cfg(not(feature = "webview-check"))]
    let builder = builder.invoke_handler(tauri::generate_handler![scan, set_level, set_all, panel_info, set_shortcuts, set_login, set_brightness_keys, open_accessibility_settings, close_panel, presets_list, preset_save, preset_apply, preset_delete, settings_info, set_keys_follow, set_hud, set_hotkey, clear_hotkey, open_settings_window, fit_panel]);
    #[cfg(feature = "webview-check")]
    let builder = builder.manage(Check(test)).invoke_handler(tauri::generate_handler![scan, set_level, set_all, panel_info, set_shortcuts, set_login, set_brightness_keys, open_accessibility_settings, close_panel, presets_list, preset_save, preset_apply, preset_delete, settings_info, set_keys_follow, set_hud, set_hotkey, clear_hotkey, open_settings_window, fit_panel, check_result]);

    let app = builder.setup(|app| {
        app.set_activation_policy(ActivationPolicy::Accessory);
        // The installer reads this URL from the signed feed and calls it before an update.
        hold::ensure_own_token();
        hold::start_server(app.handle().clone());
        #[cfg(feature = "webview-check")]
        let hidden = app.state::<Check>().0.is_some();
        #[cfg(not(feature = "webview-check"))]
        let hidden = false;

        *app.state::<Shortcuts>().0.lock().unwrap() =
            register_shortcuts(app.handle(), pref_on(app.handle(), "brightness-shortcuts"));
        #[cfg(all(feature = "webview-check", target_os = "macos"))]
        if let Some(dir) = app.state::<Snapshot>().0.as_ref().cloned() {
            // §5d: demo content in the isolated dir, both windows built hidden, then capture and quit.
            let _ = std::fs::write(dir.join("presets.json"), snapshot::DEMO_PRESETS);
            create_panel(app.handle(), true)?;
            open_settings(app.handle(), true);
            snapshot::run(app.handle().clone(), dir);
            return Ok(());
        }
        if hidden {
            create_panel(app.handle(), true)?;
            return Ok(());
        }
        // A hidden check must not install a login item or ask macOS for Accessibility.
        if pref_on(app.handle(), "launch-at-login") {
            let _ = login::apply(true);
        }
        let keys_on = pref_on(app.handle(), "brightness-keys");
        *app.state::<Keys>().0.lock().unwrap() = brightness_keys::apply(app.handle(), keys_on);
        // macOS lists an app in Accessibility only after it has asked. Ask once, on the first real
        // launch, when the keys are on and not yet trusted; the panel keeps the way back afterwards.
        let asked = pref_file(app.handle(), "accessibility-prompted").is_ok_and(|path| path.exists());
        if keys_on && !asked && !brightness_keys::accessibility_trusted() {
            let _ = save_pref(app.handle(), "accessibility-prompted", true);
            brightness_keys::request_accessibility();
        }
        let show = MenuItem::with_id(app, "show", "Show Displays", true, None::<&str>)?;
        let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("Cmd+,"))?;
        let quit = MenuItem::with_id(app, "quit", "Quit Displays Studio KH", true, Some("Cmd+Q"))?;
        let separator = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(app, &[&show, &settings, &separator, &quit])?;
        TrayIconBuilder::with_id("displays")
            .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
            .icon_as_template(true).tooltip("Displays Studio KH").menu(&menu).show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_panel(app),
                "settings" => show_settings(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                    let app = tray.app_handle();
                    // Drop the panel just under the tray icon, like a menu (Lunar's shape).
                    if let Some(panel) = app.get_webview_window("main") {
                        if let Ok(size) = panel.outer_size() {
                            let (px, py) = match rect.position {
                                tauri::Position::Physical(p) => (f64::from(p.x), f64::from(p.y)),
                                tauri::Position::Logical(p) => (p.x, p.y),
                            };
                            let (w, h) = match rect.size {
                                tauri::Size::Physical(s) => (f64::from(s.width), f64::from(s.height)),
                                tauri::Size::Logical(s) => (s.width, s.height),
                            };
                            let x = px + (w - size.width as f64) / 2.0;
                            let y = py + h + 6.0;
                            show_panel_at(app, Some((x, y)));
                            return;
                        }
                    }
                    show_panel(app);
                }
            }).build(app)?;
        // Measured check of the HUD offscreen, for the release gate. Never draws on the screen.
        // After the tray is built: the measurement covers the real menu-bar icon's frame.
        if std::env::var("DISPLAYS_STUDIO_HUD_SELFTEST").ok().as_deref() == Some("1") {
            hud::selftest(app.handle().clone());
        }
        // Last: the shim wants the menu-bar item to exist, and it must not run in the hidden check
        // (that returns above), where a refused updater would be noise on someone else's screen.
        sparkle::start();
        Ok(())
    }).build(tauri::generate_context!())?;

    app.run(|app, event| match event {
        RunEvent::Reopen { .. } => {
            #[cfg(feature = "webview-check")]
            if app.state::<Check>().0.is_some() { return; }
            show_panel(app);
        }
        // A menu-bar app stays alive when its only window hides — except while Sparkle is installing
        // or relaunching, which needs the process to exit.
        RunEvent::ExitRequested { code: None, api, .. } if !sparkle::updating() => api.prevent_exit(),
        _ => {}
    });
    Ok(())
}
