//! The music library: which files count as audio, what is known about each one, searching and
//! browsing, and the cached index that keeps a rescan of a big drive fast. Standard library only.
//!
//! The owner's own music lives in the drive's `music/` folder. This program never ships music
//! and never changes or deletes a music file. Everything it writes (the index, converted copies,
//! playlists, the player's state) goes under `user/`.

pub mod convert;
pub mod playlist;
pub mod serve;
pub mod tags;

use crate::fsio;
use crate::inventory::{quote, split_record};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;
use tags::Tags;

/// Can the web view play this file by itself, or does it need converting first?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Native,
    Convert,
}

/// Formats the web view plays: Windows (WebView2) and Android. **TO TEST** on both, for the
/// less common ones (raw AAC, WebM audio).
pub const NATIVE: &[&str] = &["mp3", "wav", "wave", "ogg", "oga", "opus", "flac", "m4a", "m4b", "aac", "weba"];
/// Formats that need the converter (ffmpeg) before the web view can play them.
pub const CONVERT: &[&str] = &["wma", "aif", "aiff", "aifc", "ape", "wv", "mpc", "tta", "amr", "au", "ac3", "mka", "mp2"];

pub fn class_of_ext(ext: &str) -> Option<Class> {
    let e = ext.to_ascii_lowercase();
    if NATIVE.contains(&e.as_str()) {
        Some(Class::Native)
    } else if CONVERT.contains(&e.as_str()) {
        Some(Class::Convert)
    } else {
        None
    }
}

pub fn ext_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn class_of(path: &Path) -> Option<Class> {
    class_of_ext(&ext_of(path))
}

pub fn mime_of_ext(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "mp3" | "mp2" => "audio/mpeg",
        "wav" | "wave" => "audio/wav",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" | "m4b" => "audio/mp4",
        "aac" => "audio/aac",
        "weba" => "audio/webm",
        _ => "application/octet-stream",
    }
}

/// One folder or file name inside the music folder. Nothing that could leave the folder or mean
/// something else on Windows: no separators, drive colons, wildcards or control characters.
pub fn valid_part(p: &str) -> bool {
    !p.is_empty()
        && p != "."
        && p != ".."
        && p.len() <= 255
        && !p.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
}

/// A path under the music folder, with `/` separators and no empty, `.` or `..` parts.
pub fn valid_rel(rel: &str) -> bool {
    !rel.is_empty() && rel.len() <= 1024 && rel.split('/').all(valid_part)
}

const MAX_DEPTH: u8 = 12;

fn skip_dir(name: &str) -> bool {
    name.starts_with('.') || name.starts_with('$') || name.eq_ignore_ascii_case("System Volume Information")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Path under the music folder, with `/` separators.
    pub rel: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track: u32,
    pub year: u32,
    /// The genre named in the file; empty when it names none (then the top folder is used, see `genre()`).
    pub tag_genre: String,
    pub size: u64,
    pub mtime: u64,
    pub ext: String,
    pub class: Class,
    /// Lower-case text the search looks in.
    hay: String,
    hay_title: String,
    hay_artist: String,
    hay_genre: String,
}

impl Track {
    fn new(rel: String, size: u64, mtime: u64, ext: String, class: Class, t: Tags) -> Track {
        let genre = genre_of(&rel, &t.genre);
        let hay = format!("{} {} {} {} {}", t.title, t.artist, t.album, genre, rel).to_lowercase();
        let (hay_title, hay_artist, hay_genre) = (t.title.to_lowercase(), t.artist.to_lowercase(), genre.to_lowercase());
        Track { rel, title: t.title, artist: t.artist, album: t.album, track: t.track, year: t.year, tag_genre: t.genre, size, mtime, ext, class, hay, hay_title, hay_artist, hay_genre }
    }

    /// The genre: what the file's tag says, otherwise the top folder it sits in (`music/Rock/Band/song.mp3` is "Rock"), otherwise "".
    pub fn genre(&self) -> String {
        genre_of(&self.rel, &self.tag_genre)
    }

    /// The folder this track sits in, "" when it is directly in the music folder.
    pub fn folder(&self) -> &str {
        self.rel.rsplit_once('/').map(|(f, _)| f).unwrap_or("")
    }
}

// ---------- names, when the file has no tags ----------

