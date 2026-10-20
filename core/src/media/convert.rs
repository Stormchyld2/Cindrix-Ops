//! Playing formats the web view cannot (WMA, AIFF, APE and others) by converting them once with
//! ffmpeg, a separate program in the drive's `engines/` folder that this program starts, just as
//! it starts llama-server and kiwix-serve. The converted copy is kept in `user/cache/music/`
//! and played from there; the original is never touched. Deleting the cache is always safe.
//!
//! ffmpeg is told to read local files only (`-protocol_whitelist file`), so a file that is not
//! really audio cannot make it fetch anything from the network.

use crate::md5;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Small (about 160 kbit/s) and plays everywhere the web view does.
    Opus,
    /// Always available in ffmpeg, but several times larger.
    Flac,
}

impl Target {
    pub fn ext(self) -> &'static str {
        match self {
            Target::Opus => "opus",
            Target::Flac => "flac",
        }
    }
    pub fn name(self) -> &'static str {
        self.ext()
    }
}

/// How long one conversion may run before it is stopped.
const TIMEOUT: Duration = Duration::from_secs(20 * 60);

fn command(ffmpeg: &Path) -> Command {
    let mut c = Command::new(ffmpeg);
    c.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // no console window
    }
    c
}

/// Ask ffmpeg which encoder to use: Opus when it has it, otherwise FLAC. None when ffmpeg will
/// not run at all.
pub fn pick_target(ffmpeg: &Path) -> Option<Target> {
    let out = command(ffmpeg).args(["-hide_banner", "-encoders"]).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(if text.lines().any(|l| l.split_whitespace().nth(1) == Some("libopus")) { Target::Opus } else { Target::Flac })
}

/// The ffmpeg arguments for one conversion. `output` is written as a whole-file `.opus` or `.flac`
/// (the format is named explicitly, so the temporary file name does not matter).
pub fn args(input: &Path, output: &Path, target: Target) -> Vec<std::ffi::OsString> {
    let mut a: Vec<std::ffi::OsString> = ["-nostdin", "-hide_banner", "-loglevel", "error", "-protocol_whitelist", "file", "-threads", "2", "-i"].iter().map(|s| (*s).into()).collect();
    a.push(input.into());
    // first audio stream only: no pictures, subtitles or data
    for s in ["-vn", "-sn", "-dn", "-map", "0:a:0", "-map_metadata", "0"] {
        a.push(s.into());
    }
    let codec: &[&str] = match target {
        Target::Opus => &["-c:a", "libopus", "-b:a", "160k", "-vbr", "on", "-f", "ogg"],
        Target::Flac => &["-c:a", "flac", "-compression_level", "5", "-f", "flac"],
    };
    for s in codec {
        a.push((*s).into());
    }
    a.push("-y".into());
    a.push(output.into());
    a
}

/// The name of the converted copy of a file: 32 hex digits of what identifies this version of it.
pub fn cache_name(rel: &str, size: u64, mtime: u64, target: Target) -> String {
    let key = format!("{}|{}|{}|{}", rel, size, mtime, target.ext());
    let mut m = md5::Md5::new();
    m.update(key.as_bytes());
    format!("{}.{}", md5::hex(&m.finish()), target.ext())
}

fn run(mut cmd: Command, limit: Duration) -> Result<(), String> {
    let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::piped()).spawn().map_err(|e| format!("could not start the converter: {}", e))?;
    let mut err = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.take(16 * 1024).read_to_string(&mut s);
        }
        s
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the converter took too long and was stopped".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let msg = reader.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        let tail: String = msg.trim().lines().last().unwrap_or("no message").chars().take(200).collect();
        Err(format!("the converter could not read this file ({})", tail))
    }
}

/// Convert `input` into `cache_dir`, unless a converted copy is already there. Returns the file
/// name (inside `cache_dir`) to play.
pub fn convert(ffmpeg: &Path, input: &Path, rel: &str, cache_dir: &Path, target: Target) -> Result<String, String> {
    let meta = fs::metadata(input).map_err(|e| e.to_string())?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
    let name = cache_name(rel, meta.len(), mtime, target);
    let done = cache_dir.join(&name);
    if done.is_file() {
        return Ok(name);
    }
    fs::create_dir_all(cache_dir).map_err(|e| e.to_string())?;
    let tmp = cache_dir.join(format!("{}.tmp", name));
    let _ = fs::remove_file(&tmp);
    let mut cmd = command(ffmpeg);
    cmd.args(args(input, &tmp, target));
    let r = run(cmd, TIMEOUT);
    if let Err(e) = r {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0) == 0 {
        let _ = fs::remove_file(&tmp);
        return Err("the converter made an empty file".into());
    }
    fs::rename(&tmp, &done).map_err(|e| e.to_string())?;
    Ok(name)
}

