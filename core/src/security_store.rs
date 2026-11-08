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
use crate::security_incident as incident;
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
    append_line(user_root, &posture_path(user_root), &security::posture_log_header(), &security::posture_line(entry), |t| security::parse_posture_log(t).map(|_| ()))
}

/// Add one line to the end of a log file. `check` reads what is already there first, so a damaged or newer file
/// stops the write before anything is added. A half line left by a power cut is ended, kept, and reported by the
/// reader; it is never removed.
fn append_line(user_root: &Path, path: &Path, header: &str, line: &str, check: impl Fn(&str) -> Result<(), String>) -> Result<(), String> {
    let existing = read(path)?;
    check(&existing)?;
    fs::create_dir_all(dir(user_root)).map_err(|_| "The security folder could not be made.".to_string())?;
    let mut f = OpenOptions::new().create(true).append(true).open(path).map_err(|_| "The log could not be opened.".to_string())?;
    let mut out = String::new();
    if existing.is_empty() {
        out.push_str(header);
    } else if !existing.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(line);
    f.write_all(out.as_bytes()).and_then(|_| f.sync_all()).map_err(|_| "The change could not be saved.".to_string())
}

pub fn incident_path(user_root: &Path) -> PathBuf {
    dir(user_root).join("incident-log.txt")
}
/// Where the exact text of a plan is kept when a person activates it during an incident.
pub fn plan_copy_path(user_root: &Path, incident: &str, entry: &str) -> PathBuf {
    dir(user_root).join("incident-plans").join(format!("{}-{}.md", incident, entry))
}

pub fn load_incident_log(user_root: &Path) -> Result<Parsed<incident::Entry>, String> {
    incident::parse(&read(&incident_path(user_root))?)
}

/// Add one line to the incident log after checking it against everything already there.
pub fn append_incident(user_root: &Path, entry: &incident::Entry) -> Result<(), String> {
    let sites: Vec<String> = load_sites(user_root)?.items.into_iter().map(|s| s.id).collect();
    let log = load_incident_log(user_root)?.items;
    if let Some(problem) = incident::check_entry(&log, &sites, entry) {
        return Err(problem);
    }
    append_line(user_root, &incident_path(user_root), &incident::header(), &incident::line(entry), |t| incident::parse(t).map(|_| ()))
}

