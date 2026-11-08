//! Check-ins sent from phones (docs/45). A phone in the field can hand in the contacts, checkpoints and notes about
//! someone who is out. They wait in the same mailbox as reports and count sheets (`user/inbox/pocket/`) and change nothing
//! until the host accepts them. Accepted lines enter the outing log as "reported", written by the person named on the sheet.
//!
//! ```text
//! # format: checkin-sheet 1
//! # device: <the phone's code>
//! # person: <who is writing>
//! # created: 2026-11-08T10:00-05:00
//! <id, starting with the device code> TAB <outing> TAB contact|checkpoint|note TAB <time> TAB <a1> TAB <a2> TAB <words>
//! ```
//! contact: a1 is ok, delayed or help. checkpoint: a1 is the checkpoint name, a2 is ok, delayed or help. note: the words.
//! A sheet can only add to an outing that is already out. It can never start, end, extend or cancel one. All lines or none.

use crate::security::{self, valid_id};
use crate::security_outing::{self as outing, Ev};
use crate::security_pocket as pocket;
use crate::security_store as store;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const MAX_LINES: usize = 200;

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub id: String,
    pub outing: String,
    pub ev: Ev,
    pub at: String,
    pub a1: String,
    pub a2: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub device: String,
    pub person: String,
    pub created: String,
    pub lines: Vec<Line>,
    pub fingerprint: String,
}

pub fn is_sheet(text: &str) -> bool {
    text.lines().find_map(|l| l.strip_prefix("# format:")).map(|f| f.split_whitespace().next() == Some("checkin-sheet")).unwrap_or(false)
}

pub fn parse(text: &str) -> Result<Sheet, String> {
    let (mut ok, mut device, mut person, mut created) = (false, String::new(), String::new(), String::new());
    let mut lines: Vec<Line> = Vec::new();
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("# format:") {
            ok = r.split_whitespace().collect::<Vec<_>>() == ["checkin-sheet", "1"];
            if !ok { return Err("This check-in sheet is from a different version of Pocket, so it was left alone.".to_string()); }
        } else if let Some(r) = l.strip_prefix("# device:") {
            device = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# person:") {
            person = r.trim().chars().take(80).collect();
        } else if let Some(r) = l.strip_prefix("# created:") {
            created = r.trim().to_string();
        } else if l.starts_with('#') || l.trim().is_empty() {
        } else {
            let f: Vec<&str> = l.split('\t').collect();
            let ev = f.get(2).and_then(|w| Ev::from_word(w)).filter(|e| matches!(e, Ev::Contact | Ev::Checkpoint | Ev::Note));
            let line = (f.len() == 7 && valid_id(f[0]) && valid_id(f[1]) && security::parse_instant(f[3]).is_some())
                .then(|| ev.map(|ev| Line { id: f[0].into(), outing: f[1].into(), ev, at: f[3].into(), a1: crate::ops::unesc(f[4]), a2: crate::ops::unesc(f[5]), text: crate::ops::unesc(f[6]) }))
                .flatten();
            match line {
                Some(x) if !lines.iter().any(|y| y.id == x.id) => lines.push(x),
                _ => return Err("A line in this check-in sheet could not be read.".to_string()),
            }
        }
    }
    if !ok { return Err("This is not a check-in sheet.".to_string()); }
    if !valid_id(&device) || device.len() > 12 { return Err("The sheet does not name its phone properly.".to_string()); }
    if person.trim().is_empty() { return Err("The sheet does not say who wrote it.".to_string()); }
    if security::parse_instant(&created).is_none() { return Err("The sheet has no readable date.".to_string()); }
    if lines.is_empty() { return Err("The sheet has nothing in it.".to_string()); }
    if lines.len() > MAX_LINES { return Err("The sheet has far more lines than expected.".to_string()); }
    let prefix = format!("{}-", device);
    if lines.iter().any(|l| !l.id.starts_with(&prefix)) { return Err("A line in the sheet does not belong to the phone that sent it.".to_string()); }
    Ok(Sheet { device, person, created, lines, fingerprint: pocket::fingerprint(text.as_bytes()) })
}

