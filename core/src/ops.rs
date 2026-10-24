//! The connecting layer: tasks (the to-do list), calendar events and projects, and what ties them to the rest.
//!
//! Everything here is plain logic with no files and no screen, so it can be tested on its own:
//!  * records with stable ids, saved as tab-separated text (`user/tasks.txt`, `events.txt`, `projects.txt`);
//!  * links from any record to any other thing in the program (an inventory item, a plan, a card, a guide page,
//!    a person, a project), written `kind:id`, so a screen can show everything connected to one thing;
//!  * the agenda: one date-ordered list built from tasks, events, and the dates other modules supply
//!    (things that expire, plan checks that fall due), by plain rules and no guessing;
//!  * one search over titles and text that every module can feed;
//!  * a short change log, so the Dashboard can say what changed lately.

use crate::calendar;

// ---------- text helpers ----------

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            c => o.push(c),
        }
    }
    o
}

fn unesc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('t') => o.push('\t'),
                Some('n') => o.push('\n'),
                Some('\\') => o.push('\\'),
                Some(x) => { o.push('\\'); o.push(x); }
                None => o.push('\\'),
            }
        } else {
            o.push(c);
        }
    }
    o
}

/// A date the program accepts: `YYYY-MM-DD`, or empty.
fn clean_date(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() || calendar::parse_date(s).is_some() { s.to_string() } else { String::new() }
}

fn clean_time(s: &str) -> String {
    let s = s.trim();
    let ok = s.len() == 5 && s.as_bytes()[2] == b':' && s[..2].parse::<u32>().map(|h| h < 24).unwrap_or(false) && s[3..].parse::<u32>().map(|m| m < 60).unwrap_or(false);
    if ok { s.to_string() } else { String::new() }
}

// ---------- links ----------

/// A pointer from one record to another thing in the program, such as `item:12`, `plan:water-storage` or `card:wound-bleeding`.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub kind: String,
    pub id: String,
}

pub const LINK_KINDS: &[&str] = &["item", "plan", "card", "guide", "person", "project", "task", "event", "course", "drill", "chat"];

fn link_ok(kind: &str, id: &str) -> bool {
    LINK_KINDS.contains(&kind) && !id.is_empty() && id.len() <= 160 && !id.contains(['\t', '\n', ',', '\r'])
}

pub fn parse_links(s: &str) -> Vec<Link> {
    s.split(',')
        .filter_map(|p| {
            let (k, i) = p.split_once(':')?;
            let (k, i) = (k.trim(), i.trim());
            if link_ok(k, i) { Some(Link { kind: k.to_string(), id: i.to_string() }) } else { None }
        })
        .collect()
}

pub fn links_text(v: &[Link]) -> String {
    v.iter().filter(|l| link_ok(&l.kind, &l.id)).map(|l| format!("{}:{}", l.kind, l.id)).collect::<Vec<_>>().join(",")
}

// ---------- tasks ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Task {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD` or empty.
    pub due: String,
    /// The day it was finished, or empty. A repeating task is never finished: its due date moves on instead.
    pub done: String,
    pub project: String,
    /// 0 for a one-time task; otherwise it comes back this many days after it is ticked.
    pub every_days: u32,
    pub notes: String,
    pub links: Vec<Link>,
}

pub fn parse_tasks(text: &str) -> Vec<Task> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 2 || f[0].is_empty() || f[1].trim().is_empty() {
                return None;
            }
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            Some(Task {
                id: f[0].to_string(),
                title: unesc(f[1]),
                due: clean_date(get(2)),
                done: clean_date(get(3)),
                project: get(4).to_string(),
                every_days: get(5).parse().unwrap_or(0),
                notes: unesc(get(6)),
                links: parse_links(get(7)),
            })
        })
        .collect()
}

pub fn serialize_tasks(v: &[Task]) -> String {
    let mut o = String::from("# id\ttitle\tdue\tdone\tproject\tevery_days\tnotes\tlinks\n");
    for t in v {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", t.id, esc(&t.title), t.due, t.done, t.project, t.every_days, esc(&t.notes), links_text(&t.links)));
    }
    o
}

/// The next free id with the given letter in front, such as `t7`.
pub fn next_id(prefix: char, existing: impl Iterator<Item = String>) -> String {
    let max = existing.filter_map(|i| i.strip_prefix(prefix).and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0);
    format!("{}{}", prefix, max + 1)
}

