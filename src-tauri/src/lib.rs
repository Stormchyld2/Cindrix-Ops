use cindrix_core::inventory::{self, Item, Status};
use cindrix_core::userdata::{self, UserDir};
use cindrix_core::{calendar, cards, cascade, pipeline};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The Cindrix root folder. Portable: the folder the program sits in.
/// Set CINDRIX_ROOT to point elsewhere (used in development).
fn root() -> PathBuf {
    if let Ok(p) = std::env::var("CINDRIX_ROOT") {
        return PathBuf::from(p);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn content_dir() -> PathBuf {
    let r = root();
    let here = r.join("content");
    if here.exists() { here } else { r.join("../../content") }
}

#[derive(Serialize)]
struct CardDto {
    id: String,
    title: String,
    kind: String,
    status: String,
    reviewer: String,
    reviewed: String,
    body: String,
}

#[derive(Serialize)]
struct ReplyDto {
    kind: &'static str,
    text: String,
    sources: Vec<String>,
    card: Option<String>,
}

#[tauri::command]
fn list_cards() -> Vec<CardDto> {
    cards::load_dir(&content_dir().join("cards"))
        .into_iter()
        .map(|c| CardDto { id: c.id, title: c.title, kind: c.kind, status: c.status, reviewer: c.reviewer, reviewed: c.reviewed, body: c.body })
        .collect()
}

#[tauri::command]
fn ask(question: String) -> ReplyDto {
    let lib = content_dir().join("library-sample");
    // NoModel until llama.cpp is connected.
    let r = pipeline::answer(&question, &lib, &pipeline::NoModel);
    ReplyDto {
        kind: match r.kind {
            pipeline::Kind::Refused => "Refused",
            pipeline::Kind::NotFound => "NotFound",
            pipeline::Kind::Answered => "Answered",
            pipeline::Kind::Replaced => "Replaced",
        },
        text: r.text,
        sources: r.sources,
        card: r.card,
    }
}

fn user() -> UserDir {
    UserDir::new(&root().join("user"))
}

#[derive(Serialize, Deserialize, Clone)]
struct ItemDto {
    id: String,
    category: String,
    name: String,
    location: String,
    quantity: f64,
    unit: String,
    use_per_day: f64,
    use_by: String,
    notes: String,
    /// "ok", "check" or "act" (filled in when listing)
    #[serde(default)]
    status: String,
    #[serde(default)]
    days_left: Option<f64>,
}

impl From<ItemDto> for Item {
    fn from(d: ItemDto) -> Item {
        Item { id: d.id, category: d.category, name: d.name, location: d.location, quantity: d.quantity,
               unit: d.unit, use_per_day: d.use_per_day, use_by: d.use_by, notes: d.notes }
    }
}

fn to_dto(i: &Item) -> ItemDto {
    let (y, m) = calendar::today_ym();
    ItemDto {
        id: i.id.clone(), category: i.category.clone(), name: i.name.clone(), location: i.location.clone(),
        quantity: i.quantity, unit: i.unit.clone(), use_per_day: i.use_per_day, use_by: i.use_by.clone(), notes: i.notes.clone(),
        status: match inventory::status(i, y, m) { Status::Ok => "ok", Status::Check => "check", Status::ActNow => "act" }.to_string(),
        days_left: inventory::days_left(i),
    }
}

#[tauri::command]
fn inventory_list() -> Result<Vec<ItemDto>, String> {
    Ok(user().load_inventory()?.iter().map(to_dto).collect())
}

/// Create (empty id) or replace an item.
#[tauri::command]
fn inventory_save_item(item: ItemDto) -> Result<Vec<ItemDto>, String> {
    let u = user();
    let mut items = u.load_inventory()?;
    let mut it: Item = item.into();
    if it.id.is_empty() {
        it.id = userdata::next_id(&items);
        items.push(it);
    } else if let Some(slot) = items.iter_mut().find(|x| x.id == it.id) {
        *slot = it;
    } else {
        items.push(it);
    }
    u.save_inventory(&items)?;
    Ok(items.iter().map(to_dto).collect())
}

#[tauri::command]
fn inventory_delete(ids: Vec<String>) -> Result<Vec<ItemDto>, String> {
    let u = user();
    let items = inventory::delete_ids(u.load_inventory()?, &ids);
    u.save_inventory(&items)?;
    Ok(items.iter().map(to_dto).collect())
}

#[derive(Serialize)]
struct RunsOutDto {
    name: String,
    location: String,
    days: f64,
}

#[derive(Serialize)]
struct ExpiringDto {
    name: String,
    months: i32,
}

#[derive(Serialize)]
struct CascadeDto {
    runs_out: Vec<RunsOutDto>,
    expiring: Vec<ExpiringDto>,
    water_days: Option<f64>,
    food_days: Option<f64>,
}

#[tauri::command]
fn what_fails_next() -> Result<CascadeDto, String> {
    let items = user().load_inventory()?;
    let (y, m) = calendar::today_ym();
    Ok(CascadeDto {
        runs_out: cascade::runs_out_first(&items).into_iter().map(|r| RunsOutDto { name: r.name, location: r.location, days: r.days }).collect(),
        expiring: cascade::expiring(&items, y, m, 3).into_iter().map(|e| ExpiringDto { name: e.name, months: e.months }).collect(),
        water_days: cascade::category_days(&items, "Water"),
        food_days: cascade::category_days(&items, "Food"),
    })
}

#[derive(Serialize)]
struct FamilyStatus {
    exists: bool,
    locked: bool,
}

#[tauri::command]
fn family_status() -> Result<FamilyStatus, String> {
    Ok(match user().read_family()? {
        None => FamilyStatus { exists: false, locked: false },
        Some(t) => FamilyStatus { exists: true, locked: cindrix_crypto::is_locked(&t) },
    })
}

/// Returns the family JSON text. Needs the passphrase if the file is locked.
#[tauri::command]
fn family_load(passphrase: Option<String>) -> Result<String, String> {
    match user().read_family()? {
        None => Ok("{}".to_string()),
        Some(t) if cindrix_crypto::is_locked(&t) => match passphrase {
            Some(p) => cindrix_crypto::unlock(&t, &p).map_err(|e| e.to_string()),
            None => Err("locked".to_string()),
        },
        Some(t) => Ok(t),
    }
}

/// Save the family JSON text. With a passphrase it is stored locked.
#[tauri::command]
fn family_save(json: String, passphrase: Option<String>) -> Result<(), String> {
    let text = match passphrase {
        Some(p) if !p.is_empty() => cindrix_crypto::lock(&json, &p).map_err(|e| e.to_string())?,
        _ => json,
    };
    user().write_family(&text)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_cards,
            ask,
            inventory_list,
            inventory_save_item,
            inventory_delete,
            what_fails_next,
            family_status,
            family_load,
            family_save
        ])
        .run(tauri::generate_context!())
        .expect("error while running Cindrix Ops");
}
