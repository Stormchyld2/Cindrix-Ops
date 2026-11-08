//! Security Operations, part 5: what the screens ask for. Each function takes the user folder and plain
//! arguments and answers with JSON text (or a plain-words error), so the Tauri layer is only a thin wrapper and
//! a second (browser) transport could use the same code. All the rules live in the modules this calls.
//!
//! The clock is the computer's own. The screen sends only its offset from UTC (minutes east), so a person's
//! typed or pasted time can never move "now".

use crate::llama::json_escape;
use crate::security::{self, format_instant, Posture, PostureEntry, Site};
use crate::security_incident as incident;
use crate::security_schedule::{self as sched, Attention, Cover};
use crate::security_store as store;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// The names of the Tauri commands that reach this module. All are host-only: none may ever be in
/// `share::GUEST_COMMANDS`. `core/tests/security_rules.rs` checks that, and that this list matches `src-tauri/src/security.rs`.
pub const SECURITY_COMMANDS: &[&str] = &[
    "security_overview",
    "security_watch_strip",
    "security_site_save",
    "security_posture_set",
    "security_incident_open",
    "security_incident_entry",
    "security_incident_list",
    "security_watch_list",
    "security_watch_draft",
    "security_watch_add",
    "security_watch_move",
    "security_watch_ack",
    "security_watch_discard",
    "security_watch_schedule_all",
    "security_pocket_list",
    "security_pocket_accept",
    "security_pocket_set_aside",
];

/// Now, in minutes since 1970 UTC, from the system clock.
pub fn now_minutes() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64 / 60).unwrap_or(0)
}

fn q(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn offset_ok(offset_minutes: i64) -> Result<i64, String> {
    if (-14 * 60..=14 * 60).contains(&offset_minutes) { Ok(offset_minutes) } else { Err("The time zone could not be read.".to_string()) }
}

/// Everything the Command and Dashboard summaries need, in one read.
pub fn overview(user_root: &Path, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    overview_at(user_root, off, now_minutes())
}

pub fn overview_at(user_root: &Path, off: i64, now: i64) -> Result<String, String> {
    let sites = store::load_sites(user_root)?;
    let log = store::load_posture_log(user_root)?;
    let watches = store::load_watches(user_root)?;
    let incidents = store::load_incident_log(user_root)?;
    let built = incident::build(&incidents.items);
    let ov = security::overview(&sites.items, &log.items, now);
    let mut rows = Vec::new();
    let mut look_total = 0usize;
    for (s, st) in sites.items.iter().zip(ov.sites.iter()) {
        let cur = security::current_entry(&log.items, &s.id);
        let on: Vec<String> = sched::on_watch_now(&watches.items, &s.id, now).iter().map(|w| q(&w.person)).collect();
        let live = incident::live_open(&built).iter().filter(|i| i.site == s.id).count();
        let looks = site_looks(&s.id, st, cur, &watches.items, off, now);
        look_total += looks.len();
        rows.push(format!(
            "{{\"id\":{},\"name\":{},\"tz\":{},\"posture\":{},\"check_due\":{},\"never_assessed\":{},\"set_by\":{},\"reason\":{},\"at\":{},\"review_by\":{},\"on_watch\":[{}],\"live_incidents\":{},\"looks\":[{}]}}",
            q(&s.id), q(&s.name), q(&s.tz), q(st.posture.word()), st.check_due, st.never_assessed,
            q(cur.map(|e| e.set_by.as_str()).unwrap_or("")), q(cur.map(|e| e.reason.as_str()).unwrap_or("")), q(cur.map(|e| e.at.as_str()).unwrap_or("")), q(cur.map(|e| e.review_by.as_str()).unwrap_or("")),
            on.join(","), live, looks.iter().map(|l| q(l)).collect::<Vec<_>>().join(",")
        ));
    }
    let sites_on_watch = sites.items.iter().filter(|s| !sched::on_watch_now(&watches.items, &s.id, now).is_empty()).count();
    let rejected = sites.rejected.len() + log.rejected.len() + watches.rejected.len() + incidents.rejected.len();
    if rejected > 0 {
        look_total += 1;
    }
    let pocket = crate::security_pocket::waiting_count(user_root);
    if pocket > 0 {
        look_total += 1;
    }
    Ok(format!(
        "{{\"now\":{},\"headline\":{},\"unknown\":{},\"check_due\":{},\"all_normal_confirmed\":{},\"live_incidents\":{},\"unreadable_lines\":{},\"pocket_waiting\":{},\"needs_a_look\":{},\"sites_on_watch\":{},\"sites\":[{}]}}",
        q(&format_instant(now, off)), q(ov.headline.word()), ov.unknown, ov.check_due, ov.all_normal_confirmed, incident::live_open(&built).len(), rejected, pocket, look_total, sites_on_watch, rows.join(",")
    ))
}

/// The plain-words "needs a look" lines for one site. One place makes them, so the Dashboard count and the
/// Command list can never disagree.
fn site_looks(site: &str, st: &security::SiteStatus, cur: Option<&PostureEntry>, watches: &[sched::Watch], off: i64, now: i64) -> Vec<String> {
    let mut out = Vec::new();
    if st.never_assessed {
        out.push("No posture has been set for this site yet. It shows Unknown.".to_string());
    } else if st.check_due {
        let who = cur.map(|e| e.set_by.as_str()).filter(|w| !w.is_empty()).unwrap_or("someone");
        out.push(format!("The {} posture is due for a check (set by {}). It has not been lowered.", st.posture.word(), who));
    }
    let mine: Vec<&sched::Watch> = watches.iter().filter(|w| w.site == site).collect();
    for (id, kind) in sched::needs_a_look(watches, now) {
        if let Some(w) = mine.iter().find(|w| w.id == id) {
            out.push(match kind {
                Attention::NotAcknowledged => format!("{} has not acknowledged a watch that has started.", w.person),
                Attention::HandoverOpen => format!("{}'s watch has ended and no handover is written down.", w.person),
            });
        }
    }
    let from = (now + off).div_euclid(1440) * 1440 - off;
    let owned: Vec<sched::Watch> = mine.iter().map(|w| (*w).clone()).collect();
    let segs = sched::coverage(&owned, site, from, from + 1440);
    let has_cover = segs.iter().any(|s| s.cover != Cover::Gap);
    let gap = sched::gap_minutes(&segs);
    if has_cover && gap > 0 {
        out.push(format!("Nobody is covering {} of today.", if gap % 60 == 0 { format!("{} hours", gap / 60) } else { format!("{} h {} min", gap / 60, gap % 60) }));
    }
    out
}

/// Add a site, or change a site's name, time zone, map or notes. The code (id) of an existing site never changes.
pub fn site_save(user_root: &Path, id: &str, name: &str, tz: &str, map_id: &str, notes: &str) -> Result<String, String> {
    let mut sites = store::load_sites(user_root)?;
    if !sites.rejected.is_empty() {
        return Err("Some lines in the site list could not be read. Nothing was changed.".to_string());
    }
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the site a name first.".to_string());
    }
    let id = if id.is_empty() { security::slug(name) } else { id.to_string() };
    if !security::valid_id(&id) {
        return Err("The site needs a name with some letters or numbers in it.".to_string());
    }
    let rec = Site { id: id.clone(), name: name.to_string(), tz: tz.trim().to_string(), map_id: map_id.trim().to_string(), notes: notes.trim().to_string() };
    match sites.items.iter_mut().find(|s| s.id == id) {
        Some(slot) => *slot = rec,
        None => sites.items.push(rec),
    }
    store::save_sites(user_root, &sites.items)?;
    Ok(id)
}

