//! Exercises and after-action reviews (AARs): doc 44 sections 8, 9 and 13 (phase 4).
//!
//! An **exercise** is practice: what was practised, when, where and who took part. An **after-action review**
//! looks back at an incident or an exercise: what went well, what to fix, and a list of actions, each with an
//! owner and a due date. An action is done when its owner says so and verified when someone checks it, and a review
//! can only be closed once every action is verified or dropped.
//!
//! One append-only log, `user/security/review-log.txt`. A line is never changed: a correction or a change of state is a
//! new line. A log that cannot be read is never added to. An exercise is never counted as a live incident, and
//! nothing here starts, ends or changes an incident.

use crate::ops::{esc, unesc};
use crate::security::{format_instant, valid_id, Parsed};
use crate::security_comms::{clean, read_text, rows};
use crate::security_incident::Incident;
use crate::security_store as store;
use std::path::{Path, PathBuf};

/// A site is nudged about a new exercise when the last one was more than this many days ago, but only once it has
/// had a first one (the program does not nag on the first day).
pub const EXERCISE_EVERY_DAYS: i64 = 90;
/// How far back "taken part" is counted for readiness.
pub const READINESS_DAYS: i64 = 180;

const EVENTS: &[&str] = &["exercise", "attended", "aar", "finding", "action", "done", "verified", "dropped", "closed"];

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub id: String,
    /// The exercise or review this line belongs to (for `exercise` and `aar` lines, the line's own id).
    pub rec: String,
    pub at: String,
    pub by: String,
    pub ev: String,
    /// aar: the incident or exercise it looks back at ("" for none). done, verified, dropped: the action.
    pub ref_id: String,
    /// exercise: title. attended: person. aar: title. finding: `good` or `fix`. action: owner.
    pub a1: String,
    /// exercise: the date held (YYYY-MM-DD). action: the due date.
    pub a2: String,
    /// exercise: what was practised and how it went (site goes in `ref_id`). Others: the words.
    pub note: String,
}

