//! The rules of sharing the library with other devices over the network: reading a web request,
//! building a reply, the PIN and the sign-in that follows, which actions a guest may use, and a small
//! limit on how many questions run at once. The server that listens on the network is in the app
//! (`src-tauri/src/share.rs`); everything here can be tested without a network.

use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::BufRead;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ---------- what a guest may do ----------

/// The actions a guest's browser may ask the host for: reading the library, nothing that writes the
/// host's own files (family page, inventory, notes, settings, saved maps, chats, progress).
pub const GUEST_COMMANDS: &[&str] = &[
    "list_cards",
    "model_status",
    "library_status",
    "product_info",
    "map_packs",
    "place_search",
    "directions",
    "route_stats",
    "zim_books",
    "zim_search_all",
    "music_status",
    "music_query",
    "music_facets",
    "music_resolve",
    "music_prepare",
    "academy_list",
    "academy_lesson",
    "plans_list",
    "drills_list",
    "shelf_list",
    "shelf_read",
    "sources_list",
    "settings_load",
    "dashboard_suggestions",
    "ask_stream",
];

pub fn guest_allowed(cmd: &str) -> bool {
    GUEST_COMMANDS.contains(&cmd)
}

// ---------- reading a request ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Request {
    pub method: String,
    pub path: String,
    /// The path as sent, still percent-encoded (the map, music and archive readers decode it themselves).
    pub raw_path: String,
    /// The part after `?`, as sent.
    pub raw_query: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn cookie(&self, name: &str) -> Option<String> {
        for line in self.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("cookie")) {
            for part in line.1.split(';') {
                if let Some((k, v)) = part.trim().split_once('=') {
                    if k == name {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }
    pub fn query_value(&self, name: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|x| u8::from_str_radix(x, 16).ok());
                match h {
                    Some(v) => { out.push(v); i += 3; }
                    None => { out.push(b'%'); i += 1; }
                }
            }
            b'+' => { out.push(b' '); i += 1; }
            c => { out.push(c); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

pub const MAX_LINE: usize = 8 * 1024;
pub const MAX_HEADERS: usize = 64;
pub const MAX_BODY: usize = 1024 * 1024;

fn read_line<R: BufRead>(r: &mut R) -> Result<String, String> {
    let mut buf = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        match r.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                if byte[0] == b'\n' { break; }
                buf.push(byte[0]);
                if buf.len() > MAX_LINE { return Err("The request line was too long.".into()); }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    while buf.last() == Some(&b'\r') { buf.pop(); }
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Read one request: the first line, the headers and, if the request says it has one, the body.
pub fn read_request<R: BufRead>(r: &mut R) -> Result<Request, String> {
    let first = read_line(r)?;
    let mut it = first.split_whitespace();
    let (Some(method), Some(target)) = (it.next(), it.next()) else { return Err("That was not a web request.".into()) };
    let (path, qs) = match target.split_once('?') { Some((p, q)) => (p, q), None => (target, "") };
    let query = qs
        .split('&')
        .filter(|s| !s.is_empty())
        .map(|kv| match kv.split_once('=') { Some((k, v)) => (url_decode(k), url_decode(v)), None => (url_decode(kv), String::new()) })
        .collect();
    let mut headers = Vec::new();
    loop {
        let l = read_line(r)?;
        if l.is_empty() { break; }
        if headers.len() >= MAX_HEADERS { return Err("The request had too many headers.".into()); }
        if let Some((k, v)) = l.split_once(':') { headers.push((k.trim().to_string(), v.trim().to_string())); }
    }
    let mut req = Request { method: method.to_uppercase(), path: url_decode(path), raw_path: path.to_string(), raw_query: qs.to_string(), query, headers, body: vec![] };
    let len: usize = req.header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len > MAX_BODY { return Err("The request was too big.".into()); }
    if len > 0 {
        let mut body = vec![0u8; len];
        r.read_exact(&mut body).map_err(|e| e.to_string())?;
        req.body = body;
    }
    Ok(req)
}

// ---------- building a reply ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: Vec<u8>) -> Response {
        Response { status, headers: vec![("Content-Type".into(), content_type.into())], body }
    }
    pub fn text(status: u16, t: &str) -> Response { Response::new(status, "text/plain; charset=utf-8", t.as_bytes().to_vec()) }
    pub fn html(status: u16, t: &str) -> Response { Response::new(status, "text/html; charset=utf-8", t.as_bytes().to_vec()) }
    pub fn json(status: u16, t: &str) -> Response { Response::new(status, "application/json; charset=utf-8", t.as_bytes().to_vec()) }
    pub fn with(mut self, k: &str, v: &str) -> Response { self.headers.push((k.into(), v.into())); self }
    /// The bytes to send: status line, headers (the length is added) and the body.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut h = format!("HTTP/1.1 {} {}\r\n", self.status, status_text(self.status));
        for (k, v) in &self.headers { h.push_str(&format!("{}: {}\r\n", k, v)); }
        h.push_str(&format!("Content-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n", self.body.len()));
        let mut out = h.into_bytes();
        out.extend_from_slice(&self.body);
        out
    }
}

pub fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// The first part of a `Range: bytes=...` header as (first byte, last byte), both inclusive.
/// `None` when the header cannot be used (the whole file is sent instead); `Err` when it asks past the end.
pub fn parse_range(h: &str, len: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(spec) = h.trim().strip_prefix("bytes=") else { return Ok(None) };
    let first = spec.split(',').next().unwrap_or("").trim();
    let Some((a, b)) = first.split_once('-') else { return Ok(None) };
    if len == 0 { return Err(()); }
    if a.is_empty() {
        // the last N bytes
        let Ok(n) = b.parse::<u64>() else { return Ok(None) };
        if n == 0 { return Err(()); }
        return Ok(Some((len.saturating_sub(n), len - 1)));
    }
    let Ok(start) = a.parse::<u64>() else { return Ok(None) };
    if start >= len { return Err(()); }
    let end = if b.is_empty() { len - 1 } else { match b.parse::<u64>() { Ok(e) => e.min(len - 1), Err(_) => return Ok(None) } };
    if end < start { return Ok(None); }
    Ok(Some((start, end)))
}

// ---------- the PIN and signing in ----------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn random_u64() -> u64 {
    let mut h = RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_usize(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.finish()
}

/// A new six-digit PIN.
pub fn new_pin() -> String { format!("{:06}", random_u64() % 1_000_000) }

/// A long random sign-in token for a browser's cookie.
pub fn new_token() -> String { format!("{:016x}{:016x}{:016x}", random_u64(), random_u64(), random_u64()) }

fn same(a: &str, b: &str) -> bool {
    // compare every byte so a wrong guess takes the same time wherever it differs
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() != b.len()) as u8;
    for i in 0..a.len().max(b.len()) { diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(1); }
    diff == 0
}