/// A person sets a site's posture. `review_hours` of 0 means no review time. Always recorded as an operator's
/// decision; nothing else can call this.
pub fn posture_set(user_root: &Path, site: &str, posture: &str, by: &str, reason: &str, review_hours: u32, offset_minutes: i64) -> Result<(), String> {
    let off = offset_ok(offset_minutes)?;
    let now = now_minutes();
    let p = match posture.trim() {
        "normal" => Posture::Normal,
        "elevated" => Posture::Elevated,
        "emergency" => Posture::Emergency,
        "unknown" => Posture::Unknown,
        _ => return Err("Pick normal, elevated, emergency or unknown.".to_string()),
    };
    if by.trim().is_empty() {
        return Err("Say who is setting this.".to_string());
    }
    let at = format_instant(now, off);
    let e = PostureEntry {
        site: site.to_string(),
        posture: p,
        set_by: by.trim().to_string(),
        reason: reason.trim().to_string(),
        source: "operator".to_string(),
        at: at.clone(),
        verified_at: at,
        review_by: if review_hours > 0 { format_instant(now + review_hours as i64 * 60, off) } else { String::new() },
    };
    store::append_posture(user_root, &e)
}

/// The watch strip for one site: local midnight today to local midnight tomorrow.
pub fn watch_strip(user_root: &Path, site: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    watch_strip_at(user_root, site, off, now_minutes())
}