pub fn path(r: &Path) -> PathBuf {
    store::dir(r).join("review-log.txt")
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` date, or None when it is not a real date from 2000 to 2100.
pub fn date_days(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (s[0..4].parse().ok()?, s[5..7].parse().ok()?, s[8..10].parse().ok()?);
    if !(2000..=2100).contains(&y) || !(1..=12).contains(&m) {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(m - 1) as usize];
    if d < 1 || d > dim {
        return None;
    }
    // days from civil (Howard Hinnant)
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Today's date as days since 1970-01-01 where the person is, from the time in minutes and the clock's offset.
pub fn today_days(now: i64, off: i64) -> i64 {
    (now + off).div_euclid(1440)
}

pub fn parse(text: &str) -> Result<Parsed<Line>, String> {
    let (ok, mut rejected) = rows(text, "review-log", 9)?;
    let mut items: Vec<Line> = Vec::new();
    for f in ok {
        let l = Line { id: f[0].clone(), rec: f[1].clone(), at: f[2].clone(), by: unesc(&f[3]), ev: f[4].clone(), ref_id: f[5].clone(), a1: unesc(&f[6]), a2: f[7].clone(), note: unesc(&f[8]) };
        let known = |ev: &str, id: &str| items.iter().any(|x| x.ev == ev && x.id == id);
        let shape = match l.ev.as_str() {
            "exercise" => l.rec == l.id && !l.a1.is_empty() && date_days(&l.a2).is_some() && (l.ref_id.is_empty() || valid_id(&l.ref_id)),
            "attended" => known("exercise", &l.rec) && !l.a1.is_empty(),
            "aar" => l.rec == l.id && !l.a1.is_empty(),
            "finding" => known("aar", &l.rec) && (l.a1 == "good" || l.a1 == "fix") && !l.note.is_empty(),
            "action" => known("aar", &l.rec) && !l.a1.is_empty() && date_days(&l.a2).is_some() && !l.note.is_empty(),
            "done" | "verified" | "dropped" => known("aar", &l.rec) && items.iter().any(|x| x.ev == "action" && x.id == l.ref_id && x.rec == l.rec),
            "closed" => known("aar", &l.rec),
            _ => false,
        };
        let good = shape && EVENTS.contains(&l.ev.as_str()) && crate::security::parse_instant(&l.at).is_some() && !items.iter().any(|x| x.id == l.id);
        if good { items.push(l) } else { rejected.push(f[0].clone()) }
    }
    Ok(Parsed { items, rejected })
}
pub fn load(r: &Path) -> Result<Parsed<Line>, String> {
    parse(&read_text(&path(r))?)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Exercise {
    pub id: String,
    pub title: String,
    pub date: String,
    pub site: String,
    pub note: String,
    pub attendees: Vec<String>,
}

pub fn exercises(lines: &[Line]) -> Vec<Exercise> {
    let mut out: Vec<Exercise> = lines.iter().filter(|l| l.ev == "exercise").map(|l| Exercise { id: l.id.clone(), title: l.a1.clone(), date: l.a2.clone(), site: l.ref_id.clone(), note: l.note.clone(), attendees: vec![] }).collect();
    for l in lines.iter().filter(|l| l.ev == "attended") {
        if let Some(e) = out.iter_mut().find(|e| e.id == l.rec) {
            if !e.attendees.iter().any(|p| p.eq_ignore_ascii_case(&l.a1)) {
                e.attendees.push(l.a1.clone());
            }
        }
    }
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Open,
    Done,
    Verified,
    Dropped,
}
impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Open => "open",
            State::Done => "done",
            State::Verified => "verified",
            State::Dropped => "dropped",
        }
    }
    /// Still needs someone's attention: not checked and not set aside.
    pub fn pending(self) -> bool {
        matches!(self, State::Open | State::Done)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub id: String,
    pub text: String,
    pub owner: String,
    pub due: String,
    pub state: State,
    /// Who last changed its state and what they said.
    pub state_by: String,
    pub state_note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Aar {
    pub id: String,
    pub ref_id: String,
    pub title: String,
    pub opened_at: String,
    pub opened_by: String,
    pub findings: Vec<(String, String)>,
    pub actions: Vec<Action>,
    pub closed: bool,
}

pub fn aars(lines: &[Line]) -> Vec<Aar> {
    let mut out: Vec<Aar> = Vec::new();
    for l in lines {
        match l.ev.as_str() {
            "aar" => out.push(Aar { id: l.id.clone(), ref_id: l.ref_id.clone(), title: l.a1.clone(), opened_at: l.at.clone(), opened_by: l.by.clone(), findings: vec![], actions: vec![], closed: false }),
            "finding" => {
                if let Some(a) = out.iter_mut().find(|a| a.id == l.rec) {
                    a.findings.push((l.a1.clone(), l.note.clone()));
                }
            }
            "action" => {
                if let Some(a) = out.iter_mut().find(|a| a.id == l.rec) {
                    a.actions.push(Action { id: l.id.clone(), text: l.note.clone(), owner: l.a1.clone(), due: l.a2.clone(), state: State::Open, state_by: String::new(), state_note: String::new() });
                }
            }
            "done" | "verified" | "dropped" => {
                if let Some(act) = out.iter_mut().find(|a| a.id == l.rec).and_then(|a| a.actions.iter_mut().find(|x| x.id == l.ref_id)) {
                    let next = match (l.ev.as_str(), act.state) {
                        ("done", State::Open) => Some(State::Done),
                        ("verified", State::Open) | ("verified", State::Done) => Some(State::Verified),
                        ("dropped", s) if s != State::Verified => Some(State::Dropped),
                        _ => None,
                    };
                    if let Some(n) = next {
                        act.state = n;
                        act.state_by = l.by.clone();
                        act.state_note = l.note.clone();
                    }
                }
            }
            "closed" => {
                if let Some(a) = out.iter_mut().find(|a| a.id == l.rec) {
                    a.closed = true;
                }
            }
            _ => {}
        }
    }
    out.reverse(); // newest review first
    out
}

/// One person's readiness from exercises: when they last took part and how often lately.
#[derive(Debug, Clone, PartialEq)]
pub struct Ready {
    pub person: String,
    pub last: Option<String>,
    pub days_ago: Option<i64>,
    pub recent: usize,
}

/// `people` are the names the program already knows (the watchbill and radio training); anyone who took part in an
/// exercise is added. Names are matched without regard to capital letters.
pub fn readiness(people: &[String], ex: &[Exercise], today: i64) -> Vec<Ready> {
    let mut names: Vec<String> = Vec::new();
    for p in people.iter().chain(ex.iter().flat_map(|e| e.attendees.iter())) {
        if !p.trim().is_empty() && !names.iter().any(|n| n.eq_ignore_ascii_case(p)) {
            names.push(p.clone());
        }
    }
    names.sort_by_key(|n| n.to_lowercase());
    names
        .into_iter()
        .map(|person| {
            let mine: Vec<&Exercise> = ex.iter().filter(|e| e.attendees.iter().any(|a| a.eq_ignore_ascii_case(&person))).collect();
            let last = mine.iter().filter_map(|e| date_days(&e.date).map(|d| (d, e.date.clone()))).max_by_key(|x| x.0);
            let recent = mine.iter().filter(|e| date_days(&e.date).map(|d| today - d <= READINESS_DAYS && d <= today + 1).unwrap_or(false)).count();
            Ready { days_ago: last.as_ref().map(|(d, _)| today - d), last: last.map(|(_, s)| s), recent, person }
        })
        .collect()
}

fn site_of_aar(a: &Aar, incidents: &[Incident], ex: &[Exercise]) -> String {
    incidents.iter().find(|i| i.id == a.ref_id).map(|i| i.site.clone()).or_else(|| ex.iter().find(|e| e.id == a.ref_id).map(|e| e.site.clone())).unwrap_or_default()
}

/// Plain-words "needs a look" lines for one site: overdue actions, closed live incidents with no review, and a
/// site whose exercises have lapsed. A review with no known site belongs to every site.
pub fn looks(site: &str, lines: &[Line], incidents: &[Incident], today: i64) -> Vec<String> {
    let ex = exercises(lines);
    let reviews = aars(lines);
    let mut out = Vec::new();
    for a in reviews.iter().filter(|a| !a.closed) {
        let s = site_of_aar(a, incidents, &ex);
        if !s.is_empty() && s != site {
            continue;
        }
        for act in a.actions.iter().filter(|x| x.state.pending()) {
            if date_days(&act.due).map(|d| d < today).unwrap_or(false) {
                out.push(format!("An action from the review \"{}\" is overdue: {} ({}, was due {}).", a.title, act.text, act.owner, act.due));
            }
        }
    }
    for i in incidents.iter().filter(|i| i.site == site && !i.exercise && i.status == crate::security_incident::Status::Closed) {
        if !reviews.iter().any(|a| a.ref_id == i.id) {
            out.push(format!("The incident \"{}\" is closed but has no after-action review yet.", i.title));
        }
    }
    let mine: Vec<&Exercise> = ex.iter().filter(|e| e.site == site || e.site.is_empty()).collect();
    if let Some(last) = mine.iter().filter_map(|e| date_days(&e.date)).max() {
        if today - last > EXERCISE_EVERY_DAYS {
            out.push(format!("The last exercise was {} days ago (it is meant to be about every {}).", today - last, EXERCISE_EVERY_DAYS));
        }
    }
    out
}

/// Add a line. Returns the new line's id (for `exercise` and `aar` that is the exercise's or review's id).
#[allow(clippy::too_many_arguments)]
pub fn add(r: &Path, incidents: &[Incident], ev: &str, rec: &str, ref_id: &str, a1: &str, a2: &str, note: &str, by: &str, off: i64, now: i64) -> Result<String, String> {
    if !EVENTS.contains(&ev) {
        return Err("That is not something this record can do.".to_string());
    }
    let log = load(r)?;
    if !log.rejected.is_empty() {
        return Err("Some lines in the exercise and review log could not be read, so nothing was added. Look at those first.".to_string());
    }
    let by = clean(by, 80);
    if by.is_empty() {
        return Err("Say who is writing this.".to_string());
    }
    let (a1, note) = (clean(a1, 120), clean(note, 600));
    let id = crate::ops::next_id('r', log.items.iter().map(|x| x.id.clone()));
    let existing = aars(&log.items);
    let review = existing.iter().find(|a| a.id == rec);
    let today = today_days(now, off);
    let mut rec = rec.to_string();
    let mut ref_id = ref_id.to_string();
    let mut a2 = a2.trim().to_string();
    match ev {
        "exercise" => {
            rec = id.clone();
            if a1.is_empty() {
                return Err("Give the exercise a short name.".to_string());
            }
            let d = date_days(&a2).ok_or("Enter the date it was held as year-month-day, like 2026-11-20.")?;
            if d > today + 1 {
                return Err("That date is in the future. Record an exercise once it has been held.".to_string());
            }
            if !ref_id.is_empty() && (!valid_id(&ref_id) || !store::load_sites(r)?.items.iter().any(|s| s.id == ref_id)) {
                return Err("That site is not on the list.".to_string());
            }
        }
        "attended" => {
            if !log.items.iter().any(|x| x.ev == "exercise" && x.id == rec) {
                return Err("That exercise is not in the record.".to_string());
            }
            if a1.is_empty() {
                return Err("Say who took part.".to_string());
            }
            if exercises(&log.items).iter().any(|e| e.id == rec && e.attendees.iter().any(|p| p.eq_ignore_ascii_case(&a1))) {
                return Err("That person is already listed for this exercise.".to_string());
            }
        }
        "aar" => {
            rec = id.clone();
            if a1.is_empty() {
                return Err("Give the review a short name.".to_string());
            }
            let known = ref_id.is_empty() || incidents.iter().any(|i| i.id == ref_id) || log.items.iter().any(|x| x.ev == "exercise" && x.id == ref_id);
            if !known {
                return Err("That incident or exercise is not in the record.".to_string());
            }
        }
        _ => {
            let a = review.ok_or("That review is not in the record.")?;
            if a.closed {
                return Err("This review is closed. Open a new one if something more needs saying.".to_string());
            }
            match ev {
                "finding" => {
                    if a1 != "good" && a1 != "fix" {
                        return Err("Say whether this went well or needs fixing.".to_string());
                    }
                    if note.is_empty() {
                        return Err("Write what you saw.".to_string());
                    }
                }
                "action" => {
                    if a1.is_empty() || note.is_empty() {
                        return Err("Say what is to be done and who owns it.".to_string());
                    }
                    let d = date_days(&a2).ok_or("Enter the due date as year-month-day, like 2026-12-01.")?;
                    if d > today + 3650 {
                        return Err("That due date is very far away.".to_string());
                    }
                }
                "done" | "verified" | "dropped" => {
                    let act = a.actions.iter().find(|x| x.id == ref_id).ok_or("That action is not in this review.")?;
                    match (ev, act.state) {
                        (_, State::Verified) => return Err("That action is already verified.".to_string()),
                        (_, State::Dropped) => return Err("That action was dropped.".to_string()),
                        ("done", State::Done) => return Err("That action is already marked done.".to_string()),
                        ("dropped", _) if note.is_empty() => return Err("Say why it is being dropped.".to_string()),
                        _ => {}
                    }
                    a2.clear();
                }
                "closed" => {
                    let open = a.actions.iter().filter(|x| x.state.pending()).count();
                    if open > 0 {
                        return Err(format!("{} action{} still {} to be verified or dropped before this review can be closed.", open, if open == 1 { "" } else { "s" }, if open == 1 { "needs" } else { "need" }));
                    }
                    ref_id.clear();
                    a2.clear();
                }
                _ => {}
            }
        }
    }
    let at = format_instant(now, off);
    let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", id, rec, at, esc(&by), ev, ref_id, esc(&a1), a2, esc(&note));
    store::append_line(r, &path(r), "# format: review-log 1\n# id, exercise or review, time, by, event (exercise, attended, aar, finding, action, done, verified, dropped, closed), reference, name or person or owner, date, words (never changed)\n", &line, |t| parse(t).map(|_| ()))?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security_incident::{Incident, Status};
    use std::fs;

    const NOW: i64 = 29_000_000; // some minute in 2025, used as "now"; the tests compute dates from it

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-review-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn date_of(days: i64) -> String {
        // inverse of date_days for the tests
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
    }

    fn incident(id: &str, site: &str, exercise: bool, status: Status) -> Incident {
        Incident { id: id.into(), site: site.into(), title: format!("Incident {}", id), priority: 2, exercise, status, opened_at: String::new(), opened_by: "Dana".into(), plans: vec![], reopened: 0 }
    }

    #[test]
    fn dates_are_checked_and_counted_in_days() {
        assert_eq!(date_days("1970-01-01"), None, "before 2000 is refused");
        assert_eq!(date_days("2000-01-01"), Some(10_957));
        assert_eq!(date_days("2026-11-20").unwrap() - date_days("2026-11-10").unwrap(), 10);
        assert!(date_days("2024-02-29").is_some());
        assert_eq!(date_days("2025-02-29"), None);
        assert_eq!(date_days("2026-13-01"), None);
        assert_eq!(date_days("2026-1-1"), None);
        assert_eq!(date_days("not a date"), None);
        assert_eq!(date_of(date_days("2026-11-20").unwrap()), "2026-11-20");
        // the local date follows the clock's offset (minutes from UTC)
        assert_eq!(today_days(1440 * 100 + 30, -60), 99);
        assert_eq!(today_days(1440 * 100 + 30, 0), 100);
    }

    #[test]
    fn an_exercise_with_who_took_part_is_recorded_and_never_in_the_future() {
        let d = tmp("exercise");
        let today = date_of(today_days(NOW, 0));
        let id = add(&d, &[], "exercise", "", "", "Night alarm drill", &today, "Practised the alarm and the radio call", "Dana", 0, NOW).unwrap();
        add(&d, &[], "attended", &id, "", "Dana", "", "", "Dana", 0, NOW).unwrap();
        add(&d, &[], "attended", &id, "", "Sam", "", "", "Dana", 0, NOW).unwrap();
        assert!(add(&d, &[], "attended", &id, "", "dana", "", "", "Dana", 0, NOW).unwrap_err().contains("already"));
        assert!(add(&d, &[], "attended", "r99", "", "Sam", "", "", "Dana", 0, NOW).is_err());
        let ex = exercises(&load(&d).unwrap().items);
        assert_eq!(ex.len(), 1);
        assert_eq!(ex[0].attendees, vec!["Dana", "Sam"]);
        let later = date_of(today_days(NOW, 0) + 10);
        assert!(add(&d, &[], "exercise", "", "", "Too soon", &later, "", "Dana", 0, NOW).unwrap_err().contains("future"));
        assert!(add(&d, &[], "exercise", "", "", "", &today, "", "Dana", 0, NOW).is_err());
        assert!(add(&d, &[], "exercise", "", "", "x", "yesterday", "", "Dana", 0, NOW).is_err());
        assert!(add(&d, &[], "exercise", "", "", "x", &today, "", "", 0, NOW).is_err(), "someone must sign it");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_review_collects_findings_and_actions_and_closes_only_when_all_are_checked() {
        let d = tmp("aar");
        let inc = [incident("i1", "camp", false, Status::Closed)];
        let today = today_days(NOW, 0);
        let a = add(&d, &inc, "aar", "", "i1", "Car at the gate", "", "", "Dana", 0, NOW).unwrap();
        add(&d, &inc, "finding", &a, "", "good", "", "The radio call was clear", "Dana", 0, NOW).unwrap();
        add(&d, &inc, "finding", &a, "", "fix", "", "Nobody knew where the spare key was", "Dana", 0, NOW).unwrap();
        assert!(add(&d, &inc, "finding", &a, "", "meh", "", "x", "Dana", 0, NOW).is_err());
        let act = add(&d, &inc, "action", &a, "", "Sam", &date_of(today + 7), "Hang a key board by the door", "Dana", 0, NOW).unwrap();
        assert!(add(&d, &inc, "action", &a, "", "Sam", "soon", "x", "Dana", 0, NOW).is_err(), "a real due date is needed");
        // cannot close with an open action
        assert!(add(&d, &inc, "closed", &a, "", "", "", "", "Dana", 0, NOW).unwrap_err().contains("1 action still needs"));
        add(&d, &inc, "done", &a, &act, "", "", "Done on Sunday", "Sam", 0, NOW).unwrap();
        assert!(add(&d, &inc, "done", &a, &act, "", "", "", "Sam", 0, NOW).is_err());
        // done is not checked: still blocks closing
        assert!(add(&d, &inc, "closed", &a, "", "", "", "", "Dana", 0, NOW).is_err());
        add(&d, &inc, "verified", &a, &act, "", "", "Saw it on the wall", "Dana", 0, NOW).unwrap();
        assert!(add(&d, &inc, "dropped", &a, &act, "", "", "no", "Dana", 0, NOW).is_err(), "a verified action cannot be dropped");
        add(&d, &inc, "closed", &a, "", "", "", "", "Dana", 0, NOW).unwrap();
        // nothing more goes into a closed review
        assert!(add(&d, &inc, "finding", &a, "", "good", "", "late", "Dana", 0, NOW).unwrap_err().contains("closed"));
        let all = aars(&load(&d).unwrap().items);
        assert_eq!(all.len(), 1);
        assert!(all[0].closed);
        assert_eq!(all[0].findings.len(), 2);
        assert_eq!(all[0].actions[0].state, State::Verified);
        assert_eq!(all[0].actions[0].state_by, "Dana");
        // a review can only look back at something that exists
        assert!(add(&d, &inc, "aar", "", "i404", "x", "", "", "Dana", 0, NOW).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_dropped_action_needs_a_reason_and_lets_the_review_close() {
        let d = tmp("drop");
        let a = add(&d, &[], "aar", "", "", "General look back", "", "", "Dana", 0, NOW).unwrap();
        let act = add(&d, &[], "action", &a, "", "Sam", &date_of(today_days(NOW, 0) + 1), "Buy a lock", "Dana", 0, NOW).unwrap();
        assert!(add(&d, &[], "dropped", &a, &act, "", "", "", "Dana", 0, NOW).is_err());
        add(&d, &[], "dropped", &a, &act, "", "", "We already had one", "Dana", 0, NOW).unwrap();
        add(&d, &[], "closed", &a, "", "", "", "", "Dana", 0, NOW).unwrap();
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn needs_a_look_for_overdue_actions_unreviewed_incidents_and_lapsed_exercises() {
        let d = tmp("looks");
        let today = today_days(NOW, 0);
        let inc = [incident("i1", "camp", false, Status::Closed), incident("i2", "camp", true, Status::Closed), incident("i3", "home", false, Status::Closed), incident("i4", "camp", false, Status::Active)];
        let a = add(&d, &inc, "aar", "", "i3", "Home alarm", "", "", "Dana", 0, NOW).unwrap();
        // an action that is overdue: written with a due date in the past through the raw log (add refuses nothing about the past)
        add(&d, &inc, "action", &a, "", "Sam", &date_of(today - 3), "Fix the latch", "Dana", 0, NOW).unwrap();
        let lines = load(&d).unwrap().items;
        let camp = looks("camp", &lines, &inc, today);
        assert!(camp.iter().any(|l| l.contains("Incident i1") && l.contains("no after-action review")), "{:?}", camp);
        assert!(!camp.iter().any(|l| l.contains("Incident i2")), "an exercise needs no review");
        assert!(!camp.iter().any(|l| l.contains("Incident i4")), "a live incident is not closed yet");
        assert!(!camp.iter().any(|l| l.contains("Fix the latch")), "that review belongs to the home site");
        let home = looks("home", &lines, &inc, today);
        assert!(home.iter().any(|l| l.contains("Fix the latch") && l.contains("overdue")), "{:?}", home);
        assert!(!home.iter().any(|l| l.contains("Incident i3")), "it has a review");
        // exercises: no nag before the first, a nag when the last is old
        assert!(!camp.iter().any(|l| l.contains("last exercise")));
        let ex = add(&d, &[], "exercise", "", "", "Drill", &date_of(today - 120), "", "Dana", 0, NOW).unwrap();
        let _ = ex;
        let lines = load(&d).unwrap().items;
        assert!(looks("camp", &lines, &inc, today).iter().any(|l| l.contains("120 days ago")));
        let fresh = add(&d, &[], "exercise", "", "", "Drill 2", &date_of(today - 5), "", "Dana", 0, NOW).unwrap();
        let _ = fresh;
        let lines = load(&d).unwrap().items;
        assert!(!looks("camp", &lines, &inc, today).iter().any(|l| l.contains("last exercise")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn readiness_counts_recent_exercises_and_includes_people_who_have_not_taken_part() {
        let d = tmp("ready");
        let today = today_days(NOW, 0);
        let e1 = add(&d, &[], "exercise", "", "", "Old", &date_of(today - 200), "", "Dana", 0, NOW).unwrap();
        let e2 = add(&d, &[], "exercise", "", "", "New", &date_of(today - 10), "", "Dana", 0, NOW).unwrap();
        for (e, p) in [(&e1, "Dana"), (&e1, "Sam"), (&e2, "Dana")] {
            add(&d, &[], "attended", e, "", p, "", "", "Dana", 0, NOW).unwrap();
        }
        let ex = exercises(&load(&d).unwrap().items);
        let r = readiness(&["dana".to_string(), "Alex".to_string()], &ex, today);
        let names: Vec<&str> = r.iter().map(|x| x.person.as_str()).collect();
        assert_eq!(names, vec!["Alex", "dana", "Sam"], "each person once, whichever capitals were used");
        let by = |n: &str| r.iter().find(|x| x.person == n).unwrap().clone();
        assert_eq!((by("dana").days_ago, by("dana").recent), (Some(10), 1));
        assert_eq!((by("Sam").days_ago, by("Sam").recent), (Some(200), 0));
        assert_eq!((by("Alex").days_ago, by("Alex").last.clone()), (None, None));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_damaged_or_odd_log_is_never_added_to_and_lines_out_of_order_are_refused() {
        let d = tmp("damaged");
        let p = path(&d);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        // an action for a review that was never opened, an unknown event, and a bad date
        fs::write(&p, "# format: review-log 1\nr1\tr9\t2025-01-01T10:00+00:00\tDana\taction\t\tSam\t2025-02-01\tNo review\nr2\tr2\t2025-01-01T10:00+00:00\tDana\tmystery\t\t\t\t\nr3\tr3\t2025-01-01T10:00+00:00\tDana\texercise\t\tDrill\t2025-13-45\t\n").unwrap();
        let parsed = load(&d).unwrap();
        assert!(parsed.items.is_empty());
        assert_eq!(parsed.rejected.len(), 3);
        let before = fs::read_to_string(&p).unwrap();
        assert!(add(&d, &[], "aar", "", "", "New", "", "", "Dana", 0, NOW).is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), before, "nothing was added or changed");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn nothing_here_can_touch_an_incident() {
        // the module only reads incidents; it has no way to open, change or close one
        let src = include_str!("security_review.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        assert!(!code.contains("store::append_incident") && !code.contains("incident::open") && !code.contains("incident-log"), "reviews never write to the incident log");
    }
}
