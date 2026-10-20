//! The in-app archive reader's side of the program. The reader screen shows pages through the
//! `cindrix-zim` address, which passes a few parts of the local library server (kiwix-serve) to
//! the web view, puts a small script into each page (so the reader knows which page is open and
//! can stop links leaving the library), and nothing else. See `core/src/kiwix.rs`.

use super::{root, AppState};
use cindrix_core::{fsio, kiwix};
use serde::Serialize;
use tauri::Manager;

fn reply(status: u16, ctype: &str, final_path: Option<&str>, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    let mut b = tauri::http::Response::builder()
        .status(status)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Expose-Headers", "X-Final-Path, Content-Type")
        .header("Cache-Control", "no-store");
    if !ctype.is_empty() {
        b = b.header("Content-Type", ctype);
    }
    if let Some(p) = final_path {
        b = b.header("X-Final-Path", p);
    }
    b.body(body).unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

/// Answer one request from the reader. Runs on a worker thread.
pub fn serve(app: &tauri::AppHandle, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    if *req.method() == tauri::http::Method::OPTIONS {
        return reply(204, "", None, Vec::new());
    }
    if *req.method() != tauri::http::Method::GET {
        return reply(405, "", None, Vec::new());
    }
    let st = app.state::<AppState>();
    let port = *st.kiwix_port.lock().unwrap();
    let Some(port) = port else {
        return reply(503, "text/plain", None, b"The library is still starting.".to_vec());
    };
    let uri = req.uri();
    let path = uri.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_else(|| uri.path().to_string());
    if !kiwix::proxy_path_ok(&path) {
        return reply(404, "", None, Vec::new());
    }
    let (r, final_path) = match kiwix::follow(port, &path) {
        Ok(x) => x,
        Err(e) => return reply(502, "text/plain", None, e.into_bytes()),
    };
    let ctype = if r.content_type.is_empty() { "application/octet-stream".to_string() } else { r.content_type.clone() };
    let is_page = r.status == 200 && (final_path.starts_with("/content/") || final_path.starts_with("/search?")) && ctype.to_ascii_lowercase().starts_with("text/html");
    if is_page {
        if let Ok(html) = String::from_utf8(r.body.clone()) {
            let mut page = kiwix::inject_reader_script(&html);
            if final_path != path {
                // reached through a redirect: make the page's own links and pictures resolve
                let origin = format!("{}://{}", uri.scheme_str().unwrap_or("http"), uri.authority().map(|a| a.as_str()).unwrap_or("localhost"));
                page = kiwix::inject_head(&page, &kiwix::base_tag(&format!("{}{}", origin, final_path)));
            }
            return reply(200, &ctype, Some(&final_path), page.into_bytes());
        }
    }
    reply(r.status, &ctype, Some(&final_path), r.body)
}

#[derive(Serialize)]
pub struct BookDto {
    name: String,
    title: String,
}

/// The archives the library server is serving, with the names used in their addresses.
#[tauri::command]
pub fn zim_books(state: tauri::State<'_, AppState>) -> Vec<BookDto> {
    let Some(port) = *state.kiwix_port.lock().unwrap() else { return vec![] };
    match kiwix::get_full(port, "/catalog/v2/entries?count=200") {
        Ok(r) if r.status == 200 => kiwix::parse_catalog(&String::from_utf8_lossy(&r.body)).into_iter().map(|b| BookDto { name: b.name, title: b.title }).collect(),
        _ => vec![],
    }
}

fn bookmarks_file() -> std::path::PathBuf {
    root().join("user").join("zim-bookmarks.json")
}

/// Bookmarks and recent pages. A JSON object, or "{}".
#[tauri::command]
pub fn zim_bookmarks_load() -> String {
    match std::fs::read_to_string(bookmarks_file()) {
        Ok(s) if s.trim_start().starts_with('{') && s.len() <= 2_000_000 => s,
        _ => "{}".to_string(),
    }
}

#[tauri::command]
pub fn zim_bookmarks_save(json: String) -> Result<(), String> {
    if !json.trim_start().starts_with('{') || !json.trim_end().ends_with('}') || json.len() > 2_000_000 {
        return Err("That is not a bookmarks list.".into());
    }
    fsio::write_atomic(&bookmarks_file(), json.as_bytes()).map_err(|e| e.to_string())
}
