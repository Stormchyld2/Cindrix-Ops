//! The Hub's mailbox for reports from the Cindrix Pocket companion (docs/45).
//!
//! Pocket writes nothing on the drive except logs. Reports are kept in an outbox on the phone and arrive here as
//! plain files in `user/inbox/pocket/` when the phone reaches the Hub. Nothing in that folder is trusted:
//!  * every file is checked (name, size, text, header, every line) and a bad one stays where it is, with the reason;
//!  * nothing goes into the records until the host accepts it;
//!  * an accepted entry is always marked **reported**, so it can add notes, decisions, tasks and a new incident but
//!    can never change an incident's status or activate a plan (doc 44 section 14);
//!  * an accepted file is moved to `done/`, never deleted, and its name and fingerprint go in a ledger the phone
//!    reads, so the phone knows it may clear its outbox. Accepting the same file twice changes nothing.
//!
//! A report is a text file:
//! ```text
//! # format: pocket-report 1
//! # device: <short device code>      lower-case letters, digits, dashes
//! # person: <name>
//! # created: 2026-11-08T14:00-05:00
//! <incident log lines, as in incident-log.txt; entry ids and new incident ids start with "<device>-">
//! ```

use crate::fsio;
use crate::md5::{hex, Md5};
use crate::security::{self, valid_id};
use crate::security_incident as incident;
use crate::security_store as store;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const MAX_FILE: u64 = 256 * 1024;
const MAX_FILES: usize = 200;
const MAX_ENTRIES: usize = 500;

pub fn inbox_dir(user_root: &Path) -> PathBuf {
    user_root.join("inbox").join("pocket")
}
pub(crate) fn done_dir(user_root: &Path) -> PathBuf {
    inbox_dir(user_root).join("done")
}
fn aside_dir(user_root: &Path) -> PathBuf {
    inbox_dir(user_root).join("set-aside")
}
/// One line per accepted file: name, fingerprint, device, entry count, when. Only ever added to.
pub fn ledger_path(user_root: &Path) -> PathBuf {
    store::dir(user_root).join("pocket-accepted.txt")
}

/// A file name the mailbox will touch: lower-case letters, digits and dashes, ending `.txt`. No folders, no dots
/// before the extension, nothing that can climb out of the mailbox.
pub fn safe_name(name: &str) -> bool {
    match name.strip_suffix(".txt") {
        Some(stem) => valid_id(stem) && stem.len() <= 80,
        None => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Waiting {
    pub name: String,
    pub size: u64,
    pub device: String,
    pub person: String,
    pub created: String,
    pub entries: usize,
    /// "report", "count-sheet" or "checkin-sheet".
    pub kind: String,
    /// For a count sheet: how many numbers in the Inventory moved since the phone looked (0 for a report).
    pub changed: usize,
    /// Empty when the file can be accepted; otherwise a plain-words reason it cannot.
    pub problem: String,
}

struct Report {
    device: String,
    person: String,
    created: String,
    entries: Vec<incident::Entry>,
    fingerprint: String,
}

pub(crate) fn fingerprint(data: &[u8]) -> String {
    let mut m = Md5::new();
    m.update(data);
    hex(&m.finish())
}

/// Read and check one report. The error is a sentence for the host.
fn read_report(path: &Path) -> Result<Report, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "The file could not be opened.".to_string())?;
    if !meta.is_file() {
        return Err("This is not an ordinary file, so it was left alone.".to_string());
    }
    if meta.len() > MAX_FILE {
        return Err("This file is much bigger than any report should be.".to_string());
    }
    let data = fsio::read(path).map_err(|_| "The file could not be read.".to_string())?;
    let text = String::from_utf8(data.clone()).map_err(|_| "This is not plain text.".to_string())?;
    let (mut format_ok, mut device, mut person, mut created) = (false, String::new(), String::new(), String::new());
    let mut body = String::new();
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("# format:") {
            format_ok = r.split_whitespace().collect::<Vec<_>>() == ["pocket-report", "1"];
            if !format_ok {
                return Err("This report is from a different version of Pocket, so it was left alone.".to_string());
            }
        } else if let Some(r) = l.strip_prefix("# device:") {
            device = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# person:") {
            person = r.trim().chars().take(80).collect();
        } else if let Some(r) = l.strip_prefix("# created:") {
            created = r.trim().to_string();
        } else if !l.starts_with('#') && !l.trim().is_empty() {
            body.push_str(l);
            body.push('\n');
        }
    }
    if !format_ok {
        return Err("This is not a Pocket report.".to_string());
    }
    if !valid_id(&device) || device.len() > 12 {
        return Err("The report does not name its phone properly.".to_string());
    }
    if person.trim().is_empty() {
        return Err("The report does not say who wrote it.".to_string());
    }
    if security::parse_instant(&created).is_none() {
        return Err("The report has no readable date.".to_string());
    }
    let parsed = incident::parse(&format!("{}{}", incident::header(), body))?;
    if !parsed.rejected.is_empty() {
        return Err(format!("{} line{} in the report could not be read.", parsed.rejected.len(), if parsed.rejected.len() == 1 { "" } else { "s" }));
    }
    if parsed.items.is_empty() {
        return Err("The report has nothing in it.".to_string());
    }
    if parsed.items.len() > MAX_ENTRIES {
        return Err("The report has far more lines than expected.".to_string());
    }
    let prefix = format!("{}-", device);
    let mut entries = Vec::new();
    for mut e in parsed.items {
        if !e.id.starts_with(&prefix) {
            return Err("An entry in the report does not belong to the phone that sent it.".to_string());
        }
        match e.kind {
            incident::Kind::Status | incident::Kind::Plan => return Err("This report tries to change an incident's status or activate a plan. Only the host can do that.".to_string()),
            incident::Kind::Open if !e.incident.starts_with(&prefix) => return Err("A new incident in the report does not belong to the phone that sent it.".to_string()),
            _ => {}
        }
        // whatever the phone claims, the host records it as "reported", written by the person named on the report
        e.source = incident::Source::Reported;
        e.by = person.clone();
        entries.push(e);
    }
    Ok(Report { device, person, created, entries, fingerprint: fingerprint(&data) })
}