pub fn read(path: &Path) -> Result<Sheet, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "The file could not be opened.".to_string())?;
    if !meta.is_file() { return Err("This is not an ordinary file, so it was left alone.".to_string()); }
    if meta.len() > pocket::MAX_FILE { return Err("This file is much bigger than any check-in sheet should be.".to_string()); }
    let data = fs::read(path).map_err(|_| "The file could not be read.".to_string())?;
    parse(&String::from_utf8(data).map_err(|_| "This is not plain text.".to_string())?)
}

fn entries(sheet: &Sheet, existing: &[outing::Entry], now_text: &str) -> Vec<outing::Entry> {
    let mut ids: Vec<String> = existing.iter().map(|e| e.id.clone()).collect();
    sheet.lines.iter().map(|l| {
        let id = crate::ops::next_id('e', ids.iter().cloned());
        ids.push(id.clone());
        outing::Entry { id, outing: l.outing.clone(), ev: l.ev, at: l.at.clone(), recorded: now_text.to_string(), by: sheet.person.clone(), source: "reported".into(), a1: l.a1.clone(), a2: l.a2.clone(), a3: String::new(), back: String::new(), next: String::new(), method: String::new(), text: l.text.clone() }
    }).collect()
}

/// Accept a waiting sheet. All lines are checked first; if any is refused nothing is added. Returns how many lines went in.
pub fn accept(user_root: &Path, name: &str) -> Result<usize, String> {
    if !pocket::safe_name(name) { return Err("That is not a file name Pocket would make.".to_string()); }
    let path = pocket::inbox_dir(user_root).join(name);
    let sheet = read(&path)?;
    let before = pocket::accepted(user_root).iter().any(|(n, f)| n == name && *f == sheet.fingerprint);
    let mut added = 0;
    if !before {
        let log = outing::load(user_root)?;
        if !log.rejected.is_empty() { return Err("Some lines in the outing log could not be read, so nothing was added. Look at those first.".to_string()); }
        let sites: Vec<String> = store::load_sites(user_root)?.items.into_iter().map(|s| s.id).collect();
        let now = security::format_instant(crate::security_api::now_minutes(), 0);
        let new = entries(&sheet, &log.items, &now);
        let mut sim = log.items.clone();
        for e in &new {
            if let Some(p) = outing::check_entry(&sim, &sites, e) { return Err(format!("{} Nothing was added. Set the sheet aside if it is out of date.", p)); }
            sim.push(e.clone());
        }
        for e in &new { outing::append(user_root, e)?; added += 1; }
        fs::create_dir_all(store::dir(user_root)).map_err(|_| "The security folder could not be made.".to_string())?;
        let mut f = OpenOptions::new().create(true).append(true).open(pocket::ledger_path(user_root)).map_err(|_| "The ledger could not be opened.".to_string())?;
        let line = format!("{}\t{}\t{}\t{}\t{}\n", name, sheet.fingerprint, sheet.device, sheet.lines.len(), now);
        f.write_all(line.as_bytes()).and_then(|_| f.sync_all()).map_err(|_| "The ledger could not be written.".to_string())?;
    }
    pocket::move_into(&pocket::done_dir(user_root), &path, name)?;
    Ok(added)
}

