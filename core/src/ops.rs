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

/// The note on an event the program made itself, so it can tell those from the ones the person typed.
pub const AUTO_NOTE: &str = "Added from the Family page.";

/// A birthday as a start date for a yearly event: `YYYY-MM-DD` as typed, or `MM-DD` (then the year is 2000).
fn birthday_start(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() { return None; }
    if calendar::parse_date(s).is_some() { return Some(s.to_string()); }
    let (m, d) = crate::seasons::month_day(s)?;
    Some(format!("2000-{:02}-{:02}", m, d))
}

/// Keep one yearly "<Name>'s birthday" event for each person who has a birthday on the Family page. Events the person
/// typed are never touched; only those carrying `AUTO_NOTE` are made, changed or removed here. True when anything changed.
pub fn sync_birthdays(events: &mut Vec<Event>, people: &[(String, String)]) -> bool {
    let mut changed = false;
    let wanted: Vec<(String, String)> = people
        .iter()
        .filter_map(|(n, b)| {
            let name = n.split_whitespace().collect::<Vec<_>>().join(" ");
            if name.is_empty() || name.contains([',', '\t']) { return None; }
            Some((name, birthday_start(b)?))
        })
        .collect();
    let before = events.len();
    events.retain(|e| e.notes != AUTO_NOTE || e.links.iter().any(|l| l.kind == "person" && wanted.iter().any(|(n, _)| *n == l.id)));
    if events.len() != before { changed = true; }
    for (name, start) in &wanted {
        let title = format!("{}'s birthday", name);
        if let Some(e) = events.iter_mut().find(|e| e.notes == AUTO_NOTE && e.links.iter().any(|l| l.kind == "person" && l.id == *name)) {
            if e.date != *start || e.title != title || e.repeat != "yearly" {
                e.date = start.clone(); e.title = title; e.repeat = "yearly".into();
                changed = true;
            }
        } else {
            let id = next_id('e', events.iter().map(|e| e.id.clone()));
            events.push(Event { id, title, date: start.clone(), time: String::new(), repeat: "yearly".into(), project: String::new(), notes: AUTO_NOTE.into(), links: vec![Link { kind: "person".into(), id: name.clone() }] });
            changed = true;
        }
    }
    changed
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
    /// Standing instructions for the assistant when a chat is about this project.
    pub instructions: String,
    /// Up to `MAX_DOCS` documents and pictures: linked where they are, or copied in.
    pub docs: Vec<ProjectDoc>,
}

/// The most documents and pictures one project holds.
pub const MAX_DOCS: usize = 10;

/// One document or picture of a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectDoc {
    /// What it is called in the list.
    pub name: String,
    /// "link": `target` is a path to the file where it already is. "upload": `target` is the file's
    /// name inside this project's own folder (`user/projects/<id>/`).
    pub kind: String,
    pub target: String,
}

/// File types a project may hold or link to: pictures (including phone photos and scans), PDFs,
/// text and office documents. Nothing that can run.
const DOC_TYPES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp", "heic", "pdf", "txt", "md", "csv", "doc", "docx", "xls", "xlsx", "odt", "ods", "rtf"];
const IMAGE_TYPES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

fn ext_lower(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}
pub fn doc_type_ok(name: &str) -> bool {
    DOC_TYPES.contains(&ext_lower(name).as_str())
}
/// A picture the screen can show itself.
pub fn doc_is_image(name: &str) -> bool {
    IMAGE_TYPES.contains(&ext_lower(name).as_str())
}

/// A plain file name for something copied into a project: no folders, nothing odd, a known type.
/// Spaces are kept; anything that cannot be in a Windows file name is replaced.
pub fn safe_upload_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("").trim();
    let cleaned: String = base.chars().map(|c| if c.is_control() || "<>:\"|?*".contains(c) { '_' } else { c }).collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_string();
    if cleaned.is_empty() || cleaned.chars().count() > 90 || !doc_type_ok(&cleaned) {
        return None;
    }
    Some(cleaned)
}

/// `name` if no stored file has it yet; otherwise `name (2)`, `name (3)`... before the extension.
pub fn unique_upload_name(existing: &[String], name: &str) -> String {
    if !existing.iter().any(|e| e.eq_ignore_ascii_case(name)) {
        return name.to_string();
    }
    let (stem, ext) = name.rsplit_once('.').map(|(a, b)| (a.to_string(), format!(".{}", b))).unwrap_or((name.to_string(), String::new()));
    (2..).map(|n| format!("{} ({}){}", stem, n, ext)).find(|c| !existing.iter().any(|e| e.eq_ignore_ascii_case(c))).unwrap_or_else(|| name.to_string())
}

