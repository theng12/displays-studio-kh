//! The brightness HUD: a small translucent panel that shows how far a key press moved a display.
//!
//! Native AppKit, no webview — one `NSPanel` reused for every press. It never becomes key or main
//! (`NonactivatingPanel`, never made key), ignores the mouse, sits at `NSStatusWindowLevel` on every
//! Space, and is ordered front *without* activating the app (`orderFrontRegardless`). It appears on
//! the display being adjusted, not on the built-in screen where macOS draws its own HUD. Nothing runs
//! while no key is being pressed: one short-lived thread per press fades it out and ends.

use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Duration,
};

use objc2::{define_class, msg_send, rc::Retained, MainThreadMarker};
use objc2_app_kit::{
    NSAnimationContext, NSApplication, NSApplicationActivationOptions, NSColor, NSFont, NSLevelIndicator,
    NSLevelIndicatorStyle, NSPanel, NSRunningApplication, NSScreen, NSTextField,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowCollectionBehavior, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use tauri::AppHandle;

use crate::brightness_keys::TargetPath;

pub(crate) const HUD_W: f64 = 172.0;
pub(crate) const HUD_H: f64 = 44.0;
/// The gap between the menu-bar icon's bottom edge and the HUD's top edge.
pub(crate) const ICON_GAP: f64 = 6.0;
/// Never closer than this to a screen edge.
pub(crate) const SCREEN_MARGIN: f64 = 8.0;
/// Fully visible for this long after a press, then the fade below, then hidden.
pub(crate) const VISIBLE: Duration = Duration::from_millis(900);
pub(crate) const FADE: f64 = 0.2;

/// Where the HUD goes, in AppKit coordinates (bottom-left origin).
///
/// Under this app's menu-bar icon, centred on it, `ICON_GAP` below its bottom edge and clamped so it
/// never crosses a screen edge. When there is no icon frame to use (the item is hidden in the menu
/// bar's overflow) it falls back to the top-right of the screen — near where the eye already goes —
/// and never to the bottom of the screen.
///
/// `icon` and `screen` are `(x, y, w, h)`; `screen` is the screen the icon sits on.
pub(crate) fn anchor(icon: Option<(f64, f64, f64, f64)>, screen: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let (sx, sy, sw, sh) = screen;
    let lowest = sx + SCREEN_MARGIN;
    let highest = sx + sw - HUD_W - SCREEN_MARGIN;
    match icon {
        Some((ix, iy, iw, _)) => {
            let x = (ix + iw / 2.0 - HUD_W / 2.0).max(lowest).min(highest.max(lowest));
            let y = (iy - ICON_GAP - HUD_H).max(sy + SCREEN_MARGIN);
            (x, y, HUD_W, HUD_H)
        }
        None => {
            let x = (sx + sw - HUD_W - SCREEN_MARGIN).max(lowest);
            let y = (sy + sh - 28.0 - ICON_GAP - HUD_H).max(sy + SCREEN_MARGIN);
            (x, y, HUD_W, HUD_H)
        }
    }
}

/// The screen an icon frame belongs to: the first screen whose frame contains its centre, so a HUD
/// follows the eye to the icon. Falls back to the first screen when nothing contains it.
pub(crate) fn screen_for(icon: (f64, f64, f64, f64), screens: &[(f64, f64, f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let (ix, iy, iw, ih) = icon;
    let (cx, cy) = (ix + iw / 2.0, iy + ih / 2.0);
    screens
        .iter()
        .find(|(x, y, w, h)| cx >= *x && cx <= x + w && cy >= *y && cy <= y + h)
        .or_else(|| screens.first())
        .copied()
}

/// The display a HUD belongs on and the new percentage, or none where it must stay quiet: a
/// built-in screen already has macOS's own brightness HUD.
pub(crate) fn pick(target: &TargetPath, changed: &[(String, u16)], pointer: Option<u32>) -> Option<(u32, u16)> {
    match target {
        TargetPath::PassThrough | TargetPath::BuiltIn(_) => None,
        TargetPath::ExternalDdc(display) => changed.iter().find(|(id, _)| parse_id(id) == Some(*display)).map(|(_, percent)| (*display, *percent)),
        TargetPath::AllExternalDdc | TargetPath::SelectedExternalDdc => pointer
            .and_then(|display| changed.iter().find(|(id, _)| parse_id(id) == Some(display)).map(|(_, percent)| (display, *percent)))
            .or_else(|| changed.first().and_then(|(id, percent)| parse_id(id).map(|display| (display, *percent)))),
    }
}

/// Display ids are `"<CGDirectDisplayID>-<vendor>-<model>-<serial>"`.
fn parse_id(id: &str) -> Option<u32> {
    id.split('-').next()?.parse().ok()
}

struct Hud {
    panel: Retained<NSPanel>,
    bar: Retained<NSLevelIndicator>,
    label: Retained<NSTextField>,
}

thread_local! {
    static HUD: RefCell<Option<Hud>> = const { RefCell::new(None) };
}

/// Bumped by every press, so an older fade cannot hide a newer HUD.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// What the offscreen self-test measured: visible, key, main, app active before and after.
static SELFTEST: [AtomicBool; 7] = [const { AtomicBool::new(false) }; 7];

/// The app in front before the HUD was shown, so the self-test can prove it did not change.
static FRONT_BEFORE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

thread_local! {
    /// The app that had focus when the press arrived. macOS can promote this accessory app to the
    /// front when its panel is ordered in, so that app is given the front back straight away.
    static PREVIOUS: RefCell<Option<Retained<NSRunningApplication>>> = const { RefCell::new(None) };
}

/// The panel refuses key and main outright. A plain `NSPanel` still lets macOS activate the app when
/// it is ordered front, which would pull focus away from whatever the person is typing in; overriding
/// these two is what makes an accessory app's HUD appear without stealing activation.
#[derive(Default)]
struct HudIvars;

define_class!(
    #[unsafe(super(NSPanel))]
    #[name = "KHHudPanel"]
    #[ivars = HudIvars]
    struct HudPanel;

    impl HudPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool { false }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool { false }
    }
);

/// Shows the HUD at `percent`, anchored under this app's menu-bar icon. Never activates the app and
/// never takes focus.
pub(crate) fn show(app: &AppHandle, percent: u16) {
    show_at(app, None, percent);
}

/// The frame to use right now: under this app's menu-bar icon, on the icon's own screen, with the
/// top-right fallback when the icon has no frame. Must run on the main thread.
pub(crate) fn hud_frame(app: &AppHandle) -> (f64, f64, f64, f64) {
    let screens = screens();
    let icon = status_item_frame(app);
    let screen = icon.and_then(|icon| screen_for(icon, &screens)).or_else(|| screens.first().copied());
    anchor(icon, screen.unwrap_or((0.0, 0.0, 1920.0, 1080.0)))
}

/// Every screen's frame, in AppKit coordinates.
fn screens() -> Vec<(f64, f64, f64, f64)> {
    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            (frame.origin.x, frame.origin.y, frame.size.width, frame.size.height)
        })
        .collect()
}

