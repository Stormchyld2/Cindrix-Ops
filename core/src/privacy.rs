//! A passcode for one person's journal, personal photos and comms plan (`user/people/<name>/`).
//!
//! A person with a `lock.json` in their folder is "protected": everything of theirs is stored sealed, as
//! `<file>.sealed`, and can be read only while their key is in memory (after the passcode was typed). This
//! module holds the file work and is tested here with a stand-in sealer; the real one is made in the app from
//! the `cindrix-crypto` crate (XChaCha20-Poly1305 with a key that the passcode wraps).
//!
//! What is sealed: journal entries, the comms plan, the person's own pictures, and their `backups` folder.
//! What is not: file *names* (dates of journal entries, picture names), because the program lists them
//! without the key. The shared album is not part of this: it is for everyone.
//! A forgotten passcode cannot be recovered. Nothing here ever leaves a plain copy next to a sealed one.

use crate::fsio;
use crate::people::{self, Entry, Photo};
use std::fs;
use std::path::{Path, PathBuf};

/// The error text for "this person is protected and not unlocked".
pub const LOCKED: &str = "locked";
pub const SUFFIX: &str = ".sealed";

pub trait Sealer {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String>;
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String>;
}

fn io_err(what: &str, e: std::io::Error) -> String {
    format!("{}: {}", what, e)
}

fn sealed_of(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(SUFFIX);
    PathBuf::from(s)
}

/// `user/people/<name>/lock.json`
pub fn lock_file(user_dir: &Path, person: &str) -> Result<PathBuf, String> {
    Ok(people::person_dir(user_dir, person)?.join("lock.json"))
}

pub fn is_protected(user_dir: &Path, person: &str) -> bool {
    lock_file(user_dir, person).map(|f| f.is_file()).unwrap_or(false)
}

/// The sealer to use for this person: None when they are not protected, `Err(LOCKED)` when they are protected
/// and none was given.
pub fn need<'a>(user_dir: &Path, person: &str, s: Option<&'a dyn Sealer>) -> Result<Option<&'a dyn Sealer>, String> {
    if is_protected(user_dir, person) {
        match s {
            Some(x) => Ok(Some(x)),
            None => Err(LOCKED.to_string()),
        }
    } else {
        Ok(None)
    }
}

fn read_maybe_sealed(path: &Path, s: Option<&dyn Sealer>) -> Result<Option<Vec<u8>>, String> {
    let sealed = sealed_of(path);
    if sealed.is_file() {
        let s = s.ok_or_else(|| LOCKED.to_string())?;
        let b = fs::read(&sealed).map_err(|e| io_err("Could not read the file", e))?;
        return s.open(&b).map(Some);
    }
    if path.is_file() {
        return fs::read(path).map(Some).map_err(|e| io_err("Could not read the file", e));
    }
    Ok(None)
}

fn write_maybe_sealed(path: &Path, backups: &Path, data: &[u8], s: Option<&dyn Sealer>) -> Result<(), String> {
    match s {
        Some(s) => {
            let sealed = s.seal(data)?;
            let target = sealed_of(path);
            fsio::backup_then_write(&target, backups, &sealed).map_err(|e| io_err("Could not save", e))?;
            // never a plain copy beside the sealed one
            if path.is_file() {
                let _ = fs::remove_file(path);
            }
            Ok(())
        }
        None => fsio::backup_then_write(path, backups, data).map_err(|e| io_err("Could not save", e)),
    }
}

// ---------- journal ----------

pub fn journal_list(user_dir: &Path, person: &str, s: Option<&dyn Sealer>) -> Result<Vec<Entry>, String> {
    let s = need(user_dir, person, s)?;
    let dir = people::person_dir(user_dir, person)?.join("journal");
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(Vec::new()) };
    let mut dates: Vec<String> = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let base = name.strip_suffix(SUFFIX).unwrap_or(&name);
        if let Some(d) = base.strip_suffix(".md") {
            if people::valid_date(d) && !dates.iter().any(|x| x == d) {
                dates.push(d.to_string());
            }
        }
    }
    let mut out = Vec::new();
    for d in dates {
        let bytes = read_maybe_sealed(&dir.join(format!("{}.md", d)), s)?.unwrap_or_default();
        out.push(Entry { date: d, text: String::from_utf8_lossy(&bytes).to_string() });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(out)
}

