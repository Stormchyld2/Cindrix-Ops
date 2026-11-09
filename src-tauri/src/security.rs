//! Tauri side of Security Operations (core/src/security_api.rs, docs/44). Thin on purpose: every rule lives in
//! `core/`, where it is tested. These commands are host-only. None of them is in the share allowlist, and
//! `core/tests/security_rules.rs` fails if one ever is, or if this list and `SECURITY_COMMANDS` disagree.

use cindrix_core::security_api as api;
use cindrix_core::security_comms::Channel;
use cindrix_core::security_site::Route;
use serde::Deserialize;
use serde_json::Value;

fn user_root() -> std::path::PathBuf {
    crate::root().join("user")
}

fn json(text: Result<String, String>) -> Result<Value, String> {
    serde_json::from_str(&text?).map_err(|_| "The security answer could not be read.".to_string())
}

#[tauri::command]
pub fn security_overview(offset_minutes: i64) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::overview(&user_root(), offset_minutes))
}

#[tauri::command]
pub fn security_watch_strip(site: String, offset_minutes: i64) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::watch_strip(&user_root(), &site, offset_minutes))
}

#[tauri::command]
pub fn security_site_save(id: String, name: String, tz: String, map_id: String, notes: String) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::site_save(&user_root(), &id, &name, &tz, &map_id, &notes)
}

#[tauri::command]
pub fn security_posture_set(site: String, posture: String, by: String, reason: String, review_hours: u32, offset_minutes: i64) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::posture_set(&user_root(), &site, &posture, &by, &reason, review_hours, offset_minutes)
}

#[tauri::command]
pub fn security_incident_open(site: String, title: String, priority: u8, exercise: bool, by: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::incident_open(&user_root(), &site, &title, priority, exercise, &by, offset_minutes)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn security_incident_entry(incident: String, kind: String, arg1: String, text: String, source: String, by: String, supersedes: String, happened_at: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::incident_entry(&user_root(), &incident, &kind, &arg1, &text, &source, &by, &supersedes, &happened_at, offset_minutes)
}

#[tauri::command]
pub fn security_incident_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::incident_list(&user_root()))
}

#[tauri::command]
pub fn security_watch_list(site: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::watch_list(&user_root(), &site))
}

#[tauri::command]
pub fn security_watch_draft(site: String, people: Vec<String>, start_local: String, hours: u32, count: u32, offset_minutes: i64) -> Result<usize, String> {
    crate::seclock::require_open()?;
    api::watch_draft(&user_root(), &site, &people, &start_local, hours, count, offset_minutes)
}

#[tauri::command]
pub fn security_watch_add(site: String, person: String, role: String, start_local: String, end_local: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::watch_add(&user_root(), &site, &person, &role, &start_local, &end_local, offset_minutes)
}

#[tauri::command]
pub fn security_watch_move(id: String, to: String) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::watch_move(&user_root(), &id, &to)
}

#[tauri::command]
pub fn security_watch_ack(id: String, at_local: String, offset_minutes: i64) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::watch_ack(&user_root(), &id, &at_local, offset_minutes)
}

#[tauri::command]
pub fn security_watch_discard(id: String) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::watch_discard(&user_root(), &id)
}

#[tauri::command]
pub fn security_watch_schedule_all(site: String) -> Result<usize, String> {
    crate::seclock::require_open()?;
    api::watch_schedule_all(&user_root(), &site)
}

#[tauri::command]
pub fn security_pocket_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::pocket_list(&user_root()))
}

#[tauri::command]
pub fn security_pocket_accept(name: String, allow_moved: Option<bool>) -> Result<usize, String> {
    crate::seclock::require_open()?;
    api::pocket_accept(&user_root(), &name, allow_moved.unwrap_or(false))
}

#[tauri::command]
pub fn security_pocket_sheet(name: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::pocket_sheet(&user_root(), &name))
}

#[tauri::command]
pub fn security_pocket_set_aside(name: String) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::pocket_set_aside(&user_root(), &name)
}

#[tauri::command]
pub fn security_outing_list(site: String, offset_minutes: i64) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::outing_list(&user_root(), &site, offset_minutes))
}

#[tauri::command]
pub fn security_outing_start(site: String, kind: String, people: String, task: String, back_local: String, next_local: String, method: String, by: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::outing_start(&user_root(), &site, &kind, &people, &task, &back_local, &next_local, &method, &by, offset_minutes)
}

#[tauri::command]
pub fn security_outing_event(outing: String, ev: String, a1: String, a2: String, text: String, back_local: String, next_local: String, by: String, offset_minutes: i64) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::outing_event(&user_root(), &outing, &ev, &a1, &a2, &text, &back_local, &next_local, &by, offset_minutes)
}

#[tauri::command]
pub fn security_comms_list(site: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::comms_list(&user_root(), &site))
}

#[tauri::command]
pub fn security_comms_add(site: String, category: String, station: String, event: String, action: String, status: String, closes: String, author: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::comms_add(&user_root(), &site, &category, &station, &event, &action, &status, &closes, &author, offset_minutes)
}

