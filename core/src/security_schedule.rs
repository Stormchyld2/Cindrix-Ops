//! Security Operations, part 2: the watchbill (who stands watch when) and what it covers.
//! Design: `docs/44-security-operations-architecture.md` sections 8 and 9, section 14 (handover). Plain logic.
//!
//! Rules this file keeps:
//!  * a draft roster is only ever `proposed`; a person has to schedule it, and the watch holder has to acknowledge it;
//!  * coverage is worked out per site and says "confirmed", "planned" or "gap" for every minute asked about;
//!  * a watch nobody acknowledged, or one that ended without a handover, is listed as "needs a look",
//!    never silently marked missed or completed;
//!  * every time carries its own offset, so a clock change cannot shift a watch by an hour.

use crate::ops::{esc, next_id, unesc};
use crate::security::{check_format, format_instant, parse_instant, valid_id, Parsed};

pub const WATCH_FORMAT: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchState {
    /// A suggestion. Covers nothing until someone schedules it.
    Proposed,
    /// Agreed, waiting for the person to acknowledge.
    Scheduled,
    /// The person said they have it.
    Acknowledged,
    OnDuty,
    /// Handed over or finished.
    Completed,
    /// Did not happen. Kept so the gap stays visible.
    Missed,
}

impl WatchState {
    pub fn word(self) -> &'static str {
        match self {
            WatchState::Proposed => "proposed",
            WatchState::Scheduled => "scheduled",
            WatchState::Acknowledged => "acknowledged",
            WatchState::OnDuty => "on-duty",
            WatchState::Completed => "completed",
            WatchState::Missed => "missed",
        }
    }
    pub fn from_word(s: &str) -> Option<WatchState> {
        Some(match s.trim() {
            "proposed" => WatchState::Proposed,
            "scheduled" => WatchState::Scheduled,
            "acknowledged" => WatchState::Acknowledged,
            "on-duty" => WatchState::OnDuty,
            "completed" => WatchState::Completed,
            "missed" => WatchState::Missed,
            _ => return None,
        })
    }
    fn confirmed(self) -> bool {
        matches!(self, WatchState::Acknowledged | WatchState::OnDuty | WatchState::Completed)
    }
    fn planned(self) -> bool {
        matches!(self, WatchState::Scheduled)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Watch {
    pub id: String,
    pub site: String,
    /// A person's name as the Family page has it.
    pub person: String,
    /// "watch", "relief", "runner" and so on; free words.
    pub role: String,
    /// Written instants with offsets (see `security::parse_instant`).
    pub start: String,
    pub end: String,
    pub state: WatchState,
    /// When the person acknowledged (or when it was entered afterwards from paper or radio), or empty.
    pub ack_at: String,
    /// When this entry was written. Kept apart from the event time so a handover entered late stays honest.
    pub entered_at: String,
    pub notes: String,
}

impl Watch {
    /// Start and end in UTC minutes; None when either is unreadable or the end is not after the start.
    pub fn span(&self) -> Option<(i64, i64)> {
        let (a, b) = (parse_instant(&self.start)?, parse_instant(&self.end)?);
        if b > a { Some((a, b)) } else { None }
    }
}

pub fn parse_watches(text: &str) -> Result<Parsed<Watch>, String> {
    check_format(text, "security-watch", WATCH_FORMAT)?;
    let mut p = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let state = WatchState::from_word(get(6));
        let w = Watch {
            id: get(0).to_string(),
            site: get(1).to_string(),
            person: unesc(get(2)).trim().chars().take(80).collect(),
            role: unesc(get(3)).trim().chars().take(40).collect(),
            start: get(4).trim().to_string(),
            end: get(5).trim().to_string(),
            state: state.unwrap_or(WatchState::Proposed),
            ack_at: get(7).trim().to_string(),
            entered_at: get(8).trim().to_string(),
            notes: unesc(get(9)).trim().chars().take(1000).collect(),
        };
        let times_ok = (w.ack_at.is_empty() || parse_instant(&w.ack_at).is_some()) && (w.entered_at.is_empty() || parse_instant(&w.entered_at).is_some());
        if !valid_id(&w.id) || !valid_id(&w.site) || w.person.is_empty() || state.is_none() || w.span().is_none() || !times_ok || p.items.iter().any(|x: &Watch| x.id == w.id) {
            p.rejected.push(l.to_string());
            continue;
        }
        p.items.push(w);
    }
    Ok(p)
}

