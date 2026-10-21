//! The program's settings: one small JSON file, `user/settings.json`, in the drive's user folder.
//! The Settings screen (src/lib/settings.svelte.ts) owns the list of settings and their defaults.
//! This side stores the file and reads the few settings the program itself acts on (what the
//! assistant searches, whether the library server restarts by itself, and so on). A setting that
//! is missing or of the wrong kind falls back to the default given here, which must match the
//! screen's default.

use super::root;
use cindrix_core::fsio;
use serde_json::Value;
use std::path::PathBuf;

fn file() -> PathBuf {
    root().join("user").join("settings.json")
}

const MAX_BYTES: usize = 200_000;

/// The settings as a JSON object. Missing, damaged or oversized files read as no settings.
fn load() -> Value {
    match std::fs::read_to_string(file()) {
        Ok(s) if s.len() <= MAX_BYTES => match serde_json::from_str::<Value>(&s) {
            Ok(v) if v.is_object() => v,
            _ => Value::Object(Default::default()),
        },
        _ => Value::Object(Default::default()),
    }
}

pub fn flag(key: &str, default: bool) -> bool {
    load().get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn num_in(v: &Value, key: &str, default: i64, lo: i64, hi: i64) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(default).clamp(lo, hi)
}

/// A whole number, kept between `lo` and `hi`.
pub fn number(key: &str, default: i64, lo: i64, hi: i64) -> i64 {
    num_in(&load(), key, default, lo, hi)
}

/// When stock turns "check" or "act now" (Settings, Plan). Defaults: 7 days, 14 days, 3 months.
pub fn thresholds() -> cindrix_core::inventory::Thresholds {
    let v = load();
    let act = num_in(&v, "planActDays", 7, 1, 60);
    // "check" can never come before "act now"
    let check = num_in(&v, "planCheckDays", 14, 2, 120).max(act);
    cindrix_core::inventory::Thresholds { act_days: act as f64, check_days: check as f64, check_months: num_in(&v, "planExpiryMonths", 3, 0, 24) as i32 }
}

/// A word, which must be one of `allowed` (otherwise the default).
pub fn choice(key: &str, default: &str, allowed: &[&str]) -> String {
    match load().get(key).and_then(|v| v.as_str()) {
        Some(s) if allowed.contains(&s) => s.to_string(),
        _ => default.to_string(),
    }
}

/// A list of folders. Only folders that exist are returned.
pub fn folders(key: &str) -> Vec<PathBuf> {
    load()
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim()).filter(|s| !s.is_empty()).map(PathBuf::from).filter(|p| p.is_dir()).take(30).collect())
        .unwrap_or_default()
}

/// The settings file's text, or "{}".
#[tauri::command]
pub fn settings_load() -> String {
    load().to_string()
}

/// Keep the settings. Must be a JSON object.
#[tauri::command]
pub fn settings_save(json: String) -> Result<(), String> {
    if json.len() > MAX_BYTES {
        return Err("The settings are too large to save.".into());
    }
    match serde_json::from_str::<Value>(&json) {
        Ok(v) if v.is_object() => fsio::write_atomic(&file(), json.as_bytes()).map_err(|e| e.to_string()),
        _ => Err("That is not a settings list.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_fall_back_to_the_default() {
        // no file at the (unset) root in a test run: every reader gives its default
        assert!(flag("nothing", true));
        assert!(!flag("nothing", false));
        assert_eq!(number("nothing", 3, 1, 6), 3);
        assert_eq!(number("nothing", 30, 1, 6), 6, "the default itself is kept in range");
        assert_eq!(choice("nothing", "normal", &["short", "normal", "long"]), "normal");
        assert!(folders("nothing").is_empty());
        assert_eq!(thresholds(), cindrix_core::inventory::Thresholds::default());
    }

    #[test]
    fn numbers_are_kept_in_range() {
        let v: Value = serde_json::from_str(r#"{"a": 500, "b": -4, "c": "x"}"#).unwrap();
        assert_eq!(num_in(&v, "a", 7, 1, 60), 60);
        assert_eq!(num_in(&v, "b", 7, 1, 60), 1);
        assert_eq!(num_in(&v, "c", 7, 1, 60), 7);
        assert_eq!(num_in(&v, "missing", 7, 1, 60), 7);
    }
}
