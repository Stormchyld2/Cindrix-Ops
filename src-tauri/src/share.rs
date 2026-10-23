//! Sharing the library with other devices over the network (docs/34-share-server.md).
//!
//! A small web server that gives a browser the same screens as the app. It switches on by itself when
//! this computer or phone starts a hotspot (or by hand), asks for a PIN, and lets a guest only read: the
//! actions a guest may use are listed in `cindrix_core::share::GUEST_COMMANDS`. The rules (reading a
//! request, the PIN, the limits) are in the core and tested there; this file is the part that listens.

use crate::{AppState, StreamEvent};
use cindrix_core::hotspot;
use cindrix_core::share::{self, Auth, Gate, Request, Response, SignIn};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Manager;

/// The port guests use. If another program has it, any free one is taken and shown on screen.
const PORT: u16 = 8765;

struct Running {
    stop: Arc<AtomicBool>,
    port: u16,
    /// "hotspot" (switched on by the hotspot) or "manual"
    reason: String,
}

pub struct Share {
    running: Mutex<Option<Running>>,
    auth: Mutex<Auth>,
    /// The PIN file is read the first time it is needed, after the drive has been found.
    pin_loaded: AtomicBool,
    gate: Gate,
    /// The person switched sharing off while a hotspot was on: do not switch it on again until the hotspot has been off.
    suppress: AtomicBool,
}

impl Default for Share {
    fn default() -> Share {
        Share { running: Mutex::new(None), auth: Mutex::new(Auth::new("")), pin_loaded: AtomicBool::new(false), gate: Gate::new(2), suppress: AtomicBool::new(false) }
    }
}

impl Share {
    fn auth(&self) -> std::sync::MutexGuard<'_, Auth> {
        let mut a = self.auth.lock().unwrap();
        if !self.pin_loaded.swap(true, Ordering::SeqCst) {
            a.set_pin(&load_pin());
        }
        a
    }
}

// ---------- the PIN is kept in a plain file so it stays the same from one start to the next ----------

fn pin_file() -> std::path::PathBuf {
    crate::root().join("user").join("share-pin.txt")
}

fn load_pin() -> String {
    if let Ok(t) = std::fs::read_to_string(pin_file()) {
        let t = t.trim().to_string();
        if valid_pin(&t) {
            return t;
        }
    }
    let p = share::new_pin();
    let _ = cindrix_core::fsio::write_atomic(&pin_file(), p.as_bytes());
    p
}

fn valid_pin(p: &str) -> bool {
    (4..=8).contains(&p.len()) && p.chars().all(|c| c.is_ascii_digit())
}

// ---------- what the screen shows ----------

#[derive(Serialize, Clone)]
pub struct AddrDto {
    pub label: String,
    pub url: String,
    pub hotspot: bool,
}

#[derive(Serialize, Clone)]
pub struct ShareInfo {
    pub on: bool,
    /// Sharing switches itself on when a hotspot starts (Settings can turn that off).
    pub auto: bool,
    /// "hotspot", "manual" or "" when off
    pub reason: String,
    pub pin: String,
    pub port: u16,
    pub addresses: Vec<AddrDto>,
    pub hotspot_up: bool,
    pub signed_in: usize,
}

pub fn info(app: &tauri::AppHandle) -> ShareInfo {
    let st = app.state::<AppState>();
    let run = st.share.running.lock().unwrap();
    let (on, reason, port) = match run.as_ref() {
        Some(r) => (true, r.reason.clone(), r.port),
        None => (false, String::new(), PORT),
    };
    drop(run);
    let adapters = hotspot::detect();
    let hs = hotspot::find_hotspot(&adapters);
    let addresses = if on {
        let mut v: Vec<AddrDto> = hotspot::reachable_addresses(&adapters)
            .into_iter()
            .map(|a| AddrDto { hotspot: hs.as_ref().map(|h| h.ip == a.ip).unwrap_or(false), label: a.name, url: format!("http://{}:{}/", a.ip, port) })
            .collect();
        v.sort_by_key(|a| !a.hotspot);
        v
    } else {
        vec![]
    };
    let a = st.share.auth();
    ShareInfo {
        on,
        auto: crate::settings::flag("shareAuto", true),
        reason,
        pin: a.pin().to_string(),
        port,
        addresses,
        hotspot_up: hs.is_some(),
        signed_in: a.signed_in(),
    }
}