pub const TRIES: u32 = 5;
pub const LOCKOUT_SECONDS: u64 = 60;

#[derive(Debug)]
pub struct Auth {
    pin: String,
    tokens: Vec<String>,
    /// per device address: wrong tries so far, and until when (seconds) it must wait
    tries: HashMap<String, (u32, u64)>,
}

#[derive(Debug, PartialEq)]
pub enum SignIn {
    Ok(String),
    Wrong,
    /// Too many wrong tries: wait this many seconds.
    Wait(u64),
}

impl Auth {
    pub fn new(pin: &str) -> Auth { Auth { pin: pin.to_string(), tokens: vec![], tries: HashMap::new() } }
    pub fn pin(&self) -> &str { &self.pin }
    /// A new PIN also signs everyone out.
    pub fn set_pin(&mut self, pin: &str) { self.pin = pin.to_string(); self.tokens.clear(); self.tries.clear(); }
    pub fn sign_in(&mut self, who: &str, pin: &str, now: u64) -> SignIn {
        let e = self.tries.entry(who.to_string()).or_insert((0, 0));
        if e.1 > now { return SignIn::Wait(e.1 - now); }
        if same(pin.trim(), &self.pin) {
            self.tries.remove(who);
            let t = new_token();
            self.tokens.push(t.clone());
            if self.tokens.len() > 64 { self.tokens.remove(0); }
            return SignIn::Ok(t);
        }
        e.0 += 1;
        if e.0 >= TRIES { *e = (0, now + LOCKOUT_SECONDS); return SignIn::Wait(LOCKOUT_SECONDS); }
        SignIn::Wrong
    }
    pub fn valid(&self, token: &str) -> bool { !token.is_empty() && self.tokens.iter().any(|t| same(t, token)) }
    pub fn signed_in(&self) -> usize { self.tokens.len() }
}

// ---------- only a few questions at once ----------

/// Lets at most `max` things run together (questions to the assistant, for example); the rest are told to wait.
#[derive(Clone)]
pub struct Gate { used: Arc<AtomicUsize>, max: usize }
pub struct Place { used: Arc<AtomicUsize> }
impl Gate {
    pub fn new(max: usize) -> Gate { Gate { used: Arc::new(AtomicUsize::new(0)), max } }
    pub fn enter(&self) -> Option<Place> {
        let mut cur = self.used.load(Ordering::SeqCst);
        loop {
            if cur >= self.max { return None; }
            match self.used.compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Some(Place { used: self.used.clone() }),
                Err(c) => cur = c,
            }
        }
    }
}
impl Drop for Place { fn drop(&mut self) { self.used.fetch_sub(1, Ordering::SeqCst); } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn req(t: &str) -> Result<Request, String> { read_request(&mut Cursor::new(t.as_bytes().to_vec())) }

