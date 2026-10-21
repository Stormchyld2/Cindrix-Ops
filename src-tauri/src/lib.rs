use cindrix_core::inventory::{self, Item, Status};
use cindrix_core::userdata::{self, UserDir};
use cindrix_core::hardware::{self, Size};
use cindrix_core::llama::LlamaModel;
use cindrix_core::crawl;
use cindrix_core::academy;
use cindrix_core::drills;
use cindrix_core::drive::{self, DriveInfo};
use cindrix_core::kiwix::{self, KiwixHost};
use cindrix_core::maps::{self, MapStore};
use cindrix_core::modelhost::ModelHost;
use cindrix_core::product::Product;
use cindrix_core::shelf::{Book, Mark, Marks, Shelf};
use cindrix_core::{calendar, cards, cascade, pipeline};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri_plugin_dialog::DialogExt;

mod fetch;
mod music;
mod zimview;
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::Manager;

/// The Cindrix root folder. Portable: the folder the program sits in.
/// Set CINDRIX_ROOT to point elsewhere (used in development).
fn root() -> PathBuf {
    if let Ok(p) = std::env::var("CINDRIX_ROOT") {
        return PathBuf::from(p);
    }
    if let Some(p) = ROOT_OVERRIDE.lock().unwrap().clone() {
        return p;
    }
    program_dir()
}

/// Set when a Cindrix drive is found somewhere other than next to the program.
static ROOT_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

fn program_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Look for the Cindrix drive (by its marker file, never by drive letter) and use it.
/// Returns true when the root folder changed.
fn detect_drive(state: &AppState) -> bool {
    if std::env::var("CINDRIX_ROOT").is_ok() {
        return false;
    }
    let before = root();
    let prog = program_dir();
    let (r, info) = drive::choose(&prog, &drive::find_all());
    *ROOT_OVERRIDE.lock().unwrap() = if r != prog { Some(r) } else { None };
    *state.drive.lock().unwrap() = info;
    root() != before
}

/// A folder of the drive, or the same folder next to the program if the drive does not have it.
fn drive_or_program(name: &str) -> PathBuf {
    let a = root().join(name);
    if a.exists() { a } else { program_dir().join(name) }
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
    kiwix_host: Mutex<Option<KiwixHost>>,
    kiwix_port: Mutex<Option<u16>>,
    kiwix_shared: Mutex<bool>,
    /// The archives the library server was started with, to notice when new ones are added.
    kiwix_zims: Mutex<Vec<PathBuf>>,
    status: Mutex<ModelStatus>,
    drive: Mutex<Option<DriveInfo>>,
    crawl: Mutex<CrawlDto>,
    crawl_cancel: Arc<AtomicBool>,
    copy: Mutex<CopyDto>,
    music: Mutex<music::MusicState>,
    music_cancel: AtomicBool,
}