pub fn watch_strip_at(user_root: &Path, site: &str, off: i64, now: i64) -> Result<String, String> {
    let all = store::load_watches(user_root)?;
    let from = (now + off).div_euclid(1440) * 1440 - off;
    let to = from + 1440;
    let segs = sched::coverage(&all.items, site, from, to);
    let seg_json: Vec<String> = segs
        .iter()
        .map(|s| format!("{{\"from\":{},\"to\":{},\"cover\":{}}}", q(&format_instant(s.from, off)), q(&format_instant(s.to, off)), q(match s.cover { Cover::Confirmed => "confirmed", Cover::Planned => "planned", Cover::Gap => "gap" })))
        .collect();
    let mine: Vec<&sched::Watch> = all.items.iter().filter(|w| w.site == site).collect();
    let look: Vec<String> = sched::needs_a_look(&all.items, now)
        .into_iter()
        .filter_map(|(id, k)| {
            let w = mine.iter().find(|w| w.id == id)?;
            Some(format!("{{\"id\":{},\"person\":{},\"kind\":{}}}", q(&id), q(&w.person), q(match k { Attention::NotAcknowledged => "not-acknowledged", Attention::HandoverOpen => "handover-open" })))
        })
        .collect();
    let list: Vec<String> = mine
        .iter()
        .filter(|w| w.span().map(|(a, b)| b > from && a < to).unwrap_or(false))
        .map(|w| format!("{{\"id\":{},\"person\":{},\"role\":{},\"start\":{},\"end\":{},\"state\":{}}}", q(&w.id), q(&w.person), q(&w.role), q(&w.start), q(&w.end), q(w.state.word())))
        .collect();
    Ok(format!(
        "{{\"site\":{},\"from\":{},\"to\":{},\"gap_minutes\":{},\"segments\":[{}],\"needs_a_look\":[{}],\"watches\":[{}]}}",
        q(site), q(&format_instant(from, off)), q(&format_instant(to, off)), sched::gap_minutes(&segs), seg_json.join(","), look.join(","), list.join(",")
    ))
}

/// Open an incident. Returns its number.
pub fn incident_open(user_root: &Path, site: &str, title: &str, priority: u8, exercise: bool, by: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let log = store::load_incident_log(user_root)?.items;
    let now = format_instant(now_minutes(), off);
    let inc = incident::next_id('i', log.iter().map(|e| e.incident.clone()));
    let e = incident::Entry {
        id: incident::next_id('e', log.iter().map(|e| e.id.clone())),
        incident: inc.clone(),
        kind: incident::Kind::Open,
        happened_at: now.clone(),
        recorded_at: now,
        by: by.trim().to_string(),
        source: incident::Source::Observed,
        arg1: site.to_string(),
        arg2: priority.to_string(),
        arg3: if exercise { "exercise" } else { "live" }.to_string(),
        text: title.trim().to_string(),
        supersedes: String::new(),
    };
    store::append_incident(user_root, &e)?;
    Ok(inc)
}

/// Add a line to an incident. `happened_at` may be empty for "now"; a late entry from paper or radio passes the
/// real time. `kind` is note, decision, task, status or correction.
#[allow(clippy::too_many_arguments)]
pub fn incident_entry(user_root: &Path, inc: &str, kind: &str, arg1: &str, text: &str, source: &str, by: &str, supersedes: &str, happened_at: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let k = match incident::Kind::from_word(kind) {
        Some(k) if !matches!(k, incident::Kind::Open | incident::Kind::Plan) => k,
        _ => return Err("That kind of entry is not allowed here.".to_string()),
    };
    let source = incident::Source::from_word(source).ok_or("Say where this came from: observed, reported, imported or corrected.")?;
    let log = store::load_incident_log(user_root)?.items;
    let now = format_instant(now_minutes(), off);
    let e = incident::Entry {
        id: incident::next_id('e', log.iter().map(|e| e.id.clone())),
        incident: inc.to_string(),
        kind: k,
        happened_at: if happened_at.trim().is_empty() { now.clone() } else { happened_at.trim().to_string() },
        recorded_at: now,
        by: by.trim().to_string(),
        source,
        arg1: arg1.to_string(),
        arg2: String::new(),
        arg3: String::new(),
        text: text.trim().to_string(),
        supersedes: supersedes.to_string(),
    };
    store::append_incident(user_root, &e)?;
    Ok(e.id)
}

/// Incidents with their timelines, newest first.
pub fn incident_list(user_root: &Path) -> Result<String, String> {
    let log = store::load_incident_log(user_root)?;
    let mut built = incident::build(&log.items);
    built.reverse();
    let rows: Vec<String> = built
        .iter()
        .map(|i| {
            let tl: Vec<String> = incident::timeline(&log.items, &i.id)
                .iter()
                .map(|l| {
                    let e = l.entry;
                    format!(
                        "{{\"id\":{},\"kind\":{},\"happened_at\":{},\"recorded_at\":{},\"by\":{},\"source\":{},\"arg1\":{},\"text\":{},\"supersedes\":{},\"superseded_by\":{}}}",
                        q(&e.id), q(e.kind.word()), q(&e.happened_at), q(&e.recorded_at), q(&e.by), q(e.source.word()), q(&e.arg1), q(&e.text), q(&e.supersedes), q(l.superseded_by.as_deref().unwrap_or(""))
                    )
                })
                .collect();
            let plans: Vec<String> = i.plans.iter().map(|p| format!("{{\"plan\":{},\"revision\":{},\"fingerprint\":{},\"by\":{},\"at\":{}}}", q(&p.plan), q(&p.revision), q(&p.fingerprint), q(&p.activated_by), q(&p.at))).collect();
            format!(
                "{{\"id\":{},\"site\":{},\"title\":{},\"priority\":{},\"exercise\":{},\"status\":{},\"opened_at\":{},\"opened_by\":{},\"reopened\":{},\"plans\":[{}],\"timeline\":[{}]}}",
                q(&i.id), q(&i.site), q(&i.title), i.priority, i.exercise, q(i.status.word()), q(&i.opened_at), q(&i.opened_by), i.reopened, plans.join(","), tl.join(",")
            )
        })
        .collect();
    Ok(format!("{{\"unreadable_lines\":{},\"incidents\":[{}]}}", log.rejected.len(), rows.join(",")))
}