    #[test]
    fn reads_a_request_with_query_cookie_and_body() {
        let r = req("POST /api/list_cards?a=1&b=x%20y HTTP/1.1\r\nHost: h\r\nCookie: x=1; cx=tok\r\nContent-Length: 5\r\n\r\nhello").unwrap();
        assert_eq!(r.method, "POST");
        assert_eq!(r.path, "/api/list_cards");
        assert_eq!(r.raw_path, "/api/list_cards");
        assert_eq!(r.raw_query, "a=1&b=x%20y");
        assert_eq!(r.query_value("b"), Some("x y"));
        assert_eq!(r.cookie("cx").as_deref(), Some("tok"));
        assert_eq!(r.body, b"hello");
        assert_eq!(r.header("HOST"), Some("h"));
    }

    #[test]
    fn rubbish_and_oversize_requests_are_refused_not_crashed() {
        assert!(req("\r\n").is_err());
        assert!(req("GET\r\n\r\n").is_err());
        assert!(req(&format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(MAX_LINE + 10))).is_err());
        assert!(req(&format!("POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n", MAX_BODY + 1)).is_err());
        assert!(req("POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort").is_err());
        assert_eq!(url_decode("%zz%4"), "%zz%4");
        assert_eq!(url_decode("a%2Fb+c"), "a/b c");
    }

    #[test]
    fn a_reply_carries_its_length() {
        let b = Response::text(404, "no").with("X-A", "1").to_bytes();
        let t = String::from_utf8(b).unwrap();
        assert!(t.starts_with("HTTP/1.1 404 Not Found\r\n"));
        assert!(t.contains("Content-Length: 2\r\n") && t.contains("X-A: 1\r\n") && t.ends_with("\r\n\r\nno"));
    }

    #[test]
    fn ranges_for_music_and_big_pages() {
        assert_eq!(parse_range("bytes=0-9", 100), Ok(Some((0, 9))));
        assert_eq!(parse_range("bytes=90-", 100), Ok(Some((90, 99))));
        assert_eq!(parse_range("bytes=-10", 100), Ok(Some((90, 99))));
        assert_eq!(parse_range("bytes=50-500", 100), Ok(Some((50, 99))));
        assert_eq!(parse_range("bytes=100-", 100), Err(()));
        assert_eq!(parse_range("items=1-2", 100), Ok(None));
        assert_eq!(parse_range("bytes=9-3", 100), Ok(None));
    }

    #[test]
    fn mime_types_for_the_interface_and_media() {
        assert_eq!(mime_for("/assets/app.js"), "text/javascript; charset=utf-8");
        assert_eq!(mime_for("a.MP3"), "audio/mpeg");
        assert_eq!(mime_for("noext"), "application/octet-stream");
    }

    #[test]
    fn the_right_pin_signs_in_and_wrong_ones_lock_out() {
        let mut a = Auth::new("123456");
        let t = match a.sign_in("1.2.3.4", " 123456 ", 100) { SignIn::Ok(t) => t, o => panic!("{:?}", o) };
        assert!(a.valid(&t) && !a.valid("") && !a.valid("nope"));
        for _ in 0..TRIES - 1 { assert_eq!(a.sign_in("5.5.5.5", "000000", 100), SignIn::Wrong); }
        assert_eq!(a.sign_in("5.5.5.5", "000000", 100), SignIn::Wait(LOCKOUT_SECONDS));
        // even the right PIN waits while locked, other devices are not affected, and it ends
        assert_eq!(a.sign_in("5.5.5.5", "123456", 130), SignIn::Wait(30));
        assert!(matches!(a.sign_in("6.6.6.6", "123456", 130), SignIn::Ok(_)));
        assert!(matches!(a.sign_in("5.5.5.5", "123456", 161), SignIn::Ok(_)));
    }

    #[test]
    fn a_new_pin_signs_everyone_out() {
        let mut a = Auth::new("111111");
        let SignIn::Ok(t) = a.sign_in("a", "111111", 0) else { panic!() };
        a.set_pin("222222");
        assert!(!a.valid(&t));
        assert_eq!(a.sign_in("a", "111111", 0), SignIn::Wrong);
    }

    #[test]
    fn pins_and_tokens_look_right_and_differ() {
        let p = new_pin();
        assert_eq!(p.len(), 6);
        assert!(p.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(new_token().len(), 48);
        assert_ne!(new_token(), new_token());
    }

    #[test]
    fn guests_read_the_library_but_cannot_write_the_hosts_files() {
        for c in ["list_cards", "shelf_read", "ask_stream", "zim_search_all", "academy_lesson"] { assert!(guest_allowed(c), "{}", c); }
        for c in ["family_save", "inventory_save_item", "settings_save", "profile_load", "profile_save", "archive_remove", "shelf_add",
                  "chat_read", "chat_save", "library_share", "pick_folder", "phone_send", "map_save", "academy_save", "plan_tick", "crawl_start"] {
            assert!(!guest_allowed(c), "{}", c);
        }
    }

    #[test]
    fn the_gate_lets_two_in_and_frees_a_place_when_one_leaves() {
        let g = Gate::new(2);
        let a = g.enter().unwrap();
        let _b = g.enter().unwrap();
        assert!(g.enter().is_none());
        drop(a);
        assert!(g.enter().is_some());
    }
}