/// Everything waiting in the mailbox, one row each, oldest name first.
pub fn waiting(user_root: &Path) -> Vec<Waiting> {
    let mut names: Vec<String> = match fs::read_dir(inbox_dir(user_root)) {
        Ok(rd) => rd.filter_map(|e| e.ok()).filter(|e| e.file_type().map(|t| !t.is_dir()).unwrap_or(false)).map(|e| e.file_name().to_string_lossy().to_string()).collect(),
        Err(_) => return Vec::new(),
    };
    names.sort();
    names.truncate(MAX_FILES);
    names
        .into_iter()
        .map(|name| {
            let path = inbox_dir(user_root).join(&name);
            let size = fs::symlink_metadata(&path).map(|m| m.len()).unwrap_or(0);
            if !safe_name(&name) {
                return Waiting { name, size, device: String::new(), person: String::new(), created: String::new(), entries: 0, kind: "report".into(), changed: 0, problem: "The file name is not one Pocket would make, so it was left alone.".to_string() };
            }
            if is_sheet_file(&path) {
                return match crate::countsheet::read(&path) {
                    Ok(sh) => {
                        let items = crate::userdata::UserDir::new(user_root).load_inventory().unwrap_or_default();
                        let (moved, missing) = crate::countsheet::tally(&items, &sh);
                        let problem = if missing > 0 { format!("{} item{} on this sheet {} no longer in the Inventory. Set it aside and count again.", missing, if missing == 1 { "" } else { "s" }, if missing == 1 { "is" } else { "are" }) } else { String::new() };
                        Waiting { name, size, device: sh.device, person: sh.person, created: sh.created, entries: sh.lines.len(), kind: "count-sheet".into(), changed: moved, problem }
                    }
                    Err(problem) => Waiting { name, size, device: String::new(), person: String::new(), created: String::new(), entries: 0, kind: "count-sheet".into(), changed: 0, problem },
                };
            }
            if is_checkin_file(&path) {
                return match crate::checkinsheet::read(&path) {
                    Ok(sh) => Waiting { name, size, device: sh.device, person: sh.person, created: sh.created, entries: sh.lines.len(), kind: "checkin-sheet".into(), changed: 0, problem: String::new() },
                    Err(problem) => Waiting { name, size, device: String::new(), person: String::new(), created: String::new(), entries: 0, kind: "checkin-sheet".into(), changed: 0, problem },
                };
            }
            match read_report(&path) {
                Ok(r) => Waiting { name, size, device: r.device, person: r.person, created: r.created, entries: r.entries.len(), kind: "report".into(), changed: 0, problem: String::new() },
                Err(problem) => Waiting { name, size, device: String::new(), person: String::new(), created: String::new(), entries: 0, kind: "report".into(), changed: 0, problem },
            }
        })
        .collect()
}

