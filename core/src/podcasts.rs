//! Podcasts: episodes saved as plain audio files in `podcasts/` on the drive, one folder per show,
//! the way Cindrix Radio (the phone app) saves them: `podcasts/<Show>/<YYYY-MM-DD> - <Episode>.mp3`.
//! A show folder may hold a `show.txt` whose first line is the show's name. This module lists the
//! shows and episodes and remembers where each episode was left off. It never uses the internet.

use crate::media::{self, valid_part, Class};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Shows with no folder of their own (files dropped straight into `podcasts/`) are listed under this name.
pub const LOOSE_SHOW: &str = "Other episodes";

#[derive(Debug, Clone, PartialEq)]
pub struct Episode {
    /// Path under the podcasts folder, with `/` separators.
    pub rel: String,
    pub title: String,
    /// `YYYY-MM-DD`, or empty when the file name has none.
    pub date: String,
    pub size: u64,
    pub ext: String,
    /// Seconds since 1970 the file was last changed (used to order episodes without a date in the name).
    pub modified: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Show {
    /// The folder name under `podcasts/` ("" for loose files).
    pub folder: String,
    /// What to show on screen: the first line of `show.txt`, else the folder name.
    pub name: String,
    /// Newest first.
    pub episodes: Vec<Episode>,
}

/// Split a file name (without extension) into its date and title. "2026-11-02 - The title" gives
/// ("2026-11-02", "The title"); a name with no date in front is all title.
pub fn parse_name(stem: &str) -> (String, String) {
    let b = stem.as_bytes();
    let dated = b.len() >= 10
        && b[..10].iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
        && stem.is_char_boundary(10);
    if !dated {
        return (String::new(), stem.trim().to_string());
    }
    let month: u32 = stem[5..7].parse().unwrap_or(0);
    let day: u32 = stem[8..10].parse().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return (String::new(), stem.trim().to_string());
    }
    let rest = stem[10..].trim_start_matches([' ', '-', '_', '.']).trim();
    let title = if rest.is_empty() { stem[..10].to_string() } else { rest.to_string() };
    (stem[..10].to_string(), title)
}

fn is_episode_file(name: &str) -> bool {
    !name.starts_with('.') && valid_part(name) && media::class_of(Path::new(name)) == Some(Class::Native)
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

fn read_episodes(dir: &Path, folder: &str) -> Vec<Episode> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !is_episode_file(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let path = e.path();
        let ext = media::ext_of(&path);
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let (date, title) = parse_name(&stem);
        let rel = if folder.is_empty() { name.clone() } else { format!("{}/{}", folder, name) };
        out.push(Episode { rel, title, date, size: meta.len(), ext, modified: modified_secs(&meta) });
    }
    // Newest first: by the date in the name, else by when the file was saved; ties by title.
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.modified.cmp(&a.modified)).then(a.title.cmp(&b.title)));
    out
}

fn show_name(dir: &Path, folder: &str) -> String {
    if let Ok(text) = fs::read_to_string(dir.join("show.txt")) {
        let first = text.lines().next().unwrap_or("").trim();
        if !first.is_empty() && first.chars().count() <= 120 && !first.chars().any(|c| c.is_control()) {
            return first.to_string();
        }
    }
    folder.to_string()
}

/// Every show under `root`, by name, each with its episodes (newest first). A show with no
/// playable episodes is left out. A missing folder gives an empty list.
pub fn scan(root: &Path) -> Vec<Show> {
    let mut shows = Vec::new();
    let Ok(rd) = fs::read_dir(root) else { return shows };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('$') || !valid_part(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let episodes = read_episodes(&e.path(), &name);
        if !episodes.is_empty() {
            let shown = show_name(&e.path(), &name);
            shows.push(Show { folder: name, name: shown, episodes });
        }
    }
    shows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.folder.cmp(&b.folder)));
    let loose = read_episodes(root, "");
    if !loose.is_empty() {
        shows.push(Show { folder: String::new(), name: LOOSE_SHOW.to_string(), episodes: loose });
    }
    shows
}

/// An episode file `rel` under `root`, only if it really is an audio file inside `root`.
pub fn resolve(root: &Path, rel: &str) -> Option<std::path::PathBuf> {
    let path = media::serve::resolve_track(root, rel)?;
    if media::class_of(&path) == Some(Class::Native) { Some(path) } else { None }
}

// ---------- where each episode was left off ----------

/// Seconds listened and whether the episode is finished, by episode path.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    map: BTreeMap<String, (u32, bool)>,
}

/// More than this many remembered episodes and the oldest finished ones are forgotten.
const MAX_REMEMBERED: usize = 20_000;