/// "03 - Song", "03. Song", "03_Song", "3-Song": a leading track number, then the rest.
/// A single digit needs a dot, dash or underscore, so "7 Years" stays whole.
fn split_track_prefix(stem: &str) -> (u32, &str) {
    let digits = stem.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return (0, stem);
    }
    let rest = &stem[digits..];
    let seps = rest.chars().take_while(|c| matches!(c, ' ' | '.' | '-' | '_')).count();
    if seps == 0 || rest.len() == seps {
        return (0, stem);
    }
    let sep: String = rest.chars().take(seps).collect();
    if digits == 1 && sep == " " {
        return (0, stem);
    }
    (stem[..digits].parse().unwrap_or(0), rest[seps..].trim())
}

/// The genre of a track: its tag, else its top folder (when it sits in a folder at all).
pub fn genre_of(rel: &str, tag: &str) -> String {
    let tag = tag.trim();
    if !tag.is_empty() {
        return tag.to_string();
    }
    match rel.split_once('/') {
        Some((top, _)) if !top.is_empty() => top.to_string(),
        _ => String::new(),
    }
}

/// Fill what the tags left empty from the file and folder names: `Artist/Album/03 - Title.mp3`
/// or `Artist - Title.mp3`. These are guesses and only ever fill blanks.
pub fn guess(rel: &str, raw: &Tags) -> Tags {
    let parts: Vec<&str> = rel.split('/').collect();
    let file = parts.last().copied().unwrap_or("");
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    let (n, rest) = split_track_prefix(stem);
    let mut t = raw.clone();
    if t.title.is_empty() {
        let mut title = rest.to_string();
        if t.artist.is_empty() {
            if let Some((a, b)) = title.split_once(" - ") {
                if !a.trim().is_empty() && !b.trim().is_empty() {
                    t.artist = a.trim().to_string();
                    title = b.trim().to_string();
                }
            }
        }
        t.title = if title.is_empty() { stem.to_string() } else { title };
    }
    if t.track == 0 {
        t.track = n;
    }
    if t.album.is_empty() && parts.len() >= 2 {
        t.album = parts[parts.len() - 2].to_string();
    }
    if t.artist.is_empty() && parts.len() >= 3 {
        t.artist = parts[parts.len() - 3].to_string();
    }
    t
}

// ---------- the index cache (a CSV in user/cache) ----------

const CACHE_HEADER: &str = "rel,size,mtime,title,artist,album,track,year,genre";

struct CacheRow {
    size: u64,
    mtime: u64,
    tags: Tags,
}

fn load_cache(path: &Path) -> HashMap<String, CacheRow> {
    let mut m = HashMap::new();
    let Ok(text) = fs::read_to_string(path) else { return m };
    let mut lines = text.lines();
    if lines.next() != Some(CACHE_HEADER) {
        return m;
    }
    for line in lines {
        let f = split_record(line);
        if f.len() != 9 {
            continue;
        }
        let (Ok(size), Ok(mtime), Ok(track), Ok(year)) = (f[1].parse(), f[2].parse(), f[6].parse(), f[7].parse()) else { continue };
        m.insert(f[0].clone(), CacheRow { size, mtime, tags: Tags { title: f[3].clone(), artist: f[4].clone(), album: f[5].clone(), track, year, genre: f[8].clone() } });
    }
    m
}

fn flat(s: &str) -> String {
    quote(&s.replace(['\n', '\r'], " "))
}

fn write_cache(path: &Path, rows: &[(String, u64, u64, Tags)]) -> std::io::Result<()> {
    let mut out = String::from(CACHE_HEADER);
    out.push('\n');
    for (rel, size, mtime, t) in rows {
        out.push_str(&format!("{},{},{},{},{},{},{},{},{}\n", flat(rel), size, mtime, flat(&t.title), flat(&t.artist), flat(&t.album), t.track, t.year, flat(&t.genre)));
    }
    fsio::write_atomic(path, out.as_bytes())
}

// ---------- scanning ----------

pub struct ScanOptions<'a> {
    /// Where to keep the index of tags, so unchanged files are not read again. Safe to delete.
    pub cache: Option<&'a Path>,
    /// Set to true from another thread to stop early.
    pub cancel: Option<&'a AtomicBool>,
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub tracks: Vec<Track>,
    /// Files whose tags were read this time.
    pub read: usize,
    /// Files whose tags came from the cached index.
    pub reused: usize,
    /// Audio files left out because their names cannot be served safely or are not text.
    pub skipped: usize,
    pub cancelled: bool,
}