impl AppState {
    fn stop(&self) {
        if let Some(mut h) = self.host.lock().unwrap().take() {
            h.stop();
        }
        *self.port.lock().unwrap() = None;
    }
    fn stop_all(&self) {
        self.stop();
        self.stop_kiwix();
    }
    fn stop_kiwix(&self) {
        if let Some(mut h) = self.kiwix_host.lock().unwrap().take() {
            h.stop();
        }
        *self.kiwix_port.lock().unwrap() = None;
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
    drive_or_program("engines").join(name)
}

fn library_dir() -> PathBuf {
    root().join("library")
}

fn start_library(state: &AppState, share: bool) {
    state.stop_kiwix();
    *state.kiwix_shared.lock().unwrap() = false;
    let zims = kiwix::zim_files(&library_dir());
    *state.kiwix_zims.lock().unwrap() = zims.clone();
    let name = if cfg!(windows) { "kiwix-serve.exe" } else { "kiwix-serve" };
    let engine = drive_or_program("engines").join(name);
    if zims.is_empty() || !engine.exists() {
        return;
    }
    if let Ok(mut h) = KiwixHost::start(&engine, &zims, share) {
        if h.wait_ready(Duration::from_secs(60)).is_ok() {
            *state.kiwix_shared.lock().unwrap() = share;
            *state.kiwix_port.lock().unwrap() = Some(h.port);
            *state.kiwix_host.lock().unwrap() = Some(h);
        } else {
            h.stop();
        }
    }
}

#[derive(Serialize)]
struct ArchiveDto {
    file: String,
    mb: u64,
    articles: u32,
    /// Saved from a website by this app, so the assistant can read it
    saved_site: bool,
    note: String,
}

#[derive(Serialize)]
struct DriveDto {
    name: String,
    edition: String,
    path: String,
}

#[derive(Serialize)]
struct LibraryStatus {
    archives: Vec<ArchiveDto>,
    ready: bool,
    /// Address for the built-in reader (this computer only)
    url: Option<String>,
    shared: bool,
    /// Address other devices on the network can use, when sharing is on
    lan_url: Option<String>,
    drive: Option<DriveDto>,
    /// The folder everything is stored in
    folder: String,
    /// True while a website is being saved: the only time the app uses the internet
    online: bool,
}

fn web_text_dir() -> PathBuf {
    library_dir().join("web-text")
}

/// Sources whose license does not allow redistribution or commercial use.
fn license_note(file: &str) -> String {
    let f = file.to_lowercase();
    if ["wikihow", "ifixit", "hesperian"].iter().any(|n| f.contains(n)) {
        "Non-commercial license: fine for your own use, but never sell or give out copies.".into()
    } else {
        String::new()
    }
}

fn archive_list() -> Vec<ArchiveDto> {
    kiwix::zim_files(&library_dir())
        .into_iter()
        .filter_map(|p| {
            let file = p.file_name()?.to_string_lossy().to_string();
            let stem = p.file_stem()?.to_string_lossy().to_string();
            let mb = std::fs::metadata(&p).map(|m| m.len() / 1_048_576).unwrap_or(0);
            let articles = cindrix_core::zim::read_header(&p).map(|h| h.articles).unwrap_or(0);
            Some(ArchiveDto {
                saved_site: web_text_dir().join(format!("{}.md", stem)).exists(),
                note: license_note(&file),
                file,
                mb,
                articles,
            })
        })
        .collect()
}

fn drive_dto(d: &Option<DriveInfo>) -> Option<DriveDto> {
    d.as_ref().map(|d| DriveDto { name: d.name.clone(), edition: d.edition.clone(), path: d.root.to_string_lossy().to_string() })
}

#[derive(Serialize)]
struct ProductDto {
    /// "go", "vault" or "hub"
    id: &'static str,
    name: &'static str,
    budget_gb: Option<f64>,
    assistant: bool,
    library: bool,
    maps: bool,
    plan: bool,
    web_saver: bool,
    serve_archives: bool,
    music: bool,
}

/// Which product this is, from the drive marker. No marker (for example a dev run) counts as Go.
#[tauri::command]
fn product_info(state: tauri::State<'_, AppState>) -> ProductDto {
    let p = state.drive.lock().unwrap().as_ref().map(|d| d.product()).unwrap_or(Product::Go);
    let f = p.features();
    ProductDto {
        id: p.id(),
        name: p.name(),
        budget_gb: p.budget_gb(),
        assistant: f.assistant,
        library: f.library,
        maps: f.maps,
        plan: f.plan,
        web_saver: f.web_saver,
        serve_archives: f.serve_archives,
        music: f.music,
    }
}

// ---------- map packs and the tile protocol ----------

#[derive(Serialize)]
struct PackDto {
    rel: String,
    name: String,
    kind: String,
    mb: u64,
}

/// The `.pmtiles` map packs on this drive (street, topo, satellite).
#[tauri::command]
fn map_packs() -> Vec<PackDto> {
    maps::packs(&drive_or_program("maps"))
        .into_iter()
        .map(|p| PackDto { rel: p.rel, name: p.name, kind: p.kind, mb: p.mb })
        .collect()
}

fn tile_reply(status: u16, content_range: Option<String>, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    let mut b = tauri::http::Response::builder()
        .status(status)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Allow-Headers", "Range")
        .header("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length")
        .header("Accept-Ranges", "bytes");
    if let Some(r) = content_range {
        b = b.header("Content-Range", r);
    }
    if !body.is_empty() {
        b = b.header("Content-Type", "application/octet-stream");
    }
    b.body(body).unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

/// Hands the web view byte ranges of `.pmtiles` files under the drive's `maps/` folder, and
/// nothing else (see `maps::resolve_tile_path`). The map library asks for a piece at a time.
fn serve_tile(req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    if *req.method() == tauri::http::Method::OPTIONS {
        return tile_reply(204, None, Vec::new());
    }
    if *req.method() != tauri::http::Method::GET {
        return tile_reply(405, None, Vec::new());
    }
    let Some(path) = maps::resolve_tile_path(&drive_or_program("maps"), req.uri().path()) else {
        return tile_reply(404, None, Vec::new());
    };
    let Ok(len) = std::fs::metadata(&path).map(|m| m.len()) else {
        return tile_reply(404, None, Vec::new());
    };
    // The whole file can be many gigabytes, so a request without a Range is refused.
    let Some(range) = req.headers().get("range").and_then(|v| v.to_str().ok()) else {
        return tile_reply(416, Some(format!("bytes */{}", len)), Vec::new());
    };
    match maps::parse_range(range, len) {
        Ok((start, end)) => match maps::read_range(&path, start, end) {
            Ok(bytes) => tile_reply(206, Some(format!("bytes {}-{}/{}", start, end, len)), bytes),
            Err(_) => tile_reply(500, None, Vec::new()),
        },
        Err(maps::RangeError::Unsatisfiable) => tile_reply(416, Some(format!("bytes */{}", len)), Vec::new()),
        Err(maps::RangeError::Unsupported) => tile_reply(400, None, Vec::new()),
    }
}

// ---------- saved maps ----------

#[derive(Serialize)]
struct MapMetaDto {
    id: String,
    name: String,
    has_picture: bool,
    has_route: bool,
}

#[derive(Serialize)]
struct MapDto {
    json: String,
    notes: String,
}

#[derive(Serialize)]
struct RouteStats {
    km: f64,
    miles: f64,
}

fn map_store() -> MapStore {
    MapStore::new(&root().join("user"))
}

#[tauri::command]
fn maps_list() -> Vec<MapMetaDto> {
    map_store()
        .list()
        .into_iter()
        .map(|m| MapMetaDto { id: m.id, name: m.name, has_picture: m.has_picture, has_route: m.has_route })
        .collect()
}

#[tauri::command]
fn map_load(id: String) -> Result<MapDto, String> {
    let (json, notes) = map_store().load(&id)?;
    Ok(MapDto { json, notes })
}

/// Save a map. `id` is None for a new map. Returns the id.
#[tauri::command]
fn map_save(id: Option<String>, name: String, json: String, notes: String) -> Result<String, String> {
    map_store().save(id.as_deref(), &name, &json, &notes)
}

#[tauri::command]
fn map_delete(id: String) -> Result<(), String> {
    map_store().delete(&id)
}

/// Write route.gpx into the saved map's folder. Returns the file's path.
#[tauri::command]
fn map_export_gpx(id: String, name: String, points: Vec<(f64, f64)>) -> Result<String, String> {
    map_store().export_gpx(&id, &name, &points).map(|p| p.to_string_lossy().to_string())
}

/// Straight-line length of a route through the points (this app does not plan roads).
#[tauri::command]
fn route_stats(points: Vec<(f64, f64)>) -> RouteStats {
    let km = maps::route_km(&points);
    RouteStats { km, miles: km * 0.621_371_192 }
}

#[tauri::command]
fn library_status(state: tauri::State<'_, AppState>) -> LibraryStatus {
    let port = *state.kiwix_port.lock().unwrap();
    let shared = *state.kiwix_shared.lock().unwrap();
    let online = state.crawl.lock().unwrap().running;
    LibraryStatus {
        archives: archive_list(),
        ready: port.is_some(),
        url: port.map(|p| format!("http://127.0.0.1:{}/", p)),
        shared,
        lan_url: if shared { port.and_then(|p| kiwix::lan_ip().map(|ip| format!("http://{}:{}/", ip, p))) } else { None },
        drive: drive_dto(&state.drive.lock().unwrap()),
        folder: root().to_string_lossy().to_string(),
        online,
    }
}

/// Look for the Cindrix drive again (after plugging it in, or moving to another computer).
#[tauri::command]
fn drive_rescan(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let moved = detect_drive(&st);
        // archives added (or removed) since the library server started need a restart of it
        let changed = *st.kiwix_zims.lock().unwrap() != kiwix::zim_files(&library_dir());
        if moved || changed {
            let shared = *st.kiwix_shared.lock().unwrap();
            start_library(&st, shared);
        }
        if moved {
            start_model(&st, None);
        }
    });
}

