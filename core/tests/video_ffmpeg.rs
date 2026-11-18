//! Converting real videos with a real ffmpeg (skipped when there is none on this computer).
use cindrix_core::videoconv::{self, parse_streams};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

fn ffmpeg() -> Option<PathBuf> {
    let p = PathBuf::from("ffmpeg");
    Command::new(&p).arg("-version").output().ok().filter(|o| o.status.success()).map(|_| p)
}

fn make(ff: &Path, out: &Path, extra: &[&str]) -> bool {
    let mut c = Command::new(ff);
    c.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=15:duration=2", "-f", "lavfi", "-i", "sine=frequency=440:duration=2"]);
    c.args(extra).arg(out);
    c.status().map(|s| s.success()).unwrap_or(false)
}

fn streams_of(ff: &Path, f: &Path) -> (Option<String>, Option<String>) {
    let o = Command::new(ff).args(["-hide_banner", "-i"]).arg(f).output().unwrap();
    parse_streams(&String::from_utf8_lossy(&o.stderr))
}

#[test]
fn real_videos_become_playable_copies_and_the_original_is_untouched() {
    let Some(ff) = ffmpeg() else {
        eprintln!("no ffmpeg here: skipped");
        return;
    };
    let d = std::env::temp_dir().join(format!("cindrix-vff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("Bush")).unwrap();
    let roots = [d.clone()];
    let stop = AtomicBool::new(false);

    // 1. MKV with H.264 picture and AC3 sound: the picture is copied, the sound made into AAC (when this ffmpeg can)
    let mkv = d.join("Bush/fire.mkv");
    if make(&ff, &mkv, &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "ac3", "-shortest"]) {
        let before = std::fs::read(&mkv).unwrap();
        let name = videoconv::convert(&ff, &roots, "Bush/fire.mkv", &stop).expect("mkv converts");
        assert!(name.ends_with(".mp4"), "{}", name);
        let out = videoconv::resolve_cache(&roots, &name).expect("the copy is found");
        let (v, a) = streams_of(&ff, &out);
        assert_eq!(v.as_deref(), Some("h264"));
        assert!(matches!(a.as_deref(), Some("aac") | Some("mp3")), "{:?}", a);
        assert_eq!(std::fs::read(&mkv).unwrap(), before, "original untouched");
        // asking again finds the copy and makes nothing new
        assert_eq!(videoconv::convert(&ff, &roots, "Bush/fire.mkv", &stop).unwrap(), name);
        assert!(videoconv::cache_size(&roots) > 0);
    } else {
        eprintln!("this ffmpeg has no libx264: the H.264 case is skipped");
    }

    // 2. AVI with an old MPEG-4 picture and MP3 sound: the picture must be re-made
    let avi = d.join("Bush/old.avi");
    if make(&ff, &avi, &["-c:v", "mpeg4", "-c:a", "libmp3lame", "-shortest"]) {
        match videoconv::convert(&ff, &roots, "Bush/old.avi", &stop) {
            Ok(name) => {
                let out = videoconv::resolve_cache(&roots, &name).unwrap();
                let (v, _) = streams_of(&ff, &out);
                assert!(matches!(v.as_deref(), Some("h264") | Some("vp9")), "{:?}", v);
            }
            // a minimal ffmpeg without any video maker must say so in plain words
            Err(e) => assert!(e.contains("video maker"), "{}", e),
        }
    }

    // 3. a file that is not a video gives an error and leaves nothing behind
    std::fs::write(d.join("Bush/broken.mkv"), b"this is not a video").unwrap();
    let e = videoconv::convert(&ff, &roots, "Bush/broken.mkv", &stop).unwrap_err();
    assert!(!e.is_empty());
    let left: Vec<_> = std::fs::read_dir(d.join(videoconv::CACHE_DIR)).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
    assert!(left.iter().all(|n| !n.ends_with(".tmp")), "{:?}", left);

    // 4. stopping part way
    let flag = AtomicBool::new(true);
    if make(&ff, &d.join("Bush/stop.mkv"), &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "ac3", "-shortest"]) {
        assert!(videoconv::convert(&ff, &roots, "Bush/stop.mkv", &flag).is_err());
    }
    let _ = std::fs::remove_dir_all(&d);
}