fn sort_key(t: &Track) -> (String, String, u32, String, &str) {
    (t.artist.to_lowercase(), t.album.to_lowercase(), t.track, t.title.to_lowercase(), t.rel.as_str())
}

/// Find every audio file under `root`. Symbolic links and hidden or system folders are not
/// followed. `progress` is told how many audio files have been seen so far.
pub fn scan(root: &Path, opts: &ScanOptions, progress: &mut dyn FnMut(usize)) -> ScanResult {
    let cache = opts.cache.map(load_cache).unwrap_or_default();
    let mut res = ScanResult::default();
    let mut rows: Vec<(String, u64, u64, Tags)> = Vec::new();
    let mut seen = 0usize;
    let mut stack = vec![(root.to_path_buf(), 0u8)];
    'walk: while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            if opts.cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false) {
                res.cancelled = true;
                break 'walk;
            }
            let Ok(ft) = e.file_type() else { continue };
            let path = e.path();
            if ft.is_dir() {
                let name = e.file_name();
                if depth < MAX_DEPTH && !name.to_str().map(skip_dir).unwrap_or(true) {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let ext = ext_of(&path);
            let Some(class) = class_of_ext(&ext) else { continue };
            seen += 1;
            if seen.is_multiple_of(32) {
                progress(seen);
            }
            let Ok(meta) = e.metadata() else { continue };
            if meta.len() == 0 {
                continue;
            }
            let rel = path.strip_prefix(root).ok().and_then(|p| p.components().map(|c| c.as_os_str().to_str()).collect::<Option<Vec<_>>>()).map(|v| v.join("/"));
            let Some(rel) = rel.filter(|r| valid_rel(r)) else {
                res.skipped += 1;
                continue;
            };
            let size = meta.len();
            let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            let raw = match cache.get(&rel) {
                Some(c) if c.size == size && c.mtime == mtime => {
                    res.reused += 1;
                    c.tags.clone()
                }
                _ => {
                    res.read += 1;
                    tags::read_tags(&path)
                }
            };
            let eff = guess(&rel, &raw);
            rows.push((rel.clone(), size, mtime, raw));
            res.tracks.push(Track::new(rel, size, mtime, ext, class, eff));
        }
    }
    progress(seen);
    res.tracks.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));
    if !res.cancelled {
        if let Some(c) = opts.cache {
            rows.sort_by(|a, b| a.0.cmp(&b.0));
            let _ = write_cache(c, &rows); // best effort: the index is only a speed-up
        }
    }
    res
}

// ---------- searching and browsing ----------

#[derive(Debug, Default, Clone)]
pub struct Query {
    /// Every word must appear in the title, artist, album or path.
    pub text: String,
    /// Exactly this artist. An empty string means tracks with no artist.
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Tracks in this folder and below.
    pub folder: Option<String>,
    /// With `folder`: only tracks directly in it, not in sub-folders.
    pub direct: bool,
    /// Exactly this genre ("" means tracks with no genre).
    pub genre: Option<String>,
    /// Where the words must appear: "title", "artist", "genre", or anything else (or empty) for everywhere.
    pub field: String,
}

fn in_folder(t: &Track, folder: &str, direct: bool) -> bool {
    if direct {
        return t.folder() == folder;
    }
    folder.is_empty() || t.rel.strip_prefix(folder).map(|r| r.starts_with('/')).unwrap_or(false)
}

fn matches(t: &Track, q: &Query, words: &[String]) -> bool {
    if let Some(a) = &q.artist {
        if t.artist.to_lowercase() != a.to_lowercase() {
            return false;
        }
    }
    if let Some(a) = &q.album {
        if t.album.to_lowercase() != a.to_lowercase() {
            return false;
        }
    }
    if let Some(g) = &q.genre {
        if t.genre().to_lowercase() != g.to_lowercase() {
            return false;
        }
    }
    if let Some(f) = &q.folder {
        if !in_folder(t, f, q.direct) {
            return false;
        }
    }
    let hay = match q.field.as_str() {
        "title" => &t.hay_title,
        "artist" => &t.hay_artist,
        "genre" => &t.hay_genre,
        _ => &t.hay,
    };
    words.iter().all(|w| hay.contains(w.as_str()))
}