/// How many files are waiting (good or not): the number shown on Command.
pub fn waiting_count(user_root: &Path) -> usize {
    fs::read_dir(inbox_dir(user_root)).map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.file_type().map(|t| !t.is_dir()).unwrap_or(false)).count()).unwrap_or(0)
}

pub(crate) fn move_into(dir: &Path, from: &Path, name: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|_| "The folder for finished reports could not be made.".to_string())?;
    let mut to = dir.join(name);
    if to.exists() {
        let stem = name.trim_end_matches(".txt");
        to = dir.join(format!("{}-{}.txt", stem, security_now_stamp()));
    }
    fs::rename(from, &to).map_err(|_| "The report could not be moved after it was handled.".to_string())
}

fn security_now_stamp() -> i64 {
    crate::security_api::now_minutes()
}

/// The host accepts a report: every entry is checked against the records as they stand, all or none are added,
/// and the file moves to `done/`. A file already accepted before (same entries) is just filed away again.
/// Returns how many entries were added.
pub fn accept(user_root: &Path, name: &str) -> Result<usize, String> {
    if !safe_name(name) {
        return Err("That is not a report file name.".to_string());
    }
    let path = inbox_dir(user_root).join(name);
    let report = read_report(&path)?;
    let sites: Vec<String> = store::load_sites(user_root)?.items.into_iter().map(|s| s.id).collect();
    let log = store::load_incident_log(user_root)?;
    if !log.rejected.is_empty() {
        return Err("Some lines in the incident log could not be read, so nothing was added. Look at those first.".to_string());
    }
    // sort entries into "already there" and "new"; an id that exists but says something else is a clash
    let mut sim = log.items.clone();
    let mut fresh: Vec<incident::Entry> = Vec::new();
    for e in &report.entries {
        match sim.iter().find(|x| x.id == e.id) {
            Some(x) if x == e => continue,
            Some(_) => return Err("An entry number in this report is already used for something different. Nothing was added.".to_string()),
            None => {}
        }
        if let Some(problem) = incident::check_entry(&sim, &sites, e) {
            return Err(format!("{} Nothing was added.", problem));
        }
        sim.push(e.clone());
        fresh.push(e.clone());
    }
    for e in &fresh {
        store::append_incident(user_root, e)?;
    }
    // the ledger is added to before the file moves, so a crash in between leaves a file that simply accepts again
    fs::create_dir_all(store::dir(user_root)).map_err(|_| "The security folder could not be made.".to_string())?;
    let line = format!("{}\t{}\t{}\t{}\t{}\n", name, report.fingerprint, report.device, report.entries.len(), security::format_instant(crate::security_api::now_minutes(), 0));
    fsio::append(&ledger_path(user_root), line.as_bytes()).map_err(|_| "The ledger could not be written.".to_string())?;
    move_into(&done_dir(user_root), &path, name)?;
    Ok(fresh.len())
}

/// True when an ordinary small file starts as a count sheet. Reads only the top of the file.
fn is_sheet_file(path: &Path) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 256];
    let n = fs::File::open(path).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    crate::countsheet::is_sheet(&String::from_utf8_lossy(&buf[..n]))
}

fn is_checkin_file(path: &Path) -> bool {
    use std::io::Read;
    let mut buf = [0u8; 256];
    let n = fs::File::open(path).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    crate::checkinsheet::is_sheet(&String::from_utf8_lossy(&buf[..n]))
}

/// Accept whatever kind of file this is. A report adds its entries; a count sheet sets the counted numbers (and, if
/// `allow_moved`, also on lines whose Hub number moved since the phone looked). Returns how many things changed.
pub fn accept_any(user_root: &Path, name: &str, allow_moved: bool) -> Result<usize, String> {
    if safe_name(name) && is_sheet_file(&inbox_dir(user_root).join(name)) {
        crate::countsheet::accept(user_root, name, allow_moved)
    } else if safe_name(name) && is_checkin_file(&inbox_dir(user_root).join(name)) {
        crate::checkinsheet::accept(user_root, name)
    } else {
        accept(user_root, name)
    }
}

