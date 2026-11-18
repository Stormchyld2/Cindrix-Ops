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

/// The remembered answers, one line each: `always<TAB>id<TAB>name` or `never<TAB>id<TAB>name`, and at most one
/// `main<TAB>id<TAB>name` line: the drive the person chose to keep their own files on (by id, so it still works
/// whichever port or order the drives are plugged in; older versions of the program skip that line).
/// A missing, damaged or oversized file reads as no answers (so the person is simply asked again).
#[derive(Debug, Default, Clone)]
pub struct Registry {
    path: PathBuf,
    items: BTreeMap<String, Remembered>,
    main: Option<(String, String)>,
}

const MAX_BYTES: u64 = 100_000;

fn one_line(s: &str) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().trim().to_string()
}

impl Registry {
    pub fn load(path: &Path) -> Registry {
        let mut items = BTreeMap::new();
        let mut main = None;
        let ok = std::fs::metadata(path).map(|m| m.len() <= MAX_BYTES).unwrap_or(false);
        if ok {
            if let Ok(text) = std::fs::read_to_string(path) {
                for line in text.lines() {
                    let mut parts = line.splitn(3, '\t');
                    let word = parts.next();
                    let answer = match word {
                        Some("always") => Answer::Always,
                        Some("never") => Answer::Never,
                        Some("main") => Answer::Always, // not an answer: handled just below
                        _ => continue,
                    };
                    let raw = parts.next().unwrap_or("");
                    let id = drive::clean_id(raw, "");
                    if id == "cindrix" && raw.trim() != "cindrix" {
                        continue; // an empty or unusable id
                    }
                    let name = one_line(parts.next().unwrap_or(""));
                    if word == Some("main") {
                        main = Some((id, name));
                    } else {
                        items.insert(id, Remembered { answer, name });
                    }
                }
            }
        }
        Registry { path: path.to_path_buf(), items, main }
    }

    pub fn answer(&self, id: &str) -> Option<Answer> {
        self.items.get(id).map(|r| r.answer)
    }

    pub fn items(&self) -> impl Iterator<Item = (&String, &Remembered)> {
        self.items.iter()
    }

    /// The id of the drive the person chose to keep their own files on, if they chose one.
    pub fn main_id(&self) -> Option<&str> {
        self.main.as_ref().map(|(id, _)| id.as_str())
    }

    /// The name that drive had when it was chosen (so a message can name it while it is unplugged).
    pub fn main_name(&self) -> Option<&str> {
        self.main.as_ref().map(|(_, n)| n.as_str())
    }

    /// Choose the main drive. Kept for this run even when the file cannot be written.
    pub fn set_main(&mut self, id: &str, name: &str) -> std::io::Result<()> {
        self.main = Some((drive::clean_id(id, ""), one_line(name)));
        self.save()
    }

    /// Go back to the usual rule (Vault, then Hub, then the first drive found).
    pub fn clear_main(&mut self) -> std::io::Result<()> {
        if self.main.take().is_some() { self.save() } else { Ok(()) }
    }