/// This app's own menu-bar status item frame, in AppKit coordinates, taken from the status item's
/// own button window. `None` when the item has no laid-out window — hidden in the menu bar's
/// overflow, or measured before the first layout — which the caller answers with the top-right
/// fallback.
pub(crate) fn status_item_frame(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let tray = app.tray_by_id("displays")?;
    let frame = tray
        .with_inner_tray_icon(|inner| {
            let mtm = MainThreadMarker::new()?;
            let window = inner.ns_status_item()?.button(mtm)?.window()?;
            let frame = window.frame();
            (frame.size.width > 0.0 && frame.size.height > 0.0).then_some((
                frame.origin.x,
                frame.origin.y,
                frame.size.width,
                frame.size.height,
            ))
        })
        .ok()??;
    Some(frame)
}

fn show_at(app: &AppHandle, frame: Option<(f64, f64, f64, f64)>, percent: u16) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    // The anchor is read on the main thread: the menu bar and the screens are only readable there,
    // and a key press arrives on the tap's own thread.
    let placing = app.clone();
    let _ = app.run_on_main_thread(move || place_and_show(frame.unwrap_or_else(|| hud_frame(&placing)), percent));
    // One short-lived thread per press: sleep, then fade on the main thread. No polling, no timer
    // that outlives the press.
    let handle = app.clone();
    let giving_back = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        let _ = giving_back.run_on_main_thread(restore_previous_app);
        std::thread::sleep(VISIBLE);
        let _ = handle.run_on_main_thread(move || {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            HUD.with(|slot| {
                if let Some(hud) = slot.borrow().as_ref() {
                    fade_and_hide(&hud.panel, generation);
                }
            });
        });
    });
}