/// A person activates an Operational Response plan during an incident. The plan's exact text is saved first
/// next to the log (never overwritten), then the log line that points at it is added. If the line is refused,
/// the copy is removed again so nothing is left half done.
pub fn activate_plan(user_root: &Path, mut entry: incident::Entry, plan_text: &str) -> Result<(), String> {
    if entry.kind != incident::Kind::Plan {
        return Err("That is not a plan activation.".to_string());
    }
    entry.arg3 = incident::fingerprint(plan_text);
    let copy = plan_copy_path(user_root, &entry.incident, &entry.id);
    if !security::valid_id(&entry.incident) || !security::valid_id(&entry.id) {
        return Err("That entry number is not valid.".to_string());
    }
    if copy.exists() {
        return Err("A saved copy with that number already exists. Nothing was changed.".to_string());
    }
    // check before writing anything
    let sites: Vec<String> = load_sites(user_root)?.items.into_iter().map(|s| s.id).collect();
    let log = load_incident_log(user_root)?.items;
    if let Some(problem) = incident::check_entry(&log, &sites, &entry) {
        return Err(problem);
    }
    fsio::write_atomic(&copy, plan_text.as_bytes()).map_err(|_| "The copy of the plan could not be saved. Nothing was changed.".to_string())?;
    if let Err(e) = append_incident(user_root, &entry) {
        let _ = fs::remove_file(&copy);
        return Err(e);
    }
    Ok(())
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

    fn inc_entry(id: &str, inc: &str, kind: incident::Kind, arg1: &str, text: &str) -> incident::Entry {
        let at = "2026-11-08T10:00-05:00".to_string();
        incident::Entry { id: id.into(), incident: inc.into(), kind, happened_at: at.clone(), recorded_at: at, by: "Dad".into(), source: incident::Source::Observed, arg1: arg1.into(), arg2: String::new(), arg3: String::new(), text: text.into(), supersedes: String::new() }
    }

    #[test]
    fn incident_log_grows_and_refuses_bad_lines() {
        let r = tmp("incident");
        save_sites(&r, &[site("home")]).unwrap();
        let mut open = inc_entry("e1", "i1", incident::Kind::Open, "home", "Car at the gate");
        open.arg2 = "2".into();
        open.arg3 = "live".into();
        append_incident(&r, &open).unwrap();
        let first = fs::read_to_string(incident_path(&r)).unwrap();
        append_incident(&r, &inc_entry("e2", "i1", incident::Kind::Note, "", "Two men")).unwrap();
        assert!(fs::read_to_string(incident_path(&r)).unwrap().starts_with(&first));
        let before = fs::read_to_string(incident_path(&r)).unwrap();
        // refused: unknown incident, reused number, closing without a reason
        assert!(append_incident(&r, &inc_entry("e3", "i9", incident::Kind::Note, "", "x")).is_err());
        assert!(append_incident(&r, &inc_entry("e2", "i1", incident::Kind::Note, "", "x")).is_err());
        assert!(append_incident(&r, &inc_entry("e3", "i1", incident::Kind::Status, "closed", "")).is_err());
        assert_eq!(fs::read_to_string(incident_path(&r)).unwrap(), before);
        assert_eq!(incident::build(&load_incident_log(&r).unwrap().items).len(), 1);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn plan_activation_keeps_a_copy_of_the_exact_text() {
        let r = tmp("plan");
        save_sites(&r, &[site("home")]).unwrap();
        let mut open = inc_entry("e1", "i1", incident::Kind::Open, "home", "Smoke smell");
        open.arg2 = "1".into();
        open.arg3 = "live".into();
        append_incident(&r, &open).unwrap();
        let mut plan = inc_entry("e2", "i1", incident::Kind::Plan, "evacuate", "");
        plan.arg2 = "4".into();
        // not allowed while the incident is only a draft: no copy is left behind
        assert!(activate_plan(&r, plan.clone(), "Step 1: go to the car").is_err());
        assert!(!plan_copy_path(&r, "i1", "e2").exists());
        append_incident(&r, &inc_entry("e3", "i1", incident::Kind::Status, "active", "confirmed")).unwrap();
        plan.id = "e4".into();
        activate_plan(&r, plan.clone(), "Step 1: go to the car").unwrap();
        let copy = fs::read_to_string(plan_copy_path(&r, "i1", "e4")).unwrap();
        assert_eq!(copy, "Step 1: go to the car");
        let log = load_incident_log(&r).unwrap().items;
        let pinned = &incident::build(&log)[0].plans[0];
        assert_eq!(pinned.fingerprint, incident::fingerprint(&copy));
        // the same entry number cannot overwrite the saved copy
        assert!(activate_plan(&r, plan, "Step 1: something else").is_err());
        assert_eq!(fs::read_to_string(plan_copy_path(&r, "i1", "e4")).unwrap(), "Step 1: go to the car");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn damaged_incident_log_blocks_new_lines() {
        let r = tmp("badinc");
        save_sites(&r, &[site("home")]).unwrap();
        fs::write(incident_path(&r), "# format: security-incidents 9\n").unwrap();
        let mut open = inc_entry("e1", "i1", incident::Kind::Open, "home", "x");
        open.arg2 = "2".into();
        open.arg3 = "live".into();
        assert!(append_incident(&r, &open).is_err());
        assert_eq!(fs::read_to_string(incident_path(&r)).unwrap(), "# format: security-incidents 9\n");
        let _ = fs::remove_dir_all(&r);
    }
}
