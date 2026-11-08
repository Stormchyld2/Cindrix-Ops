//! Security Operations, part 3: where the files live and how they are read and written.
//! Everything is plain text under `user/security/`, so the records outlive the program:
//!
//! ```text
//! user/security/sites.txt              the sites (rewritten safely, backup first)
//! user/security/posture-log.txt        posture changes, only ever added to
//! user/security/watchbill.txt          watches for every site (rewritten safely, backup first)
//! user/security/backups/               a copy of each file before it is replaced
//! ```
//!
//! A file that cannot be read is never replaced by an empty one: a read that fails returns the plain-words
//! reason, and a save always keeps a copy of what was there. Lines that cannot be used are handed back so a
//! screen can show them; they stay in the file until a person decides.

use crate::fsio;
use crate::security::{self, Parsed, PostureEntry, Site};
use crate::security_schedule::{self, Watch};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn dir(user_root: &Path) -> PathBuf {
    user_root.join("security")
}
fn backups(user_root: &Path) -> PathBuf {
    dir(user_root).join("backups")
}
pub fn sites_path(user_root: &Path) -> PathBuf {
    dir(user_root).join("sites.txt")
}
pub fn posture_path(user_root: &Path) -> PathBuf {
    dir(user_root).join("posture-log.txt")
}
pub fn watch_path(user_root: &Path) -> PathBuf {
    dir(user_root).join("watchbill.txt")
}

