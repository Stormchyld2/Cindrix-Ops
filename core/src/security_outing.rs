//! Who is out of camp, and who is on patrol (docs/46, plan COPS-COM-001 sections 6 and 9).
//!
//! One append-only log, `user/security/outing-log.txt`, kept like the incident log. An **outing** is people leaving a
//! site for a task; a **patrol** is the same with named checkpoints. Each has an expected return and, optionally, a next
//! contact. Lateness only makes a plain "needs a look" line. It is never turned into an incident by the program
//! (plan 9.1: a missed check-in is looked at in context first). Silence never closes anything: only a person's
//! "back safe" or "cancelled" does.
//!
//! ```text
//! # format: outing-log 1
//! id, outing, event, at, recorded, by, source, a1, a2, a3, back, next, method, text
//! ```
//! Events: `out` (a1 = outing|patrol, a2 = site, a3 = people, back, next, method, text = the task or route),
//! `contact` (a1 = ok|help|delayed; next = the next contact, empty = none planned), `checkpoint` (a1 = name,
//! a2 = ok|help|delayed; next as above), `extend` (new back and/or next; a reason is required), `return`, `cancel`
//! (a reason is required) and `note`. A line is never changed; a mistake is answered by a later line.

use crate::fsio;
use crate::ops::{esc, unesc};
use crate::security::{self, check_format, format_instant, parse_instant, valid_id, Parsed};
use crate::security_store as store;
use std::path::{Path, PathBuf};

pub const FORMAT: u32 = 1;
/// Minutes past a planned contact before it is called late, and past the expected return.
pub const GRACE_CONTACT: i64 = 10;
pub const GRACE_RETURN: i64 = 15;
const MAX_PEOPLE: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ev {
    Out,
    Contact,
    Checkpoint,
    Extend,
    Return,
    Cancel,
    Note,
}
impl Ev {
    pub fn word(self) -> &'static str {
        match self {
            Ev::Out => "out",
            Ev::Contact => "contact",
            Ev::Checkpoint => "checkpoint",
            Ev::Extend => "extend",
            Ev::Return => "return",
            Ev::Cancel => "cancel",
            Ev::Note => "note",
        }
    }
    pub fn from_word(w: &str) -> Option<Ev> {
        Some(match w {
            "out" => Ev::Out,
            "contact" => Ev::Contact,
            "checkpoint" => Ev::Checkpoint,
            "extend" => Ev::Extend,
            "return" => Ev::Return,
            "cancel" => Ev::Cancel,
            "note" => Ev::Note,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: String,
    pub outing: String,
    pub ev: Ev,
    pub at: String,
    pub recorded: String,
    pub by: String,
    /// observed (the person at the base heard it) or reported (a phone said so)
    pub source: String,
    pub a1: String,
    pub a2: String,
    pub a3: String,
    pub back: String,
    pub next: String,
    pub method: String,
    pub text: String,
}

pub fn path(user_root: &Path) -> PathBuf {
    store::dir(user_root).join("outing-log.txt")
}
pub fn header() -> String {
    format!("# format: outing-log {}\n# id, outing, event, at, recorded, by, source, a1, a2, a3, back, next, method, text (one line each, never changed)\n", FORMAT)
}
pub fn line(e: &Entry) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        e.id, e.outing, e.ev.word(), e.at, e.recorded, esc(&e.by), e.source, esc(&e.a1), esc(&e.a2), esc(&e.a3), e.back, e.next, esc(&e.method), esc(&e.text)
    )
}

pub fn parse(text: &str) -> Result<Parsed<Entry>, String> {
    check_format(text, "outing-log", FORMAT)?;
    let mut out: Parsed<Entry> = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let e = (|| {
            if f.len() != 14 || !valid_id(f[0]) || !valid_id(f[1]) {
                return None;
            }
            Some(Entry {
                id: f[0].into(),
                outing: f[1].into(),
                ev: Ev::from_word(f[2])?,
                at: f[3].into(),
                recorded: f[4].into(),
                by: unesc(f[5]),
                source: f[6].into(),
                a1: unesc(f[7]),
                a2: unesc(f[8]),
                a3: unesc(f[9]),
                back: f[10].into(),
                next: f[11].into(),
                method: unesc(f[12]),
                text: unesc(f[13]),
            })
        })();
        match e {
            Some(e) if !out.items.iter().any(|o: &Entry| o.id == e.id) => out.items.push(e),
            _ => out.rejected.push(l.to_string()),
        }
    }
    Ok(out)
}

