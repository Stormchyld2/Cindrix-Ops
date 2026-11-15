//! The skills matrix: the people on the Family page against the skills a group needs, so you can see at a glance
//! what is covered, what only one person can do, and what nobody can do yet.
//!
//! The list ships in `content/skills.csv` (`skill,area,proof,must_have`). A family can add its own skills in
//! `user/skills.csv`, same shape. `proof` says what shows a skill: `course:<id>` (the Academy course is signed off or
//! finished), `drill:<id>` (the drill is done at least once a year) or `hand` (someone signs it off by hand).
//! A hand sign-off, in `user/skills-signoff.csv` (`person,skill,signed_by,signed_date`), counts for any skill.
//! The drill log has no names, so a drill gives the group a yearly "done" flag, and a person is marked by a sign-off.

use crate::calendar;
use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

pub const SIGNOFF_HEADER: &str = "person,skill,signed_by,signed_date";
const LIST_HEADER: &str = "skill,area,proof,must_have";

#[derive(Debug, Clone, PartialEq)]
pub enum Proof {
    Course(String),
    Drill(String),
    Hand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillDef {
    pub skill: String,
    pub area: String,
    pub proof: Proof,
    pub must_have: bool,
}

pub fn parse_list(text: &str) -> Result<Vec<SkillDef>, String> {
    let mut lines = text.lines();
    match lines.next() {
        Some(h) if h.trim() == LIST_HEADER => {}
        None => return Ok(vec![]),
        _ => return Err("The skills list has an unexpected first line.".into()),
    }
    let mut out = Vec::new();
    for (i, l) in lines.enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let f = split_record(l);
        if f.len() != 4 || f[0].trim().is_empty() {
            return Err(format!("The skills list, line {}, is damaged.", i + 2));
        }
        let p = f[2].trim();
        let proof = if let Some(c) = p.strip_prefix("course:") {
            Proof::Course(c.trim().to_string())
        } else if let Some(d) = p.strip_prefix("drill:") {
            Proof::Drill(d.trim().to_string())
        } else {
            Proof::Hand
        };
        out.push(SkillDef { skill: f[0].trim().into(), area: f[1].trim().into(), proof, must_have: matches!(f[3].trim().to_lowercase().as_str(), "yes" | "y" | "true" | "1") });
    }
    Ok(out)
}