/// The text of a file, or empty when it does not exist yet. A file that exists but cannot be read is an error,
/// never "empty", so nothing is overwritten by mistake.
fn read(path: &Path) -> Result<String, String> {
    match fs::read(path) {
        Ok(b) => String::from_utf8(b).map_err(|_| "A security file is damaged (it is not plain text). It was left alone.".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(_) => Err("A security file could not be opened. It was left alone.".to_string()),
    }
}

fn save(user_root: &Path, path: &Path, text: &str) -> Result<(), String> {
    fsio::backup_then_write(path, &backups(user_root), text.as_bytes()).map_err(|_| "The change could not be saved. Nothing was changed.".to_string())
}

pub fn load_sites(user_root: &Path) -> Result<Parsed<Site>, String> {
    security::parse_sites(&read(&sites_path(user_root))?)
}

/// Save the site list. Refuses a list with a missing or repeated id, so a bad edit cannot lose a site's history.
pub fn save_sites(user_root: &Path, sites: &[Site]) -> Result<(), String> {
    for (i, s) in sites.iter().enumerate() {
        if !security::valid_id(&s.id) || s.name.trim().is_empty() || sites[..i].iter().any(|o| o.id == s.id) {
            return Err("Every site needs its own name and a short code of letters, numbers and dashes.".to_string());
        }
    }
    save(user_root, &sites_path(user_root), &security::serialize_sites(sites))
}

pub fn load_posture_log(user_root: &Path) -> Result<Parsed<PostureEntry>, String> {
    security::parse_posture_log(&read(&posture_path(user_root))?)
}

/// Add one posture change to the end of the log. The entry is checked first. Earlier lines are never touched.
pub fn append_posture(user_root: &Path, entry: &PostureEntry) -> Result<(), String> {
    let sites = load_sites(user_root)?.items;
    if let Some(problem) = security::check_new_entry(&sites, entry) {
        return Err(problem);
    }
    let path = posture_path(user_root);
    // read first so a newer-format or damaged file stops us before we add anything
    let existing = read(&path)?;
    security::parse_posture_log(&existing)?;
    fs::create_dir_all(dir(user_root)).map_err(|_| "The security folder could not be made.".to_string())?;
    let mut f = OpenOptions::new().create(true).append(true).open(&path).map_err(|_| "The posture log could not be opened.".to_string())?;
    let mut out = String::new();
    if existing.is_empty() {
        out.push_str(&security::posture_log_header());
    } else if !existing.ends_with('\n') {
        // an interrupted write left a half line: end it, so our line starts clean (the half line is kept and reported)
        out.push('\n');
    }
    out.push_str(&security::posture_line(entry));
    f.write_all(out.as_bytes()).and_then(|_| f.sync_all()).map_err(|_| "The posture change could not be saved.".to_string())
}

pub fn load_watches(user_root: &Path) -> Result<Parsed<Watch>, String> {
    security_schedule::parse_watches(&read(&watch_path(user_root))?)
}

/// Save the whole watchbill. Refuses to drop lines the reader had rejected: pass them back with `keep_rejected`
/// from a fresh load, so a bad line is never lost without a person choosing it.
pub fn save_watches(user_root: &Path, watches: &[Watch], keep_rejected: &[String]) -> Result<(), String> {
    let mut text = security_schedule::serialize_watches(watches);
    for l in keep_rejected {
        text.push_str(l);
        text.push('\n');
    }
    save(user_root, &watch_path(user_root), &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::Posture;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-sec-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn site(id: &str) -> Site {
        Site { id: id.into(), name: id.to_uppercase(), ..Default::default() }
    }
    fn entry(site: &str, p: Posture, at: &str) -> PostureEntry {
        PostureEntry { site: site.into(), posture: p, set_by: "Dad".into(), reason: "why".into(), source: "operator".into(), at: at.into(), verified_at: String::new(), review_by: String::new() }
    }

    #[test]
    fn nothing_on_disk_means_empty_lists() {
        let r = tmp("empty");
        assert!(load_sites(&r).unwrap().items.is_empty());
        assert!(load_posture_log(&r).unwrap().items.is_empty());
        assert!(load_watches(&r).unwrap().items.is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn sites_save_with_a_backup_of_the_old_file() {
        let r = tmp("sites");
        save_sites(&r, &[site("home")]).unwrap();
        save_sites(&r, &[site("home"), site("camp")]).unwrap();
        assert_eq!(load_sites(&r).unwrap().items.len(), 2);
        assert_eq!(fs::read_dir(backups(&r)).unwrap().count(), 1);
        // a bad list is refused and the file is unchanged
        assert!(save_sites(&r, &[site("home"), site("home")]).is_err());
        assert!(save_sites(&r, &[site("Bad Id")]).is_err());
        assert_eq!(load_sites(&r).unwrap().items.len(), 2);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn posture_log_only_grows() {
        let r = tmp("posture");
        save_sites(&r, &[site("home")]).unwrap();
        append_posture(&r, &entry("home", Posture::Normal, "2026-11-08T08:00-05:00")).unwrap();
        let first = fs::read_to_string(posture_path(&r)).unwrap();
        append_posture(&r, &entry("home", Posture::Elevated, "2026-11-08T09:00-05:00")).unwrap();
        let second = fs::read_to_string(posture_path(&r)).unwrap();
        assert!(second.starts_with(&first));
        assert_eq!(load_posture_log(&r).unwrap().items.len(), 2);
        // header written once
        assert_eq!(second.matches("# format:").count(), 1);
        // unknown site and bad time are refused and add nothing
        assert!(append_posture(&r, &entry("nowhere", Posture::Normal, "2026-11-08T10:00-05:00")).is_err());
        assert!(append_posture(&r, &entry("home", Posture::Normal, "noon")).is_err());
        assert_eq!(fs::read_to_string(posture_path(&r)).unwrap(), second);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn an_interrupted_write_is_reported_not_repaired_away() {
        let r = tmp("torn");
        save_sites(&r, &[site("home")]).unwrap();
        append_posture(&r, &entry("home", Posture::Normal, "2026-11-08T08:00-05:00")).unwrap();
        // simulate power loss halfway through a line
        let mut f = OpenOptions::new().append(true).open(posture_path(&r)).unwrap();
        f.write_all(b"home\televated\tDa").unwrap();
        drop(f);
        append_posture(&r, &entry("home", Posture::Elevated, "2026-11-08T09:00-05:00")).unwrap();
        let p = load_posture_log(&r).unwrap();
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.rejected.len(), 1);
        assert!(fs::read_to_string(posture_path(&r)).unwrap().contains("home\televated\tDa\n"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn damaged_or_newer_files_stop_every_write() {
        let r = tmp("damaged");
        save_sites(&r, &[site("home")]).unwrap();
        fs::write(posture_path(&r), "# format: security-posture 7\n").unwrap();
        assert!(append_posture(&r, &entry("home", Posture::Normal, "2026-11-08T08:00-05:00")).is_err());
        assert_eq!(fs::read_to_string(posture_path(&r)).unwrap(), "# format: security-posture 7\n");
        fs::write(sites_path(&r), [0xff, 0xfe, 0x00]).unwrap();
        assert!(load_sites(&r).is_err());
        assert!(append_posture(&r, &entry("home", Posture::Normal, "2026-11-08T08:00-05:00")).is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn watchbill_keeps_lines_it_could_not_read() {
        let r = tmp("watch");
        let good = "w1\thome\tDad\twatch\t2026-11-08T00:00Z\t2026-11-08T06:00Z\tscheduled\t\t\t";
        let bad = "w2\thome\tMom\twatch\tnot a time\t2026-11-08T06:00Z\tscheduled\t\t\t";
        fs::create_dir_all(dir(&r)).unwrap();
        fs::write(watch_path(&r), format!("{}\n{}\n", good, bad)).unwrap();
        let p = load_watches(&r).unwrap();
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.rejected, vec![bad.to_string()]);
        save_watches(&r, &p.items, &p.rejected).unwrap();
        let after = fs::read_to_string(watch_path(&r)).unwrap();
        assert!(after.contains(bad) && after.contains("w1"));
        assert_eq!(fs::read_dir(backups(&r)).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&r);
    }
}
