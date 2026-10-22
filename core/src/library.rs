//! All the ZIM archives on the drive, read directly by this program (no Kiwix program needed):
//! look up pages for the reader, suggest titles as you type, and find passages for the
//! assistant. See `zimread` for the file format side.
//!
//! Searching here is by TITLE. Kiwix's own full-text index (Xapian) is not read; when
//! `kiwix-serve` is running the app uses that for full-text search as well.

use crate::kiwix::{best_chunk, html_to_text};
use crate::retrieval::{self, Passage};
use crate::zimread::{Archive, Page};
use std::io;
use std::path::{Path, PathBuf};

pub struct Book {
    /// Safe to put in an address: letters, digits, dot, dash, underscore.
    pub name: String,
    pub title: String,
    pub file: PathBuf,
    pub archive: Archive,
}

pub struct Library {
    pub books: Vec<Book>,
}

/// The name used for an archive in addresses.
pub fn book_name(file: &Path) -> String {
    let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let n: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    if n.is_empty() { "library".into() } else { n }
}

/// Changes when a ZIM file is added, removed or replaced.
pub fn signature(files: &[PathBuf]) -> String {
    files
        .iter()
        .map(|f| {
            let m = std::fs::metadata(f).ok();
            let t = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            format!("{}:{}:{}", f.display(), m.map(|m| m.len()).unwrap_or(0), t)
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// Decides when the library server should be restarted because archives were added or removed.
/// Call `should_restart` every few seconds. It says yes only when the list of files differs from
/// what the server runs with AND the files have not changed since the previous call, so an
/// archive that is still being copied in (its size keeps growing) is waited for, not restarted on.
#[derive(Default)]
pub struct SettleWatch {
    last: Option<String>,
}

impl SettleWatch {
    pub fn should_restart(&mut self, now: &[PathBuf], running: &[PathBuf]) -> bool {
        let sig = signature(now);
        let steady = self.last.as_deref() == Some(sig.as_str());
        self.last = Some(sig);
        steady && now != running
    }
}

/// One title found by searching every archive at once.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The archive's name as used in addresses.
    pub book: String,
    pub book_title: String,
    /// The page's address inside the archive.
    pub address: String,
    pub title: String,
}

impl Library {
    /// Open every readable archive. A file that cannot be read is left out, not an error.
    pub fn open(files: &[PathBuf]) -> Library {
        let mut books: Vec<Book> = Vec::new();
        for f in files {
            let Ok(archive) = Archive::open(f) else { continue };
            let mut name = book_name(f);
            while books.iter().any(|b| b.name == name) {
                name.push('_');
            }
            let title = archive.metadata("Title").unwrap_or_else(|| name.replace('_', " "));
            books.push(Book { name, title, file: f.clone(), archive });
        }
        Library { books }
    }

    pub fn book(&self, name: &str) -> Option<&Book> {
        self.books.iter().find(|b| b.name == name)
    }

    /// A page for the reader. An empty address means the archive's start page.
    pub fn page(&self, book: &str, address: &str) -> io::Result<Option<Page>> {
        let Some(b) = self.book(book) else { return Ok(None) };
        if address.is_empty() {
            return b.archive.main_page();
        }
        b.archive.read(address)
    }

    /// Title suggestions for the search box, as JSON in the shape the reader expects.
    pub fn suggest_json(&self, book: &str, term: &str) -> String {
        let Some(b) = self.book(book) else { return "[]".into() };
        let items = b.archive.suggest(term.trim(), 12).unwrap_or_default();
        let parts: Vec<String> = items.iter().map(|(addr, title)| format!("{{\"value\":{},\"label\":{},\"path\":{}}}", json_str(title), json_str(title), json_str(addr))).collect();
        format!("[{}]", parts.join(","))
    }

    /// Titles matching `term` in EVERY archive, for the Intel search box. Up to `per_book` come
    /// from each archive and the list alternates between archives (everyone's best first), so a
    /// huge encyclopedia does not push a small manual off the list. At most `total` are returned.
    pub fn suggest_all(&self, term: &str, per_book: usize, total: usize) -> Vec<Hit> {
        let term = term.trim();
        if term.chars().count() < 2 || per_book == 0 {
            return vec![];
        }
        let words: Vec<String> = term.split_whitespace().map(|w| w.to_string()).collect();
        let mut lists: Vec<Vec<Hit>> = Vec::new();
        for b in &self.books {
            let mut found = b.archive.suggest(term, per_book).unwrap_or_default();
            // a title that merely contains the words ("Purification of water") for archives small enough to scan
            if found.len() < per_book && b.archive.articles <= 50_000 {
                for h in b.archive.search_titles(&words, per_book, 50_000).unwrap_or_default() {
                    if found.len() >= per_book {
                        break;
                    }
                    if !found.iter().any(|f| f.0 == h.0) {
                        found.push(h);
                    }
                }
            }
            lists.push(found.into_iter().map(|(address, title)| Hit { book: b.name.clone(), book_title: b.title.clone(), address, title }).collect());
        }
        let mut out: Vec<Hit> = Vec::new();
        let mut rank = 0;
        loop {
            let mut any = false;
            for l in &lists {
                if let Some(h) = l.get(rank) {
                    out.push(h.clone());
                    any = true;
                    if out.len() >= total {
                        return out;
                    }
                }
            }
            if !any {
                return out;
            }
            rank += 1;
        }
    }

    /// A simple results page for a typed search, by title.
    pub fn results_page(&self, book: &str, term: &str) -> String {
        let mut rows = String::new();
        if let Some(b) = self.book(book) {
            let words: Vec<String> = term.split_whitespace().map(|w| w.to_string()).collect();
            let mut found = b.archive.suggest(term.trim(), 40).unwrap_or_default();
            if b.archive.articles <= 200_000 {
                for h in b.archive.search_titles(&words, 40, 200_000).unwrap_or_default() {
                    if !found.iter().any(|f| f.0 == h.0) {
                        found.push(h);
                    }
                }
            }
            for (addr, title) in found.into_iter().take(60) {
                rows.push_str(&format!("<li><a href=\"/content/{}/{}\">{}</a></li>", book, encode_path(&addr), esc(&title)));
            }
        }
        let body = if rows.is_empty() { "<p>No titles match. Try fewer or shorter words.</p>".to_string() } else { format!("<ul>{}</ul>", rows) };
        format!(
            "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Search: {t}</title><style>body{{font:16px system-ui,sans-serif;margin:16px;max-width:760px}}li{{margin:6px 0}}</style></head><body><h2>Titles matching “{t}”</h2>{b}<p style=\"color:#666;font-size:13px\">This searches article titles. The full-text search needs the Kiwix library program.</p></body></html>",
            t = esc(term),
            b = body
        )
    }

    /// Passages for the assistant from EVERY archive, found by title. Up to `per_book` pages are
    /// read from each archive and the best `total` passages overall are returned.
    pub fn passages(&self, question: &str, per_book: usize, total: usize) -> Vec<Passage> {
        let words = keywords(question);
        if words.is_empty() {
            return vec![];
        }
        let stems: Vec<String> = words.iter().map(|w| stem(w)).collect();
        // single words and neighbouring pairs and triples: "water purification" finds its article
        let mut phrases: Vec<String> = Vec::new();
        for n in (1..=3).rev() {
            for w in words.windows(n) {
                phrases.push(w.join(" "));
            }
        }
        for s in &stems {
            if !phrases.contains(s) {
                phrases.push(s.clone());
            }
        }
        let mut out: Vec<Passage> = Vec::new();
        for b in &self.books {
            let mut cands: Vec<(f64, String, String)> = Vec::new(); // score, address, title
            let mut add = |addr: String, title: String| {
                if cands.iter().any(|c| c.1 == addr) {
                    return;
                }
                let t = title.to_lowercase();
                let hits = stems.iter().filter(|s| t.contains(s.as_str())).count();
                if hits == 0 {
                    return;
                }
                let score = hits as f64 * 2.0 - (t.len() as f64 / 80.0);
                cands.push((score, addr, title));
            };
            for p in &phrases {
                for (a, t) in b.archive.suggest(p, 6).unwrap_or_default() {
                    add(a, t);
                }
            }
            if b.archive.articles <= 50_000 {
                for (a, t) in b.archive.search_titles(&stems, 20, 50_000).unwrap_or_default() {
                    add(a, t);
                }
            }
            cands.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut seen: Vec<(u8, String)> = Vec::new();
            let mut taken = 0;
            for (score, addr, title) in cands {
                if taken >= per_book {
                    break;
                }
                let Ok(Some(page)) = b.archive.read(&addr) else { continue };
                if !page.mime.starts_with("text/html") || seen.contains(&(page.ns, page.url.clone())) {
                    continue;
                }
                seen.push((page.ns, page.url.clone()));
                let text = html_to_text(&String::from_utf8_lossy(&page.data));
                if text.len() < 80 {
                    continue;
                }
                let text = best_chunk(&title, &text, question);
                out.push(Passage { source: b.title.clone(), heading: title, text, score: (4.0 + score * 0.5 - taken as f64 * 0.3).min(7.0), link: format!("/content/{}/{}", encode_path(&b.name), encode_path(&addr)) });
                taken += 1;
            }
        }
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(total);
        out
    }
}

/// The meaningful words of a question, in order, without repeats.
pub fn keywords(question: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for t in retrieval::tokens(question) {
        if t.chars().count() >= 3 && !v.contains(&t) {
            v.push(t);
        }
    }
    v.truncate(8);
    v
}

/// A rough root for matching: "purify" matches "purification", "boiling" matches "boil".
fn stem(w: &str) -> String {
    let n = w.chars().count();
    let keep = if n >= 6 { (n - 2).max(4) } else { n };
    w.chars().take(keep).collect()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Percent-encode each part of an address, keeping the slashes.
pub fn encode_path(addr: &str) -> String {
    addr.split('/').map(crate::kiwix::url_encode).collect::<Vec<_>>().join("/")
}

/// Undo percent-encoding in an address from the web view. None if it is not valid text.
pub fn decode_path(s: &str) -> Option<String> {
    crate::maps::percent_decode(s)
}