/// Turn sharing the library with other devices on the same network on or off. Off by default.
#[tauri::command]
fn library_share(enable: bool, app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        start_library(&st, enable);
    });
}

fn models_dir() -> PathBuf {
    drive_or_program("models")
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
        let kport = *app.state::<AppState>().kiwix_port.lock().unwrap();
        let passages = || {
            let mut p = cindrix_core::retrieval::search(&lib, &question, 3);
            p.extend(bundled_shelf().search(&question, 2));
            p.extend(user_shelf().search(&question, 2));
            p.extend(cindrix_core::retrieval::search(&web_text_dir(), &question, 2));
            if let Some(k) = kport {
                p.extend(kiwix::search(k, &question, 2));
            }
            // every archive on the drive, by title (needs no Kiwix program)
            p.extend(zimview::library().passages(&question, 2, 3));
            p.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            // the same article found two ways counts once
            let mut seen: Vec<String> = Vec::new();
            p.retain(|x| {
                let k = x.heading.to_lowercase();
                if seen.contains(&k) { false } else { seen.push(k); true }
            });
            p.truncate(3);
            p
        };
        let mut emit = |e: pipeline::Event| {
            let ev = match e {
                pipeline::Event::Text(t) => StreamEvent { replace: false, text: t },
                pipeline::Event::Replace(t) => StreamEvent { replace: true, text: t },
            };
            let _ = on_event.send(ev);
        };
        let r = match port {
            Some(p) => pipeline::answer_stream_with(&question, &passages, &LlamaModel { port: p, max_tokens: 700 }, &mut emit),
            None => pipeline::answer_stream_with(&question, &passages, &pipeline::NoModel, &mut emit),
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

fn user_shelf() -> Shelf {
    Shelf::new(&root().join("user").join("my-library"), "u")
}

fn bundled_shelf() -> Shelf {
    Shelf::new(&content_dir().join("shelf"), "b")
}

/// Books whose id starts with "b" come with the drive and cannot be changed or removed.
fn shelf_for(id: &str) -> Shelf {
    if id.starts_with('b') { bundled_shelf() } else { user_shelf() }
}

fn marks() -> Marks {
    Marks::new(&root().join("user"))
}

#[derive(Serialize)]
struct BookDto {
    id: String,
    title: String,
    author: String,
    category: String,
    kind: String,
    added: String,
    license: String,
    mine: bool,
    searchable: bool,
    last_page: u32,
}

fn book_dto(b: Book, mine: bool, shelf: &Shelf) -> BookDto {
    BookDto { searchable: shelf.has_text(&b.id), last_page: marks().last_page(&b.id), id: b.id, title: b.title, author: b.author,
              category: b.category, kind: b.kind, added: b.added, license: b.license, mine }
}

#[tauri::command]
fn shelf_list() -> Result<Vec<BookDto>, String> {
    let (bs, us) = (bundled_shelf(), user_shelf());
    let mut out: Vec<BookDto> = bs.list()?.into_iter().map(|b| book_dto(b, false, &bs)).collect();
    out.extend(us.list()?.into_iter().map(|b| book_dto(b, true, &us)));
    Ok(out)
}

#[derive(Serialize)]
struct DriveScanDto {
    /// Titles of the books newly put on the shelf.
    added: Vec<String>,
    /// Files looked at that were already on the shelf or had been seen before.
    known: usize,
    problems: Vec<String>,
    /// Where the scan looked, for the message when nothing was found.
    folders: Vec<String>,
}

/// Put the books found on the drive (PDF, Markdown and text in `library`, `books`, `manuals`)
/// onto the person's shelf. Safe to call again: each file is added once, and a book the person
/// removed is not put back. Reading big files takes a while, so this runs off the main thread.
#[tauri::command]
async fn shelf_scan_drive() -> Result<DriveScanDto, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let r = root();
        let files = cindrix_core::shelf::drive_book_files(&r);
        let rep = user_shelf().import_from_drive(&r, &files, &calendar::today_string());
        Ok(DriveScanDto {
            added: rep.added,
            known: rep.known,
            problems: rep.problems,
            folders: ["library", "books", "manuals"].iter().map(|d| r.join(d).to_string_lossy().to_string()).collect(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Add a book. The file bytes are the raw request body; the name and category come in headers.
#[tauri::command]
fn shelf_add(request: tauri::ipc::Request<'_>) -> Result<BookDto, String> {
    let bytes = match request.body() {
        tauri::ipc::InvokeBody::Raw(v) => v.clone(),
        _ => return Err("No file was received.".into()),
    };
    let header = |k: &str| -> String {
        request.headers().get(k).and_then(|v| v.to_str().ok()).map(percent_decode).unwrap_or_default()
    };
    let name = header("x-file-name");
    let category = header("x-category");
    let shelf = user_shelf();
    let b = shelf.add(&name, &bytes, &category, &calendar::today_string())?;
    Ok(book_dto(b, true, &shelf))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

#[tauri::command]
fn shelf_read(id: String) -> Result<tauri::ipc::Response, String> {
    Ok(tauri::ipc::Response::new(shelf_for(&id).read(&id)?))
}

#[tauri::command]
fn shelf_update(id: String, title: String, author: String, category: String) -> Result<(), String> {
    if id.starts_with('b') {
        return Err("Books that come with the drive cannot be edited.".into());
    }
    user_shelf().update(&id, &title, &author, &category)
}

#[tauri::command]
fn shelf_remove(id: String) -> Result<(), String> {
    if id.starts_with('b') {
        return Err("Books that come with the drive cannot be removed.".into());
    }
    user_shelf().remove(&id)?;
    marks().remove_book(&id)
}

/// The reader sends the text of each page so the assistant can search the book.
#[tauri::command]
fn shelf_save_text(id: String, pages: Vec<String>) -> Result<(), String> {
    if id.starts_with('b') {
        return Err("Books that come with the drive are indexed when the drive is built.".into());
    }
    user_shelf().save_pages(&id, &pages)
}

#[derive(Serialize, Deserialize)]
struct MarkDto {
    kind: String,
    page: u32,
    text: String,
}

#[tauri::command]
fn marks_list(book: String) -> Result<Vec<MarkDto>, String> {
    Ok(marks().for_book(&book)?.into_iter().filter(|m| m.kind != "last").map(|m| MarkDto { kind: m.kind, page: m.page, text: m.text }).collect())
}

#[tauri::command]
fn marks_add(book: String, kind: String, page: u32, text: String) -> Result<(), String> {
    if !["bookmark", "highlight", "note"].contains(&kind.as_str()) {
        return Err("Unknown kind of mark.".into());
    }
    marks().add(Mark { book, kind, page, text })
}

#[tauri::command]
fn marks_remove(book: String, kind: String, page: u32, text: String) -> Result<(), String> {
    marks().remove(&book, &kind, page, &text)
}

#[tauri::command]
fn marks_set_last(book: String, page: u32) -> Result<(), String> {
    marks().set_last(&book, page)
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


// ---------- saving websites ----------

#[derive(Serialize, Clone, Default)]
struct CrawlDto {
    running: bool,
    /// "crawling", "writing" or "done"
    phase: String,
    pages: usize,
    files: usize,
    mb: f64,
    queued: usize,
    current: String,
    failed: usize,
    /// Set once finished
    finished: bool,
    ok: bool,
    message: String,
    file: String,
    blocked: usize,
    hit_limit: bool,
}

/// Save a website into the library as a .zim archive. The only feature that uses the internet.
#[tauri::command]
fn crawl_start(url: String, name: String, section: bool, max_pages: usize, max_mb: u64, fast: bool, app: tauri::AppHandle) -> Result<(), String> {
    let st = app.state::<AppState>();
    {
        let mut c = st.crawl.lock().unwrap();
        if c.running {
            return Err("A website is already being saved.".into());
        }
        let start = crawl::Url::parse(url.trim()).ok_or("That does not look like a web address. It should start with http:// or https://")?;
        let _ = start;
        *c = CrawlDto { running: true, phase: "crawling".into(), ..Default::default() };
    }
    st.crawl_cancel.store(false, Ordering::Relaxed);
    let cancel = st.crawl_cancel.clone();
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let url = url.trim().to_string();
        let name = if name.trim().is_empty() { crawl::default_name(&url) } else { name.trim().to_string() };
        let mut stem = crawl::slug(&name);
        if stem.is_empty() {
            stem = "website".into();
        }
        let mut n = 1;
        let mut final_stem = stem.clone();
        while library_dir().join(format!("{}.zim", final_stem)).exists() {
            n += 1;
            final_stem = format!("{}-{}", stem, n);
        }
        let part = library_dir().join(format!("{}.part", final_stem));
        let target = library_dir().join(format!("{}.zim", final_stem));
        let text = web_text_dir().join(format!("{}.md", final_stem));
        let spool = root().join("user").join("crawl-tmp");
        let mut o = crawl::Options::new(&url, &name);
        if fast {
            o = o.fast();
        }
        o.section_only = section;
        o.max_pages = max_pages.clamp(1, 20_000);
        o.max_mb = max_mb.clamp(10, 100_000);
        let fetcher = fetch::WebFetcher::new();
        let result = crawl::run(&o, &fetcher, &part, &text, &spool, &cancel, &mut |p| {
            let mut c = st.crawl.lock().unwrap();
            c.phase = p.phase.clone();
            c.pages = p.pages;
            c.files = p.files;
            c.mb = p.mb;
            c.queued = p.queued;
            c.current = p.current.clone();
            c.failed = p.failed;
        });
        let result = result.and_then(|s| std::fs::rename(&part, &target).map(|_| s).map_err(|e| format!("Cannot finish the archive: {}", e)));
        {
            let mut c = st.crawl.lock().unwrap();
            c.running = false;
            c.finished = true;
            match &result {
                Ok(s) => {
                    c.ok = true;
                    c.file = format!("{}.zim", final_stem);
                    c.pages = s.pages;
                    c.files = s.files;
                    c.mb = s.bytes as f64 / 1048576.0;
                    c.blocked = s.blocked_by_robots;
                    c.hit_limit = s.hit_limit;
                    c.failed = s.failed.len();
                    c.message = String::new();
                }
                Err(e) => {
                    c.ok = false;
                    c.message = e.clone();
                    let _ = std::fs::remove_file(&part);
                }
            }
        }
        if result.is_ok() {
            let shared = *st.kiwix_shared.lock().unwrap();
            start_library(&st, shared);
        }
    });
    Ok(())
}

#[tauri::command]
fn crawl_status(state: tauri::State<'_, AppState>) -> CrawlDto {
    state.crawl.lock().unwrap().clone()
}

#[tauri::command]
fn crawl_cancel(state: tauri::State<'_, AppState>) {
    state.crawl_cancel.store(true, Ordering::Relaxed);
}

// ---------- adding and removing archives ----------

#[derive(Serialize, Clone, Default)]
struct CopyDto {
    running: bool,
    finished: bool,
    ok: bool,
    name: String,
    copied_mb: u64,
    total_mb: u64,
    message: String,
}

/// Ask for a .zim file and copy it into the library (in the background; poll `archive_copy_status`).
/// Returns the file name, or None if the person cancelled the dialog.
#[tauri::command]
async fn archive_add(app: tauri::AppHandle) -> Result<Option<String>, String> {
    if app.state::<AppState>().copy.lock().unwrap().running {
        return Err("An archive is already being copied.".into());
    }
    let a2 = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        a2.dialog().file().add_filter("Kiwix archive (.zim)", &["zim"]).blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(fp) = picked else { return Ok(None) };
    let src = fp.into_path().map_err(|e| e.to_string())?;
    cindrix_core::zim::read_header(&src).map_err(|_| "That file is not a ZIM archive.".to_string())?;
    let lib = library_dir();
    let file = src.file_name().ok_or("No file name")?.to_string_lossy().to_string();
    if src.parent() == Some(lib.as_path()) {
        return Err("That archive is already in the library.".into());
    }
    std::fs::create_dir_all(&lib).map_err(|e| e.to_string())?;
    let mut dest_name = file.clone();
    let mut n = 1;
    while lib.join(&dest_name).exists() {
        n += 1;
        dest_name = format!("{}-{}.zim", file.trim_end_matches(".zim"), n);
    }
    let total = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
    {
        let st = app.state::<AppState>();
        *st.copy.lock().unwrap() = CopyDto { running: true, name: dest_name.clone(), total_mb: total / 1_048_576, ..Default::default() };
    }
    let name_out = dest_name.clone();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        let st = app.state::<AppState>();
        let part = lib.join(format!("{}.part", dest_name));
        let res: Result<(), String> = (|| {
            let mut r = std::fs::File::open(&src).map_err(|e| e.to_string())?;
            let mut w = std::fs::File::create(&part).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 4 << 20];
            let mut done = 0u64;
            loop {
                let k = r.read(&mut buf).map_err(|e| e.to_string())?;
                if k == 0 {
                    break;
                }
                w.write_all(&buf[..k]).map_err(|e| format!("Could not write to the drive (is it full?): {}", e))?;
                done += k as u64;
                st.copy.lock().unwrap().copied_mb = done / 1_048_576;
            }
            w.sync_all().map_err(|e| e.to_string())?;
            std::fs::rename(&part, lib.join(&dest_name)).map_err(|e| e.to_string())
        })();
        if res.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        {
            let mut c = st.copy.lock().unwrap();
            c.running = false;
            c.finished = true;
            c.ok = res.is_ok();
            c.message = res.clone().err().unwrap_or_default();
        }
        if res.is_ok() {
            let shared = *st.kiwix_shared.lock().unwrap();
            start_library(&st, shared);
        }
    });
    Ok(Some(name_out))
}

#[tauri::command]
fn archive_copy_status(state: tauri::State<'_, AppState>) -> CopyDto {
    state.copy.lock().unwrap().clone()
}

/// Remove an archive from the library (and the assistant's text copy of it).
#[tauri::command]
fn archive_remove(name: String, app: tauri::AppHandle) -> Result<(), String> {
    if name.contains('/') || name.contains('\\') || !name.ends_with(".zim") {
        return Err("Not an archive name.".into());
    }
    let p = library_dir().join(&name);
    if !p.exists() {
        return Err("That archive is not in the library.".into());
    }
    std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(web_text_dir().join(format!("{}.md", name.trim_end_matches(".zim"))));
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let shared = *st.kiwix_shared.lock().unwrap();
        start_library(&st, shared);
    });
    Ok(())
}

