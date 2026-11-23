use core_graphics::display::CGDisplay;
use ddc::{Ddc, DdcHost};
use ddc_macos::Monitor;
use serde::Serialize;

const BRIGHTNESS: u8 = 0x10;
const FEATURES: [u8; 3] = [BRIGHTNESS, 0x12, 0x62];

#[derive(Serialize)]
pub struct Display {
    id: String,
    name: String,
    levels: Vec<Level>,
}

#[derive(Clone, Copy, Serialize)]
pub struct Level {
    code: u8,
    /// None = the monitor did not report this setting. It is never shown as a number, and the
    /// panel offers no slider for it, so nothing can be written from an unknown value.
    percent: Option<u16>,
}

fn monitors() -> Result<Vec<Monitor>, String> {
    Monitor::enumerate()
        .map(|items| items.into_iter().filter(|d| !d.handle().is_builtin()).collect())
        .map_err(|e| format!("Couldn't find external monitors: {e}"))
}

fn id(d: CGDisplay) -> String {
    // The model name alone would send changes to the wrong display on identical monitors.
    format!("{}-{}-{}-{}", d.id, d.vendor_number(), d.model_number(), d.serial_number())
}

fn read(monitor: &mut Monitor, code: u8) -> Result<(u16, u16), String> {
    monitor.sleep();
    let value = monitor
        .get_vcp_feature(code)
        .map_err(|e| format!("The monitor didn't return this setting: {e}"))?;
    let (current, maximum) = (value.value(), value.maximum());
    if maximum == 0 || current > maximum {
        return Err("The monitor returned an invalid range.".into());
    }
    Ok((current, maximum))
}

fn level(code: u8, value: u16, max: u16) -> Level {
    Level { code, percent: Some(((u32::from(value) * 100 + u32::from(max) / 2) / u32::from(max)) as u16) }
}

/// A row per feature: the monitor's own reading, or an explicit "not reported".
fn row(code: u8, reading: Result<(u16, u16), String>) -> Level {
    match reading {
        Ok((value, max)) => level(code, value, max),
        Err(_) => Level { code, percent: None },
    }
}

fn from_percent(percent: u16, max: u16) -> u16 {
    ((u32::from(percent) * u32::from(max) + 50) / 100) as u16
}

fn write(monitor: &mut Monitor, code: u8, target: u16, max: u16) -> Result<Level, String> {
    // The DDC crate records the mandatory command interval but leaves waiting to the caller.
    monitor.sleep();
    monitor
        .set_vcp_feature(code, target)
        .map_err(|e| format!("The monitor didn't accept the change: {e}. Refresh and try again."))?;
    for _ in 0..2 {
        if let Ok((actual, reported_max)) = read(monitor, code) {
            if actual == target && reported_max == max {
                return Ok(level(code, actual, max));
            }
        }
    }
    Err("Change sent, but the monitor didn't confirm the requested value. Refresh before retrying.".into())
}

pub fn scan() -> Result<Vec<Display>, String> {
    let mut monitors = monitors()?;
    let active = CGDisplay::active_displays().map_err(|_| "Couldn't read the Mac's display list.")?;
    Ok(active.into_iter().map(CGDisplay::new).filter(|d| !d.is_builtin()).map(|d| {
        let monitor = monitors.iter_mut().find(|m| m.handle().id == d.id);
        let (name, levels) = match monitor {
            Some(m) => (m.description(), FEATURES.iter().map(|&code| row(code, read(m, code))).collect()),
            None => (format!("External display {:04x}:{:04x}", d.vendor_number(), d.model_number()), Vec::new()),
        };
        Display { id: id(d), name, levels }
    }).collect())
}

/// One monitor's write, from the value it reports to the value it confirms.
fn set_one(monitor: &mut Monitor, code: u8, percent: u16) -> Result<Level, String> {
    let (current, max) = read(monitor, code)?;
    let target = from_percent(percent, max);
    if target == current {
        return Ok(level(code, current, max));
    }
    write(monitor, code, target, max)
}

