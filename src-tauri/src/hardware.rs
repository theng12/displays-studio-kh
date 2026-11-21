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

pub fn set(display: &str, code: u8, percent: u16) -> Result<Level, String> {
    if !FEATURES.contains(&code) || percent > 100 {
        return Err("Choose a supported setting between 0% and 100%.".into());
    }
    let mut monitor = monitors()?.into_iter().find(|m| id(m.handle()) == display)
        .ok_or("This monitor isn't responding. Reconnect it and refresh.")?;
    let (current, max) = read(&mut monitor, code)?;
    let target = from_percent(percent, max);
    if target == current {
        return Ok(level(code, current, max));
    }
    write(&mut monitor, code, target, max)
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
}