// ---------- academy ----------

#[derive(Serialize)]
struct BlockDto {
    kind: &'static str,
    text: String,
    items: Vec<String>,
}

#[derive(Serialize)]
struct QuestionDto {
    text: String,
    options: Vec<String>,
    correct: usize,
    explain: String,
}

#[derive(Serialize)]
struct LessonDto {
    id: String,
    title: String,
    minutes: u32,
    status: String,
    source: String,
    questions: usize,
    tries: usize,
    best_score: u32,
    total: u32,
    last_date: String,
}

#[derive(Serialize)]
struct ModuleDto {
    id: String,
    title: String,
    summary: String,
    lessons: Vec<LessonDto>,
}

#[derive(Serialize)]
struct LessonBodyDto {
    id: String,
    title: String,
    minutes: u32,
    status: String,
    source: String,
    blocks: Vec<BlockDto>,
    quiz: Vec<QuestionDto>,
}

#[tauri::command]
fn academy_list() -> Result<Vec<ModuleDto>, String> {
    let results = academy::Log::new(&root().join("user")).list()?;
    Ok(academy::load(&content_dir().join("academy"))
        .into_iter()
        .map(|m| ModuleDto {
            id: m.id,
            title: m.title,
            summary: m.summary,
            lessons: m
                .lessons
                .into_iter()
                .map(|l| {
                    let p = academy::progress_for(&results, &l.id);
                    LessonDto { questions: l.quiz.len(), tries: p.tries, best_score: p.best_score, total: p.total, last_date: p.last_date, id: l.id, title: l.title, minutes: l.minutes, status: l.status, source: l.source }
                })
                .collect(),
        })
        .collect())
}

