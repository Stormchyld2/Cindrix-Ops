//! Videos: how-to videos saved as plain files in `library/videos/` on a drive, one folder per topic
//! ("Bushcraft", "Butchering and Processing"), the way the person sorts them on the stick:
//! `library/videos/<Topic>/<Title>.mp4`, or one level deeper: `<Topic>/<Group>/<Title>.mp4`.
//! A picture with the same name as the video (`Title.jpg`) is its thumbnail. Several drives may each hold a
//! `videos` folder; the same topic on two drives is one topic, and when two drives hold the same path the
//! first drive's copy wins. This module lists them, finds one safely and remembers where each was left off.
//! It never uses the internet and never changes a video file.

use crate::media::{self, valid_part, valid_rel};
use crate::podcasts::Progress;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Files dropped straight into `videos/` are listed under this topic.
pub const LOOSE_TOPIC: &str = "Other";

/// What the built-in player can play (the browser inside the program, on a phone or a laptop).
pub const PLAYABLE: &[&str] = &["mp4", "m4v", "webm", "ogv"];
/// Common video files that need turning into MP4 first. They are listed so nothing seems to be missing.
pub const NEEDS_CONVERTING: &[&str] = &["mkv", "avi", "mov", "wmv", "flv", "mpg", "mpeg", "ts", "3gp"];
const PICTURES: &[&str] = &["jpg", "jpeg", "png", "webp"];
const MAX_DEPTH: usize = 2; // topic, then one group folder

#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    /// Path under `videos/`, with `/` separators.
    pub rel: String,
    pub title: String,
    /// The group folder inside the topic ("" when the video sits directly in the topic).
    pub group: String,
    pub size: u64,
    pub ext: String,
    /// True when the player can play it; false when it needs converting to MP4.
    pub playable: bool,
    /// Path of the thumbnail picture, if there is one.
    pub thumb: Option<String>,
    pub modified: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Topic {
    /// The folder name under `videos/` ("" for loose files).
    pub folder: String,
    /// What to show on screen: the first line of `topic.txt`, else the folder name.
    pub name: String,
    /// By group, then title.
    pub videos: Vec<Video>,
}

fn ext_lower(name: &str) -> String {
    Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn is_video_ext(ext: &str) -> bool {
    PLAYABLE.contains(&ext) || NEEDS_CONVERTING.contains(&ext)
}

fn skip_name(name: &str) -> bool {
    name.starts_with('.') || name.starts_with('$') || name.eq_ignore_ascii_case("System Volume Information") || !valid_part(name)
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// A title for display: the file name without its extension, underscores read as spaces.
pub fn title_of(stem: &str) -> String {
    let t = stem.replace('_', " ");
    let t = t.trim();
    if t.is_empty() { stem.to_string() } else { t.to_string() }
}

/// Collect the videos in `dir` (one folder level), `rel_dir` being its path under `videos/` ("" for the top).
fn read_dir_videos(dir: &Path, rel_dir: &str, group: &str, out: &mut Vec<Video>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut pictures: BTreeMap<String, String> = BTreeMap::new(); // lowercase stem -> file name
    let mut files = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if skip_name(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let ext = ext_lower(&name);
        if PICTURES.contains(&ext.as_str()) {
            let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            pictures.entry(stem).or_insert(name);
        } else if is_video_ext(&ext) {
            files.push((name, ext, meta));
        }
    }
    for (name, ext, meta) in files {
        let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let join = |n: &str| if rel_dir.is_empty() { n.to_string() } else { format!("{}/{}", rel_dir, n) };
        let thumb = pictures.get(&stem.to_lowercase()).map(|p| join(p));
        out.push(Video {
            rel: join(&name),
            title: title_of(&stem),
            group: group.to_string(),
            size: meta.len(),
            playable: PLAYABLE.contains(&ext.as_str()),
            ext,
            thumb,
            modified: modified_secs(&meta),
        });
    }
}

fn topic_name(dir: &Path, folder: &str) -> String {
    if let Ok(text) = fs::read_to_string(dir.join("topic.txt")) {
        let first = text.lines().next().unwrap_or("").trim();
        if !first.is_empty() && first.chars().count() <= 120 && !first.chars().any(|c| c.is_control()) {
            return first.to_string();
        }
    }
    folder.to_string()
}

fn sort_videos(v: &mut [Video]) {
    v.sort_by(|a, b| {
        a.group.to_lowercase().cmp(&b.group.to_lowercase()).then(a.title.to_lowercase().cmp(&b.title.to_lowercase())).then(a.rel.cmp(&b.rel))
    });
}

/// Every topic under one `videos` folder. A topic with no videos is left out; a missing folder gives nothing.
pub fn scan(root: &Path) -> Vec<Topic> {
    let mut topics = Vec::new();
    let Ok(rd) = fs::read_dir(root) else { return topics };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if skip_name(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let mut videos = Vec::new();
        read_dir_videos(&e.path(), &name, "", &mut videos);
        if MAX_DEPTH > 1 {
            if let Ok(sub) = fs::read_dir(e.path()) {
                for g in sub.flatten() {
                    let gname = g.file_name().to_string_lossy().to_string();
                    if skip_name(&gname) || !g.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    read_dir_videos(&g.path(), &format!("{}/{}", name, gname), &gname, &mut videos);
                }
            }
        }
        if !videos.is_empty() {
            sort_videos(&mut videos);
            let shown = topic_name(&e.path(), &name);
            topics.push(Topic { folder: name, name: shown, videos });
        }
    }
    topics.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.folder.cmp(&b.folder)));
    let mut loose = Vec::new();
    read_dir_videos(root, "", "", &mut loose);
    if !loose.is_empty() {
        sort_videos(&mut loose);
        topics.push(Topic { folder: String::new(), name: LOOSE_TOPIC.to_string(), videos: loose });
    }
    topics
}

