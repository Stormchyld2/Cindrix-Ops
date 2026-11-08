//! Inventory count sheets from phones (docs/45). A phone that has scanned and counted sends a **count sheet** to the same
//! waiting mailbox as its reports (`user/inbox/pocket/`). Nothing in the Inventory changes until the host accepts it.
//!
//! ```text
//! # format: count-sheet 1
//! # device: <the phone's code>
//! # person: <who counted>
//! # created: 2026-11-08T10:00-05:00
//! <item uid> TAB <number the phone saw> TAB <number counted>
//! ```
//! Rules: a sheet can only set the number on lines that already exist (it never adds, renames or deletes a line);
//! all lines or none are applied; a line whose Hub number moved since the phone looked is flagged, and is applied only
//! when the host says to accept anyway; the same sheet twice changes nothing. Accepting also records the day it was counted.

use crate::inventory::Item;
use crate::security::{self, valid_id};
use crate::security_pocket as pocket;
use crate::userdata::UserDir;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

const MAX_LINES: usize = 2000;
const BIGGEST: f64 = 1.0e9;

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub uid: String,
    pub was: f64,
    pub counted: f64,
}

#[derive(Debug, Clone)]
pub struct Sheet {
    pub device: String,
    pub person: String,
    pub created: String,
    pub lines: Vec<Line>,
    pub fingerprint: String,
}

/// True when the text is a count sheet (by its first format line), so the mailbox knows which reader to use.
pub fn is_sheet(text: &str) -> bool {
    text.lines().find_map(|l| l.strip_prefix("# format:")).map(|f| f.split_whitespace().next() == Some("count-sheet")).unwrap_or(false)
}

fn number(s: &str) -> Option<f64> {
    let n: f64 = s.trim().parse().ok()?;
    if n.is_finite() && (0.0..=BIGGEST).contains(&n) { Some(n) } else { None }
}

pub fn parse(text: &str) -> Result<Sheet, String> {
    let (mut fmt, mut device, mut person, mut created) = (false, String::new(), String::new(), String::new());
    let mut lines: Vec<Line> = Vec::new();
    let mut seen = HashSet::new();
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("# format:") {
            if r.split_whitespace().collect::<Vec<_>>() != ["count-sheet", "1"] {
                return Err("This count sheet is from a different version of Pocket, so it was left alone.".to_string());
            }
            fmt = true;
        } else if let Some(r) = l.strip_prefix("# device:") {
            device = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# person:") {
            person = r.trim().chars().take(80).collect();
        } else if let Some(r) = l.strip_prefix("# created:") {
            created = r.trim().to_string();
        } else if !l.starts_with('#') && !l.trim().is_empty() {
            let f: Vec<&str> = l.split('\t').collect();
            let ok = f.len() == 3 && !f[0].is_empty() && f[0].len() <= 64 && !f[0].chars().any(|c| c.is_control() || c == ',');
            let (Some(was), Some(counted)) = (if ok { number(f[1]) } else { None }, if ok { number(f[2]) } else { None }) else {
                return Err("A line in this count sheet could not be read, so none of it can be used.".to_string());
            };
            if !seen.insert(f[0].to_string()) {
                return Err("This count sheet counts the same item twice, so none of it can be used.".to_string());
            }
            lines.push(Line { uid: f[0].to_string(), was, counted });
            if lines.len() > MAX_LINES {
                return Err("This count sheet has far more lines than a count should.".to_string());
            }
        }
    }
    if !fmt {
        return Err("This is not a count sheet.".to_string());
    }
    if !valid_id(&device) || device.len() > 12 {
        return Err("The count sheet does not name its phone properly.".to_string());
    }
    if person.trim().is_empty() {
        return Err("The count sheet does not say who counted.".to_string());
    }
    if security::parse_instant(&created).is_none() {
        return Err("The count sheet has no readable date.".to_string());
    }
    if lines.is_empty() {
        return Err("The count sheet is empty.".to_string());
    }
    Ok(Sheet { device, person, created, lines, fingerprint: pocket::fingerprint(text.as_bytes()) })
}