/// The shipped list, then the family's own. A skill with the same name in a later list replaces the earlier one.
pub fn load(paths: &[PathBuf]) -> Result<Vec<SkillDef>, String> {
    let mut out: Vec<SkillDef> = Vec::new();
    for p in paths {
        let text = match fsio::read_to_string(p) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        for d in parse_list(&text)? {
            match out.iter().position(|o| o.skill == d.skill) {
                Some(i) => out[i] = d,
                None => out.push(d),
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Signoff {
    pub person: String,
    pub skill: String,
    pub signed_by: String,
    pub signed_date: String,
}

pub struct Signoffs {
    pub path: PathBuf,
    backups: PathBuf,
}

impl Signoffs {
    pub fn new(user_dir: &Path) -> Signoffs {
        Signoffs { path: user_dir.join("skills-signoff.csv"), backups: user_dir.join("backups") }
    }

    /// A missing file is an empty list. A damaged one is an error and is never overwritten.
    pub fn list(&self) -> Result<Vec<Signoff>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(h) if h.trim() == SIGNOFF_HEADER => {}
            None => return Ok(vec![]),
            _ => return Err("skills-signoff.csv has an unexpected header".into()),
        }
        let mut out = Vec::new();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 4 {
                return Err(format!("skills-signoff.csv line {} is damaged", i + 2));
            }
            out.push(Signoff { person: f[0].clone(), skill: f[1].clone(), signed_by: f[2].clone(), signed_date: f[3].clone() });
        }
        Ok(out)
    }

    /// Sign a skill off for a person, or take the sign-off back. The earlier file is kept in the backups folder.
    pub fn set(&self, person: &str, skill: &str, by: &str, date: &str, on: bool) -> Result<(), String> {
        if person.trim().is_empty() || skill.trim().is_empty() {
            return Err("Choose a person and a skill.".into());
        }
        if on && by.trim().is_empty() {
            return Err("Write who is signing this off.".into());
        }
        let mut all = self.list()?;
        all.retain(|e| !(e.person == person && e.skill == skill));
        if on {
            all.push(Signoff { person: person.into(), skill: skill.into(), signed_by: by.trim().into(), signed_date: date.into() });
        }
        let mut csv = String::from(SIGNOFF_HEADER);
        csv.push('\n');
        for e in &all {
            csv.push_str(&format!("{},{},{},{}\n", quote(&e.person), quote(&e.skill), quote(&e.signed_by), quote(&e.signed_date)));
        }
        fsio::backup_then_write(&self.path, &self.backups, csv.as_bytes()).map_err(|e| e.to_string())
    }
}

/// What the grid needs to know about one person, worked out by the caller from the Academy.
#[derive(Debug, Clone, Default)]
pub struct PersonFacts {
    pub name: String,
    /// Courses signed off or finished.
    pub courses_done: Vec<String>,
    /// Courses followed or started but not finished.
    pub courses_started: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub skill: String,
    pub area: String,
    pub must_have: bool,
    pub proof: String,
    /// One per person, in order: "signed", "learning" or "none".
    pub cells: Vec<&'static str>,
    pub signed: usize,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub people: Vec<String>,
    pub rows: Vec<Row>,
    /// How many people must be able to do a must-have skill for it to count as covered.
    pub bar: usize,
    pub must_total: usize,
    pub must_covered: usize,
}

/// Two people for a group of two or more, one for a person on their own.
pub fn coverage_bar(people: usize) -> usize {
    people.clamp(1, 2)
}

/// `drills_done` is (drill id, date as YYYY-MM-DD) for every recorded run.
pub fn build(defs: &[SkillDef], people: &[PersonFacts], signoffs: &[Signoff], drills_done: &[(String, String)], today: i64) -> Grid {
    let bar = coverage_bar(people.len());
    let mut rows = Vec::new();
    for d in defs {
        let mut cells = Vec::new();
        for p in people {
            let by_hand = signoffs.iter().any(|s| s.person == p.name && s.skill == d.skill);
            let state = if by_hand {
                "signed"
            } else {
                match &d.proof {
                    Proof::Course(c) if p.courses_done.contains(c) => "signed",
                    Proof::Course(c) if p.courses_started.contains(c) => "learning",
                    _ => "none",
                }
            };
            cells.push(state);
        }
        let signed = cells.iter().filter(|c| **c == "signed").count();
        let mut flags = Vec::new();
        if !people.is_empty() {
            if signed == 0 {
                flags.push("Nobody can do this yet".to_string());
            } else if signed == 1 && people.len() > 1 {
                flags.push("Only one person can do this".to_string());
            }
        }
        let proof = match &d.proof {
            Proof::Course(c) => format!("Course: {}", c),
            Proof::Drill(x) => {
                let recent = drills_done.iter().any(|(id, date)| id == x && calendar::parse_date(date).map_or(false, |dd| today - dd <= 365));
                if !recent {
                    flags.push("Drill not done this year".to_string());
                }
                format!("Drill: {}", x)
            }
            Proof::Hand => "Signed off by hand".to_string(),
        };
        rows.push(Row { skill: d.skill.clone(), area: d.area.clone(), must_have: d.must_have, proof, cells, signed, flags });
    }
    let must_total = rows.iter().filter(|r| r.must_have).count();
    let must_covered = rows.iter().filter(|r| r.must_have && r.signed >= bar).count();
    Grid { people: people.iter().map(|p| p.name.clone()).collect(), rows, bar, must_total, must_covered }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-skills-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn defs() -> Vec<SkillDef> {
        parse_list("skill,area,proof,must_have\nWater,Water,course:water,yes\nFire,Warmth,drill:fire,yes\nSoap,Trades,hand,no\n").unwrap()
    }

    fn person(n: &str, done: &[&str], started: &[&str]) -> PersonFacts {
        PersonFacts { name: n.into(), courses_done: done.iter().map(|s| s.to_string()).collect(), courses_started: started.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn the_list_reads_and_its_kinds_of_proof_are_told_apart() {
        let d = defs();
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].proof, Proof::Course("water".into()));
        assert_eq!(d[1].proof, Proof::Drill("fire".into()));
        assert_eq!(d[2].proof, Proof::Hand);
        assert!(d[0].must_have && !d[2].must_have);
        assert!(parse_list("wrong\n").is_err());
        assert!(parse_list("skill,area,proof,must_have\nonly,two\n").is_err());
    }

    #[test]
    fn the_shipped_list_reads_and_each_course_and_drill_it_names_exists() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content");
        let defs = load(&[base.join("skills.csv")]).unwrap();
        assert!(defs.len() >= 40, "{}", defs.len());
        let courses: Vec<String> = fs::read_dir(base.join("academy")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        let drills: Vec<String> = fs::read_dir(base.join("drills")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().trim_end_matches(".md").to_string()).collect();
        for d in &defs {
            match &d.proof {
                Proof::Course(c) => assert!(courses.contains(c), "{} names a course that is not there: {}", d.skill, c),
                Proof::Drill(x) => assert!(drills.contains(x), "{} names a drill that is not there: {}", d.skill, x),
                Proof::Hand => {}
            }
        }
        assert!(defs.iter().filter(|d| d.must_have).count() >= 30);
    }

    #[test]
    fn a_later_list_adds_and_replaces() {
        let d = tmp("later");
        fs::write(d.join("a.csv"), "skill,area,proof,must_have\nWater,Water,hand,yes\n").unwrap();
        fs::write(d.join("b.csv"), "skill,area,proof,must_have\nWater,Water,hand,no\nBeekeeping,Food,hand,no\n").unwrap();
        let l = load(&[d.join("a.csv"), d.join("b.csv"), d.join("missing.csv")]).unwrap();
        assert_eq!(l.len(), 2);
        assert!(!l[0].must_have);
    }

    #[test]
    fn coverage_counts_people_and_flags_the_weak_rows() {
        let people = vec![person("Sam", &["water"], &[]), person("Alex", &[], &["water"])];
        let g = build(&defs(), &people, &[], &[], 20000);
        assert_eq!(g.bar, 2);
        assert_eq!(g.rows[0].cells, vec!["signed", "learning"]);
        assert_eq!(g.rows[0].flags, vec!["Only one person can do this".to_string()]);
        assert!(g.rows[1].flags.contains(&"Nobody can do this yet".to_string()));
        assert!(g.rows[1].flags.contains(&"Drill not done this year".to_string()));
        assert_eq!(g.must_total, 2);
        assert_eq!(g.must_covered, 0);
    }

    #[test]
    fn a_hand_sign_off_counts_and_two_people_cover_a_skill() {
        let people = vec![person("Sam", &["water"], &[]), person("Alex", &[], &[])];
        let sign = vec![Signoff { person: "Alex".into(), skill: "Water".into(), signed_by: "Sam".into(), signed_date: "2026-11-01".into() }];
        let g = build(&defs(), &people, &sign, &[("fire".into(), "2026-10-01".into())], calendar::parse_date("2026-11-15").unwrap());
        assert_eq!(g.rows[0].signed, 2);
        assert!(g.rows[0].flags.is_empty());
        assert!(!g.rows[1].flags.contains(&"Drill not done this year".to_string()));
        assert_eq!(g.must_covered, 1);
    }

    #[test]
    fn an_old_drill_run_does_not_count() {
        let people = vec![person("Sam", &[], &[])];
        let g = build(&defs(), &people, &[], &[("fire".into(), "2024-01-01".into())], calendar::parse_date("2026-11-15").unwrap());
        assert!(g.rows[1].flags.contains(&"Drill not done this year".to_string()));
    }

    #[test]
    fn one_person_alone_needs_only_themselves_and_is_not_told_only_one_can() {
        let people = vec![person("Sam", &["water"], &[])];
        let g = build(&defs(), &people, &[], &[], 20000);
        assert_eq!(g.bar, 1);
        assert!(g.rows[0].flags.is_empty());
        assert_eq!(g.must_covered, 1);
    }

    #[test]
    fn sign_offs_round_trip_keep_a_backup_and_protect_a_damaged_file() {
        let d = tmp("sign");
        let s = Signoffs::new(&d);
        assert!(s.list().unwrap().is_empty());
        assert!(s.set("Sam", "Fire", "", "2026-11-01", true).is_err());
        s.set("Sam", "Fire, in rain", "Alex", "2026-11-01", true).unwrap();
        s.set("Alex", "Fire, in rain", "Sam", "2026-11-02", true).unwrap();
        assert_eq!(s.list().unwrap().len(), 2);
        s.set("Sam", "Fire, in rain", "", "", false).unwrap();
        let l = s.list().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].person, "Alex");
        fs::write(&s.path, "wrong,header\n1,2\n").unwrap();
        assert!(s.list().is_err());
        assert!(s.set("Sam", "Fire", "Alex", "d", true).is_err());
        assert!(fs::read_to_string(&s.path).unwrap().contains("wrong,header"));
    }
}