/// Put a file aside without using it (it stays in `set-aside/`, never deleted).
pub fn set_aside(user_root: &Path, name: &str) -> Result<(), String> {
    if !safe_name(name) {
        // a file with an odd name can still be set aside: look it up by listing, never by building a path from the text
        let found = fs::read_dir(inbox_dir(user_root)).map_err(|_| "The mailbox could not be opened.".to_string())?.filter_map(|e| e.ok()).find(|e| e.file_name().to_string_lossy() == name && e.file_type().map(|t| t.is_file()).unwrap_or(false));
        let Some(e) = found else { return Err("That file is not in the mailbox.".to_string()) };
        let safe = format!("odd-name-{}.txt", security_now_stamp());
        fs::create_dir_all(aside_dir(user_root)).map_err(|_| "The folder could not be made.".to_string())?;
        return fs::rename(e.path(), aside_dir(user_root).join(safe)).map_err(|_| "The file could not be moved.".to_string());
    }
    let path = inbox_dir(user_root).join(name);
    if !path.is_file() {
        return Err("That file is not in the mailbox.".to_string());
    }
    move_into(&aside_dir(user_root), &path, name)
}

/// The names and fingerprints the host has accepted, so a phone knows which outbox files it may clear.
pub fn accepted(user_root: &Path) -> Vec<(String, String)> {
    fsio::read_to_string(&ledger_path(user_root))
        .map(|t| t.lines().filter_map(|l| { let mut f = l.split('\t'); Some((f.next()?.to_string(), f.next()?.to_string())) }).collect())
        .unwrap_or_default()
}

/// Write the file the way Pocket would, for the tests and for a future host-side helper.
pub fn render_report(device: &str, person: &str, created: &str, entries: &[incident::Entry]) -> String {
    let mut o = format!("# format: pocket-report 1\n# device: {}\n# person: {}\n# created: {}\n", device, person, created);
    for e in entries {
        o.push_str(&incident::line(e));
    }
    o
}

/// A trusted phone hands in a report over the Hub's network. `device` is the id the Hub knows the phone by (taken
/// from its sign-in, never from what it says), so a phone can only hand in reports that carry its own code.
/// The file goes through every check an accept would make, and is removed again if any fails. Handing in the same
/// report twice is fine (nothing changes); a different report under the same name is refused.
pub fn upload(user_root: &Path, device: &str, name: &str, text: &str) -> Result<(), String> {
    if !valid_id(device) || !safe_name(name) || !name.starts_with(&format!("{}-", device)) {
        return Err("That is not a report file name this phone may use.".to_string());
    }
    if text.len() as u64 > MAX_FILE {
        return Err("This report is much bigger than any report should be.".to_string());
    }
    if accepted(user_root).iter().any(|(n, _)| n == name) {
        return Ok(());
    }
    let target = inbox_dir(user_root).join(name);
    if let Ok(old) = fsio::read(&target) {
        return if old == text.as_bytes() { Ok(()) } else { Err("A different report with this name is already waiting.".to_string()) };
    }
    if waiting_count(user_root) >= MAX_FILES {
        return Err("The mailbox is full. The host needs to look at the waiting reports first.".to_string());
    }
    fs::create_dir_all(inbox_dir(user_root)).map_err(|_| "The mailbox could not be opened.".to_string())?;
    fsio::write_atomic(&target, text.as_bytes()).map_err(|_| "The report could not be saved.".to_string())?;
    if is_sheet_file(&target) {
        return match crate::countsheet::read(&target) {
            Ok(sh) if sh.device == device => Ok(()),
            Ok(_) => {
                let _ = fs::remove_file(&target);
                Err("The count sheet names a different phone than the one that sent it.".to_string())
            }
            Err(e) => {
                let _ = fs::remove_file(&target);
                Err(e)
            }
        };
    }
    if is_checkin_file(&target) {
        return match crate::checkinsheet::read(&target) {
            Ok(sh) if sh.device == device => Ok(()),
            Ok(_) => {
                let _ = fs::remove_file(&target);
                Err("The check-in sheet names a different phone than the one that sent it.".to_string())
            }
            Err(e) => {
                let _ = fs::remove_file(&target);
                Err(e)
            }
        };
    }
    match read_report(&target) {
        Ok(r) if r.device == device => Ok(()),
        Ok(_) => {
            let _ = fs::remove_file(&target);
            Err("The report names a different phone than the one that sent it.".to_string())
        }
        Err(e) => {
            let _ = fs::remove_file(&target);
            Err(e)
        }
    }
}