/// Read one file from the mailbox, with the same care as a report (ordinary file, not too big, plain text).
pub fn read(path: &Path) -> Result<Sheet, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "The file could not be opened.".to_string())?;
    if !meta.is_file() {
        return Err("This is not an ordinary file, so it was left alone.".to_string());
    }
    if meta.len() > pocket::MAX_FILE {
        return Err("This file is much bigger than any count sheet should be.".to_string());
    }
    let data = fs::read(path).map_err(|_| "The file could not be read.".to_string())?;
    parse(&String::from_utf8(data).map_err(|_| "This is not plain text.".to_string())?)
}

/// How a line stands against the Inventory right now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Standing {
    /// The Hub still has the number the phone saw.
    Matches,
    /// The Hub number moved since the phone looked.
    Moved,
    /// The Hub already has the counted number (applied before, or counted the same).
    AlreadyThere,
    /// No such line in the Inventory any more.
    Missing,
}

pub fn standing(items: &[Item], l: &Line) -> (Standing, Option<usize>) {
    match items.iter().position(|i| i.uid == l.uid) {
        None => (Standing::Missing, None),
        Some(ix) => {
            let q = items[ix].quantity;
            let s = if (q - l.counted).abs() < 1e-9 {
                Standing::AlreadyThere
            } else if (q - l.was).abs() < 1e-9 {
                Standing::Matches
            } else {
                Standing::Moved
            };
            (s, Some(ix))
        }
    }
}

/// (lines that moved since the phone looked, lines no longer in the Inventory)
pub fn tally(items: &[Item], s: &Sheet) -> (usize, usize) {
    let mut moved = 0;
    let mut missing = 0;
    for l in &s.lines {
        match standing(items, l).0 {
            Standing::Moved => moved += 1,
            Standing::Missing => missing += 1,
            _ => {}
        }
    }
    (moved, missing)
}

/// Apply a waiting count sheet. Returns how many Inventory lines changed.
/// Refuses (changing nothing) if any line is no longer in the Inventory, or if any line moved and `allow_moved` is false.
pub fn accept(user_root: &Path, name: &str, allow_moved: bool) -> Result<usize, String> {
    if !pocket::safe_name(name) {
        return Err("That is not a file name Pocket would make.".to_string());
    }
    let path = pocket::inbox_dir(user_root).join(name);
    let sheet = read(&path)?;
    let u = UserDir::new(user_root);
    let mut items = u.load_inventory()?;
    // a sheet taken in before (same name and contents) is only filed away again
    let before = pocket::accepted(user_root).iter().any(|(n, f)| n == name && *f == sheet.fingerprint);
    let mut changed = 0;
    if !before {
        let (moved, missing) = tally(&items, &sheet);
        if missing > 0 {
            return Err(format!("{} item{} on this sheet {} no longer in the Inventory, so nothing was changed. Set the sheet aside and count again.", missing, if missing == 1 { "" } else { "s" }, if missing == 1 { "is" } else { "are" }));
        }
        if moved > 0 && !allow_moved {
            return Err(format!("{} number{} in the Inventory changed after the phone looked, so nothing was changed. Look at them, then accept anyway if the phone's count is right.", moved, if moved == 1 { "" } else { "s" }));
        }
        let day: String = sheet.created.chars().take(10).collect();
        for l in &sheet.lines {
            if let (_, Some(ix)) = standing(&items, l) {
                if (items[ix].quantity - l.counted).abs() >= 1e-9 {
                    changed += 1;
                }
                items[ix].quantity = l.counted;
                items[ix].checked = day.clone();
            }
        }
        u.save_inventory(&items)?;
        fs::create_dir_all(crate::security_store::dir(user_root)).map_err(|_| "The security folder could not be made.".to_string())?;
        let mut f = OpenOptions::new().create(true).append(true).open(pocket::ledger_path(user_root)).map_err(|_| "The ledger could not be opened.".to_string())?;
        let line = format!("{}\t{}\t{}\t{}\t{}\n", name, sheet.fingerprint, sheet.device, sheet.lines.len(), security::format_instant(crate::security_api::now_minutes(), 0));
        f.write_all(line.as_bytes()).and_then(|_| f.sync_all()).map_err(|_| "The ledger could not be written.".to_string())?;
    }
    pocket::move_into(&pocket::done_dir(user_root), &path, name)?;
    Ok(changed)
}