pub fn set(display: &str, code: u8, percent: u16) -> Result<Level, String> {
    if !FEATURES.contains(&code) || percent > 100 {
        return Err("Choose a supported setting between 0% and 100%.".into());
    }
    let mut monitor = monitors()?.into_iter().find(|m| id(m.handle()) == display)
        .ok_or("This monitor isn't responding. Reconnect it and refresh.")?;
    set_one(&mut monitor, code, percent)
}

#[derive(Clone, Serialize)]
pub struct All {
    changed: Vec<(String, Level)>,
    message: String,
    failed: bool,
}

/// A real or fake monitor: the id is captured with the handle so a write can never be credited
/// to another display.
trait Target {
    fn id(&self) -> String;
    fn apply(&mut self, code: u8, percent: u16) -> Result<Level, String>;
}

struct HardwareTarget {
    id: String,
    monitor: Monitor,
}

impl Target for HardwareTarget {
    fn id(&self) -> String { self.id.clone() }
    fn apply(&mut self, code: u8, percent: u16) -> Result<Level, String> {
        set_one(&mut self.monitor, code, percent)
    }
}

/// Every external display at once. Reports only what each target confirmed.
fn apply_all(targets: &mut [impl Target], code: u8, percent: u16) -> All {
    let mut changed = Vec::new();
    let mut failed = 0;
    for target in targets {
        match target.apply(code, percent) {
            Ok(value) => changed.push((target.id(), value)),
            Err(_) => failed += 1,
        }
    }
    let (message, error) = summary(changed.len(), failed, "displays");
    All { changed, message, failed: error }
}

pub fn set_all(code: u8, percent: u16) -> Result<All, String> {
    if !FEATURES.contains(&code) || percent > 100 {
        return Err("Choose a supported setting between 0% and 100%.".into());
    }
    let mut targets: Vec<HardwareTarget> = monitors()?.into_iter()
        .map(|m| HardwareTarget { id: id(m.handle()), monitor: m })
        .collect();
    Ok(apply_all(&mut targets, code, percent))
}

/// What the external displays report right now — the state a preset remembers.
/// A setting a monitor does not report is left out, so applying cannot invent a value.
pub fn snapshot() -> Result<Vec<(String, u8, u16)>, String> {
    let mut out = Vec::new();
    for mut monitor in monitors()? {
        let display = id(monitor.handle());
        for code in FEATURES {
            if let Ok((value, max)) = read(&mut monitor, code) {
                if let Some(percent) = level(code, value, max).percent {
                    out.push((display.clone(), code, percent));
                }
            }
        }
    }
    Ok(out)
}

/// One preset's settings, in order. Each one goes to the display that reported it.
fn apply_items(targets: &mut [impl Target], items: &[(String, u8, u16)]) -> All {
    let mut changed = Vec::new();
    let mut failed = 0;
    for (display, code, percent) in items {
        let target = targets.iter_mut().find(|t| t.id() == *display);
        if !FEATURES.contains(code) || *percent > 100 {
            failed += 1;
            continue;
        }
        match target {
            Some(target) => match target.apply(*code, *percent) {
                Ok(value) => changed.push((target.id(), value)),
                Err(_) => failed += 1,
            },
            // The display the preset was saved on is not here: say so, never guess another one.
            None => failed += 1,
        }
    }
    let message = match (changed.len(), failed) {
        (0, _) => "Nothing in this preset could be applied. Refresh to check the displays.".to_string(),
        (n, 0) => format!("Applied {n} settings."),
        (n, f) => format!("Applied {n} of {} settings. Refresh to check the rest.", n + f),
    };
    All { changed, message, failed: failed > 0 }
}

pub fn set_many(items: Vec<(String, u8, u16)>) -> Result<All, String> {
    let mut targets: Vec<HardwareTarget> = monitors()?.into_iter()
        .map(|m| HardwareTarget { id: id(m.handle()), monitor: m })
        .collect();
    Ok(apply_items(&mut targets, &items))
}

