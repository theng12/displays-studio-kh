mod hardware;

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

async fn ddc<T: Send + 'static>(app: &AppHandle, run: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let bus = app.state::<Bus>().0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = bus.try_lock().map_err(|_| "Another display action is in progress. Try again in a moment.")?;
        run()
    }).await.map_err(|_| "The display action stopped unexpectedly.")?
}

#[tauri::command]
async fn scan(app: AppHandle) -> Result<Vec<hardware::Display>, String> {
    ddc(&app, hardware::scan).await
}

#[tauri::command]
async fn set_level(app: AppHandle, display: String, code: u8, percent: u16) -> Result<hardware::Level, String> {
    ddc(&app, move || hardware::set(&display, code, percent)).await
}

fn keys() -> [Shortcut; 2] {
    let mods = Some(Modifiers::CONTROL | Modifiers::SHIFT);
    [Shortcut::new(mods, Code::BracketRight), Shortcut::new(mods, Code::BracketLeft)]
}

fn pref_file(app: &AppHandle) -> Result<PathBuf, String> {
    #[cfg(feature = "webview-check")]
    if let Some(test) = app.state::<Check>().0.as_ref() {
        return Ok(test.join("brightness-shortcuts"));
    }
    Ok(app.path().app_config_dir().map_err(|e| e.to_string())?.join("brightness-shortcuts"))
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
fn panel_info(app: AppHandle) -> (String, ShortcutStatus, &'static str) {
    (
        app.package_info().version.to_string(),
        app.state::<Shortcuts>().0.lock().unwrap().clone(),
        include_str!("../../ui-shell/whats-new.json"),
    )
}

#[tauri::command]
fn set_shortcuts(app: AppHandle, enabled: bool) -> ShortcutStatus {
    let mut status = register_shortcuts(&app, enabled);
    if status.error.is_none() {
        if let Err(e) = pref_file(&app).and_then(|path| {
            std::fs::create_dir_all(path.parent().expect("preference directory"))
                .and_then(|()| std::fs::write(path, if enabled { "on" } else { "off" }))
                .map_err(|e| e.to_string())
        }) {
            status.error = Some(format!("Changed for this run, but couldn't save the shortcut setting: {e}"));
        }
    }
    *app.state::<Shortcuts>().0.lock().unwrap() = status.clone();
    status
}

#[tauri::command]
fn close_panel(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

fn create_panel(app: &AppHandle, hidden: bool) -> tauri::Result<tauri::WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Displays Studio KH").inner_size(370.0, 370.0).min_inner_size(300.0, 260.0)
        .visible(!hidden).focused(!hidden).maximizable(false)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny);
    #[cfg(feature = "webview-check")]
    let builder = if let Some(dir) = app.state::<Check>().0.as_ref() {
        builder.data_directory(dir.join("webkit")).incognito(true).on_page_load(|window, event| {
            if event.event() == tauri::webview::PageLoadEvent::Finished {
                if let Err(e) = window.eval(include_str!("webview-check.js")) { eprintln!("check eval: {e}"); }
            }
        })
    } else { builder };
    builder.build()
}

fn show_panel(app: &AppHandle) {
    let result = match app.get_webview_window("main") {
        Some(window) => window.show().and_then(|()| window.unminimize()).and_then(|()| window.set_focus()),
        None => create_panel(app, false).map(|_| ()),
    };
    if let Err(e) = result {
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
        [flag, dir] if flag == "--webview-check" && std::path::Path::new(dir).is_dir() => Some(PathBuf::from(dir)),
        _ => return Err("Expected --webview-check <isolated data directory> or --ddc-check <exact monitor name>.".into()),
    };

    let builder = tauri::Builder::default().manage(Bus::default()).manage(Shortcuts::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed { return; }
            let app = app.clone();
            let brighter = shortcut.id() == keys()[0].id();
            tauri::async_runtime::spawn(async move {
                let outcome = ddc(&app, move || hardware::step(brighter)).await;
                if let Some(tray) = app.tray_by_id("displays") {
                    let message = match &outcome { Ok(step) => step.message(), Err(reason) => reason };
                    let _ = tray.set_tooltip(Some(message));
                }
                let _ = app.emit_to("main", "brightness-step", outcome);
            });
        }).build());
    #[cfg(not(feature = "webview-check"))]
    let builder = builder.invoke_handler(tauri::generate_handler![scan, set_level, panel_info, set_shortcuts, close_panel]);
    #[cfg(feature = "webview-check")]
    let builder = builder.manage(Check(test)).invoke_handler(tauri::generate_handler![scan, set_level, panel_info, set_shortcuts, close_panel, check_result]);

    let app = builder.setup(|app| {
        app.set_activation_policy(ActivationPolicy::Accessory);
        #[cfg(feature = "webview-check")]
        let hidden = app.state::<Check>().0.is_some();
        #[cfg(not(feature = "webview-check"))]
        let hidden = false;

        let should_register = pref_file(app.handle())
            .ok().and_then(|path| std::fs::read_to_string(path).ok()).as_deref() != Some("off");
        *app.state::<Shortcuts>().0.lock().unwrap() = register_shortcuts(app.handle(), should_register);
        if hidden {
            create_panel(app.handle(), true)?;
            return Ok(());
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
                if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) {
                    show_panel(tray.app_handle());
                }
            }).build(app)?;
        Ok(())
    }).build(tauri::generate_context!())?;

    app.run(|app, event| match event {
        RunEvent::Reopen { .. } => {
            #[cfg(feature = "webview-check")]
            if app.state::<Check>().0.is_some() { return; }
            show_panel(app);
        }
        RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        _ => {}
    });
    Ok(())
}
