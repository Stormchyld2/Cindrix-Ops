//! Security Operations, part 4: incidents and their timelines.
//! Design: `docs/44-security-operations-architecture.md` sections 8 and 14. Plain logic, no screen.
//!
//! One append-only log (`user/security/incident-log.txt`) holds everything. An incident is *built* from its lines,
//! so there is no second file to disagree with it:
//!  * `open` starts an incident (site, priority 1 to 4, live or exercise);
//!  * `status` moves it: draft, active, stabilized, closed; `reopen` is its own audited step;
//!  * `note`, `decision` and `task` add to the timeline;
//!  * `plan` is an explicit person's activation of an Operational Response plan, pinning the exact revision;
//!  * `correction` supersedes an earlier line; the earlier line stays.
//! Each line has the time the thing happened and the time it was written, and where it came from.
//! Nothing here closes an incident because an alert stopped, and an exercise is never counted as live.

use crate::md5::{hex, Md5};
use crate::ops::{esc, unesc};
use crate::security::{check_format, parse_instant, valid_id, Parsed};

pub const INCIDENT_FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Open,
    Status,
    Note,
    Decision,
    Task,
    Plan,
    Correction,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Open => "open",
            Kind::Status => "status",
            Kind::Note => "note",
            Kind::Decision => "decision",
            Kind::Task => "task",
            Kind::Plan => "plan",
            Kind::Correction => "correction",
        }
    }
    pub fn from_word(s: &str) -> Option<Kind> {
        Some(match s.trim() {
            "open" => Kind::Open,
            "status" => Kind::Status,
            "note" => Kind::Note,
            "decision" => Kind::Decision,
            "task" => Kind::Task,
            "plan" => Kind::Plan,
            "correction" => Kind::Correction,
            _ => return None,
        })
    }
}

/// Where a line's information came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Someone saw or did it themselves.
    Observed,
    /// Someone else said so.
    Reported,
    /// Came from another program or an outside alert. Untrusted until a person confirms it.
    Imported,
    /// A later fix to an earlier line.
    Corrected,
}

impl Source {
    pub fn word(self) -> &'static str {
        match self {
            Source::Observed => "observed",
            Source::Reported => "reported",
            Source::Imported => "imported",
            Source::Corrected => "corrected",
        }
    }
    pub fn from_word(s: &str) -> Option<Source> {
        Some(match s.trim() {
            "observed" => Source::Observed,
            "reported" => Source::Reported,
            "imported" => Source::Imported,
            "corrected" => Source::Corrected,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Draft,
    Active,
    Stabilized,
    Closed,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Active => "active",
            Status::Stabilized => "stabilized",
            Status::Closed => "closed",
        }
    }
    pub fn from_word(s: &str) -> Option<Status> {
        Some(match s.trim() {
            "draft" => Status::Draft,
            "active" => Status::Active,
            "stabilized" => Status::Stabilized,
            "closed" => Status::Closed,
            _ => return None,
        })
    }
}

/// One line of the log.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// `e1`, `e2` ... never reused.
    pub id: String,
    /// The incident this belongs to (`i1`, `i2` ...). For `open`, the id the new incident will have.
    pub incident: String,
    pub kind: Kind,
    /// When it happened.
    pub happened_at: String,
    /// When it was written down.
    pub recorded_at: String,
    /// Who wrote it.
    pub by: String,
    pub source: Source,
    /// open: site id. status: the new status word, or `reopen`. plan: plan id. Others: unused.
    pub arg1: String,
    /// open: priority 1 to 4 (1 is most serious). plan: revision.
    pub arg2: String,
    /// open: `live` or `exercise`. plan: fingerprint of the pinned plan text.
    pub arg3: String,
    /// open: title. status: the reason (for closed: what shows it is safe to close). Others: the words.
    pub text: String,
    /// The id of the earlier line a correction replaces, or empty.
    pub supersedes: String,
}

const MAX_TEXT: usize = 4000;

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').take(MAX_TEXT).collect::<String>().trim().to_string()
}