pub fn serialize_watches(v: &[Watch]) -> String {
    let mut o = format!("# format: security-watch {}\n# id\tsite\tperson\trole\tstart\tend\tstate\tack_at\tentered_at\tnotes\n", WATCH_FORMAT);
    for w in v {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", w.id, w.site, esc(&w.person), esc(&w.role), w.start, w.end, w.state.word(), w.ack_at, w.entered_at, esc(&w.notes)));
    }
    o
}

// ---------- changing a watch ----------

/// Move a watch forward one step. Only these moves are allowed: proposed to scheduled, scheduled to
/// acknowledged, acknowledged to on-duty, on-duty to completed; anything not yet completed may become missed;
/// a missed watch may be scheduled again (someone has dealt with it). Completed never changes.
pub fn can_move(from: WatchState, to: WatchState) -> bool {
    use WatchState::*;
    matches!(
        (from, to),
        (Proposed, Scheduled) | (Scheduled, Acknowledged) | (Acknowledged, OnDuty) | (OnDuty, Completed) | (Proposed | Scheduled | Acknowledged | OnDuty, Missed) | (Missed, Scheduled)
    )
}

/// The person says they have the watch. `at` is when it really happened (it may be entered later from paper
/// or radio); `entered` is now.
pub fn acknowledge(w: &Watch, at: &str, entered: &str) -> Result<Watch, String> {
    if w.state != WatchState::Scheduled {
        return Err("Only a scheduled watch can be acknowledged.".to_string());
    }
    if parse_instant(at).is_none() || parse_instant(entered).is_none() {
        return Err("The times could not be read.".to_string());
    }
    Ok(Watch { state: WatchState::Acknowledged, ack_at: at.to_string(), entered_at: entered.to_string(), ..w.clone() })
}

pub fn change_state(w: &Watch, to: WatchState) -> Result<Watch, String> {
    if to == WatchState::Acknowledged {
        return Err("Use acknowledge, so the time is recorded.".to_string());
    }
    if !can_move(w.state, to) {
        return Err("That change is not allowed from where the watch is now.".to_string());
    }
    Ok(Watch { state: to, ..w.clone() })
}

// ---------- drafting a roster ----------

/// A draft watchbill: back-to-back watches of `hours` hours starting at `start`, handed round the people in
/// order. Every entry is `proposed`. Nothing is saved or scheduled by this function.
pub fn draft_watchbill(site: &str, people: &[String], start: &str, hours: u32, count: u32, existing: &[Watch]) -> Result<Vec<Watch>, String> {
    let s = parse_instant(start).ok_or("The start time needs a date, a time and the time zone offset.")?;
    if people.is_empty() {
        return Err("Add at least one person first.".to_string());
    }
    if !(1..=24).contains(&hours) || !(1..=200).contains(&count) {
        return Err("A watch is 1 to 24 hours, and a draft covers up to 200 watches.".to_string());
    }
    if !valid_id(site) {
        return Err("That site is not valid.".to_string());
    }
    let off = crate::security::instant_offset(start).unwrap_or(0);
    let mut ids: Vec<String> = existing.iter().map(|w| w.id.clone()).collect();
    let mut out = Vec::new();
    for i in 0..count as i64 {
        let (a, b) = (s + i * hours as i64 * 60, s + (i + 1) * hours as i64 * 60);
        let id = next_id('w', ids.iter().cloned());
        ids.push(id.clone());
        out.push(Watch {
            id,
            site: site.to_string(),
            person: people[(i as usize) % people.len()].clone(),
            role: "watch".to_string(),
            start: format_instant(a, off),
            end: format_instant(b, off),
            state: WatchState::Proposed,
            ack_at: String::new(),
            entered_at: String::new(),
            notes: String::new(),
        });
    }
    Ok(out)
}