/// One line for the host to look at: what the item is, what the phone saw, what the Hub has now, what was counted.
pub fn detail_json(user_root: &Path, name: &str) -> Result<String, String> {
    if !pocket::safe_name(name) {
        return Err("That is not a file name Pocket would make.".to_string());
    }
    let sheet = read(&pocket::inbox_dir(user_root).join(name))?;
    let items = UserDir::new(user_root).load_inventory()?;
    let q = |s: &str| format!("\"{}\"", crate::llama::json_escape(s));
    let rows: Vec<String> = sheet
        .lines
        .iter()
        .map(|l| {
            let (st, ix) = standing(&items, l);
            let (n, loc, unit, now) = ix.map(|i| (items[i].name.clone(), items[i].location.clone(), items[i].unit.clone(), items[i].quantity)).unwrap_or_default();
            let word = match st { Standing::Matches => "matches", Standing::Moved => "moved", Standing::AlreadyThere => "already", Standing::Missing => "missing" };
            format!("{{\"name\":{},\"location\":{},\"unit\":{},\"was\":{},\"now\":{},\"counted\":{},\"standing\":\"{}\"}}", q(&n), q(&loc), q(&unit), l.was, now, l.counted, word)
        })
        .collect();
    Ok(format!("{{\"person\":{},\"created\":{},\"lines\":[{}]}}", q(&sheet.person), q(&sheet.created), rows.join(",")))
}

