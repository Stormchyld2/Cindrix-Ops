//! Training sign-off: for each person, which Academy courses they follow, which lessons they have
//! passed, what is left, and what to take next. Everything is worked out from the Academy folders and
//! the quiz results, so it follows the courses on the drive with nothing to set up by hand.
//!
//! Only two small things are kept here, in `user/training.csv`: which person follows which course,
//! and who signed a course off, and when. A lesson counts as learned when the best quiz score is at
//! least `PASS_PERCENT`. A lesson with no quiz cannot be tested, so it is shown but not counted.

use crate::academy::{Course, Result_};
use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

pub const PASS_PERCENT: u32 = 70;
pub const HEADER: &str = "person,course,signed_by,signed_date";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Enrol {
    pub person: String,
    pub course: String,
    pub signed_by: String,
    pub signed_date: String,
}

pub struct Roster {
    pub path: PathBuf,
}

impl Roster {
    pub fn new(user_dir: &Path) -> Roster {
        Roster { path: user_dir.join("training.csv") }
    }

    /// A missing file is an empty roster. A damaged one is an error and is never overwritten.
    pub fn list(&self) -> Result<Vec<Enrol>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(h) if h.trim() == HEADER => {}
            None => return Ok(vec![]),
            _ => return Err("training.csv has an unexpected header".into()),
        }
        let mut out = Vec::new();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 4 {
                return Err(format!("training.csv line {} is damaged", i + 2));
            }
            out.push(Enrol { person: f[0].clone(), course: f[1].clone(), signed_by: f[2].clone(), signed_date: f[3].clone() });
        }
        Ok(out)
    }

    fn save(&self, all: &[Enrol]) -> Result<(), String> {
        let mut csv = String::from(HEADER);
        csv.push('\n');
        for e in all {
            csv.push_str(&format!("{},{},{},{}\n", quote(&e.person), quote(&e.course), quote(&e.signed_by), quote(&e.signed_date)));
        }
        fsio::write_atomic(&self.path, csv.as_bytes()).map_err(|e| e.to_string())
    }

    /// Put a person on a course, or take them off it (their quiz results are kept either way).
    pub fn set_enrolled(&self, person: &str, course: &str, on: bool) -> Result<(), String> {
        if person.trim().is_empty() || course.trim().is_empty() {
            return Err("Choose a person and a course.".into());
        }
        let mut all = self.list()?;
        all.retain(|e| !(e.person == person && e.course == course));
        if on {
            all.push(Enrol { person: person.into(), course: course.into(), ..Default::default() });
        }
        self.save(&all)
    }

    /// Record that `by` signed the course off for the person on `date`, or take the sign-off back.
    pub fn set_signed(&self, person: &str, course: &str, by: &str, date: &str, on: bool) -> Result<(), String> {
        let mut all = self.list()?;
        if !all.iter().any(|e| e.person == person && e.course == course) {
            all.push(Enrol { person: person.into(), course: course.into(), ..Default::default() });
        }
        for e in all.iter_mut().filter(|e| e.person == person && e.course == course) {
            if on {
                if by.trim().is_empty() {
                    return Err("Write who is signing this off.".into());
                }
                e.signed_by = by.trim().into();
                e.signed_date = date.into();
            } else {
                e.signed_by.clear();
                e.signed_date.clear();
            }
        }
        self.save(&all)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LessonState {
    pub id: String,
    pub title: String,
    pub module: String,
    pub minutes: u32,
    pub questions: u32,
    /// "passed", "tried" (taken, not yet good enough), "new" or "noquiz"
    pub state: &'static str,
    pub best_score: u32,
    pub total: u32,
    pub last_date: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CourseState {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub chosen: bool,
    pub signed_by: String,
    pub signed_date: String,
    pub lessons: Vec<LessonState>,
    /// Lessons with a quiz that have been passed, and all lessons with a quiz.
    pub passed: u32,
    pub needed: u32,
    pub minutes_left: u32,
}

impl CourseState {
    pub fn complete(&self) -> bool {
        self.needed > 0 && self.passed >= self.needed
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub course: String,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub person: String,
    pub courses: Vec<CourseState>,
    pub suggestions: Vec<Suggestion>,
    /// Over the courses the person follows.
    pub passed: u32,
    pub needed: u32,
    pub signed: u32,
    pub chosen: u32,
}

fn state_of(best: u32, total: u32, tries: usize) -> &'static str {
    if total == 0 && tries == 0 {
        "new"
    } else if total > 0 && best * 100 >= PASS_PERCENT * total {
        "passed"
    } else if tries > 0 {
        "tried"
    } else {
        "new"
    }
}

pub fn report(courses: &[Course], results: &[Result_], roster: &[Enrol], person: &str) -> Report {
    let mine: Vec<Result_> = results.iter().filter(|r| r.person == person).cloned().collect();
    let mut states: Vec<CourseState> = Vec::new();
    for c in courses {
        let enrol = roster.iter().find(|e| e.person == person && e.course == c.id);
        let mut lessons = Vec::new();
        for m in &c.modules {
            for l in &m.lessons {
                let q = l.quiz.len() as u32;
                let p = crate::academy::progress_of(&mine, &l.id, Some(person));
                let state = if q == 0 { "noquiz" } else { state_of(p.best_score, p.total, p.tries) };
                lessons.push(LessonState {
                    id: l.id.clone(), title: l.title.clone(), module: m.title.clone(), minutes: l.minutes, questions: q, state,
                    best_score: p.best_score, total: p.total, last_date: p.last_date,
                });
            }
        }
        let needed = lessons.iter().filter(|l| l.state != "noquiz").count() as u32;
        let passed = lessons.iter().filter(|l| l.state == "passed").count() as u32;
        let minutes_left = lessons.iter().filter(|l| l.state == "new" || l.state == "tried").map(|l| l.minutes).sum();
        states.push(CourseState {
            id: c.id.clone(), title: c.title.clone(), summary: c.summary.clone(),
            chosen: enrol.is_some() || mine.iter().any(|r| lessons.iter().any(|l| l.id == r.lesson)),
            signed_by: enrol.map(|e| e.signed_by.clone()).unwrap_or_default(),
            signed_date: enrol.map(|e| e.signed_date.clone()).unwrap_or_default(),
            lessons, passed, needed, minutes_left,
        });
    }
    states.sort_by(|a, b| a.id.cmp(&b.id));
    // A course the person has quizzed in counts as followed, but a sign-off on its own is not needed to follow it.
    let followed: Vec<&CourseState> = states.iter().filter(|c| c.chosen).collect();
    let (passed, needed) = (followed.iter().map(|c| c.passed).sum(), followed.iter().map(|c| c.needed).sum());
    let signed = followed.iter().filter(|c| !c.signed_date.is_empty()).count() as u32;
    let chosen = followed.len() as u32;

    let mut suggestions: Vec<Suggestion> = Vec::new();
    // 1. a course already started: finish it
    let mut going: Vec<&CourseState> = states.iter().filter(|c| c.chosen && c.needed > 0 && !c.complete()).collect();
    going.sort_by_key(|c| (c.needed - c.passed, c.id.clone()));
    for c in going.iter().take(2) {
        let left = c.needed - c.passed;
        suggestions.push(Suggestion { course: c.id.clone(), title: c.title.clone(), reason: format!("Keep going: {} lesson{} left, about {} minutes.", left, if left == 1 { "" } else { "s" }, c.minutes_left) });
    }
    // 2. then the next one nobody has begun, in the usual order
    for c in states.iter().filter(|c| !c.chosen && c.needed > 0) {
        if suggestions.len() >= 3 {
            break;
        }
        suggestions.push(Suggestion { course: c.id.clone(), title: c.title.clone(), reason: if followed.is_empty() { "A good place to start.".into() } else { "Next in the usual order.".into() } });
    }
    Report { person: person.into(), courses: states, suggestions, passed, needed, signed, chosen }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::academy::{parse_lesson, Module};

    const LESSON: &str = "---\ntitle: Water\nminutes: 8\n---\nText.\n\n## Check yourself\n? Q1\n- a\n+ b\n? Q2\n+ a\n- b\n? Q3\n+ a\n- b\n";
    const NO_QUIZ: &str = "---\ntitle: Read only\nminutes: 5\n---\nJust text.\n";

    fn course(id: &str, lessons: &[(&str, &str)]) -> Course {
        Course {
            id: id.into(), title: format!("Course {}", id), summary: String::new(), author: String::new(), version: 1, license: String::new(),
            modules: vec![Module { id: "m".into(), title: "M".into(), summary: String::new(), lessons: lessons.iter().map(|(i, t)| parse_lesson(&format!("{}/{}", id, i), t).unwrap()).collect() }],
        }
    }
    fn r(lesson: &str, score: u32, person: &str) -> Result_ {
        Result_ { date: "2026-10-22".into(), lesson: lesson.into(), score, total: 3, person: person.into() }
    }
    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-training-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn each_person_has_their_own_progress_and_a_pass_needs_70_percent() {
        let cs = vec![course("01-a", &[("l1", LESSON), ("l2", LESSON), ("l3", NO_QUIZ)])];
        let results = vec![r("01-a/l1", 2, "Ann"), r("01-a/l1", 3, "Ben"), r("01-a/l2", 1, "Ann")];
        let a = report(&cs, &results, &[], "Ann");
        let ls = &a.courses[0].lessons;
        // 2 of 3 is 66%: not yet; 1 of 3: tried; no quiz: not counted
        assert_eq!((ls[0].state, ls[1].state, ls[2].state), ("tried", "tried", "noquiz"));
        assert_eq!((a.courses[0].passed, a.courses[0].needed), (0, 2));
        assert!(a.courses[0].chosen, "quizzes taken count as following the course");
        let b = report(&cs, &results, &[], "Ben");
        assert_eq!(b.courses[0].lessons[0].state, "passed");
        assert_eq!(b.courses[0].lessons[1].state, "new");
        assert_eq!(b.courses[0].minutes_left, 8);
        let c = report(&cs, &results, &[], "Cy");
        assert!(!c.courses[0].chosen);
        assert_eq!((c.chosen, c.passed, c.needed), (0, 0, 0));
    }

    #[test]
    fn suggestions_finish_what_is_started_then_go_in_order() {
        let cs = vec![course("01-a", &[("l1", LESSON), ("l2", LESSON)]), course("02-b", &[("l1", LESSON)]), course("03-c", &[("l1", LESSON)])];
        let roster = vec![Enrol { person: "Ann".into(), course: "02-b".into(), ..Default::default() }];
        let rep = report(&cs, &[], &roster, "Ann");
        assert_eq!(rep.suggestions[0].course, "02-b");
        assert!(rep.suggestions[0].reason.starts_with("Keep going"));
        assert_eq!(rep.suggestions[1].course, "01-a");
        assert_eq!(rep.suggestions[1].reason, "Next in the usual order.");
        assert_eq!(rep.suggestions.len(), 3);
        // someone with nothing started is told where to begin
        let fresh = report(&cs, &[], &[], "Zed");
        assert_eq!((fresh.suggestions[0].course.as_str(), fresh.suggestions[0].reason.as_str()), ("01-a", "A good place to start."));
        // a finished course is no longer suggested
        let done = report(&cs, &[r("02-b/l1", 3, "Ann")], &roster, "Ann");
        assert!(done.courses[1].complete());
        assert!(done.suggestions.iter().all(|s| s.course != "02-b"));
    }

    #[test]
    fn the_roster_saves_enrols_and_signs_off_and_leaves_a_damaged_file_alone() {
        let d = tmp("roster");
        let ro = Roster::new(&d);
        assert!(ro.list().unwrap().is_empty());
        ro.set_enrolled("Ann, Jr.", "01-a", true).unwrap();
        ro.set_enrolled("Ben", "01-a", true).unwrap();
        assert!(ro.set_signed("Ann, Jr.", "01-a", " ", "2026-10-22", true).is_err());
        ro.set_signed("Ann, Jr.", "01-a", "Dad", "2026-10-22", true).unwrap();
        let l = ro.list().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].person.as_str(), l[0].signed_by.as_str(), l[0].signed_date.as_str()), ("Ann, Jr.", "Dad", "2026-10-22"));
        let cs = vec![course("01-a", &[("l1", LESSON)])];
        let rep = report(&cs, &[], &l, "Ann, Jr.");
        assert_eq!((rep.chosen, rep.signed), (1, 1));
        ro.set_signed("Ann, Jr.", "01-a", "", "", false).unwrap();
        assert!(ro.list().unwrap()[0].signed_date.is_empty());
        ro.set_enrolled("Ben", "01-a", false).unwrap();
        assert_eq!(ro.list().unwrap().len(), 1);
        fs::write(&ro.path, "person,course,signed_by,signed_date\nonly,two\n").unwrap();
        assert!(ro.list().is_err());
        assert!(ro.set_enrolled("X", "01-a", true).is_err());
        assert_eq!(fs::read_to_string(&ro.path).unwrap(), "person,course,signed_by,signed_date\nonly,two\n");
    }

    #[test]
    fn old_academy_results_without_a_person_still_load() {
        let d = tmp("v1");
        let log = crate::academy::Log::new(&d);
        fs::write(&log.path, "date,lesson,score,total\n2026-10-01,m/l,2,3\n").unwrap();
        let all = log.list().unwrap();
        assert_eq!((all[0].person.as_str(), all[0].score), ("", 2));
        log.add(Result_ { date: "2026-10-02".into(), lesson: "m/l".into(), score: 3, total: 3, person: "Ann".into() }).unwrap();
        let all = log.list().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].person, "Ann");
        assert!(fs::read_to_string(&log.path).unwrap().starts_with("date,lesson,score,total,person\n"));
    }
}
