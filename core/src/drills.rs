//! Drills: short checklists to practice before it is real, and a log of how they went.
//!
//! A drill is a Markdown file with a small header; each "- " line in the body is one step:
//! ```text
//! ---
//! title: 72-hour power outage
//! minutes: 25
//! where: at home
//! summary: Light, water, food, phones and a plan for the first three days.
//! status: draft
//! ---
//! - Find flashlights and spare batteries
//! ```
//! Results go in `drills.csv` in the user's folder.

use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Drill {
    pub id: String,
    pub title: String,
    pub minutes: u32,
    pub place: String,
    pub summary: String,
    /// "draft" until someone has reviewed the steps
    pub status: String,
    pub steps: Vec<String>,
}

pub fn parse(id: &str, text: &str) -> Result<Drill, String> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or("drill must start with ---")?;
    let (head, body) = rest.split_once("\n---\n").ok_or("drill header is not closed with ---")?;
    let mut d = Drill {
        id: id.to_string(),
        title: String::new(),
        minutes: 0,
        place: String::new(),
        summary: String::new(),
        status: "draft".into(),
        steps: Vec::new(),
    };
    for line in head.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().to_string();
            match k.trim() {
                "title" => d.title = v,
                "minutes" => d.minutes = v.parse().unwrap_or(0),
                "where" => d.place = v,
                "summary" => d.summary = v,
                "status" => d.status = v,
                _ => {}
            }
        }
    }
    for line in body.lines() {
        if let Some(s) = line.trim().strip_prefix("- ") {
            if !s.trim().is_empty() {
                d.steps.push(s.trim().to_string());
            }
        }
    }
    if d.title.is_empty() {
        return Err("drill has no title".into());
    }
    if d.steps.is_empty() {
        return Err("drill has no steps".into());
    }
    Ok(d)
}

pub fn load_dir(dir: &Path) -> Vec<Drill> {
    let mut out = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let hidden = p.file_name().map(|n| n.to_string_lossy().starts_with('_')).unwrap_or(false);
            if !hidden && p.extension().and_then(|x| x.to_str()) == Some("md") {
                let id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Ok(t) = fs::read_to_string(&p) {
                    if let Ok(d) = parse(&id, &t) {
                        out.push(d);
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Drills from several folders (shipped, the drive, the person's own). A later folder replaces an earlier drill of the same id.
pub fn load_dirs(dirs: &[std::path::PathBuf]) -> Vec<Drill> {
    let mut out: Vec<Drill> = Vec::new();
    for d in dirs {
        for x in load_dir(d) {
            match out.iter().position(|o| o.id == x.id) {
                Some(i) => out[i] = x,
                None => out.push(x),
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

// ---------- results ----------

pub const HEADER: &str = "date,drill,seconds,done,total,notes";

#[derive(Debug, Clone, PartialEq)]
pub struct Result_ {
    pub date: String,
    pub drill: String,
    pub seconds: u32,
    pub done: u32,
    pub total: u32,
    pub notes: String,
}

pub struct Log {
    pub path: PathBuf,
}

impl Log {
    pub fn new(user_dir: &Path) -> Log {
        Log { path: user_dir.join("drills.csv") }
    }

    /// A missing file is an empty log. A damaged one is an error and is never overwritten.
    pub fn list(&self) -> Result<Vec<Result_>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(h) if h.trim() == HEADER => {}
            None => return Ok(vec![]),
            _ => return Err("drills.csv has an unexpected header".into()),
        }
        let mut out = Vec::new();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 6 {
                return Err(format!("drills.csv line {} is damaged", i + 2));
            }
            let n = |s: &str| s.trim().parse::<u32>().map_err(|_| format!("drills.csv line {}: not a number: {}", i + 2, s));
            out.push(Result_ { date: f[0].clone(), drill: f[1].clone(), seconds: n(&f[2])?, done: n(&f[3])?, total: n(&f[4])?, notes: f[5].clone() });
        }
        Ok(out)
    }

    pub fn add(&self, mut r: Result_) -> Result<(), String> {
        let mut all = self.list()?;
        r.notes = r.notes.replace(['\r', '\n'], " / ").trim().to_string();
        all.push(r);
        let mut csv = String::from(HEADER);
        csv.push('\n');
        for r in &all {
            csv.push_str(&format!("{},{},{},{},{},{}\n", quote(&r.date), quote(&r.drill), r.seconds, r.done, r.total, quote(&r.notes)));
        }
        fsio::write_atomic(&self.path, csv.as_bytes()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\ntitle: Test drill\nminutes: 15\nwhere: at home\nsummary: A test.\n---\nIntro line.\n- First step\n- Second, with a comma\n";

    #[test]
    fn parses_steps_and_header() {
        let d = parse("t", SAMPLE).unwrap();
        assert_eq!((d.title.as_str(), d.minutes, d.place.as_str(), d.status.as_str()), ("Test drill", 15, "at home", "draft"));
        assert_eq!(d.steps, vec!["First step", "Second, with a comma"]);
    }

    #[test]
    fn rejects_bad_files() {
        assert!(parse("x", "no header").is_err());
        assert!(parse("x", "---\ntitle: A\n---\nno steps\n").is_err());
        assert!(parse("x", "---\nminutes: 3\n---\n- a\n").is_err());
    }

    #[test]
    fn the_shipped_drills_all_parse_and_never_give_doses() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/drills");
        let drills = load_dir(&dir);
        assert!(drills.len() >= 6, "found {}", drills.len());
        for d in &drills {
            assert!(d.minutes > 0 && !d.summary.is_empty(), "{}", d.id);
            assert!(d.status == "draft" || d.status == "reviewed");
            for s in &d.steps {
                assert!(!crate::safety::output_contains_dose(s), "step looks like a dosing instruction: {}", s);
            }
        }
    }

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-drill-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn results_round_trip_and_notes_stay_on_one_line() {
        let d = tmp("log");
        let log = Log::new(&d);
        assert!(log.list().unwrap().is_empty());
        log.add(Result_ { date: "2026-10-18".into(), drill: "power".into(), seconds: 754, done: 7, total: 8, notes: "No \"spare\" batteries,\nneed lantern".into() }).unwrap();
        log.add(Result_ { date: "2026-10-19".into(), drill: "water".into(), seconds: 60, done: 5, total: 5, notes: String::new() }).unwrap();
        let l = log.list().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].notes, "No \"spare\" batteries, / need lantern");
        assert_eq!(l[1].done, 5);
    }

    #[test]
    fn damaged_log_is_an_error_not_wiped() {
        let d = tmp("bad");
        fs::write(d.join("drills.csv"), "wrong,header\n1,2\n").unwrap();
        let log = Log::new(&d);
        assert!(log.list().is_err());
        assert!(log.add(Result_ { date: "d".into(), drill: "x".into(), seconds: 1, done: 1, total: 1, notes: String::new() }).is_err());
        assert!(fs::read_to_string(d.join("drills.csv")).unwrap().contains("wrong,header"));
    }

    #[test]
    fn templates_are_skipped_and_a_later_folder_wins() {
        let base = std::env::temp_dir().join(format!("cx-drills-{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let d = |t: &str| format!("---\ntitle: {}\nminutes: 5\nwhere: at home\nstatus: draft\n---\n- one thing\n", t);
        std::fs::write(a.join("x.md"), d("Old")).unwrap();
        std::fs::write(a.join("_template-drill.md"), d("Template")).unwrap();
        std::fs::write(b.join("x.md"), d("New")).unwrap();
        let l = load_dirs(&[a, b]);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].title, "New");
        let _ = std::fs::remove_dir_all(&base);
    }
}
