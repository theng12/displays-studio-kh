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
mod login;

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
    ddc(&app, "Refreshing displays", hardware::scan).await
}

#[tauri::command]
async fn set_level(app: AppHandle, display: String, code: u8, percent: u16) -> Result<hardware::Level, String> {
    ddc(&app, "A display change", move || hardware::set(&display, code, percent)).await
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

fn register_shortcuts(app: &AppHandle, enabled: bool) -> ShortcutStatus {
    let manager = app.global_shortcut();
    let error = manager.unregister_all().and_then(|()| {
        if enabled { manager.register_multiple(keys()) } else { Ok(()) }
    }).err().map(|e| {
        let _ = manager.unregister_all();
        format!("Couldn't change the brightness shortcuts: {e}")
    });
    ShortcutStatus { enabled: enabled && error.is_none(), error }
}

#[tauri::command]
fn panel_info(app: AppHandle) -> (String, ShortcutStatus, login::Status, brightness_keys::Status, &'static str) {
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
    #[cfg(feature = "webview-check")]
    let builder = if let Some(dir) = app.state::<Check>().0.as_ref() {
        builder.data_directory(dir.join("webkit")).incognito(true).on_page_load(|window, event| {
            if event.event() == tauri::webview::PageLoadEvent::Finished {
                if let Err(e) = window.eval(include_str!("webview-check.js")) { eprintln!("check eval: {e}"); }
            }
        })
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
        [flag, dir] if (flag == "--webview-check" || flag == "--test-mode") && std::path::Path::new(dir).is_dir() => Some(PathBuf::from(dir)),
        _ => return Err("Expected --webview-check (or --test-mode) <isolated data directory> or --ddc-check <exact monitor name>.".into()),
    };

    let builder = tauri::Builder::default().manage(Bus::default()).manage(Shortcuts::default()).manage(Keys::default()).manage(hold::Hold::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed { return; }
            let app = app.clone();
            let brighter = shortcut.id() == keys()[0].id();
            tauri::async_runtime::spawn(async move {
                let outcome = ddc(&app, "Brightness keys", move || hardware::step(brighter)).await;
                if let Some(tray) = app.tray_by_id("displays") {
                    let message = match &outcome { Ok(step) => step.message(), Err(reason) => reason };
                    let _ = tray.set_tooltip(Some(message));
                }
                let _ = app.emit_to("main", "brightness-step", outcome);
            });
        }).build());
    #[cfg(not(feature = "webview-check"))]
    let builder = builder.invoke_handler(tauri::generate_handler![scan, set_level, set_all, panel_info, set_shortcuts, set_login, set_brightness_keys, open_accessibility_settings, close_panel]);
    #[cfg(feature = "webview-check")]
    let builder = builder.manage(Check(test)).invoke_handler(tauri::generate_handler![scan, set_level, set_all, panel_info, set_shortcuts, set_login, set_brightness_keys, open_accessibility_settings, close_panel, check_result]);

    let app = builder.setup(|app| {
        app.set_activation_policy(ActivationPolicy::Accessory);
        // The installer reads this URL from the signed feed and calls it before an update.
        hold::start_server(app.handle().clone());
        #[cfg(feature = "webview-check")]
        let hidden = app.state::<Check>().0.is_some();
        #[cfg(not(feature = "webview-check"))]
        let hidden = false;

        *app.state::<Shortcuts>().0.lock().unwrap() =
            register_shortcuts(app.handle(), pref_on(app.handle(), "brightness-shortcuts"));
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
        let quit = MenuItem::with_id(app, "quit", "Quit Displays Studio KH", true, Some("Cmd+Q"))?;
        let separator = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(app, &[&show, &separator, &quit])?;
        TrayIconBuilder::with_id("displays")
            .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
            .icon_as_template(true).tooltip("Displays Studio KH").menu(&menu).show_menu_on_left_click(false)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "show" => show_panel(app),
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