pub fn load(user_root: &Path) -> Result<Parsed<Entry>, String> {
    match fsio::read_to_string(&path(user_root)) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Parsed::default()),
        Err(_) => Err("The outing log could not be read.".to_string()),
    }
}

// ---------- building the picture from the lines ----------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Out,
    Back,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outing {
    pub id: String,
    pub patrol: bool,
    pub site: String,
    pub people: Vec<String>,
    pub task: String,
    pub method: String,
    pub left: i64,
    pub back: i64,
    pub next: Option<i64>,
    pub state: State,
    pub ended: Option<i64>,
    /// Latest contact or checkpoint: (time, status, what)
    pub last: Option<(i64, String, String)>,
    pub checkpoints: Vec<(i64, String, String)>,
    pub notes: usize,
}

fn mins(s: &str) -> Option<i64> {
    if s.is_empty() { None } else { parse_instant(s) }
}

pub fn build(entries: &[Entry]) -> Vec<Outing> {
    let mut v: Vec<Outing> = Vec::new();
    for e in entries {
        if e.ev == Ev::Out {
            if v.iter().any(|o| o.id == e.outing) {
                continue;
            }
            let Some(left) = mins(&e.at) else { continue };
            v.push(Outing {
                id: e.outing.clone(),
                patrol: e.a1 == "patrol",
                site: e.a2.clone(),
                people: e.a3.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect(),
                task: e.text.clone(),
                method: e.method.clone(),
                left,
                back: mins(&e.back).unwrap_or(left),
                next: mins(&e.next),
                state: State::Out,
                ended: None,
                last: None,
                checkpoints: Vec::new(),
                notes: 0,
            });
            continue;
        }
        let Some(o) = v.iter_mut().find(|o| o.id == e.outing) else { continue };
        let at = mins(&e.at).unwrap_or(o.left);
        if o.state != State::Out && e.ev != Ev::Note {
            continue;
        }
        match e.ev {
            Ev::Contact => {
                o.last = Some((at, e.a1.clone(), e.text.clone()));
                o.next = mins(&e.next);
            }
            Ev::Checkpoint => {
                o.checkpoints.push((at, e.a1.clone(), e.text.clone()));
                o.last = Some((at, if e.a2.is_empty() { "ok".into() } else { e.a2.clone() }, format!("{}{}", e.a1, if e.text.is_empty() { String::new() } else { format!(": {}", e.text) })));
                o.next = mins(&e.next);
            }
            Ev::Extend => {
                if let Some(b) = mins(&e.back) {
                    o.back = b;
                }
                if !e.next.is_empty() {
                    o.next = mins(&e.next);
                }
            }
            Ev::Return => {
                o.state = State::Back;
                o.ended = Some(at);
                o.next = None;
            }
            Ev::Cancel => {
                o.state = State::Cancelled;
                o.ended = Some(at);
                o.next = None;
            }
            Ev::Note => o.notes += 1,
            Ev::Out => {}
        }
    }
    v
}

fn who(o: &Outing) -> String {
    match o.people.len() {
        0 => "Someone".to_string(),
        1 => o.people[0].clone(),
        2 => format!("{} and {}", o.people[0], o.people[1]),
        _ => format!("{} and {} others", o.people[0], o.people.len() - 1),
    }
}
fn clock(utc: i64, off: i64) -> String {
    format_instant(utc, off).get(11..16).unwrap_or("").to_string()
}
fn ago(m: i64) -> String {
    if m >= 120 { format!("{} hours", m / 60) } else { format!("{} minutes", m) }
}

