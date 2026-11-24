//! §5d screenshots, taken from the build being released: the app's own webviews, in an isolated data
//! directory, with invented demo content — never a pointer, never the owner's desktop, never a real
//! device name. `--snapshot <dir>` writes `<window>-<light|dark>.tiff`; the release step converts to PNG.
use std::{
    path::{Path, PathBuf},
    sync::{mpsc, Mutex},
    time::{Duration, Instant},
};

use objc2::{rc::Retained, runtime::NSObjectProtocol, ClassType, MainThreadMarker};
use objc2_app_kit::{NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSImage, NSView, NSWindow};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSNumber, NSRunLoop, NSString};
use objc2_web_kit::{WKSnapshotConfiguration, WKWebView};
use tauri::Manager;

/// Demo content, so no real product name ever reaches the pixels.
pub const DEMO_PRESETS: &str = r#"[
  {"name":"Editing","entries":[{"display":"demo-main","code":16,"percent":62},{"display":"demo-side","code":16,"percent":45}]},
  {"name":"Late night","entries":[{"display":"demo-main","code":16,"percent":18},{"display":"demo-main","code":18,"percent":40},{"display":"demo-side","code":16,"percent":12}]}
]"#;

static LOADED: Mutex<u32> = Mutex::new(0);

/// Called from each webview's page-load hook.
pub fn note_page_loaded() {
    *LOADED.lock().unwrap() += 1;
}

/// Capture both windows in both appearances, then quit with 0 only when every file was written.
pub fn run(app: tauri::AppHandle, dir: PathBuf) {
    std::thread::spawn(move || {
        let start = Instant::now();
        while *LOADED.lock().unwrap() < 2 && start.elapsed() < Duration::from_secs(20) {
            std::thread::sleep(Duration::from_millis(100));
        }
        // The pages render from an invoke() promise after load; a settled beat is enough for demo data.
        std::thread::sleep(Duration::from_millis(700));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let mut failed = Vec::new();
            for (window, name) in [("main", "panel"), ("settings", "settings")] {
                prepare(&handle, window);
                for (dark, tone) in [(false, "light"), (true, "dark")] {
                    let file = dir.join(format!("{name}-{tone}.tiff"));
                    // A first pass settles layout for a window that was never ordered on screen; the
                    // second one is the picture that ships.
                    let _ = capture(&handle, window, dark, &file);
                    if let Err(why) = capture(&handle, window, dark, &file) {
                        eprintln!("snapshot {name}-{tone}: {why}");
                        failed.push(format!("{name}-{tone}"));
                    }
                }
            }
            println!("SNAPSHOT {} files, {} failed{}{}", dir.display(), failed.len(),
                if failed.is_empty() { "" } else { ": " }, failed.join(","));
            handle.exit(if failed.is_empty() { 0 } else { 1 });
        });
    });
}

/// Demo setup before any capture: the intro banner belongs to a first run, and the panel is told to
/// fit the demo content once, so both tones are the same size.
fn prepare(app: &tauri::AppHandle, label: &str) {
    if label != "main" { return; }
    if let Some(window) = app.get_webview_window(label) {
        // Hide the first-run banner, let the layout settle, then measure — scrollHeight read in the
        // same tick as the hide still counts the banner.
        let _ = window.eval("try{localStorage.setItem('displays-intro-seen','1');const i=document.querySelector('#intro');if(i)i.hidden=true;}catch{}");
    }
    pump(0.4);
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.eval("try{const h=Math.ceil([...document.body.children].reduce((m,n)=>Math.max(m,n.getBoundingClientRect().bottom),0))+14;window.__TAURI__.core.invoke('fit_panel',{height:h}).catch(()=>{});}catch{}");
    }
    pump(1.5); // the window resizes to the demo content; the webview follows a beat later
}

fn capture(app: &tauri::AppHandle, label: &str, dark: bool, file: &Path) -> Result<(), String> {
    let window = app.get_webview_window(label).ok_or("window is not open")?;
    let ns = window.ns_window().map_err(|e| e.to_string())? as *mut NSWindow;
    let native: &NSWindow = unsafe { &*ns };
    let appearance = unsafe {
        NSAppearance::appearanceNamed(if dark { NSAppearanceNameDarkAqua } else { NSAppearanceNameAqua })
    };
    unsafe { native.setAppearance(appearance.as_deref()) };
    pump(0.6); // let the webview re-render prefers-color-scheme

    // Demo setup: the intro banner belongs to a first run, not to the picture.
    let content = native.contentView().ok_or("window has no content view")?;
    let view = unsafe { find_webview(&content) }.ok_or("no webview under the window")?;
    let width = unsafe { (*view).frame().size.width };

    let mtm = MainThreadMarker::new().ok_or("the capture must run on the main thread")?;
    let config = unsafe { WKSnapshotConfiguration::new(mtm) };
    let pixels = unsafe { NSNumber::numberWithDouble(width * 2.0) };
    unsafe { config.setSnapshotWidth(Some(&pixels)) };

    let path = file.to_path_buf();
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    let handler = block2::RcBlock::new(move |image: *mut NSImage, error: *mut objc2_foundation::NSError| {
        let result = if image.is_null() {
            let why = if error.is_null() { "no image".to_string() }
                else { unsafe { (*error).localizedDescription().to_string() } };
            Err(why)
        } else {
            let data = unsafe { (*image).TIFFRepresentation() };
            match data {
                Some(data) => {
                    let name = NSString::from_str(&path.to_string_lossy());
                    if unsafe { data.writeToFile_atomically(&name, true) } {
                        Ok(())
                    } else { Err(format!("could not write {}", path.display())) }
                }
                None => Err("no TIFF data".to_string()),
            }
        };
        let _ = tx.send(result);
    });
    unsafe { (*view).takeSnapshotWithConfiguration_completionHandler(Some(&config), &handler) };

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(result) = rx.try_recv() {
            unsafe { native.setAppearance(None) };
            return result;
        }
        if Instant::now() > deadline {
            unsafe { native.setAppearance(None) };
            return Err("timed out waiting for the snapshot".into());
        }
        pump(0.05);
    }
}

/// Depth-first: wry nests the WKWebView under its own parent view, so the first webview found wins.
unsafe fn find_webview(view: &NSView) -> Option<*mut WKWebView> {
    if view.isKindOfClass(WKWebView::class()) {
        return Some(view as *const NSView as *mut WKWebView);
    }
    for sub in view.subviews().iter() {
        if let Some(found) = find_webview(&sub) {
            return Some(found);
        }
    }
    None
}

/// The completion handler runs on the main queue, so the main run loop must turn while we wait.
fn pump(seconds: f64) {
    let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
    unsafe { NSRunLoop::currentRunLoop().runMode_beforeDate(NSDefaultRunLoopMode, &until) };
}

// Keep the retained type used above justified: the window owns its views for the whole capture.
#[allow(dead_code)]
fn _assert_retained(_: Retained<NSView>) {}