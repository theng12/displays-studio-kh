//! Saved presets: a named set of per-display settings, kept in a file beside the preferences.
//!
//! A preset remembers what the monitors actually reported when it was saved — a setting a monitor
//! did not report is never written into it, so applying one can never invent a value.
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub display: String,
    pub code: u8,
    pub percent: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub entries: Vec<Entry>,
}

/// A missing or unreadable file means "no presets yet" — never an error the panel has to explain.
pub fn load(path: &Path) -> Vec<Preset> {
    std::fs::read_to_string(path).ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Written to a temporary file and renamed, so a crash mid-write cannot leave half a preset list.
pub fn save(path: &Path, presets: &[Preset]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(presets).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// One preset per name: saving under a name that exists replaces it where it sits.
pub fn upsert(presets: &mut Vec<Preset>, preset: Preset) {
    match presets.iter_mut().find(|p| p.name == preset.name) {
        Some(existing) => *existing = preset,
        None => presets.push(preset),
    }
}

pub fn remove(presets: &mut Vec<Preset>, name: &str) -> bool {
    let before = presets.len();
    presets.retain(|p| p.name != name);
    presets.len() != before
}

/// The name the dropdown will show: trimmed, present, at most 40 characters.
pub fn clean_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the preset a name first.".into());
    }
    if name.chars().count() > 40 {
        return Err("A preset name is at most 40 characters.".into());
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str, percent: u16) -> Preset {
        Preset { name: name.into(), entries: vec![Entry { display: "d1".into(), code: 16, percent }] }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("dskh-presets-{}-{name}.json", std::process::id()))
    }

    #[test]
    fn round_trips_through_the_file() {
        let path = temp("roundtrip");
        let list = vec![preset("Evening", 20), preset("Day", 80)];
        save(&path, &list).unwrap();
        assert_eq!(load(&path), list);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_missing_or_broken_file_reads_as_no_presets() {
        let path = temp("broken");
        assert!(load(&path).is_empty());
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path).is_empty());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn saving_a_name_again_replaces_it_in_place() {
        let mut list = vec![preset("Evening", 20), preset("Day", 80)];
        upsert(&mut list, preset("Evening", 35));
        assert_eq!(list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Evening", "Day"]);
        assert_eq!(list[0].entries[0].percent, 35);
    }

    #[test]
    fn removing_reports_whether_anything_went() {
        let mut list = vec![preset("Evening", 20)];
        assert!(remove(&mut list, "Evening"));
        assert!(list.is_empty());
        assert!(!remove(&mut list, "Evening"));
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(clean_name("  Evening ").unwrap(), "Evening");
        assert!(clean_name("   ").is_err());
        assert!(clean_name(&"x".repeat(41)).is_err());
        assert!(clean_name(&"x".repeat(40)).is_ok());
    }
}
