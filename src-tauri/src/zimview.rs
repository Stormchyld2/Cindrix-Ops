//! The in-app archive reader's side of the program. The reader screen shows pages through the
//! `cindrix-zim` address, which passes a few parts of the local library server (kiwix-serve) to
//! the web view, puts a small script into each page (so the reader knows which page is open and
//! can stop links leaving the library), and nothing else. See `core/src/kiwix.rs`.

use super::{library_dir, root, AppState};
use cindrix_core::library::{self, Library};
use cindrix_core::{fsio, kiwix};
use std::sync::{Arc, Mutex};
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

static LIB: Mutex<Option<(String, Arc<Library>)>> = Mutex::new(None);

/// The archives on the drive, read directly. Reopened only when a file is added, removed or replaced.
pub fn library() -> Arc<Library> {
    let files = kiwix::zim_files(&library_dir());
    let sig = library::signature(&files);
    let mut g = LIB.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((s, l)) = g.as_ref() {
        if *s == sig {
            return l.clone();
        }
    }
    let l = Arc::new(Library::open(&files));
    *g = Some((sig, l.clone()));
    l
}

fn query_param(q: &str, key: &str) -> Option<String> {
    for part in q.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return library::decode_path(&v.replace('+', " "));
            }
        }
    }
    None
}

fn html_reply(html: String, origin: &str, base_path: Option<&str>) -> tauri::http::Response<Vec<u8>> {
    let mut page = kiwix::inject_reader_script(&html);
    if let Some(p) = base_path {
        page = kiwix::inject_head(&page, &kiwix::base_tag(&format!("{}{}", origin, p)));
    }
    reply(200, "text/html; charset=utf-8", None, page.into_bytes())
}

/// Answer one request from the reader. Runs on a worker thread. Pages, pictures and suggestions
/// come straight from the archives; full-text search and Kiwix's style files come from
/// `kiwix-serve` when it is running.
pub fn serve(app: &tauri::AppHandle, req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    if *req.method() == tauri::http::Method::OPTIONS {
        return reply(204, "", None, Vec::new());
    }
    if *req.method() != tauri::http::Method::GET {
        return reply(405, "", None, Vec::new());
    }
    let uri = req.uri();
    let origin = format!("{}://{}", uri.scheme_str().unwrap_or("http"), uri.authority().map(|a| a.as_str()).unwrap_or("localhost"));
    let path = uri.path();
    let query = uri.query().unwrap_or("");
    let kport = *app.state::<AppState>().kiwix_port.lock().unwrap();

    if let Some(rest) = path.strip_prefix("/content/") {
        let (book, addr) = rest.split_once('/').unwrap_or((rest, ""));
        let (Some(book), Some(addr)) = (library::decode_path(book), library::decode_path(addr)) else {
            return reply(400, "", None, Vec::new());
        };
        let lib = library();
        return match lib.page(&book, &addr) {
            Ok(Some(p)) => {
                let is_html = p.mime.to_ascii_lowercase().starts_with("text/html");
                let ctype = if p.mime.starts_with("text/") && !p.mime.contains("charset") { format!("{}; charset=utf-8", p.mime) } else { p.mime.clone() };
                if is_html {
                    if let Ok(html) = String::from_utf8(p.data.clone()) {
                        // the start page and redirected pages: make links resolve from where the page really is
                        let real = lib.book(&book).map(|b| format!("/content/{}/{}", library::encode_path(&book), library::encode_path(&b.archive.address_of(p.ns, &p.url))));
                        let base = real.filter(|r| r.as_str() != path);
                        return html_reply(html, &origin, base.as_deref());
                    }
                }
                reply(200, &ctype, None, p.data)
            }
            Ok(None) => reply(404, "text/plain", None, b"That page is not in this archive.".to_vec()),
            Err(e) => reply(500, "text/plain", None, e.to_string().into_bytes()),
        };
    }
    if path == "/suggest" {
        let (Some(book), Some(term)) = (query_param(query, "content"), query_param(query, "term")) else {
            return reply(400, "", None, Vec::new());
        };
        return reply(200, "application/json; charset=utf-8", None, library().suggest_json(&book, &term).into_bytes());
    }
    if path == "/search" {
        if let Some(port) = kport {
            // full-text search, from Kiwix's index
            let pq = format!("{}?{}", path, query);
            if kiwix::proxy_path_ok(&pq) {
                if let Ok((r, fp)) = kiwix::follow(port, &pq) {
                    let ctype = if r.content_type.is_empty() { "text/html".to_string() } else { r.content_type.clone() };
                    if r.status == 200 && ctype.to_ascii_lowercase().starts_with("text/html") {
                        if let Ok(html) = String::from_utf8(r.body.clone()) {
                            return html_reply(html, &origin, if fp != pq { Some(fp.as_str()) } else { None });
                        }
                    }
                    return reply(r.status, &ctype, None, r.body);
                }
            }
        }
        let (Some(book), Some(term)) = (query_param(query, "content"), query_param(query, "pattern")) else {
            return reply(400, "", None, Vec::new());
        };
        return html_reply(library().results_page(&book, &term), &origin, None);
    }
    if path.starts_with("/skin/") {
        if let Some(port) = kport {
            if let Ok((r, _)) = kiwix::follow(port, path) {
                let ctype = if r.content_type.is_empty() { "application/octet-stream".to_string() } else { r.content_type.clone() };
                return reply(r.status, &ctype, None, r.body);
            }
        }
    }
    reply(404, "", None, Vec::new())
}

#[derive(Serialize)]
pub struct BookDto {
    name: String,
    title: String,
}

/// The archives on the drive, with the names used in their addresses.
#[tauri::command]
pub fn zim_books() -> Vec<BookDto> {
    library().books.iter().map(|b| BookDto { name: b.name.clone(), title: b.title.clone() }).collect()
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