// ---------- starting and stopping ----------

pub fn start(app: &tauri::AppHandle, reason: &str) -> Result<(), String> {
    let st = app.state::<AppState>();
    let mut run = st.share.running.lock().unwrap();
    if run.is_some() {
        return Ok(());
    }
    // a server that was just stopped needs a moment to let go of the port
    let mut bound = None;
    for _ in 0..10 {
        match TcpListener::bind(("0.0.0.0", PORT)) {
            Ok(l) => { bound = Some(l); break; }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let listener = match bound {
        Some(l) => l,
        None => TcpListener::bind(("0.0.0.0", 0)).map_err(|e| format!("Sharing could not start: {}. Check that nothing blocks this program on the network.", e))?,
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(PORT);
    let stop = Arc::new(AtomicBool::new(false));
    *run = Some(Running { stop: stop.clone(), port, reason: reason.to_string() });
    drop(run);
    let app2 = app.clone();
    std::thread::spawn(move || accept_loop(app2, listener, stop));
    Ok(())
}

pub fn stop(app: &tauri::AppHandle) {
    let st = app.state::<AppState>();
    let taken = st.share.running.lock().unwrap().take();
    if let Some(r) = taken {
        r.stop.store(true, Ordering::SeqCst);
    }
}

/// Watches for a hotspot: sharing comes on with it and goes off when it does (unless the person started it by hand).
pub fn watch(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(5));
        let up = hotspot::find_hotspot(&hotspot::detect()).is_some();
        let st = app.state::<AppState>();
        if !up {
            st.share.suppress.store(false, Ordering::SeqCst);
        }
        let auto = crate::settings::flag("shareAuto", true);
        let reason = st.share.running.lock().unwrap().as_ref().map(|r| r.reason.clone());
        match reason.as_deref() {
            None if up && auto && !st.share.suppress.load(Ordering::SeqCst) => {
                let _ = start(&app, "hotspot");
            }
            Some("hotspot") if !up || !auto => stop(&app),
            _ => {}
        }
    });
}

#[tauri::command]
pub fn share_info(app: tauri::AppHandle) -> ShareInfo {
    info(&app)
}

#[tauri::command]
pub fn share_set(enable: bool, app: tauri::AppHandle) -> Result<ShareInfo, String> {
    if enable {
        app.state::<AppState>().share.suppress.store(false, Ordering::SeqCst);
        // already on because of the hotspot: it now stays on by hand
        let already = {
            let st = app.state::<AppState>();
            let mut run = st.share.running.lock().unwrap();
            match run.as_mut() {
                Some(r) => { r.reason = "manual".into(); true }
                None => false,
            }
        };
        if !already {
            start(&app, "manual")?;
        }
    } else {
        let up = hotspot::find_hotspot(&hotspot::detect()).is_some();
        app.state::<AppState>().share.suppress.store(up, Ordering::SeqCst);
        stop(&app);
    }
    Ok(info(&app))
}

/// A PIN of 4 to 8 digits, or empty for a new random one. Everyone signed in has to enter it again.
#[tauri::command]
pub fn share_set_pin(pin: String, app: tauri::AppHandle) -> Result<ShareInfo, String> {
    let pin = pin.trim().to_string();
    let pin = if pin.is_empty() { share::new_pin() } else { pin };
    if !valid_pin(&pin) {
        return Err("A PIN is 4 to 8 digits, with no letters or spaces.".into());
    }
    cindrix_core::fsio::write_atomic(&pin_file(), pin.as_bytes()).map_err(|e| format!("The PIN could not be saved: {}", e))?;
    app.state::<AppState>().share.auth().set_pin(&pin);
    Ok(info(&app))
}

// ---------- the server ----------

