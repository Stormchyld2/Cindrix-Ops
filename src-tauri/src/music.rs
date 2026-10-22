//! The music player's side of the program: a quiet background scan of `music/`, the
//! `cindrix-music` address the web view plays from, and the commands the Music screen uses.
//! All the real work (reading tags, the index, safe paths, playlists, converting) is in the
//! core's `media` module, which has its own tests.

use super::{drive_or_program, engine_file, root, AppState};
use cindrix_core::fsio;
use cindrix_core::maps;
use cindrix_core::media::{self, convert, playlist, serve, tags, Class, Query, Track};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Default)]
pub struct MusicState {
    tracks: Arc<Vec<Track>>,
    scanning: bool,
    seen: usize,
    read: usize,
    reused: usize,
    skipped: usize,
    scanned_once: bool,
    /// None until ffmpeg has been asked; Some(None) when there is no working converter.
    target: Option<Option<convert::Target>>,
}

/// One conversion at a time: they are heavy, and two for the same file would clash.
static CONVERT_LOCK: Mutex<()> = Mutex::new(());

fn music_dir() -> PathBuf {
    drive_or_program("music")
}
fn cache_dir() -> PathBuf {
    root().join("user").join("cache").join("music")
}
fn index_file() -> PathBuf {
    root().join("user").join("cache").join("music-index.csv")
}
fn ffmpeg_path() -> PathBuf {
    engine_file(convert::engine_name())
}
fn user_dir() -> PathBuf {
    root().join("user")
}

/// Ask ffmpeg once which format to convert to. Called from background threads only.
fn target(st: &AppState) -> Option<convert::Target> {
    if let Some(t) = st.music.lock().unwrap().target {
        return t;
    }
    let ff = ffmpeg_path();
    let t = if ff.is_file() { convert::pick_target(&ff) } else { None };
    st.music.lock().unwrap().target = Some(t);
    t
}

/// Look through `music/` in the background. Does nothing if a scan is already running.
pub fn start_scan(app: &tauri::AppHandle) {
    {
        let st = app.state::<AppState>();
        let mut m = st.music.lock().unwrap();
        if m.scanning {
            return;
        }
        m.scanning = true;
        m.seen = 0;
    }
    st_cancel(app, false);
    let app = app.clone();
    std::thread::spawn(move || {
        let st = app.state::<AppState>();
        let dir = music_dir();
        let _ = std::fs::create_dir_all(root().join("user").join("cache"));
        // converted copies are only ever kept for the song that is playing, so none survive a restart
        static FIRST: std::sync::Once = std::sync::Once::new();
        FIRST.call_once(|| {
            convert::clear_cache(&cache_dir());
        });
        let idx = index_file();
        let opts = media::ScanOptions { cache: Some(&idx), cancel: Some(&st.music_cancel) };
        let mut n = 0usize;
        let res = media::scan(&dir, &opts, &mut |seen| {
            n += 1;
            if n % 25 == 0 {
                if let Ok(mut m) = st.music.try_lock() {
                    m.seen = seen;
                }
            }
        });
        if !res.cancelled {
            // Find out about the converter too, so the first WMA does not wait for it.
            let has_wma = res.tracks.iter().any(|t| t.class == Class::Convert);
            let mut m = st.music.lock().unwrap();
            m.tracks = Arc::new(res.tracks);
            m.read = res.read;
            m.reused = res.reused;
            m.skipped = res.skipped;
            m.scanned_once = true;
            drop(m);
            if has_wma {
                let _ = target(&st);
            }
        }
        st.music.lock().unwrap().scanning = false;
    });
}

fn st_cancel(app: &tauri::AppHandle, v: bool) {
    app.state::<AppState>().music_cancel.store(v, Ordering::SeqCst);
}

// ---------- the cindrix-music address ----------

fn reply(status: u16, content_range: Option<String>, ctype: &str, body: Vec<u8>) -> tauri::http::Response<Vec<u8>> {
    let mut b = tauri::http::Response::builder()
        .status(status)
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, OPTIONS")
        .header("Access-Control-Allow-Headers", "Range")
        .header("Access-Control-Expose-Headers", "Content-Range, Accept-Ranges, Content-Length")
        .header("Accept-Ranges", "bytes");
    if let Some(r) = content_range {
        b = b.header("Content-Range", r);
    }
    if !body.is_empty() {
        b = b.header("Content-Type", ctype);
    }
    b.body(body).unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
}

fn image_mime(path: &std::path::Path) -> &'static str {
    if path.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case("png")).unwrap_or(false) {
        "image/png"
    } else {
        "image/jpeg"
    }
}