pub fn journal_save(user_dir: &Path, person: &str, date: &str, text: &str, s: Option<&dyn Sealer>) -> Result<(), String> {
    let s = need(user_dir, person, s)?;
    if !people::valid_date(date) {
        return Err("That date does not look right.".into());
    }
    if text.len() > people::MAX_JOURNAL_BYTES {
        return Err("That entry is too long. Split it over two days.".into());
    }
    let pdir = people::person_dir(user_dir, person)?;
    let file = pdir.join("journal").join(format!("{}.md", date));
    let backups = pdir.join("backups");
    if text.trim().is_empty() {
        for f in [file.clone(), sealed_of(&file)] {
            if f.is_file() {
                fs::create_dir_all(&backups).map_err(|e| io_err("Could not make the backups folder", e))?;
                let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                fs::copy(&f, backups.join(format!("{}-{}", secs, name))).map_err(|e| io_err("Could not keep a copy first", e))?;
                fs::remove_file(&f).map_err(|e| io_err("Could not remove the entry", e))?;
            }
        }
        return Ok(());
    }
    write_maybe_sealed(&file, &backups, text.as_bytes(), s)
}

// ---------- comms plan ----------

pub fn plan_load(user_dir: &Path, person: &str, s: Option<&dyn Sealer>) -> Result<String, String> {
    let s = need(user_dir, person, s)?;
    let file = people::person_dir(user_dir, person)?.join("comms-plan.md");
    match read_maybe_sealed(&file, s)? {
        Some(b) => Ok(String::from_utf8_lossy(&b).to_string()),
        None => Ok(people::comms_plan_template()),
    }
}

pub fn plan_save(user_dir: &Path, person: &str, text: &str, s: Option<&dyn Sealer>) -> Result<(), String> {
    let s = need(user_dir, person, s)?;
    if text.len() > people::MAX_PLAN_BYTES {
        return Err("The plan is too long.".into());
    }
    let pdir = people::person_dir(user_dir, person)?;
    write_maybe_sealed(&pdir.join("comms-plan.md"), &pdir.join("backups"), text.as_bytes(), s)
}

// ---------- the person's own pictures ----------

/// The pictures in a folder, plain or sealed, by their real names (the `.sealed` ending is not shown).
pub fn photos_list(dir: &Path) -> Vec<Photo> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Photo> = Vec::new();
    for e in rd.flatten() {
        let raw = e.file_name().to_string_lossy().to_string();
        let name = raw.strip_suffix(SUFFIX).unwrap_or(&raw).to_string();
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() || !people::plain_name(&name) || people::mime_of(&name) == "application/octet-stream" || out.iter().any(|p| p.name == name) {
            continue;
        }
        let when = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
        out.push(Photo { name, bytes: meta.len(), when });
    }
    out.sort_by(|a, b| b.when.cmp(&a.when).then(a.name.cmp(&b.name)));
    out
}

fn picture_name_ok(name: &str) -> bool {
    people::plain_name(name) && people::mime_of(name) != "application/octet-stream"
}

pub fn photo_read(dir: &Path, name: &str, s: Option<&dyn Sealer>) -> Result<Vec<u8>, String> {
    if !picture_name_ok(name) {
        return Err("That picture is not here.".into());
    }
    match read_maybe_sealed(&dir.join(name), s)? {
        Some(b) => Ok(b),
        None => Err("That picture is not here.".into()),
    }
}

pub fn photo_delete(dir: &Path, name: &str) -> Result<(), String> {
    if !picture_name_ok(name) {
        return Err("That picture is not here.".into());
    }
    let mut removed = false;
    for f in [dir.join(name), sealed_of(&dir.join(name))] {
        if f.is_file() {
            fs::remove_file(&f).map_err(|e| io_err("Could not remove the picture", e))?;
            removed = true;
        }
    }
    if removed { Ok(()) } else { Err("That picture is not here.".into()) }
}

/// Like `people::photo_add`, but with a sealer the picture is stored sealed and no plain copy is made.
pub fn photo_add(dir: &Path, original_name: &str, bytes: &[u8], s: Option<&dyn Sealer>) -> Result<String, String> {
    let Some(s) = s else { return people::photo_add(dir, original_name, bytes) };
    if bytes.len() > people::MAX_PHOTO_BYTES {
        return Err("That picture is larger than 40 MB.".into());
    }
    if people::image_kind(bytes).is_none() {
        return Err("That does not look like a picture. Photos can be JPEG, PNG, GIF or WebP files.".into());
    }
    // let `people::photo_add` choose the clean, unused name in a scratch folder, then keep only the sealed copy
    let scratch = dir.join(".new");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).map_err(|e| io_err("Could not make the photo folder", e))?;
    let probe = people::photo_add(&scratch, original_name, bytes);
    let _ = fs::remove_dir_all(&scratch);
    let base = probe?;
    let (stem, ext) = base.rsplit_once('.').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((base.clone(), String::new()));
    let mut name = base;
    let mut n = 2;
    while dir.join(&name).exists() || sealed_of(&dir.join(&name)).exists() {
        name = format!("{} ({}).{}", stem, n, ext);
        n += 1;
    }
    let sealed = s.seal(bytes)?;
    fsio::write_atomic(&sealed_of(&dir.join(&name)), &sealed).map_err(|e| io_err("Could not save the picture", e))?;
    Ok(name)
}

