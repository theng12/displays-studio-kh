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
    NSAnimationContext, NSColor, NSFont, NSLevelIndicator, NSLevelIndicatorStyle, NSPanel, NSTextField,
    NSVisualEffectBlendingMode, NSWorkspace, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use tauri::AppHandle;

use crate::brightness_keys::TargetPath;

pub(crate) const HUD_W: f64 = 172.0;
pub(crate) const HUD_H: f64 = 44.0;
/// Above the bottom edge, clear of the Dock's usual place — the system HUD sits here too.
pub(crate) const HUD_BOTTOM_MARGIN: f64 = 96.0;
/// Fully visible for this long after a press, then the fade below, then hidden.
pub(crate) const VISIBLE: Duration = Duration::from_millis(900);
pub(crate) const FADE: f64 = 0.2;

/// The panel's frame in AppKit coordinates (bottom-left origin) for a display whose CoreGraphics
/// bounds (top-left origin) and the main display's height are given.
pub(crate) fn frame_for(cg: (f64, f64, f64, f64), main_height: f64) -> (f64, f64, f64, f64) {
    let (x, y, w, h) = cg;
    (x + (w - HUD_W) / 2.0, main_height - y - h + HUD_BOTTOM_MARGIN, HUD_W, HUD_H)
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
static SELFTEST: [AtomicBool; 6] = [const { AtomicBool::new(false) }; 6];

/// The app in front before the HUD was shown, so the self-test can prove it did not change.
static FRONT_BEFORE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

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

/// Shows the HUD on `display` at `percent`. Never activates the app and never takes focus.
pub(crate) fn show(app: &AppHandle, display: u32, percent: u16) {
    let bounds = core_graphics::display::CGDisplay::new(display).bounds();
    let main_height = core_graphics::display::CGDisplay::main().bounds().size.height;
    show_at(
        app,
        frame_for((bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height), main_height),
        percent,
    );
}

fn show_at(app: &AppHandle, frame: (f64, f64, f64, f64), percent: u16) {
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = app.run_on_main_thread(move || place_and_show(frame, percent));
    // One short-lived thread per press: sleep, then fade on the main thread. No polling, no timer
    // that outlives the press.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(VISIBLE);
        let _ = handle.run_on_main_thread(move || {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            HUD.with(|slot| {
                if let Some(hud) = slot.borrow().as_ref() {
                    fade_and_hide(&hud.panel);
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
        hud.panel.setFrame_display(NSRect::new(NSPoint::new(x, y), NSSize::new(w, h)), false);
        hud.bar.setDoubleValue(f64::from(percent));
        hud.label.setStringValue(&NSString::from_str(&format!("{percent}%")));
        hud.panel.setAlphaValue(1.0);
        hud.panel.orderFrontRegardless();
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

/// Fades out over `FADE` seconds and hides; the panel is kept for the next press.
fn fade_and_hide(panel: &Retained<NSPanel>) {
    let fading = panel.clone();
    let hiding = panel.clone();
    unsafe {
        NSAnimationContext::runAnimationGroup_completionHandler(
            &block2::RcBlock::new(move |context: NonNull<NSAnimationContext>| {
                context.as_ref().setDuration(FADE);
                let animator: Retained<objc2::runtime::AnyObject> = msg_send![&*fading, animator];
                let _: () = msg_send![&animator, setAlphaValue: 0.0f64];
            }),
            Some(&block2::RcBlock::new(move || hiding.orderOut(None))),
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
    let (x, y, w, h) = frame_for((0.0, 0.0, 1920.0, 1080.0), 1080.0);
    let frame_ok = (x - (1920.0 - HUD_W) / 2.0).abs() < 0.5
        && (y - HUD_BOTTOM_MARGIN).abs() < 0.5
        && w == HUD_W && h == HUD_H;
    println!("HUD SELFTEST: frame on a 1920x1080 display = {x:.0},{y:.0} {w:.0}x{h:.0} — bottom-centre, {HUD_BOTTOM_MARGIN:.0} above the bottom edge: {}", if frame_ok { "ok" } else { "FAIL" });

    let showing = app.clone();
    let reading = app.clone();
    let finishing = app.clone();
    std::thread::spawn(move || {
        // The app finishes launching first: anything from startup is not the HUD's doing.
        std::thread::sleep(Duration::from_millis(1200));
        let _ = showing.run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else { return };
            *FRONT_BEFORE.lock().unwrap() = frontmost_bundle(mtm);
            println!("HUD SELFTEST: frontmost before the HUD = {:?}", FRONT_BEFORE.lock().unwrap());
            show_at(&app, (-6000.0, -6000.0, HUD_W, HUD_H), 50);
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
        std::thread::sleep(VISIBLE + Duration::from_millis(500));
        let _ = finishing.run_on_main_thread(move || {
            let hidden = HUD.with(|slot| slot.borrow().as_ref().map(|hud| !hud.panel.isVisible()).unwrap_or(false));
            let ok = |index: usize| SELFTEST[index].load(Ordering::SeqCst);
            let focus_ok = !ok(4) || !ok(3);
            let pass = frame_ok && ok(0) && !ok(1) && !ok(2) && focus_ok && hidden;
            println!("HUD SELFTEST: hidden after {} ms, fade {:.0} ms — visible_now={}", VISIBLE.as_millis() + 500, FADE * 1000.0, !hidden);
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
    fn the_hud_sits_bottom_centre_on_each_display() {
        let centre = |x: f64, w: f64| x + (w - HUD_W) / 2.0;
        assert_eq!(frame_for((0.0, 0.0, 1920.0, 1080.0), 1080.0), (centre(0.0, 1920.0), HUD_BOTTOM_MARGIN, HUD_W, HUD_H));
        // A taller second display to the right still puts the HUD on its own bottom edge.
        assert_eq!(
            frame_for((1920.0, 0.0, 2560.0, 1440.0), 1080.0),
            (centre(1920.0, 2560.0), 1080.0 - 1440.0 + HUD_BOTTOM_MARGIN, HUD_W, HUD_H)
        );
        // A display above the main one converts back into AppKit's upward-positive coordinates.
        assert_eq!(
            frame_for((0.0, -1080.0, 1920.0, 1080.0), 1080.0),
            (centre(0.0, 1920.0), 1080.0 + 1080.0 - 1080.0 + HUD_BOTTOM_MARGIN, HUD_W, HUD_H)
        );
    }

    #[test]
    fn the_hud_stays_about_a_second_then_fades_quickly() {
        assert!((800..=1000).contains(&VISIBLE.as_millis()), "visible for {VISIBLE:?}");
        assert!((0.1..=0.3).contains(&FADE), "fade {FADE}");
    }
}