/// The plain-words "needs a look" lines for one outing that is still out. One place makes them, so Command, the
/// Dashboard count and the outing list always agree. Nothing here opens an incident.
pub fn looks(o: &Outing, now: i64, off: i64) -> Vec<String> {
    let mut out = Vec::new();
    if o.state != State::Out {
        return out;
    }
    let what = if o.patrol { "on patrol" } else { "out" };
    match o.last.as_ref().map(|l| l.1.as_str()) {
        Some("help") => out.push(format!("{} asked for help ({}). Respond first, then write it down.", who(o), o.last.as_ref().map(|l| l.2.as_str()).filter(|t| !t.is_empty()).unwrap_or("no details"))),
        Some("delayed") => out.push(format!("{} reported a delay ({}).", who(o), o.last.as_ref().map(|l| l.2.as_str()).filter(|t| !t.is_empty()).unwrap_or("no reason given"))),
        _ => {}
    }
    if now > o.back + GRACE_RETURN {
        out.push(format!("{} {} and {} overdue back (expected {}, {} ago). Check in context before treating this as an incident.", who(o), what, if o.people.len() == 1 { "is" } else { "are" }, clock(o.back, off), ago(now - o.back)));
    }
    if let Some(n) = o.next {
        if now > n + GRACE_CONTACT && !(now > o.back + GRACE_RETURN) {
            out.push(format!("{}: the next contact was due at {} ({} ago). It may only be a late radio call.", who(o), clock(n, off), ago(now - n)));
        }
    }
    if o.people.len() == 1 {
        out.push(format!("{} is {} alone. The group goes out in pairs.", who(o), what));
    }
    out
}

/// When this site's last patrol came back, if one has.
pub fn last_patrol(all: &[Outing], site: &str) -> Option<i64> {
    all.iter().filter(|o| o.patrol && o.site == site && o.state == State::Back).filter_map(|o| o.ended).max()
}

// ---------- checking a new line ----------

fn status_ok(s: &str) -> bool {
    matches!(s, "ok" | "help" | "delayed")
}

/// A reason in plain words if this line cannot be added; None if it can.
pub fn check_entry(existing: &[Entry], sites: &[String], e: &Entry) -> Option<String> {
    if existing.iter().any(|x| x.id == e.id) || !valid_id(&e.id) || !valid_id(&e.outing) {
        return Some("That entry number is already used.".to_string());
    }
    if e.by.trim().is_empty() || e.by.len() > 80 {
        return Some("Say who is writing this.".to_string());
    }
    if !matches!(e.source.as_str(), "observed" | "reported") {
        return Some("Say where this came from: observed or reported.".to_string());
    }
    let Some(at) = parse_instant(&e.at) else { return Some("The time could not be read.".to_string()) };
    if e.text.len() > 600 {
        return Some("That note is too long.".to_string());
    }
    let built = build(existing);
    if e.ev == Ev::Out {
        if built.iter().any(|o| o.id == e.outing) {
            return Some("That outing already exists.".to_string());
        }
        if !matches!(e.a1.as_str(), "outing" | "patrol") {
            return Some("Say whether this is an outing or a patrol.".to_string());
        }
        if !sites.iter().any(|s| s == &e.a2) {
            return Some("That site is not on the list.".to_string());
        }
        let people: Vec<&str> = e.a3.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
        if people.is_empty() || people.len() > MAX_PEOPLE || people.iter().any(|p| p.len() > 40) {
            return Some("Say who is going (up to ten names).".to_string());
        }
        if e.text.trim().is_empty() {
            return Some("Say what the task or route is.".to_string());
        }
        let Some(back) = parse_instant(&e.back) else { return Some("Say when they expect to be back.".to_string()) };
        if back <= at {
            return Some("The expected return has to be after they leave.".to_string());
        }
        if !e.next.is_empty() {
            match parse_instant(&e.next) {
                Some(n) if n > at && n <= back => {}
                _ => return Some("The next contact has to be between leaving and the expected return.".to_string()),
            }
        }
        if !matches!(e.method.as_str(), "" | "radio" | "runner" | "none") {
            return Some("The way to stay in touch is radio, runner or none.".to_string());
        }
        return None;
    }
    let Some(o) = built.iter().find(|o| o.id == e.outing) else { return Some("That outing is not in the log.".to_string()) };
    if o.state != State::Out && e.ev != Ev::Note {
        return Some("That outing has already ended.".to_string());
    }
    if at < o.left {
        return Some("That is before they left.".to_string());
    }
    if !e.next.is_empty() && !matches!(e.ev, Ev::Contact | Ev::Checkpoint | Ev::Extend) {
        return Some("A next contact only goes with a check-in, a checkpoint or an extension.".to_string());
    }
    if !e.next.is_empty() && parse_instant(&e.next).map(|n| n <= at).unwrap_or(true) {
        return Some("The next contact has to be after this one.".to_string());
    }
    match e.ev {
        Ev::Contact if !status_ok(&e.a1) => Some("Say how they are: ok, help or delayed.".to_string()),
        Ev::Checkpoint if e.a1.trim().is_empty() || e.a1.len() > 60 => Some("Name the checkpoint.".to_string()),
        Ev::Checkpoint if !e.a2.is_empty() && !status_ok(&e.a2) => Some("Say how they are: ok, help or delayed.".to_string()),
        Ev::Extend if e.text.trim().is_empty() => Some("Say why the time is being changed.".to_string()),
        Ev::Extend if e.back.is_empty() && e.next.is_empty() => Some("Give a new return time or a new next contact.".to_string()),
        Ev::Extend if !e.back.is_empty() && parse_instant(&e.back).map(|b| b <= at).unwrap_or(true) => Some("The new return has to be in the future.".to_string()),
        Ev::Cancel if e.text.trim().is_empty() => Some("Say why it was cancelled.".to_string()),
        Ev::Note if e.text.trim().is_empty() => Some("Write the note.".to_string()),
        _ => None,
    }
}