/// Hands the web view audio files, converted copies and cover pictures, and nothing else
/// (see `media::serve::route`). Range requests are answered in pieces of up to 32 MB.
pub fn serve_music(req: &tauri::http::Request<Vec<u8>>) -> tauri::http::Response<Vec<u8>> {
    if *req.method() == tauri::http::Method::OPTIONS {
        return reply(204, None, "", Vec::new());
    }
    if *req.method() != tauri::http::Method::GET {
        return reply(405, None, "", Vec::new());
    }
    let Some(route) = serve::route(&music_dir(), &cache_dir(), req.uri().path()) else {
        return reply(404, None, "", Vec::new());
    };
    match route {
        serve::Route::Art(track) => {
            if let Some(a) = tags::read_art(&track) {
                let mime = if a.mime.is_empty() { "image/jpeg" } else { a.mime };
                return reply(200, None, mime, a.data);
            }
            if let Some(p) = serve::folder_art(&track) {
                if std::fs::metadata(&p).map(|m| m.len() <= tags::MAX_ART as u64).unwrap_or(false) {
                    if let Ok(bytes) = std::fs::read(&p) {
                        return reply(200, None, image_mime(&p), bytes);
                    }
                }
            }
            reply(404, None, "", Vec::new())
        }
        serve::Route::Track(path) | serve::Route::Converted(path) => {
            let Ok(len) = std::fs::metadata(&path).map(|m| m.len()) else {
                return reply(404, None, "", Vec::new());
            };
            let ctype = media::mime_of_ext(&media::ext_of(&path));
            let header = req.headers().get("range").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
            let range = match header {
                Some(h) => maps::parse_range(&h, len),
                // No Range: the whole file when it is small, otherwise the first piece.
                None if len == 0 => Err(maps::RangeError::Unsatisfiable),
                None => Ok((0, (len - 1).min(maps::MAX_RANGE_BYTES - 1))),
            };
            match range {
                Ok((start, end)) => match maps::read_range(&path, start, end) {
                    Ok(bytes) => {
                        let whole = start == 0 && end == len - 1 && req.headers().get("range").is_none();
                        if whole {
                            reply(200, None, ctype, bytes)
                        } else {
                            reply(206, Some(format!("bytes {}-{}/{}", start, end, len)), ctype, bytes)
                        }
                    }
                    Err(_) => reply(500, None, "", Vec::new()),
                },
                Err(maps::RangeError::Unsatisfiable) => reply(416, Some(format!("bytes */{}", len)), "", Vec::new()),
                Err(maps::RangeError::Unsupported) => reply(400, None, "", Vec::new()),
            }
        }
    }
}

// ---------- commands ----------

#[derive(Serialize)]
pub struct TrackDto {
    rel: String,
    title: String,
    artist: String,
    album: String,
    track: u32,
    year: u32,
    ext: String,
    convert: bool,
}

fn dto(t: &Track) -> TrackDto {
    TrackDto { rel: t.rel.clone(), title: t.title.clone(), artist: t.artist.clone(), album: t.album.clone(), track: t.track, year: t.year, ext: t.ext.clone(), convert: t.class == Class::Convert }
}

#[derive(Serialize)]
pub struct MusicStatus {
    /// Is there a `music/` folder at all?
    folder_exists: bool,
    scanning: bool,
    scanned_once: bool,
    seen: usize,
    count: usize,
    read: usize,
    reused: usize,
    skipped: usize,
    /// "opus", "flac", or "none" (no working converter, so WMA and similar cannot play).
    converter: &'static str,
    /// Does the library hold files that need the converter?
    needs_converter: bool,
    cache_bytes: u64,
}

#[tauri::command]
pub fn music_status(state: tauri::State<'_, AppState>) -> MusicStatus {
    let m = state.music.lock().unwrap();
    MusicStatus {
        folder_exists: music_dir().is_dir(),
        scanning: m.scanning,
        scanned_once: m.scanned_once,
        seen: m.seen,
        count: m.tracks.len(),
        read: m.read,
        reused: m.reused,
        skipped: m.skipped,
        converter: match m.target {
            Some(Some(t)) => t.name(),
            Some(None) => "none",
            None => "unknown",
        },
        needs_converter: m.tracks.iter().any(|t| t.class == Class::Convert),
        cache_bytes: convert::cache_size(&cache_dir()),
    }
}

#[tauri::command]
pub fn music_scan(app: tauri::AppHandle) {
    start_scan(&app);
}