/// One lesson, found by id in the loaded course (the id is never turned into a file path).
#[tauri::command]
fn academy_lesson(id: String) -> Result<LessonBodyDto, String> {
    let l = academy::load(&content_dir().join("academy"))
        .into_iter()
        .flat_map(|m| m.lessons)
        .find(|l| l.id == id)
        .ok_or("There is no such lesson.")?;
    let blocks = l
        .body
        .into_iter()
        .map(|b| match b {
            academy::Block::Heading(t) => BlockDto { kind: "heading", text: t, items: vec![] },
            academy::Block::Para(t) => BlockDto { kind: "para", text: t, items: vec![] },
            academy::Block::Note(t) => BlockDto { kind: "note", text: t, items: vec![] },
            academy::Block::Bullets(v) => BlockDto { kind: "bullets", text: String::new(), items: v },
            academy::Block::Steps(v) => BlockDto { kind: "steps", text: String::new(), items: v },
        })
        .collect();
    let quiz = l.quiz.into_iter().map(|q| QuestionDto { text: q.text, options: q.options, correct: q.correct, explain: q.explain }).collect();
    Ok(LessonBodyDto { id: l.id, title: l.title, minutes: l.minutes, status: l.status, source: l.source, blocks, quiz })
}

#[tauri::command]
fn academy_save(lesson: String, score: u32, total: u32) -> Result<(), String> {
    let known = academy::load(&content_dir().join("academy")).iter().any(|m| m.lessons.iter().any(|l| l.id == lesson));
    if !known {
        return Err("There is no such lesson.".into());
    }
    academy::Log::new(&root().join("user")).add(academy::Result_ { date: calendar::today_string(), lesson, score, total })
}