/// Tick a task. A one-time task is marked done today; a repeating one moves its due date forward and stays open.
pub fn complete(t: &Task, today: i64) -> Task {
    let mut n = t.clone();
    if t.every_days > 0 {
        let (y, m, d) = calendar::civil_from_days(today + t.every_days as i64);
        n.due = format!("{:04}-{:02}-{:02}", y, m, d);
        n.done.clear();
    } else {
        let (y, m, d) = calendar::civil_from_days(today);
        n.done = format!("{:04}-{:02}-{:02}", y, m, d);
    }
    n
}

pub fn reopen(t: &Task) -> Task {
    Task { done: String::new(), ..t.clone() }
}

// ---------- calendar events ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub date: String,
    /// `HH:MM` or empty for an all-day event.
    pub time: String,
    /// "", "weekly", "monthly" or "yearly".
    pub repeat: String,
    pub project: String,
    pub notes: String,
    pub links: Vec<Link>,
}

pub fn parse_events(text: &str) -> Vec<Event> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            if f.len() < 3 || f[0].is_empty() || f[1].trim().is_empty() || clean_date(get(2)).is_empty() {
                return None;
            }
            let rep = match get(4) { "weekly" => "weekly", "monthly" => "monthly", "yearly" => "yearly", _ => "" };
            Some(Event { id: f[0].to_string(), title: unesc(f[1]), date: get(2).to_string(), time: clean_time(get(3)), repeat: rep.to_string(), project: get(5).to_string(), notes: unesc(get(6)), links: parse_links(get(7)) })
        })
        .collect()
}

pub fn serialize_events(v: &[Event]) -> String {
    let mut o = String::from("# id\ttitle\tdate\ttime\trepeat\tproject\tnotes\tlinks\n");
    for e in v {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", e.id, esc(&e.title), e.date, e.time, e.repeat, e.project, esc(&e.notes), links_text(&e.links)));
    }
    o
}

fn days_in_month(y: i32, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (calendar::days_from_civil(ny, nm, 1) - calendar::days_from_civil(y, m, 1)) as u32
}

/// The days (as day numbers) on which an event falls between `from` and `to`, both included.
pub fn occurrences(e: &Event, from: i64, to: i64) -> Vec<i64> {
    let Some(start) = calendar::parse_date(&e.date) else { return Vec::new() };
    let mut out = Vec::new();
    match e.repeat.as_str() {
        "weekly" => {
            if to >= start {
                let first = if from <= start { start } else { start + ((from - start + 6) / 7) * 7 };
                let mut d = first;
                while d <= to && out.len() < 400 { out.push(d); d += 7; }
            }
        }
        "monthly" | "yearly" => {
            let (sy, sm, sd) = calendar::civil_from_days(start);
            let step = if e.repeat == "yearly" { 12 } else { 1 };
            let (fy, fm, _) = calendar::civil_from_days(from.max(start));
            let mut k = ((fy - sy) * 12 + fm as i32 - sm as i32).max(0);
            k -= k % step;
            for _ in 0..400 {
                let total = sm as i32 - 1 + k;
                let (y, m) = (sy + total.div_euclid(12), (total.rem_euclid(12) + 1) as u32);
                // a repeat on the 31st lands on the last day of a shorter month
                let d = sd.min(days_in_month(y, m));
                let day = calendar::days_from_civil(y, m, d);
                if day > to { break; }
                if day >= from && day >= start { out.push(day); }
                k += step;
            }
        }
        _ => {
            if start >= from && start <= to { out.push(start); }
        }
    }
    out
}

// ---------- projects ----------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Project {
    pub id: String,
    pub name: String,
    /// "open" or "done".
    pub status: String,
    pub notes: String,
    /// Saved Ask chats that belong to this project (file names in `user/chats`).
    pub chats: Vec<String>,
}

pub fn parse_projects(text: &str) -> Vec<Project> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let get = |i: usize| f.get(i).copied().unwrap_or("");
            if f.len() < 2 || f[0].is_empty() || f[1].trim().is_empty() {
                return None;
            }
            let chats = get(4).split('|').filter(|c| !c.is_empty() && c.ends_with(".txt") && !c.contains(['/', '\\', ':'])).map(|c| c.to_string()).collect();
            Some(Project { id: f[0].to_string(), name: unesc(f[1]), status: if get(2) == "done" { "done".into() } else { "open".into() }, notes: unesc(get(3)), chats })
        })
        .collect()
}

