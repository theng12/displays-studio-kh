//! Per-display hotkeys: one chord pair per display, kept in a file beside the preferences.
//!
//! A chord is stored the way the global-shortcut plugin parses it ("Control+Shift+KeyB"), so what is
//! saved is exactly what gets registered — no second notation to drift.
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hotkey {
    pub display: String,
    /// true = brighter, false = dimmer.
    pub brighter: bool,
    pub chord: String,
}

/// Missing or unreadable file means "none set" — never an error a settings screen has to explain.
pub fn load(path: &Path) -> Vec<Hotkey> {
    std::fs::read_to_string(path).ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, list: &[Hotkey]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// One chord per display and direction; a chord belongs to one display and direction only, so two
/// displays can never answer the same key press.
pub fn upsert(list: &mut Vec<Hotkey>, hotkey: Hotkey) {
    list.retain(|h| !(h.display == hotkey.display && h.brighter == hotkey.brighter) && h.chord != hotkey.chord);
    list.push(hotkey);
}

pub fn remove(list: &mut Vec<Hotkey>, display: &str, brighter: bool) -> bool {
    let before = list.len();
    list.retain(|h| !(h.display == display && h.brighter == brighter));
    list.len() != before
}

/// A chord must parse with the same parser that will register it, and must carry a modifier — a bare
/// key would swallow that key system-wide.
pub fn clean_chord(chord: &str) -> Result<String, String> {
    let chord = chord.trim();
    if chord.is_empty() {
        return Err("Press the keys you want first.".into());
    }
    if !chord.contains('+') {
        return Err("A shortcut needs a modifier, like Control or Shift, plus a key.".into());
    }
    tauri_plugin_global_shortcut::Shortcut::from_str(chord)
        .map_err(|_| "macOS doesn't recognise that combination. Try another one.".to_string())?;
    Ok(chord.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hotkey(display: &str, brighter: bool, chord: &str) -> Hotkey {
        Hotkey { display: display.into(), brighter, chord: chord.into() }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("dskh-hotkeys-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn round_trips_through_the_file() {
        let path = temp("roundtrip");
        let list = vec![hotkey("d1", true, "Control+Shift+KeyB"), hotkey("d1", false, "Control+Shift+KeyD")];
        save(&path, &list).unwrap();
        assert_eq!(load(&path), list);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_missing_or_broken_file_reads_as_none_set() {
        let path = temp("broken");
        assert!(load(&path).is_empty());
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn one_chord_belongs_to_one_display_and_direction() {
        let mut list = vec![hotkey("d1", true, "Control+Shift+KeyB")];
        upsert(&mut list, hotkey("d1", true, "Control+Shift+KeyN"));      // replaces in place
        assert_eq!(list, vec![hotkey("d1", true, "Control+Shift+KeyN")]);
        upsert(&mut list, hotkey("d2", true, "Control+Shift+KeyN"));      // steals the chord
        assert_eq!(list, vec![hotkey("d2", true, "Control+Shift+KeyN")]);
        upsert(&mut list, hotkey("d2", false, "Control+Alt+KeyN"));       // other direction is free
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn removing_reports_whether_anything_went() {
        let mut list = vec![hotkey("d1", true, "Control+Shift+KeyB")];
        assert!(remove(&mut list, "d1", true));
        assert!(list.is_empty());
        assert!(!remove(&mut list, "d1", true));
    }

    #[test]
    fn chords_need_a_modifier_and_must_parse() {
        assert_eq!(clean_chord(" Control+Shift+KeyB ").unwrap(), "Control+Shift+KeyB");
        assert!(clean_chord("").is_err());
        assert!(clean_chord("KeyB").is_err());
        assert!(clean_chord("Control+NotAKey").is_err());
    }
}