/// Every topic across several `videos` folders (the main drive's first). The same topic folder on two drives is one
/// topic; the same path on two drives is listed once, from the first drive.
pub fn scan_all(roots: &[PathBuf]) -> Vec<Topic> {
    let mut merged: Vec<Topic> = Vec::new();
    for r in roots {
        for t in scan(r) {
            match merged.iter_mut().find(|m| m.folder.eq_ignore_ascii_case(&t.folder)) {
                Some(m) => {
                    for v in t.videos {
                        if !m.videos.iter().any(|x| x.rel.eq_ignore_ascii_case(&v.rel)) {
                            m.videos.push(v);
                        }
                    }
                    sort_videos(&mut m.videos);
                }
                None => merged.push(t),
            }
        }
    }
    // topics by name, the catch-all last
    merged.sort_by(|a, b| {
        (a.folder.is_empty()).cmp(&b.folder.is_empty()).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    merged
}

/// The topics left after a search: a video stays when every word is in its title, group or topic name.
/// An empty search keeps everything.
pub fn search(topics: &[Topic], q: &str) -> Vec<Topic> {
    let words: Vec<String> = q.split_whitespace().map(|w| w.to_lowercase()).collect();
    if words.is_empty() {
        return topics.to_vec();
    }
    let mut out = Vec::new();
    for t in topics {
        let kept: Vec<Video> = t
            .videos
            .iter()
            .filter(|v| {
                let hay = format!("{} {} {}", v.title, v.group, t.name).to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .cloned()
            .collect();
        if !kept.is_empty() {
            out.push(Topic { folder: t.folder.clone(), name: t.name.clone(), videos: kept });
        }
    }
    out
}

fn inside(root: &Path, full: &Path) -> Option<PathBuf> {
    let real = full.canonicalize().ok()?;
    let root = root.canonicalize().ok()?;
    if real.starts_with(&root) && real.is_file() { Some(real) } else { None }
}

/// A video file or thumbnail `rel`, found under the first of `roots` that has it, only when it really is a video or
/// a picture inside that folder. Nothing else (no text files, no parent folders, no links out) is ever handed back.
pub fn resolve(roots: &[PathBuf], rel: &str) -> Option<PathBuf> {
    if !valid_rel(rel) || rel.split('/').count() > MAX_DEPTH + 1 || rel.split('/').any(|p| p.starts_with('.')) {
        return None;
    }
    let ext = ext_lower(rel);
    // only what the player plays, and pictures: a file that needs converting is never sent
    if !PLAYABLE.contains(&ext.as_str()) && !PICTURES.contains(&ext.as_str()) {
        return None;
    }
    for r in roots {
        let mut full = r.clone();
        for p in rel.split('/') {
            full.push(p);
        }
        if let Some(found) = inside(r, &full) {
            return Some(found);
        }
    }
    None
}

/// Remembered positions use the same small file format as podcasts.
pub fn progress_from(text: &str) -> Progress {
    Progress::parse(text)
}

/// A video is done when it was played to within this many seconds of its end.
pub const DONE_WITHIN: u32 = 30;

/// Whether a position counts as finished, given the video's length (0 when unknown, which is never finished).
pub fn is_done(position: u32, length: u32) -> bool {
    length > 0 && position + DONE_WITHIN >= length
}

/// The suggested folder names, created by the drive builder and shown in the docs.
pub const SUGGESTED_TOPICS: &[&str] = &[
    "Butchering and Processing",
    "Bushcraft",
    "Foraging",
    "Trapping and Fishing",
    "Shelter and Fire",
    "Water",
    "Medical",
    "Gardening and Food",
    "Repairs and Skills",
    "Other",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-videos-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn topics_groups_titles_and_thumbnails_are_found() {
        let d = tmp("list");
        touch(&d.join("Bushcraft/Friction fire.mp4"));
        touch(&d.join("Bushcraft/Friction fire.JPG"));
        touch(&d.join("Bushcraft/Fire/Bow_drill_tips.mp4"));
        touch(&d.join("Bushcraft/readme.txt"));
        touch(&d.join("Bushcraft/.hidden.mp4"));
        touch(&d.join("Butchering and Processing/Field dressing a deer.webm"));
        touch(&d.join("Butchering and Processing/Old clip.mkv"));
        touch(&d.join("Empty/notes.txt"));
        touch(&d.join("loose.mp4"));
        let topics = scan(&d);
        let names: Vec<&str> = topics.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, vec!["Bushcraft", "Butchering and Processing", LOOSE_TOPIC]);
        let b = &topics[0];
        assert_eq!(b.videos.len(), 2);
        // the video in the topic itself comes before the one in the group folder
        assert_eq!(b.videos[0].title, "Friction fire");
        assert_eq!(b.videos[0].thumb.as_deref(), Some("Bushcraft/Friction fire.JPG"));
        assert_eq!(b.videos[1].title, "Bow drill tips");
        assert_eq!(b.videos[1].group, "Fire");
        assert_eq!(b.videos[1].rel, "Bushcraft/Fire/Bow_drill_tips.mp4");
        let bp = &topics[1];
        assert_eq!(bp.videos.len(), 2);
        let mkv = bp.videos.iter().find(|v| v.ext == "mkv").unwrap();
        assert!(!mkv.playable, "listed, but flagged as needing conversion");
        assert!(bp.videos.iter().find(|v| v.ext == "webm").unwrap().playable);
        assert_eq!(topics[2].videos[0].rel, "loose.mp4");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_topic_can_have_a_nicer_name_and_a_bad_one_is_ignored() {
        let d = tmp("name");
        touch(&d.join("Bush/a.mp4"));
        fs::write(d.join("Bush/topic.txt"), "Bushcraft and Woodcraft\n").unwrap();
        assert_eq!(scan(&d)[0].name, "Bushcraft and Woodcraft");
        fs::write(d.join("Bush/topic.txt"), "bad\u{7}name").unwrap();
        assert_eq!(scan(&d)[0].name, "Bush");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_folder_is_just_empty() {
        assert!(scan(Path::new("/definitely/not/here/videos")).is_empty());
        assert!(scan_all(&[PathBuf::from("/definitely/not/here")]).is_empty());
    }

    #[test]
    fn two_drives_make_one_topic_and_the_first_copy_wins() {
        let a = tmp("drive-a");
        let b = tmp("drive-b");
        touch(&a.join("Bushcraft/Fire.mp4"));
        touch(&b.join("Bushcraft/Fire.mp4"));
        touch(&b.join("Bushcraft/Shelter.mp4"));
        touch(&b.join("bushcraft/Knots.mp4")); // same topic, different capital letter
        touch(&b.join("Water/Boiling.mp4"));
        let topics = scan_all(&[a.clone(), b.clone()]);
        let names: Vec<&str> = topics.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names.iter().filter(|n| n.eq_ignore_ascii_case("bushcraft")).count(), 1);
        assert!(names.contains(&"Water"));
        let bush = topics.iter().find(|t| t.folder.eq_ignore_ascii_case("bushcraft")).unwrap();
        assert_eq!(bush.videos.iter().filter(|v| v.title == "Fire").count(), 1);
        // the file comes from the first drive
        let found = resolve(&[a.clone(), b.clone()], "Bushcraft/Fire.mp4").unwrap();
        assert!(found.starts_with(a.canonicalize().unwrap()));
        // a video only on the second drive is still found
        let only_b = resolve(&[a.clone(), b.clone()], "Water/Boiling.mp4").unwrap();
        assert!(only_b.starts_with(b.canonicalize().unwrap()));
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn only_videos_the_player_plays_and_pictures_can_be_served() {
        let d = tmp("resolve");
        touch(&d.join("Bushcraft/a.mp4"));
        touch(&d.join("Bushcraft/a.jpg"));
        touch(&d.join("Bushcraft/b.mkv"));
        touch(&d.join("Bushcraft/c.txt"));
        touch(&d.join("Bushcraft/Fire/deep.mp4"));
        touch(&d.join("Bushcraft/Fire/Too/deep.mp4"));
        let roots = [d.clone()];
        assert!(resolve(&roots, "Bushcraft/a.mp4").is_some());
        assert!(resolve(&roots, "Bushcraft/a.jpg").is_some());
        assert!(resolve(&roots, "Bushcraft/Fire/deep.mp4").is_some());
        assert!(resolve(&roots, "Bushcraft/b.mkv").is_none(), "needs converting, not sent");
        assert!(resolve(&roots, "Bushcraft/c.txt").is_none());
        assert!(resolve(&roots, "Bushcraft/Fire/Too/deep.mp4").is_none(), "only one group level");
        assert!(resolve(&roots, "../a.mp4").is_none());
        assert!(resolve(&roots, "Bushcraft/../../a.mp4").is_none());
        assert!(resolve(&roots, "/etc/passwd").is_none());
        assert!(resolve(&roots, "Bushcraft\\a.mp4").is_none());
        assert!(resolve(&roots, "Bushcraft/missing.mp4").is_none());
        assert!(resolve(&roots, "").is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leads_outside_the_folder_is_refused() {
        let d = tmp("link");
        let outside = tmp("outside");
        touch(&outside.join("secret.mp4"));
        fs::create_dir_all(d.join("Bush")).unwrap();
        std::os::unix::fs::symlink(outside.join("secret.mp4"), d.join("Bush/secret.mp4")).unwrap();
        assert!(resolve(&[d.clone()], "Bush/secret.mp4").is_none());
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn search_needs_every_word_and_looks_at_topic_group_and_title() {
        let d = tmp("search");
        touch(&d.join("Butchering and Processing/Field dressing a deer.mp4"));
        touch(&d.join("Butchering and Processing/Quartering.mp4"));
        touch(&d.join("Bushcraft/Fire/Bow drill.mp4"));
        let topics = scan(&d);
        assert_eq!(search(&topics, "").len(), 2);
        let deer = search(&topics, "DEER dressing");
        assert_eq!(deer.len(), 1);
        assert_eq!(deer[0].videos.len(), 1);
        // the topic name counts: every butchering video matches "butchering"
        assert_eq!(search(&topics, "butchering")[0].videos.len(), 2);
        // so does the group folder
        assert_eq!(search(&topics, "fire drill")[0].videos[0].title, "Bow drill");
        assert!(search(&topics, "submarine").is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn finished_means_nearly_to_the_end() {
        assert!(is_done(590, 600));
        assert!(is_done(600, 600));
        assert!(!is_done(300, 600));
        assert!(!is_done(5, 0), "an unknown length is never finished");
    }

    #[test]
    fn positions_use_the_shared_progress_format() {
        let mut p = progress_from("");
        assert!(p.set("Bushcraft/Fire.mp4", 90, false));
        assert!(!p.set("../x.mp4", 1, false));
        let q = progress_from(&p.to_text());
        assert_eq!(q.get("Bushcraft/Fire.mp4"), Some((90, false)));
    }

    #[test]
    fn the_suggested_topics_are_plain_valid_folder_names() {
        for t in SUGGESTED_TOPICS {
            assert!(valid_part(t), "{}", t);
        }
    }
}
