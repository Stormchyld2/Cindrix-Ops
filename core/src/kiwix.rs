//! The big offline library (Wikipedia, medical guides, repair manuals ...) lives in Kiwix ZIM files.
//! The `kiwix-serve` program serves them on this computer only (127.0.0.1); this module starts it,
//! searches it and turns the matching articles into plain-text passages for the answer path.

use crate::retrieval::{self, Passage};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const SHARE_PORT: u16 = 8787;

pub struct KiwixHost {
    child: Child,
    pub port: u16,
}

/// All .zim files in a folder and its subfolders (up to three levels down), whatever the case of
/// the extension. Half-finished downloads (`.zim.part`) do not end in `.zim` and are left out.
pub fn zim_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            // `file_type` does not follow links, so a link back up the tree cannot loop
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                if depth > 0 {
                    walk(&p, depth - 1, out);
                }
            } else if ft.is_file() && p.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("zim")).unwrap_or(false) {
                out.push(p);
            }
        }
    }
    let mut v = Vec::new();
    walk(dir, 3, &mut v);
    v.sort();
    v
}

/// `share` false: only this computer can connect (127.0.0.1). `share` true: other devices on the
/// same network can read the library too (they cannot change anything).
pub fn args(zims: &[PathBuf], port: u16, share: bool) -> Vec<String> {
    let addr = if share { "0.0.0.0" } else { "127.0.0.1" };
    let mut a = vec!["--address".to_string(), addr.into(), "--port".into(), port.to_string()];
    a.extend(zims.iter().map(|z| z.to_string_lossy().to_string()));
    a
}