// ---------- watch editing ----------

/// A time typed on a screen (`2026-11-08T06:00`, local) written with this computer's offset.
fn local_instant(local: &str, off: i64) -> Result<String, String> {
    let l = local.trim();
    let utc = security::parse_instant(&format!("{}Z", if l.len() == 16 { l.to_string() } else { format!("{:.16}", l) })).filter(|_| l.len() >= 16 && l.is_ascii()).ok_or("The time needs a date and a time.")?;
    Ok(format_instant(utc - off, off))
}

fn load_all(user_root: &Path) -> Result<security::Parsed<sched::Watch>, String> {
    store::load_watches(user_root)
}

fn put(user_root: &Path, all: &security::Parsed<sched::Watch>) -> Result<(), String> {
    store::save_watches(user_root, &all.items, &all.rejected)
}

/// Every watch at a site (soonest first) and the plain-words warnings about them.
pub fn watch_list(user_root: &Path, site: &str) -> Result<String, String> {
    let all = load_all(user_root)?;
    let mut mine: Vec<sched::Watch> = all.items.iter().filter(|w| w.site == site).cloned().collect();
    mine.sort_by_key(|w| w.span().map(|(a, _)| a).unwrap_or(0));
    let name = |id: &str| all.items.iter().find(|w| w.id == id).map(|w| w.person.clone()).unwrap_or_default();
    let mut warnings: Vec<String> = Vec::new();
    for (a, b, len) in sched::overlaps(&mine, 15) {
        warnings.push(format!("{} and {} overlap by {} minutes.", name(&a), name(&b), len));
    }
    for (a, b) in sched::double_booked(&all.items) {
        if mine.iter().any(|w| w.id == a || w.id == b) {
            warnings.push(format!("{} is on two watches at the same time.", name(&a)));
        }
    }
    for (who, _, _, rest) in sched::short_rest(&mine, 6) {
        warnings.push(format!("{} gets only {} between watches.", who, if rest % 60 == 0 { format!("{} hours", rest / 60) } else { format!("{} h {} min", rest / 60, rest % 60) }));
    }
    let rows: Vec<String> = mine
        .iter()
        .map(|w| format!("{{\"id\":{},\"person\":{},\"role\":{},\"start\":{},\"end\":{},\"state\":{},\"ack_at\":{},\"entered_at\":{},\"notes\":{}}}", q(&w.id), q(&w.person), q(&w.role), q(&w.start), q(&w.end), q(w.state.word()), q(&w.ack_at), q(&w.entered_at), q(&w.notes)))
        .collect();
    Ok(format!("{{\"site\":{},\"watches\":[{}],\"warnings\":[{}],\"unreadable_lines\":{}}}", q(site), rows.join(","), warnings.iter().map(|w| q(w)).collect::<Vec<_>>().join(","), all.rejected.len()))
}

fn known_site(user_root: &Path, site: &str) -> Result<(), String> {
    if store::load_sites(user_root)?.items.iter().any(|s| s.id == site) { Ok(()) } else { Err("That site is not on the list.".to_string()) }
}

/// Draft a watchbill: back-to-back watches handed round the people in order. Saved as `proposed`: they cover
/// nothing until a person schedules them. Returns how many were made.
pub fn watch_draft(user_root: &Path, site: &str, people: &[String], start_local: &str, hours: u32, count: u32, offset_minutes: i64) -> Result<usize, String> {
    let off = offset_ok(offset_minutes)?;
    known_site(user_root, site)?;
    let people: Vec<String> = people.iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    let mut all = load_all(user_root)?;
    let start = local_instant(start_local, off)?;
    let drafts = sched::draft_watchbill(site, &people, &start, hours, count, &all.items)?;
    let n = drafts.len();
    all.items.extend(drafts);
    put(user_root, &all)?;
    Ok(n)
}