/// Write a sheet the way Pocket would, for the tests and a future host-side helper.
pub fn render(device: &str, person: &str, created: &str, lines: &[Line]) -> String {
    let mut o = format!("# format: count-sheet 1\n# device: {}\n# person: {}\n# created: {}\n", device, person, created);
    for l in lines {
        o.push_str(&format!("{}\t{}\t{}\n", l.uid, l.was, l.counted));
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const T: &str = "2026-11-08T10:00-05:00";

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-counts-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        let items = vec![
            Item { id: "1".into(), name: "Rice".into(), location: "Pantry".into(), quantity: 3.0, unit: "lb".into(), uid: "u-rice".into(), ..Default::default() },
            Item { id: "2".into(), name: "Batteries AA".into(), location: "Garage".into(), quantity: 12.0, unit: "pk".into(), uid: "u-aa".into(), ..Default::default() },
        ];
        UserDir::new(&p).save_inventory(&items).unwrap();
        p
    }
    fn l(uid: &str, was: f64, counted: f64) -> Line {
        Line { uid: uid.into(), was, counted }
    }
    fn put(r: &Path, name: &str, lines: &[Line]) {
        pocket::drop_file(r, name, &render("n10", "Sam", T, lines)).unwrap();
    }
    fn qty(r: &Path, uid: &str) -> f64 {
        UserDir::new(r).load_inventory().unwrap().iter().find(|i| i.uid == uid).unwrap().quantity
    }

    #[test]
    fn nothing_changes_until_the_host_accepts_and_then_the_counts_and_day_are_set() {
        let r = tmp("good");
        put(&r, "n10-c1.txt", &[l("u-rice", 3.0, 2.0), l("u-aa", 12.0, 12.0)]);
        assert_eq!(qty(&r, "u-rice"), 3.0);
        let w = pocket::waiting(&r);
        assert_eq!((w[0].kind.as_str(), w[0].entries, w[0].problem.as_str(), w[0].changed), ("count-sheet", 2, "", 0));
        assert_eq!(accept(&r, "n10-c1.txt", false).unwrap(), 1);
        assert_eq!(qty(&r, "u-rice"), 2.0);
        let it = UserDir::new(&r).load_inventory().unwrap();
        assert_eq!(it.iter().find(|i| i.uid == "u-rice").unwrap().checked, "2026-11-08");
        assert_eq!(it.len(), 2, "a sheet never adds or deletes a line");
        assert_eq!(pocket::waiting_count(&r), 0);
        assert_eq!(pocket::accepted_for(&r, "n10"), vec!["n10-c1.txt".to_string()]);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_number_that_moved_since_the_phone_looked_waits_for_accept_anyway() {
        let r = tmp("moved");
        put(&r, "n10-c1.txt", &[l("u-rice", 5.0, 2.0), l("u-aa", 12.0, 10.0)]);
        let w = pocket::waiting(&r);
        assert_eq!(w[0].changed, 1);
        let e = accept(&r, "n10-c1.txt", false).unwrap_err();
        assert!(e.contains("changed after the phone looked"));
        assert_eq!((qty(&r, "u-rice"), qty(&r, "u-aa")), (3.0, 12.0), "all or none");
        assert_eq!(accept(&r, "n10-c1.txt", true).unwrap(), 2);
        assert_eq!((qty(&r, "u-rice"), qty(&r, "u-aa")), (2.0, 10.0));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn an_item_that_is_gone_stops_the_whole_sheet() {
        let r = tmp("gone");
        put(&r, "n10-c1.txt", &[l("u-rice", 3.0, 2.0), l("u-ghost", 1.0, 1.0)]);
        assert!(accept(&r, "n10-c1.txt", true).unwrap_err().contains("no longer in the Inventory"));
        assert_eq!(qty(&r, "u-rice"), 3.0);
        assert_eq!(pocket::waiting_count(&r), 1);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn bad_sheets_are_refused_with_a_reason() {
        let ok = render("n10", "Sam", T, &[l("u-rice", 3.0, 2.0)]);
        assert!(parse(&ok).is_ok());
        for (bad, why) in [
            (ok.replace("count-sheet 1", "count-sheet 2"), "different version"),
            (ok.replace("# person: Sam\n", ""), "who counted"),
            (ok.replace(T, "yesterday"), "readable date"),
            (ok.replace("u-rice\t3\t2", "u-rice\t3\t-4"), "could not be read"),
            (ok.replace("u-rice\t3\t2", "u-rice\t3\tNaN"), "could not be read"),
            (ok.replace("u-rice\t3\t2", "u-rice\t3\t1e12"), "could not be read"),
            (format!("{}u-rice\t3\t1\n", ok), "twice"),
            (render("n10", "Sam", T, &[]), "empty"),
            ("# format: count-sheet 1\n".to_string(), "not name its phone"),
        ] {
            let e = parse(&bad).unwrap_err();
            assert!(e.contains(why), "{} -> {}", why, e);
        }
        assert!(!is_sheet("# format: pocket-report 1\n") && is_sheet(&ok));
    }

    #[test]
    fn the_same_sheet_twice_changes_nothing_and_a_crash_before_filing_is_safe() {
        let r = tmp("twice");
        put(&r, "n10-c1.txt", &[l("u-rice", 3.0, 2.0)]);
        accept(&r, "n10-c1.txt", false).unwrap();
        // the phone sends it again (it had not heard back yet): it is filed away again with no change
        put(&r, "n10-c1.txt", &[l("u-rice", 3.0, 2.0)]);
        assert_eq!(accept(&r, "n10-c1.txt", false).unwrap(), 0);
        assert_eq!(qty(&r, "u-rice"), 2.0);
        assert_eq!(pocket::accepted(&r).len(), 1);
        // a crash after the Inventory was saved but before filing: the file is still there and the number is already set
        put(&r, "n10-c2.txt", &[l("u-aa", 12.0, 9.0)]);
        UserDir::new(&r).load_inventory().map(|mut it| { it.iter_mut().find(|i| i.uid == "u-aa").unwrap().quantity = 9.0; UserDir::new(&r).save_inventory(&it).unwrap(); }).unwrap();
        assert_eq!(accept(&r, "n10-c2.txt", false).unwrap(), 0);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_host_sees_what_the_phone_saw_what_the_hub_has_and_what_was_counted() {
        let r = tmp("detail");
        put(&r, "n10-c1.txt", &[l("u-rice", 5.0, 2.0), l("u-aa", 12.0, 12.0)]);
        let j = detail_json(&r, "n10-c1.txt").unwrap();
        assert!(j.contains("\"name\":\"Rice\"") && j.contains("\"standing\":\"moved\"") && j.contains("\"now\":3"));
        assert!(j.contains("\"standing\":\"already\""));
        assert!(detail_json(&r, "../x.txt").is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