impl KiwixHost {
    pub fn start(engine: &Path, zims: &[PathBuf], share: bool) -> Result<KiwixHost, String> {
        if zims.is_empty() {
            return Err("no .zim library files found".into());
        }
        if !engine.exists() {
            return Err(format!("the library server was not found at {}", engine.display()));
        }
        // A steady port when sharing, so other devices can bookmark the address.
        let port = if share && std::net::TcpListener::bind(("0.0.0.0", SHARE_PORT)).is_ok() { SHARE_PORT } else { crate::modelhost::free_port()? };
        let mut cmd = Command::new(engine);
        cmd.args(args(zims, port, share)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(dir) = engine.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let child = cmd.spawn().map_err(|e| format!("could not start the library server: {}", e))?;
        Ok(KiwixHost { child, port })
    }

    pub fn wait_ready(&mut self, timeout: Duration) -> Result<(), String> {
        let t0 = Instant::now();
        loop {
            if get(self.port, "/").map(|(s, _)| s == 200).unwrap_or(false) {
                return Ok(());
            }
            if let Ok(Some(st)) = self.child.try_wait() {
                return Err(format!("the library server stopped while starting ({})", st));
            }
            if t0.elapsed() > timeout {
                return Err("the library server took too long to start".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for KiwixHost {
    fn drop(&mut self) {
        self.stop();
    }
}

/// This computer's address on the local network, if it has one (for telling other devices where to connect).
/// Nothing is sent: the socket only asks the system which network address it would use.
pub fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() { None } else { Some(ip.to_string()) }
}

/// Minimal HTTP GET to 127.0.0.1. Returns (status, body).
pub fn get(port: u16, path: &str) -> Result<(u16, Vec<u8>), String> {
    let addr = ("127.0.0.1", port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok();
    let req = format!("GET {} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", path);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("bad response")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status: u16 = head.split_whitespace().nth(1).and_then(|x| x.parse().ok()).ok_or("bad status")?;
    Ok((status, raw[split + 4..].to_vec()))
}

pub fn url_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(&s[a..b])
}

pub fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

/// Percent-encode only the bytes that cannot appear in a request line (spaces, control
/// characters, anything outside printable ASCII). Everything else, including any `%XX`
/// already there, is left alone.
fn encode_unsafe(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if (0x21..=0x7e).contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// The text inside `<![CDATA[ ... ]]>`, if that is how the value was written.
fn strip_cdata(s: &str) -> Option<&str> {
    s.trim().strip_prefix("<![CDATA[").and_then(|x| x.strip_suffix("]]>"))
}

/// An XML text value: CDATA is taken as it is, anything else has its entities decoded.
fn xml_text(s: &str) -> String {
    match strip_cdata(s) {
        Some(x) => x.trim().to_string(),
        None => decode_entities(s).trim().to_string(),
    }
}

/// The part of a result link the library server is asked for, always starting with `/`.
/// Depending on the kiwix-serve version a link comes as `/content/book/A/Page` or as a full
/// `http://host:port/content/book/A/Page`; either way only the path is kept (so the program
/// only ever talks to its own server), and characters that cannot sit in a request line are encoded.
pub fn link_path(link: &str) -> String {
    let text = xml_text(link);
    let path: String = if let Some(rest) = text.strip_prefix("http://").or_else(|| text.strip_prefix("https://")) {
        match rest.find('/') {
            Some(i) => rest[i..].to_string(),
            None => "/".to_string(),
        }
    } else if text.starts_with('/') {
        text.clone()
    } else {
        format!("/{}", text)
    };
    encode_unsafe(&path)
}

/// (title, link) for each result in kiwix-serve's XML search output. Links are paths
/// (see `link_path`). Titles and links may be plain text or CDATA.
pub fn parse_search(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<item>") {
        let after = &rest[i + 6..];
        let end = after.find("</item>").unwrap_or(after.len());
        let item = &after[..end];
        if let (Some(t), Some(l)) = (between(item, "<title>", "</title>"), between(item, "<link>", "</link>")) {
            let link = link_path(l);
            if link.len() > 1 {
                out.push((xml_text(t), link));
            }
        }
        rest = &after[end..];
    }
    out
}

/// The question as search words: lower case, without little words like "how" and "the",
/// each word once, in the order asked. A whole sentence is a poor full-text query.
pub fn keywords(question: &str) -> Vec<String> {
    // a few more little words than the answer path's list: they only hurt a full-text query
    const LITTLE: &[&str] = &[
        "by", "from", "as", "so", "but", "not", "this", "that", "there", "was", "were", "has", "have", "had", "will", "would", "could", "your", "our", "they", "them", "than", "then", "into", "about", "which", "who", "where", "why",
    ];
    let mut seen: Vec<String> = Vec::new();
    for t in retrieval::tokens(question) {
        if !LITTLE.contains(&t.as_str()) && !seen.contains(&t) {
            seen.push(t);
        }
    }
    seen
}

/// The top-level `{ ... }` objects of a JSON array. String contents (which may hold braces
/// or escaped quotes) are skipped properly.
fn json_objects(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start, mut in_str, mut esc) = (Vec::new(), 0i32, 0usize, false, false);
    for (i, &c) in s.as_bytes().iter().enumerate() {
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    out.push(&s[start..=i]);
                }
            }
            _ => {}
        }
    }
    out
}

/// Decode a JSON string body: `s` starts just after the opening quote and the decoding stops at the closing one.
fn json_unescape(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let hex4 = |at: usize| -> Option<u32> {
        let h: String = chars.get(at..at + 4)?.iter().collect();
        u32::from_str_radix(&h, 16).ok()
    };
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '"' => break,
            '\\' => {
                i += 1;
                match chars.get(i) {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('b') => out.push('\u{8}'),
                    Some('f') => out.push('\u{c}'),
                    Some('u') => {
                        if let Some(n) = hex4(i + 1) {
                            i += 4;
                            let mut code = n;
                            // a high surrogate is followed by `\uXXXX`, its low half
                            if (0xD800..0xDC00).contains(&n) && chars.get(i + 1) == Some(&'\\') && chars.get(i + 2) == Some(&'u') {
                                if let Some(lo) = hex4(i + 3).filter(|l| (0xDC00..0xE000).contains(l)) {
                                    code = 0x10000 + ((n - 0xD800) << 10) + (lo - 0xDC00);
                                    i += 6;
                                }
                            }
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                    }
                    Some(c) => out.push(*c), // \" \\ \/ and anything else stand for themselves
                    None => break,
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// The string value of `"key": "..."` inside one JSON object, if it has one.
fn json_string(obj: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let mut from = 0;
    while let Some(i) = obj[from..].find(&pat) {
        let after = from + i + pat.len();
        if let Some(r) = obj[after..].trim_start().strip_prefix(':') {
            return r.trim_start().strip_prefix('"').map(json_unescape);
        }
        from = after;
    }
    None
}

/// Plain text from a suggestion label. kiwix-serve marks the matching part with `<b>` tags, and
/// sends them escaped (`&lt;b&gt;`), so entities are decoded first and then real tags are dropped.
/// A `<` that does not start a tag (as in "a < b") is kept.
fn plain_label(s: &str) -> String {
    let t = decode_entities(s);
    let mut out = String::new();
    let mut rest = t.as_str();
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let looks_like_tag = after.chars().next().map(|c| c.is_ascii_alphabetic() || c == '/').unwrap_or(false);
        match (looks_like_tag, after.find('>')) {
            (true, Some(j)) => rest = &after[j + 1..],
            _ => {
                out.push('<');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// (title, link) for each title match in kiwix-serve's `/suggest` JSON. This is the fallback
/// when full-text search is not available (an archive with no search index answers a search with an error).
pub fn parse_suggest(json: &str, book: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for obj in json_objects(json) {
        if json_string(obj, "kind").as_deref() == Some("pattern") {
            continue; // the "containing ..." entry, not an article
        }
        let Some(path) = json_string(obj, "path").map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) else { continue };
        // `value` is the plain title; `label` is the same text with the match highlighted
        let title = json_string(obj, "value")
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| json_string(obj, "label").map(|l| plain_label(&l)).filter(|l| !l.is_empty()))
            .unwrap_or_else(|| path.clone());
        // Some versions give the whole address, others the path inside the archive.
        let link = if path.starts_with("/content/") || path.starts_with("http") { link_path(&path) } else { link_path(&format!("/content/{}/{}", book, path.trim_start_matches('/'))) };
        if link.len() > 1 {
            out.push((title.trim().to_string(), link));
        }
    }
    out
}

/// Plain text from an article's HTML: drops scripts, styles and tags, keeps paragraph breaks.
pub fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0;
    let b = html.as_bytes();
    while i < b.len() {
        if b[i] == b'<' {
            let skip_to = ["script", "style"].iter().find_map(|tag| {
                if lower[i..].starts_with(&format!("<{}", tag)) {
                    lower[i..].find(&format!("</{}>", tag)).map(|e| i + e + tag.len() + 3)
                } else {
                    None
                }
            });
            if let Some(j) = skip_to {
                i = j;
                continue;
            }
            let close = match html[i..].find('>') {
                Some(c) => i + c,
                None => break,
            };
            let tag = lower[i + 1..close].trim_start_matches('/');
            let name: String = tag.chars().take_while(|c| c.is_alphanumeric()).collect();
            if matches!(name.as_str(), "p" | "br" | "div" | "li" | "h1" | "h2" | "h3" | "h4" | "tr") {
                out.push('\n');
            }
            i = close + 1;
        } else {
            let next = html[i..].find('<').map(|x| i + x).unwrap_or(html.len());
            out.push_str(&html[i..next]);
            i = next;
        }
    }
    let text = decode_entities(&out);
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        if !l.is_empty() {
            lines.push(l);
        }
    }
    lines.join("\n\n")
}

/// The part of an article that best matches the question: paragraphs are grouped into chunks of
/// about 1200 characters, each chunk is scored against the question, and the best one is returned.
/// A long article is therefore not cut off at its introduction when the answer is further down.
pub fn best_chunk(title: &str, text: &str, question: &str) -> String {
    let q = retrieval::tokens(question);
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for para in text.split("\n\n") {
        if !cur.is_empty() && cur.len() + para.len() > 1200 {
            chunks.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(para);
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    let mut best = (f64::MIN, 0usize);
    for (i, c) in chunks.iter().enumerate() {
        // the first chunk gets a small bonus: introductions are usually the right summary
        let sc = retrieval::score(&q, title, c) + if i == 0 { 0.5 } else { 0.0 };
        if sc > best.0 {
            best = (sc, i);
        }
    }
    let chosen = chunks.get(best.1).cloned().unwrap_or_default();
    chosen.chars().take(1500).collect()
}

/// Search the archives and return up to `limit` passages. Each is the best-matching part of its article.
/// Names of every archive the library server holds.
fn book_names(port: u16) -> Vec<String> {
    match get(port, "/catalog/v2/entries?count=500") {
        Ok((200, body)) => parse_catalog(&String::from_utf8_lossy(&body)).into_iter().map(|b| b.name).collect(),
        _ => vec![],
    }
}

/// Search EVERY archive on the drive, not just one: each book is asked on its own, and the
/// results are mixed best-first so that a small archive (a saved website, a repair manual) is
/// not buried by Wikipedia. Falls back to one all-library search if the book list is unavailable.
pub fn search(port: u16, question: &str, limit: usize) -> Vec<Passage> {
    let names = book_names(port);
    let keys = {
        let k = keywords(question);
        if k.is_empty() { retrieval::tokens(question) } else { k }
    };
    // No book list: one search over the whole library (no title search, which needs a book name).
    let targets: Vec<Option<String>> = if names.is_empty() { vec![None] } else { names.into_iter().map(Some).collect() };
    // candidates: (book number, rank inside its book, title, link)
    let mut cands: Vec<(usize, usize, String, String)> = Vec::new();
    for (bi, target) in targets.iter().enumerate() {
        for (rank, (title, link)) in book_hits(port, target.as_deref(), question, limit).into_iter().enumerate() {
            if !cands.iter().any(|c| c.3 == link) {
                cands.push((bi, rank, title, link));
            }
        }
    }
    cands.sort_by_key(|c| (c.1, c.0)); // everyone's best first, so the fetch cap never starves a small archive
    let mut scored: Vec<(usize, Passage)> = Vec::new();
    for (bi, rank, title, link) in cands.into_iter().take(MAX_FETCH) {
        // `follow` also refuses any address outside the library and follows kiwix-serve's redirects
        let Ok((reply, path)) = follow(port, &link) else { continue };
        if reply.status != 200 {
            continue;
        }
        let text = html_to_text(&String::from_utf8_lossy(&reply.body));
        if text.len() < 80 {
            continue;
        }
        let text = best_chunk(&title, &text, question);
        // /content/<book>/A/<article>
        let book = path.trim_start_matches("/content/").split('/').next().unwrap_or("library").replace('_', " ");
        // Our own measure of how well the page answers the question (the search engine's order only
        // says which pages contain the words), kept in the range 3 to 8.
        let score = 3.0 + 0.5 * relevance(&keys, &title, &text).min(10.0) - 0.01 * rank as f64;
        scored.push((bi, Passage { source: book, heading: title, text, score }));
    }
    scored.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap_or(std::cmp::Ordering::Equal));
    // The best page of every archive first, so a small archive is not buried by Wikipedia; then the best of the rest.
    let (mut chosen, mut rest, mut seen): (Vec<Passage>, Vec<Passage>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
    for (bi, p) in scored {
        if seen.contains(&bi) {
            rest.push(p);
        } else {
            seen.push(bi);
            chosen.push(p);
        }
    }
    chosen.extend(rest);
    chosen.truncate(limit);
    chosen
}

/// Most pages fetched for one question, across all archives.
const MAX_FETCH: usize = 24;

/// Language codes used in the addresses of translated pages.
const LANGS: &[&str] = &[
    "ar", "bg", "bn", "ca", "cs", "da", "de", "el", "es", "fa", "fi", "fr", "he", "hi", "hr", "ht", "hu", "id", "it", "ja", "ko", "ms", "nl", "no", "pl", "pt", "ro", "ru", "sk", "sr", "sv", "sw", "ta", "th", "tl", "tr", "uk", "ur", "vi", "zh",
];

/// True for the address of a translated page: a language code as a path part after the first,
/// like `.../Anaphylaxis_(peds)/de` (a wiki sub-page) or `.../www.ready.gov/ar/node/6015` (a
/// site's language folder). A real archive holds many of these, and a German or Arabic page
/// is no help to a reader (or a small model) working in English.
pub fn is_translation(link: &str) -> bool {
    let path = link.split(['?', '#']).next().unwrap_or("");
    let rest = path.strip_prefix("/content/").unwrap_or(path);
    // the first part is the book; what follows is the page's own address
    rest.split('/').skip(1).enumerate().any(|(i, part)| {
        let base = part.split('-').next().unwrap_or("");
        let tail = &part[base.len()..];
        i >= 1 && LANGS.contains(&base) && (tail.is_empty() || (tail.len() <= 5 && tail[1..].bytes().all(|b| b.is_ascii_lowercase())))
    })
}

/// A word matches a key word when they are the same, or one begins with the other (so "burns"
/// and "treating" match "burn" and "treat"). Short words must match exactly.
fn loosely(word: &str, key: &str) -> bool {
    word == key || (key.chars().count() >= 4 && word.starts_with(key)) || (word.chars().count() >= 4 && key.starts_with(word))
}

/// How well a page answers the question: 3 for each key word in its title, and up to 3 for each
/// key word in the part of the text that was picked.
fn relevance(keys: &[String], title: &str, text: &str) -> f64 {
    let (ht, tt) = (retrieval::tokens(title), retrieval::tokens(text));
    let mut s = 0.0;
    for k in keys {
        if ht.iter().any(|w| loosely(w, k)) {
            s += 3.0;
        }
        s += (tt.iter().filter(|w| loosely(w, k)).count() as f64).min(3.0);
    }
    s
}

fn merge(into: &mut Vec<(String, String)>, list: Vec<(String, String)>) {
    for (title, link) in list {
        if !into.iter().any(|f| f.1 == link) {
            into.push((title, link));
        }
    }
}

/// Candidate (title, link) matches for the question in one book (or in the whole library when
/// `book` is None), up to three times `limit`:
/// - full-text search with the question's key words (then with just its two longest words, if
///   the first finds nothing: a long query can match no page),
/// - title search (`/suggest`), one key word at a time. It adds the page that is simply *called*
///   "Burns" when the question is about treating a burn, and it is the only way into an archive
///   saved without a search index, where full-text search answers with an error.
/// The two lists are interleaved. Translated pages come last, or are dropped when there are
/// original pages.
fn book_hits(port: u16, book: Option<&str>, question: &str, limit: usize) -> Vec<(String, String)> {
    let want = (limit * 3).max(4);
    let kw = keywords(question);
    let mut by_length = kw.clone();
    by_length.sort_by(|a, b| b.chars().count().cmp(&a.chars().count())); // stable: ties keep the order asked
    let mut patterns: Vec<String> = Vec::new();
    if kw.is_empty() {
        patterns.push(question.trim().to_string());
    } else {
        patterns.push(kw.iter().take(6).cloned().collect::<Vec<_>>().join(" "));
        if kw.len() > 2 {
            patterns.push(by_length.iter().take(2).cloned().collect::<Vec<_>>().join(" "));
        }
    }
    let mut full_text: Vec<(String, String)> = Vec::new();
    for pat in patterns.iter().filter(|p| !p.is_empty()) {
        let path = match book {
            Some(n) => format!("/search?content={}&pattern={}&pageLength={}&format=xml", url_encode(n), url_encode(pat), want * 2),
            None => format!("/search?pattern={}&pageLength={}&format=xml", url_encode(pat), want * 2),
        };
        if let Ok((200, body)) = get(port, &path) {
            let found = parse_search(&String::from_utf8_lossy(&body));
            if !found.is_empty() {
                merge(&mut full_text, found);
                break;
            }
        }
    }
    let mut titles: Vec<(String, String)> = Vec::new();
    if let Some(n) = book {
        for term in by_length.iter().take(3) {
            let path = format!("/suggest?content={}&term={}&count={}", url_encode(n), url_encode(term), want);
            if let Ok((200, body)) = get(port, &path) {
                merge(&mut titles, parse_suggest(&String::from_utf8_lossy(&body), n));
            }
        }
    }
    let mut all: Vec<(String, String)> = Vec::new();
    let (mut a, mut t) = (full_text.into_iter(), titles.into_iter());
    loop {
        let (x, y) = (a.next(), t.next());
        if x.is_none() && y.is_none() {
            break;
        }
        merge(&mut all, x.into_iter().chain(y).collect());
    }
    let (originals, translations): (Vec<_>, Vec<_>) = all.into_iter().partition(|h| !is_translation(&h.1));
    let mut out = if originals.is_empty() { translations } else { originals };
    out.truncate(want);
    out
}

// ---------- the in-app reader: a safe window onto kiwix-serve ----------

/// What kiwix-serve answered, with the headers the reader needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

/// Biggest page or picture handed to the reader.
pub const MAX_BODY: u64 = 64 * 1024 * 1024;

fn parse_head(head: &str) -> Option<(u16, String, Option<String>)> {
    let mut lines = head.lines();
    let status: u16 = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let (mut ct, mut loc) = (String::new(), None);
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-type" => ct = v.trim().to_string(),
                "location" => loc = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    Some((status, ct, loc))
}

/// Like `get`, but keeps the content type and any redirect, and caps the size.
pub fn get_full(port: u16, path: &str) -> Result<Reply, String> {
    let addr = ("127.0.0.1", port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(20))).ok();
    let req = format!("GET {} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", path);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.take(MAX_BODY + 65536).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("bad response")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let (status, content_type, location) = parse_head(&head).ok_or("bad status")?;
    let body = raw[split + 4..].to_vec();
    if body.len() as u64 > MAX_BODY {
        return Err("that file is too large to show here".into());
    }
    Ok(Reply { status, content_type, location, body })
}

/// Only these parts of the library server may be reached from the reader, and only with plain
/// printable characters (so a request line cannot be split or smuggled).
pub fn proxy_path_ok(path: &str) -> bool {
    const ALLOWED: [&str; 5] = ["/content/", "/skin/", "/suggest?", "/search?", "/catalog/v2/"];
    path.len() <= 2048
        && path.bytes().all(|b| (0x21..=0x7e).contains(&b))
        && !path.contains("/../")
        && !path.contains("%2e%2e")
        && !path.contains("%2E%2E")
        && ALLOWED.iter().any(|p| path.starts_with(p))
}

/// Fetch a path, following up to four redirects that stay on this server. Returns the reply and
/// the path it finally came from (kiwix-serve redirects a book to its main page).
pub fn follow(port: u16, path: &str) -> Result<(Reply, String), String> {
    let mut cur = path.to_string();
    for _ in 0..5 {
        if !proxy_path_ok(&cur) {
            return Err("that address is not part of the library".into());
        }
        let r = get_full(port, &cur)?;
        if (300..400).contains(&r.status) {
            let Some(loc) = r.location else { return Ok((Reply { location: None, ..r }, cur)) };
            let own = format!("http://127.0.0.1:{}", port);
            let next = if let Some(rest) = loc.strip_prefix(&own) { rest.to_string() } else { loc };
            if next.starts_with('/') {
                cur = next;
                continue;
            }
            return Err("the library tried to send you to another site".into());
        }
        return Ok((r, cur));
    }
    Err("too many redirects".into())
}

/// One archive known to kiwix-serve.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    /// The name used in addresses: `/content/<name>/...`
    pub name: String,
    pub title: String,
}

/// Read kiwix-serve's catalog (`/catalog/v2/entries`) for the books it serves.
pub fn parse_catalog(xml: &str) -> Vec<Book> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<entry>") {
        let after = &rest[i + 7..];
        let end = after.find("</entry>").unwrap_or(after.len());
        let e = &after[..end];
        // the address in the text/html link is the dependable source of the name
        let mut name = None;
        let mut scan = e;
        while let Some(l) = scan.find("<link ") {
            let tag_end = scan[l..].find('>').map(|x| l + x).unwrap_or(scan.len());
            let tag = &scan[l..tag_end];
            if tag.contains("text/html") {
                if let Some(h) = between(tag, "href=\"", "\"") {
                    if let Some(n) = h.strip_prefix("/content/") {
                        name = Some(n.trim_end_matches('/').split('/').next().unwrap_or("").to_string());
                        break;
                    }
                }
            }
            scan = &scan[tag_end.min(scan.len())..];
        }
        let name = name.or_else(|| between(e, "<name>", "</name>").map(|n| n.trim().to_string()));
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            let title = between(e, "<title>", "</title>").map(|t| decode_entities(t).trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| name.replace('_', " "));
            out.push(Book { name, title });
        }
        rest = &after[end..];
    }
    out
}

/// The small script put into every page shown in the reader. It tells the reader which page
/// is open (for Back, Forward and bookmarks), keeps links that leave the library from opening,
/// and handles the text-size setting.
pub const READER_SCRIPT: &str = "<script>(function(){\
var O=location.origin;\
function tell(){try{parent.postMessage({cx:'page',url:document.baseURI,title:document.title||''},'*')}catch(e){}}\
document.addEventListener('click',function(e){var a=e.target&&e.target.closest?e.target.closest('a[href]'):null;if(!a)return;var h=a.href;\
if(/^(https?|ftp|mailto|tel):/i.test(h)&&h.indexOf(O)!==0){e.preventDefault();try{parent.postMessage({cx:'external',url:h},'*')}catch(x){}}},true);\
addEventListener('message',function(e){var d=e.data;if(d&&d.cx==='zoom'){document.documentElement.style.zoom=d.v}});\
if(document.readyState==='complete')tell();else addEventListener('load',tell);\
})();</script>";

/// Put a snippet into an HTML page, right after `<head>` (or at the very start).
pub fn inject_head(html: &str, snippet: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(i) = lower.find("<head") {
        if let Some(j) = lower[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &html[..at], snippet, &html[at..]);
        }
    }
    format!("{}{}", snippet, html)
}

