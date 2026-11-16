//! Skill and plan ticks from phones (docs/45). Pocket keeps a person's own ticks (skills they say they can do, plan steps
//! they ticked) on the phone. A **progress sheet** carries them to the same waiting mailbox as reports
//! (`user/inbox/pocket/`). Nothing is kept until the host accepts it.
//!
//! ```text
//! # format: progress-sheet 1
//! # device: <the phone's code>
//! # person: <who ticked>
//! # created: 2026-11-15T20:00-05:00
//! skill TAB <skill name> TAB <day, days since 1970 (UTC)>
//! plan  TAB <plan file name>/<step number> TAB <day>
//! ```
//! Rules: a sheet is the person's whole current state, so accepting it replaces that person's earlier phone ticks (an
//! unticked step goes away). A phone tick is only what the person says: it never signs a skill off, never counts in the
//! skills grid and never changes a plan or the Inventory. The same sheet twice changes nothing. Kept in
//! `user/pocket-progress.txt`, host-only.

use crate::fsio;
use crate::security::{self, valid_id};
use crate::security_pocket as pocket;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_LINES: usize = 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct Tick {
    pub kind: String,
    pub key: String,
    pub day: i64,
}

#[derive(Debug, Clone)]
pub struct Sheet {
    pub device: String,
    pub person: String,
    pub created: String,
    pub ticks: Vec<Tick>,
    pub fingerprint: String,
}

pub fn is_sheet(text: &str) -> bool {
    text.lines().find_map(|l| l.strip_prefix("# format:")).map(|f| f.split_whitespace().next() == Some("progress-sheet")).unwrap_or(false)
}

fn clean_key(kind: &str, key: &str) -> bool {
    if key.is_empty() || key.chars().count() > 120 || key.chars().any(|c| c.is_control()) {
        return false;
    }
    if kind == "plan" {
        // "<plan file name>/<step number>"
        match key.rsplit_once('/') {
            Some((p, n)) => valid_id(p) && !n.is_empty() && n.len() <= 4 && n.bytes().all(|b| b.is_ascii_digit()),
            None => false,
        }
    } else {
        true
    }
}

pub fn parse(text: &str) -> Result<Sheet, String> {
    let (mut ok, mut device, mut person, mut created) = (false, String::new(), String::new(), String::new());
    let mut ticks: Vec<Tick> = Vec::new();
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("# format:") {
            ok = r.split_whitespace().collect::<Vec<_>>() == ["progress-sheet", "1"];
            if !ok {
                return Err("This progress sheet is from a different version of Pocket, so it was left alone.".to_string());
            }
        } else if let Some(r) = l.strip_prefix("# device:") {
            device = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# person:") {
            person = r.trim().chars().take(80).collect();
        } else if let Some(r) = l.strip_prefix("# created:") {
            created = r.trim().to_string();
        } else if l.starts_with('#') || l.trim().is_empty() {
        } else {
            let f: Vec<&str> = l.split('\t').collect();
            let day = f.get(2).and_then(|d| d.trim().parse::<i64>().ok()).filter(|d| (0..=1_000_000).contains(d));
            match (f.len() == 3 && (f[0] == "skill" || f[0] == "plan") && clean_key(f[0], f[1]), day) {
                (true, Some(day)) if !ticks.iter().any(|t| t.kind == f[0] && t.key == f[1]) => ticks.push(Tick { kind: f[0].into(), key: f[1].into(), day }),
                _ => return Err("A line in this progress sheet could not be read, so none of it can be used.".to_string()),
            }
        }
    }
    if !ok { return Err("This is not a progress sheet.".to_string()); }
    if !valid_id(&device) || device.len() > 12 { return Err("The sheet does not name its phone properly.".to_string()); }
    if person.trim().is_empty() || person.contains('\t') { return Err("The sheet does not say who ticked.".to_string()); }
    if security::parse_instant(&created).is_none() { return Err("The sheet has no readable date.".to_string()); }
    if ticks.len() > MAX_LINES { return Err("The sheet has far more lines than expected.".to_string()); }
    Ok(Sheet { device, person, created, ticks, fingerprint: pocket::fingerprint(text.as_bytes()) })
}

