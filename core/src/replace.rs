//! What replaces it: for each thing a household stores, what takes its place when the store is gone, which skill that
//! needs, and how long it takes to set up. The list ships in `content/replacements.csv`
//! (`need,stored_as,replaced_by,skill,lead_time`). The family's own state for each need (not started, learning, in
//! place) is kept in `user/replacements.csv` (`need,state`).

use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

const LIST_HEADER: &str = "need,stored_as,replaced_by,skill,lead_time";
pub const STATE_HEADER: &str = "need,state";
pub const STATES: [&str; 3] = ["notstarted", "learning", "inplace"];

#[derive(Debug, Clone, PartialEq)]
pub struct Replacement {
    pub need: String,
    /// Matched against inventory names and categories to find how long the stored supply lasts.
    pub stored_as: String,
    pub replaced_by: String,
    pub skill: String,
    pub lead_time: String,
    pub lead_days: u32,
}

/// "2 weeks", "1 season", "3 months", "1 year", "5 days". A season is about 120 days.
pub fn lead_days(text: &str) -> u32 {
    let t = text.trim().to_lowercase();
    let mut parts = t.split_whitespace();
    let n: u32 = match parts.next() {
        Some(w) => w.parse().unwrap_or(match w { "a" | "one" => 1, "two" => 2, "three" => 3, _ => 0 }),
        None => return 0,
    };
    let unit = parts.next().unwrap_or("");
    let per = if unit.starts_with("day") {
        1
    } else if unit.starts_with("week") {
        7
    } else if unit.starts_with("month") {
        30
    } else if unit.starts_with("season") {
        120
    } else if unit.starts_with("year") {
        365
    } else {
        0
    };
    n * per
}

pub fn parse_list(text: &str) -> Result<Vec<Replacement>, String> {
    let mut lines = text.lines();
    match lines.next() {
        Some(h) if h.trim() == LIST_HEADER => {}
        None => return Ok(vec![]),
        _ => return Err("The replacements list has an unexpected first line.".into()),
    }
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let f = split_record(l);
        if f.len() != 5 || f[0].trim().is_empty() {
            return Err(format!("The replacements list, line {}, is damaged.", i + 2));
        }
        out.push(Replacement {
            need: f[0].trim().into(),
            stored_as: f[1].trim().into(),
            replaced_by: f[2].trim().into(),
            skill: f[3].trim().into(),
            lead_time: f[4].trim().into(),
            lead_days: lead_days(&f[4]),
        });
    }
    Ok(out)
}

/// The shipped list, then the family's own. A need with the same name in a later list replaces the earlier one.
pub fn load(paths: &[PathBuf]) -> Result<Vec<Replacement>, String> {
    let mut out: Vec<Replacement> = Vec::new();
    for p in paths {
        let text = match fs::read_to_string(p) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        for r in parse_list(&text)? {
            match out.iter().position(|o| o.need == r.need) {
                Some(i) => out[i] = r,
                None => out.push(r),
            }
        }
    }
    Ok(out)
}

/// The family's state for each need. Needs not listed are "notstarted".
pub struct States {
    pub path: PathBuf,
    backups: PathBuf,
}

impl States {
    pub fn new(user_dir: &Path) -> States {
        States { path: user_dir.join("replacements.csv"), backups: user_dir.join("backups") }
    }

    pub fn list(&self) -> Result<Vec<(String, String)>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(h) if h.trim() == STATE_HEADER => {}
            None => return Ok(vec![]),
            _ => return Err("replacements.csv has an unexpected header".into()),
        }
        let mut out = Vec::new();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 2 || !STATES.contains(&f[1].as_str()) {
                return Err(format!("replacements.csv line {} is damaged", i + 2));
            }
            out.push((f[0].clone(), f[1].clone()));
        }
        Ok(out)
    }

    /// Set a need to "notstarted", "learning" or "inplace". The earlier file is kept in the backups folder.
    pub fn set(&self, need: &str, state: &str) -> Result<(), String> {
        if need.trim().is_empty() {
            return Err("Choose a need.".into());
        }
        if !STATES.contains(&state) {
            return Err("That is not a state this page knows.".into());
        }
        let mut all = self.list()?;
        all.retain(|(n, _)| n != need);
        if state != "notstarted" {
            all.push((need.into(), state.into()));
        }
        let mut csv = String::from(STATE_HEADER);
        csv.push('\n');
        for (n, s) in &all {
            csv.push_str(&format!("{},{}\n", quote(n), quote(s)));
        }
        fsio::backup_then_write(&self.path, &self.backups, csv.as_bytes()).map_err(|e| e.to_string())
    }
}

