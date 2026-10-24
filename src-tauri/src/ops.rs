//! Tauri side of the connecting layer (core/src/ops.rs): the to-do list, the calendar, projects, the agenda,
//! one search over everything, and "what is connected to this". These are the person's own notes, so they are
//! host-only: another device on the hotspot cannot reach them (they are not in the share allowlist).

use cindrix_core::ops::{self as core_ops, Dated, Entry, Event, Link, Project, Task};
use cindrix_core::calendar;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LinkDto {
    kind: String,
    id: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TaskDto {
    id: String,
    title: String,
    due: String,
    done: String,
    project: String,
    every_days: u32,
    notes: String,
    links: Vec<LinkDto>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EventDto {
    id: String,
    title: String,
    date: String,
    time: String,
    repeat: String,
    project: String,
    notes: String,
    links: Vec<LinkDto>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProjectDto {
    id: String,
    name: String,
    status: String,
    notes: String,
    chats: Vec<String>,
}

#[derive(Serialize)]
pub struct AgendaDto {
    date: String,
    kind: String,
    id: String,
    title: String,
    time: String,
    overdue: bool,
    project: String,
}

#[derive(Serialize)]
pub struct HitDto {
    kind: String,
    id: String,
    title: String,
    snippet: String,
}

#[derive(Serialize)]
pub struct RefDto {
    kind: String,
    id: String,
    title: String,
}

#[derive(Serialize)]
pub struct OpsDto {
    tasks: Vec<TaskDto>,
    events: Vec<EventDto>,
    projects: Vec<ProjectDto>,
    today: String,
}

#[derive(Serialize)]
pub struct ChangeDto {
    when: u64,
    kind: String,
    id: String,
    action: String,
}

fn links_in(v: Vec<LinkDto>) -> Vec<Link> {
    v.into_iter().map(|l| Link { kind: l.kind, id: l.id }).collect()
}
fn links_out(v: &[Link]) -> Vec<LinkDto> {
    v.iter().map(|l| LinkDto { kind: l.kind.clone(), id: l.id.clone() }).collect()
}
fn task_out(t: &Task) -> TaskDto {
    TaskDto { id: t.id.clone(), title: t.title.clone(), due: t.due.clone(), done: t.done.clone(), project: t.project.clone(), every_days: t.every_days, notes: t.notes.clone(), links: links_out(&t.links) }
}
fn event_out(e: &Event) -> EventDto {
    EventDto { id: e.id.clone(), title: e.title.clone(), date: e.date.clone(), time: e.time.clone(), repeat: e.repeat.clone(), project: e.project.clone(), notes: e.notes.clone(), links: links_out(&e.links) }
}
fn project_out(p: &Project) -> ProjectDto {
    ProjectDto { id: p.id.clone(), name: p.name.clone(), status: p.status.clone(), notes: p.notes.clone(), chats: p.chats.clone() }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[tauri::command]
pub fn ops_load() -> Result<OpsDto, String> {
    let u = crate::user();
    Ok(OpsDto {
        tasks: u.load_tasks()?.iter().map(task_out).collect(),
        events: u.load_events()?.iter().map(event_out).collect(),
        projects: u.load_projects()?.iter().map(project_out).collect(),
        today: calendar::today_string(),
    })
}

/// Create (empty id) or replace a task.
#[tauri::command]
pub fn task_save(task: TaskDto) -> Result<Vec<TaskDto>, String> {
    let u = crate::user();
    let mut all = u.load_tasks()?;
    if task.title.trim().is_empty() {
        return Err("Give the task a name first.".into());
    }
    if !task.due.is_empty() && calendar::parse_date(&task.due).is_none() {
        return Err("That date is not a real day. Use the calendar to pick one.".into());
    }
    let mut t = Task { id: task.id, title: task.title.trim().to_string(), due: task.due, done: task.done, project: task.project, every_days: task.every_days, notes: task.notes, links: links_in(task.links) };
    let action;
    if t.id.is_empty() {
        t.id = core_ops::next_id('t', all.iter().map(|x| x.id.clone()));
        action = "added";
        all.push(t.clone());
    } else if let Some(slot) = all.iter_mut().find(|x| x.id == t.id) {
        *slot = t.clone();
        action = "changed";
    } else {
        action = "added";
        all.push(t.clone());
    }
    u.save_tasks(&all)?;
    u.note_change(now_secs(), "task", &t.id, action);
    Ok(all.iter().map(task_out).collect())
}

#[tauri::command]
pub fn task_delete(id: String) -> Result<Vec<TaskDto>, String> {
    let u = crate::user();
    let mut all = u.load_tasks()?;
    all.retain(|t| t.id != id);
    u.save_tasks(&all)?;
    u.note_change(now_secs(), "task", &id, "removed");
    Ok(all.iter().map(task_out).collect())
}

/// Tick (or un-tick) a task. A repeating task moves to its next due date instead of closing.
#[tauri::command]
pub fn task_done(id: String, done: bool) -> Result<Vec<TaskDto>, String> {
    let u = crate::user();
    let mut all = u.load_tasks()?;
    let today = calendar::today_days();
    if let Some(slot) = all.iter_mut().find(|t| t.id == id) {
        *slot = if done { core_ops::complete(slot, today) } else { core_ops::reopen(slot) };
    }
    u.save_tasks(&all)?;
    u.note_change(now_secs(), "task", &id, if done { "ticked" } else { "reopened" });
    Ok(all.iter().map(task_out).collect())
}

#[tauri::command]
pub fn event_save(event: EventDto) -> Result<Vec<EventDto>, String> {
    let u = crate::user();
    let mut all = u.load_events()?;
    if event.title.trim().is_empty() {
        return Err("Give the event a name first.".into());
    }
    if calendar::parse_date(&event.date).is_none() {
        return Err("Pick a day for the event.".into());
    }
    let mut e = Event { id: event.id, title: event.title.trim().to_string(), date: event.date, time: event.time, repeat: event.repeat, project: event.project, notes: event.notes, links: links_in(event.links) };
    let action;
    if e.id.is_empty() {
        e.id = core_ops::next_id('e', all.iter().map(|x| x.id.clone()));
        action = "added";
        all.push(e.clone());
    } else if let Some(slot) = all.iter_mut().find(|x| x.id == e.id) {
        *slot = e.clone();
        action = "changed";
    } else {
        action = "added";
        all.push(e.clone());
    }
    u.save_events(&all)?;
    u.note_change(now_secs(), "event", &e.id, action);
    Ok(all.iter().map(event_out).collect())
}

#[tauri::command]
pub fn event_delete(id: String) -> Result<Vec<EventDto>, String> {
    let u = crate::user();
    let mut all = u.load_events()?;
    all.retain(|e| e.id != id);
    u.save_events(&all)?;
    u.note_change(now_secs(), "event", &id, "removed");
    Ok(all.iter().map(event_out).collect())
}

#[tauri::command]
pub fn project_save(project: ProjectDto) -> Result<Vec<ProjectDto>, String> {
    let u = crate::user();
    let mut all = u.load_projects()?;
    if project.name.trim().is_empty() {
        return Err("Give the project a name first.".into());
    }
    let mut p = Project { id: project.id, name: project.name.trim().to_string(), status: if project.status == "done" { "done".into() } else { "open".into() }, notes: project.notes, chats: project.chats };
    let action;
    if p.id.is_empty() {
        p.id = core_ops::next_id('p', all.iter().map(|x| x.id.clone()));
        action = "added";
        all.push(p.clone());
    } else if let Some(slot) = all.iter_mut().find(|x| x.id == p.id) {
        // the chat list is kept by file_chat, not overwritten by a form that never showed it
        p.chats = slot.chats.clone();
        *slot = p.clone();
        action = "changed";
    } else {
        action = "added";
        all.push(p.clone());
    }
    u.save_projects(&all)?;
    u.note_change(now_secs(), "project", &p.id, action);
    Ok(all.iter().map(project_out).collect())
}

/// Delete a project. Its tasks and events stay, with no project; its chats stay as saved chats.
#[tauri::command]
pub fn project_delete(id: String) -> Result<Vec<ProjectDto>, String> {
    let u = crate::user();
    let mut all = u.load_projects()?;
    all.retain(|p| p.id != id);
    let mut tasks = u.load_tasks()?;
    if tasks.iter().any(|t| t.project == id) {
        for t in tasks.iter_mut().filter(|t| t.project == id) { t.project.clear(); }
        u.save_tasks(&tasks)?;
    }
    let mut events = u.load_events()?;
    if events.iter().any(|e| e.project == id) {
        for e in events.iter_mut().filter(|e| e.project == id) { e.project.clear(); }
        u.save_events(&events)?;
    }
    u.save_projects(&all)?;
    u.note_change(now_secs(), "project", &id, "removed");
    Ok(all.iter().map(project_out).collect())
}

/// File a saved Ask chat under a project (empty project id takes it out of every project).
#[tauri::command]
pub fn project_file_chat(project: String, chat: String) -> Result<Vec<ProjectDto>, String> {
    let u = crate::user();
    let mut all = u.load_projects()?;
    if !core_ops::file_chat(&mut all, &project, &chat) {
        return Err("That chat or project is not known.".into());
    }
    u.save_projects(&all)?;
    u.note_change(now_secs(), "project", &project, "chat filed");
    Ok(all.iter().map(project_out).collect())
}

/// Dates other parts of the program know about: things that run out, and plan checks that fall due.
fn extra_dates() -> Vec<Dated> {
    let mut out = Vec::new();
    let today = calendar::today_days();
    if let Ok(items) = crate::user().load_inventory() {
        for it in items.iter().filter(|i| !i.use_by.is_empty()) {
            let Some((y, m)) = it.use_by.split_once('-').and_then(|(y, m)| Some((y.parse::<i32>().ok()?, m.parse::<u32>().ok()?))) else { continue };
            if !(1..=12).contains(&m) { continue; }
            let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
            let day = calendar::days_from_civil(ny, nm, 1) - 1;
            if day < today - 180 { continue; } // long gone: the Inventory screen already shows it
            out.push(Dated { kind: "item".into(), id: it.id.clone(), title: format!("{} runs out or expires", it.name), day });
        }
    }
    if let Ok(plans) = crate::plans_list(None) {
        for p in plans.iter().filter(|p| p.followed) {
            for s in &p.steps {
                if let Some(due_in) = s.due_in {
                    if s.state == "due" || s.state == "done" {
                        out.push(Dated { kind: "plan".into(), id: p.id.clone(), title: format!("{}: {}", p.title, s.title), day: today + due_in });
                    }
                }
            }
        }
    }
    out
}

/// One list in date order: tasks, events, things that expire and plan checks, from `from` to `to` (YYYY-MM-DD; empty means today and 60 days on).
#[tauri::command]
pub fn agenda_get(from: String, to: String) -> Result<Vec<AgendaDto>, String> {
    let u = crate::user();
    let today = calendar::today_days();
    let f = calendar::parse_date(&from).unwrap_or(today);
    let t = calendar::parse_date(&to).unwrap_or(f + 60);
    let list = core_ops::agenda(&u.load_tasks()?, &u.load_events()?, &extra_dates(), f, t.max(f));
    Ok(list.into_iter().map(|a| AgendaDto { date: a.date, kind: a.kind, id: a.id, title: a.title, time: a.time, overdue: a.overdue, project: a.project }).collect())
}

/// Everything that can be searched from one place, with a stable `kind` and `id` for each.
fn all_entries() -> Vec<Entry> {
    let u = crate::user();
    let mut v: Vec<Entry> = Vec::new();
    if let Ok(items) = u.load_inventory() {
        for i in items {
            v.push(Entry { kind: "item".into(), id: i.id, title: i.name, text: format!("{} {} {} {}", i.category, i.location, i.notes, i.brand) });
        }
    }
    if let Ok(plans) = crate::plans_list(None) {
        for p in plans {
            let text = format!("{} {}", p.summary, p.steps.iter().map(|s| s.title.clone()).collect::<Vec<_>>().join(" "));
            v.push(Entry { kind: "plan".into(), id: p.id, title: p.title, text });
        }
    }
    for c in crate::list_cards() {
        v.push(Entry { kind: "card".into(), id: c.id, title: c.title, text: c.body });
    }
    for (section, _) in cindrix_core::guides::SECTIONS {
        if let Ok(pages) = crate::guides_for(section, false) {
            for c in pages {
                v.push(Entry { kind: "guide".into(), id: format!("{}/{}", section, c.id), title: c.title, text: c.body });
            }
        }
    }
    if let Ok(ts) = u.load_tasks() {
        for t in ts { v.push(Entry { kind: "task".into(), id: t.id, title: t.title, text: t.notes }); }
    }
    if let Ok(es) = u.load_events() {
        for e in es { v.push(Entry { kind: "event".into(), id: e.id, title: e.title, text: format!("{} {}", e.date, e.notes) }); }
    }
    if let Ok(ps) = u.load_projects() {
        for p in ps { v.push(Entry { kind: "project".into(), id: p.id, title: p.name, text: p.notes }); }
    }
    v
}

/// One search over the cards, plans, guides, inventory, tasks, events and projects.
#[tauri::command]
pub fn connect_search(query: String) -> Vec<HitDto> {
    core_ops::search(&all_entries(), &query, 40).into_iter().map(|h| HitDto { kind: h.kind, id: h.id, title: h.title, snippet: h.snippet }).collect()
}

/// Names for a list of links, so a screen can show "Water filter" instead of "item:5". Unknown ones keep their id.
#[tauri::command]
pub fn connect_resolve(links: Vec<LinkDto>) -> Vec<RefDto> {
    let entries = all_entries();
    links
        .into_iter()
        .map(|l| {
            let title = entries.iter().find(|e| e.kind == l.kind && e.id == l.id).map(|e| e.title.clone()).unwrap_or_else(|| l.id.clone());
            RefDto { kind: l.kind, id: l.id, title }
        })
        .collect()
}

/// Everything that points at one thing (tasks, events and projects that link to it; a project's own contents).
#[tauri::command]
pub fn connect_related(kind: String, id: String) -> Result<Vec<RefDto>, String> {
    let u = crate::user();
    Ok(core_ops::related(&kind, &id, &u.load_tasks()?, &u.load_events()?, &u.load_projects()?).into_iter().map(|r| RefDto { kind: r.kind, id: r.id, title: r.title }).collect())
}

#[tauri::command]
pub fn changes_recent(n: usize) -> Vec<ChangeDto> {
    crate::user().changes(n.min(100)).into_iter().map(|(when, kind, id, action)| ChangeDto { when, kind, id, action }).collect()
}
