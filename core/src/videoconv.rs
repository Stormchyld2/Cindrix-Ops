//! Making videos the player cannot play (MKV, AVI, MOV, WMV and the like) playable, once, with ffmpeg:
//! a separate program in the drive's `engines/` folder that this program starts (see `media::convert`
//! for the music side). The new copy is kept in a hidden `.converted` folder inside the `videos` folder
//! the original lives in, so it is found again by any host that reads that drive. The original is never
//! touched, and deleting `.converted` is always safe.
//!
//! The quick way is used whenever it can be: when the picture is already H.264 (or VP8, VP9 or AV1) it is
//! copied as it is, which takes seconds, and only a sound the player cannot use is re-made. Only other
//! pictures (old DivX, WMV, MPEG-2 and so on) are re-encoded, which takes a while.
//! ffmpeg is told to read local files only (`-protocol_whitelist file`).

use crate::md5::Md5;
use crate::media::valid_rel;
use crate::videos::NEEDS_CONVERTING;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The folder (inside a `videos` folder) that holds the converted copies.
pub const CACHE_DIR: &str = ".converted";
/// How long one conversion may run before it is stopped.
const TIMEOUT: Duration = Duration::from_secs(3 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    /// Keep it exactly as it is.
    Copy,
    /// Make it again with this ffmpeg encoder.
    Encode(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// `mp4` or `webm`.
    pub ext: &'static str,
    pub video: Part,
    pub audio: Part,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

/// The name (without extension) of the converted copy of a video: the same file always gets the same name, and
/// a file that was replaced (different size or date) gets a new one.
pub fn cache_stem(rel: &str, size: u64, modified: u64) -> String {
    let mut m = Md5::new();
    m.update(format!("{}|{}|{}", rel, size, modified).as_bytes());
    hex(&m.finish())
}

/// True for a name this program made: 32 hex letters and `.mp4` or `.webm` (and nothing that can leave the folder).
pub fn valid_cache_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else { return false };
    stem.len() == 32 && stem.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && (ext == "mp4" || ext == "webm")
}

fn cache_dirs(roots: &[PathBuf]) -> Vec<PathBuf> {
    roots.iter().map(|r| r.join(CACHE_DIR)).collect()
}

/// The converted copy of a video, if one has been made (on any drive): its file name.
pub fn converted_in(roots: &[PathBuf], rel: &str, size: u64, modified: u64) -> Option<String> {
    let stem = cache_stem(rel, size, modified);
    for d in cache_dirs(roots) {
        for ext in ["mp4", "webm"] {
            let name = format!("{}.{}", stem, ext);
            if d.join(&name).is_file() {
                return Some(name);
            }
        }
    }
    None
}

/// A converted copy by its name, for the player: only a name this program made, found in a `.converted` folder.
pub fn resolve_cache(roots: &[PathBuf], name: &str) -> Option<PathBuf> {
    if !valid_cache_name(name) {
        return None;
    }
    for d in cache_dirs(roots) {
        let p = d.join(name);
        if let (Ok(real), Ok(dir)) = (p.canonicalize(), d.canonicalize()) {
            if real.starts_with(&dir) && real.is_file() {
                return Some(real);
            }
        }
    }
    None
}

fn ext_lower(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// The `videos` folder holding `rel` and the file itself, for a file that needs converting. Only the video kinds
/// that need converting are ever handed to ffmpeg, and only from inside a `videos` folder.
pub fn find_original(roots: &[PathBuf], rel: &str) -> Option<(PathBuf, PathBuf)> {
    if !valid_rel(rel) || rel.split('/').count() > 3 || rel.split('/').any(|p| p.starts_with('.')) {
        return None;
    }
    if !NEEDS_CONVERTING.contains(&ext_lower(rel).as_str()) {
        return None;
    }
    for r in roots {
        let mut full = r.clone();
        for p in rel.split('/') {
            full.push(p);
        }
        if let (Ok(real), Ok(root)) = (full.canonicalize(), r.canonicalize()) {
            if real.starts_with(&root) && real.is_file() {
                return Some((r.clone(), real));
            }
        }
    }
    None
}

// ---------- what ffmpeg says ----------

/// The picture and sound codec names from what `ffmpeg -i file` prints about the first of each,
/// e.g. `Stream #0:0: Video: h264 (High), yuv420p` gives `h264`.
pub fn parse_streams(text: &str) -> (Option<String>, Option<String>) {
    let (mut v, mut a) = (None, None);
    for line in text.lines() {
        let l = line.trim();
        if !l.starts_with("Stream #") {
            continue;
        }
        for (key, slot) in [("Video: ", &mut v), ("Audio: ", &mut a)] {
            if slot.is_none() {
                if let Some(i) = l.find(key) {
                    let rest = &l[i + key.len()..];
                    let codec: String = rest.chars().take_while(|c| !c.is_whitespace() && *c != ',' && *c != '(').collect();
                    // a cover picture inside the file is not the video
                    if !codec.is_empty() && !(key == "Video: " && l.contains("attached pic")) {
                        *slot = Some(codec.to_lowercase());
                    }
                }
            }
        }
    }
    (v, a)
}

/// The encoder names in the output of `ffmpeg -encoders`.
pub fn parse_encoders(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let flags = it.next()?;
            let name = it.next()?;
            (flags.len() == 6 && flags.chars().all(|c| c.is_ascii_alphabetic() || c == '.') && (flags.starts_with('V') || flags.starts_with('A'))).then(|| name.to_string())
        })
        .collect()
}