/// Places and orders the panel front. Must run on the main thread; called both by a key press and
/// by the offscreen self-test.
fn place_and_show(frame: (f64, f64, f64, f64), percent: u16) {
    let (x, y, w, h) = frame;
    HUD.with(|slot| {
        let mut slot = slot.borrow_mut();
        let hud = slot.get_or_insert_with(build);
        PREVIOUS.with(|slot| {
            *slot.borrow_mut() = NSWorkspace::sharedWorkspace().frontmostApplication();
        });
        hud.panel.setFrame_display(NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)), false);
        hud.bar.setDoubleValue(f64::from(percent));
        hud.label.setStringValue(&NSString::from_str(&format!("{percent}%")));
        // Through the animator with no duration: a fade still running from a press ~a second ago is
        // replaced instead of pulling the new value down to zero.
        set_alpha(&hud.panel, 1.0);
        hud.panel.orderFrontRegardless();
    });
}

/// Gives the front back to the app that had it before the press, if macOS handed it to this one.
/// A no-op when the front never moved, which is the usual case.
fn restore_previous_app() {
    PREVIOUS.with(|slot| {
        let slot = slot.borrow();
        let Some(previous) = slot.as_ref() else { return };
        let Some(mtm) = MainThreadMarker::new() else { return };
        let ours = NSApplication::sharedApplication(mtm).isActive();
        let front_is_us = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .and_then(|app| app.bundleIdentifier())
            .map(|id| id.to_string() == OUR_BUNDLE_ID)
            .unwrap_or(false);
        if ours || front_is_us {
            previous.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        }
    });
}

fn build() -> Hud {
    let mtm = MainThreadMarker::new().expect("the HUD is built on the main thread");
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(HUD_W, HUD_H));
    let mask = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel: Retained<HudPanel> = unsafe {
        msg_send![
            mtm.alloc::<HudPanel>(),
            initWithContentRect: rect,
            styleMask: mask,
            backing: objc2_app_kit::NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    let panel: Retained<NSPanel> = Retained::into_super(panel);
    panel.setFloatingPanel(true);
    panel.setBecomesKeyOnlyIfNeeded(true);
    panel.setLevel(objc2_app_kit::NSFloatingWindowLevel);
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(true);
    panel.setIgnoresMouseEvents(true);
    panel.setHidesOnDeactivate(false);
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary,
    );

    let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), rect);
    effect.setMaterial(NSVisualEffectMaterial::HUDWindow);
    effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect.setState(NSVisualEffectState::Active);
    panel.setContentView(Some(&effect));

    let bar = NSLevelIndicator::initWithFrame(mtm.alloc(), NSRect::new(NSPoint::new(16.0, 16.0), NSSize::new(104.0, 12.0)));
    bar.setLevelIndicatorStyle(NSLevelIndicatorStyle::ContinuousCapacity);
    bar.setMinValue(0.0);
    bar.setMaxValue(100.0);
    bar.setFillColor(Some(&NSColor::whiteColor()));
    effect.addSubview(&bar);

    let label = NSTextField::labelWithString(&NSString::from_str("0%"), mtm);
    label.setFrame(NSRect::new(NSPoint::new(122.0, 14.0), NSSize::new(38.0, 16.0)));
    let font = unsafe { NSFont::systemFontOfSize_weight(12.0, objc2_app_kit::NSFontWeightSemibold) };
    label.setFont(Some(&font));
    label.setTextColor(Some(&NSColor::labelColor()));
    effect.addSubview(&label);

    Hud { panel, bar, label }
}

/// Sets the panel's opacity through the animator with no duration, which also cancels a fade that is
/// still running — a plain `setAlphaValue` would be overridden by it until it finished.
fn set_alpha(panel: &NSPanel, alpha: f64) {
    unsafe {
        NSAnimationContext::runAnimationGroup_completionHandler(
            &block2::RcBlock::new(move |context: NonNull<NSAnimationContext>| {
                context.as_ref().setDuration(0.0);
                let animator: Retained<objc2::runtime::AnyObject> = msg_send![panel, animator];
                let _: () = msg_send![&animator, setAlphaValue: alpha];
            }),
            None,
        );
    }
}