/// One sentence for a batch result, and whether it counts as a failure.
fn summary(changed: usize, failed: usize, what: &str) -> (String, bool) {
    match (changed, failed) {
        (0, 0) => (format!("No external {what} reported this control."), true),
        (_, 0) => (format!("Set on all {changed} {what} that reported it."), false),
        (count, _) => (format!("Set on {count} of {} {what}. Refresh to check the rest.", count + failed), true),
    }
}

#[derive(Clone, Serialize)]
pub struct Step {
    changed: Vec<(String, Level)>,
    message: String,
    failed: bool,
}

impl Step {
    pub fn message(&self) -> &str { &self.message }
}

pub fn step(brighter: bool) -> Result<Step, String> {
    let mut changed = Vec::new();
    let mut failed = 0;
    for mut monitor in monitors()? {
        let Ok((value, max)) = read(&mut monitor, BRIGHTNESS) else { continue };
        let delta = max.div_ceil(20);
        let target = if brighter { value.saturating_add(delta).min(max) } else { value.saturating_sub(delta) };
        let result = if value == target { Ok(level(BRIGHTNESS, value, max)) } else { write(&mut monitor, BRIGHTNESS, target, max) };
        match result {
            Ok(value) => changed.push((id(monitor.handle()), value)),
            Err(_) => failed += 1,
        }
    }
    let (message, error) = match (changed.len(), failed) {
        (0, 0) => ("No external monitor reported brightness control.".into(), true),
        (_, 0) => ("Brightness checked on all supported displays.".into(), false),
        (count, _) => (format!("Brightness confirmed on {count} of {} displays. Refresh to check the rest.", count + failed), true),
    };
    Ok(Step { changed, message, failed: error })
}