pub fn state_of(states: &[(String, String)], need: &str) -> &'static str {
    match states.iter().find(|(n, _)| n == need).map(|(_, s)| s.as_str()) {
        Some("inplace") => "inplace",
        Some("learning") => "learning",
        _ => "notstarted",
    }
}

/// A plain warning when the supply will be gone before its replacement could be ready.
/// `supply_days` is how long the stored supply lasts (None when unknown). Nothing is said for a need already in place.
pub fn warning(r: &Replacement, state: &str, supply_days: Option<f64>) -> Option<String> {
    if state == "inplace" {
        return None;
    }
    let days = supply_days?;
    if (r.lead_days as f64) > days {
        let how = match r.lead_days {
            d if d >= 300 => "takes a year".to_string(),
            d if d >= 100 => "takes a season".to_string(),
            _ => format!("takes about {}", r.lead_time),
        };
        Some(format!("Start now: this {} and your stored supply lasts {}.", how, if days < 1.0 { "less than a day".to_string() } else { format!("about {} days", days.round()) }))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-repl-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lead_times_become_days() {
        assert_eq!(lead_days("2 weeks"), 14);
        assert_eq!(lead_days("1 season"), 120);
        assert_eq!(lead_days("a year"), 365);
        assert_eq!(lead_days("3 months"), 90);
        assert_eq!(lead_days("5 days"), 5);
        assert_eq!(lead_days("soon"), 0);
        assert_eq!(lead_days(""), 0);
    }

    #[test]
    fn the_shipped_list_reads_and_every_need_has_a_known_skill() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content");
        let l = load(&[base.join("replacements.csv")]).unwrap();
        assert!(l.len() >= 12, "{}", l.len());
        let skills = crate::skills::load(&[base.join("skills.csv")]).unwrap();
        for r in &l {
            assert!(r.lead_days > 0, "{} has no lead time", r.need);
            assert!(skills.iter().any(|s| s.skill == r.skill), "{} names a skill that is not in skills.csv: {}", r.need, r.skill);
        }
    }

    #[test]
    fn the_warning_only_comes_when_the_supply_is_shorter_than_the_setup() {
        let r = Replacement { need: "Vegetables".into(), stored_as: "Canned".into(), replaced_by: "Garden".into(), skill: "Garden".into(), lead_time: "1 season".into(), lead_days: 120 };
        assert!(warning(&r, "notstarted", Some(30.0)).unwrap().starts_with("Start now"));
        assert!(warning(&r, "learning", Some(30.0)).is_some());
        assert!(warning(&r, "inplace", Some(30.0)).is_none());
        assert!(warning(&r, "notstarted", Some(400.0)).is_none());
        assert!(warning(&r, "notstarted", None).is_none());
    }

    #[test]
    fn state_round_trips_with_a_backup_and_a_damaged_file_is_left_alone() {
        let d = tmp("state");
        let s = States::new(&d);
        assert!(s.list().unwrap().is_empty());
        assert!(s.set("Soap", "done").is_err());
        s.set("Soap, bar", "learning").unwrap();
        s.set("Fire", "inplace").unwrap();
        s.set("Soap, bar", "inplace").unwrap();
        let l = s.list().unwrap();
        assert_eq!(state_of(&l, "Soap, bar"), "inplace");
        assert_eq!(state_of(&l, "Fire"), "inplace");
        assert_eq!(state_of(&l, "Water"), "notstarted");
        s.set("Fire", "notstarted").unwrap();
        assert_eq!(s.list().unwrap().len(), 1);
        fs::write(&s.path, "bad,header\n1,2\n").unwrap();
        assert!(s.list().is_err());
        assert!(s.set("Fire", "inplace").is_err());
        assert!(fs::read_to_string(&s.path).unwrap().contains("bad,header"));
    }
}