    fn save(&self) -> std::io::Result<()> {
        let mut out = String::new();
        if let Some((id, name)) = &self.main {
            out.push_str(&format!("main\t{}\t{}\n", id, one_line(name)));
        }
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

/// Which library holds the person's own files, settings, models and engines. The main drive
/// wins: an allowed Vault drive first, then an allowed Hub drive, even when the program itself
/// sits in a folder that carries a marker (the downloadable Windows build does, as a Go drive).
/// Otherwise the usual rule applies (`drive::choose`): the program's own drive, else the first
/// allowed one, else the folder the program is in.
pub fn primary(program_dir: &Path, enabled: &[DriveInfo]) -> (PathBuf, Option<DriveInfo>) {
    primary_with_main(program_dir, enabled, None)
}

/// Like `primary`, but the drive the person chose as their main one (by id) comes first, when it is plugged in and
/// allowed. When it is not, the usual rule applies; `main_is_missing` tells the screens to say so.
pub fn primary_with_main(program_dir: &Path, enabled: &[DriveInfo], main: Option<&str>) -> (PathBuf, Option<DriveInfo>) {
    let pick = |d: &DriveInfo| (d.root.clone(), Some(d.clone()));
    // 1. a Vault drive, 2. the Hub drive itself (id "hub"), 3. the drive the person chose (Field Hub 1 on thumb drives),
    // 4. any other Hub drive, 5. the usual last choice. A second stick (Field Hub 2) is still a library, just never main.
    if let Some(d) = enabled.iter().find(|d| d.edition == "vault") {
        return pick(d);
    }
    if let Some(d) = enabled.iter().find(|d| d.edition == "hub" && d.id == "hub") {
        return pick(d);
    }
    if let Some(id) = main {
        if let Some(d) = enabled.iter().find(|d| d.id == id) {
            return pick(d);
        }
    }
    if let Some(d) = enabled.iter().find(|d| d.edition == "hub") {
        return pick(d);
    }
    drive::choose(program_dir, enabled)
}


/// The places to look for a named folder (`models`, `engines`), in order: the main drive's first, then each other
/// readable library's, then the folder next to the program. Each place appears once.
pub fn places_in_order(main_root: &Path, others: &[PathBuf], program_dir: &Path, sub: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    for r in std::iter::once(main_root).chain(others.iter().map(|p| p.as_path())).chain(std::iter::once(program_dir)) {
        let p = r.join(sub);
        if !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

/// The first of `places` that holds a file with this exact name.
pub fn find_file(places: &[PathBuf], name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains(['/', '\\']) || name == ".." || name == "." {
        return None;
    }
    places.iter().map(|p| p.join(name)).find(|f| f.is_file())
}

/// Every file name in `places` that `accept` likes, each once, from the first place that has it, in the order
/// of the places (names sorted within a place).
pub fn files_in_order(places: &[PathBuf], accept: impl Fn(&str) -> bool) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for p in places {
        let mut names: Vec<String> = std::fs::read_dir(p)
            .map(|rd| rd.flatten().filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false)).filter_map(|e| e.file_name().into_string().ok()).collect())
            .unwrap_or_default();
        names.sort();
        for n in names {
            if accept(&n) && !out.iter().any(|(m, _)| m.eq_ignore_ascii_case(&n)) {
                let f = p.join(&n);
                out.push((n, f));
            }
        }
    }
    out
}

/// True when a main drive was chosen but it is not plugged in (or not allowed), so another one is being used.
pub fn main_is_missing(enabled: &[DriveInfo], main: Option<&str>) -> bool {
    main.map(|id| !enabled.iter().any(|d| d.id == id)).unwrap_or(false)
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
        let libs = classify(std::slice::from_ref(&vault), &prog, &Registry::default(), &HashSet::new());
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
        let libs = classify(std::slice::from_ref(&other), &prog, &Registry::default(), &once);
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
    fn the_main_drive_is_primary_even_when_the_program_folder_carries_a_marker() {
        // the downloadable build is a folder with a Go marker; the 4 TB drive is Vault
        let (prog, own) = drive_with("portable", "go", "go");
        let (_v, vault) = drive_with("main-vault", "vault", "vault");
        let libs = classify(std::slice::from_ref(&vault), &prog, &Registry::default(), &HashSet::new());
        // not yet answered: nothing is read from the Vault, so the program's own folder is used
        let (root, info) = primary(&prog, &enabled(&libs));
        assert_eq!(root, own.root);
        assert_eq!(info.unwrap().edition, "go");
        // answered "always": the Vault drive becomes the one that holds your files
        let mut reg = Registry::load(&tmp("main-reg").join("libraries.txt"));
        reg.set("vault", "Vault", Answer::Always).unwrap();
        let libs = classify(std::slice::from_ref(&vault), &prog, &reg, &HashSet::new());
        let (root, info) = primary(&prog, &enabled(&libs));
        assert_eq!(root, vault.root);
        assert_eq!(info.unwrap().edition, "vault");
        // refused: back to the program's own folder
        reg.set("vault", "Vault", Answer::Never).unwrap();
        let libs = classify(&[vault], &prog, &reg, &HashSet::new());
        assert_eq!(primary(&prog, &enabled(&libs)).0, own.root);
    }

    #[test]
    fn vault_before_hub_before_the_programs_own_drive() {
        let (prog, _own) = drive_with("own-go", "go", "go");
        let (_h, hub) = drive_with("a-hub", "hub", "hub");
        let (_v, vault) = drive_with("a-vault", "vault", "vault");
        let (_g, go) = drive_with("other-go", "go", "other");
        assert_eq!(primary(&prog, &[go.clone(), hub.clone()]).0, hub.root);
        assert_eq!(primary(&prog, &[go, hub, vault.clone()]).0, vault.root);
        // nothing allowed at all: the folder the program runs from
        let plain = tmp("plain-prog");
        let (root, info) = primary(&plain, &[]);
        assert_eq!(root, plain);
        assert!(info.is_none());
    }

    #[test]
    fn the_chosen_main_drive_wins_whatever_the_order_and_a_missing_one_falls_back() {
        // two Field Hub sticks, both of the Hub edition, listed in either order
        let (prog, _own) = drive_with("fh-prog", "go", "go");
        let (_a, one) = drive_with("fh-1", "hub", "fieldhub1");
        let (_b, two) = drive_with("fh-2", "hub", "fieldhub2");
        // no choice: whichever is listed first is main, so the order decides
        assert_eq!(primary(&prog, &[two.clone(), one.clone()]).0, two.root);
        assert_eq!(primary(&prog, &[one.clone(), two.clone()]).0, one.root);
        // chosen: Field Hub 1 is main in both orders
        assert_eq!(primary_with_main(&prog, &[two.clone(), one.clone()], Some("fieldhub1")).0, one.root);
        assert_eq!(primary_with_main(&prog, &[one.clone(), two.clone()], Some("fieldhub1")).0, one.root);
        // Vault and the Hub hard drive still come first; the chosen stick is main when they are not plugged in
        let (_v, vault) = drive_with("fh-vault", "vault", "vault");
        assert_eq!(primary_with_main(&prog, &[one.clone(), vault.clone()], Some("fieldhub1")).0, vault.root);
        let (_h, hub) = drive_with("fh-hub", "hub", "hub");
        assert_eq!(primary_with_main(&prog, &[one.clone(), two.clone(), hub.clone()], Some("fieldhub1")).0, hub.root);
        // chosen but not plugged in: the usual rule, and the screens are told
        assert_eq!(primary_with_main(&prog, &[two.clone()], Some("fieldhub1")).0, two.root);
        assert!(main_is_missing(&[two.clone()], Some("fieldhub1")));
        assert!(!main_is_missing(&[one, two], Some("fieldhub1")));
        assert!(!main_is_missing(&[], None));
    }

    #[test]
    fn model_and_engine_files_are_found_on_any_drive_in_order() {
        let main = tmp("any-main");
        let other = tmp("any-other");
        let prog = tmp("any-prog");
        for (d, f) in [(&main, "models/small-2b.gguf"), (&other, "models/big-9b.gguf"), (&other, "models/small-2b.gguf"), (&other, "engines/ffmpeg.exe"), (&prog, "models/old-4b.gguf")] {
            fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
            fs::write(d.join(f), f.as_bytes()).unwrap();
        }
        let models = places_in_order(&main, &[other.clone(), main.clone()], &prog, "models");
        assert_eq!(models, vec![main.join("models"), other.join("models"), prog.join("models")], "main first, no repeats");
        let list = files_in_order(&models, |n| n.ends_with(".gguf"));
        let names: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["small-2b.gguf", "big-9b.gguf", "old-4b.gguf"]);
        // the same name on two drives comes from the first
        assert_eq!(list[0].1, main.join("models/small-2b.gguf"));
        assert_eq!(find_file(&models, "big-9b.gguf"), Some(other.join("models/big-9b.gguf")));
        // an engine only on a second drive is still found
        let engines = places_in_order(&main, &[other.clone()], &prog, "engines");
        assert_eq!(find_file(&engines, "ffmpeg.exe"), Some(other.join("engines/ffmpeg.exe")));
        assert_eq!(find_file(&engines, "llama-server.exe"), None);
        // never a path out of the folder
        assert_eq!(find_file(&models, "../x"), None);
        assert_eq!(find_file(&models, "a/b.gguf"), None);
        assert_eq!(find_file(&models, ""), None);
        for d in [main, other, prog] {
            let _ = fs::remove_dir_all(d);
        }
    }