// ---------- coverage ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    /// Someone has acknowledged (or stood, or finished) a watch here.
    Confirmed,
    /// A watch is scheduled but nobody has acknowledged it yet.
    Planned,
    Gap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub from: i64,
    pub to: i64,
    pub cover: Cover,
}

/// Cover for a site between `from` and `to` (UTC minutes), as joined-up segments. Proposed and missed watches
/// cover nothing.
pub fn coverage(watches: &[Watch], site: &str, from: i64, to: i64) -> Vec<Segment> {
    if to <= from {
        return Vec::new();
    }
    let spans: Vec<(i64, i64, Cover)> = watches
        .iter()
        .filter(|w| w.site == site)
        .filter_map(|w| {
            let (a, b) = w.span()?;
            let c = if w.state.confirmed() { Cover::Confirmed } else if w.state.planned() { Cover::Planned } else { return None };
            Some((a, b, c))
        })
        .collect();
    let mut cuts = vec![from, to];
    for (a, b, _) in &spans {
        for t in [*a, *b] {
            if t > from && t < to {
                cuts.push(t);
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<Segment> = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let inside = |c: Cover| spans.iter().any(|(x, y, k)| *k == c && *x <= a && *y >= b);
        let cover = if inside(Cover::Confirmed) { Cover::Confirmed } else if inside(Cover::Planned) { Cover::Planned } else { Cover::Gap };
        match out.last_mut() {
            Some(last) if last.cover == cover => last.to = b,
            _ => out.push(Segment { from: a, to: b, cover }),
        }
    }
    out
}

/// Minutes of `from..to` that are not Confirmed or Planned.
pub fn gap_minutes(segs: &[Segment]) -> i64 {
    segs.iter().filter(|s| s.cover == Cover::Gap).map(|s| s.to - s.from).sum()
}

/// Two covering watches at the same site that overlap by more than `allowed_minutes` (a short overlap at a
/// handover is normal). Reported as watch id pairs; the same person twice is reported by `double_booked`.
pub fn overlaps(watches: &[Watch], allowed_minutes: i64) -> Vec<(String, String, i64)> {
    let cov: Vec<(&Watch, i64, i64)> = watches.iter().filter(|w| w.state.confirmed() || w.state.planned()).filter_map(|w| w.span().map(|(a, b)| (w, a, b))).collect();
    let mut out = Vec::new();
    for i in 0..cov.len() {
        for j in i + 1..cov.len() {
            let (x, y) = (&cov[i], &cov[j]);
            if x.0.site != y.0.site {
                continue;
            }
            let len = x.2.min(y.2) - x.1.max(y.1);
            if len > allowed_minutes {
                out.push((x.0.id.clone(), y.0.id.clone(), len));
            }
        }
    }
    out
}

/// The same person on two watches at once, at any site. Pairs of watch ids.
pub fn double_booked(watches: &[Watch]) -> Vec<(String, String)> {
    let live: Vec<(&Watch, i64, i64)> = watches.iter().filter(|w| !matches!(w.state, WatchState::Missed)).filter_map(|w| w.span().map(|(a, b)| (w, a, b))).collect();
    let mut out = Vec::new();
    for i in 0..live.len() {
        for j in i + 1..live.len() {
            let (x, y) = (&live[i], &live[j]);
            if x.0.person.eq_ignore_ascii_case(&y.0.person) && x.1.max(y.1) < x.2.min(y.2) {
                out.push((x.0.id.clone(), y.0.id.clone()));
            }
        }
    }
    out
}

/// People who get less than `min_rest_hours` between the end of one watch and the start of their next.
/// Returns (person, first watch id, next watch id, rest in minutes). A warning to read, not a ruling.
pub fn short_rest(watches: &[Watch], min_rest_hours: i64) -> Vec<(String, String, String, i64)> {
    let mut mine: Vec<(String, i64, i64, &str)> = watches
        .iter()
        .filter(|w| !matches!(w.state, WatchState::Missed | WatchState::Proposed))
        .filter_map(|w| w.span().map(|(a, b)| (w.person.to_lowercase(), a, b, w.id.as_str())))
        .collect();
    mine.sort();
    let mut out = Vec::new();
    for pair in mine.windows(2) {
        let (x, y) = (&pair[0], &pair[1]);
        if x.0 == y.0 && y.1 >= x.2 && y.1 - x.2 < min_rest_hours * 60 {
            out.push((x.0.clone(), x.3.to_string(), y.3.to_string(), y.1 - x.2));
        }
    }
    out
}

// ---------- needs a look ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    /// The watch has started and nobody has acknowledged it.
    NotAcknowledged,
    /// The watch has ended and nobody has recorded the handover.
    HandoverOpen,
}

/// Watches that need a person to look at them now. Nothing is changed: a person decides whether it was missed,
/// handed over by radio, or something else.
pub fn needs_a_look(watches: &[Watch], now: i64) -> Vec<(String, Attention)> {
    let mut out = Vec::new();
    for w in watches {
        let Some((a, b)) = w.span() else { continue };
        match w.state {
            WatchState::Scheduled if now >= a => out.push((w.id.clone(), Attention::NotAcknowledged)),
            WatchState::Acknowledged | WatchState::OnDuty if now >= b => out.push((w.id.clone(), Attention::HandoverOpen)),
            _ => {}
        }
    }
    out
}

/// Who is on watch at a site right now: acknowledged or on-duty watches that include `now`.
pub fn on_watch_now<'a>(watches: &'a [Watch], site: &str, now: i64) -> Vec<&'a Watch> {
    watches.iter().filter(|w| w.site == site && matches!(w.state, WatchState::Acknowledged | WatchState::OnDuty) && w.span().map(|(a, b)| a <= now && now < b).unwrap_or(false)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> i64 {
        parse_instant(s).unwrap()
    }
    fn w(id: &str, site: &str, person: &str, start: &str, end: &str, state: WatchState) -> Watch {
        Watch { id: id.into(), site: site.into(), person: person.into(), role: "watch".into(), start: start.into(), end: end.into(), state, ack_at: String::new(), entered_at: String::new(), notes: String::new() }
    }
    const D: &str = "2026-11-08T";

    #[test]
    fn draft_is_proposed_only_and_round_robin() {
        let people: Vec<String> = ["Dad", "Mom", "Sam", "Lee"].iter().map(|s| s.to_string()).collect();
        let d = draft_watchbill("camp", &people, "2026-11-08T00:00-05:00", 6, 8, &[]).unwrap();
        assert_eq!(d.len(), 8);
        assert!(d.iter().all(|x| x.state == WatchState::Proposed));
        assert_eq!(d[0].person, "Dad");
        assert_eq!(d[4].person, "Dad");
        assert_eq!(d[1].start, "2026-11-08T06:00-05:00");
        assert_eq!(d[7].end, "2026-11-10T00:00-05:00");
        // a draft covers nothing until it is scheduled
        let segs = coverage(&d, "camp", t("2026-11-08T00:00-05:00"), t("2026-11-10T00:00-05:00"));
        assert_eq!(segs, vec![Segment { from: t("2026-11-08T00:00-05:00"), to: t("2026-11-10T00:00-05:00"), cover: Cover::Gap }]);
        // ids do not clash with existing ones
        let more = draft_watchbill("camp", &people, "2026-11-10T00:00-05:00", 6, 2, &d).unwrap();
        assert_eq!(more[0].id, "w9");
        assert!(draft_watchbill("camp", &[], "2026-11-08T00:00-05:00", 6, 2, &[]).is_err());
        assert!(draft_watchbill("camp", &people, "tomorrow", 6, 2, &[]).is_err());
        assert!(draft_watchbill("camp", &people, "2026-11-08T00:00Z", 0, 2, &[]).is_err());
    }

    #[test]
    fn draft_across_a_clock_change_keeps_real_hours() {
        let people = vec!["Dad".to_string()];
        // Clocks go back on 2026-11-01 in the US. A 6-hour watch written in -04:00 stays 6 real hours.
        let d = draft_watchbill("home", &people, "2026-11-01T00:00-04:00", 6, 1, &[]).unwrap();
        let (a, b) = d[0].span().unwrap();
        assert_eq!(b - a, 360);
    }

    #[test]
    fn state_changes_follow_the_lifecycle() {
        use WatchState::*;
        assert!(can_move(Proposed, Scheduled) && can_move(Scheduled, Acknowledged) && can_move(OnDuty, Completed) && can_move(Scheduled, Missed) && can_move(Missed, Scheduled));
        assert!(!can_move(Proposed, OnDuty) && !can_move(Completed, Missed) && !can_move(Completed, Scheduled) && !can_move(Scheduled, Completed));
        let x = w("w1", "camp", "Dad", "2026-11-08T00:00Z", "2026-11-08T06:00Z", Scheduled);
        assert!(change_state(&x, Acknowledged).is_err());
        let a = acknowledge(&x, "2026-11-07T20:00Z", "2026-11-08T01:00Z").unwrap();
        assert_eq!(a.state, Acknowledged);
        assert_eq!(a.ack_at, "2026-11-07T20:00Z");
        assert_eq!(a.entered_at, "2026-11-08T01:00Z");
        assert!(acknowledge(&a, "2026-11-07T20:00Z", "2026-11-08T01:00Z").is_err());
        assert!(acknowledge(&x, "later", "2026-11-08T01:00Z").is_err());
        let p = w("w2", "camp", "Mom", "2026-11-08T06:00Z", "2026-11-08T12:00Z", Proposed);
        assert!(acknowledge(&p, "2026-11-07T20:00Z", "2026-11-08T01:00Z").is_err());
    }

    #[test]
    fn coverage_shows_confirmed_planned_and_gap() {
        let ws = vec![
            w("w1", "home", "Dad", "2026-11-08T00:00Z", "2026-11-08T06:00Z", WatchState::Completed),
            w("w2", "home", "Mom", "2026-11-08T06:00Z", "2026-11-08T12:00Z", WatchState::Acknowledged),
            w("w3", "home", "Sam", "2026-11-08T12:00Z", "2026-11-08T16:00Z", WatchState::Scheduled),
            // 16:00 to 18:00 nobody
            w("w4", "home", "Lee", "2026-11-08T18:00Z", "2026-11-09T00:00Z", WatchState::Proposed),
            w("w5", "camp", "Dad", "2026-11-08T00:00Z", "2026-11-09T00:00Z", WatchState::Acknowledged),
            w("w6", "home", "Dad", "2026-11-08T16:00Z", "2026-11-08T18:00Z", WatchState::Missed),
        ];
        let segs = coverage(&ws, "home", t("2026-11-08T00:00Z"), t("2026-11-09T00:00Z"));
        let covers: Vec<Cover> = segs.iter().map(|s| s.cover).collect();
        assert_eq!(covers, vec![Cover::Confirmed, Cover::Planned, Cover::Gap]);
        assert_eq!(segs[0].to, t("2026-11-08T12:00Z"));
        assert_eq!(segs[2].from, t("2026-11-08T16:00Z"));
        assert_eq!(gap_minutes(&segs), 8 * 60);
        // another site's watches never fill this site's gaps
        assert_eq!(coverage(&ws, "camp", t("2026-11-08T00:00Z"), t("2026-11-09T00:00Z")).len(), 1);
        // empty or backwards window
        assert!(coverage(&ws, "home", 10, 10).is_empty());
    }

    #[test]
    fn confirmed_beats_planned_where_they_overlap() {
        let ws = vec![w("a", "home", "Dad", "2026-11-08T00:00Z", "2026-11-08T06:00Z", WatchState::Scheduled), w("b", "home", "Mom", "2026-11-08T05:00Z", "2026-11-08T08:00Z", WatchState::Acknowledged)];
        let segs = coverage(&ws, "home", t("2026-11-08T00:00Z"), t("2026-11-08T08:00Z"));
        let covers: Vec<Cover> = segs.iter().map(|s| s.cover).collect();
        assert_eq!(covers, vec![Cover::Planned, Cover::Confirmed]);
    }

    #[test]
    fn overlap_and_double_booking_are_found() {
        let ws = vec![
            w("a", "home", "Dad", &format!("{}00:00Z", D), &format!("{}06:00Z", D), WatchState::Scheduled),
            w("b", "home", "Mom", &format!("{}05:50Z", D), &format!("{}12:00Z", D), WatchState::Scheduled),
            w("c", "home", "Sam", &format!("{}04:00Z", D), &format!("{}09:00Z", D), WatchState::Scheduled),
            w("d", "camp", "Dad", &format!("{}02:00Z", D), &format!("{}03:00Z", D), WatchState::Scheduled),
        ];
        // a 10 minute handover overlap is allowed at 15; a 2-hour one is not
        let o = overlaps(&ws, 15);
        assert!(!o.iter().any(|(x, y, _)| x == "a" && y == "b"));
        assert!(o.iter().any(|(x, y, len)| x == "a" && y == "c" && *len == 120));
        // the camp watch is a different site, so no overlap with home
        assert!(!o.iter().any(|(x, y, _)| x == "d" || y == "d"));
        // but Dad is on two places at once
        assert_eq!(double_booked(&ws), vec![("a".to_string(), "d".to_string())]);
    }

    #[test]
    fn short_rest_is_a_warning() {
        let ws = vec![
            w("a", "home", "Dad", &format!("{}00:00Z", D), &format!("{}06:00Z", D), WatchState::Scheduled),
            w("b", "home", "Dad", &format!("{}09:00Z", D), &format!("{}15:00Z", D), WatchState::Scheduled),
            w("c", "home", "Mom", &format!("{}06:00Z", D), &format!("{}12:00Z", D), WatchState::Scheduled),
            w("d", "home", "Mom", &format!("{}23:00Z", D), "2026-11-09T05:00Z", WatchState::Scheduled),
        ];
        let r = short_rest(&ws, 6);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].0.as_str(), r[0].1.as_str(), r[0].2.as_str(), r[0].3), ("dad", "a", "b", 180));
    }

    #[test]
    fn missed_handover_stays_open_and_is_never_auto_closed() {
        let ws = vec![
            w("a", "home", "Dad", &format!("{}00:00Z", D), &format!("{}06:00Z", D), WatchState::Scheduled),
            w("b", "home", "Mom", &format!("{}00:00Z", D), &format!("{}06:00Z", D), WatchState::OnDuty),
            w("c", "home", "Sam", &format!("{}00:00Z", D), &format!("{}06:00Z", D), WatchState::Completed),
            w("d", "home", "Lee", &format!("{}12:00Z", D), &format!("{}18:00Z", D), WatchState::Scheduled),
        ];
        let now = t(&format!("{}07:00Z", D));
        let n = needs_a_look(&ws, now);
        assert_eq!(n, vec![("a".to_string(), Attention::NotAcknowledged), ("b".to_string(), Attention::HandoverOpen)]);
        // listing it changed nothing
        assert_eq!(ws[0].state, WatchState::Scheduled);
        // nobody is shown as on watch once the time has passed
        assert!(on_watch_now(&ws, "home", now).is_empty());
        assert_eq!(on_watch_now(&ws, "home", t(&format!("{}03:00Z", D))).len(), 1);
    }

    #[test]
    fn watch_file_round_trips_and_keeps_bad_lines() {
        let mut x = w("w1", "camp", "Dad", "2026-11-08T00:00-05:00", "2026-11-08T06:00-05:00", WatchState::Acknowledged);
        x.ack_at = "2026-11-07T20:00-05:00".into();
        x.entered_at = "2026-11-08T01:00-05:00".into();
        x.notes = "Radio\nchannel 3".into();
        let text = serialize_watches(&[x.clone()]);
        assert_eq!(parse_watches(&text).unwrap().items, vec![x.clone()]);
        let bad = format!(
            "{}w2\tcamp\tMom\twatch\t2026-11-08T06:00Z\t2026-11-08T00:00Z\tscheduled\t\t\t\nw1\tcamp\tDup\twatch\t2026-11-08T06:00Z\t2026-11-08T07:00Z\tscheduled\t\t\t\nw3\t../x\tMom\twatch\t2026-11-08T06:00Z\t2026-11-08T07:00Z\tscheduled\t\t\t\nw4\tcamp\tMom\twatch\t2026-11-08T06:00Z\t2026-11-08T07:00Z\tdone-ish\t\t\t\n",
            text
        );
        let p = parse_watches(&bad).unwrap();
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.rejected.len(), 4);
        assert!(parse_watches("# format: security-watch 9\n").is_err());
    }
}