/// A person writes one watch in by hand. Entering it is the person's approval, so it starts as `scheduled`.
pub fn watch_add(user_root: &Path, site: &str, person: &str, role: &str, start_local: &str, end_local: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    known_site(user_root, site)?;
    if person.trim().is_empty() {
        return Err("Say who is on watch.".to_string());
    }
    let mut all = load_all(user_root)?;
    let w = sched::Watch {
        id: crate::ops::next_id('w', all.items.iter().map(|w| w.id.clone())),
        site: site.to_string(),
        person: person.trim().to_string(),
        role: if role.trim().is_empty() { "watch".to_string() } else { role.trim().to_string() },
        start: local_instant(start_local, off)?,
        end: local_instant(end_local, off)?,
        state: sched::WatchState::Scheduled,
        ack_at: String::new(),
        entered_at: format_instant(now_minutes(), off),
        notes: String::new(),
    };
    if w.span().is_none() {
        return Err("The watch has to end after it starts.".to_string());
    }
    let id = w.id.clone();
    all.items.push(w);
    put(user_root, &all)?;
    Ok(id)
}

fn find_mut<'a>(all: &'a mut security::Parsed<sched::Watch>, id: &str) -> Result<&'a mut sched::Watch, String> {
    all.items.iter_mut().find(|w| w.id == id).ok_or_else(|| "That watch is not on the list.".to_string())
}

/// Move a watch along: scheduled, on-duty, completed or missed. (Acknowledging has its own command so the time is kept.)
pub fn watch_move(user_root: &Path, id: &str, to: &str) -> Result<(), String> {
    let to = sched::WatchState::from_word(to).ok_or("That is not a watch state.")?;
    let mut all = load_all(user_root)?;
    let w = find_mut(&mut all, id)?;
    *w = sched::change_state(w, to)?;
    put(user_root, &all)
}

/// The person says they have the watch. `at_local` empty means now; a handover that really happened earlier (by
/// radio, or on paper) passes its real time, and the entry time is still now.
pub fn watch_ack(user_root: &Path, id: &str, at_local: &str, offset_minutes: i64) -> Result<(), String> {
    let off = offset_ok(offset_minutes)?;
    let now = now_minutes();
    let entered = format_instant(now, off);
    let at = if at_local.trim().is_empty() { entered.clone() } else { local_instant(at_local, off)? };
    if security::parse_instant(&at).map(|t| t > now + 5).unwrap_or(true) {
        return Err("That time is in the future.".to_string());
    }
    let mut all = load_all(user_root)?;
    let w = find_mut(&mut all, id)?;
    *w = sched::acknowledge(w, &at, &entered)?;
    put(user_root, &all)
}

/// Throw away a draft that was never scheduled. Anything a person has agreed to stays in the record (use
/// "missed" instead), so history is never quietly removed.
pub fn watch_discard(user_root: &Path, id: &str) -> Result<(), String> {
    let mut all = load_all(user_root)?;
    let w = all.items.iter().find(|w| w.id == id).ok_or("That watch is not on the list.")?;
    if w.state != sched::WatchState::Proposed {
        return Err("Only a draft can be thrown away. Mark this one missed instead, so the record stays.".to_string());
    }
    all.items.retain(|w| w.id != id);
    put(user_root, &all)
}

/// Schedule every draft at a site in one step: the person has read the roster and agrees to it.
pub fn watch_schedule_all(user_root: &Path, site: &str) -> Result<usize, String> {
    let mut all = load_all(user_root)?;
    let mut n = 0;
    for w in all.items.iter_mut().filter(|w| w.site == site && w.state == sched::WatchState::Proposed) {
        *w = sched::change_state(w, sched::WatchState::Scheduled)?;
        n += 1;
    }
    if n > 0 {
        put(user_root, &all)?;
    }
    Ok(n)
}

// ---------- reports from Pocket phones ----------

/// Reports waiting in the Pocket mailbox, each with the reason it cannot be used if it cannot.
pub fn pocket_list(user_root: &Path) -> Result<String, String> {
    let rows: Vec<String> = crate::security_pocket::waiting(user_root)
        .iter()
        .map(|w| format!("{{\"name\":{},\"size\":{},\"device\":{},\"person\":{},\"created\":{},\"entries\":{},\"problem\":{}}}", q(&w.name), w.size, q(&w.device), q(&w.person), q(&w.created), w.entries, q(&w.problem)))
        .collect();
    Ok(format!("{{\"waiting\":[{}]}}", rows.join(",")))
}

/// The host accepts a report into the records. Returns how many entries were added.
pub fn pocket_accept(user_root: &Path, name: &str) -> Result<usize, String> {
    crate::security_pocket::accept(user_root, name)
}

