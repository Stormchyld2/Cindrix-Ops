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
    #[serde(default)]
    instructions: String,
    /// Read-only here: documents are added and removed with their own commands.
    #[serde(default, skip_deserializing)]
    docs: Vec<DocDto>,
}

#[derive(Serialize, Clone)]
pub struct DocDto {
    name: String,
    kind: String,
    target: String,
    image: bool,
    /// A linked file that is no longer where it was.
    missing: bool,
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
    let u = crate::user();
    let docs = p
        .docs
        .iter()
        .map(|d| DocDto {
            name: d.name.clone(),
            kind: d.kind.clone(),
            target: d.target.clone(),
            image: core_ops::doc_is_image(&d.target),
            missing: if d.kind == "upload" { u.upload_path(&p.id, &d.target).is_none() } else { !std::path::Path::new(&d.target).is_file() },
        })
        .collect();
    ProjectDto { id: p.id.clone(), name: p.name.clone(), status: p.status.clone(), notes: p.notes.clone(), chats: p.chats.clone(), instructions: p.instructions.clone(), docs }
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
    let mut p = Project { id: project.id, name: project.name.trim().to_string(), status: if project.status == "done" { "done".into() } else { "open".into() }, notes: project.notes, chats: project.chats, instructions: project.instructions.chars().take(4000).collect(), docs: Vec::new() };
    let action;
    if p.id.is_empty() {
        p.id = core_ops::next_id('p', all.iter().map(|x| x.id.clone()));
        action = "added";
        all.push(p.clone());
    } else if let Some(slot) = all.iter_mut().find(|x| x.id == p.id) {
        // the chat list is kept by file_chat, not overwritten by a form that never showed it
        p.chats = slot.chats.clone();
        p.docs = slot.docs.clone();
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

/// Link a document where it already is (a path on this computer or the drive).
#[tauri::command]
pub fn project_doc_link(project: String, name: String, path: String) -> Result<Vec<ProjectDto>, String> {
    let u = crate::user();
    let mut all = u.load_projects()?;
    let p = all.iter_mut().find(|p| p.id == project).ok_or("That project is not known.")?;
    let path = path.trim().trim_matches('"').to_string();
    core_ops::add_doc(p, "link", &name, &path)?;
    if !std::path::Path::new(&path).is_file() {
        return Err("There is no file at that path. Check the spelling, or use Add a file instead.".into());
    }
    u.save_projects(&all)?;
    u.note_change(now_secs(), "project", &project, "document linked");
    Ok(all.iter().map(project_out).collect())
}

/// Copy a document or picture into the project (the screen sends the file as base64 text).
#[tauri::command]
pub fn project_doc_upload(project: String, name: String, data: String) -> Result<Vec<ProjectDto>, String> {
    let u = crate::user();
    let mut all = u.load_projects()?;
    let p = all.iter_mut().find(|p| p.id == project).ok_or("That project is not known.")?;
    if p.docs.len() >= core_ops::MAX_DOCS {
        return Err(format!("A project holds up to {} documents. Take one out first.", core_ops::MAX_DOCS));
    }
    let bytes = cindrix_core::coursepack::unbase64(&data).ok_or("That file could not be read.")?;
    let stored = u.save_upload(&project, &name, &bytes)?;
    if let Err(e) = core_ops::add_doc(p, "upload", &name, &stored) {
        u.delete_upload(&project, &stored);
        return Err(e);
    }
    if let Err(e) = u.save_projects(&all) {
        u.delete_upload(&project, &stored);
        return Err(e);
    }
    u.note_change(now_secs(), "project", &project, "document added");
    Ok(all.iter().map(project_out).collect())
}

/// Take a document out of a project. A copied file is deleted from the project folder; a linked one is left alone.
#[tauri::command]
pub fn project_doc_remove(project: String, index: usize) -> Result<Vec<ProjectDto>, String> {
    let u = crate::user();
    let mut all = u.load_projects()?;
    let p = all.iter_mut().find(|p| p.id == project).ok_or("That project is not known.")?;
    let doc = core_ops::remove_doc(p, index).ok_or("That document is not in the list.")?;
    u.save_projects(&all)?;
    if doc.kind == "upload" {
        u.delete_upload(&project, &doc.target);
    }
    u.note_change(now_secs(), "project", &project, "document removed");
    Ok(all.iter().map(project_out).collect())
}

fn doc_file(project: &str, index: usize) -> Result<std::path::PathBuf, String> {
    let u = crate::user();
    let all = u.load_projects()?;
    let p = all.iter().find(|p| p.id == project).ok_or("That project is not known.")?;
    let d = p.docs.get(index).ok_or("That document is not in the list.")?;
    let path = if d.kind == "upload" { u.upload_path(project, &d.target) } else { Some(std::path::PathBuf::from(&d.target)).filter(|p| p.is_file()) };
    path.ok_or_else(|| "That file is not there any more. It may have been moved; link it again.".to_string())
}

/// Open a document with the program the computer uses for that kind of file.
#[tauri::command]
pub fn project_doc_open(project: String, index: usize) -> Result<(), String> {
    let path = doc_file(&project, index)?;
    if !core_ops::doc_type_ok(&path.to_string_lossy()) {
        return Err("That kind of file is not opened from here.".into());
    }
    #[cfg(windows)]
    let started = std::process::Command::new("explorer").arg(&path).spawn();
    #[cfg(target_os = "macos")]
    let started = std::process::Command::new("open").arg(&path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let started = std::process::Command::new("xdg-open").arg(&path).spawn();
    started.map(|_| ()).map_err(|e| format!("Could not open the file: {}", e))
}

/// A picture document as a `data:` address for a small preview (nothing when it is not a picture or is very large).
#[tauri::command]
pub fn project_doc_image(project: String, index: usize) -> Result<Option<String>, String> {
    let path = doc_file(&project, index)?;
    if !core_ops::doc_is_image(&path.to_string_lossy()) {
        return Ok(None);
    }
    Ok(cindrix_core::academy::image_data_url(&path))
}

/// Things the given text seems to be about, offered as links. Nothing is linked by this call.
#[tauri::command]
pub fn connect_suggest(text: String, kind: String, id: String, linked: Vec<LinkDto>) -> Vec<HitDto> {
    let mut skip: Vec<(String, String)> = linked.into_iter().map(|l| (l.kind, l.id)).collect();
    skip.push((kind, id));
    core_ops::suggest_links(&text, &all_entries(), &skip, 6).into_iter().map(|h| HitDto { kind: h.kind, id: h.id, title: h.title, snippet: h.snippet }).collect()
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

/// Dates the program works out by itself: planting times from the seed vault and the person's usual frost dates, and the frost days.
/// Only days inside the range are made, so none of them ever shows as late.
fn seasonal_dates(from: i64, to: i64) -> Vec<Dated> {
    let user_dir = crate::root().join("user");
    let Ok(prof) = cindrix_core::profile::load(&user_dir) else { return Vec::new() };
    let seeds = cindrix_core::seeds::Vault::new(&user_dir).list().unwrap_or_default();
    cindrix_core::seasons::seasonal_dates(&seeds, &prof.last_frost, &prof.first_frost, from, to.min(from + 400))
}

#[derive(Deserialize)]
pub struct BirthdayDto {
    name: String,
    birthday: String,
}

/// The Family page's birthdays, kept on the calendar as yearly events. Only events this makes itself are touched.
#[tauri::command]
pub fn birthdays_sync(people: Vec<BirthdayDto>) -> Result<(), String> {
    let u = crate::user();
    let mut all = u.load_events()?;
    let list: Vec<(String, String)> = people.into_iter().map(|p| (p.name, p.birthday)).collect();
    if core_ops::sync_birthdays(&mut all, &list) {
        u.save_events(&all)?;
    }
    Ok(())
}

/// One list in date order: tasks, events, things that expire and plan checks, from `from` to `to` (YYYY-MM-DD; empty means today and 60 days on).
#[tauri::command]
pub fn agenda_get(from: String, to: String) -> Result<Vec<AgendaDto>, String> {
    let u = crate::user();
    let today = calendar::today_days();
    let f = calendar::parse_date(&from).unwrap_or(today);
    let t = calendar::parse_date(&to).unwrap_or(f + 60);
    let (f, t) = (f, t.max(f));
    let mut extra = extra_dates();
    extra.extend(seasonal_dates(f, t));
    let list = core_ops::agenda(&u.load_tasks()?, &u.load_events()?, &extra, f, t);
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
    // Books and manuals (by title, author and shelf). The Reading Corner is for enjoyment and stays out.
    if let Ok(books) = crate::shelf_list() {
        for b in books.into_iter().filter(|b| !cindrix_core::shelf::is_leisure(&b.category)) {
            v.push(Entry { kind: "book".into(), id: b.id, title: b.title, text: format!("{} {}", b.author, b.category) });
        }
    }
    if let Ok(ts) = u.load_tasks() {
        for t in ts { v.push(Entry { kind: "task".into(), id: t.id, title: t.title, text: t.notes }); }
    }
    if let Ok(es) = u.load_events() {
        for e in es { v.push(Entry { kind: "event".into(), id: e.id, title: e.title, text: format!("{} {}", e.date, e.notes) }); }
    }
    if let Ok(ps) = u.load_projects() {
        for p in ps { v.push(Entry { kind: "project".into(), id: p.id, title: p.name, text: format!("{} {}", p.notes, p.instructions) }); }
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
