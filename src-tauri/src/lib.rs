use cindrix_core::inventory::{self, Item, Status};
use cindrix_core::userdata::{self, UserDir};
use cindrix_core::hardware::{self, Size};
use cindrix_core::llama::LlamaModel;
use cindrix_core::modelhost::ModelHost;
use cindrix_core::{calendar, cards, cascade, pipeline};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::Manager;

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

// ---------- the model ----------

#[derive(Serialize, Clone, Default)]
struct ModelStatus {
    /// "none", "loading", "ready" or "error"
    state: String,
    model: String,
    size: String,
    message: String,
    ram_gb: f64,
}

#[derive(Default)]
struct AppState {
    host: Mutex<Option<ModelHost>>,
    port: Mutex<Option<u16>>,
    status: Mutex<ModelStatus>,
}

impl AppState {
    fn stop(&self) {
        if let Some(mut h) = self.host.lock().unwrap().take() {
            h.stop();
        }
        *self.port.lock().unwrap() = None;
    }
    fn set(&self, state: &str, model: &str, size: &str, message: &str) {
        let mut st = self.status.lock().unwrap();
        st.state = state.into();
        st.model = model.into();
        st.size = size.into();
        st.message = message.into();
        st.ram_gb = hardware::total_ram_gb().unwrap_or(0.0);
    }
}

fn engine_path() -> PathBuf {
    let name = if cfg!(windows) { "llama-server.exe" } else { "llama-server" };
    root().join("engines").join(name)
}

fn models_dir() -> PathBuf {
    root().join("models")
}

fn model_files() -> Vec<String> {
    std::fs::read_dir(models_dir())
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| hardware::size_of(n).is_some()).collect())
        .unwrap_or_default()
}

/// Start the model engine with `file`, or the best file for this computer's memory.
fn start_model(state: &AppState, file: Option<String>) {
    state.stop();
    let ram = hardware::total_ram_gb().unwrap_or(8.0);
    let files = model_files();
    if files.is_empty() {
        state.set("none", "", "", "No model file yet. Put a .gguf model file in the models folder next to the program, then restart.");
        return;
    }
    let chosen = match file {
        Some(f) if files.contains(&f) => hardware::size_of(&f).map(|s| (f, s)),
        _ => hardware::choose_model(ram, &files, None),
    };
    let (name, size): (String, Size) = match chosen {
        Some(c) => c,
        None => {
            state.set("none", "", "", "No usable model file found in the models folder.");
            return;
        }
    };
    if !engine_path().exists() {
        state.set("error", &name, size.label(), "The model engine (llama-server) was not found in the engines folder.");
        return;
    }
    state.set("loading", &name, size.label(), "Loading the model. The first start can take a minute.");
    let started = ModelHost::start(&engine_path(), &models_dir().join(&name), hardware::threads(), hardware::context_for(ram));
    match started {
        Ok(mut host) => match host.wait_ready(Duration::from_secs(300)) {
            Ok(()) => {
                *state.port.lock().unwrap() = Some(host.port);
                *state.host.lock().unwrap() = Some(host);
                state.set("ready", &name, size.label(), "Ready.");
            }
            Err(e) => {
                host.stop();
                state.set("error", &name, size.label(), &e);
            }
        },
        Err(e) => state.set("error", &name, size.label(), &e),
    }
}

#[tauri::command]
fn model_status(state: tauri::State<'_, AppState>) -> ModelStatus {
    state.status.lock().unwrap().clone()
}

#[derive(Serialize)]
struct ModelChoice {
    file: String,
    size: String,
    recommended: bool,
}

#[tauri::command]
fn list_models() -> Vec<ModelChoice> {
    let ram = hardware::total_ram_gb().unwrap_or(8.0);
    let files = model_files();
    let rec = hardware::choose_model(ram, &files, None).map(|x| x.0);
    files
        .into_iter()
        .map(|f| ModelChoice { size: hardware::size_of(&f).map(|s| s.label()).unwrap_or("").to_string(), recommended: Some(&f) == rec.as_ref(), file: f })
        .collect()
}

/// Switch to another model file. Runs in the background; poll model_status.
#[tauri::command]
fn select_model(file: String, app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        start_model(&st, Some(file));
    });
}

#[derive(Serialize, Clone)]
struct StreamEvent {
    /// true: replace everything shown with `text`. false: add `text` to the end.
    replace: bool,
    text: String,
}

#[tauri::command]
async fn ask_stream(question: String, on_event: Channel<StreamEvent>, app: tauri::AppHandle) -> Result<ReplyDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let lib = content_dir().join("library-sample");
        let port = *app.state::<AppState>().port.lock().unwrap();
        let mut emit = |e: pipeline::Event| {
            let ev = match e {
                pipeline::Event::Text(t) => StreamEvent { replace: false, text: t },
                pipeline::Event::Replace(t) => StreamEvent { replace: true, text: t },
            };
            let _ = on_event.send(ev);
        };
        let r = match port {
            Some(p) => pipeline::answer_stream(&question, &lib, &LlamaModel { port: p, max_tokens: 700 }, &mut emit),
            None => pipeline::answer_stream(&question, &lib, &pipeline::NoModel, &mut emit),
        };
        Ok(reply_dto(r))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn reply_dto(r: pipeline::Reply) -> ReplyDto {
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
    let app = tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let st = handle.state::<AppState>();
                start_model(&st, None);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_cards,
            ask_stream,
            model_status,
            list_models,
            select_model,
            inventory_list,
            inventory_save_item,
            inventory_delete,
            what_fails_next,
            family_status,
            family_load,
            family_save
        ])
        .build(tauri::generate_context!())
        .expect("error while building Cindrix Ops");
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(st) = handle.try_state::<AppState>() {
                st.stop();
            }
        }
    });
}