/// Add a document to a project. A link is a path on this computer or the drive (never a web
/// address: the program does not go online). Refused when the project already holds ten.
pub fn add_doc(p: &mut Project, kind: &str, name: &str, target: &str) -> Result<(), String> {
    if p.docs.len() >= MAX_DOCS {
        return Err(format!("A project holds up to {} documents. Take one out first.", MAX_DOCS));
    }
    let target = target.trim();
    let shown = name.trim();
    match kind {
        "link" => {
            let lower = target.to_lowercase();
            if target.is_empty() || lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("ftp:") || lower.starts_with("file:") || lower.starts_with("\\\\") {
                return Err("Type the path of a file on this computer or the drive, such as D:\\Scans\\deed.pdf. Web addresses are not used because the program stays offline.".into());
            }
            if target.contains(['\n', '\t', '|']) || !doc_type_ok(target) {
                return Err("That kind of file cannot be added. Pictures, PDFs, text and office documents can.".into());
            }
            let label = if shown.is_empty() { target.rsplit(['/', '\\']).next().unwrap_or(target).to_string() } else { shown.to_string() };
            p.docs.push(ProjectDoc { name: clean_doc_text(&label), kind: "link".into(), target: target.to_string() });
        }
        "upload" => {
            let file = safe_upload_name(target).ok_or("That kind of file cannot be added. Pictures, PDFs, text and office documents can.")?;
            if file != target {
                return Err("That is not a plain file name.".into());
            }
            let label = if shown.is_empty() { file.clone() } else { shown.to_string() };
            p.docs.push(ProjectDoc { name: clean_doc_text(&label), kind: "upload".into(), target: file });
        }
        _ => return Err("Choose a link or an upload.".into()),
    }
    Ok(())
}

fn clean_doc_text(s: &str) -> String {
    s.chars().map(|c| if c.is_control() || c == '|' { ' ' } else { c }).collect::<String>().trim().chars().take(90).collect()
}

/// The project's standing instructions as a passage for the assistant (nothing when there are none).
/// It ranks first. The assistant's safety rules stay in force whatever the instructions say.
pub fn instructions_passage(p: &Project) -> Option<crate::retrieval::Passage> {
    let text = p.instructions.trim();
    if text.is_empty() {
        return None;
    }
    let docs: Vec<&str> = p.docs.iter().map(|d| d.name.as_str()).collect();
    let mut t = format!("Instructions from the person for the project \"{}\": {}", p.name, text);
    if !docs.is_empty() {
        t.push_str(&format!("\nDocuments kept with this project: {}.", docs.join(", ")));
    }
    Some(crate::retrieval::Passage { source: "Project".into(), heading: format!("Project instructions: {}", p.name), text: t, score: 100.0, link: String::new() })
}

