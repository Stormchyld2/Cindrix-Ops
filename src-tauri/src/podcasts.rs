//! The podcast page's side of the program: the `cindrix-pod` address the web view plays from,
//! and the commands for listing shows and remembering where each episode was left off.
//! The real work (listing, safe paths, the progress file) is in the core's `podcasts` module,
//! which has its own tests.

use super::{drive_or_program, root};
use crate::music::{reply, serve_range};
use cindrix_core::fsio;
use cindrix_core::podcasts::{self, Progress};
use serde::Serialize;
use std::path::PathBuf;

/// The podcast folder: the one chosen in Settings when it exists, otherwise `podcasts` on the drive.
fn podcasts_dir() -> PathBuf {
    match crate::settings::folders("podcastsFolder").into_iter().next() {
        Some(f) => f,
        None => drive_or_program("podcasts"),
    }
}

fn progress_file() -> PathBuf {
    root().join("user").join("podcast-progress.txt")
}

/// Hands the web view episode files and nothing else: `/e/<path under podcasts/>`.
pub fn serve_podcast(req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    if *req.method() == tauri::http::Method::OPTIONS {
        return reply(204, None, "", Vec::new());
    }
    if *req.method() != tauri::http::Method::GET {
        return reply(405, None, "", Vec::new());
    }
    let requested = req.uri().path().split(['?', '#']).next().unwrap_or("");
    let Some(decoded) = cindrix_core::maps::percent_decode(requested) else {
        return reply(404, None, "", Vec::new());
    };
    let Some(rel) = decoded.trim_start_matches('/').strip_prefix("e/") else {
        return reply(404, None, "", Vec::new());
    };
    match podcasts::resolve(&podcasts_dir(), rel) {
        Some(path) => serve_range(req, &path),
        None => reply(404, None, "", Vec::new()),
    }
}

#[derive(Serialize)]
pub struct EpisodeDto {
    rel: String,
    title: String,
    date: String,
    size: u64,
}

#[derive(Serialize)]
pub struct ShowDto {
    folder: String,
    name: String,
    episodes: Vec<EpisodeDto>,
}

#[derive(Serialize)]
pub struct PodcastList {
    /// False when there is no podcasts folder at all (so the page can say where to put one).
    folder_found: bool,
    shows: Vec<ShowDto>,
}

/// Every show and episode in the podcasts folder. Read on demand: a folder of episodes is small
/// next to a music library, so there is no background scan.
#[tauri::command]
pub async fn podcasts_list() -> PodcastList {
    tauri::async_runtime::spawn_blocking(|| {
        let dir = podcasts_dir();
        let shows = podcasts::scan(&dir)
            .into_iter()
            .map(|s| ShowDto {
                folder: s.folder,
                name: s.name,
                episodes: s.episodes.into_iter().map(|e| EpisodeDto { rel: e.rel, title: e.title, date: e.date, size: e.size }).collect(),
            })
            .collect();
        PodcastList { folder_found: dir.is_dir(), shows }
    })
    .await
    .unwrap_or(PodcastList { folder_found: false, shows: Vec::new() })
}

#[derive(Serialize)]
pub struct ProgressDto {
    rel: String,
    seconds: u32,
    done: bool,
}

fn load_progress() -> Progress {
    match std::fs::read_to_string(progress_file()) {
        Ok(s) if s.len() <= 4_000_000 => Progress::parse(&s),
        _ => Progress::default(),
    }
}

#[tauri::command]
pub fn podcasts_progress_load() -> Vec<ProgressDto> {
    load_progress().entries().into_iter().map(|(rel, seconds, done)| ProgressDto { rel, seconds, done }).collect()
}

#[tauri::command]
pub fn podcasts_progress_save(rel: String, seconds: u32, done: bool) -> Result<(), String> {
    let mut p = load_progress();
    if !p.set(&rel, seconds, done) {
        return Err("That is not an episode of this library.".into());
    }
    let _ = std::fs::create_dir_all(root().join("user"));
    fsio::write_atomic(&progress_file(), p.to_text().as_bytes()).map_err(|e| e.to_string())
}