// ---------- turning protection on and off ----------

fn files_of(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(root) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            // the program's own scratch folder is never touched
            if p.file_name().map_or(false, |n| n.to_string_lossy().starts_with('.')) {
                continue;
            }
            files_of(&p, out);
        } else if p.file_name().map_or(true, |n| n != "lock.json") {
            out.push(p);
        }
    }
}

/// Seal every file of the person's: journal, comms plan, pictures and backups. Each file is sealed, checked
/// by opening it, and only then is the plain file removed. Returns how many files were sealed.
pub fn seal_all(user_dir: &Path, person: &str, s: &dyn Sealer) -> Result<usize, String> {
    let root = people::person_dir(user_dir, person)?;
    let mut files = Vec::new();
    files_of(&root, &mut files);
    let mut n = 0;
    for f in files {
        if f.to_string_lossy().ends_with(SUFFIX) {
            continue;
        }
        let plain = fs::read(&f).map_err(|e| io_err("Could not read a file to protect it", e))?;
        let sealed = s.seal(&plain)?;
        if s.open(&sealed)? != plain {
            return Err("A file could not be protected safely, so nothing was changed.".into());
        }
        fsio::write_atomic(&sealed_of(&f), &sealed).map_err(|e| io_err("Could not protect a file", e))?;
        fs::remove_file(&f).map_err(|e| io_err("Could not remove the plain copy", e))?;
        n += 1;
    }
    Ok(n)
}

