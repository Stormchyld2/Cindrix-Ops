//! Tauri side of Bible study (core/src/bible.rs). The Bible, commentaries, dictionaries and
//! cross-references are Canonleaf's resource files, read from `library/bible` on each library
//! drive. Every function here only reads. The screen calls them as commands, and the share server
//! calls the same functions, so a guest on the hotspot gets the same Bible study as the host.
//! Nothing here gives away a folder name: the status says where to put files in plain words.

use cindrix_core::bible::{self, Library};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Where the program looks, below each library drive's `library` folder.
const FOLDER: &str = "bible";
/// How long a look at the drive is reused. Files copied on show up within this time without a restart.
const REUSE: Duration = Duration::from_secs(20);

static CACHE: Mutex<Option<(Instant, Vec<PathBuf>, Arc<Library>)>> = Mutex::new(None);

/// Where to look: `library/bible` on each library drive, then the folder chosen in Settings > Library
/// (for example Canonleaf's own folder on this computer), which is read where it is and never copied.
fn roots() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = crate::libs::library_roots().into_iter().map(|r| r.join(FOLDER)).collect();
    for f in crate::settings::folders("bibleFolder").into_iter().take(1) {
        if !v.contains(&f) {
            v.push(f);
        }
    }
    v
}

/// The Bible study books on the drives that may be read. `fresh` looks at the drive again now.
pub fn library(fresh: bool) -> Arc<Library> {
    let wanted = roots();
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if !fresh {
        // a change of folder in Settings is picked up at once, not after the reuse time
        if let Some((at, had, lib)) = g.as_ref() {
            if at.elapsed() < REUSE && *had == wanted {
                return lib.clone();
            }
        }
    }
    let lib = Arc::new(Library::open(&wanted));
    *g = Some((Instant::now(), wanted, lib.clone()));
    lib
}

fn short(s: &str, max: usize) -> Result<&str, String> {
    let s = s.trim();
    if s.chars().count() > max {
        return Err("That is too long to look up.".into());
    }
    Ok(s)
}

#[derive(Serialize)]
pub struct ModuleDto {
    id: String,
    title: String,
    kind: String,
    license: String,
    attribution: String,
}

#[derive(Serialize)]
pub struct StatusDto {
    /// at least one Bible is on the drive
    found: bool,
    /// the Bible to open first
    default_bible: Option<String>,
    modules: Vec<ModuleDto>,
    /// where to put the files, in plain words
    where_to_put: String,
}

#[tauri::command]
pub fn bible_status(fresh: Option<bool>) -> StatusDto {
    let lib = library(fresh.unwrap_or(false));
    StatusDto {
        found: lib.default_bible().is_some(),
        default_bible: lib.default_bible().map(|m| m.id.clone()),
        modules: lib
            .modules
            .iter()
            .map(|m| ModuleDto { id: m.id.clone(), title: m.title.clone(), kind: m.kind.clone(), license: m.license.clone(), attribution: m.attribution.clone() })
            .collect(),
        where_to_put: "the folder library\\bible on the drive, or choose Canonleaf's folder in Settings > Library".to_string(),
    }
}

#[derive(Serialize)]
pub struct BookDto {
    code: &'static str,
    name: &'static str,
    chapters: u32,
}

/// The books one Bible has, in the usual order.
#[tauri::command]
pub fn bible_books(bible: String) -> Result<Vec<BookDto>, String> {
    let lib = library(false);
    let m = lib.module(&bible).filter(|m| m.kind == "bible").ok_or("That Bible is not on the drive any more.")?;
    Ok(lib.books_in(m).into_iter().map(|b| BookDto { code: b.code, name: b.name, chapters: b.chapters }).collect())
}

#[derive(Serialize)]
pub struct VerseDto {
    number: String,
    text: String,
}

#[tauri::command]
pub fn bible_chapter(bible: String, book: String, chapter: u32) -> Result<Vec<VerseDto>, String> {
    let v = library(false).chapter(&bible, &book, chapter)?;
    Ok(v.into_iter().map(|v| VerseDto { number: v.number, text: v.text }).collect())
}

#[derive(Serialize)]
pub struct NoteDto {
    label: String,
    text: String,
}

#[tauri::command]
pub fn bible_notes(module: String, book: String, chapter: u32, verse: Option<u32>) -> Result<Vec<NoteDto>, String> {
    let n = library(false).notes(&module, &book, chapter, verse)?;
    Ok(n.into_iter().map(|n| NoteDto { label: n.label, text: n.text }).collect())
}

/// A place named in a list ("Rom 5:8"): the words, and the address to open it (empty when it is not a place).
#[derive(Serialize)]
pub struct RefDto {
    text: String,
    code: String,
}

fn ref_dto(text: &str) -> RefDto {
    RefDto { text: text.to_string(), code: bible::parse_reference(text).map(|r| r.code()).unwrap_or_default() }
}

#[derive(Serialize)]
pub struct XRefDto {
    anchor: String,
    refs: Vec<RefDto>,
}

