//! Libraries: the Cindrix drives on this computer, and which of them the person has said may be
//! read. A drive only counts when it carries the marker file (`drive.rs`); other disks are never
//! asked about. The answer to "may I read this drive?" is remembered by the drive's id (for the
//! owner's drives simply `vault`, `go` or `hub`), not by its drive letter, in a small plain file
//! kept on this computer. Standard library only.

use crate::drive::{self, DriveInfo};
use crate::fsio;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// What the person said they want for a drive, when it is remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Always,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remembered {
    pub answer: Answer,
    /// The drive's name when it was last seen, so the list can still show it while unplugged.
    pub name: String,
}

/// The remembered answers, one line each: `always<TAB>id<TAB>name` or `never<TAB>id<TAB>name`.
/// A missing, damaged or oversized file reads as no answers (so the person is simply asked again).
#[derive(Debug, Default, Clone)]
pub struct Registry {
    path: PathBuf,
    items: BTreeMap<String, Remembered>,
}

const MAX_BYTES: u64 = 100_000;

fn one_line(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_string()
}

impl Registry {
    pub fn load(path: &Path) -> Registry {
        let mut items = BTreeMap::new();
        let ok = std::fs::metadata(path).map(|m| m.len() <= MAX_BYTES).unwrap_or(false);
        if ok {
            if let Ok(text) = std::fs::read_to_string(path) {
                for line in text.lines() {
                    let mut parts = line.splitn(3, '\t');
                    let answer = match parts.next() {
                        Some("always") => Answer::Always,
                        Some("never") => Answer::Never,
                        _ => continue,
                    };
                    let raw = parts.next().unwrap_or("");
                    let id = drive::clean_id(raw, "");
                    if id == "cindrix" && raw.trim() != "cindrix" {
                        continue; // an empty or unusable id
                    }
                    let name = one_line(parts.next().unwrap_or(""));
                    items.insert(id, Remembered { answer, name });
                }
            }
        }
        Registry { path: path.to_path_buf(), items }
    }

    pub fn answer(&self, id: &str) -> Option<Answer> {
        self.items.get(id).map(|r| r.answer)
    }

    pub fn items(&self) -> impl Iterator<Item = (&String, &Remembered)> {
        self.items.iter()
    }

    fn save(&self) -> std::io::Result<()> {
        let mut out = String::new();
        for (id, r) in &self.items {
            let word = if r.answer == Answer::Always { "always" } else { "never" };
            out.push_str(&format!("{}\t{}\t{}\n", word, id, one_line(&r.name)));
        }
        fsio::write_atomic(&self.path, out.as_bytes())
    }

    /// Remember an answer. It is kept for this run even when the file cannot be written; the error
    /// says so, so the program can tell the person it will ask again next time.
    pub fn set(&mut self, id: &str, name: &str, answer: Answer) -> std::io::Result<()> {
        let id = drive::clean_id(id, "");
        self.items.insert(id, Remembered { answer, name: one_line(name) });
        self.save()
    }

    pub fn forget(&mut self, id: &str) -> std::io::Result<()> {
        let id = drive::clean_id(id, "");
        if self.items.remove(&id).is_some() { self.save() } else { Ok(()) }
    }
}

/// Where the answers are kept: on this computer, not on the drive (the answer is about this
/// computer's trust in the drive).
pub fn registry_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("Cindrix").join("libraries.txt")
}

/// Where a library stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The drive the program itself is running from: it travels with its own data.
    Own,
    /// Remembered "always".
    Always,
    /// "Just this time": read until the program is closed.
    Once,
    /// Remembered "never".
    Never,
    /// No answer yet: nothing is read from it until the person says.
    Ask,
}