fn accept_loop(app: tauri::AppHandle, listener: TcpListener, stop: Arc<AtomicBool>) {
    let _ = listener.set_nonblocking(true);
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, peer)) => {
                let app = app.clone();
                std::thread::spawn(move || {
                    let _ = handle(&app, stream, peer);
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(40)),
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

fn send(stream: &mut TcpStream, r: &Response) {
    let _ = stream.write_all(&r.to_bytes());
    let _ = stream.flush();
}

fn handle(app: &tauri::AppHandle, mut stream: TcpStream, peer: SocketAddr) -> Result<(), String> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(120)));
    let read = stream.try_clone().map_err(|e| e.to_string())?;
    let req = match share::read_request(&mut BufReader::new(read)) {
        Ok(r) => r,
        Err(e) => {
            send(&mut stream, &Response::text(400, &e));
            return Ok(());
        }
    };
    route(app, &req, &peer.ip().to_string(), &mut stream);
    Ok(())
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn esc(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn pin_page(message: &str) -> Response {
    let note = if message.is_empty() { String::new() } else { format!("<p class=\"m\">{}</p>", esc(message)) };
    Response::html(
        200,
        &format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>Cindrix</title>\
<style>body{{font:18px system-ui,sans-serif;background:#14171a;color:#e8e6e1;margin:0;display:flex;min-height:100vh;align-items:center;justify-content:center}}\
form{{width:min(360px,90vw);padding:24px}}h1{{font-size:22px;margin:0 0 6px}}p{{color:#a8b0a8;margin:0 0 14px}}.m{{color:#f0d9a0}}\
input{{width:100%;box-sizing:border-box;font-size:28px;letter-spacing:.3em;text-align:center;padding:12px;border-radius:10px;border:1px solid #4a5148;background:#1d2227;color:inherit}}\
button{{margin-top:14px;width:100%;font-size:18px;padding:12px;border-radius:10px;border:0;background:#6fcf8d;color:#10150f;cursor:pointer}}</style></head>\
<body><form method=\"post\" action=\"/pin\"><h1>Cindrix library</h1><p>Type the PIN shown on the host computer or phone.</p>{}\
<input name=\"pin\" inputmode=\"numeric\" autocomplete=\"off\" autofocus aria-label=\"PIN\"><button type=\"submit\">Open the library</button></form></body></html>",
            note
        ),
    )
}

fn json_err(status: u16, msg: &str) -> Response {
    Response::json(status, &json!({ "error": msg }).to_string())
}

fn route(app: &tauri::AppHandle, req: &Request, who: &str, stream: &mut TcpStream) {
    let st = app.state::<AppState>();
    let sh = &st.share;

    // the PIN form
    if req.path == "/pin" && req.method == "POST" {
        let body = String::from_utf8_lossy(&req.body).to_string();
        let pin = body
            .split('&')
            .find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == "pin").map(|(_, v)| share::url_decode(v)))
            .unwrap_or_default();
        let out = sh.auth().sign_in(who, &pin, now());
        let r = match out {
            SignIn::Ok(t) => Response::new(302, "text/plain", vec![])
                .with("Location", "/")
                .with("Set-Cookie", &format!("cx={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000", t)),
            SignIn::Wrong => pin_page("That PIN is not right. Try again."),
            SignIn::Wait(n) => {
                let mut r = pin_page(&format!("Too many wrong tries. Wait {} seconds and try again.", n));
                r.status = 429;
                r
            }
        };
        send(stream, &r);
        return;
    }

    // everything else needs the sign-in
    let signed = sh.auth().valid(&req.cookie("cx").unwrap_or_default());
    if !signed {
        let api = req.path.starts_with("/api/") || req.path.starts_with("/tiles/") || req.path.starts_with("/music/");
        send(stream, &if api { json_err(401, "Type the PIN first.") } else { pin_page("") });
        return;
    }

    if req.method == "POST" {
        if let Some(cmd) = req.path.strip_prefix("/api/") {
            if cmd == "ask_stream" {
                ask_sse(app, req, stream);
            } else {
                send(stream, &api_call(app, cmd, &req.body));
            }
            return;
        }
    }
    if req.method != "GET" && req.method != "HEAD" {
        send(stream, &Response::text(405, "Not allowed."));
        return;
    }
    if req.path == "/api/shelf_read" {
        let id = req.query_value("id").unwrap_or("").to_string();
        let r = match crate::shelf_for(&id).read(&id) {
            Ok(b) => Response::new(200, "application/octet-stream", b),
            Err(e) => json_err(400, &e),
        };
        send(stream, &r);
        return;
    }
    // maps, music and archive pages are answered by the same code the app's own windows use
    let media = if let Some(rest) = req.raw_path.strip_prefix("/tiles") {
        Some(("tiles", rest.to_string()))
    } else if let Some(rest) = req.raw_path.strip_prefix("/music") {
        Some(("music", rest.to_string()))
    } else if req.raw_path.starts_with("/content/") || req.raw_path == "/suggest" || req.raw_path == "/search" || req.raw_path.starts_with("/skin/") {
        Some(("zim", req.raw_path.clone()))
    } else {
        None
    };
    if let Some((kind, path)) = media {
        send(stream, &media_reply(app, req, kind, &path));
        return;
    }
    send(stream, &static_file(app, &req.path));
}

/// Run one of the actions guests may use and give the answer as JSON.
fn api_call(app: &tauri::AppHandle, cmd: &str, body: &[u8]) -> Response {
    if !share::guest_allowed(cmd) {
        return json_err(403, "This can only be done on the host computer or phone.");
    }
    let args: Value = if body.is_empty() { json!({}) } else { serde_json::from_slice(body).unwrap_or_else(|_| json!({})) };
    match dispatch(app, cmd, &args) {
        Ok(v) => Response::json(200, &v.to_string()),
        Err(e) => json_err(400, &e),
    }
}

fn tv<T: Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn arg<T: serde::de::DeserializeOwned>(a: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(a.get(key).cloned().unwrap_or(Value::Null)).map_err(|_| format!("Something was missing from the request ({}).", key))
}

fn dispatch(app: &tauri::AppHandle, cmd: &str, a: &Value) -> Result<Value, String> {
    let st = app.state::<AppState>();
    match cmd {
        "list_cards" => tv(crate::list_cards()),
        "model_status" => tv(crate::model_status(st)),
        "library_status" => {
            // what the Archives screen needs, without the host's drive names or folders
            let mut v = tv(crate::library_status(st))?;
            if let Some(o) = v.as_object_mut() {
                o.insert("drive".into(), Value::Null);
                o.insert("folder".into(), json!(""));
                o.insert("lan_url".into(), Value::Null);
            }
            Ok(v)
        }
        "product_info" => tv(crate::product_info(st)),
        "map_packs" => tv(crate::map_packs()),
        "place_search" => tv(tauri::async_runtime::block_on(crate::place_search(arg(a, "query")?, arg(a, "nearLat")?, arg(a, "nearLon")?))),
        "directions" => tv(tauri::async_runtime::block_on(crate::directions(arg(a, "from")?, arg(a, "to")?, arg(a, "avoid")?))?),
        "route_stats" => tv(crate::route_stats(arg(a, "points")?)),
        "zim_books" => tv(crate::zimview::zim_books()),
        "zim_search_all" => tv(tauri::async_runtime::block_on(crate::zimview::zim_search_all(arg(a, "term")?))?),
        "music_status" => tv(crate::music::music_status(st)),
        "music_query" => tv(crate::music::music_query(st, arg(a, "args")?, arg(a, "offset")?, arg(a, "limit")?)),
        "music_facets" => tv(crate::music::music_facets(st, arg(a, "kind")?, arg(a, "prefix")?)),
        "music_resolve" => tv(crate::music::music_resolve(st, arg(a, "rels")?)),
        "music_prepare" => tv(tauri::async_runtime::block_on(crate::music::music_prepare(st, arg(a, "rel")?))?),
        "academy_list" => tv(crate::academy_list(arg(a, "person")?)?),
        "academy_lesson" => tv(crate::academy_lesson(arg(a, "id")?)?),
        "plans_list" => tv(crate::plans_list()?),
        "drills_list" => tv(crate::drills_list()?),
        "shelf_list" => tv(crate::shelf_list()?),
        "sources_list" => tv(crate::sources_list()),
        "settings_load" => tv(crate::settings::settings_load()),
        "dashboard_suggestions" => tv(crate::dashboard_suggestions(
            arg(a, "maps")?,
            arg(a, "models")?,
            arg(a, "assistantReady")?,
            arg(a, "archives")?,
            arg(a, "drillsDone")?,
            arg(a, "drillsTotal")?,
        )?),
        _ => Err("This is not available from another device.".into()),
    }
}

/// A question to the assistant. The words are sent as they come (server-sent events) and the last
/// message carries the whole reply. Only two questions run at once; the rest are asked to wait.
fn ask_sse(app: &tauri::AppHandle, req: &Request, stream: &mut TcpStream) {
    let question = serde_json::from_slice::<Value>(&req.body).ok().and_then(|v| v.get("question").and_then(|q| q.as_str()).map(|s| s.to_string())).unwrap_or_default();
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-cache\r\nConnection: close\r\nX-Accel-Buffering: no\r\n\r\n";
    let _ = stream.write_all(head.as_bytes());
    let mut out = &*stream;
    let mut event = |v: Value| {
        let _ = out.write_all(format!("data: {}\n\n", v).as_bytes());
        let _ = out.flush();
    };
    if question.trim().is_empty() {
        event(json!({ "error": "Type a question first." }));
        return;
    }
    let st = app.state::<AppState>();
    let Some(_place) = st.share.gate.enter() else {
        event(json!({ "error": "Others are asking right now. Wait a moment and ask again." }));
        return;
    };
    let r = crate::ask_run(app, question, true, |e: StreamEvent| event(json!({ "replace": e.replace, "text": e.text })));
    match r {
        Ok(dto) => event(json!({ "done": dto })),
        Err(e) => event(json!({ "error": e })),
    }
}

fn media_reply(app: &tauri::AppHandle, req: &Request, kind: &str, path: &str) -> Response {
    let host = req.header("host").unwrap_or("localhost");
    let target = if req.raw_query.is_empty() { path.to_string() } else { format!("{}?{}", path, req.raw_query) };
    let mut b = tauri::http::Request::builder().method("GET").uri(format!("http://{}{}", host, target));
    if let Some(r) = req.header("range") {
        b = b.header("range", r);
    }
    let Ok(hreq) = b.body(Vec::<u8>::new()) else { return Response::text(400, "Bad address.") };
    let resp = match kind {
        "tiles" => crate::serve_tile(&hreq),
        "music" => crate::music::serve_music(&hreq),
        _ => crate::zimview::serve(app, &hreq),
    };
    let mut out = Response { status: resp.status().as_u16(), headers: vec![], body: resp.body().clone() };
    for (k, v) in resp.headers() {
        let name = k.as_str().to_lowercase();
        if name == "content-length" || name == "connection" || name == "cache-control" {
            continue;
        }
        if let Ok(v) = v.to_str() {
            out.headers.push((k.as_str().to_string(), v.to_string()));
        }
    }
    out
}

/// The interface itself: the files the app window shows, handed to the browser as they are.
fn static_file(app: &tauri::AppHandle, path: &str) -> Response {
    let want = path.trim_start_matches('/');
    let is_page = want.is_empty() || !want.rsplit('/').next().unwrap_or("").contains('.');
    let name = if is_page { "index.html" } else { want };
    let Some(asset) = app.asset_resolver().get(name.to_string()) else {
        return Response::text(404, "Not found.");
    };
    let mime = if asset.mime_type.is_empty() { share::mime_for(name).to_string() } else { asset.mime_type.clone() };
    let mut body = asset.bytes;
    if is_page {
        // tells the interface it is in a browser on another device, so it asks the host instead of the app
        let html = String::from_utf8_lossy(&body).to_string();
        let tag = "<meta name=\"cindrix-share\" content=\"1\">";
        let html = match html.find("<head>") {
            Some(i) => format!("{}{}{}", &html[..i + 6], tag, &html[i + 6..]),
            None => format!("{}{}", tag, html),
        };
        body = html.into_bytes();
    }
    Response::new(200, &mime, body)
}