/// Add one line after checking it against everything already there. A log that cannot be read is never added to.
pub fn append(user_root: &Path, e: &Entry) -> Result<(), String> {
    let sites: Vec<String> = store::load_sites(user_root)?.items.into_iter().map(|s| s.id).collect();
    let log = load(user_root)?;
    if !log.rejected.is_empty() {
        return Err("Some lines in the outing log could not be read, so nothing was added. Look at those first.".to_string());
    }
    if let Some(p) = check_entry(&log.items, &sites, e) {
        return Err(p);
    }
    store::append_line(user_root, &path(user_root), &header(), &line(e), |t| parse(t).map(|_| ()))
}

pub fn instant_ok(s: &str) -> bool {
    security::parse_instant(s).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-outing-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        store::save_sites(&p, &[security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        p
    }
    const T0: &str = "2026-11-08T10:00-05:00"; // 15:00 UTC
    fn at(h: i64, m: i64) -> String {
        format_instant(parse_instant(T0).unwrap() + h * 60 + m, -300)
    }
    fn e(id: &str, outing: &str, ev: Ev, when: String) -> Entry {
        Entry { id: id.into(), outing: outing.into(), ev, at: when, recorded: T0.into(), by: "Base".into(), source: "observed".into(), a1: String::new(), a2: String::new(), a3: String::new(), back: String::new(), next: String::new(), method: String::new(), text: String::new() }
    }
    fn out(id: &str, outing: &str, kind: &str, people: &str, back: String, next: String) -> Entry {
        Entry { a1: kind.into(), a2: "camp".into(), a3: people.into(), back, next, method: "radio".into(), text: "Check the south fence".into(), ..e(id, outing, Ev::Out, at(0, 0)) }
    }
    fn now_at(h: i64, m: i64) -> i64 {
        parse_instant(&at(h, m)).unwrap()
    }

    #[test]
    fn lines_survive_writing_and_odd_characters() {
        let mut o = out("e1", "o1", "outing", "Sam, Dana", at(2, 0), at(1, 0));
        o.text = "Fence\tline, \"north\"\nrepairs".into();
        let t = format!("{}{}", header(), line(&o));
        let p = parse(&t).unwrap();
        assert_eq!(p.items, vec![o]);
        assert!(p.rejected.is_empty());
    }

    #[test]
    fn a_pair_out_and_back_never_makes_a_look() {
        let log = vec![out("e1", "o1", "outing", "Sam, Dana", at(2, 0), at(1, 0)), e("e2", "o1", Ev::Contact, at(1, 0)), e("e3", "o1", Ev::Return, at(1, 50))];
        let b = build(&log);
        assert_eq!(b[0].state, State::Back);
        assert!(looks(&b[0], now_at(9, 0), -300).is_empty());
    }

    #[test]
    fn late_contact_and_late_return_are_plain_lines_never_incidents() {
        let log = vec![out("e1", "o1", "outing", "Sam, Dana", at(2, 0), at(1, 0))];
        let b = build(&log);
        assert!(looks(&b[0], now_at(1, 5), -300).is_empty(), "inside the grace");
        let l = looks(&b[0], now_at(1, 30), -300);
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("next contact was due at 11:00") && l[0].contains("late radio"));
        let l = looks(&b[0], now_at(2, 30), -300);
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("overdue back") && l[0].contains("expected 12:00") && l[0].contains("in context"));
        // nothing in the incident log was touched
        let r = tmp("noinc");
        append(&r, &log[0]).unwrap();
        assert!(store::load_incident_log(&r).unwrap().items.is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_check_in_moves_the_next_contact_and_help_comes_first() {
        let mut c = e("e2", "o1", Ev::Contact, at(1, 0));
        c.a1 = "ok".into();
        c.next = at(1, 30);
        let log = vec![out("e1", "o1", "outing", "Sam, Dana", at(3, 0), at(1, 0)), c];
        let b = build(&log);
        assert_eq!(b[0].next, Some(now_at(1, 30)));
        assert!(looks(&b[0], now_at(1, 35), -300).is_empty());
        assert_eq!(looks(&b[0], now_at(1, 50), -300).len(), 1);
        // an ok check-in with no next time leaves only the return deadline
        let mut c2 = e("e3", "o1", Ev::Contact, at(1, 40));
        c2.a1 = "ok".into();
        let mut log2 = log.clone();
        log2.push(c2);
        assert!(looks(&build(&log2)[0], now_at(2, 30), -300).is_empty());
        // help is the first thing said
        let mut h = e("e4", "o1", Ev::Contact, at(1, 45));
        h.a1 = "help".into();
        h.text = "Twisted ankle at the creek".into();
        log2.push(h);
        let l = looks(&build(&log2)[0], now_at(1, 46), -300);
        assert!(l[0].contains("asked for help") && l[0].contains("Twisted ankle"));
    }

    #[test]
    fn going_out_alone_is_called_out() {
        let b = build(&[out("e1", "o1", "outing", "Sam", at(2, 0), String::new())]);
        let l = looks(&b[0], now_at(0, 10), -300);
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("alone") && l[0].contains("pairs"));
    }

    #[test]
    fn a_patrol_records_checkpoints_and_the_last_patrol_is_known() {
        let mut cp = e("e2", "p1", Ev::Checkpoint, at(0, 20));
        cp.a1 = "North gate".into();
        cp.next = at(0, 40);
        let mut cp2 = e("e3", "p1", Ev::Checkpoint, at(0, 40));
        cp2.a1 = "Creek crossing".into();
        cp2.a2 = "ok".into();
        let log = vec![out("e1", "p1", "patrol", "Sam, Dana", at(1, 0), at(0, 20)), cp, cp2, e("e4", "p1", Ev::Return, at(0, 55))];
        let b = build(&log);
        assert!(b[0].patrol && b[0].checkpoints.len() == 2 && b[0].state == State::Back);
        assert_eq!(last_patrol(&b, "camp"), Some(now_at(0, 55)));
        assert_eq!(last_patrol(&b, "home"), None);
    }

    #[test]
    fn extending_needs_a_reason_and_silence_never_closes_anything() {
        let r = tmp("extend");
        append(&r, &out("e1", "o1", "outing", "Sam, Dana", at(2, 0), at(1, 0))).unwrap();
        let mut x = e("e2", "o1", Ev::Extend, at(0, 30));
        x.back = at(3, 0);
        assert!(append(&r, &x).unwrap_err().contains("why"));
        x.text = "Road washed out, going around".into();
        append(&r, &x).unwrap();
        let b = build(&load(&r).unwrap().items);
        assert_eq!(b[0].back, now_at(3, 0));
        // days later, still "out" and still on the list
        assert_eq!(b[0].state, State::Out);
        assert!(!looks(&b[0], now_at(72, 0), -300).is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn bad_lines_are_refused_with_a_reason() {
        let r = tmp("bad");
        let good = out("e1", "o1", "outing", "Sam, Dana", at(2, 0), at(1, 0));
        append(&r, &good).unwrap();
        let cases: Vec<(Entry, &str)> = vec![
            (out("e1", "o9", "outing", "Sam, Dana", at(2, 0), String::new()), "already used"),
            (out("e2", "o1", "outing", "Sam", at(2, 0), String::new()), "already exists"),
            (Entry { a2: "nowhere".into(), ..out("e2", "o2", "outing", "Sam, Dana", at(2, 0), String::new()) }, "site"),
            (out("e2", "o2", "outing", "  ,  ", at(2, 0), String::new()), "who is going"),
            (out("e2", "o2", "outing", "Sam, Dana", at(0, 0), String::new()), "after they leave"),
            (out("e2", "o2", "outing", "Sam, Dana", at(2, 0), at(3, 0)), "next contact"),
            (out("e2", "o2", "outing", "Sam, Dana", String::new(), String::new()), "back"),
            (Entry { text: String::new(), ..out("e2", "o2", "patrol", "Sam, Dana", at(2, 0), String::new()) }, "task or route"),
            (Entry { a1: "ok".into(), ..e("e2", "zz", Ev::Contact, at(1, 0)) }, "not in the log"),
            (Entry { a1: "fine".into(), ..e("e2", "o1", Ev::Contact, at(1, 0)) }, "ok, help or delayed"),
            (Entry { a1: "ok".into(), ..e("e2", "o1", Ev::Contact, at(-1, 0)) }, "before they left"),
            (e("e2", "o1", Ev::Cancel, at(1, 0)), "why"),
            (Entry { by: String::new(), ..e("e2", "o1", Ev::Return, at(1, 0)) }, "who is writing"),
            (Entry { next: at(5, 0), ..e("e2", "o1", Ev::Return, at(1, 0)) }, "only goes with"),
        ];
        for (c, why) in cases {
            let err = append(&r, &c).unwrap_err();
            assert!(err.contains(why), "{} -> {}", why, err);
        }
        append(&r, &e("e2", "o1", Ev::Return, at(1, 50))).unwrap();
        assert!(append(&r, &Entry { a1: "ok".into(), ..e("e3", "o1", Ev::Contact, at(1, 55)) }).unwrap_err().contains("already ended"));
        append(&r, &Entry { text: "Brought wood back too".into(), ..e("e3", "o1", Ev::Note, at(1, 55)) }).unwrap();
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_damaged_or_newer_log_is_never_added_to() {
        let r = tmp("damaged");
        fs::create_dir_all(store::dir(&r)).unwrap();
        fs::write(path(&r), "# format: outing-log 1\nthis line is damaged\n").unwrap();
        assert!(append(&r, &out("e1", "o1", "outing", "Sam, Dana", at(2, 0), String::new())).unwrap_err().contains("could not be read"));
        fs::write(path(&r), "# format: outing-log 9\n").unwrap();
        assert!(append(&r, &out("e1", "o1", "outing", "Sam, Dana", at(2, 0), String::new())).is_err());
        assert_eq!(fs::read_to_string(path(&r)).unwrap(), "# format: outing-log 9\n");
        let _ = fs::remove_dir_all(&r);
    }
}