impl Progress {
    /// Reads the saved text (`rel<TAB>seconds<TAB>done` per line). Lines that make no sense are skipped.
    pub fn parse(text: &str) -> Progress {
        let mut map = BTreeMap::new();
        for line in text.lines() {
            let mut it = line.split('\t');
            let (Some(rel), Some(secs), Some(done)) = (it.next(), it.next(), it.next()) else { continue };
            if !media::valid_rel(rel) {
                continue;
            }
            let Ok(secs) = secs.trim().parse::<u32>() else { continue };
            map.insert(rel.to_string(), (secs, done.trim() == "1"));
        }
        Progress { map }
    }

    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for (rel, (secs, done)) in &self.map {
            s.push_str(&format!("{}\t{}\t{}\n", rel, secs, if *done { 1 } else { 0 }));
        }
        s
    }

    /// Remember a position. Finished episodes keep `done`; going back into one (more than a
    /// minute from its end, reported by the caller as `done = false`) makes it unfinished again.
    pub fn set(&mut self, rel: &str, secs: u32, done: bool) -> bool {
        if !media::valid_rel(rel) {
            return false;
        }
        self.map.insert(rel.to_string(), (secs, done));
        if self.map.len() > MAX_REMEMBERED {
            let drop: Vec<String> = self.map.iter().filter(|(_, v)| v.1).map(|(k, _)| k.clone()).take(self.map.len() - MAX_REMEMBERED).collect();
            for k in drop {
                self.map.remove(&k);
            }
        }
        true
    }

    pub fn get(&self, rel: &str) -> Option<(u32, bool)> {
        self.map.get(rel).copied()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn entries(&self) -> Vec<(String, u32, bool)> {
        self.map.iter().map(|(k, v)| (k.clone(), v.0, v.1)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-podcasts-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn names_are_split_into_date_and_title() {
        assert_eq!(parse_name("2026-11-02 - The title"), ("2026-11-02".into(), "The title".into()));
        assert_eq!(parse_name("2026-11-02_The title"), ("2026-11-02".into(), "The title".into()));
        assert_eq!(parse_name("2026-11-02"), ("2026-11-02".into(), "2026-11-02".into()));
        assert_eq!(parse_name("Just a title"), (String::new(), "Just a title".into()));
        // not a real date: all title
        assert_eq!(parse_name("2026-13-40 - nope"), (String::new(), "2026-13-40 - nope".into()));
        assert_eq!(parse_name("12345"), (String::new(), "12345".into()));
        assert_eq!(parse_name("é"), (String::new(), "é".into()));
    }

    #[test]
    fn shows_and_episodes_are_listed_newest_first() {
        let d = tmp("list");
        touch(&d.join("Prepper Talk/2026-10-01 - First.mp3"));
        touch(&d.join("Prepper Talk/2026-11-01 - Second.mp3"));
        touch(&d.join("Prepper Talk/notes.txt"));
        touch(&d.join("Prepper Talk/.hidden.mp3"));
        touch(&d.join("Alpha/one.m4a"));
        touch(&d.join("Empty/readme.txt"));
        touch(&d.join("loose.mp3"));
        fs::write(d.join("Prepper Talk/show.txt"), "The Prepper Talk Show\nhttps://example.invalid/feed\n").unwrap();
        let shows = scan(&d);
        let names: Vec<&str> = shows.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "The Prepper Talk Show", LOOSE_SHOW]);
        let pt = &shows[1];
        assert_eq!(pt.folder, "Prepper Talk");
        assert_eq!(pt.episodes.len(), 2);
        assert_eq!(pt.episodes[0].title, "Second");
        assert_eq!(pt.episodes[0].rel, "Prepper Talk/2026-11-01 - Second.mp3");
        assert_eq!(pt.episodes[1].date, "2026-10-01");
        assert_eq!(shows[2].episodes[0].rel, "loose.mp3");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_missing_folder_is_just_empty() {
        assert!(scan(Path::new("/definitely/not/here/podcasts")).is_empty());
    }

    #[test]
    fn a_bad_show_name_file_falls_back_to_the_folder() {
        let d = tmp("name");
        touch(&d.join("Show/a.mp3"));
        fs::write(d.join("Show/show.txt"), "\n\n").unwrap();
        assert_eq!(scan(&d)[0].name, "Show");
        fs::write(d.join("Show/show.txt"), "bad\u{7}name").unwrap();
        assert_eq!(scan(&d)[0].name, "Show");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn only_real_episode_files_can_be_served() {
        let d = tmp("resolve");
        touch(&d.join("Show/a.mp3"));
        touch(&d.join("Show/b.wma"));
        touch(&d.join("Show/c.txt"));
        assert!(resolve(&d, "Show/a.mp3").is_some());
        assert!(resolve(&d, "Show/b.wma").is_none(), "needs converting, not offered here");
        assert!(resolve(&d, "Show/c.txt").is_none());
        assert!(resolve(&d, "../a.mp3").is_none());
        assert!(resolve(&d, "Show/missing.mp3").is_none());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn progress_round_trips_and_skips_bad_lines() {
        let mut p = Progress::default();
        assert!(p.set("Show/a.mp3", 125, false));
        assert!(p.set("Show/b.mp3", 3000, true));
        assert!(!p.set("../evil.mp3", 1, false));
        let text = p.to_text();
        let q = Progress::parse(&format!("{}garbage\nnot\tanumber\t0\n../x.mp3\t5\t0\n", text));
        assert_eq!(q, p);
        assert_eq!(q.get("Show/a.mp3"), Some((125, false)));
        assert_eq!(q.get("Show/b.mp3"), Some((3000, true)));
        assert_eq!(q.get("Show/none.mp3"), None);
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn going_back_into_a_finished_episode_unfinishes_it() {
        let mut p = Progress::default();
        p.set("S/a.mp3", 3000, true);
        p.set("S/a.mp3", 40, false);
        assert_eq!(p.get("S/a.mp3"), Some((40, false)));
    }
}
