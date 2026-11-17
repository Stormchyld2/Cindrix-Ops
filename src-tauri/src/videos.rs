//! The Videos page's side of the program: the `cindrix-vid` address the web view plays from, and the
//! commands for listing videos and remembering where each was left off. The real work (listing, safe
//! paths, searching, the saved places) is in the core's `videos` module, which has its own tests.
//! Videos are read from `library/videos` on every library that may be read (the main drive's first).

use super::{library_dir, root};
use crate::music::{reply, serve_range};
use cindrix_core::fsio;
use cindrix_core::videos::{self, Topic};
use serde::Serialize;
use std::path::PathBuf;

/// Every `videos` folder that may be read: the main drive's, then those of the other libraries.
fn video_roots() -> Vec<PathBuf> {
    let mut v = vec![library_dir().join("videos")];
    for l in crate::libs::library_roots() {
        let p = l.join("videos");
        if !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

fn progress_file() -> PathBuf {
    root().join("user").join("video-progress.txt")
}

/// Hands the web view video files and their thumbnail pictures, and nothing else: `/e/<path under videos/>`.
pub fn serve_video(req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
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
    match videos::resolve(&video_roots(), rel) {
        Some(path) => serve_range(req, &path),
        None => reply(404, None, "", Vec::new()),
    }
}

#[derive(Serialize)]
pub struct VideoDto {
    rel: String,
    title: String,
    group: String,
    size: u64,
    ext: String,
    playable: bool,
    thumb: Option<String>,
}

#[derive(Serialize)]
pub struct TopicDto {
    folder: String,
    name: String,
    videos: Vec<VideoDto>,
}

#[derive(Serialize)]
pub struct VideoList {
    /// False when no library has a `videos` folder at all (so the page can say where to put one).
    folder_found: bool,
    topics: Vec<TopicDto>,
}

fn dto(t: Topic) -> TopicDto {
    TopicDto {
        folder: t.folder,
        name: t.name,
        videos: t
            .videos
            .into_iter()
            .map(|v| VideoDto { rel: v.rel, title: v.title, group: v.group, size: v.size, ext: v.ext, playable: v.playable, thumb: v.thumb })
            .collect(),
    }
}

/// Every topic and video, narrowed by the words in `query` when there are any. Read on demand: a folder of
/// videos is a small list of names, so there is no background scan.
#[tauri::command]
pub async fn videos_list(query: Option<String>) -> VideoList {
    tauri::async_runtime::spawn_blocking(move || {
        let roots = video_roots();
        let all = videos::scan_all(&roots);
        let shown = videos::search(&all, query.as_deref().unwrap_or(""));
        VideoList { folder_found: roots.iter().any(|r| r.is_dir()), topics: shown.into_iter().map(dto).collect() }
    })
    .await
    .unwrap_or(VideoList { folder_found: false, topics: Vec::new() })
}

#[derive(Serialize)]
pub struct ProgressDto {
    rel: String,
    seconds: u32,
    done: bool,
}

fn load_progress() -> cindrix_core::podcasts::Progress {
    match std::fs::read_to_string(progress_file()) {
        Ok(s) if s.len() <= 4_000_000 => videos::progress_from(&s),
        _ => Default::default(),
    }
}

#[tauri::command]
pub fn videos_progress_load() -> Vec<ProgressDto> {
    load_progress().entries().into_iter().map(|(rel, seconds, done)| ProgressDto { rel, seconds, done }).collect()
}

#[tauri::command]
pub fn videos_progress_save(rel: String, seconds: u32, done: bool) -> Result<(), String> {
    let mut p = load_progress();
    if !p.set(&rel, seconds, done) {
        return Err("That is not a video of this library.".into());
    }
    let _ = std::fs::create_dir_all(root().join("user"));
    fsio::write_atomic(&progress_file(), p.to_text().as_bytes()).map_err(|e| e.to_string())
}