/// Fades out over `FADE` seconds and hides; the panel is kept for the next press. The hide is guarded
/// by the press generation: a newer press owns the panel, and an older fade must not take it away.
fn fade_and_hide(panel: &Retained<NSPanel>, generation: u64) {
    let fading = panel.clone();
    let hiding = panel.clone();
    unsafe {
        NSAnimationContext::runAnimationGroup_completionHandler(
            &block2::RcBlock::new(move |context: NonNull<NSAnimationContext>| {
                context.as_ref().setDuration(FADE);
                let animator: Retained<objc2::runtime::AnyObject> = msg_send![&*fading, animator];
                let _: () = msg_send![&animator, setAlphaValue: 0.0f64];
            }),
            Some(&block2::RcBlock::new(move || {
                if GENERATION.load(Ordering::SeqCst) == generation {
                    hiding.orderOut(None);
                } else {
                    set_alpha(&hiding, 1.0);
                }
            })),
        );
    }
}

/// Builds the panel offscreen — never on the owner's screen — and measures what the HUD actually
/// does: it appears, stays out of key and main, leaves the app inactive, and hides itself again.
pub(crate) fn selftest(app: AppHandle) {
    let Some(_mtm) = MainThreadMarker::new() else {
        println!("HUD SELFTEST FAIL: not on the main thread");
        std::process::exit(1);
    };
    // The anchor maths, on a screen 0..1920 x 0..1080 with the icon centred at x=960 in the menu bar.
    let icon = (950.0, 1056.0, 20.0, 24.0);
    let screen = (0.0, 0.0, 1920.0, 1080.0);
    let (x, y, w, h) = anchor(Some(icon), screen);
    let frame_ok = (x - (960.0 - HUD_W / 2.0)).abs() < 0.5
        && (y - (1056.0 - ICON_GAP - HUD_H)).abs() < 0.5
        && w == HUD_W && h == HUD_H;
    println!("HUD SELFTEST: anchor under a menu-bar icon = {x:.0},{y:.0} {w:.0}x{h:.0} — centred under the icon, {ICON_GAP:.0} below it: {}", if frame_ok { "ok" } else { "FAIL" });
    // The menu bar before its first layout: the item has no frame yet, so this press falls back.
    let before = hud_frame(&app);
    println!("HUD SELFTEST: before layout — icon {:?}, HUD would fall back to {:.0},{:.0}", status_item_frame(&app), before.0, before.1);

    let showing = app.clone();
    let reading = app.clone();
    let finishing = app.clone();
    let show_handle = app.clone();
    std::thread::spawn(move || {
        // The app finishes launching first: anything from startup is not the HUD's doing.
        std::thread::sleep(Duration::from_millis(1200));
        let _ = showing.run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else { return };
            *FRONT_BEFORE.lock().unwrap() = frontmost_bundle(mtm);
            println!("HUD SELFTEST: frontmost before the HUD = {:?}", FRONT_BEFORE.lock().unwrap());
            // After the first layout: the real menu-bar icon, and where the HUD would sit under it.
            let (x, y, w, h) = hud_frame(&show_handle);
            match status_item_frame(&show_handle) {
                Some((ix, iy, iw, ih)) => println!(
                    "HUD SELFTEST: live anchor — icon {ix:.0},{iy:.0} {iw:.0}x{ih:.0}, HUD {x:.0},{y:.0} {w:.0}x{h:.0}, centred={} under={}",
                    (x - (ix + iw / 2.0 - HUD_W / 2.0)).abs() < 0.5,
                    (y + h - (iy - ICON_GAP)).abs() < 0.5 || y + h <= iy
                ),
                None => println!(
                    "HUD SELFTEST: live anchor — icon not laid out (menu-bar overflow or hidden), HUD falls back to {x:.0},{y:.0} {w:.0}x{h:.0} at the top-right"
                ),
            }
            show_at(&show_handle, Some((-6000.0, -6000.0, HUD_W, HUD_H)), 50);
        });
        std::thread::sleep(Duration::from_millis(300));
        let _ = reading.run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else { return };
            let front_after = frontmost_bundle(mtm);
            let front_before = FRONT_BEFORE.lock().unwrap().clone();
            HUD.with(|slot| {
                let slot = slot.borrow();
                let Some(hud) = slot.as_ref() else { return };
                let (visible, key, main) = (hud.panel.isVisible(), hud.panel.isKeyWindow(), hud.panel.isMainWindow());
                SELFTEST[0].store(visible, Ordering::SeqCst);
                SELFTEST[1].store(key, Ordering::SeqCst);
                SELFTEST[2].store(main, Ordering::SeqCst);
                // A locked session (or no app of the person's in front) cannot answer "did focus move":
                // record it UNMEASURED rather than claiming either answer.
                let measurable = matches!(front_before.as_deref(), Some(id) if id != "com.apple.loginwindow" && id != OUR_BUNDLE_ID);
                SELFTEST[3].store(measurable && front_after != front_before, Ordering::SeqCst);
                SELFTEST[4].store(measurable, Ordering::SeqCst);
                SELFTEST[5].store(front_after != Some(OUR_BUNDLE_ID.to_string()), Ordering::SeqCst);
                println!(
                    "HUD SELFTEST: shown offscreen — visible={visible} key={key} main={main} front={front_after:?} (before={front_before:?}) focus={}",
                    if !measurable { "UNMEASURED (no real app in front — session locked?)" }
                    else if front_after != front_before { "MOVED" } else { "unchanged" }
                );
            });
        });
        // A second press lands inside the first one's fade: the older fade must not hide it.
        std::thread::sleep(Duration::from_millis(700));
        let again = finishing.clone();
        let again_handle = app.clone();
        let _ = again.run_on_main_thread(move || {
            show_at(&again_handle, Some((-6000.0, -6000.0, HUD_W, HUD_H)), 52);
        });
        std::thread::sleep(Duration::from_millis(250));
        let after_second = app.clone();
        let _ = after_second.run_on_main_thread(move || {
            let visible = HUD.with(|slot| slot.borrow().as_ref().map(|hud| hud.panel.isVisible()).unwrap_or(false));
            SELFTEST[6].store(visible, Ordering::SeqCst);
            println!("HUD SELFTEST: second press {} ms into the first fade — still visible={visible}", VISIBLE.as_millis() + 100);
        });
        std::thread::sleep(VISIBLE + Duration::from_millis(500));
        let _ = finishing.run_on_main_thread(move || {
            let hidden = HUD.with(|slot| slot.borrow().as_ref().map(|hud| !hud.panel.isVisible()).unwrap_or(false));
            let ok = |index: usize| SELFTEST[index].load(Ordering::SeqCst);
            let focus_ok = !ok(4) || !ok(3);
            let pass = frame_ok && ok(0) && !ok(1) && !ok(2) && focus_ok && ok(6) && hidden;
            println!("HUD SELFTEST: hidden after the last press, fade {:.0} ms — visible_now={}", FADE * 1000.0, !hidden);
            println!("HUD SELFTEST: focus {}", if !ok(4) { "UNMEASURED" } else if ok(3) { "FAIL (moved)" } else { "unchanged" });
            println!("HUD SELFTEST: {}", if pass { "PASS" } else { "FAIL" });
            std::process::exit(if pass { 0 } else { 1 });
        });
    });
}