/// The names of this phone's reports the host has accepted. The phone may clear exactly these from its outbox.
pub fn accepted_for(user_root: &Path, device: &str) -> Vec<String> {
    fsio::read_to_string(&ledger_path(user_root))
        .map(|t| {
            t.lines()
                .filter_map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    if f.len() >= 3 && f[2] == device { Some(f[0].to_string()) } else { None }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Used by the tests and by anything that needs to put a file in the mailbox safely.
pub fn drop_file(user_root: &Path, name: &str, text: &str) -> Result<(), String> {
    if !safe_name(name) {
        return Err("That is not a report file name.".to_string());
    }
    fsio::write_atomic(&inbox_dir(user_root).join(name), text.as_bytes()).map_err(|_| "The report could not be saved.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::Site;
    use crate::security_incident::{Entry, Kind, Source};

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-pocket-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        store::save_sites(&p, &[Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        p
    }
    const T: &str = "2026-11-08T10:00-05:00";
    fn e(id: &str, inc: &str, kind: Kind, arg1: &str, text: &str) -> Entry {
        Entry { id: id.into(), incident: inc.into(), kind, happened_at: T.into(), recorded_at: T.into(), by: "ignored".into(), source: Source::Observed, arg1: arg1.into(), arg2: String::new(), arg3: String::new(), text: text.into(), supersedes: String::new() }
    }
    fn open(id: &str, inc: &str) -> Entry {
        Entry { arg2: "2".into(), arg3: "live".into(), ..e(id, inc, Kind::Open, "camp", "Noise at the edge") }
    }
    fn put(r: &Path, name: &str, entries: &[Entry]) {
        drop_file(r, name, &render_report("n10", "Sam", T, entries)).unwrap();
    }

    #[test]
    fn a_good_report_waits_then_is_accepted_as_reported() {
        let r = tmp("good");
        put(&r, "n10-0001.txt", &[open("n10-e1", "n10-i1"), e("n10-e2", "n10-i1", Kind::Note, "", "Two lights, moving")]);
        let w = waiting(&r);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].problem.as_str(), w[0].entries, w[0].person.as_str(), w[0].device.as_str()), ("", 2, "Sam", "n10"));
        assert_eq!(waiting_count(&r), 1);
        // nothing is in the records until the host accepts
        assert!(store::load_incident_log(&r).unwrap().items.is_empty());
        assert_eq!(accept(&r, "n10-0001.txt").unwrap(), 2);
        let log = store::load_incident_log(&r).unwrap().items;
        assert_eq!(log.len(), 2);
        assert!(log.iter().all(|x| x.source == Source::Reported && x.by == "Sam"));
        assert_eq!(waiting_count(&r), 0);
        assert!(done_dir(&r).join("n10-0001.txt").exists());
        let acc = accepted(&r);
        assert_eq!(acc.len(), 1);
        assert_eq!(acc[0].0, "n10-0001.txt");
        assert_eq!(acc[0].1.len(), 32);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_same_report_twice_changes_nothing() {
        let r = tmp("twice");
        let entries = [open("n10-e1", "n10-i1")];
        put(&r, "n10-0001.txt", &entries);
        assert_eq!(accept(&r, "n10-0001.txt").unwrap(), 1);
        // the phone did not hear back and sends it again
        put(&r, "n10-0001.txt", &entries);
        assert_eq!(accept(&r, "n10-0001.txt").unwrap(), 0);
        assert_eq!(store::load_incident_log(&r).unwrap().items.len(), 1);
        assert!(fs::read_dir(done_dir(&r)).unwrap().count() == 2, "both copies are kept");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_phone_cannot_change_status_activate_a_plan_or_borrow_ids() {
        let r = tmp("rules");
        // status change
        put(&r, "a.txt", &[open("n10-e1", "n10-i1"), e("n10-e2", "n10-i1", Kind::Status, "closed", "all fine")]);
        assert!(accept(&r, "a.txt").is_err());
        // plan activation
        put(&r, "b.txt", &[open("n10-e1", "n10-i1"), e("n10-e2", "n10-i1", Kind::Plan, "evacuate", "")]);
        assert!(accept(&r, "b.txt").is_err());
        // an entry id that does not carry the phone's code
        put(&r, "c.txt", &[open("e1", "n10-i1")]);
        assert!(accept(&r, "c.txt").is_err());
        // a new incident number that is not the phone's own
        put(&r, "d.txt", &[open("n10-e1", "i1")]);
        assert!(accept(&r, "d.txt").is_err());
        // unknown site
        let mut o = open("n10-e1", "n10-i1");
        o.arg1 = "nowhere".into();
        put(&r, "e.txt", &[o]);
        assert!(accept(&r, "e.txt").is_err());
        // all of the above left the records alone and every file where it was
        assert!(store::load_incident_log(&r).unwrap().items.is_empty());
        assert_eq!(waiting_count(&r), 5);
        assert!(waiting(&r).iter().all(|w| !w.problem.is_empty() || w.name == "e.txt"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn one_bad_entry_means_none_are_added() {
        let r = tmp("allornone");
        put(&r, "n10-0001.txt", &[open("n10-e1", "n10-i1"), e("n10-e2", "n10-i1", Kind::Note, "", ""), e("n10-e3", "n10-i1", Kind::Note, "", "fine")]);
        let err = accept(&r, "n10-0001.txt").unwrap_err();
        assert!(err.contains("Nothing was added"), "{}", err);
        assert!(store::load_incident_log(&r).unwrap().items.is_empty());
        assert!(inbox_dir(&r).join("n10-0001.txt").exists());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn an_entry_number_used_for_something_different_is_refused() {
        let r = tmp("clash");
        put(&r, "n10-0001.txt", &[open("n10-e1", "n10-i1")]);
        accept(&r, "n10-0001.txt").unwrap();
        let mut other = open("n10-e1", "n10-i1");
        other.text = "A different story".into();
        put(&r, "n10-0002.txt", &[other]);
        assert!(accept(&r, "n10-0002.txt").unwrap_err().contains("already used"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_note_can_join_an_incident_the_host_already_has() {
        let r = tmp("join");
        // the host opened i1 itself
        let mut o = open("e1", "i1");
        o.by = "Dad".into();
        store::append_incident(&r, &o).unwrap();
        put(&r, "n10-0001.txt", &[e("n10-e1", "i1", Kind::Note, "", "Saw the truck leave")]);
        assert_eq!(accept(&r, "n10-0001.txt").unwrap(), 1);
        let log = store::load_incident_log(&r).unwrap().items;
        assert_eq!(log.len(), 2);
        assert_eq!(log[1].incident, "i1");
        assert_eq!(log[1].source, Source::Reported);
        // but not one for an incident that does not exist
        put(&r, "n10-0002.txt", &[e("n10-e2", "i9", Kind::Note, "", "x")]);
        assert!(accept(&r, "n10-0002.txt").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn junk_files_are_listed_with_a_reason_and_never_trusted() {
        let r = tmp("junk");
        let dir = inbox_dir(&r);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.txt.exe"), "x").unwrap();
        fs::write(dir.join("Report One.txt"), "x").unwrap();
        fs::write(dir.join("empty.txt"), "").unwrap();
        fs::write(dir.join("binary.txt"), [0xff, 0xfe, 0x00]).unwrap();
        fs::write(dir.join("newer.txt"), "# format: pocket-report 9\n").unwrap();
        fs::write(dir.join("big.txt"), vec![b'a'; (MAX_FILE + 1) as usize]).unwrap();
        fs::write(dir.join("noperson.txt"), "# format: pocket-report 1\n# device: n10\n# created: 2026-11-08T10:00Z\n").unwrap();
        fs::write(dir.join("badline.txt"), "# format: pocket-report 1\n# device: n10\n# person: Sam\n# created: 2026-11-08T10:00Z\nthis is not a log line\n").unwrap();
        let w = waiting(&r);
        assert_eq!(w.len(), 8);
        assert!(w.iter().all(|x| !x.problem.is_empty()), "{:?}", w);
        for x in &w {
            assert!(accept(&r, &x.name).is_err(), "{}", x.name);
        }
        // names that try to climb out are refused before any path is built
        for bad in ["../x.txt", "a/b.txt", "..", "", "x.txt/../../y.txt", "C:\\x.txt"] {
            assert!(accept(&r, bad).is_err(), "{}", bad);
            assert!(!safe_name(bad));
        }
        assert!(store::load_incident_log(&r).unwrap().items.is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_mailbox_is_not_followed() {
        let r = tmp("link");
        let dir = inbox_dir(&r);
        fs::create_dir_all(&dir).unwrap();
        let outside = r.join("secret.txt");
        fs::write(&outside, "# format: pocket-report 1\n").unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("sneaky.txt")).unwrap();
        let w = waiting(&r);
        assert_eq!(w.len(), 1);
        assert!(!w[0].problem.is_empty());
        assert!(accept(&r, "sneaky.txt").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn set_aside_moves_and_never_deletes() {
        let r = tmp("aside");
        put(&r, "n10-0001.txt", &[open("n10-e1", "n10-i1")]);
        set_aside(&r, "n10-0001.txt").unwrap();
        assert_eq!(waiting_count(&r), 0);
        assert!(aside_dir(&r).join("n10-0001.txt").exists());
        assert!(set_aside(&r, "n10-0001.txt").is_err());
        // an oddly named file can be set aside too, under a safe name
        fs::write(inbox_dir(&r).join("Weird Name.EXE"), "x").unwrap();
        set_aside(&r, "Weird Name.EXE").unwrap();
        assert_eq!(waiting_count(&r), 0);
        assert!(set_aside(&r, "../../etc/passwd").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_damaged_incident_log_blocks_acceptance() {
        let r = tmp("damaged");
        put(&r, "n10-0001.txt", &[open("n10-e1", "n10-i1")]);
        fs::create_dir_all(store::dir(&r)).unwrap();
        fs::write(store::incident_path(&r), "# format: security-incidents 1\nthis line is damaged\n").unwrap();
        assert!(accept(&r, "n10-0001.txt").is_err());
        assert!(inbox_dir(&r).join("n10-0001.txt").exists());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_phone_can_only_hand_in_reports_that_carry_its_own_code() {
        let r = tmp("upload");
        let ok = render_report("n10", "Sam", T, &[open("n10-0001", "n10-inc1")]);
        upload(&r, "n10", "n10-0001.txt", &ok).unwrap();
        upload(&r, "n10", "n10-0001.txt", &ok).unwrap(); // again: nothing changes
        assert_eq!(waiting_count(&r), 1);
        // another phone's code, a name that is not its own, a changed report under the same name
        assert!(upload(&r, "zz99", "n10-0002.txt", &ok).is_err());
        assert!(upload(&r, "n10", "other-0002.txt", &ok).is_err());
        assert!(upload(&r, "n10", "../n10-0003.txt", &ok).is_err());
        let other = render_report("n10", "Sam", T, &[open("n10-0009", "n10-inc9")]);
        assert!(upload(&r, "n10", "n10-0001.txt", &other).is_err());
        // a report that says it is from another phone is refused and leaves nothing behind
        let liar = render_report("zz99", "Sam", T, &[open("zz99-0001", "zz99-inc1")]);
        assert!(upload(&r, "n10", "n10-0004.txt", &liar).is_err());
        assert!(!inbox_dir(&r).join("n10-0004.txt").exists());
        assert!(upload(&r, "n10", "n10-0005.txt", "not a report").is_err());
        assert!(!inbox_dir(&r).join("n10-0005.txt").exists());
        assert!(upload(&r, "n10", "n10-0006.txt", &"x".repeat(300_000)).is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_phone_learns_what_was_accepted_and_only_its_own() {
        let r = tmp("syncd");
        let ok = render_report("n10", "Sam", T, &[open("n10-0001", "n10-inc1")]);
        upload(&r, "n10", "n10-0001.txt", &ok).unwrap();
        assert!(accepted_for(&r, "n10").is_empty());
        accept(&r, "n10-0001.txt").unwrap();
        assert_eq!(accepted_for(&r, "n10"), vec!["n10-0001.txt".to_string()]);
        assert!(accepted_for(&r, "zz99").is_empty());
        // handing it in again after it was accepted changes nothing and adds nothing to the mailbox
        upload(&r, "n10", "n10-0001.txt", &ok).unwrap();
        assert_eq!(waiting_count(&r), 0);
        let _ = fs::remove_dir_all(&r);
    }
}
