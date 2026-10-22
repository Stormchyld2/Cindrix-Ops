//! Deciding what the web view may be handed. The `cindrix-music` address has three kinds of
//! request, and nothing else is answered:
//!   `t/<path>`  an audio file under the music folder
//!   `a/<path>`  the cover picture for that audio file (embedded, or a picture in its folder)
//!   `c/<name>`  a converted copy this program made, in `user/cache/music/`
//! Parent folders, absolute paths, other file types and links that lead outside are refused.

use super::{class_of, valid_part, valid_rel};
use crate::maps::percent_decode;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Route {
    Track(PathBuf),
    /// The audio file whose cover is wanted.
    Art(PathBuf),
    Converted(PathBuf),
}

/// An audio file `rel` under `root`, only if it really is a file inside `root`.
pub fn resolve_track(root: &Path, rel: &str) -> Option<PathBuf> {
    if !valid_rel(rel) {
        return None;
    }
    let mut full = root.to_path_buf();
    for p in rel.split('/') {
        full.push(p);
    }
    class_of(&full)?;
    inside(root, &full)
}

fn inside(root: &Path, full: &Path) -> Option<PathBuf> {
    let real = full.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    if real.starts_with(&root) && real.is_file() { Some(real) } else { None }
}

/// A name this program gives its converted copies: 32 hex digits, then `.opus` or `.flac`.
pub fn valid_cache_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else { return false };
    stem.len() == 32 && stem.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && (ext == "opus" || ext == "flac")
}

pub fn route(music_root: &Path, cache_dir: &Path, requested: &str) -> Option<Route> {
    let path = requested.split(['?', '#']).next().unwrap_or("");
    let decoded = percent_decode(path)?;
    let (kind, rest) = decoded.trim_start_matches('/').split_once('/')?;
    match kind {
        "t" => resolve_track(music_root, rest).map(Route::Track),
        "a" => resolve_track(music_root, rest).map(Route::Art),
        "c" => {
            if !valid_cache_name(rest) {
                return None;
            }
            inside(cache_dir, &cache_dir.join(rest)).map(Route::Converted)
        }
        _ => None,
    }
}