#[derive(Serialize)]
pub struct Page {
    total: usize,
    tracks: Vec<TrackDto>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct QueryArgs {
    text: String,
    artist: Option<String>,
    album: Option<String>,
    folder: Option<String>,
    direct: bool,
}

#[tauri::command]
pub fn music_query(state: tauri::State<'_, AppState>, args: QueryArgs, offset: usize, limit: usize) -> Page {
    let lib = state.music.lock().unwrap().tracks.clone();
    let q = Query { text: args.text, artist: args.artist, album: args.album, folder: args.folder, direct: args.direct };
    let (total, rows) = media::query(&lib, &q, offset, limit.min(500));
    Page { total, tracks: rows.iter().map(dto).collect() }
}

#[derive(Serialize)]
pub struct FacetDto {
    name: String,
    sub: String,
    folder: String,
    count: usize,
}

/// `kind` is "artists", "albums" or "folders" (the sub-folders under `prefix`).
#[tauri::command]
pub fn music_facets(state: tauri::State<'_, AppState>, kind: String, prefix: Option<String>) -> Vec<FacetDto> {
    let lib = state.music.lock().unwrap().tracks.clone();
    let f = match kind.as_str() {
        "artists" => media::artists(&lib),
        "albums" => media::albums(&lib),
        "folders" => media::subfolders(&lib, prefix.as_deref().unwrap_or("")),
        _ => vec![],
    };
    f.into_iter().map(|f| FacetDto { name: f.name, sub: f.sub, folder: f.folder, count: f.count }).collect()
}

/// Look up tracks by path (a saved queue or playlist). Paths that are gone are left out.
#[tauri::command]
pub fn music_resolve(state: tauri::State<'_, AppState>, rels: Vec<String>) -> Vec<TrackDto> {
    let lib = state.music.lock().unwrap().tracks.clone();
    media::find(&lib, &rels[..rels.len().min(20_000)]).iter().map(dto).collect()
}

/// Make a track playable. Returns the address path to play: `t/<path>` for what the web view
/// plays itself, `c/<name>` for a converted copy. Converting can take a while, so this runs
/// off the main thread.
#[tauri::command]
pub async fn music_prepare(state: tauri::State<'_, AppState>, rel: String) -> Result<String, String> {
    let dir = music_dir();
    let Some(path) = serve::resolve_track(&dir, &rel) else {
        return Err("That file is no longer in the music folder.".into());
    };
    let class = media::class_of(&path).ok_or("That is not a music file.")?;
    // Songs the web view plays itself (MP3, WAV, FLAC, M4A, OGG...) are played as they are, from the music
    // folder. The path is handed back as plain text; the interface encodes it once.
    // Nothing converted is kept: the copy of an earlier song (a WMA) is deleted as soon as another song starts.
    if class == Class::Native {
        convert::clear_cache(&cache_dir());
        return Ok(format!("t/{}", rel));
    }
    let tg = {
        let m = state.music.lock().unwrap();
        m.target
    };
    let ff = ffmpeg_path();
    let result = tauri::async_runtime::spawn_blocking(move || {
        // Asked again when the answer was "none", so copying ffmpeg in later just works.
        let tg = match tg {
            Some(Some(t)) => Some(t),
            _ => {
                if ff.is_file() { convert::pick_target(&ff) } else { None }
            }
        };
        let Some(tg) = tg else {
            return Err::<(String, Option<convert::Target>), String>(
                "This kind of file (such as WMA) has to be changed into one the player can read while it plays, and the tool that does that (ffmpeg) is not on this drive. Copy ffmpeg into the engines folder, or save the song as an MP3.".to_string(),
            );
        };
        let _g = CONVERT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // keep only this song's copy: whatever was converted for an earlier song is deleted first
        convert::clear_cache(&cache_dir());
        let name = convert::convert(&ff, &path, &rel, &cache_dir(), tg)?;
        Ok((format!("c/{}", name), Some(tg)))
    })
    .await
    .map_err(|e| e.to_string())??;
    state.music.lock().unwrap().target = Some(result.1);
    Ok(result.0)
}

#[tauri::command]
pub fn music_clear_cache() -> u64 {
    convert::clear_cache(&cache_dir())
}

// ---------- playlists ----------

#[derive(Serialize)]
pub struct PlaylistMeta {
    name: String,
    count: usize,
}

#[derive(Serialize, Deserialize)]
pub struct EntryDto {
    path: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
}

#[tauri::command]
pub fn music_playlists() -> Vec<PlaylistMeta> {
    playlist::Store::new(&user_dir()).list().into_iter().map(|m| PlaylistMeta { name: m.name, count: m.count }).collect()
}

#[tauri::command]
pub fn music_playlist_load(name: String) -> Result<Vec<EntryDto>, String> {
    Ok(playlist::Store::new(&user_dir()).load(&name)?.into_iter().map(|e| EntryDto { path: e.path, title: e.title, artist: e.artist, album: e.album }).collect())
}

#[tauri::command]
pub fn music_playlist_save(name: String, entries: Vec<EntryDto>) -> Result<(), String> {
    let v: Vec<playlist::Entry> = entries.into_iter().map(|e| playlist::Entry { path: e.path, title: e.title, artist: e.artist, album: e.album }).collect();
    playlist::Store::new(&user_dir()).save(&name, &v)
}

#[tauri::command]
pub fn music_playlist_delete(name: String) -> Result<(), String> {
    playlist::Store::new(&user_dir()).delete(&name)
}

// ---------- remembered player settings ----------

fn state_file() -> PathBuf {
    user_dir().join("music-state.json")
}

/// What the player remembers (volume, equalizer, queue). A JSON object, or "{}".
#[tauri::command]
pub fn music_state_load() -> String {
    match std::fs::read_to_string(state_file()) {
        Ok(s) if s.trim_start().starts_with('{') && s.len() <= 1_000_000 => s,
        _ => "{}".to_string(),
    }
}

#[tauri::command]
pub fn music_state_save(json: String) -> Result<(), String> {
    let t = json.trim_start();
    if !t.starts_with('{') || !json.trim_end().ends_with('}') || json.len() > 1_000_000 {
        return Err("That is not a player state.".into());
    }
    fsio::write_atomic(&state_file(), json.as_bytes()).map_err(|e| e.to_string())
}