/// The host sets a report aside without using it. It is kept, not deleted.
pub fn pocket_set_aside(user_root: &Path, name: &str) -> Result<(), String> {
    crate::security_pocket::set_aside(user_root, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-secapi-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn a_new_program_says_unknown_not_safe() {
        let r = tmp("fresh");
        let j = overview(&r, -300).unwrap();
        assert!(j.contains("\"headline\":\"unknown\"") && j.contains("\"all_normal_confirmed\":false") && j.contains("\"sites\":[]"));
        site_save(&r, "", "Home", "America/New_York", "", "").unwrap();
        let j = overview(&r, -300).unwrap();
        assert!(j.contains("\"posture\":\"unknown\"") && j.contains("\"never_assessed\":true"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn site_codes_come_from_the_name_and_never_change() {
        let r = tmp("sites");
        assert_eq!(site_save(&r, "", "Evacuation destination", "", "", "").unwrap(), "evacuation-destination");
        // renaming keeps the code, so history stays attached
        site_save(&r, "evacuation-destination", "Aunt's farm", "", "", "").unwrap();
        let s = store::load_sites(&r).unwrap().items;
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].id.as_str(), s[0].name.as_str()), ("evacuation-destination", "Aunt's farm"));
        assert!(site_save(&r, "", "  ", "", "", "").is_err());
        assert!(site_save(&r, "", "???", "", "", "").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn setting_posture_uses_the_computers_clock_and_shows_on_the_overview() {
        let r = tmp("posture");
        site_save(&r, "", "Home", "", "", "").unwrap();
        assert!(posture_set(&r, "home", "elevated", "Dad", "", 4, -300).is_err(), "elevated needs a reason");
        assert!(posture_set(&r, "home", "safe", "Dad", "x", 4, -300).is_err());
        assert!(posture_set(&r, "home", "normal", "", "", 4, -300).is_err());
        assert!(posture_set(&r, "home", "normal", "Dad", "", 4, 99_999).is_err());
        posture_set(&r, "home", "elevated", "Dad", "Strange car", 4, -300).unwrap();
        let j = overview(&r, -300).unwrap();
        assert!(j.contains("\"headline\":\"elevated\"") && j.contains("Strange car") && j.contains("\"set_by\":\"Dad\""));
        // four hours on, with nobody checking, it is flagged but not lowered
        let later = now_minutes() + 5 * 60;
        let j = overview_at(&r, -300, later).unwrap();
        assert!(j.contains("\"posture\":\"elevated\"") && j.contains("\"check_due\":true"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn watch_strip_reports_gaps_and_open_handovers() {
        let r = tmp("strip");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        // a watch that ended an hour ago with no handover, and one now being waited for
        let now = now_minutes();
        let w = |id: &str, a: i64, b: i64, st: sched::WatchState, who: &str| sched::Watch {
            id: id.into(), site: "camp".into(), person: who.into(), role: "watch".into(), start: format_instant(a, 0), end: format_instant(b, 0), state: st, ack_at: String::new(), entered_at: String::new(), notes: String::new(),
        };
        let ws = vec![w("w1", now - 300, now - 60, sched::WatchState::OnDuty, "Dad"), w("w2", now - 30, now + 300, sched::WatchState::Scheduled, "Mom")];
        store::save_watches(&r, &ws, &[]).unwrap();
        let j = watch_strip_at(&r, "camp", 0, now).unwrap();
        assert!(j.contains("handover-open") && j.contains("not-acknowledged"));
        assert!(j.contains("\"cover\":\"planned\"") && j.contains("\"cover\":\"confirmed\"") || j.contains("\"cover\":\"gap\""));
        // another site sees none of it
        let j = watch_strip_at(&r, "home", 0, now).unwrap();
        assert!(j.contains("\"needs_a_look\":[]") && j.contains("\"watches\":[]"));
        // the window is exactly one local day
        assert!(j.contains("\"gap_minutes\":1440"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn incidents_open_and_collect_entries_through_the_api() {
        let r = tmp("inc");
        site_save(&r, "", "Home", "", "", "").unwrap();
        assert!(incident_open(&r, "nowhere", "x", 2, false, "Dad", -300).is_err());
        assert!(incident_open(&r, "home", "Car at gate", 7, false, "Dad", -300).is_err());
        let id = incident_open(&r, "home", "Car at gate", 2, false, "Dad", -300).unwrap();
        assert_eq!(id, "i1");
        assert_eq!(incident_open(&r, "home", "Practice drill", 3, true, "Dad", -300).unwrap(), "i2");
        incident_entry(&r, "i1", "note", "", "Two men, blue truck", "observed", "Dad", "", "", -300).unwrap();
        assert!(incident_entry(&r, "i1", "plan", "x", "", "observed", "Dad", "", "", -300).is_err(), "plans go through activation");
        assert!(incident_entry(&r, "i1", "open", "home", "again", "observed", "Dad", "", "", -300).is_err());
        assert!(incident_entry(&r, "i1", "note", "", "hi", "rumour", "Dad", "", "", -300).is_err());
        assert!(incident_entry(&r, "i1", "status", "closed", "", "observed", "Dad", "", "", -300).is_err(), "closing needs a reason");
        incident_entry(&r, "i1", "status", "active", "Confirmed on camera", "observed", "Dad", "", "", -300).unwrap();
        let j = incident_list(&r).unwrap();
        assert!(j.contains("Two men, blue truck") && j.contains("\"status\":\"active\"") && j.contains("\"exercise\":true"));
        // only the real incident is counted as live
        let o = overview(&r, -300).unwrap();
        assert!(o.contains("\"live_incidents\":1"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn unreadable_lines_are_counted_for_the_screen() {
        let r = tmp("bad");
        site_save(&r, "", "Home", "", "", "").unwrap();
        fs::write(store::watch_path(&r), "w1\thome\tDad\twatch\tnot a time\tx\tscheduled\t\t\t\n").unwrap();
        assert!(overview(&r, 0).unwrap().contains("\"unreadable_lines\":1"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn text_in_records_cannot_break_the_json() {
        let r = tmp("json");
        site_save(&r, "", "Home", "", "", "").unwrap();
        incident_open(&r, "home", "He said \"stop\"\nthen left\\", 2, false, "Dad", 0).unwrap();
        let j = incident_list(&r).unwrap();
        assert!(j.contains("He said \\\"stop\\\"\\nthen left\\\\"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn overview_looks_are_one_list_for_dashboard_and_command() {
        let r = tmp("looks");
        site_save(&r, "", "Home", "", "", "").unwrap();
        let j = overview(&r, 0).unwrap();
        assert!(j.contains("\"needs_a_look\":1") && j.contains("No posture has been set"));
        posture_set(&r, "home", "normal", "Dad", "", 0, 0).unwrap();
        let j = overview(&r, 0).unwrap();
        assert!(j.contains("\"needs_a_look\":0") && j.contains("\"looks\":[]"));
        // a site with no watchbill at all is not flagged for gaps; one with a watch and a hole is
        let day = now_minutes().div_euclid(1440) * 1440; // a watch inside today, whatever time the test runs
        watch_add(&r, "home", "Dad", "", &local_for(day + 60), &local_for(day + 120), 0).unwrap();
        let j = overview(&r, 0).unwrap();
        assert!(j.contains("Nobody is covering"));
        assert!(j.contains("\"sites_on_watch\":0"));
        let _ = fs::remove_dir_all(&r);
    }

    fn local_for(utc_min: i64) -> String {
        format_instant(utc_min, 0)[..16].to_string()
    }

    #[test]
    fn local_times_become_offset_times() {
        assert_eq!(local_instant("2026-11-08T06:00", -300).unwrap(), "2026-11-08T06:00-05:00");
        assert_eq!(local_instant("2026-11-08T06:00", 0).unwrap(), "2026-11-08T06:00Z");
        assert!(local_instant("tomorrow", 0).is_err());
        assert!(local_instant("2026-13-08T06:00", 0).is_err());
        assert!(local_instant("2026-11-08T06:00:00", 0).is_ok(), "seconds are ignored, not trusted");
    }

    #[test]
    fn drafting_then_scheduling_then_acknowledging() {
        let r = tmp("draft");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        let people: Vec<String> = ["Dad", "Mom", " ", "Sam", "Lee"].iter().map(|s| s.to_string()).collect();
        assert!(watch_draft(&r, "nowhere", &people, "2026-11-08T00:00", 6, 4, -300).is_err());
        assert_eq!(watch_draft(&r, "camp", &people, "2026-11-08T00:00", 6, 4, -300).unwrap(), 4);
        let j = watch_list(&r, "camp").unwrap();
        assert!(j.contains("\"state\":\"proposed\"") && !j.contains("\"state\":\"scheduled\""));
        // a draft covers nothing
        let strip = watch_strip_at(&r, "camp", -300, security::parse_instant("2026-11-08T12:00-05:00").unwrap()).unwrap();
        assert!(strip.contains("\"gap_minutes\":1440"));
        assert_eq!(watch_schedule_all(&r, "camp").unwrap(), 4);
        assert_eq!(watch_schedule_all(&r, "camp").unwrap(), 0);
        let strip = watch_strip_at(&r, "camp", -300, security::parse_instant("2026-11-08T12:00-05:00").unwrap()).unwrap();
        assert!(strip.contains("\"gap_minutes\":0") && strip.contains("planned"));
        // acknowledging w1: needs scheduled, and not in the future
        assert!(watch_ack(&r, "w9", "", 0).is_err());
        watch_ack(&r, "w1", "", -300).unwrap();
        assert!(watch_ack(&r, "w1", "", -300).is_err(), "only once");
        assert!(watch_ack(&r, "w2", "2999-01-01T00:00", -300).is_err());
        let all = store::load_watches(&r).unwrap().items;
        let w1 = all.iter().find(|w| w.id == "w1").unwrap();
        assert_eq!(w1.state, sched::WatchState::Acknowledged);
        assert!(!w1.ack_at.is_empty() && !w1.entered_at.is_empty());
        // a late entry keeps the real time apart from the entry time
        let earlier = local_for(now_minutes() - 90);
        watch_ack(&r, "w2", &earlier, 0).unwrap();
        let all = store::load_watches(&r).unwrap().items;
        let w2 = all.iter().find(|w| w.id == "w2").unwrap();
        assert!(security::parse_instant(&w2.ack_at).unwrap() < security::parse_instant(&w2.entered_at).unwrap());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn manual_watches_moves_and_discards_follow_the_rules() {
        let r = tmp("manual");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        assert!(watch_add(&r, "camp", "", "", "2026-11-08T00:00", "2026-11-08T06:00", 0).is_err());
        assert!(watch_add(&r, "camp", "Dad", "", "2026-11-08T06:00", "2026-11-08T00:00", 0).is_err());
        let id = watch_add(&r, "camp", "Dad", "", "2026-11-08T00:00", "2026-11-08T06:00", 0).unwrap();
        assert_eq!(id, "w1");
        // scheduled work cannot be deleted, only marked missed
        assert!(watch_discard(&r, "w1").is_err());
        assert!(watch_move(&r, "w1", "completed").is_err());
        assert!(watch_move(&r, "w1", "nonsense").is_err());
        watch_move(&r, "w1", "missed").unwrap();
        watch_move(&r, "w1", "scheduled").unwrap();
        // a draft can be thrown away
        watch_draft(&r, "camp", &["Mom".to_string()], "2026-11-09T00:00", 6, 1, 0).unwrap();
        watch_discard(&r, "w2").unwrap();
        assert_eq!(store::load_watches(&r).unwrap().items.len(), 1);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn warnings_say_what_is_wrong_in_plain_words() {
        let r = tmp("warn");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        watch_add(&r, "camp", "Dad", "", "2026-11-08T00:00", "2026-11-08T06:00", 0).unwrap();
        watch_add(&r, "camp", "Mom", "", "2026-11-08T03:00", "2026-11-08T09:00", 0).unwrap();
        watch_add(&r, "camp", "Dad", "", "2026-11-08T08:00", "2026-11-08T12:00", 0).unwrap();
        let j = watch_list(&r, "camp").unwrap();
        assert!(j.contains("Dad and Mom overlap by 180 minutes."), "{}", j);
        assert!(j.contains("Dad gets only") || j.contains("dad gets only"), "{}", j);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn unreadable_watch_lines_survive_every_edit() {
        let r = tmp("keep");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        let bad = "w7\tcamp\tMom\twatch\tnot a time\tx\tscheduled\t\t\t";
        fs::create_dir_all(store::dir(&r)).unwrap();
        fs::write(store::watch_path(&r), format!("{}\n", bad)).unwrap();
        watch_add(&r, "camp", "Dad", "", "2026-11-08T00:00", "2026-11-08T06:00", 0).unwrap();
        watch_move(&r, "w1", "missed").unwrap();
        assert!(fs::read_to_string(store::watch_path(&r)).unwrap().contains(bad));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn waiting_pocket_reports_show_on_the_overview_and_through_the_api() {
        let r = tmp("pocketapi");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        posture_set(&r, "camp", "normal", "Dad", "", 0, 0).unwrap();
        assert!(overview(&r, 0).unwrap().contains("\"pocket_waiting\":0"));
        let t = "2026-11-08T10:00Z";
        let o = incident::Entry { id: "n10-e1".into(), incident: "n10-i1".into(), kind: incident::Kind::Open, happened_at: t.into(), recorded_at: t.into(), by: "x".into(), source: incident::Source::Observed, arg1: "camp".into(), arg2: "2".into(), arg3: "live".into(), text: "Noise".into(), supersedes: String::new() };
        crate::security_pocket::drop_file(&r, "n10-0001.txt", &crate::security_pocket::render_report("n10", "Sam", t, &[o])).unwrap();
        let j = overview(&r, 0).unwrap();
        assert!(j.contains("\"pocket_waiting\":1") && j.contains("\"needs_a_look\":1"), "{}", j);
        assert!(pocket_list(&r).unwrap().contains("\"person\":\"Sam\""));
        assert_eq!(pocket_accept(&r, "n10-0001.txt").unwrap(), 1);
        assert!(overview(&r, 0).unwrap().contains("\"pocket_waiting\":0"));
        assert!(incident_list(&r).unwrap().contains("\"source\":\"reported\""));
        let _ = fs::remove_dir_all(&r);
    }
}
