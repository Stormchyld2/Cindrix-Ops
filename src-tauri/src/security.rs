//! Tauri side of Security Operations (core/src/security_api.rs, docs/44). Thin on purpose: every rule lives in
//! `core/`, where it is tested. These commands are host-only. None of them is in the share allowlist, and
//! `core/tests/security_rules.rs` fails if one ever is, or if this list and `SECURITY_COMMANDS` disagree.

use cindrix_core::security_api as api;
use serde_json::Value;

fn user_root() -> std::path::PathBuf {
    crate::root().join("user")
}

fn json(text: Result<String, String>) -> Result<Value, String> {
    serde_json::from_str(&text?).map_err(|_| "The security answer could not be read.".to_string())
}

#[tauri::command]
pub fn security_overview(offset_minutes: i64) -> Result<Value, String> {
    json(api::overview(&user_root(), offset_minutes))
}

#[tauri::command]
pub fn security_watch_strip(site: String, offset_minutes: i64) -> Result<Value, String> {
    json(api::watch_strip(&user_root(), &site, offset_minutes))
}

#[tauri::command]
pub fn security_site_save(id: String, name: String, tz: String, map_id: String, notes: String) -> Result<String, String> {
    api::site_save(&user_root(), &id, &name, &tz, &map_id, &notes)
}

#[tauri::command]
pub fn security_posture_set(site: String, posture: String, by: String, reason: String, review_hours: u32, offset_minutes: i64) -> Result<(), String> {
    api::posture_set(&user_root(), &site, &posture, &by, &reason, review_hours, offset_minutes)
}

#[tauri::command]
pub fn security_incident_open(site: String, title: String, priority: u8, exercise: bool, by: String, offset_minutes: i64) -> Result<String, String> {
    api::incident_open(&user_root(), &site, &title, priority, exercise, &by, offset_minutes)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn security_incident_entry(incident: String, kind: String, arg1: String, text: String, source: String, by: String, supersedes: String, happened_at: String, offset_minutes: i64) -> Result<String, String> {
    api::incident_entry(&user_root(), &incident, &kind, &arg1, &text, &source, &by, &supersedes, &happened_at, offset_minutes)
}

#[tauri::command]
pub fn security_incident_list() -> Result<Value, String> {
    json(api::incident_list(&user_root()))
}