/// H.264 encoders in the order they are preferred: the good software one first, then Windows' own, then graphics cards.
const H264_ENCODERS: &[&str] = &["libx264", "h264_mf", "h264_nvenc", "h264_qsv", "h264_amf"];

/// Decide what to do with the picture and the sound, given the codecs found and the encoders this ffmpeg has.
pub fn plan(vcodec: Option<&str>, acodec: Option<&str>, encoders: &[String]) -> Result<Plan, String> {
    let has = |n: &str| encoders.iter().any(|e| e == n);
    let vc = vcodec.ok_or("There is no picture in this file.")?;
    let (ext, video) = match vc {
        "h264" => ("mp4", Part::Copy),
        "vp8" | "vp9" | "av1" => ("webm", Part::Copy),
        _ => {
            if let Some(e) = H264_ENCODERS.iter().find(|e| has(e)) {
                ("mp4", Part::Encode(e.to_string()))
            } else if has("libvpx-vp9") {
                ("webm", Part::Encode("libvpx-vp9".into()))
            } else {
                return Err("This video's picture has to be re-made, and the ffmpeg in the engines folder has no video maker for that. Use a full ffmpeg build.".into());
            }
        }
    };
    let audio = match (ext, acodec) {
        (_, None) => Part::Copy, // no sound: nothing to do (the arguments leave sound out)
        ("mp4", Some("aac")) | ("mp4", Some("mp3")) => Part::Copy,
        ("webm", Some("opus")) | ("webm", Some("vorbis")) => Part::Copy,
        ("mp4", _) => Part::Encode("aac".into()),
        _ => {
            if has("libopus") {
                Part::Encode("libopus".into())
            } else if has("libvorbis") {
                Part::Encode("libvorbis".into())
            } else {
                return Err("This video's sound has to be re-made, and the ffmpeg in the engines folder has no sound maker for the web format. Use a full ffmpeg build.".into());
            }
        }
    };
    Ok(Plan { ext, video, audio })
}