impl State {
    pub fn enabled(self) -> bool {
        matches!(self, State::Own | State::Always | State::Once)
    }
    pub fn word(self) -> &'static str {
        match self {
            State::Own => "own",
            State::Always => "always",
            State::Once => "once",
            State::Never => "never",
            State::Ask => "ask",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Library {
    pub info: DriveInfo,
    pub state: State,
    /// The question suggests "Always" for the main drives (Vault and Hub), never for others.
    pub suggest_always: bool,
}

/// Work out where each drive that is plugged in stands. `once` holds the folders the person
/// answered "just this time" for in this run. The program's own drive is always included and
/// always allowed, even if it is not among `found`.
pub fn classify(found: &[DriveInfo], program_dir: &Path, reg: &Registry, once: &HashSet<PathBuf>) -> Vec<Library> {
    let own = drive::check(program_dir);
    let mut all: Vec<DriveInfo> = Vec::new();
    if let Some(o) = &own {
        all.push(o.clone());
    }
    for d in found {
        if !all.iter().any(|x| x.root == d.root) {
            all.push(d.clone());
        }
    }
    all.into_iter()
        .map(|info| {
            let state = if own.as_ref().map(|o| o.root == info.root).unwrap_or(false) {
                State::Own
            } else if once.contains(&info.root) {
                State::Once
            } else {
                match reg.answer(&info.id) {
                    Some(Answer::Always) => State::Always,
                    Some(Answer::Never) => State::Never,
                    None => State::Ask,
                }
            };
            let suggest_always = info.edition == "vault" || info.edition == "hub";
            Library { info, state, suggest_always }
        })
        .collect()
}

/// The drives that may be read, in the order found.
pub fn enabled(libs: &[Library]) -> Vec<DriveInfo> {
    libs.iter().filter(|l| l.state.enabled()).map(|l| l.info.clone()).collect()
}

/// Find the drives on this computer and work out where each stands.
pub fn scan(program_dir: &Path, reg: &Registry, once: &HashSet<PathBuf>) -> Vec<Library> {
    classify(&drive::find_all(), program_dir, reg, once)
}

/// The `library` folders to read archives from: the primary library first, then every other
/// library that may be read. Where two libraries hold a file with the same name, the first one
/// wins, so a name never appears twice.
pub fn archive_files(primary_library_dir: &Path, others: &[PathBuf], list: impl Fn(&Path) -> Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let mut names: HashSet<String> = HashSet::new();
    let mut dirs: Vec<PathBuf> = vec![primary_library_dir.to_path_buf()];
    for o in others {
        if !dirs.contains(o) {
            dirs.push(o.clone());
        }
    }
    for dir in dirs {
        for f in list(&dir) {
            let name = f.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            if names.insert(name) {
                out.push(f);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-libs-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn drive_with(name: &str, edition: &str, id: &str) -> (PathBuf, DriveInfo) {
        let d = tmp(name);
        fs::write(d.join(drive::MARKER), format!(r#"{{"name":"{}","edition":"{}","id":"{}"}}"#, name, edition, id)).unwrap();
        let info = drive::check(&d).unwrap();
        (d, info)
    }

    #[test]
    fn answers_are_remembered_between_runs() {
        let dir = tmp("reg");
        let file = dir.join("libraries.txt");
        let mut r = Registry::load(&file);
        assert_eq!(r.answer("vault"), None);
        r.set("vault", "Cindrix Ops Vault", Answer::Always).unwrap();
        r.set("old-stick", "Someone's stick", Answer::Never).unwrap();
        let again = Registry::load(&file);
        assert_eq!(again.answer("vault"), Some(Answer::Always));
        assert_eq!(again.answer("old-stick"), Some(Answer::Never));
        assert_eq!(again.items().count(), 2);
        let mut again = again;
        again.forget("vault").unwrap();
        assert_eq!(Registry::load(&file).answer("vault"), None);
        assert_eq!(Registry::load(&file).answer("old-stick"), Some(Answer::Never));
    }

    #[test]
    fn a_damaged_answers_file_just_means_asking_again() {
        let dir = tmp("damaged");
        let file = dir.join("libraries.txt");
        fs::write(&file, "garbage\n\u{0}\u{1}\nmaybe\tvault\tx\nalways\t\tno id\nalways\tgood\tName\twith tab\n").unwrap();
        let r = Registry::load(&file);
        assert_eq!(r.answer("vault"), None);
        assert_eq!(r.answer("good"), Some(Answer::Always));
        assert_eq!(r.items().count(), 1);
        // a file that does not exist, and one that is far too big, both read as nothing
        assert_eq!(Registry::load(&dir.join("nope.txt")).items().count(), 0);
        let big = dir.join("big.txt");
        fs::write(&big, "always\tvault\t".to_string() + &"x".repeat(200_000)).unwrap();
        assert_eq!(Registry::load(&big).items().count(), 0);
    }

    #[test]
    fn a_name_with_odd_characters_cannot_break_the_file() {
        let dir = tmp("odd-name");
        let file = dir.join("libraries.txt");
        let mut r = Registry::load(&file);
        r.set("vault", "My\tVault\nalways\tevil\tx", Answer::Never).unwrap();
        let again = Registry::load(&file);
        assert_eq!(again.items().count(), 1);
        assert_eq!(again.answer("evil"), None);
        assert_eq!(again.answer("vault"), Some(Answer::Never));
    }

    #[test]
    fn a_new_drive_is_asked_about_and_nothing_is_read_until_answered() {
        let (_d, vault) = drive_with("new-vault", "vault", "vault");
        let prog = tmp("prog-a"); // not a drive
        let libs = classify(&[vault.clone()], &prog, &Registry::default(), &HashSet::new());
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].state, State::Ask);
        assert!(libs[0].suggest_always);
        assert!(enabled(&libs).is_empty());
    }

    #[test]
    fn remembered_answers_apply_by_id_even_on_another_drive_letter() {
        let dir = tmp("letters");
        let mut reg = Registry::load(&dir.join("libraries.txt"));
        reg.set("vault", "Cindrix Ops Vault", Answer::Always).unwrap();
        reg.set("stick", "A stick", Answer::Never).unwrap();
        let prog = tmp("prog-b");
        // the same drive, found at two different places on two different "computers"
        let (_a, vault_here) = drive_with("letter-e", "vault", "vault");
        let (_b, vault_there) = drive_with("letter-f", "vault", "vault");
        for v in [vault_here, vault_there] {
            let libs = classify(&[v], &prog, &reg, &HashSet::new());
            assert_eq!(libs[0].state, State::Always);
            assert_eq!(enabled(&libs).len(), 1);
        }
        let (_c, stick) = drive_with("stick", "go", "stick");
        let libs = classify(&[stick], &prog, &reg, &HashSet::new());
        assert_eq!(libs[0].state, State::Never);
        assert!(!libs[0].suggest_always);
        assert!(enabled(&libs).is_empty());
    }

    #[test]
    fn just_this_time_lasts_only_for_that_folder_in_this_run() {
        let (_d, other) = drive_with("once", "go", "somebody");
        let prog = tmp("prog-c");
        let mut once = HashSet::new();
        once.insert(other.root.clone());
        let libs = classify(&[other.clone()], &prog, &Registry::default(), &once);
        assert_eq!(libs[0].state, State::Once);
        assert!(libs[0].state.enabled());
        // a fresh run has an empty set again: asked once more
        let libs = classify(&[other], &prog, &Registry::default(), &HashSet::new());
        assert_eq!(libs[0].state, State::Ask);
    }

    #[test]
    fn the_drive_the_program_runs_from_is_always_allowed() {
        let (own, own_info) = drive_with("own", "go", "go");
        // not found by the scan of drive letters, yet still listed and allowed
        let libs = classify(&[], &own, &Registry::default(), &HashSet::new());
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].state, State::Own);
        // and not listed twice when it is also found, even after a "never" for its id
        let dir = tmp("own-never");
        let mut reg = Registry::load(&dir.join("libraries.txt"));
        reg.set("go", "Go", Answer::Never).unwrap();
        let libs = classify(&[own_info], &own, &reg, &HashSet::new());
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].state, State::Own);
    }

    #[test]
    fn two_drives_with_the_same_id_stay_two_libraries() {
        let (_a, one) = drive_with("twin-1", "vault", "vault");
        let (_b, two) = drive_with("twin-2", "vault", "vault");
        let prog = tmp("prog-d");
        let libs = classify(&[one, two], &prog, &Registry::default(), &HashSet::new());
        assert_eq!(libs.len(), 2);
        assert!(libs.iter().all(|l| l.state == State::Ask));
    }

    #[test]
    fn no_marker_no_question() {
        let plain = tmp("plain-disk"); // an ordinary folder or disk
        let prog = tmp("prog-e");
        let found = drive::find_in(&[plain]);
        assert!(classify(&found, &prog, &Registry::default(), &HashSet::new()).is_empty());
    }

    #[test]
    fn archives_from_every_library_without_a_name_twice() {
        let a = tmp("arch-a");
        let b = tmp("arch-b");
        for (d, files) in [(&a, vec!["wiki.zim", "medical.zim"]), (&b, vec!["Wiki.zim", "maps.zim"])] {
            for f in files {
                fs::write(d.join(f), b"x").unwrap();
            }
        }
        let list = |d: &Path| -> Vec<PathBuf> {
            let mut v: Vec<PathBuf> = fs::read_dir(d).unwrap().flatten().map(|e| e.path()).collect();
            v.sort();
            v
        };
        let got = archive_files(&a, &[b.clone(), a.clone()], list);
        let names: Vec<String> = got.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["medical.zim", "wiki.zim", "maps.zim"]); // the primary's wiki.zim wins
        assert!(got[1].starts_with(&a));
    }
}