#[cfg(feature = "webview-check")]
pub fn verify_write(name: &str) -> Result<(u16, u16, u16), String> {
    let mut matching = monitors()?.into_iter().filter(|m| m.description() == name);
    let mut m = matching.next().ok_or("The named monitor was not found.")?;
    if matching.next().is_some() { return Err("Multiple monitors have the same name; no write attempted.".into()); }
    let (original, max) = read(&mut m, BRIGHTNESS)?;
    let step = max.div_ceil(20);
    let target = if max - original >= step { original + step } else { original - step };
    let changed = write(&mut m, BRIGHTNESS, target, max);
    // Always restore the exact raw value, even if readback of the test change failed.
    write(&mut m, BRIGHTNESS, original, max)
        .map_err(|e| format!("Could not verify restore to {original}/{max}: {e}"))?;
    changed?;
    Ok((original, target, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn scales_to_the_monitors_reported_range() {
        assert_eq!(from_percent(100, u16::MAX), u16::MAX);
        assert_eq!(from_percent(50, 255), 128);
        assert_eq!(level(BRIGHTNESS, 128, 255).percent, Some(50));
        assert_eq!(from_percent(0, 1), 0);
    }

    #[test]
    fn a_setting_the_monitor_did_not_report_is_never_a_number() {
        // The stub is a 0x62 read that failed: the panel must say "Not reported" and draw no
        // slider, because a slider showing 0 would be a value nobody read.
        let volume = row(0x62, Err("The monitor didn't return this setting: no reply".into()));
        assert_eq!(volume.code, 0x62);
        assert_eq!(volume.percent, None);
        // And no write can start from an unknown state: set() reads first and returns before
        // write() when the read fails, so set_vcp_feature is never reached.
        assert!(set("no monitor", 0x62, 0).is_err());
    }

    #[test]
    fn rejects_unknown_setting_without_accessing_hardware() {
        assert!(set("no monitor", 0x60, 50).is_err());
        assert!(set("no monitor", BRIGHTNESS, 101).is_err());
    }

    struct FakeTarget {
        id: String,
        max: u16,
        fail_read: bool,
        fail_confirm: bool,
        writes: Rc<RefCell<Vec<(String, u8, u16)>>>,
    }

    impl Target for FakeTarget {
        fn id(&self) -> String { self.id.clone() }
        fn apply(&mut self, code: u8, percent: u16) -> Result<Level, String> {
            if self.fail_read { return Err("read failed".into()); }
            let target = from_percent(percent, self.max);
            self.writes.borrow_mut().push((self.id.clone(), code, target));
            if self.fail_confirm { return Err("confirm failed".into()); }
            Ok(level(code, target, self.max))
        }
    }

    fn fake(id: &str, max: u16, read: bool, confirm: bool, writes: &Rc<RefCell<Vec<(String, u8, u16)>>>) -> FakeTarget {
        FakeTarget { id: id.into(), max, fail_read: !read, fail_confirm: !confirm, writes: Rc::clone(writes) }
    }

    #[test]
    fn two_displays_get_their_own_scaled_writes() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let mut targets = [
            fake("display-1-aaaa-bbbb-1111", 100, true, true, &writes),
            fake("display-2-cccc-dddd-2222", 255, true, true, &writes),
        ];
        let all = apply_all(&mut targets, BRIGHTNESS, 50);
        assert!(!all.failed);
        assert_eq!(all.changed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
                   vec!["display-1-aaaa-bbbb-1111", "display-2-cccc-dddd-2222"]);
        assert_eq!(all.changed[0].1.percent, Some(50));
        assert_eq!(all.changed[1].1.percent, Some(50));
        assert_eq!(*writes.borrow(), vec![
            ("display-1-aaaa-bbbb-1111".into(), BRIGHTNESS, 50),
            ("display-2-cccc-dddd-2222".into(), BRIGHTNESS, 128),
        ]);
    }

    #[test]
    fn a_display_that_fails_to_read_is_failed_not_changed() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let mut targets = [
            fake("unreadable", 100, false, true, &writes),
            fake("readable", 255, true, true, &writes),
        ];
        let all = apply_all(&mut targets, 0x12, 50);
        assert!(all.failed);
        assert_eq!(all.changed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["readable"]);
        assert_eq!(*writes.borrow(), vec![("readable".into(), 0x12, 128)]);
    }

    #[test]
    fn a_display_that_does_not_confirm_is_failed_not_changed() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let mut targets = [
            fake("unconfirmed", 100, true, false, &writes),
            fake("confirmed", 255, true, true, &writes),
        ];
        let all = apply_all(&mut targets, 0x62, 50);
        assert!(all.failed);
        assert_eq!(all.changed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["confirmed"]);
        assert_eq!(*writes.borrow(), vec![
            ("unconfirmed".into(), 0x62, 50),
            ("confirmed".into(), 0x62, 128),
        ]);
    }

    #[test]
    fn a_preset_writes_each_setting_to_the_display_that_reported_it() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let mut targets = [
            fake("display-1-aaaa-bbbb-1111", 100, true, true, &writes),
            fake("display-2-cccc-dddd-2222", 255, true, true, &writes),
        ];
        let items = [
            ("display-2-cccc-dddd-2222".to_string(), BRIGHTNESS, 20),
            ("display-1-aaaa-bbbb-1111".to_string(), 0x12, 60),
            ("display-2-cccc-dddd-2222".to_string(), 0x62, 80),
        ];
        let all = apply_items(&mut targets, &items);
        assert!(!all.failed, "{}", all.message);
        assert_eq!(all.changed.len(), 3);
        assert_eq!(*writes.borrow(), vec![
            ("display-2-cccc-dddd-2222".into(), BRIGHTNESS, 51),
            ("display-1-aaaa-bbbb-1111".into(), 0x12, 60),
            ("display-2-cccc-dddd-2222".into(), 0x62, 204),
        ]);
    }

    #[test]
    fn a_preset_naming_a_display_that_is_gone_is_failed_not_redirected() {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let mut targets = [fake("here", 100, true, true, &writes)];
        let items = [
            ("gone".to_string(), BRIGHTNESS, 40),
            ("here".to_string(), BRIGHTNESS, 40),
        ];
        let all = apply_items(&mut targets, &items);
        assert!(all.failed);
        assert_eq!(all.changed.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), vec!["here"]);
        assert_eq!(*writes.borrow(), vec![("here".into(), BRIGHTNESS, 40)]);
        assert!(all.message.contains("1 of 2"), "{}", all.message);
    }
}