/// The ffmpeg arguments for one conversion. The output is written whole to `output` (named `.tmp`), so the
/// format is given on the command line. Only the first picture and first sound are kept; subtitles are left out.
pub fn args(input: &Path, output: &Path, p: &Plan, has_audio: bool) -> Vec<OsString> {
    let mut a: Vec<OsString> = Vec::new();
    let mut push = |s: &str| a.push(OsString::from(s));
    for s in ["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-protocol_whitelist", "file", "-i"] {
        push(s);
    }
    a.push(input.as_os_str().to_os_string());
    let mut push = |s: &str| a.push(OsString::from(s));
    for s in ["-map", "0:v:0"] {
        push(s);
    }
    if has_audio {
        for s in ["-map", "0:a:0"] {
            push(s);
        }
    }
    for s in ["-sn", "-dn"] {
        push(s);
    }
    match &p.video {
        Part::Copy => {
            push("-c:v");
            push("copy");
        }
        Part::Encode(e) => {
            push("-c:v");
            push(e);
            push("-pix_fmt");
            push("yuv420p");
            match e.as_str() {
                "libx264" => {
                    for s in ["-preset", "veryfast", "-crf", "23"] {
                        push(s);
                    }
                }
                "libvpx-vp9" => {
                    for s in ["-b:v", "0", "-crf", "33", "-deadline", "good", "-cpu-used", "4", "-row-mt", "1"] {
                        push(s);
                    }
                }
                _ => {
                    for s in ["-b:v", "3M"] {
                        push(s);
                    }
                }
            }
        }
    }
    if has_audio {
        match &p.audio {
            Part::Copy => {
                push("-c:a");
                push("copy");
            }
            Part::Encode(e) => {
                push("-c:a");
                push(e);
                push("-b:a");
                push("160k");
            }
        }
    }
    if p.ext == "mp4" {
        push("-movflags");
        push("+faststart");
    }
    push("-f");
    push(p.ext);
    a.push(output.as_os_str().to_os_string());
    a
}

// ---------- running ffmpeg ----------

fn command(ffmpeg: &Path) -> Command {
    let mut c = Command::new(ffmpeg);
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    c
}

/// Run to the end, a timeout or a stop. Returns the exit success and up to 16 KB of what was printed.
fn run(mut cmd: Command, timeout: Duration, cancel: &AtomicBool) -> Result<(bool, String), String> {
    let mut child = cmd.spawn().map_err(|e| format!("the converter could not start ({})", e))?;
    let text = Arc::new(Mutex::new(String::new()));
    let reader = child.stderr.take().map(|mut err| {
        let text = text.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = err.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut t = text.lock().unwrap();
                if t.len() < 16_384 {
                    t.push_str(&String::from_utf8_lossy(&buf[..n]));
                }
            }
        })
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s.success()),
            Ok(None) => {}
            Err(e) => break Err(e.to_string()),
        }
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Stopped.".into());
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("The converter took too long and was stopped.".into());
        }
        std::thread::sleep(Duration::from_millis(150));
    }?;
    if let Some(r) = reader {
        let _ = r.join();
    }
    let out = text.lock().unwrap().clone();
    Ok((status, out))
}

fn last_lines(text: &str) -> String {
    let t: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    t[t.len().saturating_sub(2)..].join(" ").chars().take(200).collect()
}