#[derive(Serialize)]
pub struct CrossDto {
    /// the list they came from, or empty when no cross-reference list is on the drive
    source: String,
    items: Vec<XRefDto>,
}

#[tauri::command]
pub fn bible_crossrefs(bible: String, book: String, chapter: u32, verse: u32) -> Result<CrossDto, String> {
    let lib = library(false);
    let Some(m) = lib.crossref_module_for(&bible) else { return Ok(CrossDto { source: String::new(), items: Vec::new() }) };
    let items = lib.crossrefs(&m.id, &book, chapter, verse)?;
    Ok(CrossDto { source: m.title.clone(), items: items.into_iter().map(|x| XRefDto { anchor: x.anchor, refs: x.refs.iter().map(|r| ref_dto(r)).collect() }).collect() })
}

#[derive(Serialize)]
pub struct HitDto {
    book: &'static str,
    name: &'static str,
    chapter: u32,
    verse: String,
    text: String,
}

/// Verses with all the words typed (at most 60). Runs off the screen's thread: it reads the whole Bible.
pub fn search_for(bible: &str, query: &str) -> Result<Vec<HitDto>, String> {
    let q = short(query, 200)?;
    let hits = library(false).search(bible, q, 60)?;
    Ok(hits
        .into_iter()
        .map(|h| HitDto { book: h.book, name: bible::book_by_code(h.book).map(|b| b.name).unwrap_or(h.book), chapter: h.chapter, verse: h.verse, text: h.text })
        .collect())
}

#[tauri::command]
pub async fn bible_search(bible: String, query: String) -> Result<Vec<HitDto>, String> {
    tauri::async_runtime::spawn_blocking(move || search_for(&bible, &query)).await.map_err(|_| "The search stopped early. Try again.".to_string())?
}

#[derive(Serialize)]
pub struct PlaceDto {
    book: &'static str,
    chapter: u32,
    verse: Option<u32>,
}

/// Is what was typed a place in the Bible ("John 3:16", "1 Cor 13")?
#[tauri::command]
pub fn bible_parse(text: String) -> Option<PlaceDto> {
    bible::parse_reference(&text).map(|r| PlaceDto { book: r.book, chapter: r.chapter, verse: r.verse })
}

#[derive(Serialize)]
pub struct EntryDto {
    source: String,
    kind: String,
    title: String,
    body: String,
    refs: Vec<RefDto>,
}

pub fn define_for(term: &str) -> Result<Vec<EntryDto>, String> {
    let t = short(term, 100)?;
    Ok(library(false)
        .define(t, 12)
        .into_iter()
        .map(|e| EntryDto { source: e.source, kind: e.kind, title: e.title, body: e.body, refs: e.refs.iter().map(|r| ref_dto(r)).collect() })
        .collect())
}

/// Dictionary, topical and theology entries for a word.
#[tauri::command]
pub async fn bible_define(term: String) -> Result<Vec<EntryDto>, String> {
    tauri::async_runtime::spawn_blocking(move || define_for(&term)).await.map_err(|_| "The look-up stopped early. Try again.".to_string())?
}

#[derive(Serialize)]
pub struct StrongsDto {
    number: String,
    lemma: String,
    transliteration: String,
    gloss: String,
    morphology: String,
    source: String,
}

pub fn strongs_for(number: &str) -> Result<Vec<StrongsDto>, String> {
    let n = short(number, 12)?;
    Ok(library(false)
        .strongs(n)
        .into_iter()
        .map(|s| StrongsDto { number: s.number, lemma: s.lemma, transliteration: s.transliteration, gloss: s.gloss, morphology: s.morphology, source: s.source })
        .collect())
}

#[tauri::command]
pub async fn bible_strongs(number: String) -> Result<Vec<StrongsDto>, String> {
    tauri::async_runtime::spawn_blocking(move || strongs_for(&number)).await.map_err(|_| "The look-up stopped early. Try again.".to_string())?
}

#[derive(Serialize)]
pub struct MentionDto {
    module_id: String,
    source: String,
    kind: String,
    title: String,
    points: Vec<String>,
    excerpt: String,
    verses: Vec<String>,
}

/// Topical books and Bible dictionaries that cite any of these verses (`BOOK.CHAPTER.VERSE`).
pub fn mentions_for(verses: &[String]) -> Result<Vec<MentionDto>, String> {
    if verses.len() > 30 || verses.iter().any(|v| v.len() > 24) {
        return Err("Pick fewer verses and try again.".into());
    }
    Ok(library(false)
        .mentions(verses)
        .into_iter()
        .map(|m| MentionDto { module_id: m.module_id, source: m.source, kind: m.kind, title: m.title, points: m.points, excerpt: m.excerpt, verses: m.verses })
        .collect())
}

#[tauri::command]
pub async fn bible_mentions(verses: Vec<String>) -> Result<Vec<MentionDto>, String> {
    tauri::async_runtime::spawn_blocking(move || mentions_for(&verses)).await.map_err(|_| "The look-up stopped early. Try again.".to_string())?
}
