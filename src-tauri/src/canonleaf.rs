//! The Canonleaf add-on's side of the app: which folder Canonleaf lives in (Settings > Assistant),
//! the passages Ask may quote, and the chapter reader. Read-only: nothing here writes to Canonleaf's
//! folder. The rules and tests are in `core/src/canonleaf.rs`; the design is `docs/44-canonleaf-addon.md`.

use cindrix_core::canonleaf as cl;
use cindrix_core::retrieval::Passage;
use serde::Serialize;
use std::path::PathBuf;

/// The folder the person chose for Canonleaf, if it still exists.
fn folder() -> Option<PathBuf> {
    crate::settings::folders("canonleafFolder").into_iter().next()
}

/// Verses for Ask, or nothing when the add-on is off, no folder is set, or nothing matches.
pub fn passages(question: &str, limit: usize) -> Vec<Passage> {
    if !crate::settings::flag("searchCanonleaf", true) {
        return Vec::new();
    }
    folder().map(|f| cl::search(&f, question, limit)).unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub folder: String,
    pub found: bool,
    pub editions: Vec<String>,
    pub message: String,
}

#[tauri::command]
pub fn canonleaf_status() -> Status {
    let Some(f) = folder() else {
        return Status { folder: String::new(), found: false, editions: vec![], message: "No Canonleaf folder chosen yet. Choose the folder where the Canonleaf program is kept.".into() };
    };
    let editions: Vec<String> = cl::find_bibles(&f).into_iter().map(|b| b.title).collect();
    let message = if editions.is_empty() {
        "That folder has no Bible text in it. Choose the main Canonleaf folder (the one that holds a \"resources\" folder).".to_string()
    } else {
        format!("Found {} Bible text{}. Ask can now quote verses and name where they came from.", editions.len(), if editions.len() == 1 { "" } else { "s" })
    };
    Status { folder: f.to_string_lossy().to_string(), found: !editions.is_empty(), editions, message }
}

#[derive(Serialize)]
pub struct BookDto {
    pub code: String,
    pub name: String,
}

#[tauri::command]
pub fn canonleaf_books() -> Vec<BookDto> {
    cl::books().into_iter().map(|(c, n)| BookDto { code: c.to_string(), name: n.to_string() }).collect()
}

#[derive(Serialize)]
pub struct VerseDto {
    pub number: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct ChapterDto {
    pub book: String,
    pub name: String,
    pub chapter: u32,
    pub chapters: u32,
    pub edition: String,
    pub verses: Vec<VerseDto>,
}

/// One chapter of the default edition. Fails in plain words when Canonleaf is not set up or the chapter is not there.
#[tauri::command]
pub fn canonleaf_chapter(book: String, chapter: u32) -> Result<ChapterDto, String> {
    let f = folder().ok_or("Canonleaf has not been set up. Choose its folder in Settings.")?;
    let bible = cl::default_bible(&f).ok_or("No Bible text was found in the Canonleaf folder.")?;
    let name = cl::book_name(&book).ok_or("That book is not in the list.")?;
    let verses = cl::chapter(&bible, &book, chapter);
    if verses.is_empty() {
        return Err(format!("{} {} is not in {}.", name, chapter, bible.title));
    }
    Ok(ChapterDto {
        chapters: cl::chapter_count(&bible, &book),
        verses: verses.into_iter().map(|(number, text)| VerseDto { number, text }).collect(),
        edition: bible.title,
        name: name.to_string(),
        book,
        chapter,
    })
}