pub fn read(path: &Path) -> Result<Sheet, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "The file could not be opened.".to_string())?;
    if !meta.is_file() { return Err("This is not an ordinary file, so it was left alone.".to_string()); }
    if meta.len() > pocket::MAX_FILE { return Err("This file is much bigger than any progress sheet should be.".to_string()); }
    let data = fsio::read(path).map_err(|_| "The file could not be read.".to_string())?;
    parse(&String::from_utf8(data).map_err(|_| "This is not plain text.".to_string())?)
}

pub fn store_path(user_root: &Path) -> PathBuf {
    user_root.join("pocket-progress.txt")
}

/// A person's phone ticks as the host holds them: (person, device, tick), file order.
pub fn load(user_root: &Path) -> Result<Vec<(String, String, Tick)>, String> {
    let text = match fs::read_to_string(store_path(user_root)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("The phone ticks file could not be read.".to_string()),
    };
    let mut out = Vec::new();
    for l in text.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = l.split('\t').collect();
        let day = f.get(4).and_then(|d| d.parse::<i64>().ok());
        match (f.len() == 5, day) {
            (true, Some(day)) => out.push((f[0].to_string(), f[1].to_string(), Tick { kind: f[2].into(), key: f[3].into(), day })),
            _ => return Err("The phone ticks file is damaged, so nothing was changed. It was left as it is.".to_string()),
        }
    }
    Ok(out)
}

/// Accept a waiting sheet: this person's earlier ticks from that phone are replaced by the sheet's. Returns how many ticks are now kept for them.
pub fn accept(user_root: &Path, name: &str) -> Result<usize, String> {
    if !pocket::safe_name(name) { return Err("That is not a file name Pocket would make.".to_string()); }
    let path = pocket::inbox_dir(user_root).join(name);
    let sheet = read(&path)?;
    let before = pocket::accepted(user_root).iter().any(|(n, f)| n == name && *f == sheet.fingerprint);
    if !before {
        let mut all = load(user_root)?;
        all.retain(|(p, d, _)| !(*p == sheet.person && *d == sheet.device));
        for t in &sheet.ticks { all.push((sheet.person.clone(), sheet.device.clone(), t.clone())); }
        let mut text = String::new();
        for (p, d, t) in &all { text.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", p, d, t.kind, t.key, t.day)); }
        let backups = user_root.join("backups");
        fsio::backup_then_write(&store_path(user_root), &backups, text.as_bytes()).map_err(|_| "The phone ticks could not be saved.".to_string())?;
        fs::create_dir_all(crate::security_store::dir(user_root)).map_err(|_| "The security folder could not be made.".to_string())?;
        let now = security::format_instant(crate::security_api::now_minutes(), 0);
        let line = format!("{}\t{}\t{}\t{}\t{}\n", name, sheet.fingerprint, sheet.device, sheet.ticks.len(), now);
        fsio::append(&pocket::ledger_path(user_root), line.as_bytes()).map_err(|_| "The ledger could not be written.".to_string())?;
    }
    pocket::move_into(&pocket::done_dir(user_root), &path, name)?;
    Ok(sheet.ticks.len())
}

/// What one person's phone says, for the host's screens.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonTicks {
    pub person: String,
    pub skills: Vec<String>,
    pub plan_steps: usize,
}

/// The kept phone ticks, one row per person (several phones of one person are put together), in the order first seen.
pub fn summary(user_root: &Path) -> Result<Vec<PersonTicks>, String> {
    let mut out: Vec<PersonTicks> = Vec::new();
    for (person, _device, t) in load(user_root)? {
        let at = match out.iter().position(|p| p.person == person) {
            Some(i) => i,
            None => { out.push(PersonTicks { person, skills: Vec::new(), plan_steps: 0 }); out.len() - 1 }
        };
        if t.kind == "skill" { if !out[at].skills.contains(&t.key) { out[at].skills.push(t.key); } } else { out[at].plan_steps += 1; }
    }
    Ok(out)
}