/// What the project remembers, as passages for the assistant: a summary (notes, open tasks, coming dates, the
/// chats filed under it) that always comes first, and the most relevant words from those earlier chats.
/// `chats` is each filed chat's file name and text. The standing instructions come from `instructions_passage`.
/// Plain facts only: nothing here makes the assistant's safety rules weaker.
pub fn memory_passages(p: &Project, tasks: &[Task], events: &[Event], chats: &[(String, String)], question: &str, today: i64) -> Vec<crate::retrieval::Passage> {
    use crate::retrieval::{search_text, Passage};
    let mut out = Vec::new();
    let mut s = format!("What this project (\"{}\", {}) holds.", p.name, if p.status == "done" { "finished" } else { "open" });
    let notes = p.notes.trim();
    if !notes.is_empty() {
        s.push_str(&format!("\nNotes: {}", notes.chars().take(1200).collect::<String>()));
    }
    let open: Vec<&Task> = tasks.iter().filter(|t| t.project == p.id && t.done.is_empty()).collect();
    if !open.is_empty() {
        let list: Vec<String> = open.iter().take(15).map(|t| if t.due.is_empty() { t.title.clone() } else { format!("{} (due {})", t.title, t.due) }).collect();
        s.push_str(&format!("\nOpen tasks: {}.", list.join("; ")));
    }
    let mut dates: Vec<(i64, String)> = Vec::new();
    for e in events.iter().filter(|e| e.project == p.id) {
        for d in occurrences(e, today, today + 90) {
            dates.push((d, format!("{} {}{}", ymd(d), if e.time.is_empty() { String::new() } else { format!("{} ", e.time) }, e.title)));
        }
    }
    dates.sort();
    if !dates.is_empty() {
        let list: Vec<String> = dates.into_iter().take(10).map(|(_, t)| t).collect();
        s.push_str(&format!("\nComing dates: {}.", list.join("; ")));
    }
    if !p.docs.is_empty() {
        let names: Vec<&str> = p.docs.iter().map(|d| d.name.as_str()).collect();
        s.push_str(&format!("\nDocuments kept here: {}.", names.join(", ")));
    }
    if !chats.is_empty() {
        s.push_str(&format!("\nEarlier chats about this project: {}.", chats.len()));
    }
    out.push(Passage { source: "Project".into(), heading: format!("Project memory: {}", p.name), text: s, score: 95.0, link: String::new() });
    // the best-matching words from the earlier chats, newest first when they tie
    let mut found: Vec<Passage> = Vec::new();
    for (name, text) in chats {
        let nice = name.trim_end_matches(".txt");
        for mut x in search_text(&format!("Project chat {}", nice), text, question, 1) {
            x.score += 50.0;
            found.push(x);
        }
    }
    found.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(2);
    out.extend(found);
    out
}

/// Take a document out of the list. The caller deletes the stored copy of an upload.
pub fn remove_doc(p: &mut Project, index: usize) -> Option<ProjectDoc> {
    if index < p.docs.len() { Some(p.docs.remove(index)) } else { None }
}

// A document line is "kind~name~target" with % escapes for the separators, joined by "|".
fn doc_enc(s: &str) -> String {
    s.replace('%', "%25").replace('|', "%7C").replace('~', "%7E")
}
fn doc_dec(s: &str) -> String {
    s.replace("%7E", "~").replace("%7C", "|").replace("%25", "%")
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
            let docs = get(6)
                .split('|')
                .filter_map(|d| {
                    let x: Vec<&str> = d.split('~').collect();
                    if x.len() == 3 && (x[0] == "link" || x[0] == "upload") && !x[2].is_empty() { Some(ProjectDoc { kind: x[0].to_string(), name: doc_dec(x[1]), target: doc_dec(x[2]) }) } else { None }
                })
                .take(MAX_DOCS)
                .collect();
            Some(Project { id: f[0].to_string(), name: unesc(f[1]), status: if get(2) == "done" { "done".into() } else { "open".into() }, notes: unesc(get(3)), chats, instructions: unesc(get(5)), docs })
        })
        .collect()
}