pub fn serialize_projects(v: &[Project]) -> String {
    let mut o = String::from("# id\tname\tstatus\tnotes\tchats\n");
    for p in v {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", p.id, esc(&p.name), p.status, esc(&p.notes), p.chats.join("|")));
    }
    o
}

/// Put a saved chat in a project (taking it out of any other, since a chat belongs to one project).
pub fn file_chat(projects: &mut [Project], project_id: &str, chat: &str) -> bool {
    if !chat.ends_with(".txt") || chat.contains(['/', '\\', ':', '|']) {
        return false;
    }
    for p in projects.iter_mut() {
        p.chats.retain(|c| c != chat);
    }
    match projects.iter_mut().find(|p| p.id == project_id) {
        Some(p) => { p.chats.push(chat.to_string()); true }
        None => project_id.is_empty(), // an empty project id just takes the chat out of every project
    }
}

pub fn project_of_chat<'a>(projects: &'a [Project], chat: &str) -> Option<&'a Project> {
    projects.iter().find(|p| p.chats.iter().any(|c| c == chat))
}

// ---------- the agenda ----------

/// A dated thing another module knows about, such as an item that expires or a plan check that falls due.
#[derive(Debug, Clone, PartialEq)]
pub struct Dated {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub day: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgendaEntry {
    pub date: String,
    pub day: i64,
    /// "task", "event", or whatever kind another module supplied ("item", "plan").
    pub kind: String,
    pub id: String,
    pub title: String,
    pub time: String,
    pub overdue: bool,
    pub project: String,
}

fn ymd(day: i64) -> String {
    let (y, m, d) = calendar::civil_from_days(day);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

/// Everything dated between `from` and `to` in one list, earliest first. Open tasks and other modules' dates that are
/// already late are shown on `from` and marked overdue, so nothing slips off the list.
pub fn agenda(tasks: &[Task], events: &[Event], extra: &[Dated], from: i64, to: i64) -> Vec<AgendaEntry> {
    let mut out: Vec<(i64, AgendaEntry)> = Vec::new();
    for t in tasks.iter().filter(|t| t.done.is_empty()) {
        let Some(d) = calendar::parse_date(&t.due) else { continue };
        if d > to { continue; }
        let late = d < from;
        let day = if late { from } else { d };
        out.push((d, AgendaEntry { date: ymd(day), day, kind: "task".into(), id: t.id.clone(), title: t.title.clone(), time: String::new(), overdue: late, project: t.project.clone() }));
    }
    for e in events {
        for d in occurrences(e, from, to) {
            out.push((d, AgendaEntry { date: ymd(d), day: d, kind: "event".into(), id: e.id.clone(), title: e.title.clone(), time: e.time.clone(), overdue: false, project: e.project.clone() }));
        }
    }
    for x in extra {
        if x.day > to { continue; }
        let late = x.day < from;
        let day = if late { from } else { x.day };
        out.push((x.day, AgendaEntry { date: ymd(day), day, kind: x.kind.clone(), id: x.id.clone(), title: x.title.clone(), time: String::new(), overdue: late, project: String::new() }));
    }
    // late things first (the longest overdue at the top), then by day, then timed events by time, then kind and title for a steady order
    out.sort_by(|(oa, a), (ob, b)| (!a.overdue, *oa, a.day, a.time.is_empty(), a.time.clone(), a.kind.clone(), a.title.clone()).cmp(&(!b.overdue, *ob, b.day, b.time.is_empty(), b.time.clone(), b.kind.clone(), b.title.clone())));
    out.into_iter().map(|(_, e)| e).collect()
}

// ---------- what is connected to what ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Ref {
    pub kind: String,
    pub id: String,
    pub title: String,
}

/// Everything that points at `(kind, id)`: tasks, events and projects that link to it, the tasks and events inside a project,
/// and the chats filed under it. Sorted by kind then title so the screen is steady.
pub fn related(kind: &str, id: &str, tasks: &[Task], events: &[Event], projects: &[Project]) -> Vec<Ref> {
    let has = |links: &[Link]| links.iter().any(|l| l.kind == kind && l.id == id);
    let mut out: Vec<Ref> = Vec::new();
    let in_project = kind == "project";
    for t in tasks {
        if has(&t.links) || (in_project && t.project == id) {
            out.push(Ref { kind: "task".into(), id: t.id.clone(), title: t.title.clone() });
        }
    }
    for e in events {
        if has(&e.links) || (in_project && e.project == id) {
            out.push(Ref { kind: "event".into(), id: e.id.clone(), title: e.title.clone() });
        }
    }
    if in_project {
        if let Some(p) = projects.iter().find(|p| p.id == id) {
            for c in &p.chats {
                out.push(Ref { kind: "chat".into(), id: c.clone(), title: c.trim_end_matches(".txt").to_string() });
            }
        }
    }
    out.sort_by(|a, b| (a.kind.clone(), a.title.to_lowercase()).cmp(&(b.kind.clone(), b.title.to_lowercase())));
    out
}

// ---------- one search for every module ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub score: u32,
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_string()).collect()
}