pub fn render(device: &str, person: &str, created: &str, ticks: &[Tick]) -> String {
    let mut o = format!("# format: progress-sheet 1\n# device: {}\n# person: {}\n# created: {}\n", device, person, created);
    for t in ticks { o.push_str(&format!("{}\t{}\t{}\n", t.kind, t.key, t.day)); }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-prog-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(&r).unwrap();
        r
    }
    fn tk(kind: &str, key: &str, day: i64) -> Tick { Tick { kind: kind.into(), key: key.into(), day } }
    fn created() -> String { security::format_instant(crate::security_api::now_minutes(), 0) }

    #[test]
    fn a_sheet_waits_then_is_kept_and_a_newer_one_replaces_it() {
        let r = root("ok");
        pocket::drop_file(&r, "n10-progress-1.txt", &render("n10", "Sam", &created(), &[tk("skill", "Start a fire", 20000), tk("plan", "long-emergency/3", 20001)])).unwrap();
        assert_eq!(pocket::accept_any(&r, "n10-progress-1.txt", false).unwrap(), 2);
        assert_eq!(load(&r).unwrap().len(), 2);
        // the same file again changes nothing
        pocket::drop_file(&r, "n10-progress-1.txt", &render("n10", "Sam", &created(), &[tk("skill", "Start a fire", 20000), tk("plan", "long-emergency/3", 20001)])).unwrap();
        assert_eq!(pocket::accept_any(&r, "n10-progress-1.txt", false).unwrap(), 2);
        assert_eq!(load(&r).unwrap().len(), 2);
        // a newer sheet is the whole state: the unticked step goes away, another person's ticks stay
        pocket::drop_file(&r, "n11-progress-1.txt", &render("n11", "Dana", &created(), &[tk("skill", "Purify water", 20002)])).unwrap();
        pocket::accept_any(&r, "n11-progress-1.txt", false).unwrap();
        pocket::drop_file(&r, "n10-progress-2.txt", &render("n10", "Sam", &created(), &[tk("skill", "Start a fire", 20000)])).unwrap();
        pocket::accept_any(&r, "n10-progress-2.txt", false).unwrap();
        let all = load(&r).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|(p, _, t)| p == "Dana" && t.key == "Purify water"));
        assert!(all.iter().any(|(p, _, t)| p == "Sam" && t.key == "Start a fire"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_summary_puts_a_persons_phones_together_and_counts_plan_steps() {
        let r = root("sum");
        pocket::drop_file(&r, "n10-progress-1.txt", &render("n10", "Sam", &created(), &[tk("skill", "Start a fire", 1), tk("plan", "long-emergency/3", 2), tk("plan", "flood/1", 2)])).unwrap();
        pocket::drop_file(&r, "n12-progress-1.txt", &render("n12", "Sam", &created(), &[tk("skill", "Start a fire", 1), tk("skill", "Purify water", 1)])).unwrap();
        pocket::accept_any(&r, "n10-progress-1.txt", false).unwrap();
        pocket::accept_any(&r, "n12-progress-1.txt", false).unwrap();
        assert_eq!(summary(&r).unwrap(), vec![PersonTicks { person: "Sam".into(), skills: vec!["Start a fire".into(), "Purify water".into()], plan_steps: 2 }]);
        assert!(summary(&root("none")).unwrap().is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_sheet_that_cannot_be_read_changes_nothing_and_never_signs_off_a_skill() {
        let r = root("bad");
        let good = render("n10", "Sam", &created(), &[tk("skill", "Start a fire", 20000)]);
        assert!(parse(&good).is_ok());
        assert!(parse(&good.replace("skill\t", "signoff\t")).is_err());
        assert!(parse(&good.replace("20000", "soon")).is_err());
        assert!(parse(&good.replace("# person: Sam\n", "")).is_err());
        assert!(parse(&good.replace("progress-sheet 1", "progress-sheet 2")).is_err());
        assert!(parse(&render("n10", "Sam", &created(), &[tk("plan", "../x/1", 5)])).is_err());
        assert!(parse(&render("n10", "Sam", &created(), &[tk("plan", "long-emergency/3", 5), tk("plan", "long-emergency/3", 6)])).is_err());
        pocket::drop_file(&r, "n10-progress-9.txt", &good.replace("20000", "soon")).unwrap();
        assert!(pocket::accept_any(&r, "n10-progress-9.txt", false).is_err());
        assert!(load(&r).unwrap().is_empty());
        assert!(!crate::skills::Signoffs::new(&r).path.exists(), "a phone tick never signs a skill off");
        // a damaged ticks file is left alone
        fs::write(store_path(&r), "junk\n").unwrap();
        pocket::drop_file(&r, "n10-progress-3.txt", &good).unwrap();
        assert!(pocket::accept_any(&r, "n10-progress-3.txt", false).is_err());
        assert_eq!(fs::read_to_string(store_path(&r)).unwrap(), "junk\n");
        let _ = fs::remove_dir_all(&r);
    }
}