pub fn serialize_projects(v: &[Project]) -> String {
    let mut o = String::from("# id\tname\tstatus\tnotes\tchats\tinstructions\tdocs\n");
    for p in v {
        let docs: Vec<String> = p.docs.iter().map(|d| format!("{}~{}~{}", d.kind, doc_enc(&d.name), doc_enc(&d.target))).collect();
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", p.id, esc(&p.name), p.status, esc(&p.notes), p.chats.join("|"), esc(&p.instructions), docs.join("|")));
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

/// Words too common to show that two things are about the same subject.
const COMMON: &[&str] = &[
    "about", "after", "again", "also", "back", "before", "buy", "call", "check", "clean", "come", "could", "does", "done", "each", "every", "find", "from",
    "gather", "get", "give", "have", "home", "house", "into", "keep", "know", "look", "make", "more", "need", "next", "only", "other", "over", "plan",
    "please", "prepare", "put", "ready", "remember", "review", "set", "should", "some", "start", "take", "task", "test", "that", "their", "then", "there",
    "these", "thing", "things", "this", "time", "update", "using", "want", "week", "what", "when", "where", "which", "will", "with", "work", "would", "year", "your",
];

/// A word reduced so that "batteries"/"battery" and "radios"/"radio" compare equal.
fn stem(w: &str) -> String {
    let w = w.to_lowercase();
    if let Some(x) = w.strip_suffix("ies") {
        if x.len() >= 3 { return format!("{}y", x); }
    }
    if let Some(x) = w.strip_suffix("es") {
        if (x.len() >= 3 && (x.ends_with('s') || x.ends_with('x') || x.ends_with("ch") || x.ends_with("sh"))) || (x.len() >= 4 && x.ends_with('o')) { return x.to_string(); }
    }
    if let Some(x) = w.strip_suffix('s') {
        if x.len() >= 3 && !x.ends_with('s') { return x.to_string(); }
    }
    w
}

fn subject_words(s: &str) -> Vec<String> {
    let mut v: Vec<String> = words(s).into_iter().filter(|w| w.chars().count() >= 4 && !COMMON.contains(&w.as_str())).map(|w| stem(&w)).collect();
    v.sort();
    v.dedup();
    v
}

/// Things the words of `text` are probably about, to offer as links. Nothing is linked by this:
/// the person decides. An entry is offered when its title shares two subject words with the text,
/// or shares its only subject word (a title like "Generator"). Entries in `skip` (what is
/// already linked, and the thing itself) are left out. The best come first.
pub fn suggest_links(text: &str, entries: &[Entry], skip: &[(String, String)], limit: usize) -> Vec<Hit> {
    let mine = subject_words(text);
    if mine.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<Hit> = Vec::new();
    for e in entries {
        if skip.iter().any(|(k, i)| *k == e.kind && *i == e.id) {
            continue;
        }
        let theirs = subject_words(&e.title);
        if theirs.is_empty() {
            continue;
        }
        let shared = theirs.iter().filter(|w| mine.contains(w)).count();
        if shared == 0 || (shared == 1 && theirs.len() > 1) {
            continue;
        }
        // more shared words, and more of the title covered, rank higher
        let score = (shared * 10 + shared * 10 / theirs.len()) as u32;
        hits.push(Hit { kind: e.kind.clone(), id: e.id.clone(), title: e.title.clone(), snippet: String::new(), score });
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
    fn birthdays_become_yearly_events_and_the_persons_own_events_are_left_alone() {
        let mine = Event { id: "e1".into(), title: "Sam's birthday party".into(), date: "2026-06-01".into(), notes: "my own".into(), ..Default::default() };
        let mut ev = vec![mine.clone()];
        let people = vec![("Sam".to_string(), "03-14".to_string()), ("Ana Lee".to_string(), "2015-11-02".to_string()), ("Nobody".to_string(), "".to_string()), ("Bad".to_string(), "31-31".to_string())];
        assert!(sync_birthdays(&mut ev, &people));
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0], mine);
        let sam = ev.iter().find(|e| e.title == "Sam's birthday").unwrap();
        assert_eq!((sam.date.as_str(), sam.repeat.as_str()), ("2000-03-14", "yearly"));
        let ana = ev.iter().find(|e| e.title == "Ana Lee's birthday").unwrap();
        assert_eq!(ana.date, "2015-11-02");
        // it shows up on the right day in later years
        assert_eq!(occurrences(sam, day("2027-01-01"), day("2027-12-31")), vec![day("2027-03-14")]);
        // nothing changed: nothing to write
        assert!(!sync_birthdays(&mut ev, &people));
        // a changed date moves the same event, a removed person takes theirs away
        let people2 = vec![("Sam".to_string(), "03-15".to_string())];
        assert!(sync_birthdays(&mut ev, &people2));
        assert_eq!(ev.len(), 2);
        assert_eq!(ev.iter().find(|e| e.title == "Sam's birthday").unwrap().date, "2000-03-15");
        assert!(ev.iter().any(|e| e.id == "e1"));
    }

    #[test]
    fn project_memory_has_a_summary_and_old_chat_words() {
        let p = Project { id: "p1".into(), name: "Winter prep".into(), status: "open".into(), notes: "Cabin goal: 3 months of food".into(), instructions: "Use metric.".into(), ..Default::default() };
        let tasks = vec![
            Task { id: "t1".into(), title: "Buy rice".into(), due: "2026-11-05".into(), project: "p1".into(), ..Default::default() },
            Task { id: "t2".into(), title: "Already done".into(), done: "2026-10-01".into(), project: "p1".into(), ..Default::default() },
            Task { id: "t3".into(), title: "Other project".into(), project: "p2".into(), ..Default::default() },
        ];
        let events = vec![Event { id: "e1".into(), title: "Stove check".into(), date: "2026-11-10".into(), project: "p1".into(), ..Default::default() }];
        let chats = vec![("2026-10-20-0900-wood.txt".to_string(), "YOU: how much firewood does a cord of oak give\nCINDRIX: A cord of oak firewood heats a small cabin for about two months.".to_string())];
        let m = memory_passages(&p, &tasks, &events, &chats, "how much firewood do we need", day("2026-10-25"));
        assert_eq!(m[0].heading, "Project memory: Winter prep");
        assert!(m[0].text.contains("Cabin goal"));
        assert!(m[0].text.contains("Buy rice (due 2026-11-05)"));
        assert!(!m[0].text.contains("Already done") && !m[0].text.contains("Other project"));
        assert!(m[0].text.contains("2026-11-10 Stove check"));
        assert!(m[0].text.contains("Earlier chats about this project: 1"));
        assert!(m.iter().any(|x| x.heading.is_empty() || x.source.starts_with("Project chat")), "the old chat should be searched");
        assert!(m.iter().skip(1).all(|x| x.score > 50.0));
    }

    #[test]
    fn project_memory_without_anything_is_still_a_short_summary() {
        let p = Project { id: "p1".into(), name: "Garden".into(), status: "done".into(), ..Default::default() };
        let m = memory_passages(&p, &[], &[], &[], "anything", day("2026-10-25"));
        assert_eq!(m.len(), 1);
        assert!(m[0].text.contains("finished"));
    }

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
        let projects = vec![Project { id: "p1".into(), name: "Winter".into(), status: "open".into(), notes: String::new(), chats: vec!["2026-10-01 plan.txt".into()], ..Default::default() }];
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
            Project { id: "p1".into(), name: "Winter prep".into(), status: "open".into(), notes: "a\nb".into(), chats: vec![], instructions: "Answer in short steps.\nAsk before assuming.".into(), docs: vec![ProjectDoc { kind: "link".into(), name: "Deed | scan ~1".into(), target: "D:\\Scans\\deed ~1.pdf".into() }, ProjectDoc { kind: "upload".into(), name: "Photo".into(), target: "roof.jpg".into() }] },
            Project { id: "p2".into(), name: "Garden".into(), status: "done".into(), notes: String::new(), chats: vec![], ..Default::default() },
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

    #[test]
    fn project_documents_are_limited_checked_and_saved() {
        let mut p = Project { id: "p1".into(), name: "Roof".into(), status: "open".into(), ..Default::default() };
        add_doc(&mut p, "link", "", "D:\\Scans\\deed.pdf").unwrap();
        assert_eq!(p.docs[0].name, "deed.pdf");
        add_doc(&mut p, "link", "Quote", "/home/me/quote.docx").unwrap();
        add_doc(&mut p, "upload", "Roof photo", "roof.jpg").unwrap();
        // not: web addresses, programs, folders in an upload name, odd link characters, an unknown kind
        assert!(add_doc(&mut p, "link", "x", "https://example.com/a.pdf").is_err());
        assert!(add_doc(&mut p, "link", "x", "file:///c:/a.pdf").is_err());
        assert!(add_doc(&mut p, "link", "x", "\\\\server\\share\\a.pdf").is_err());
        assert!(add_doc(&mut p, "link", "x", "C:\\tools\\run.exe").is_err());
        assert!(add_doc(&mut p, "link", "x", "C:\\a|b.pdf").is_err());
        assert!(add_doc(&mut p, "upload", "x", "../roof.jpg").is_err());
        assert!(add_doc(&mut p, "upload", "x", "setup.bat").is_err());
        assert!(add_doc(&mut p, "other", "x", "a.pdf").is_err());
        // ten at most
        for i in 0..7 {
            add_doc(&mut p, "upload", "", &format!("scan{}.png", i)).unwrap();
        }
        assert_eq!(p.docs.len(), MAX_DOCS);
        assert!(add_doc(&mut p, "upload", "", "one-too-many.png").unwrap_err().contains("up to 10"));
        // saved and read back, awkward characters and all
        p.instructions = "Short answers.\nAsk first.".into();
        p.docs[0].name = "Deed | scan ~1 100%".into();
        let back = parse_projects(&serialize_projects(&[p.clone()]));
        assert_eq!(back, vec![p.clone()]);
        let gone = remove_doc(&mut p, 1).unwrap();
        assert_eq!(gone.name, "Quote");
        assert_eq!(p.docs.len(), 9);
        assert!(remove_doc(&mut p, 50).is_none());
        // an older file with five columns still reads
        let old = parse_projects("p9\tOld\topen\tn\ta.txt\n");
        assert_eq!((old[0].instructions.as_str(), old[0].docs.len(), old[0].chats.len()), ("", 0, 1));
    }

    #[test]
    fn project_instructions_become_a_passage_only_when_there_are_some() {
        let mut p = Project { id: "p1".into(), name: "Cabin move".into(), ..Default::default() };
        assert!(instructions_passage(&p).is_none());
        p.instructions = "  We are 4 adults. Use metric.  ".into();
        add_doc(&mut p, "link", "Deed", "D:\\Scans\\deed.pdf").unwrap();
        let ps = instructions_passage(&p).unwrap();
        assert!(ps.text.contains("We are 4 adults. Use metric."));
        assert!(ps.text.contains("Deed"));
        assert!(ps.heading.contains("Cabin move"));
    }

    #[test]
    fn upload_names_are_plain_and_never_clash() {
        assert_eq!(safe_upload_name("C:\\Users\\me\\Roof Photo.JPG").as_deref(), Some("Roof Photo.JPG"));
        assert_eq!(safe_upload_name("../../etc/passwd.txt").as_deref(), Some("passwd.txt"));
        assert_eq!(safe_upload_name("a<b>c?.pdf").as_deref(), Some("a_b_c_.pdf"));
        assert_eq!(safe_upload_name(".hidden.pdf").as_deref(), Some("hidden.pdf"));
        assert!(safe_upload_name("virus.exe").is_none());
        assert!(safe_upload_name("noextension").is_none());
        assert!(safe_upload_name("").is_none());
        let have = vec!["scan.jpg".to_string(), "Scan (2).jpg".to_string()];
        assert_eq!(unique_upload_name(&have, "other.jpg"), "other.jpg");
        assert_eq!(unique_upload_name(&have, "SCAN.jpg"), "SCAN (3).jpg");
        assert!(doc_is_image("a.PNG") && !doc_is_image("a.pdf") && doc_type_ok("a.heic"));
    }

    #[test]
    fn links_are_suggested_from_shared_subject_words_and_never_made() {
        let e = |kind: &str, id: &str, title: &str| Entry { kind: kind.into(), id: id.into(), title: title.into(), text: String::new() };
        let entries = vec![
            e("plan", "water-storage", "Water storage"),
            e("card", "boil-water", "Boil water"),
            e("item", "12", "Stored water barrels"),
            e("plan", "smoke", "Check smoke alarms"),
            e("guide", "crop-guides/tomatoes", "Tomatoes"),
            e("plan", "generator", "Generator"),
            e("task", "t1", "Fill water storage"),
        ];
        let skip = vec![("task".to_string(), "t1".to_string())];
        let h = suggest_links("Fill the water storage containers before winter", &entries, &skip, 5);
        let ids: Vec<&str> = h.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids[0], "water-storage", "{:?}", h); // two shared subject words
        assert!(!ids.contains(&"smoke") && !ids.contains(&"t1"), "{:?}", h); // "check" is too common; the thing itself is skipped
        assert!(!ids.contains(&"boil-water"), "{:?}", h); // one shared word of two is not enough
        // a single-word title that matches is offered, plurals count as the same word
        let h2 = suggest_links("Service the generators and buy tomato seeds", &entries, &[], 5);
        let ids2: Vec<&str> = h2.iter().map(|x| x.id.as_str()).collect();
        assert!(ids2.contains(&"generator") && ids2.contains(&"crop-guides/tomatoes"), "{:?}", h2);
        // already linked things are not offered again; the limit is kept; nothing common gives nothing
        let h3 = suggest_links("Fill the water storage", &entries, &[("plan".into(), "water-storage".into())], 5);
        assert!(!h3.iter().any(|x| x.id == "water-storage"));
        assert!(suggest_links("Check the plan", &entries, &[], 5).is_empty());
        assert_eq!(suggest_links("water storage generator tomatoes", &entries, &[], 1).len(), 1);
        assert_eq!(stem("batteries"), "battery");
        assert_eq!(stem("radios"), "radio");
        assert_eq!(stem("boxes"), "box");
        assert_eq!(stem("glass"), "glass");
        assert_eq!(stem("tomatoes"), "tomato");
        assert_eq!(stem("shoes"), "shoe");
    }
}