pub fn render(device: &str, person: &str, created: &str, lines: &[Line]) -> String {
    let mut o = format!("# format: checkin-sheet 1\n# device: {}\n# person: {}\n# created: {}\n", device, person, created);
    for l in lines {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", l.id, l.outing, l.ev.word(), l.at, crate::ops::esc(&l.a1), crate::ops::esc(&l.a2), crate::ops::esc(&l.text)));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(name: &str) -> std::path::PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-ci-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(&r).unwrap();
        crate::security_api::site_save(&r, "", "Camp", "", "", "").unwrap();
        r
    }
    fn go_out(r: &Path) -> String {
        let site = store::load_sites(r).unwrap().items[0].id.clone();
        crate::security_api::outing_start(r, &site, "patrol", "Sam, Dana", "Perimeter loop", &security::format_instant(crate::security_api::now_minutes() + 120, 0).replace('Z', "").chars().take(16).collect::<String>(), "", "radio", "Base", 0).unwrap()
    }
    fn ln(id: &str, o: &str, ev: Ev, a1: &str, a2: &str, t: &str) -> Line {
        Line { id: id.into(), outing: o.into(), ev, at: security::format_instant(crate::security_api::now_minutes(), 0), a1: a1.into(), a2: a2.into(), text: t.into() }
    }
    fn put(r: &Path, name: &str, lines: &[Line]) {
        pocket::drop_file(r, name, &render("n10", "Sam", &security::format_instant(crate::security_api::now_minutes(), 0), lines)).unwrap();
    }

    #[test]
    fn phone_check_ins_wait_then_enter_the_outing_log_as_reported() {
        let r = setup("ok");
        let o = go_out(&r);
        put(&r, "n10-checkin-1.txt", &[ln("n10-1", &o, Ev::Checkpoint, "North gate", "ok", ""), ln("n10-2", &o, Ev::Contact, "delayed", "", "Creek is high")]);
        assert_eq!(accept_any_for_test(&r, "n10-checkin-1.txt").unwrap(), 2);
        let log = outing::load(&r).unwrap().items;
        let rep: Vec<_> = log.iter().filter(|e| e.source == "reported").collect();
        assert_eq!((rep.len(), rep[0].by.as_str(), rep[0].a1.as_str()), (2, "Sam", "North gate"));
        let b = outing::build(&log);
        assert_eq!(b[0].checkpoints.len(), 1);
        // accepting the same file again changes nothing
        put(&r, "n10-checkin-1.txt", &[ln("n10-1", &o, Ev::Checkpoint, "North gate", "ok", ""), ln("n10-2", &o, Ev::Contact, "delayed", "", "Creek is high")]);
        assert_eq!(accept_any_for_test(&r, "n10-checkin-1.txt").unwrap(), 0);
        assert_eq!(outing::load(&r).unwrap().items.len(), log.len());
        let _ = fs::remove_dir_all(&r);
    }

    fn accept_any_for_test(r: &Path, n: &str) -> Result<usize, String> {
        pocket::accept_any(r, n, false)
    }

    #[test]
    fn a_sheet_that_cannot_all_go_in_adds_nothing_and_cannot_start_or_end_an_outing() {
        let r = setup("bad");
        let o = go_out(&r);
        put(&r, "n10-checkin-2.txt", &[ln("n10-1", &o, Ev::Contact, "ok", "", ""), ln("n10-2", "e999", Ev::Contact, "ok", "", "")]);
        let before = outing::load(&r).unwrap().items.len();
        assert!(accept_any_for_test(&r, "n10-checkin-2.txt").is_err());
        assert_eq!(outing::load(&r).unwrap().items.len(), before, "all or none");
        // a sheet cannot return or cancel an outing
        let forged = format!("# format: checkin-sheet 1\n# device: n10\n# person: Sam\n# created: {}\nn10-1\t{}\treturn\t{}\t\t\t\n", security::format_instant(crate::security_api::now_minutes(), 0), o, security::format_instant(crate::security_api::now_minutes(), 0));
        assert!(parse(&forged).is_err());
        // another phone's ids, and no author, are refused
        assert!(parse(&forged.replace("return", "contact").replace("n10-1", "n11-1")).is_err());
        assert!(parse(&forged.replace("return", "contact").replace("# person: Sam\n", "")).is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