#[tauri::command]
pub fn security_coverage_list(site: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::coverage_list(&user_root(), &site))
}

#[tauri::command]
pub fn security_coverage_add(site: String, place: String, p_in: String, p_out: String, a_in: String, a_out: String, by: String, note: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::coverage_add(&user_root(), &site, &place, &p_in, &p_out, &a_in, &a_out, &by, &note, offset_minutes)
}

#[tauri::command]
pub fn security_channels_get(site: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::channels_get(&user_root(), &site))
}

#[derive(Deserialize)]
pub struct ChannelDto {
    channel: String,
    code: String,
    role: String,
    purpose: String,
}

#[tauri::command]
pub fn security_channels_save(site: String, channels: Vec<ChannelDto>) -> Result<(), String> {
    crate::seclock::require_open()?;
    let list: Vec<Channel> = channels.into_iter().map(|c| Channel { site: site.clone(), channel: c.channel, code: c.code, role: c.role, purpose: c.purpose }).collect();
    api::channels_save(&user_root(), &site, &list)
}

#[tauri::command]
pub fn security_training_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::training_list(&user_root()))
}

#[tauri::command]
pub fn security_training_record(person: String, skill: u8, result: String, by: String, note: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::training_record(&user_root(), &person, skill, &result, &by, &note, offset_minutes)
}


#[tauri::command]
pub fn security_assets_list(site: String, offset_minutes: i64) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::assets_list(&user_root(), &site, offset_minutes))
}

#[tauri::command]
pub fn security_asset_save(site: String, id: String, kind: String, name: String, every_days: u32, notes: String) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::asset_save(&user_root(), &site, &id, &kind, &name, every_days, &notes)
}

#[tauri::command]
pub fn security_asset_inspect(asset: String, result: String, by: String, note: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::asset_inspect(&user_root(), &asset, &result, &by, &note, offset_minutes)
}

#[derive(Deserialize)]
pub struct RouteDto {
    name: String,
    checkpoints: Vec<String>,
}

#[tauri::command]
pub fn security_routes_get(site: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::routes_get(&user_root(), &site))
}

#[tauri::command]
pub fn security_routes_save(site: String, routes: Vec<RouteDto>) -> Result<(), String> {
    crate::seclock::require_open()?;
    let list: Vec<Route> = routes.into_iter().map(|r| Route { site: site.clone(), name: r.name, checkpoints: r.checkpoints }).collect();
    api::routes_save(&user_root(), &site, &list)
}

#[tauri::command]
pub fn security_availability_list(offset_minutes: i64) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::availability_list(&user_root(), offset_minutes))
}

#[tauri::command]
pub fn security_availability_set(person: String, status: String, until_local: String, reason: String, by: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::availability_set(&user_root(), &person, &status, &until_local, &reason, &by, offset_minutes)
}

#[tauri::command]
pub fn security_plans_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::plans_list(&user_root()))
}

#[tauri::command]
pub fn security_plan_read(id: String, revision: u32) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::plan_read(&user_root(), &id, revision))
}

#[tauri::command]
pub fn security_plan_save(id: String, text: String) -> Result<u32, String> {
    crate::seclock::require_open()?;
    api::plan_save(&user_root(), &id, &text)
}

#[tauri::command]
pub fn security_plan_activate(incident: String, plan: String, revision: u32, by: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::plan_activate(&user_root(), &incident, &plan, revision, &by, offset_minutes)
}

#[tauri::command]
pub fn security_weather_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(Ok(api::weather_list()))
}

#[tauri::command]
pub fn security_weather_set(site: String, call_sign: String) -> Result<(), String> {
    crate::seclock::require_open()?;
    api::weather_set(&user_root(), &site, &call_sign)
}

#[tauri::command]
pub fn security_export_records(offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::export_records(&user_root(), offset_minutes)
}

#[derive(Deserialize)]
pub struct RuleDto {
    route: String,
    every_hours: u32,
    from: String,
    to: String,
}

#[tauri::command]
pub fn security_schedule_get(site: String) -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::schedule_get(&user_root(), &site))
}

#[tauri::command]
pub fn security_schedule_save(site: String, rules: Vec<RuleDto>) -> Result<(), String> {
    crate::seclock::require_open()?;
    let list: Vec<(String, u32, String, String)> = rules.into_iter().map(|r| (r.route, r.every_hours, r.from, r.to)).collect();
    api::schedule_save(&user_root(), &site, &list)
}

#[tauri::command]
pub fn security_tracks_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::tracks_list(&user_root()))
}

#[tauri::command]
pub fn security_track_export(id: String) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::track_export(&user_root(), &id)
}

#[tauri::command]
pub fn security_equipment_list() -> Result<Value, String> {
    crate::seclock::require_open()?;
    json(api::equipment_list(&user_root()))
}

#[tauri::command]
pub fn security_equipment_add(uid: String, ev: String, kind: String, site: String, test_days: u32, by: String, note: String, offset_minutes: i64) -> Result<String, String> {
    crate::seclock::require_open()?;
    api::equipment_add(&user_root(), &uid, &ev, &kind, &site, test_days, &by, &note, offset_minutes)
}
