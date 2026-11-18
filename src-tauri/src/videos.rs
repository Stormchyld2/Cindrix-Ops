//! The Videos page's side of the program: the `cindrix-vid` address the web view plays from, and the
//! commands for listing videos and remembering where each was left off. The real work (listing, safe
//! paths, searching, the saved places) is in the core's `videos` module, which has its own tests.
//! Videos are read from `library/videos` on every library that may be read (the main drive's first).

use super::{library_dir, root};
use crate::music::{reply, serve_range};
use cindrix_core::fsio;
use cindrix_core::videoconv;
use cindrix_core::videos::{self, Topic};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The one conversion that may run at a time (a long job; the page asks how it is going).
struct Job {
    rel: String,
    state: &'static str, // running, done, failed, stopped
    message: String,
    started: Instant,
    finished_secs: Option<u64>,
    stop: Arc<AtomicBool>,
}
static JOB: Mutex<Option<Job>> = Mutex::new(None);

fn ffmpeg_path() -> PathBuf {
    super::engine_file(cindrix_core::media::convert::engine_name())
}

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
    let path = decoded.trim_start_matches('/');
    let found = if let Some(rel) = path.strip_prefix("e/") {
        videos::resolve(&video_roots(), rel)
    } else if let Some(name) = path.strip_prefix("c/") {
        videoconv::resolve_cache(&video_roots(), name)
    } else {
        None
    };
    match found {
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
    /// The name of the playable copy, when a converted one has been made.
    converted: Option<String>,
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
    /// True when the converter (ffmpeg.exe in the engines folder) is there.
    converter_ready: bool,
    /// How much room the converted copies take.
    converted_bytes: u64,
    topics: Vec<TopicDto>,
}

fn dto(t: Topic, roots: &[PathBuf]) -> TopicDto {
    TopicDto {
        folder: t.folder,
        name: t.name,
        videos: t
            .videos
            .into_iter()
            .map(|v| {
                let converted = if v.playable { None } else { videoconv::converted_in(roots, &v.rel, v.size, v.modified) };
                VideoDto { rel: v.rel, title: v.title, group: v.group, size: v.size, ext: v.ext, playable: v.playable, thumb: v.thumb, converted }
            })
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
        VideoList {
            folder_found: roots.iter().any(|r| r.is_dir()),
            converter_ready: ffmpeg_path().is_file(),
            converted_bytes: videoconv::cache_size(&roots),
            topics: shown.into_iter().map(|t| dto(t, &roots)).collect(),
        }
    })
    .await
    .unwrap_or(VideoList { folder_found: false, converter_ready: false, converted_bytes: 0, topics: Vec::new() })
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

#[derive(Serialize)]
pub struct ConvertStatus {
    rel: String,
    /// running, done, failed or stopped
    state: String,
    message: String,
    seconds: u64,
}

/// Start making a playable copy of a video the player cannot play. Host only. One at a time.
#[tauri::command]
pub fn video_convert_start(rel: String) -> Result<(), String> {
    let ff = ffmpeg_path();
    if !ff.is_file() {
        return Err("To make this video playable, the small helper ffmpeg.exe has to be in the engines folder on the drive. Put it there and try again, or save the video as an MP4.".into());
    }
    let mut job = JOB.lock().unwrap();
    if let Some(j) = job.as_ref() {
        if j.state == "running" {
            return Err("Another video is being converted. Wait for it to finish, or stop it first.".into());
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    *job = Some(Job { rel: rel.clone(), state: "running", message: String::new(), started: Instant::now(), finished_secs: None, stop: stop.clone() });
    drop(job);
    std::thread::spawn(move || {
        let r = videoconv::convert(&ff, &video_roots(), &rel, &stop);
        let mut g = JOB.lock().unwrap();
        if let Some(j) = g.as_mut() {
            if j.rel == rel {
                j.finished_secs = Some(j.started.elapsed().as_secs());
                match r {
                    Ok(_) => j.state = "done",
                    Err(e) if e == "Stopped." => {
                        j.state = "stopped";
                        j.message = "Stopped.".into();
                    }
                    Err(e) => {
                        j.state = "failed";
                        j.message = e;
                    }
                }
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn video_convert_status() -> Option<ConvertStatus> {
    JOB.lock().unwrap().as_ref().map(|j| ConvertStatus {
        rel: j.rel.clone(),
        state: j.state.to_string(),
        message: j.message.clone(),
        seconds: j.finished_secs.unwrap_or_else(|| j.started.elapsed().as_secs()),
    })
}

#[tauri::command]
pub fn video_convert_stop() {
    if let Some(j) = JOB.lock().unwrap().as_ref() {
        if j.state == "running" {
            j.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Delete the converted copies (the originals are never touched). Returns the bytes freed.
#[tauri::command]
pub fn videos_converted_clear() -> Result<u64, String> {
    if JOB.lock().unwrap().as_ref().map(|j| j.state == "running").unwrap_or(false) {
        return Err("A video is being converted. Stop it first.".into());
    }
    Ok(videoconv::clear_cache(&video_roots()))
}