pub fn header() -> String {
    format!("# format: security-incidents {}\n# id\tincident\tkind\thappened_at\trecorded_at\tby\tsource\targ1\targ2\targ3\ttext\tsupersedes\n", INCIDENT_FORMAT)
}

pub fn line(e: &Entry) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
        e.id, e.incident, e.kind.word(), e.happened_at, e.recorded_at, esc(&e.by), e.source.word(), esc(&e.arg1), esc(&e.arg2), esc(&e.arg3), esc(&e.text), e.supersedes
    )
}

pub fn parse(text: &str) -> Result<Parsed<Entry>, String> {
    check_format(text, "security-incidents", INCIDENT_FORMAT)?;
    let mut p = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let kind = Kind::from_word(get(2));
        let source = Source::from_word(get(6));
        let e = kind.zip(source).map(|(kind, source)| Entry {
            id: get(0).to_string(),
            incident: get(1).to_string(),
            kind,
            happened_at: get(3).trim().to_string(),
            recorded_at: get(4).trim().to_string(),
            by: clean(&unesc(get(5))).chars().take(80).collect(),
            source,
            arg1: clean(&unesc(get(7))),
            arg2: clean(&unesc(get(8))),
            arg3: clean(&unesc(get(9))),
            text: clean(&unesc(get(10))),
            supersedes: get(11).trim().to_string(),
        });
        match e {
            Some(e) if valid_id(&e.id) && valid_id(&e.incident) && parse_instant(&e.happened_at).is_some() && parse_instant(&e.recorded_at).is_some() && !p.items.iter().any(|x: &Entry| x.id == e.id) => p.items.push(e),
            _ => p.rejected.push(l.to_string()),
        }
    }
    Ok(p)
}

/// The next free id with a letter in front (`e7`, `i3`), counting every id ever written.
pub fn next_id(prefix: char, existing: impl Iterator<Item = String>) -> String {
    crate::ops::next_id(prefix, existing)
}

// ---------- building an incident from its lines ----------

#[derive(Debug, Clone, PartialEq)]
pub struct PinnedPlan {
    pub entry: String,
    pub plan: String,
    pub revision: String,
    pub fingerprint: String,
    pub activated_by: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Incident {
    pub id: String,
    pub site: String,
    pub title: String,
    pub priority: u8,
    /// An exercise is practice. It is never counted as a live incident.
    pub exercise: bool,
    pub status: Status,
    pub opened_at: String,
    pub opened_by: String,
    /// Plans a person activated for this incident, oldest first.
    pub plans: Vec<PinnedPlan>,
    /// How many times it has been reopened.
    pub reopened: u32,
}

/// Every incident, built from the log. Lines for an incident that was never opened are ignored here (the log
/// itself keeps them).
pub fn build(entries: &[Entry]) -> Vec<Incident> {
    let mut out: Vec<Incident> = Vec::new();
    for e in entries {
        if e.kind == Kind::Open {
            if out.iter().any(|i| i.id == e.incident) {
                continue;
            }
            out.push(Incident {
                id: e.incident.clone(),
                site: e.arg1.clone(),
                title: e.text.clone(),
                priority: e.arg2.parse::<u8>().ok().filter(|p| (1..=4).contains(p)).unwrap_or(3),
                exercise: e.arg3 == "exercise",
                status: Status::Draft,
                opened_at: e.happened_at.clone(),
                opened_by: e.by.clone(),
                plans: Vec::new(),
                reopened: 0,
            });
            continue;
        }
        let Some(inc) = out.iter_mut().find(|i| i.id == e.incident) else { continue };
        match e.kind {
            Kind::Status if e.arg1 == "reopen" => {
                if inc.status == Status::Closed {
                    inc.status = Status::Active;
                    inc.reopened += 1;
                }
            }
            Kind::Status => {
                if let Some(s) = Status::from_word(&e.arg1) {
                    if can_move(inc.status, s) {
                        inc.status = s;
                    }
                }
            }
            Kind::Plan => inc.plans.push(PinnedPlan { entry: e.id.clone(), plan: e.arg1.clone(), revision: e.arg2.clone(), fingerprint: e.arg3.clone(), activated_by: e.by.clone(), at: e.happened_at.clone() }),
            _ => {}
        }
    }
    out
}

/// Allowed status moves. Closed only leaves through `reopen`.
pub fn can_move(from: Status, to: Status) -> bool {
    use Status::*;
    matches!((from, to), (Draft, Active) | (Draft, Closed) | (Active, Stabilized) | (Active, Closed) | (Stabilized, Active) | (Stabilized, Closed))
}

/// Incidents that count as live: not an exercise, and not closed. This is what a dashboard counts.
pub fn live_open(incidents: &[Incident]) -> Vec<&Incident> {
    incidents.iter().filter(|i| !i.exercise && i.status != Status::Closed).collect()
}

/// A timeline line as the screen shows it: with who replaced it, if anyone. Replaced lines are still shown.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineLine<'a> {
    pub entry: &'a Entry,
    pub superseded_by: Option<String>,
}