fn is_ours(name: &str) -> bool {
    super::serve::valid_cache_name(name) || name.strip_suffix(".tmp").map(super::serve::valid_cache_name).unwrap_or(false)
}

/// Total size in bytes of the converted copies.
pub fn cache_size(cache_dir: &Path) -> u64 {
    fs::read_dir(cache_dir)
        .map(|rd| rd.flatten().filter(|e| is_ours(&e.file_name().to_string_lossy())).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
        .unwrap_or(0)
}

/// Delete the converted copies, and only those: other files in the folder are left alone.
/// Returns how many bytes were freed.
pub fn clear_cache(cache_dir: &Path) -> u64 {
    let mut freed = 0;
    if let Ok(rd) = fs::read_dir(cache_dir) {
        for e in rd.flatten() {
            if is_ours(&e.file_name().to_string_lossy()) && e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                let n = e.metadata().map(|m| m.len()).unwrap_or(0);
                if fs::remove_file(e.path()).is_ok() {
                    freed += n;
                }
            }
        }
    }
    freed
}

/// Where the converter lives: `engines/ffmpeg(.exe)` on the drive.
pub fn engine_name() -> &'static str {
    if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }
}

#[allow(dead_code)]
fn _assert_pathbuf(_: PathBuf) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_read_one_local_file_and_write_one_audio_stream() {
        let a: Vec<String> = args(Path::new("/m/a.wma"), Path::new("/c/x.tmp"), Target::Opus).iter().map(|s| s.to_string_lossy().to_string()).collect();
        let joined = a.join(" ");
        assert!(joined.contains("-protocol_whitelist file"));
        assert!(joined.contains("-i /m/a.wma"));
        assert!(joined.contains("-map 0:a:0") && joined.contains("-vn"));
        assert!(joined.contains("-c:a libopus") && joined.contains("-f ogg"));
        assert_eq!(a.last().unwrap(), "/c/x.tmp");
        // the input is always the argument right after -i, never mistaken for an option
        let i = a.iter().position(|s| s == "-i").unwrap();
        assert_eq!(a[i + 1], "/m/a.wma");
        let f = args(Path::new("/m/a.wma"), Path::new("/c/x.tmp"), Target::Flac).iter().map(|s| s.to_string_lossy().to_string()).collect::<Vec<_>>().join(" ");
        assert!(f.contains("-c:a flac") && f.contains("-f flac") && !f.contains("libopus"));
    }

    #[test]
    fn cache_names_change_when_the_file_changes() {
        let a = cache_name("A/b.wma", 100, 5, Target::Opus);
        assert!(super::super::serve::valid_cache_name(&a), "{}", a);
        assert_eq!(a, cache_name("A/b.wma", 100, 5, Target::Opus));
        assert_ne!(a, cache_name("A/b.wma", 101, 5, Target::Opus));
        assert_ne!(a, cache_name("A/b.wma", 100, 6, Target::Opus));
        assert_ne!(a, cache_name("A/c.wma", 100, 5, Target::Opus));
        assert!(cache_name("A/b.wma", 100, 5, Target::Flac).ends_with(".flac"));
    }

    #[test]
    fn clearing_the_cache_touches_only_converted_copies() {
        let d = std::env::temp_dir().join(format!("cindrix-cc-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let mine = "0123456789abcdef0123456789abcdef.opus";
        fs::write(d.join(mine), vec![0u8; 1000]).unwrap();
        fs::write(d.join(format!("{}.tmp", mine)), vec![0u8; 24]).unwrap();
        fs::write(d.join("my-notes.txt"), b"keep").unwrap();
        fs::write(d.join("song.opus"), b"keep").unwrap();
        assert_eq!(cache_size(&d), 1024);
        assert_eq!(clear_cache(&d), 1024);
        assert!(d.join("my-notes.txt").exists() && d.join("song.opus").exists());
        assert_eq!(cache_size(&d), 0);
        assert_eq!(clear_cache(&d.join("missing")), 0);
    }

    #[test]
    fn a_missing_converter_is_a_plain_error() {
        let d = std::env::temp_dir().join(format!("cindrix-cm-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let input = d.join("a.wma");
        fs::write(&input, b"x").unwrap();
        assert_eq!(pick_target(&d.join("no-such-ffmpeg")), None);
        let e = convert(&d.join("no-such-ffmpeg"), &input, "a.wma", &d.join("cache"), Target::Opus).unwrap_err();
        assert!(e.contains("could not start the converter"), "{}", e);
        // nothing half-written is left behind
        assert_eq!(fs::read_dir(d.join("cache")).map(|r| r.count()).unwrap_or(0), 0);
    }
}