/// Every word of the question must appear in the title or text; a word in the title counts more, and a whole-word match more again.
pub fn search(entries: &[Entry], query: &str, limit: usize) -> Vec<Hit> {
    let q = words(query);
    if q.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Hit> = Vec::new();
    for e in entries {
        let (tl, bl) = (e.title.to_lowercase(), e.text.to_lowercase());
        let (tw, bw) = (words(&e.title), words(&e.text));
        let mut score = 0u32;
        let mut all = true;
        for w in &q {
            let mut s = 0;
            if tl.contains(w.as_str()) { s += 3; if tw.iter().any(|x| x == w) { s += 2; } }
            if bl.contains(w.as_str()) { s += 1; if bw.iter().any(|x| x == w) { s += 1; } }
            if s == 0 { all = false; break; }
            score += s;
        }
        if !all { continue; }
        let snippet = if e.text.is_empty() { String::new() } else { snippet_of(&e.text, &q[0]) };
        hits.push(Hit { kind: e.kind.clone(), id: e.id.clone(), title: e.title.clone(), snippet, score });
    }
    hits.sort_by(|a, b| b.score.cmp(&a.score).then(a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    hits.truncate(limit);
    hits
}

fn snippet_of(text: &str, word: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let low = flat.to_lowercase();
    // find in the lowercase copy, then cut the original on a character boundary
    let at = low.find(word).map(|b| low[..b].chars().count()).unwrap_or(0);
    let start = at.saturating_sub(40);
    let s: String = flat.chars().skip(start).take(120).collect();
    format!("{}{}{}", if start > 0 { "…" } else { "" }, s.trim(), if flat.chars().count() > start + 120 { "…" } else { "" })
}

// ---------- the change log ----------

/// One line for `user/changes.log`: the day and time (seconds since 1970), what kind of thing, its id, and what happened.
pub fn change_line(now_secs: u64, kind: &str, id: &str, action: &str) -> String {
    let clean = |s: &str| s.chars().filter(|c| !c.is_control() && *c != '\t').take(80).collect::<String>();
    format!("{}\t{}\t{}\t{}", now_secs, clean(kind), clean(id), clean(action))
}

/// Keep only the newest `keep` lines.
pub fn trim_log(text: &str, keep: usize) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(keep);
    let mut o = lines[from..].join("\n");
    if !o.is_empty() { o.push('\n'); }
    o
}

/// The newest `n` log lines, newest first, as (time, kind, id, action).
pub fn recent_changes(text: &str, n: usize) -> Vec<(u64, String, String, String)> {
    let mut v: Vec<(u64, String, String, String)> = text
        .lines()
        .filter_map(|l| {
            let mut p = l.splitn(4, '\t');
            Some((p.next()?.parse().ok()?, p.next()?.to_string(), p.next()?.to_string(), p.next()?.to_string()))
        })
        .collect();
    v.reverse();
    v.truncate(n);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> i64 { calendar::parse_date(s).unwrap() }

    #[test]
    fn tasks_round_trip_with_awkward_text() {
        let t = Task { id: "t1".into(), title: "Check\tthe \"water\"".into(), due: "2026-11-01".into(), done: String::new(), project: "p1".into(), every_days: 30, notes: "line one\nline two\\end".into(), links: vec![Link { kind: "item".into(), id: "12".into() }, Link { kind: "plan".into(), id: "water-storage".into() }] };
        let back = parse_tasks(&serialize_tasks(&[t.clone()]));
        assert_eq!(back, vec![t]);
    }

    #[test]
    fn bad_lines_and_dates_are_dropped_not_fatal() {
        let v = parse_tasks("# header\n\nt1\tGood\t2026-13-45\t\t\t0\t\t\nonlyone\nt2\t\t\n");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].due, "", "an impossible date is cleared");
        assert!(parse_events("e1\tNo date\t\t\n").is_empty());
    }

    #[test]
    fn links_only_keep_known_kinds_and_safe_ids() {
        let l = parse_links("item:3, nonsense:1, plan:, card:a-b,guide:x");
        assert_eq!(links_text(&l), "item:3,card:a-b,guide:x");
    }

    #[test]
    fn next_id_goes_past_the_largest() {
        assert_eq!(next_id('t', ["t1", "t9", "t3", "e50"].iter().map(|s| s.to_string())), "t10");
        assert_eq!(next_id('t', std::iter::empty()), "t1");
    }

    #[test]
    fn ticking_a_one_time_task_marks_it_done_and_a_repeating_one_moves_on() {
        let one = Task { id: "t1".into(), title: "x".into(), ..Default::default() };
        assert_eq!(complete(&one, day("2026-10-24")).done, "2026-10-24");
        let rep = Task { id: "t2".into(), title: "Rotate water".into(), due: "2026-10-01".into(), every_days: 180, ..Default::default() };
        let n = complete(&rep, day("2026-10-24"));
        assert_eq!(n.due, "2027-04-22");
        assert!(n.done.is_empty());
        assert!(reopen(&complete(&one, 0)).done.is_empty());
    }

    #[test]
    fn events_repeat_on_the_right_days() {
        let w = Event { id: "e1".into(), title: "Radio check".into(), date: "2026-10-04".into(), repeat: "weekly".into(), ..Default::default() };
        let d = occurrences(&w, day("2026-10-20"), day("2026-11-03"));
        assert_eq!(d, vec![day("2026-10-25"), day("2026-11-01")]);
        let m = Event { id: "e2".into(), title: "Pay".into(), date: "2026-01-31".into(), repeat: "monthly".into(), ..Default::default() };
        let d = occurrences(&m, day("2026-02-01"), day("2026-04-30"));
        assert_eq!(d, vec![day("2026-02-28"), day("2026-03-31"), day("2026-04-30")], "the 31st lands on the last day of a short month");
        let y = Event { id: "e3".into(), title: "Birthday".into(), date: "2020-11-05".into(), repeat: "yearly".into(), ..Default::default() };
        assert_eq!(occurrences(&y, day("2026-10-01"), day("2027-12-31")), vec![day("2026-11-05"), day("2027-11-05")]);
        let once = Event { id: "e4".into(), title: "Once".into(), date: "2026-10-30".into(), ..Default::default() };
        assert_eq!(occurrences(&once, day("2026-10-01"), day("2026-10-29")), Vec::<i64>::new());
        assert_eq!(occurrences(&once, day("2026-10-01"), day("2026-10-30")).len(), 1);
    }

    #[test]
    fn a_repeat_does_not_show_before_it_starts() {
        let w = Event { id: "e1".into(), title: "x".into(), date: "2026-12-01".into(), repeat: "weekly".into(), ..Default::default() };
        assert!(occurrences(&w, day("2026-10-01"), day("2026-11-30")).is_empty());
        let m = Event { id: "e2".into(), title: "x".into(), date: "2026-12-15".into(), repeat: "monthly".into(), ..Default::default() };
        assert_eq!(occurrences(&m, day("2026-10-01"), day("2027-01-31")), vec![day("2026-12-15"), day("2027-01-15")]);
    }

    #[test]
    fn the_agenda_merges_everything_and_puts_late_things_first() {
        let tasks = vec![
            Task { id: "t1".into(), title: "Late job".into(), due: "2026-10-20".into(), ..Default::default() },
            Task { id: "t2".into(), title: "Done job".into(), due: "2026-10-25".into(), done: "2026-10-24".into(), ..Default::default() },
            Task { id: "t3".into(), title: "No date".into(), ..Default::default() },
            Task { id: "t4".into(), title: "Soon".into(), due: "2026-10-26".into(), ..Default::default() },
        ];
        let events = vec![Event { id: "e1".into(), title: "Call".into(), date: "2026-10-26".into(), time: "09:00".into(), ..Default::default() }];
        let extra = vec![
            Dated { kind: "item".into(), id: "5".into(), title: "Water filter expires".into(), day: day("2026-10-22") },
            Dated { kind: "plan".into(), id: "water".into(), title: "Check water".into(), day: day("2026-12-01") },
        ];
        let a = agenda(&tasks, &events, &extra, day("2026-10-24"), day("2026-11-07"));
        let titles: Vec<&str> = a.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(titles, vec!["Late job", "Water filter expires", "Call", "Soon"]);
        assert!(a[0].overdue && a[1].overdue && !a[2].overdue);
        assert_eq!(a[0].date, "2026-10-24", "late things sit on today");
        assert_eq!(a[2].time, "09:00");
    }

    #[test]
    fn related_finds_what_points_at_a_thing_and_what_is_in_a_project() {
        let tasks = vec![
            Task { id: "t1".into(), title: "Buy filters".into(), links: vec![Link { kind: "item".into(), id: "5".into() }], ..Default::default() },
            Task { id: "t2".into(), title: "In project".into(), project: "p1".into(), ..Default::default() },
        ];
        let events = vec![Event { id: "e1".into(), title: "Swap".into(), date: "2026-10-30".into(), links: vec![Link { kind: "item".into(), id: "5".into() }], ..Default::default() }];
        let projects = vec![Project { id: "p1".into(), name: "Winter".into(), status: "open".into(), notes: String::new(), chats: vec!["2026-10-01 plan.txt".into()] }];
        let r = related("item", "5", &tasks, &events, &projects);
        assert_eq!(r.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), vec!["Swap", "Buy filters"]);
        let p = related("project", "p1", &tasks, &events, &projects);
        assert_eq!(p.len(), 2);
        assert!(p.iter().any(|x| x.kind == "chat"));
        assert!(related("item", "99", &tasks, &events, &projects).is_empty());
    }

    #[test]
    fn projects_round_trip_and_a_chat_belongs_to_one_project() {
        let mut v = vec![
            Project { id: "p1".into(), name: "Winter prep".into(), status: "open".into(), notes: "a\nb".into(), chats: vec![] },
            Project { id: "p2".into(), name: "Garden".into(), status: "done".into(), notes: String::new(), chats: vec![] },
        ];
        assert!(file_chat(&mut v, "p1", "chat one.txt"));
        assert!(file_chat(&mut v, "p2", "chat one.txt"));
        assert!(v[0].chats.is_empty() && v[1].chats == vec!["chat one.txt".to_string()]);
        assert_eq!(project_of_chat(&v, "chat one.txt").unwrap().id, "p2");
        assert!(!file_chat(&mut v, "p1", "../evil.txt"), "a chat name cannot climb folders");
        assert!(!file_chat(&mut v, "nope", "x.txt"));
        assert!(file_chat(&mut v, "", "chat one.txt"), "an empty id takes it out");
        assert!(project_of_chat(&v, "chat one.txt").is_none());
        let back = parse_projects(&serialize_projects(&v));
        assert_eq!(back, v);
    }

    #[test]
    fn search_needs_every_word_and_prefers_titles() {
        let e = vec![
            Entry { kind: "guide".into(), id: "a".into(), title: "Potatoes".into(), text: "Plant seed potatoes in spring and hill the stems.".into() },
            Entry { kind: "card".into(), id: "b".into(), title: "Water storage".into(), text: "Store potatoes? no. One gallon a day per person.".into() },
            Entry { kind: "plan".into(), id: "c".into(), title: "Heat".into(), text: "Nothing here".into() },
        ];
        let h = search(&e, "potatoes", 10);
        assert_eq!(h[0].id, "a");
        assert_eq!(h.len(), 2);
        assert_eq!(search(&e, "potatoes gallon", 10).len(), 1);
        assert!(search(&e, "", 10).is_empty());
        assert!(search(&e, "zebra", 10).is_empty());
        assert_eq!(search(&e, "potatoes", 1).len(), 1);
    }

    #[test]
    fn snippets_cope_with_unicode() {
        let e = vec![Entry { kind: "guide".into(), id: "u".into(), title: "Crème".into(), text: "ééééééééééééééééééééééééééééééééééééééééééééééééé needle ééé".into() }];
        let h = search(&e, "needle", 5);
        assert!(h[0].snippet.contains("needle"));
    }

    #[test]
    fn the_change_log_keeps_the_newest_lines() {
        let mut log = String::new();
        for i in 0..10u64 {
            log.push_str(&change_line(1000 + i, "task", &format!("t{}", i), "added"));
            log.push('\n');
        }
        let kept = trim_log(&log, 3);
        assert_eq!(kept.lines().count(), 3);
        let r = recent_changes(&kept, 2);
        assert_eq!(r[0].2, "t9");
        assert_eq!(r[1].2, "t8");
        assert!(change_line(1, "a\tb", "i\nd", "x").lines().count() == 1);
    }
}