/// A picture sitting in the same folder as a track: cover, folder, front or album art.
pub fn folder_art(track: &Path) -> Option<PathBuf> {
    let dir = track.parent()?;
    let track_stem = track.file_stem().map(|s| s.to_string_lossy().to_lowercase());
    let mut best: Option<(usize, PathBuf)> = None;
    let mut images: Vec<PathBuf> = Vec::new();
    for e in fs::read_dir(dir).ok()?.flatten() {
        if !e.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_lowercase();
        let Some((stem, ext)) = name.rsplit_once('.') else { continue };
        if !matches!(ext, "jpg" | "jpeg" | "png") || !valid_part(&name) {
            continue;
        }
        images.push(e.path());
        // a picture named like the song (Song.wav and Song.jpg) is the best match, then the usual album names
        let rank = if Some(stem.to_string()) == track_stem {
            Some(0)
        } else {
            ["cover", "folder", "front", "albumart", "albumartsmall", "albumartlarge", "album", "art", "thumb"].iter().position(|s| *s == stem).map(|r| r + 1)
        };
        if let Some(r) = rank {
            if best.as_ref().map(|(b, _)| r < *b).unwrap_or(true) {
                best = Some((r, e.path()));
            }
        }
    }
    // A folder with one picture and nothing else named: that picture is the cover.
    best.map(|(_, p)| p).or_else(|| if images.len() == 1 { images.pop() } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("cindrix-serve-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let music = base.join("music");
        let cache = base.join("cache");
        fs::create_dir_all(music.join("Artist/Album")).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::write(music.join("Artist/Album/01 - Song (live).mp3"), b"mp3").unwrap();
        fs::write(music.join("Artist/Album/notes.txt"), b"text").unwrap();
        fs::write(base.join("secret.mp3"), b"secret").unwrap();
        fs::write(cache.join("0123456789abcdef0123456789abcdef.opus"), b"opus").unwrap();
        fs::write(cache.join("not-a-cache-name.opus"), b"x").unwrap();
        (music, cache)
    }

    #[test]
    fn tracks_art_and_converted_copies_are_found() {
        let (m, c) = setup("ok");
        let want = m.join("Artist/Album/01 - Song (live).mp3").canonicalize().unwrap();
        assert_eq!(route(&m, &c, "/t/Artist/Album/01%20-%20Song%20(live).mp3"), Some(Route::Track(want.clone())));
        // the interface may send the separators encoded too
        assert_eq!(route(&m, &c, "/t%2FArtist%2FAlbum%2F01%20-%20Song%20(live).mp3"), Some(Route::Track(want.clone())));
        assert_eq!(route(&m, &c, "/a/Artist/Album/01%20-%20Song%20(live).mp3?x=1"), Some(Route::Art(want)));
        let conv = c.join("0123456789abcdef0123456789abcdef.opus").canonicalize().unwrap();
        assert_eq!(route(&m, &c, "/c/0123456789abcdef0123456789abcdef.opus"), Some(Route::Converted(conv)));
    }

    #[test]
    fn a_name_with_spaces_and_an_apostrophe_is_found_when_encoded_once() {
        // This is how the web view asks: the whole path encoded one time (convertFileSrc), slashes included.
        let (m, c) = setup("quote");
        fs::create_dir_all(m.join("The Man I'm Tryin To Be")).unwrap();
        fs::write(m.join("The Man I'm Tryin To Be/i'LL BE THERE.mp3"), b"mp3").unwrap();
        let want = m.join("The Man I'm Tryin To Be/i'LL BE THERE.mp3").canonicalize().unwrap();
        let once = "/t%2FThe%20Man%20I'm%20Tryin%20To%20Be%2Fi'LL%20BE%20THERE.mp3";
        assert_eq!(route(&m, &c, once), Some(Route::Track(want)));
        // encoded twice (what the program used to send) is not a real name, so it is refused
        let twice = "/t%2FThe%2520Man%2520I'm%2520Tryin%2520To%2520Be%2Fi'LL%2520BE%2520THERE.mp3";
        assert_eq!(route(&m, &c, twice), None);
    }

    #[test]
    fn folder_pictures_are_matched_by_song_name_usual_names_or_being_the_only_one() {
        let (m, _c) = setup("folderart");
        let dir = m.join("Artist/Album");
        let track = dir.join("01 - Song (live).mp3");
        assert_eq!(folder_art(&track), None); // no pictures
        fs::write(dir.join("whatever.jpg"), b"x").unwrap();
        assert_eq!(folder_art(&track), Some(dir.join("whatever.jpg"))); // the only picture
        fs::write(dir.join("other.png"), b"x").unwrap();
        assert_eq!(folder_art(&track), None); // two unnamed pictures: no guess
        fs::write(dir.join("AlbumArtSmall.jpg"), b"x").unwrap();
        assert_eq!(folder_art(&track), Some(dir.join("AlbumArtSmall.jpg")));
        fs::write(dir.join("01 - Song (live).png"), b"x").unwrap();
        assert_eq!(folder_art(&track), Some(dir.join("01 - Song (live).png"))); // named like the song wins
    }

    #[test]
    fn everything_else_is_refused() {
        let (m, c) = setup("bad");
        for bad in [
            "",
            "/",
            "/t",
            "/t/",
            "/x/Artist/Album/01%20-%20Song%20(live).mp3",
            "/t/Artist/Album/notes.txt",
            "/t/Artist/Album/missing.mp3",
            "/t/../secret.mp3",
            "/t/Artist/../../secret.mp3",
            "/t/Artist/%2e%2e/%2e%2e/secret.mp3",
            "/t//Artist/Album/01%20-%20Song%20(live).mp3",
            "/t/Artist\\Album\\01%20-%20Song%20(live).mp3",
            "/t/Artist/Album/01%20-%20Song%20(live).mp3%00.txt",
            "/t/%zz.mp3",
            "/c/not-a-cache-name.opus",
            "/c/../music/Artist/Album/01.mp3",
            "/c/0123456789abcdef0123456789abcdef.mp3",
            "/c/0123456789ABCDEF0123456789ABCDEF.opus",
            "/c/0123456789abcdef0123456789abcdef.opus.exe",
            "/c/ffffffffffffffffffffffffffffffff.opus", // right shape, not there
        ] {
            assert_eq!(route(&m, &c, bad), None, "should refuse {:?}", bad);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leaves_the_music_folder_is_refused() {
        let (m, c) = setup("link");
        std::os::unix::fs::symlink(m.parent().unwrap().join("secret.mp3"), m.join("Artist/leak.mp3")).unwrap();
        assert_eq!(route(&m, &c, "/t/Artist/leak.mp3"), None);
        std::os::unix::fs::symlink(m.parent().unwrap(), m.join("up")).unwrap();
        assert_eq!(route(&m, &c, "/t/up/secret.mp3"), None);
    }

    #[test]
    fn cover_pictures_in_the_folder_are_ranked() {
        let (m, _) = setup("art");
        let dir = m.join("Artist/Album");
        let track = dir.join("01 - Song (live).mp3");
        assert_eq!(folder_art(&track), None);
        fs::write(dir.join("Album.JPG"), b"x").unwrap();
        assert_eq!(folder_art(&track).unwrap().file_name().unwrap(), "Album.JPG");
        fs::write(dir.join("front.png"), b"x").unwrap();
        assert_eq!(folder_art(&track).unwrap().file_name().unwrap(), "front.png");
        fs::write(dir.join("cover.jpg"), b"x").unwrap();
        assert_eq!(folder_art(&track).unwrap().file_name().unwrap(), "cover.jpg");
        fs::write(dir.join("holiday.jpg"), b"x").unwrap(); // other pictures are not covers
        assert_eq!(folder_art(&track).unwrap().file_name().unwrap(), "cover.jpg");
    }
}
