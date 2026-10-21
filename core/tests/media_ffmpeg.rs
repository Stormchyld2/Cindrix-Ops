//! Real-file check: make audio in many formats with a real ffmpeg, then read their tags and
//! covers and convert the ones the web view cannot play. Skipped when ffmpeg is missing,
//! unless CINDRIX_REQUIRE_FFMPEG=1 (set in CI), when a missing ffmpeg fails the test.

use cindrix_core::media::{self, convert, tags, Class};
use std::path::{Path, PathBuf};
use std::process::Command;

fn ffmpeg() -> Option<PathBuf> {
    let p = PathBuf::from("ffmpeg");
    if Command::new(&p).arg("-version").output().map(|o| o.status.success()).unwrap_or(false) {
        Some(p)
    } else if std::env::var("CINDRIX_REQUIRE_FFMPEG").as_deref() == Ok("1") {
        panic!("ffmpeg is required but not found");
    } else {
        None
    }
}

fn make(ff: &Path, dir: &Path, name: &str, extra: &[&str]) -> PathBuf {
    let out = dir.join(name);
    let mut c = Command::new(ff);
    c.args(["-nostdin", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=1"]);
    c.args(extra);
    c.arg(&out);
    let o = c.output().unwrap();
    assert!(o.status.success(), "{}: {}", name, String::from_utf8_lossy(&o.stderr));
    out
}

#[test]
fn tags_and_conversion_with_real_files() {
    let Some(ff) = ffmpeg() else { return };
    let d = std::env::temp_dir().join(format!("cindrix-ff-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let music = d.join("music/Test Artist/Test Album");
    std::fs::create_dir_all(&music).unwrap();
    let meta = ["-metadata", "title=Real Title", "-metadata", "artist=Real Artist", "-metadata", "album=Real Album", "-metadata", "track=7"];
    let mut files = vec![];
    for (name, codec) in [("07 - a.mp3", &["-c:a", "libmp3lame"][..]), ("07 - b.flac", &["-c:a", "flac"]), ("07 - c.ogg", &["-c:a", "libvorbis"]), ("07 - d.opus", &["-c:a", "libopus"]), ("07 - e.wav", &["-c:a", "pcm_s16le"]), ("07 - f.m4a", &["-c:a", "aac"]), ("07 - g.wma", &["-c:a", "wmav2"])] {
        let mut extra: Vec<&str> = meta.to_vec();
        extra.extend_from_slice(codec);
        files.push(make(&ff, &music, name, &extra));
    }
    for f in &files {
        let t = tags::read_tags(f);
        let ext = f.extension().unwrap().to_string_lossy().to_string();
        if ext != "wav" {
            assert_eq!(t.title, "Real Title", "{:?}", f);
            assert_eq!(t.artist, "Real Artist", "{:?}", f);
            assert_eq!(t.album, "Real Album", "{:?}", f);
        }
    }

    let r = media::scan(&d.join("music"), &media::ScanOptions { cache: None, cancel: None }, &mut |_| {});
    assert_eq!(r.tracks.len(), 7);
    let wma = r.tracks.iter().find(|t| t.rel.ends_with(".wma")).unwrap();
    assert_eq!(wma.class, Class::Convert);

    let target = convert::pick_target(&ff).expect("ffmpeg runs");
    let cache = d.join("cache");
    let name = convert::convert(&ff, &music.join("07 - g.wma"), &wma.rel, &cache, target).unwrap();
    assert!(cache.join(&name).metadata().unwrap().len() > 100);
    // second call reuses the copy
    assert_eq!(convert::convert(&ff, &music.join("07 - g.wma"), &wma.rel, &cache, target).unwrap(), name);
    // and the converted copy is itself readable audio
    let back = tags::read_tags(&cache.join(&name));
    assert_eq!(back.title, "Real Title");
    assert!(convert::cache_size(&cache) > 0);
    convert::clear_cache(&cache);
    assert_eq!(convert::cache_size(&cache), 0);

    // not audio at all: a plain error, nothing left behind
    let junk = music.join("junk.wma");
    std::fs::write(&junk, b"this is not audio").unwrap();
    assert!(convert::convert(&ff, &junk, "junk.wma", &cache, target).is_err());
    assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 0);
}

#[test]
fn covers_embedded_in_real_files_are_found() {
    let Some(ff) = ffmpeg() else { return };
    let d = std::env::temp_dir().join(format!("cindrix-ffa-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let img = d.join("c.jpg");
    let o = Command::new(&ff).args(["-nostdin", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "color=c=red:s=64x64:d=1", "-frames:v", "1"]).arg(&img).output().unwrap();
    assert!(o.status.success());
    for (name, codec) in [("a.mp3", "libmp3lame"), ("b.m4a", "aac"), ("c.flac", "flac")] {
        let out = d.join(name);
        let o = Command::new(&ff)
            .args(["-nostdin", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=1", "-i"])
            .arg(&img)
            .args(["-map", "0:a", "-map", "1:v", "-c:a", codec, "-c:v", "copy", "-disposition:v", "attached_pic"])
            .arg(&out)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}: {}", name, String::from_utf8_lossy(&o.stderr));
        let art = tags::read_art(&out).unwrap_or_else(|| panic!("no cover in {}", name));
        assert_eq!(art.mime, "image/jpeg", "{}", name);
        assert!(art.data.starts_with(&[0xFF, 0xD8]), "{}", name);
    }
}

/// WAV files the web view's own decoder may refuse (24-bit, floating point, 8-bit, mu-law, with
/// a big metadata chunk) must still come out playable through the converter, because the player
/// falls back to it when the built-in decoder fails. Also: our tag reader must not choke on them.
#[test]
fn awkward_wav_files_convert_and_read() {
    let Some(ff) = ffmpeg() else { return };
    let d = std::env::temp_dir().join(format!("cindrix-wav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let music = d.join("music");
    let cache = d.join("cache");
    std::fs::create_dir_all(&music).unwrap();
    let target = convert::pick_target(&ff).expect("ffmpeg runs");
    let cases: [(&str, &[&str]); 6] = [
        ("pcm24.wav", &["-c:a", "pcm_s24le"]),
        ("pcm32.wav", &["-c:a", "pcm_s32le"]),
        ("float32.wav", &["-c:a", "pcm_f32le"]),
        ("pcm8.wav", &["-c:a", "pcm_u8"]),
        ("mulaw.wav", &["-c:a", "pcm_mulaw", "-ar", "8000"]),
        ("stereo48k.wav", &["-c:a", "pcm_s16le", "-ac", "2", "-ar", "48000", "-metadata", "title=Stereo", "-metadata", "artist=Someone"]),
    ];
    for (name, codec) in cases {
        let f = make(&ff, &music, name, codec);
        let t = tags::read_tags(&f); // must not panic or hang
        let _ = t;
        assert_eq!(media::class_of(&f), Some(Class::Native));
        let out = convert::convert(&ff, &f, name, &cache, target).unwrap_or_else(|e| panic!("{}: {}", name, e));
        let bytes = std::fs::read(cache.join(&out)).unwrap();
        assert!(bytes.len() > 1000, "{}: converted file is {} bytes", name, bytes.len());
        match target {
            convert::Target::Flac => assert_eq!(&bytes[..4], b"fLaC", "{}", name),
            convert::Target::Opus => assert_eq!(&bytes[..4], b"OggS", "{}", name),
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}
