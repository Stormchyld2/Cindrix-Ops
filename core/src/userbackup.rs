//! A copy of the person's own files (`user/`: Family page, inventory, security records, plans, tasks and the rest) on
//! another drive or folder, so a dead or lost drive does not take them all.
//!
//! The copy is an ordinary folder of ordinary files, `user-backup-YYYY-MM-DD-HHMM`, with a `BACKUP-LIST.txt` that holds
//! each file's size and a checksum. After copying, every file is read back and compared, and the backup is only called
//! good if all match. Nothing here deletes or overwrites anything: a backup never replaces an earlier one, and "bring
//! back missing files" only adds files that are not there now. The checksum (FNV-1a, 64 bit) finds damage; it is not
//! a lock. Locked files stay locked in the copy, and an unlocked file stays readable, so keep the copy somewhere as safe
//! as the drive itself.

use crate::fsio;
use std::fs;
use std::path::{Path, PathBuf};

pub const LIST_NAME: &str = "BACKUP-LIST.txt";
const PREFIX: &str = "user-backup-";
const MAX_FILES: usize = 200_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub folder: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub skipped: Vec<String>,
}

pub fn fnv(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>, skipped: &mut Vec<String>) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|_| "A folder could not be read.".to_string())?;
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
        let meta = match fs::symlink_metadata(&p) { Ok(m) => m, Err(_) => { skipped.push(rel); continue } };
        if meta.file_type().is_symlink() { skipped.push(rel); }
        else if meta.is_dir() { walk(root, &p, out, skipped)?; }
        else if meta.is_file() { out.push(p); if out.len() > MAX_FILES { return Err("There are far more files than expected, so nothing was copied.".to_string()); } }
    }
    Ok(())
}

/// `stamp` is "YYYY-MM-DD-HHMM". The destination must be outside the user folder.
pub fn backup(user_root: &Path, dest_parent: &Path, stamp: &str) -> Result<Report, String> {
    if !user_root.is_dir() { return Err("There is no folder of your own files to copy yet.".to_string()); }
    if stamp.len() != 15 || !stamp.chars().all(|c| c.is_ascii_digit() || c == '-') { return Err("The time stamp is not valid.".to_string()); }
    let (u, d) = (user_root.canonicalize().map_err(|_| "Your files folder could not be found.".to_string())?, dest_parent.canonicalize().map_err(|_| "That place to keep the copy could not be found.".to_string())?);
    if d.starts_with(&u) { return Err("Pick a place outside your files folder, ideally on another drive.".to_string()); }
    let folder = d.join(format!("{}{}", PREFIX, stamp));
    if folder.exists() { return Err("A copy with this time already exists. Nothing was changed.".to_string()); }
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    walk(&u, &u, &mut files, &mut skipped)?;
    let mut list = String::from("# format: user-backup 1\n# size, checksum (FNV-1a 64), path\n");
    let mut bytes = 0u64;
    for f in &files {
        let rel = f.strip_prefix(&u).unwrap().to_string_lossy().replace('\\', "/");
        let data = fs::read(f).map_err(|_| format!("{} could not be read, so the copy was stopped.", rel))?;
        let to = folder.join(&rel);
        fs::create_dir_all(to.parent().unwrap()).map_err(|_| "The copy's folder could not be made.".to_string())?;
        fsio::write_atomic(&to, &data).map_err(|_| format!("{} could not be written to the copy.", rel))?;
        bytes += data.len() as u64;
        list.push_str(&format!("{}\t{:016x}\t{}\n", data.len(), fnv(&data), rel));
    }
    fs::create_dir_all(&folder).map_err(|_| "The copy's folder could not be made.".to_string())?;
    fsio::write_atomic(&folder.join(LIST_NAME), list.as_bytes()).map_err(|_| "The list for the copy could not be written.".to_string())?;
    let bad = verify(&folder)?;
    if !bad.is_empty() { return Err(format!("The copy was made but {} file{} did not read back the same, so do not trust it: {}.", bad.len(), if bad.len() == 1 { "" } else { "s" }, bad.join(", "))); }
    Ok(Report { folder, files: files.len(), bytes, skipped })
}