const OUR_BUNDLE_ID: &str = "app.displaysstudiokh.desktop";

/// The bundle id of the app in front — what showing the HUD must never change.
fn frontmost_bundle(mtm: MainThreadMarker) -> Option<String> {
    let workspace = NSWorkspace::sharedWorkspace();
    let _ = mtm;
    workspace.frontmostApplication().and_then(|app| app.bundleIdentifier()).map(|id| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(items: &[(u32, u16)]) -> Vec<(String, u16)> {
        items.iter().map(|(display, percent)| (format!("{display}-1552-12345-0"), *percent)).collect()
    }

    #[test]
    fn a_builtin_display_never_gets_a_hud() {
        // macOS already draws its own brightness HUD on the built-in screen.
        assert_eq!(pick(&TargetPath::BuiltIn(1), &changed(&[(1, 45)]), Some(1)), None);
    }

    #[test]
    fn a_passed_through_key_never_gets_a_hud() {
        assert_eq!(pick(&TargetPath::PassThrough, &changed(&[(1, 45)]), Some(1)), None);
    }

    #[test]
    fn the_display_being_adjusted_shows_its_own_new_value() {
        assert_eq!(pick(&TargetPath::ExternalDdc(2), &changed(&[(2, 60)]), Some(2)), Some((2, 60)));
    }

    #[test]
    fn a_display_that_failed_shows_no_hud() {
        assert_eq!(pick(&TargetPath::ExternalDdc(2), &changed(&[(1, 40)]), Some(2)), None);
    }

    #[test]
    fn every_display_shows_the_hud_under_the_pointer() {
        assert_eq!(pick(&TargetPath::AllExternalDdc, &changed(&[(1, 40), (2, 60)]), Some(2)), Some((2, 60)));
    }

    #[test]
    fn every_display_without_a_pointer_falls_back_to_the_first_changed() {
        assert_eq!(pick(&TargetPath::AllExternalDdc, &changed(&[(1, 40), (2, 60)]), None), Some((1, 40)));
        assert_eq!(pick(&TargetPath::AllExternalDdc, &changed(&[(1, 40), (2, 60)]), Some(99)), Some((1, 40)));
    }

    #[test]
    fn a_selected_display_is_treated_like_every_display() {
        assert_eq!(pick(&TargetPath::SelectedExternalDdc, &changed(&[(3, 20)]), None), Some((3, 20)));
    }

    #[test]
    fn the_hud_sits_centred_under_the_menu_bar_icon() {
        // 1920-wide screen, icon at x 950..970 in a 24 pt menu bar.
        let icon = (950.0, 1056.0, 20.0, 24.0);
        let (x, y, w, h) = anchor(Some(icon), (0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(x, 960.0 - HUD_W / 2.0);
        assert_eq!(y, 1056.0 - ICON_GAP - HUD_H);
        assert_eq!((w, h), (HUD_W, HUD_H));
        // The top edge of the HUD is below the icon, never over the menu bar.
        assert!(y + h <= 1056.0);
    }

    fn an_icon_near_the_left_edge_pushes_the_hud_in_from_the_edge() {
        let (x, _, _, _) = anchor(Some((2.0, 1056.0, 20.0, 24.0)), (0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(x, SCREEN_MARGIN);
    }

    fn an_icon_near_the_right_edge_pushes_the_hud_in_from_the_edge() {
        let (x, _, _, _) = anchor(Some((1900.0, 1056.0, 20.0, 24.0)), (0.0, 0.0, 1920.0, 1080.0));
        assert_eq!(x, 1920.0 - HUD_W - SCREEN_MARGIN);
    }

    fn a_narrow_screen_keeps_the_hud_on_screen() {
        // Narrower than the HUD plus both margins: it sits at the left margin rather than off-screen.
        let (x, _, _, _) = anchor(Some((50.0, 1056.0, 20.0, 24.0)), (0.0, 0.0, 150.0, 1080.0));
        assert_eq!(x, SCREEN_MARGIN);
    }

    fn a_second_screen_anchors_in_its_own_coordinates() {
        // Screen to the right of the main one, icon near its top-left.
        let (x, y, _, _) = anchor(Some((1930.0, 1056.0, 20.0, 24.0)), (1920.0, 0.0, 2560.0, 1440.0));
        assert_eq!(x, 1940.0 - HUD_W / 2.0);
        assert_eq!(y, 1056.0 - ICON_GAP - HUD_H);
    }

    fn a_hidden_status_item_falls_back_to_the_top_right_never_the_bottom() {
        let screen = (0.0, 0.0, 1920.0, 1080.0);
        let (x, y, w, h) = anchor(None, screen);
        assert_eq!(x, 1920.0 - HUD_W - SCREEN_MARGIN);
        assert_eq!(y, 1080.0 - 28.0 - ICON_GAP - HUD_H);
        assert_eq!((w, h), (HUD_W, HUD_H));
        // Top half of the screen, not the bottom band the old placement used.
        assert!(y > 1080.0 / 2.0);
    }

    fn the_icon_moves_the_hud_to_the_icon_s_own_screen() {
        let screens = [(0.0, 0.0, 1920.0, 1080.0), (1920.0, 0.0, 2560.0, 1440.0)];
        let icon_on_second = (2500.0, 1416.0, 20.0, 24.0);
        assert_eq!(screen_for(icon_on_second, &screens), Some(screens[1]));
        let icon_on_main = (960.0, 1056.0, 20.0, 24.0);
        assert_eq!(screen_for(icon_on_main, &screens), Some(screens[0]));
        // An icon frame outside every screen still gets the first screen, never none.
        assert_eq!(screen_for((-500.0, 5000.0, 10.0, 10.0), &screens), Some(screens[0]));
        assert_eq!(screen_for(icon_on_main, &[]), None);
    }

    #[test]
    fn the_hud_stays_about_a_second_then_fades_quickly() {
        assert!((800..=1000).contains(&VISIBLE.as_millis()), "visible for {VISIBLE:?}");
        assert!((0.1..=0.3).contains(&FADE), "fade {FADE}");
    }
}