// ---------- drills ----------

#[derive(Serialize)]
struct DrillResultDto {
    date: String,
    seconds: u32,
    done: u32,
    total: u32,
    notes: String,
}

#[derive(Serialize)]
struct DrillDto {
    id: String,
    title: String,
    minutes: u32,
    place: String,
    summary: String,
    status: String,
    steps: Vec<String>,
    /// Most recent result, if the drill has been done
    last: Option<DrillResultDto>,
    times_done: usize,
}

#[tauri::command]
fn drills_list() -> Result<Vec<DrillDto>, String> {
    let results = drills::Log::new(&root().join("user")).list()?;
    Ok(drills::load_dir(&content_dir().join("drills"))
        .into_iter()
        .map(|d| {
            let mine: Vec<&drills::Result_> = results.iter().filter(|r| r.drill == d.id).collect();
            DrillDto {
                last: mine.last().map(|r| DrillResultDto { date: r.date.clone(), seconds: r.seconds, done: r.done, total: r.total, notes: r.notes.clone() }),
                times_done: mine.len(),
                id: d.id,
                title: d.title,
                minutes: d.minutes,
                place: d.place,
                summary: d.summary,
                status: d.status,
                steps: d.steps,
            }
        })
        .collect())
}

#[tauri::command]
fn drill_save(drill: String, seconds: u32, done: u32, total: u32, notes: String) -> Result<(), String> {
    drills::Log::new(&root().join("user")).add(drills::Result_ { date: calendar::today_string(), drill, seconds, done, total, notes })
}