/// The tracks that match, from `offset`, at most `limit`, in library order. Also the total.
pub fn query(lib: &[Track], q: &Query, offset: usize, limit: usize) -> (usize, Vec<Track>) {
    let words: Vec<String> = q.text.to_lowercase().split_whitespace().map(|w| w.to_string()).collect();
    let mut total = 0usize;
    let mut out = Vec::new();
    for t in lib.iter().filter(|t| matches(t, q, &words)) {
        if total >= offset && out.len() < limit {
            out.push(t.clone());
        }
        total += 1;
    }
    (total, out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Facet {
    pub name: String,
    /// For an album, its artist ("Various artists" when they differ); for a folder, its full path.
    pub sub: String,
    /// The folder an album sits in (so two albums with the same name stay apart).
    pub folder: String,
    pub count: usize,
}

pub fn artists(lib: &[Track]) -> Vec<Facet> {
    let mut m: HashMap<String, Facet> = HashMap::new();
    for t in lib {
        let e = m.entry(t.artist.to_lowercase()).or_insert_with(|| Facet { name: t.artist.clone(), sub: String::new(), folder: String::new(), count: 0 });
        e.count += 1;
    }
    let mut v: Vec<Facet> = m.into_values().collect();
    v.sort_by_key(|f| f.name.to_lowercase());
    v
}

/// The genres, with how many tracks each holds. Tracks with none are listed together as "Not sorted".
pub fn genres(lib: &[Track]) -> Vec<Facet> {
    let mut m: HashMap<String, Facet> = HashMap::new();
    for t in lib {
        let g = t.genre();
        let e = m.entry(g.to_lowercase()).or_insert_with(|| Facet { name: g.clone(), sub: String::new(), folder: String::new(), count: 0 });
        e.count += 1;
    }
    let mut v: Vec<Facet> = m.into_values().collect();
    v.sort_by_key(|f| (f.name.is_empty(), f.name.to_lowercase()));
    v
}

pub fn albums(lib: &[Track]) -> Vec<Facet> {
    let mut m: HashMap<(String, String), Facet> = HashMap::new();
    for t in lib.iter().filter(|t| !t.album.is_empty()) {
        let e = m.entry((t.album.to_lowercase(), t.folder().to_string())).or_insert_with(|| Facet {
            name: t.album.clone(),
            sub: t.artist.clone(),
            folder: t.folder().to_string(),
            count: 0,
        });
        if !e.sub.eq_ignore_ascii_case(&t.artist) {
            e.sub = "Various artists".to_string();
        }
        e.count += 1;
    }
    let mut v: Vec<Facet> = m.into_values().collect();
    v.sort_by(|a, b| (a.name.to_lowercase(), &a.folder).cmp(&(b.name.to_lowercase(), &b.folder)));
    v
}

/// The folders directly under `prefix` ("" for the top), each with the number of tracks below it.
pub fn subfolders(lib: &[Track], prefix: &str) -> Vec<Facet> {
    let mut m: HashMap<String, usize> = HashMap::new();
    for t in lib {
        let rest = if prefix.is_empty() { Some(t.rel.as_str()) } else { t.rel.strip_prefix(prefix).and_then(|r| r.strip_prefix('/')) };
        if let Some((first, _)) = rest.and_then(|r| r.split_once('/')) {
            *m.entry(first.to_string()).or_insert(0) += 1;
        }
    }
    let mut v: Vec<Facet> = m
        .into_iter()
        .map(|(name, count)| Facet { sub: String::new(), folder: if prefix.is_empty() { name.clone() } else { format!("{}/{}", prefix, name) }, name, count })
        .collect();
    v.sort_by_key(|f| f.name.to_lowercase());
    v
}

/// Look tracks up by path, in the order asked, leaving out any that are not in the library.
pub fn find(lib: &[Track], rels: &[String]) -> Vec<Track> {
    let idx: HashMap<&str, &Track> = lib.iter().map(|t| (t.rel.as_str(), t)).collect();
    rels.iter().filter_map(|r| idx.get(r.as_str()).map(|t| (*t).clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-media-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }

    fn scan_plain(root: &Path, cache: Option<&Path>) -> ScanResult {
        scan(root, &ScanOptions { cache, cancel: None }, &mut |_| {})
    }

    #[test]
    fn formats_are_classified_and_unknown_ones_are_not_audio() {
        for e in ["mp3", "WAV", "flac", "ogg", "opus", "m4a", "aac"] {
            assert_eq!(class_of_ext(e), Some(Class::Native), "{}", e);
        }
        for e in ["wma", "WMA", "aiff", "ape", "wv"] {
            assert_eq!(class_of_ext(e), Some(Class::Convert), "{}", e);
        }
        for e in ["txt", "jpg", "exe", "", "pmtiles", "zim", "mid"] {
            assert_eq!(class_of_ext(e), None, "{}", e);
        }
        assert_eq!(mime_of_ext("MP3"), "audio/mpeg");
        assert_eq!(mime_of_ext("opus"), "audio/ogg");
    }

    #[test]
    fn names_that_could_escape_are_refused() {
        assert!(valid_rel("Artist/Album/01 - Song (live) & more.mp3"));
        assert!(valid_rel("Beyoncé/Lemonade/Halo's.flac"));
        for bad in ["", "/a.mp3", "a//b.mp3", "../a.mp3", "a/../b.mp3", "a\\b.mp3", "C:\\x.mp3", "a:b.mp3", "a/b.mp3/", "a*.mp3", "a\0b.mp3", "a\nb.mp3"] {
            assert!(!valid_rel(bad), "{:?}", bad);
        }
    }

    #[test]
    fn track_prefix_rules() {
        assert_eq!(split_track_prefix("03 - Song"), (3, "Song"));
        assert_eq!(split_track_prefix("03. Song"), (3, "Song"));
        assert_eq!(split_track_prefix("12_Song"), (12, "Song"));
        assert_eq!(split_track_prefix("7-Song"), (7, "Song"));
        assert_eq!(split_track_prefix("7 Years"), (0, "7 Years")); // a title that starts with a number
        assert_eq!(split_track_prefix("1999"), (0, "1999"));
        assert_eq!(split_track_prefix("2112 Overture"), (0, "2112 Overture"));
        assert_eq!(split_track_prefix("Song"), (0, "Song"));
        assert_eq!(split_track_prefix("05 -"), (0, "05 -"));
    }

    #[test]
    fn untagged_files_are_named_from_their_path() {
        let g = guess("Camp Band/Fire Songs/03 - Stay Warm.wma", &Tags::default());
        assert_eq!((g.title.as_str(), g.artist.as_str(), g.album.as_str(), g.track), ("Stay Warm", "Camp Band", "Fire Songs", 3));
        let g = guess("Some Band - Some Song.mp3", &Tags::default());
        assert_eq!((g.title.as_str(), g.artist.as_str(), g.album.as_str()), ("Some Song", "Some Band", ""));
        // tags win; only blanks are filled
        let raw = Tags { title: "Real Title".into(), artist: String::new(), album: "Real Album".into(), track: 0, year: 2001, genre: String::new() };
        let g = guess("A/B/09 - Other.mp3", &raw);
        assert_eq!((g.title.as_str(), g.artist.as_str(), g.album.as_str(), g.track, g.year), ("Real Title", "A", "Real Album", 9, 2001));
    }

    #[test]
    fn scan_finds_audio_only_and_sorts_by_artist_album_track() {
        let root = tmp("scan");
        put(&root, "Zed/Late/02 - B.mp3", b"x");
        put(&root, "Zed/Late/01 - A.mp3", b"x");
        put(&root, "Abe/First/01 - Z.wma", b"x");
        put(&root, "Abe/First/cover.jpg", b"x");
        put(&root, "Abe/First/notes.txt", b"x");
        put(&root, "Abe/First/empty.mp3", b"");
        put(&root, ".hidden/01 - H.mp3", b"x");
        put(&root, "$RECYCLE.BIN/01 - R.mp3", b"x");
        let r = scan_plain(&root, None);
        let names: Vec<_> = r.tracks.iter().map(|t| t.rel.as_str()).collect();
        assert_eq!(names, vec!["Abe/First/01 - Z.wma", "Zed/Late/01 - A.mp3", "Zed/Late/02 - B.mp3"]);
        assert_eq!(r.tracks[0].class, Class::Convert);
        assert_eq!(r.tracks[1].class, Class::Native);
        assert_eq!(r.tracks[0].title, "Z");
        assert_eq!((r.read, r.reused, r.skipped), (3, 0, 0));
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed_and_odd_names_are_counted() {
        let root = tmp("links");
        let outside = tmp("links-outside");
        put(&outside, "secret.mp3", b"x");
        std::os::unix::fs::symlink(&outside, root.join("out")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.mp3"), root.join("link.mp3")).unwrap();
        put(&root, "Colon: Song.mp3", b"x");
        put(&root, "Fine.mp3", b"x");
        let r = scan_plain(&root, None);
        assert_eq!(r.tracks.iter().map(|t| t.rel.as_str()).collect::<Vec<_>>(), vec!["Fine.mp3"]);
        assert_eq!(r.skipped, 1);
    }

    #[test]
    fn the_index_cache_skips_unchanged_files_and_notices_changes() {
        let root = tmp("cache");
        let cache = tmp("cache-dir").join("music-index.csv");
        put(&root, "A/B/01 - One, with commas.mp3", b"aaaa");
        put(&root, "A/B/02 - Two.mp3", b"bbbb");
        let first = scan_plain(&root, Some(&cache));
        assert_eq!((first.read, first.reused), (2, 0));
        assert!(cache.exists());
        let second = scan_plain(&root, Some(&cache));
        assert_eq!((second.read, second.reused), (0, 2));
        assert_eq!(first.tracks, second.tracks);
        // a changed size means a re-read; a new file is read; a removed file disappears
        put(&root, "A/B/02 - Two.mp3", b"bbbbbbbb");
        put(&root, "A/B/03 - Three.mp3", b"c");
        fs::remove_file(root.join("A/B/01 - One, with commas.mp3")).unwrap();
        let third = scan_plain(&root, Some(&cache));
        assert_eq!((third.read, third.reused, third.tracks.len()), (2, 0, 2));
        // a damaged or old-format cache is ignored, not trusted
        fs::write(&cache, "garbage\nnot,a,row\n").unwrap();
        let fourth = scan_plain(&root, Some(&cache));
        assert_eq!((fourth.read, fourth.reused), (2, 0));
    }

    #[test]
    fn a_cancelled_scan_stops_and_leaves_the_cache_alone() {
        let root = tmp("cancel");
        put(&root, "a.mp3", b"x");
        let cache = tmp("cancel-dir").join("i.csv");
        let stop = AtomicBool::new(true);
        let r = scan(&root, &ScanOptions { cache: Some(&cache), cancel: Some(&stop) }, &mut |_| {});
        assert!(r.cancelled);
        assert!(!cache.exists());
    }

    fn library() -> Vec<Track> {
        let t = |rel: &str, title: &str, artist: &str, album: &str, n: u32| {
            Track::new(rel.to_string(), 1, 1, "mp3".into(), Class::Native, Tags { title: title.into(), artist: artist.into(), album: album.into(), track: n, year: 0, genre: String::new() })
        };
        vec![
            t("Rock/Amp/Loud/01.mp3", "Feedback", "Amp", "Loud", 1),
            t("Rock/Amp/Loud/02.mp3", "Distortion", "Amp", "Loud", 2),
            t("Rock/Amp/Quiet/01.mp3", "Whisper", "Amp", "Quiet", 1),
            t("Mix/Various/01.mp3", "Campfire", "Ember", "Gathering", 1),
            t("Mix/Various/02.mp3", "Lantern", "Wick", "Gathering", 2),
            t("Loose.mp3", "Single", "", "", 0),
        ]
    }

    #[test]
    fn search_needs_every_word_and_ignores_case() {
        let lib = library();
        let (n, v) = query(&lib, &Query { text: "amp LOUD".into(), ..Default::default() }, 0, 10);
        assert_eq!(n, 2);
        assert_eq!(v[0].title, "Feedback");
        assert_eq!(query(&lib, &Query { text: "camp lantern".into(), ..Default::default() }, 0, 10).0, 0);
        assert_eq!(query(&lib, &Query { text: "  ".into(), ..Default::default() }, 0, 100).0, 6);
        assert_eq!(query(&lib, &Query { text: "mix".into(), ..Default::default() }, 0, 10).0, 2); // the path counts
    }

    #[test]
    fn genre_comes_from_the_tag_else_the_top_folder() {
        let mk = |rel: &str, title: &str, artist: &str, genre: &str| {
            Track::new(rel.to_string(), 1, 1, "mp3".into(), Class::Native, Tags { title: title.into(), artist: artist.into(), album: String::new(), track: 0, year: 0, genre: genre.into() })
        };
        let lib = vec![
            mk("Rock/Amp/a.mp3", "Feedback", "Amp", ""),
            mk("Rock/Amp/b.mp3", "Static", "Amp", "Blues"),
            mk("Jazz/c.mp3", "Smoke", "Horn", ""),
            mk("loose.mp3", "Single", "Solo", ""),
        ];
        assert_eq!(lib[0].genre(), "Rock");
        assert_eq!(lib[1].genre(), "Blues"); // the tag wins over the folder
        assert_eq!(lib[3].genre(), "");
        let g = genres(&lib);
        let names: Vec<&str> = g.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["Blues", "Jazz", "Rock", ""]); // unsorted last
        assert_eq!(query(&lib, &Query { genre: Some("rock".into()), ..Default::default() }, 0, 9).0, 1);
        assert_eq!(query(&lib, &Query { genre: Some(String::new()), ..Default::default() }, 0, 9).0, 1);
        // search only in one field
        assert_eq!(query(&lib, &Query { text: "amp".into(), field: "artist".into(), ..Default::default() }, 0, 9).0, 2);
        assert_eq!(query(&lib, &Query { text: "amp".into(), field: "title".into(), ..Default::default() }, 0, 9).0, 0);
        assert_eq!(query(&lib, &Query { text: "blues".into(), field: "genre".into(), ..Default::default() }, 0, 9).0, 1);
        assert_eq!(query(&lib, &Query { text: "jazz".into(), field: "title".into(), ..Default::default() }, 0, 9).0, 0);
        assert_eq!(query(&lib, &Query { text: "jazz".into(), ..Default::default() }, 0, 9).0, 1);
    }

    #[test]
    fn paging_gives_the_total_and_a_slice() {
        let lib = library();
        let (n, v) = query(&lib, &Query::default(), 2, 2);
        assert_eq!((n, v.len(), v[0].title.as_str()), (6, 2, "Whisper"));
        assert_eq!(query(&lib, &Query::default(), 10, 5).1.len(), 0);
    }

    #[test]
    fn filters_by_artist_album_and_folder() {
        let lib = library();
        assert_eq!(query(&lib, &Query { artist: Some("amp".into()), ..Default::default() }, 0, 99).0, 3);
        assert_eq!(query(&lib, &Query { artist: Some(String::new()), ..Default::default() }, 0, 99).0, 1); // no artist
        assert_eq!(query(&lib, &Query { album: Some("Gathering".into()), ..Default::default() }, 0, 99).0, 2);
        assert_eq!(query(&lib, &Query { folder: Some("Rock/Amp".into()), ..Default::default() }, 0, 99).0, 3);
        assert_eq!(query(&lib, &Query { folder: Some("Rock/Amp".into()), direct: true, ..Default::default() }, 0, 99).0, 0);
        assert_eq!(query(&lib, &Query { folder: Some("Rock/Amp/Loud".into()), direct: true, ..Default::default() }, 0, 99).0, 2);
        assert_eq!(query(&lib, &Query { folder: Some("Rock/Am".into()), ..Default::default() }, 0, 99).0, 0); // not a part-name match
        assert_eq!(query(&lib, &Query { folder: Some(String::new()), direct: true, ..Default::default() }, 0, 99).0, 1); // top level
    }

    #[test]
    fn artist_album_and_folder_lists() {
        let lib = library();
        let a = artists(&lib);
        assert_eq!(a.iter().map(|f| (f.name.as_str(), f.count)).collect::<Vec<_>>(), vec![("", 1), ("Amp", 3), ("Ember", 1), ("Wick", 1)]);
        let al = albums(&lib);
        let names: Vec<_> = al.iter().map(|f| (f.name.as_str(), f.sub.as_str(), f.count)).collect();
        assert_eq!(names, vec![("Gathering", "Various artists", 2), ("Loud", "Amp", 2), ("Quiet", "Amp", 1)]);
        let top = subfolders(&lib, "");
        assert_eq!(top.iter().map(|f| (f.name.as_str(), f.count)).collect::<Vec<_>>(), vec![("Mix", 2), ("Rock", 3)]);
        let rock = subfolders(&lib, "Rock/Amp");
        assert_eq!(rock.iter().map(|f| (f.folder.as_str(), f.count)).collect::<Vec<_>>(), vec![("Rock/Amp/Loud", 2), ("Rock/Amp/Quiet", 1)]);
        assert!(subfolders(&lib, "Nothing").is_empty());
    }

    #[test]
    fn find_keeps_order_and_drops_missing() {
        let lib = library();
        let got = find(&lib, &["Loose.mp3".into(), "gone.mp3".into(), "Mix/Various/01.mp3".into()]);
        assert_eq!(got.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), vec!["Single", "Campfire"]);
    }
}