    #[test]
    fn the_main_choice_is_remembered_and_older_programs_can_still_read_the_file() {
        let dir = tmp("main-choice");
        let file = dir.join("libraries.txt");
        let mut reg = Registry::load(&file);
        assert_eq!(reg.main_id(), None);
        reg.set("fieldhub1", "Field Hub 1", Answer::Always).unwrap();
        reg.set_main("FieldHub1", "Field Hub 1").unwrap();
        let again = Registry::load(&file);
        assert_eq!(again.main_id(), Some("fieldhub1"));
        assert_eq!(again.main_name(), Some("Field Hub 1"));
        assert_eq!(again.answer("fieldhub1"), Some(Answer::Always));
        // the main line is not an answer: it adds no library to the list
        assert_eq!(again.items().count(), 1);
        // forgetting the answer keeps the choice; clearing the choice keeps the answer
        let mut r = again;
        r.forget("fieldhub1").unwrap();
        assert_eq!(Registry::load(&file).main_id(), Some("fieldhub1"));
        r.set("fieldhub1", "Field Hub 1", Answer::Always).unwrap();
        r.clear_main().unwrap();
        let cleared = Registry::load(&file);
        assert_eq!(cleared.main_id(), None);
        assert_eq!(cleared.answer("fieldhub1"), Some(Answer::Always));
        // a damaged main line is ignored
        fs::write(&file, "main\t\t\nalways\tvault\tVault\n").unwrap();
        let bad = Registry::load(&file);
        assert_eq!(bad.main_id(), None);
        assert_eq!(bad.answer("vault"), Some(Answer::Always));
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