/// Read every file back. Returns the files that are missing or differ (empty = good).
pub fn verify(folder: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(folder.join(LIST_NAME)).map_err(|_| "This folder has no backup list, so it cannot be checked.".to_string())?;
    if !text.starts_with("# format: user-backup 1") { return Err("The backup list is from a different version, so it was left alone.".to_string()); }
    let mut bad = Vec::new();
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = l.splitn(3, '\t').collect();
        if f.len() != 3 { bad.push("(a line of the list)".to_string()); continue; }
        let ok = fs::read(folder.join(f[2])).map(|d| d.len().to_string() == f[0] && format!("{:016x}", fnv(&d)) == f[1]).unwrap_or(false);
        if !ok { bad.push(f[2].to_string()); }
    }
    Ok(bad)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub name: String,
    /// "YYYY-MM-DD-HHMM"
    pub stamp: String,
    pub files: usize,
}
/// Backups in a place, newest first.
pub fn list(dest_parent: &Path) -> Vec<Found> {
    let mut v: Vec<Found> = fs::read_dir(dest_parent).into_iter().flatten().flatten().filter_map(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        let stamp = name.strip_prefix(PREFIX)?.to_string();
        let text = fs::read_to_string(e.path().join(LIST_NAME)).ok()?;
        (stamp.len() == 15).then(|| Found { files: text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).count(), name, stamp })
    }).collect();
    v.sort_by(|a, b| b.stamp.cmp(&a.stamp));
    v
}

/// Days since a stamp, given today's stamp-style date parts. Both are "YYYY-MM-DD..." strings.
pub fn days_between(older: &str, newer: &str) -> Option<i64> {
    fn day(s: &str) -> Option<i64> {
        let y: i64 = s.get(0..4)?.parse().ok()?;
        let m: i64 = s.get(5..7)?.parse().ok()?;
        let d: i64 = s.get(8..10)?.parse().ok()?;
        let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        Some(365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 + (m2 * 306 + 5) / 10 + d)
    }
    Some(day(newer)? - day(older)?)
}

