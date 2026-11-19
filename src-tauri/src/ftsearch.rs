//! Tauri side of search inside articles (core/src/ftindex.rs). Searching is open to guests (library content);
//! making, stopping and deleting an index is host-only. One index is made at a time, in the background.

use cindrix_core::ftindex::{self, Depth, Outcome, Progress, Status};
use cindrix_core::library;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

#[derive(Serialize)]
pub struct TextHitDto {
    book: String,
    book_title: String,
    title: String,
    path: String,
    snippet: String,
}

/// Articles whose text has the words, from every archive that has a search index.
#[tauri::command]
pub async fn zim_search_text(term: String, category: Option<String>) -> Result<Vec<TextHitDto>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::zimview::library()
            .search_text(category.as_deref(), &term, 6, 30)
            .into_iter()
            .map(|h| TextHitDto { path: format!("/content/{}/{}", library::encode_path(&h.book), library::encode_path(&h.address)), book: h.book, book_title: h.book_title, title: h.title, snippet: h.snippet })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

struct Job {
    book: String,
    progress: Arc<Progress>,
    finished: bool,
    message: String,
}
static JOB: Mutex<Option<Job>> = Mutex::new(None);

#[derive(Serialize)]
pub struct IndexBookDto {
    name: String,
    title: String,
    articles: u32,
    mb: u64,
    category: String,
    /// none, partial, ready or stale
    state: String,
    /// Partial: articles read so far.
    done: u32,
    docs: u32,
    depth: String,
    index_mb: u64,
    est_lead_mb: u64,
    est_full_mb: u64,
    running: bool,
}

#[derive(Serialize)]
pub struct IndexJobDto {
    book: String,
    running: bool,
    joining: bool,
    done: u64,
    total: u64,
    percent: u32,
    finished: bool,
    message: String,
}

#[derive(Serialize)]
pub struct IndexListDto {
    books: Vec<IndexBookDto>,
    job: IndexJobDto,
}

fn job_dto() -> IndexJobDto {
    let g = JOB.lock().unwrap_or_else(|e| e.into_inner());
    match g.as_ref() {
        None => IndexJobDto { book: String::new(), running: false, joining: false, done: 0, total: 0, percent: 0, finished: false, message: String::new() },
        Some(j) => {
            let phase = j.progress.phase.load(Ordering::Relaxed);
            let (done, total) = (j.progress.done.load(Ordering::Relaxed), j.progress.total.load(Ordering::Relaxed));
            IndexJobDto { book: j.book.clone(), running: !j.finished, joining: phase == 2, done, total, percent: if total == 0 { 0 } else { ((done * 100) / total).min(100) as u32 }, finished: j.finished, message: j.message.clone() }
        }
    }
}

/// Every archive with the state of its search index, and the one being made now.
#[tauri::command]
pub fn zim_index_list() -> IndexListDto {
    let lib = crate::zimview::library();
    let job = job_dto();
    let books = lib
        .books
        .iter()
        .map(|b| {
            let mut d = IndexBookDto {
                name: b.name.clone(),
                title: b.title.clone(),
                articles: b.archive.articles,
                mb: std::fs::metadata(&b.file).map(|m| m.len() / 1_048_576).unwrap_or(0),
                category: b.category.clone(),
                state: "none".into(),
                done: 0,
                docs: 0,
                depth: String::new(),
                index_mb: 0,
                est_lead_mb: b.archive.articles as u64 * Depth::Lead.bytes_per_article() / 1_048_576,
                est_full_mb: b.archive.articles as u64 * Depth::Full.bytes_per_article() / 1_048_576,
                running: job.running && job.book == b.name,
            };
            if let Ok((dir, fp)) = lib.index_dir(b) {
                match ftindex::status(&dir, &fp) {
                    Status::NotBuilt => {}
                    Status::Partial { done, .. } => {
                        d.state = "partial".into();
                        d.done = done;
                    }
                    Status::Ready { docs, depth, bytes, .. } => {
                        d.state = "ready".into();
                        d.docs = docs;
                        d.depth = depth.word().into();
                        d.index_mb = bytes / 1_048_576;
                    }
                    Status::OutOfDate => d.state = "stale".into(),
                }
            }
            d
        })
        .collect();
    IndexListDto { books, job }
}

#[tauri::command]
pub fn zim_index_status() -> IndexJobDto {
    job_dto()
}

/// Start making (or carry on making) one archive's search index. `depth` is "lead" or "full".
#[tauri::command]
pub fn zim_index_start(book: String, depth: String) -> Result<(), String> {
    let depth = Depth::parse(&depth).ok_or("Choose how much of each article to read.")?;
    let lib = crate::zimview::library();
    let idx = lib.books.iter().position(|b| b.name == book).ok_or("That archive is not on the drive any more.")?;
    let (dir, fp) = lib.index_dir(&lib.books[idx]).map_err(|_| "That archive could not be read.".to_string())?;
    let progress = Progress::new();
    {
        let mut g = JOB.lock().unwrap_or_else(|e| e.into_inner());
        if g.as_ref().map(|j| !j.finished).unwrap_or(false) {
            return Err("A search index is being made already. Wait for it, or stop it first.".to_string());
        }
        *g = Some(Job { book: book.clone(), progress: progress.clone(), finished: false, message: String::new() });
    }
    // a finished index of the other depth is replaced, and Windows will not replace a file that is open
    lib.forget_index(&book);
    std::thread::spawn(move || {
        let b = &lib.books[idx];
        let msg = match ftindex::build(&b.archive, &dir, &fp, depth, ftindex::DEFAULT_BUDGET, &progress) {
            Ok(Outcome::Done) => "The search index is ready.".to_string(),
            Ok(Outcome::Stopped) => "Stopped. It will carry on from here when you start it again.".to_string(),
            Err(e) => e,
        };
        let mut g = JOB.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(j) = g.as_mut() {
            j.finished = true;
            j.message = msg;
        }
    });
    Ok(())
}

#[tauri::command]
pub fn zim_index_stop() {
    if let Some(j) = JOB.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if !j.finished {
            j.progress.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Remove an archive's search index (the archive itself is not touched).
#[tauri::command]
pub fn zim_index_delete(book: String) -> Result<(), String> {
    if job_dto().running && job_dto().book == book {
        return Err("That index is being made. Stop it first.".to_string());
    }
    let lib = crate::zimview::library();
    let b = lib.books.iter().find(|b| b.name == book).ok_or("That archive is not on the drive any more.")?;
    let (dir, _) = lib.index_dir(b).map_err(|_| "That archive could not be read.".to_string())?;
    lib.forget_index(&book);
    ftindex::delete(&dir).map_err(|_| "The search index could not be removed. Close anything that is using that drive and try again.".to_string())
}