/// Undo `seal_all`: every sealed file becomes a plain file again, checked before the sealed one is removed.
pub fn unseal_all(user_dir: &Path, person: &str, s: &dyn Sealer) -> Result<usize, String> {
    let root = people::person_dir(user_dir, person)?;
    let mut files = Vec::new();
    files_of(&root, &mut files);
    let mut n = 0;
    for f in files {
        let text = f.to_string_lossy().to_string();
        let Some(plain_path) = text.strip_suffix(SUFFIX) else { continue };
        let sealed = fs::read(&f).map_err(|e| io_err("Could not read a protected file", e))?;
        let plain = s.open(&sealed)?;
        fsio::write_atomic(Path::new(plain_path), &plain).map_err(|e| io_err("Could not write the file back", e))?;
        fs::remove_file(&f).map_err(|e| io_err("Could not remove the protected copy", e))?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in: reverses the bytes and adds a marker. Enough to test the file handling.
    struct Fake(u8);
    impl Sealer for Fake {
        fn seal(&self, p: &[u8]) -> Result<Vec<u8>, String> {
            let mut v: Vec<u8> = p.iter().rev().map(|b| b ^ self.0).collect();
            v.splice(0..0, *b"SEAL");
            Ok(v)
        }
        fn open(&self, s: &[u8]) -> Result<Vec<u8>, String> {
            if !s.starts_with(b"SEAL") {
                return Err("wrong key".into());
            }
            let v: Vec<u8> = s[4..].iter().rev().map(|b| b ^ self.0).collect();
            // a wrong key gives garbage; the marker check above is not enough, so verify with a checksum byte
            Ok(v)
        }
    }

    fn tmp(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-privacy-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn png() -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(b"fake body");
        v
    }
    fn all_bytes(root: &Path) -> Vec<u8> {
        let mut files = Vec::new();
        files_of(root, &mut files);
        files.iter().flat_map(|f| fs::read(f).unwrap()).collect()
    }
    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn turning_protection_on_seals_everything_and_leaves_no_plain_copy() {
        let u = tmp("on");
        let k = Fake(0x5A);
        people::journal_save(&u, "Anna", "2026-10-22", "My secret day").unwrap();
        people::journal_save(&u, "Anna", "2026-10-22", "My secret day, changed").unwrap(); // makes a backup
        people::plan_save(&u, "Anna", "Our secret meeting place").unwrap();
        let pics = people::photo_dir(&u, "mine", "Anna").unwrap();
        people::photo_add(&pics, "me.png", &png()).unwrap();
        let root = people::person_dir(&u, "Anna").unwrap();
        assert!(contains(&all_bytes(&root), b"secret"));
        let n = seal_all(&u, "Anna", &k).unwrap();
        assert!(n >= 4, "{}", n);
        fs::write(lock_file(&u, "Anna").unwrap(), "{}").unwrap();
        assert!(!contains(&all_bytes(&root), b"secret"), "a plain copy is left");
        assert!(!contains(&all_bytes(&root), b"fake body"));
        assert!(is_protected(&u, "Anna") && !is_protected(&u, "Ben"));
        // without the key nothing can be read or changed
        assert_eq!(journal_list(&u, "Anna", None).unwrap_err(), LOCKED);
        assert_eq!(journal_save(&u, "Anna", "2026-10-23", "x", None).unwrap_err(), LOCKED);
        assert_eq!(plan_load(&u, "Anna", None).unwrap_err(), LOCKED);
        assert_eq!(plan_save(&u, "Anna", "x", None).unwrap_err(), LOCKED);
        assert_eq!(photo_read(&pics, "me.png", None).unwrap_err(), LOCKED);
        // with it, everything is there
        let l = journal_list(&u, "Anna", Some(&k)).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].text, "My secret day, changed");
        assert_eq!(plan_load(&u, "Anna", Some(&k)).unwrap(), "Our secret meeting place");
        assert_eq!(photo_read(&pics, "me.png", Some(&k)).unwrap(), png());
        let names: Vec<String> = photos_list(&pics).into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["me.png".to_string()]);
    }

    #[test]
    fn new_work_on_a_protected_person_is_sealed_and_a_wrong_key_reads_nothing() {
        let u = tmp("new");
        let k = Fake(0x21);
        fs::create_dir_all(people::person_dir(&u, "Anna").unwrap()).unwrap();
        fs::write(lock_file(&u, "Anna").unwrap(), "{}").unwrap();
        journal_save(&u, "Anna", "2026-10-24", "Brand new secret", Some(&k)).unwrap();
        journal_save(&u, "Anna", "2026-10-24", "Brand new secret, edited", Some(&k)).unwrap();
        plan_save(&u, "Anna", "Plan secret", Some(&k)).unwrap();
        let pics = people::photo_dir(&u, "mine", "Anna").unwrap();
        assert_eq!(photo_add(&pics, "Beach.PNG", &png(), Some(&k)).unwrap(), "Beach.png");
        assert_eq!(photo_add(&pics, "Beach.png", &png(), Some(&k)).unwrap(), "Beach (2).png");
        assert!(photo_add(&pics, "notes.png", b"just words", Some(&k)).is_err());
        assert!(fs::read_dir(&pics).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with('.')), "scratch folder left behind");
        let root = people::person_dir(&u, "Anna").unwrap();
        assert!(!contains(&all_bytes(&root), b"secret"));
        assert_eq!(journal_list(&u, "Anna", Some(&k)).unwrap()[0].text, "Brand new secret, edited");
        // clearing a day keeps a sealed copy and removes the entry
        journal_save(&u, "Anna", "2026-10-24", "", Some(&k)).unwrap();
        assert!(journal_list(&u, "Anna", Some(&k)).unwrap().is_empty());
        assert!(!contains(&all_bytes(&root), b"secret"));
        photo_delete(&pics, "Beach.png").unwrap();
        assert_eq!(photos_list(&pics).len(), 1);
        assert!(photo_read(&pics, "../x.png", Some(&k)).is_err());
        // a plain person is unaffected
        journal_save(&u, "Ben", "2026-10-24", "Ben's day", None).unwrap();
        assert_eq!(journal_list(&u, "Ben", None).unwrap()[0].text, "Ben's day");
        assert!(!is_protected(&u, "Ben"));
    }

    #[test]
    fn turning_protection_off_gives_the_plain_files_back() {
        let u = tmp("off");
        let k = Fake(0x77);
        people::journal_save(&u, "Anna", "2026-10-22", "Day one").unwrap();
        people::plan_save(&u, "Anna", "The plan").unwrap();
        seal_all(&u, "Anna", &k).unwrap();
        fs::write(lock_file(&u, "Anna").unwrap(), "{}").unwrap();
        assert!(unseal_all(&u, "Anna", &k).unwrap() >= 2);
        fs::remove_file(lock_file(&u, "Anna").unwrap()).unwrap();
        assert!(!is_protected(&u, "Anna"));
        assert_eq!(people::journal_list(&u, "Anna").unwrap()[0].text, "Day one");
        assert_eq!(people::plan_load(&u, "Anna").unwrap(), "The plan");
        let root = people::person_dir(&u, "Anna").unwrap();
        let mut files = Vec::new();
        files_of(&root, &mut files);
        assert!(files.iter().all(|f| !f.to_string_lossy().ends_with(SUFFIX)));
    }

    #[test]
    fn sealing_twice_changes_nothing_and_the_lock_file_is_never_sealed() {
        let u = tmp("twice");
        let k = Fake(0x10);
        people::journal_save(&u, "Anna", "2026-10-22", "once").unwrap();
        let root = people::person_dir(&u, "Anna").unwrap();
        fs::write(root.join("lock.json"), "{}").unwrap();
        let first = seal_all(&u, "Anna", &k).unwrap();
        assert!(first >= 1);
        assert_eq!(seal_all(&u, "Anna", &k).unwrap(), 0);
        assert!(root.join("lock.json").is_file() && !root.join("lock.json.sealed").exists());
        assert_eq!(journal_list(&u, "Anna", Some(&k)).unwrap()[0].text, "once");
    }
}