/// The list of sources and licenses shown in Settings (a JSON file that ships with the content).
#[tauri::command]
fn sources_list() -> String {
    std::fs::read_to_string(content_dir().join("sources.json")).unwrap_or_else(|_| "{\"groups\":[]}".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("cindrix-tiles", |_ctx, request| serve_tile(&request))
        .register_uri_scheme_protocol("cindrix-music", |_ctx, request| music::serve_music(&request))
        .register_asynchronous_uri_scheme_protocol("cindrix-zim", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || responder.respond(zimview::serve(&app, &request)));
        })
        .manage(AppState::default())
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let st = handle.state::<AppState>();
                detect_drive(&st);
                start_library(&st, false);
                start_model(&st, None);
                if st.drive.lock().unwrap().as_ref().map(|d| d.product().features().music).unwrap_or(true) {
                    music::start_scan(&handle);
                }
                // No Cindrix drive yet? Keep looking, so plugging it in later (or into another
                // port, with another drive letter) just works.
                while st.drive.lock().unwrap().is_none() && std::env::var("CINDRIX_ROOT").is_err() {
                    std::thread::sleep(Duration::from_secs(8));
                    if detect_drive(&st) {
                        start_library(&st, false);
                        start_model(&st, None);
                        music::start_scan(&handle);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_cards,
            ask_stream,
            model_status,
            library_status,
            product_info,
            maps_list,
            map_load,
            map_save,
            map_delete,
            map_export_gpx,
            route_stats,
            map_packs,
            zimview::zim_books,
            zimview::zim_bookmarks_load,
            zimview::zim_bookmarks_save,
            music::music_status,
            music::music_scan,
            music::music_query,
            music::music_facets,
            music::music_resolve,
            music::music_prepare,
            music::music_clear_cache,
            music::music_playlists,
            music::music_playlist_load,
            music::music_playlist_save,
            music::music_playlist_delete,
            music::music_state_load,
            music::music_state_save,
            library_share,
            drive_rescan,
            academy_list,
            academy_lesson,
            academy_save,
            drills_list,
            drill_save,
            sources_list,
            crawl_start,
            crawl_status,
            crawl_cancel,
            archive_add,
            archive_copy_status,
            archive_remove,
            shelf_list,
            shelf_scan_drive,
            shelf_add,
            shelf_read,
            shelf_update,
            shelf_remove,
            shelf_save_text,
            marks_list,
            marks_add,
            marks_remove,
            marks_set_last,
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
                st.stop_all();
            }
        }
    });
}