/// Make a playable copy of the video `rel` (found in one of `roots`) and return the copy's file name.
/// A copy that is already there is returned at once. `cancel` stops it part way.
pub fn convert(ffmpeg: &Path, roots: &[PathBuf], rel: &str, cancel: &AtomicBool) -> Result<String, String> {
    let (root, input) = find_original(roots, rel).ok_or("That video is no longer in the videos folder.")?;
    let meta = fs::metadata(&input).map_err(|e| e.to_string())?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    if let Some(name) = converted_in(&[root.clone()], rel, meta.len(), mtime) {
        return Ok(name);
    }
    let never = AtomicBool::new(false);
    let (_, probe) = run({
        let mut c = command(ffmpeg);
        c.args(["-hide_banner", "-protocol_whitelist", "file", "-i"]).arg(&input);
        c
    }, Duration::from_secs(120), cancel)?;
    let (vc, ac) = parse_streams(&probe);
    let (_, enc_text) = run({
        let mut c = command(ffmpeg);
        c.args(["-hide_banner", "-encoders"]);
        c
    }, Duration::from_secs(60), &never)?;
    let p = plan(vc.as_deref(), ac.as_deref(), &parse_encoders(&enc_text))?;
    let dir = root.join(CACHE_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("could not make the folder for converted videos ({})", e))?;
    let name = format!("{}.{}", cache_stem(rel, meta.len(), mtime), p.ext);
    let tmp = dir.join(format!("{}.tmp", name));
    let _ = fs::remove_file(&tmp);
    let mut cmd = command(ffmpeg);
    cmd.args(args(&input, &tmp, &p, ac.is_some()));
    match run(cmd, TIMEOUT, cancel) {
        Ok((true, _)) => {}
        Ok((false, out)) => {
            let _ = fs::remove_file(&tmp);
            return Err(format!("the converter could not read this video ({})", last_lines(&out)));
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
    }
    if fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = fs::remove_file(&tmp);
        return Err("the converter made an empty file".into());
    }
    fs::rename(&tmp, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(name)
}

fn ours(name: &str) -> bool {
    valid_cache_name(name) || name.strip_suffix(".tmp").map(valid_cache_name).unwrap_or(false)
}

/// Total size in bytes of the converted copies on these drives.
pub fn cache_size(roots: &[PathBuf]) -> u64 {
    cache_dirs(roots)
        .iter()
        .filter_map(|d| fs::read_dir(d).ok())
        .flat_map(|rd| rd.flatten())
        .filter(|e| ours(&e.file_name().to_string_lossy()))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Delete the converted copies, and only those. Returns how many bytes were freed.
pub fn clear_cache(roots: &[PathBuf]) -> u64 {
    let mut freed = 0;
    for d in cache_dirs(roots) {
        if let Ok(rd) = fs::read_dir(&d) {
            for e in rd.flatten() {
                if ours(&e.file_name().to_string_lossy()) && e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    let n = e.metadata().map(|m| m.len()).unwrap_or(0);
                    if fs::remove_file(e.path()).is_ok() {
                        freed += n;
                    }
                }
            }
        }
        let _ = fs::remove_dir(&d); // only goes if now empty
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-vconv-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn encs(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn codecs_are_read_from_what_ffmpeg_prints() {
        let t = "Input #0, matroska,webm, from 'a.mkv':\n  Stream #0:0(eng): Video: h264 (High), yuv420p(tv), 1920x1080, 24 fps\n  Stream #0:1(eng): Audio: ac3, 48000 Hz, 5.1(side), fltp\n  Stream #0:2: Subtitle: subrip\n";
        assert_eq!(parse_streams(t), (Some("h264".into()), Some("ac3".into())));
        assert_eq!(parse_streams("Stream #0:0: Video: mpeg4 (Simple Profile), yuv420p\nStream #0:1: Audio: mp3, 44100 Hz"), (Some("mpeg4".into()), Some("mp3".into())));
        // a cover picture is not the video
        let c = "Stream #0:0: Video: mjpeg, yuvj420p (attached pic)\nStream #0:1: Video: vp9, yuv420p\nStream #0:2: Audio: opus, 48000 Hz";
        assert_eq!(parse_streams(c), (Some("vp9".into()), Some("opus".into())));
        assert_eq!(parse_streams("nothing useful here"), (None, None));
    }

    #[test]
    fn encoders_are_read_from_the_encoder_list() {
        let t = "Encoders:\n V..... = Video\n ------\n V....D libx264              libx264 H.264\n V....D mpeg4               MPEG-4 part 2\n A....D aac                 AAC (Advanced Audio Coding)\n A....D libopus             libopus Opus\n S..... srt                 SubRip subtitle\n";
        let e = parse_encoders(t);
        assert!(e.contains(&"libx264".to_string()) && e.contains(&"aac".to_string()) && e.contains(&"libopus".to_string()));
        assert!(!e.contains(&"srt".to_string()));
    }

    #[test]
    fn a_picture_the_player_can_use_is_copied_and_only_unusable_sound_is_remade() {
        let all = encs(&["libx264", "aac", "libopus"]);
        // H.264 with AAC: nothing to re-make at all
        let p = plan(Some("h264"), Some("aac"), &all).unwrap();
        assert_eq!((p.ext, p.video, p.audio), ("mp4", Part::Copy, Part::Copy));
        // H.264 with AC3 sound (common in MKV): picture copied, sound made into AAC
        let p = plan(Some("h264"), Some("ac3"), &all).unwrap();
        assert_eq!((p.ext, p.video, p.audio), ("mp4", Part::Copy, Part::Encode("aac".into())));
        // VP9 with Opus (what downloaders often save): copied into WebM
        let p = plan(Some("vp9"), Some("opus"), &all).unwrap();
        assert_eq!((p.ext, p.video, p.audio), ("webm", Part::Copy, Part::Copy));
        // a video with no sound
        assert_eq!(plan(Some("h264"), None, &all).unwrap().audio, Part::Copy);
    }

    #[test]
    fn an_old_picture_is_remade_with_the_best_encoder_there_is() {
        let p = plan(Some("wmv3"), Some("wmav2"), &encs(&["libx264", "aac", "h264_mf"])).unwrap();
        assert_eq!((p.ext, p.video, p.audio), ("mp4", Part::Encode("libx264".into()), Part::Encode("aac".into())));
        // a lighter ffmpeg without libx264 uses Windows' own
        let p = plan(Some("mpeg2video"), Some("mp2"), &encs(&["aac", "h264_mf", "libvpx-vp9", "libopus"])).unwrap();
        assert_eq!(p.video, Part::Encode("h264_mf".into()));
        // with only a web encoder the copy is WebM, and its sound goes to Opus
        let p = plan(Some("msmpeg4v3"), Some("mp3"), &encs(&["libvpx-vp9", "libopus", "aac"])).unwrap();
        assert_eq!((p.ext, p.video, p.audio), ("webm", Part::Encode("libvpx-vp9".into()), Part::Encode("libopus".into())));
        // nothing to make video with: a clear message, not a crash
        assert!(plan(Some("wmv3"), Some("wmav2"), &encs(&["aac"])).unwrap_err().contains("video maker"));
        assert!(plan(None, Some("aac"), &[]).is_err());
        // web video with sound that needs re-making, and no web sound encoder
        assert!(plan(Some("vp9"), Some("ac3"), &encs(&["aac"])).unwrap_err().contains("sound maker"));
    }

    #[test]
    fn arguments_read_one_local_file_and_write_one_video() {
        let p = Plan { ext: "mp4", video: Part::Encode("libx264".into()), audio: Part::Encode("aac".into()) };
        let a: Vec<String> = args(Path::new("/v/a.avi"), Path::new("/v/.converted/x.mp4.tmp"), &p, true).iter().map(|s| s.to_string_lossy().to_string()).collect();
        let j = a.join(" ");
        assert!(j.contains("-protocol_whitelist file"));
        let i = a.iter().position(|s| s == "-i").unwrap();
        assert_eq!(a[i + 1], "/v/a.avi", "the input is always right after -i");
        assert!(j.contains("-map 0:v:0") && j.contains("-map 0:a:0") && j.contains("-sn"));
        assert!(j.contains("-c:v libx264") && j.contains("-pix_fmt yuv420p") && j.contains("-c:a aac"));
        assert!(j.contains("-movflags +faststart") && j.contains("-f mp4"));
        assert_eq!(a.last().unwrap(), "/v/.converted/x.mp4.tmp");
        // copying: no encoder settings, and no sound arguments when there is no sound
        let c = Plan { ext: "webm", video: Part::Copy, audio: Part::Copy };
        let j = args(Path::new("/v/a.mkv"), Path::new("/o.tmp"), &c, false).iter().map(|s| s.to_string_lossy().to_string()).collect::<Vec<_>>().join(" ");
        assert!(j.contains("-c:v copy") && !j.contains("-c:a") && !j.contains("0:a:0") && j.contains("-f webm") && !j.contains("faststart"));
    }

    #[test]
    fn cache_names_are_stable_and_only_ours_are_accepted() {
        assert_eq!(cache_stem("Bush/a.mkv", 10, 20), cache_stem("Bush/a.mkv", 10, 20));
        assert_ne!(cache_stem("Bush/a.mkv", 10, 20), cache_stem("Bush/a.mkv", 11, 20), "a changed file is converted again");
        assert_ne!(cache_stem("Bush/a.mkv", 10, 20), cache_stem("Bush/b.mkv", 10, 20));
        let ok = format!("{}.mp4", cache_stem("a.mkv", 1, 2));
        assert!(valid_cache_name(&ok));
        assert!(valid_cache_name(&format!("{}.webm", cache_stem("a.mkv", 1, 2))));
        for bad in ["", "x.mp4", "../a.mp4", "A".repeat(32).as_str(), &format!("{}.mkv", cache_stem("a", 1, 2)), &format!("{}.mp4/../x", cache_stem("a", 1, 2)), &format!("{}.tmp", cache_stem("a", 1, 2))] {
            assert!(!valid_cache_name(bad), "{}", bad);
        }
    }

    #[test]
    fn a_made_copy_is_found_served_counted_and_cleared() {
        let d = tmp("cache");
        let rel = "Bush/old.avi";
        fs::create_dir_all(d.join("Bush")).unwrap();
        fs::write(d.join(rel), b"original").unwrap();
        let roots = [d.clone()];
        assert_eq!(converted_in(&roots, rel, 8, 5), None);
        let name = format!("{}.mp4", cache_stem(rel, 8, 5));
        fs::create_dir_all(d.join(CACHE_DIR)).unwrap();
        fs::write(d.join(CACHE_DIR).join(&name), b"converted!").unwrap();
        fs::write(d.join(CACHE_DIR).join("notes.txt"), b"mine").unwrap();
        assert_eq!(converted_in(&roots, rel, 8, 5), Some(name.clone()));
        assert!(resolve_cache(&roots, &name).is_some());
        assert!(resolve_cache(&roots, "notes.txt").is_none());
        assert!(resolve_cache(&roots, &format!("../{}", name)).is_none());
        assert_eq!(cache_size(&roots), 10);
        // the scan never lists the hidden folder
        assert!(crate::videos::scan(&d).iter().all(|t| t.videos.iter().all(|v| !v.rel.contains(CACHE_DIR))));
        assert_eq!(clear_cache(&roots), 10);
        assert!(d.join(CACHE_DIR).join("notes.txt").exists(), "files that are not ours are left alone");
        assert_eq!(converted_in(&roots, rel, 8, 5), None);
        assert_eq!(d.join(rel).metadata().unwrap().len(), 8, "the original is untouched");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn only_videos_that_need_converting_are_handed_to_the_converter() {
        let d = tmp("orig");
        for f in ["Bush/a.mkv", "Bush/b.mp4", "Bush/c.txt", "Bush/.h.mkv", "Bush/Fire/d.avi", "Bush/Fire/Too/e.avi"] {
            fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
            fs::write(d.join(f), b"x").unwrap();
        }
        let roots = [d.clone()];
        assert!(find_original(&roots, "Bush/a.mkv").is_some());
        assert!(find_original(&roots, "Bush/Fire/d.avi").is_some());
        assert!(find_original(&roots, "Bush/b.mp4").is_none(), "already playable");
        assert!(find_original(&roots, "Bush/c.txt").is_none());
        assert!(find_original(&roots, "Bush/.h.mkv").is_none());
        assert!(find_original(&roots, "Bush/Fire/Too/e.avi").is_none());
        assert!(find_original(&roots, "../a.mkv").is_none());
        assert!(find_original(&roots, "Bush/missing.mkv").is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_converter_or_video_is_a_plain_error() {
        let d = tmp("noff");
        fs::create_dir_all(d.join("Bush")).unwrap();
        fs::write(d.join("Bush/a.mkv"), b"not really a video").unwrap();
        let stop = AtomicBool::new(false);
        let e = convert(&d.join("no-such-ffmpeg"), &[d.clone()], "Bush/a.mkv", &stop).unwrap_err();
        assert!(e.contains("could not start"), "{}", e);
        let e = convert(&d.join("no-such-ffmpeg"), &[d.clone()], "Bush/none.mkv", &stop).unwrap_err();
        assert!(e.contains("no longer"), "{}", e);
        assert!(!d.join("Bush/a.mkv.tmp").exists());
        let _ = fs::remove_dir_all(&d);
    }
}