/// Put the reader script into an HTML page.
pub fn inject_reader_script(html: &str) -> String {
    inject_head(html, READER_SCRIPT)
}

/// A `<base>` tag, so a page that was reached through a redirect finds its pictures and links.
pub fn base_tag(url: &str) -> String {
    let safe: String = url.chars().map(|c| match c { '"' => "%22".to_string(), '<' => "%3C".to_string(), '>' => "%3E".to_string(), '&' => "&amp;".to_string(), c => c.to_string() }).collect();
    format!("<base href=\"{}\">", safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn archives_are_found_in_subfolders_whatever_the_extension_case() {
        let d = std::env::temp_dir().join(format!("cindrix-zimfind-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("manuals/deep/deeper/deepest/too_deep")).unwrap();
        for f in ["a.zim", "B.ZIM", "manuals/c.Zim", "manuals/deep/deeper/d.zim", "manuals/deep/deeper/deepest/too_deep/e.zim", "f.zim.part", "g.zimaa", "h.txt"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        let names: Vec<String> = zim_files(&d).iter().map(|p| p.strip_prefix(&d).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(names, vec!["B.ZIM", "a.zim", "manuals/c.Zim", "manuals/deep/deeper/d.zim"]);
        assert!(zim_files(&d.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn parses_results() {
        let xml = "<rss><channel><item><title>Water &amp; health</title><link>/content/wiki/A/Water</link></item><item><title>B</title><link>/content/wiki/A/B</link></item></channel></rss>";
        let r = parse_search(xml);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], ("Water & health".to_string(), "/content/wiki/A/Water".to_string()));
    }

    #[test]
    fn html_becomes_text() {
        let t = html_to_text("<html><head><style>p{}</style></head><body><h1>Title</h1><p>One <b>two</b> &amp; three</p><script>x()</script><p>Four</p></body></html>");
        assert_eq!(t, "Title\n\nOne two & three\n\nFour");
    }

    #[test]
    fn long_articles_give_the_matching_part_not_just_the_start() {
        let intro = "Water is a clear liquid that covers most of the earth and is used for many things every day. ".repeat(10);
        let answer = "To purify river water, filter it through cloth and then boil it for one minute so germs are killed. ".repeat(5);
        let filler = "Lakes and oceans have been studied by many scientists over the years in different ways. ".repeat(12);
        let article = format!("{}\n\n{}\n\n{}", intro, filler, answer);
        let got = best_chunk("Water", &article, "how do I purify river water by boiling");
        assert!(got.contains("boil it for one minute"), "{}", got);
        assert!(got.len() <= 1500);
    }

    #[test]
    fn encodes_urls() {
        assert_eq!(url_encode("boil water?"), "boil%20water%3F");
    }

    #[test]
    fn arguments_bind_only_to_this_computer() {
        let a = args(&[PathBuf::from("a.zim")], 8123, false);
        assert!(a.windows(2).any(|w| w[0] == "--address" && w[1] == "127.0.0.1"));
        assert_eq!(a.last().unwrap(), "a.zim");
    }

    #[test]
    fn sharing_is_only_on_when_asked_for() {
        let on = args(&[PathBuf::from("a.zim")], 8787, true);
        assert!(on.windows(2).any(|w| w[0] == "--address" && w[1] == "0.0.0.0"));
        let off = args(&[PathBuf::from("a.zim")], 8787, false);
        assert!(!off.iter().any(|x| x == "0.0.0.0"));
    }

    #[test]
    fn searches_a_fake_server() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.starts_with("GET /search") {
                    "<item><title>Water purification</title><link>/content/wiki_en/A/Water_purification</link></item>".to_string()
                } else {
                    format!("<html><body><p>{}</p></body></html>", "Boil water for one minute to make it safe to drink. ".repeat(4))
                };
                let _ = write!(s, "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
            }
        });
        let p = search(port, "how do I purify water", 3);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].heading, "Water purification");
        assert_eq!(p[0].source, "wiki en");
        assert!(p[0].text.contains("Boil water"));
    }

    #[test]
    fn head_lines_are_read() {
        let (st, ct, loc) = parse_head("HTTP/1.1 302 Found\r\nContent-Type: text/html; charset=utf-8\r\nLocation: /content/wiki/A/Main\r\nX: y").unwrap();
        assert_eq!((st, ct.as_str(), loc.as_deref()), (302, "text/html; charset=utf-8", Some("/content/wiki/A/Main")));
        assert!(parse_head("garbage").is_none());
    }

    #[test]
    fn the_reader_can_only_reach_the_library_parts_of_the_server() {
        for ok in ["/content/wiki/A/Water", "/skin/kiwix.css", "/suggest?content=wiki&term=water", "/search?content=wiki&pattern=x", "/catalog/v2/entries"] {
            assert!(proxy_path_ok(ok), "{}", ok);
        }
        for bad in ["", "/", "/admin", "/random", "/content/../etc", "/content/a%2e%2e/b", "/content/a b", "/content/a\r\nHost: x", "/content/a\nb", "http://evil/content/x", "//evil/content/", &format!("/content/{}", "a".repeat(3000))] {
            assert!(!proxy_path_ok(bad), "{:?}", bad);
        }
    }

    #[test]
    fn follows_redirects_on_this_server_only() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                let out = if line.starts_with("GET /content/book ") {
                    "HTTP/1.0 302 Found\r\nLocation: /content/book/A/Main\r\nContent-Length: 0\r\n\r\n".to_string()
                } else if line.starts_with("GET /content/away ") {
                    "HTTP/1.0 302 Found\r\nLocation: http://example.com/x\r\nContent-Length: 0\r\n\r\n".to_string()
                } else if line.starts_with("GET /content/loop ") {
                    "HTTP/1.0 302 Found\r\nLocation: /content/loop\r\nContent-Length: 0\r\n\r\n".to_string()
                } else {
                    let body = "<html><head><title>Main</title></head><body>hi</body></html>";
                    format!("HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                };
                let _ = s.write_all(out.as_bytes());
            }
        });
        let (r, final_path) = follow(port, "/content/book").unwrap();
        assert_eq!((r.status, final_path.as_str(), r.content_type.as_str()), (200, "/content/book/A/Main", "text/html"));
        assert!(String::from_utf8_lossy(&r.body).contains("hi"));
        assert!(follow(port, "/content/away").unwrap_err().contains("another site"));
        assert!(follow(port, "/content/loop").unwrap_err().contains("redirects"));
        assert!(follow(port, "/admin").is_err());
    }

    #[test]
    fn the_catalog_gives_book_names_and_titles() {
        let xml = r#"<feed><entry><title>Wikipedia &amp; more</title><name>wikipedia_en_x</name><link rel="alternate" type="text/html" href="/content/wikipedia_en_x_2025-01"/><link type="application/x-zim" href="/x.zim"/></entry><entry><title></title><name>fallback_book</name></entry></feed>"#;
        let b = parse_catalog(xml);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].name.as_str(), b[0].title.as_str()), ("wikipedia_en_x_2025-01", "Wikipedia & more"));
        assert_eq!((b[1].name.as_str(), b[1].title.as_str()), ("fallback_book", "fallback book"));
        assert!(parse_catalog("<feed></feed>").is_empty());
    }

    #[test]
    fn the_reader_script_goes_into_the_head() {
        let h = inject_reader_script("<!DOCTYPE html><HTML><HEAD lang=x><title>T</title></HEAD><body>b</body></HTML>");
        let at = h.find("<script>").unwrap();
        assert!(h[..at].ends_with("<HEAD lang=x>"));
        assert!(h.contains("<title>T</title>"));
        assert!(inject_reader_script("plain").starts_with("<script>"));
        assert_eq!(base_tag("http://h/a\"><script>"), "<base href=\"http://h/a%22%3E%3Cscript%3E\">");
        let both = inject_head(&inject_head("<head></head>", READER_SCRIPT), &base_tag("http://h/x/"));
        assert!(both.starts_with("<head><base"));
        // multi-byte text before the head cannot split a character
        assert!(inject_reader_script("<html>é<head></head>").contains("<script>"));
    }

    #[test]
    fn the_assistant_searches_every_archive() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let line = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
                let article = format!("<html><body><p>{}</p></body></html>", "Boil water for one minute to make it safe to drink. ".repeat(4));
                let body = if line.contains("/catalog/v2/entries") {
                    "<feed><entry><title>Wiki</title><link type=\"text/html\" href=\"/content/wiki\"/></entry><entry><title>Manual</title><link type=\"text/html\" href=\"/content/manual\"/></entry></feed>".to_string()
                } else if line.contains("/search?content=wiki") {
                    "<item><title>Water in Wiki</title><link>/content/wiki/A/Water</link></item><item><title>Wiki two</title><link>/content/wiki/A/Two</link></item>".to_string()
                } else if line.contains("/search?content=manual") {
                    "<item><title>Water in Manual</title><link>/content/manual/A/Water</link></item>".to_string()
                } else {
                    article
                };
                let _ = write!(s, "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
            }
        });
        let p = search(port, "how do I make water safe", 3);
        let sources: Vec<&str> = p.iter().map(|x| x.source.as_str()).collect();
        assert_eq!(p.len(), 3, "{:?}", sources);
        // the small archive's best hit is not pushed out by the big one's second hit
        assert!(sources.contains(&"manual") && sources.contains(&"wiki"), "{:?}", sources);
    }

    #[test]
    fn links_become_paths_whatever_the_server_version_sends() {
        assert_eq!(link_path("/content/wiki/A/Water"), "/content/wiki/A/Water");
        assert_eq!(link_path("http://127.0.0.1:8123/content/wiki/A/Water"), "/content/wiki/A/Water");
        assert_eq!(link_path("https://host.example:8080/content/wiki/A/Water"), "/content/wiki/A/Water");
        assert_eq!(link_path("content/wiki/A/Water"), "/content/wiki/A/Water");
        assert_eq!(link_path("  /content/wiki/A/Water &amp; more\n"), "/content/wiki/A/Water%20&%20more");
        assert_eq!(link_path("/content/wiki/A/Café"), "/content/wiki/A/Caf%C3%A9");
        assert_eq!(link_path("<![CDATA[http://h/content/b/A/X]]>"), "/content/b/A/X");
        // a link to another site keeps only its path, so the program still only asks its own server
        assert_eq!(link_path("http://evil.example/content/b/A/X"), "/content/b/A/X");
        assert_eq!(link_path("http://evil.example"), "/");
    }

    #[test]
    fn results_may_use_cdata_and_full_addresses() {
        let xml = "<?xml version=\"1.0\"?><rss><channel><title>Search: x</title><link>http://127.0.0.1:8080/search?pattern=x</link>\
            <item><title><![CDATA[Water & Health]]></title><link>http://127.0.0.1:8080/content/wiki_en/A/Water</link><description>d</description></item>\
            <item><title>No link here</title></item>\
            <item><title>Second</title><link>/content/wiki_en/A/Second_page</link></item></channel></rss>";
        let r = parse_search(xml);
        // the channel's own title and link are not results; an item without a link is skipped
        assert_eq!(r, vec![("Water & Health".to_string(), "/content/wiki_en/A/Water".to_string()), ("Second".to_string(), "/content/wiki_en/A/Second_page".to_string())]);
    }

    #[test]
    fn a_question_becomes_search_words() {
        assert_eq!(keywords("How do I purify river water by boiling it?"), vec!["purify", "river", "water", "boiling"]);
        assert_eq!(keywords("water water WATER"), vec!["water"]);
        assert!(keywords("how do I").is_empty());
    }

    #[test]
    fn suggest_json_is_read() {
        let json = r#"[{"value":"Water purification","label":"Water purification","kind":"path","path":"A/Water_purification"},
            {"value":"Café \"Water\" {x}","label":"Café \"Water\" {x}","kind":"path","path":"Café page"},
            {"value":"A😀","label":"emoji","kind":"path","path":"/content/other/A/Smile"},
            {"value":"water ","label":"containing 'water'...","kind":"pattern"},
            {"value":"no path","kind":"path"}]"#;
        let r = parse_suggest(json, "wiki_en");
        assert_eq!(
            r,
            vec![
                ("Water purification".to_string(), "/content/wiki_en/A/Water_purification".to_string()),
                ("Café \"Water\" {x}".to_string(), "/content/wiki_en/Caf%C3%A9%20page".to_string()),
                ("A😀".to_string(), "/content/other/A/Smile".to_string()),
            ]
        );
        assert_eq!(json_string(r#"{"label":"A😀"}"#, "label").unwrap(), "A😀");
        assert!(parse_suggest("[]", "b").is_empty());
        assert!(parse_suggest("not json at all", "b").is_empty());
    }

    #[test]
    fn highlighted_suggestion_labels_become_plain_titles() {
        // as the real kiwix-serve 3.8 sends them: tags escaped inside the JSON text
        let json = r#"[{"value":"Emergency Alerts | Ready.gov","label":"&lt;b&gt;Emergency&lt;/b&gt; Alerts | Ready.gov","kind":"path","path":"/content/ready.gov_en_all_2026-10/www.ready.gov/alerts"},
            {"label":"&lt;b&gt;Emergency&lt;/b&gt; Contraception","kind":"path","path":"Emergency_Contraception"},
            {"label":"a < b &amp; c","kind":"path","path":"Less_than"}]"#;
        let r = parse_suggest(json, "wikem");
        assert_eq!(r[0], ("Emergency Alerts | Ready.gov".to_string(), "/content/ready.gov_en_all_2026-10/www.ready.gov/alerts".to_string()));
        assert_eq!(r[1], ("Emergency Contraception".to_string(), "/content/wikem/Emergency_Contraception".to_string()));
        assert_eq!(r[2].0, "a < b & c");
        assert_eq!(plain_label("x &lt;i&gt;y&lt;/i&gt; z"), "x y z");
    }

    #[test]
    fn translated_pages_are_recognised_from_real_addresses() {
        // seen in the real WikEM and Ready.gov archives
        for t in [
            "/content/wikem_en_all_maxi_2026-07/Anaphylaxis_(peds)/de",
            "/content/wikem_en_all_maxi_2026-07/Water_Purification/ru",
            "/content/ready.gov_en_all_2026-10/www.ready.gov/ar/node/6015",
            "/content/ready.gov_en_all_2026-10/www.ready.gov/ht/kids/prepare-pedro",
            "/content/b/A/Some_page/zh-hans",
            "/content/b/A/Some_page/pt-br?x=1",
        ] {
            assert!(is_translation(t), "{}", t);
        }
        for o in [
            "/content/wikem_en_all_maxi_2026-07/Anaphylaxis",
            "/content/wikem_en_all_maxi_2026-07/Anaphylaxis_(peds)",
            "/content/ready.gov_en_all_2026-10/www.ready.gov/kids/prepare-pedro",
            "/content/ready.gov_en_all_2026-10/www.ready.gov/alerts",
            "/content/ready.gov_en_all_2026-10/www.ready.gov/kit",
            "/content/wikipedia_en/A/It",
            "/content/de/A/Water", // a book that happens to be called "de" is not a translation of one page
            "/content/b/de",
            "/content/b/A/Word_de_word",
        ] {
            assert!(!is_translation(o), "{}", o);
        }
    }

    #[test]
    fn original_pages_come_before_their_translations() {
        let (port, _) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok("<feed><entry><title>WikEM</title><link type=\"text/html\" href=\"/content/wikem\"/></entry></feed>")
            } else if line.contains("/search?") {
                ok("<rss><channel>\
                    <item><title>Anaphylaxis (peds)/de</title><link>/content/wikem/Anaphylaxis_(peds)/de</link></item>\
                    <item><title>Anaphylaxis/ru</title><link>/content/wikem/Anaphylaxis/ru</link></item>\
                    <item><title>Anaphylaxis</title><link>/content/wikem/Anaphylaxis</link></item></channel></rss>")
            } else if line.contains("/suggest?") {
                ok("[]")
            } else if line.contains("/content/wikem/Anaphylaxis ") {
                ok("<html><body><p>Anaphylaxis is a severe allergic reaction. Give adrenaline into the thigh muscle at once and call for help. Lie the person flat.</p></body></html>")
            } else {
                ok("<html><body><p>Das ist eine maschinell uebersetzte Seite ueber Anaphylaxie und Adrenalin, die kein Englisch enthaelt, aber lang genug ist.</p></body></html>")
            }
        });
        let p = search(port, "What is anaphylaxis?", 3);
        assert_eq!(p.iter().map(|x| x.heading.as_str()).collect::<Vec<_>>(), vec!["Anaphylaxis"]);
    }

    #[test]
    fn translations_are_kept_only_when_nothing_else_exists() {
        let (port, _) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok("<feed><entry><title>W</title><link type=\"text/html\" href=\"/content/w\"/></entry></feed>")
            } else if line.contains("/search?") {
                ok("<rss><channel><item><title>Wasser/de</title><link>/content/w/Wasser/de</link></item></channel></rss>")
            } else if line.contains("/suggest?") {
                ok("[]")
            } else {
                ok(&article())
            }
        });
        let p = search(port, "wasser", 2);
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn a_page_called_what_you_asked_about_beats_a_passing_mention() {
        // The full-text engine returns the page with the most mentions; the page named "Burns" is the better answer.
        let (port, _) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok("<feed><entry><title>WikEM</title><link type=\"text/html\" href=\"/content/wikem\"/></entry></feed>")
            } else if line.contains("/search?") {
                ok("<rss><channel><item><title>Hydrofluoric acid</title><link>/content/wikem/Hydrofluoric_acid</link></item></channel></rss>")
            } else if line.contains("/suggest?") {
                if line.contains("term=burn&") {
                    ok(r#"[{"value":"Burns","label":"&lt;b&gt;Burn&lt;/b&gt;s","kind":"path","path":"Burns"}]"#)
                } else {
                    ok("[]")
                }
            } else if line.contains("/content/wikem/Burns ") {
                ok("<html><body><p>Cool the burn under cool running water for twenty minutes. Do not use ice. Cover it with a clean dry dressing and treat pain.</p></body></html>")
            } else {
                ok("<html><body><p>Hydrofluoric acid is used to etch glass and as a rust remover. Skin contact can cause a deep chemical injury that needs urgent care.</p></body></html>")
            }
        });
        let p = search(port, "How do I treat a burn?", 2);
        assert_eq!(p.iter().map(|x| x.heading.as_str()).collect::<Vec<_>>(), vec!["Burns", "Hydrofluoric acid"]);
        assert!(p[0].score > p[1].score);
    }

    // A fake kiwix-serve that records what it was asked, for the search fallbacks.
    fn recording_server(handler: impl Fn(&str) -> String + Send + 'static) -> (u16, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let line = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_string();
                log.lock().unwrap().push(line.clone());
                let _ = s.write_all(handler(&line).as_bytes());
            }
        });
        (port, seen)
    }

    fn ok(body: &str) -> String {
        format!("HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
    }

    fn article() -> String {
        format!("<html><body><p>{}</p></body></html>", "Boil water for one minute to make it safe to drink. ".repeat(4))
    }

    const CATALOG: &str = "<feed><entry><title>Manual</title><link type=\"text/html\" href=\"/content/manual\"/></entry></feed>";

    #[test]
    fn a_long_question_is_searched_by_its_key_words_not_as_a_sentence() {
        let (port, seen) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok(CATALOG)
            } else if line.contains("/search?") {
                // only a short query finds anything, like a real index that wants every word present
                if line.contains("pattern=boiling%20purify") {
                    ok("<rss><channel><item><title>Boiling</title><link>http://127.0.0.1:1/content/manual/A/Boiling</link></item></channel></rss>")
                } else {
                    ok("<rss><channel></channel></rss>")
                }
            } else {
                ok(&article())
            }
        });
        let p = search(port, "How do I purify river water by boiling it?", 2);
        assert_eq!(p.len(), 1, "{:?}", seen.lock().unwrap());
        assert_eq!((p[0].heading.as_str(), p[0].source.as_str()), ("Boiling", "manual"));
        let lines = seen.lock().unwrap().clone();
        assert!(lines.iter().all(|l| !l.contains("how%20do")), "the whole sentence was sent: {:?}", lines);
        assert!(lines.iter().any(|l| l.contains("pattern=purify%20river%20water%20boiling")), "{:?}", lines);
    }

    #[test]
    fn an_archive_without_a_search_index_is_found_by_title() {
        let (port, seen) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok(CATALOG)
            } else if line.contains("/search?") {
                "HTTP/1.0 404 Not Found\r\nContent-Length: 18\r\n\r\nNo search index.\r\n".to_string()
            } else if line.contains("/suggest?") {
                if line.contains("term=boiling") {
                    ok(r#"[{"value":"Boiling water","label":"Boiling water","kind":"path","path":"A/Boiling_water"},{"value":"boiling ","label":"containing 'boiling'...","kind":"pattern"}]"#)
                } else {
                    ok("[]")
                }
            } else {
                ok(&article())
            }
        });
        let p = search(port, "how do I purify river water by boiling", 2);
        assert_eq!(p.len(), 1, "{:?}", seen.lock().unwrap());
        assert_eq!((p[0].heading.as_str(), p[0].source.as_str()), ("Boiling water", "manual"));
        assert!(p[0].text.contains("Boil water"));
    }

    #[test]
    fn nothing_found_anywhere_is_an_empty_answer_not_a_crash() {
        let (port, _) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok(CATALOG)
            } else if line.contains("/suggest?") {
                ok("[]")
            } else {
                "HTTP/1.0 500 Server Error\r\nContent-Length: 0\r\n\r\n".to_string()
            }
        });
        assert!(search(port, "how do I tune a violin", 2).is_empty());
    }

    #[test]
    fn a_result_link_outside_the_library_is_never_fetched() {
        let (port, seen) = recording_server(|line| {
            if line.contains("/catalog/v2/entries") {
                ok(CATALOG)
            } else if line.contains("/search?") {
                ok("<rss><channel><item><title>Odd</title><link>/admin/shutdown</link></item></channel></rss>")
            } else {
                ok(&article())
            }
        });
        assert!(search(port, "water", 2).is_empty());
        assert!(!seen.lock().unwrap().iter().any(|l| l.contains("/admin")));
    }
}