/// The timeline of one incident in the order things happened (ties keep the order written).
pub fn timeline<'a>(entries: &'a [Entry], incident: &str) -> Vec<TimelineLine<'a>> {
    let mut v: Vec<&Entry> = entries.iter().filter(|e| e.incident == incident).collect();
    v.sort_by_key(|e| parse_instant(&e.happened_at));
    v.into_iter()
        .map(|e| TimelineLine { entry: e, superseded_by: entries.iter().rev().find(|c| c.kind == Kind::Correction && c.incident == incident && c.supersedes == e.id).map(|c| c.id.clone()) })
        .collect()
}

/// A short fingerprint of a plan's exact text, kept in the log with the activation. The program also saves a copy
/// of the text next to the log, so what was followed can be read back even if the plan is edited later.
pub fn fingerprint(plan_text: &str) -> String {
    let mut m = Md5::new();
    m.update(plan_text.as_bytes());
    hex(&m.finish())
}

// ---------- checking a new line before it is added ----------

/// The plain-words problem with a new line, or None when it can be added. `now` is the recorded time.
pub fn check_entry(existing: &[Entry], sites: &[String], e: &Entry) -> Option<String> {
    if !valid_id(&e.id) || existing.iter().any(|x| x.id == e.id) {
        return Some("That entry number is already used.".to_string());
    }
    let (Some(happened), Some(recorded)) = (parse_instant(&e.happened_at), parse_instant(&e.recorded_at)) else {
        return Some("The time needs a date, a time and the time zone offset.".to_string());
    };
    if happened > recorded + 5 {
        return Some("That time is in the future.".to_string());
    }
    if e.by.trim().is_empty() {
        return Some("Say who is writing this.".to_string());
    }
    let incidents = build(existing);
    let inc = incidents.iter().find(|i| i.id == e.incident);
    if e.kind == Kind::Open {
        if inc.is_some() || !valid_id(&e.incident) {
            return Some("That incident number is already used.".to_string());
        }
        if !sites.iter().any(|s| *s == e.arg1) {
            return Some("That site is not on the list.".to_string());
        }
        if e.text.trim().is_empty() {
            return Some("Give the incident a short title.".to_string());
        }
        if !matches!(e.arg2.as_str(), "1" | "2" | "3" | "4") || !matches!(e.arg3.as_str(), "live" | "exercise") {
            return Some("Pick a priority from 1 to 4, and say whether this is real or an exercise.".to_string());
        }
        return None;
    }
    let Some(inc) = inc else { return Some("That incident does not exist.".to_string()) };
    if e.source == Source::Imported && matches!(e.kind, Kind::Status | Kind::Plan) {
        return Some("An imported alert cannot change an incident or activate a plan. A person must do that.".to_string());
    }
    match e.kind {
        Kind::Status if e.arg1 == "reopen" => {
            if inc.status != Status::Closed {
                return Some("Only a closed incident can be reopened.".to_string());
            }
            if e.text.trim().is_empty() {
                return Some("Say why it is being reopened.".to_string());
            }
        }
        Kind::Status => {
            let Some(to) = Status::from_word(&e.arg1) else { return Some("That status is not one of draft, active, stabilized or closed.".to_string()) };
            if !can_move(inc.status, to) {
                return Some(if inc.status == Status::Closed { "A closed incident must be reopened first.".to_string() } else { "That change is not allowed from where the incident is now.".to_string() });
            }
            if to == Status::Closed && e.text.trim().is_empty() {
                return Some("Say what shows it is safe to close.".to_string());
            }
        }
        Kind::Plan => {
            if !matches!(inc.status, Status::Active | Status::Stabilized) {
                return Some("A plan can only be activated on an active incident.".to_string());
            }
            if e.arg1.trim().is_empty() || e.arg2.trim().is_empty() || e.arg3.len() != 32 {
                return Some("A plan needs its name, its revision and its saved copy.".to_string());
            }
        }
        Kind::Correction => {
            if e.supersedes.is_empty() || e.supersedes == e.id || !existing.iter().any(|x| x.id == e.supersedes && x.incident == e.incident) {
                return Some("Say which earlier entry this fixes.".to_string());
            }
            if e.text.trim().is_empty() {
                return Some("Write the corrected words.".to_string());
            }
        }
        Kind::Note | Kind::Decision | Kind::Task => {
            if e.text.trim().is_empty() {
                return Some("There is nothing written.".to_string());
            }
        }
        Kind::Open => {}
    }
    if e.kind != Kind::Correction && !e.supersedes.is_empty() {
        return Some("Only a correction can replace an earlier entry.".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: &str = "2026-11-08T10:00-05:00";
    const T1: &str = "2026-11-08T10:30-05:00";
    const T2: &str = "2026-11-08T11:00-05:00";

    fn e(id: &str, inc: &str, kind: Kind, at: &str, arg1: &str, text: &str) -> Entry {
        Entry { id: id.into(), incident: inc.into(), kind, happened_at: at.into(), recorded_at: at.into(), by: "Dad".into(), source: Source::Observed, arg1: arg1.into(), arg2: String::new(), arg3: String::new(), text: text.into(), supersedes: String::new() }
    }
    fn open(id: &str, inc: &str, site: &str, exercise: bool) -> Entry {
        Entry { arg2: "2".into(), arg3: if exercise { "exercise" } else { "live" }.into(), ..e(id, inc, Kind::Open, T0, site, "Car at the gate") }
    }
    fn sites() -> Vec<String> {
        vec!["home".to_string(), "camp".to_string()]
    }
    /// Add a line the way the store does: check, then append.
    fn add(log: &mut Vec<Entry>, x: Entry) -> Option<String> {
        let p = check_entry(log, &sites(), &x);
        if p.is_none() {
            log.push(x);
        }
        p
    }

    #[test]
    fn lifecycle_runs_draft_active_stabilized_closed() {
        let mut log = vec![];
        assert!(add(&mut log, open("e1", "i1", "home", false)).is_none());
        assert_eq!(build(&log)[0].status, Status::Draft);
        assert!(add(&mut log, e("e2", "i1", Kind::Status, T1, "active", "confirmed")).is_none());
        assert!(add(&mut log, e("e3", "i1", Kind::Status, T1, "stabilized", "")).is_none());
        // closing needs a reason
        assert!(add(&mut log, e("e4", "i1", Kind::Status, T2, "closed", "")).is_some());
        assert!(add(&mut log, e("e4", "i1", Kind::Status, T2, "closed", "Car left, gate checked")).is_none());
        let i = &build(&log)[0];
        assert_eq!(i.status, Status::Closed);
        assert_eq!(i.opened_by, "Dad");
    }

    #[test]
    fn closed_stays_closed_until_an_explicit_reopen() {
        let mut log = vec![open("e1", "i1", "home", false), e("e2", "i1", Kind::Status, T1, "closed", "False alarm")];
        assert!(add(&mut log, e("e3", "i1", Kind::Status, T2, "active", "")).is_some());
        assert!(add(&mut log, e("e3", "i1", Kind::Status, T2, "reopen", "")).is_some(), "reopen needs a reason");
        assert!(add(&mut log, e("e3", "i1", Kind::Status, T2, "reopen", "Car came back")).is_none());
        let i = &build(&log)[0];
        assert_eq!(i.status, Status::Active);
        assert_eq!(i.reopened, 1);
        // an open incident cannot be "reopened"
        assert!(add(&mut log, e("e4", "i1", Kind::Status, T2, "reopen", "again")).is_some());
        // skipping steps is refused
        let mut log2 = vec![open("e1", "i1", "home", false)];
        assert!(add(&mut log2, e("e2", "i1", Kind::Status, T1, "stabilized", "")).is_some());
    }

    #[test]
    fn imported_alerts_cannot_change_status_or_activate_plans() {
        let mut log = vec![open("e1", "i1", "home", false)];
        let mut s = e("e2", "i1", Kind::Status, T1, "active", "alert says so");
        s.source = Source::Imported;
        assert!(add(&mut log, s).is_some());
        // but an imported note is welcome, marked as imported
        let mut n = e("e2", "i1", Kind::Note, T1, "", "Overwatch: unusual traffic nearby");
        n.source = Source::Imported;
        assert!(add(&mut log, n).is_none());
        assert_eq!(timeline(&log, "i1")[1].entry.source, Source::Imported);
    }

    #[test]
    fn plan_activation_pins_the_exact_revision_and_survives_plan_edits() {
        let mut log = vec![open("e1", "i1", "home", false)];
        let plan_v2 = "# Shelter in place\nStep 1: lock the doors\n";
        let mut p = e("e2", "i1", Kind::Plan, T1, "shelter-in-place", "");
        p.arg2 = "2".into();
        p.arg3 = fingerprint(plan_v2);
        // not while the incident is only a draft
        assert!(add(&mut log, p.clone()).is_some());
        assert!(add(&mut log, e("e3", "i1", Kind::Status, T1, "active", "confirmed")).is_none());
        assert!(add(&mut log, p.clone()).is_none());
        let i = &build(&log)[0];
        assert_eq!(i.plans.len(), 1);
        assert_eq!(i.plans[0].revision, "2");
        assert_eq!(i.plans[0].fingerprint, fingerprint(plan_v2));
        // editing the master plan later changes its fingerprint, not the incident's record
        assert_ne!(fingerprint("# Shelter in place\nStep 1: lock the doors and windows\n"), i.plans[0].fingerprint);
        // missing revision or copy is refused
        let mut bad = p.clone();
        bad.id = "e9".into();
        bad.arg3 = "short".into();
        assert!(add(&mut log, bad).is_some());
    }

    #[test]
    fn corrections_replace_but_never_erase() {
        let mut log = vec![open("e1", "i1", "home", false), e("e2", "i1", Kind::Note, T1, "", "Red truck, two men")];
        let mut c = e("e3", "i1", Kind::Correction, T2, "", "Blue truck, two men");
        c.source = Source::Corrected;
        c.supersedes = "e2".into();
        assert!(add(&mut log, c).is_none());
        let t = timeline(&log, "i1");
        let old = t.iter().find(|l| l.entry.id == "e2").unwrap();
        assert_eq!(old.superseded_by.as_deref(), Some("e3"));
        assert_eq!(old.entry.text, "Red truck, two men");
        // a correction needs a real earlier entry of the same incident, and words
        let mut bad = e("e4", "i1", Kind::Correction, T2, "", "x");
        bad.supersedes = "e99".into();
        assert!(add(&mut log, bad).is_some());
        let mut none = e("e4", "i1", Kind::Correction, T2, "", "x");
        assert!(add(&mut log, none.clone()).is_some());
        none.supersedes = "e2".into();
        none.text = String::new();
        assert!(add(&mut log, none).is_some());
        // an ordinary note cannot claim to replace something
        let mut sneaky = e("e5", "i1", Kind::Note, T2, "", "hello");
        sneaky.supersedes = "e2".into();
        assert!(add(&mut log, sneaky).is_some());
    }

    #[test]
    fn late_entries_keep_event_time_and_entry_time_apart() {
        let mut log = vec![open("e1", "i1", "home", false)];
        // something that happened at 10:30, written down at 11:00 from the paper log
        let mut n = e("e2", "i1", Kind::Note, T1, "", "Handover by radio, Sam took the watch");
        n.recorded_at = T2.into();
        n.source = Source::Reported;
        assert!(add(&mut log, n).is_none());
        // and one written at 11:00 about something at 10:45, so the timeline orders by what happened
        let mut m = e("e3", "i1", Kind::Note, "2026-11-08T10:15-05:00", "", "Dog barked first");
        m.recorded_at = "2026-11-08T11:05-05:00".into();
        assert!(add(&mut log, m).is_none());
        let t = timeline(&log, "i1");
        let order: Vec<&str> = t.iter().map(|l| l.entry.id.as_str()).collect();
        assert_eq!(order, vec!["e1", "e3", "e2"]);
        // but "happened" cannot be in the future
        let mut f = e("e4", "i1", Kind::Note, "2026-11-08T13:00-05:00", "", "tomorrow");
        f.recorded_at = T2.into();
        assert!(add(&mut log, f).is_some());
    }

    #[test]
    fn exercises_are_never_counted_as_live() {
        let mut log = vec![];
        add(&mut log, open("e1", "i1", "home", false));
        add(&mut log, open("e2", "i2", "home", true));
        let inc = build(&log);
        assert_eq!(inc.len(), 2);
        let live = live_open(&inc);
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].id, "i1");
        // closing the live one leaves none
        add(&mut log, e("e3", "i1", Kind::Status, T1, "closed", "False alarm"));
        assert!(live_open(&build(&log)).is_empty());
    }

    #[test]
    fn opening_is_checked() {
        let mut log = vec![];
        assert!(add(&mut log, open("e1", "i1", "nowhere", false)).is_some());
        let mut x = open("e1", "i1", "home", false);
        x.text = " ".into();
        assert!(add(&mut log, x).is_some());
        let mut x = open("e1", "i1", "home", false);
        x.arg2 = "9".into();
        assert!(add(&mut log, x).is_some());
        let mut x = open("e1", "i1", "home", false);
        x.arg3 = "maybe".into();
        assert!(add(&mut log, x).is_some());
        assert!(add(&mut log, open("e1", "i1", "home", false)).is_none());
        // same incident number or entry number again
        assert!(add(&mut log, open("e2", "i1", "home", false)).is_some());
        assert!(add(&mut log, open("e1", "i2", "home", false)).is_some());
        // lines for an incident that does not exist
        assert!(add(&mut log, e("e2", "i9", Kind::Note, T1, "", "x")).is_some());
        // nobody wrote it
        let mut n = e("e2", "i1", Kind::Note, T1, "", "x");
        n.by = " ".into();
        assert!(add(&mut log, n).is_some());
    }

    #[test]
    fn log_file_round_trips_and_keeps_bad_lines() {
        let mut o = open("e1", "i1", "home", false);
        o.text = "Gate\tsticks\nand creaks".into();
        let mut p = e("e2", "i1", Kind::Plan, T1, "shelter", "");
        p.arg2 = "3".into();
        p.arg3 = fingerprint("x");
        let text = format!("{}{}{}", header(), line(&o), line(&p));
        let parsed = parse(&text).unwrap();
        assert_eq!(parsed.items, vec![o, p]);
        let bad = format!("{}e3\ti1\tshout\t{}\t{}\tDad\tobserved\t\t\t\tx\t\ne1\ti1\tnote\t{}\t{}\tDad\tobserved\t\t\t\tdup\t\ne4\t../x\tnote\t{}\t{}\tDad\tobserved\t\t\t\tx\t\n", text, T1, T1, T1, T1, T1, T1);
        let parsed = parse(&bad).unwrap();
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.rejected.len(), 3);
        assert!(parse("# format: security-incidents 5\n").is_err());
    }

    #[test]
    fn log_lines_for_an_unopened_incident_do_not_make_one() {
        let log = vec![e("e1", "i7", Kind::Note, T0, "", "orphan"), e("e2", "i7", Kind::Status, T0, "active", "orphan")];
        assert!(build(&log).is_empty());
    }

    #[test]
    fn fingerprint_is_stable_and_sensitive() {
        assert_eq!(fingerprint("abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_ne!(fingerprint("abc"), fingerprint("abd"));
        assert_eq!(fingerprint("abc").len(), 32);
    }
}