/// Copy back only the files that are missing from the user folder. Files that are there, even if different, are left
/// alone and counted. Refuses a backup that does not check out.
pub fn bring_back_missing(backup_folder: &Path, user_root: &Path) -> Result<(usize, usize), String> {
    let bad = verify(backup_folder)?;
    if !bad.is_empty() { return Err(format!("This backup has {} file{} that do not read back the same, so nothing was copied.", bad.len(), if bad.len() == 1 { "" } else { "s" })); }
    let text = fs::read_to_string(backup_folder.join(LIST_NAME)).map_err(|_| "The backup list could not be read.".to_string())?;
    let (mut added, mut differ) = (0, 0);
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let rel = l.splitn(3, '\t').nth(2).unwrap_or("");
        if rel.split('/').any(|p| p == ".." || p.is_empty()) || rel.starts_with('/') || rel.contains(':') { return Err("The backup list has a path that is not allowed.".to_string()); }
        let to = user_root.join(rel);
        let data = fs::read(backup_folder.join(rel)).map_err(|_| "A file in the backup could not be read.".to_string())?;
        match fs::read(&to) {
            Ok(now) => { if now != data { differ += 1 } }
            Err(_) => {
                fs::create_dir_all(to.parent().unwrap()).map_err(|_| "A folder could not be made.".to_string())?;
                fsio::write_atomic(&to, &data).map_err(|_| "A file could not be put back.".to_string())?;
                added += 1;
            }
        }
    }
    Ok((added, differ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(n: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("cindrix-ub-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let (u, d) = (base.join("user"), base.join("elsewhere"));
        fs::create_dir_all(u.join("security")).unwrap();
        fs::create_dir_all(&d).unwrap();
        fs::write(u.join("tasks.txt"), "buy water\n").unwrap();
        fs::write(u.join("security/incident-log.txt"), "# format: incident-log 1\nx\n").unwrap();
        (u, d)
    }

    #[test]
    fn a_backup_copies_everything_checks_it_and_never_replaces_an_earlier_one() {
        let (u, d) = setup("a");
        let r = backup(&u, &d, "2026-11-08-2100").unwrap();
        assert_eq!(r.files, 2);
        assert_eq!(fs::read_to_string(r.folder.join("security/incident-log.txt")).unwrap(), "# format: incident-log 1\nx\n");
        assert!(verify(&r.folder).unwrap().is_empty());
        assert!(backup(&u, &d, "2026-11-08-2100").unwrap_err().contains("already exists"));
        backup(&u, &d, "2026-11-09-0800").unwrap();
        let l = list(&d);
        assert_eq!((l.len(), l[0].stamp.as_str(), l[0].files), (2, "2026-11-09-0800", 2));
        let _ = fs::remove_dir_all(u.parent().unwrap());
    }

    #[test]
    fn a_backup_inside_the_user_folder_or_with_a_bad_stamp_is_refused() {
        let (u, _d) = setup("b");
        assert!(backup(&u, &u.join("security"), "2026-11-08-2100").unwrap_err().contains("outside"));
        assert!(backup(&u, u.parent().unwrap(), "../../x").is_err());
        assert!(backup(&u.join("nope"), u.parent().unwrap(), "2026-11-08-2100").is_err());
        let _ = fs::remove_dir_all(u.parent().unwrap());
    }

    #[test]
    fn damage_is_found_and_a_damaged_backup_is_never_used() {
        let (u, d) = setup("c");
        let r = backup(&u, &d, "2026-11-08-2100").unwrap();
        fs::write(r.folder.join("tasks.txt"), "buy water!\n").unwrap();
        assert_eq!(verify(&r.folder).unwrap(), vec!["tasks.txt".to_string()]);
        assert!(bring_back_missing(&r.folder, &u).unwrap_err().contains("do not read back"));
        fs::remove_file(r.folder.join(LIST_NAME)).unwrap();
        assert!(verify(&r.folder).is_err());
        let _ = fs::remove_dir_all(u.parent().unwrap());
    }

    #[test]
    fn bringing_back_adds_only_missing_files_and_leaves_changed_ones_alone() {
        let (u, d) = setup("d");
        let r = backup(&u, &d, "2026-11-08-2100").unwrap();
        fs::remove_file(u.join("security/incident-log.txt")).unwrap();
        fs::write(u.join("tasks.txt"), "buy water and rice\n").unwrap();
        let (added, differ) = bring_back_missing(&r.folder, &u).unwrap();
        assert_eq!((added, differ), (1, 1));
        assert!(u.join("security/incident-log.txt").exists());
        assert_eq!(fs::read_to_string(u.join("tasks.txt")).unwrap(), "buy water and rice\n", "newer work is not overwritten");
        // a list with a path that climbs out is refused
        let mut t = fs::read_to_string(r.folder.join(LIST_NAME)).unwrap();
        t.push_str("1\t0000000000000000\t../evil.txt\n");
        fs::write(r.folder.join(LIST_NAME), t).unwrap();
        assert!(bring_back_missing(&r.folder, &u).is_err());
        let _ = fs::remove_dir_all(u.parent().unwrap());
    }

    #[test]
    fn days_between_counts_calendar_days() {
        assert_eq!(days_between("2026-11-08-2100", "2026-11-09-0800"), Some(1));
        assert_eq!(days_between("2026-02-27-0000", "2026-03-02-0000"), Some(3));
        assert_eq!(days_between("2025-12-31-0000", "2026-01-01-0000"), Some(1));
        assert_eq!(days_between("bad", "2026-01-01"), None);
    }
}
