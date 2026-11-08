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
    for (s, st) in sites.items.iter().zip(ov.sites.iter()) {
        let cur = security::current_entry(&log.items, &s.id);
        let on: Vec<String> = sched::on_watch_now(&watches.items, &s.id, now).iter().map(|w| q(&w.person)).collect();
        let live = incident::live_open(&built).iter().filter(|i| i.site == s.id).count();
        rows.push(format!(
            "{{\"id\":{},\"name\":{},\"tz\":{},\"posture\":{},\"check_due\":{},\"never_assessed\":{},\"set_by\":{},\"reason\":{},\"at\":{},\"review_by\":{},\"on_watch\":[{}],\"live_incidents\":{}}}",
            q(&s.id), q(&s.name), q(&s.tz), q(st.posture.word()), st.check_due, st.never_assessed,
            q(cur.map(|e| e.set_by.as_str()).unwrap_or("")), q(cur.map(|e| e.reason.as_str()).unwrap_or("")), q(cur.map(|e| e.at.as_str()).unwrap_or("")), q(cur.map(|e| e.review_by.as_str()).unwrap_or("")),
            on.join(","), live
        ));
    }
    let rejected = sites.rejected.len() + log.rejected.len() + watches.rejected.len() + incidents.rejected.len();
    Ok(format!(
        "{{\"now\":{},\"headline\":{},\"unknown\":{},\"check_due\":{},\"all_normal_confirmed\":{},\"live_incidents\":{},\"unreadable_lines\":{},\"sites\":[{}]}}",
        q(&format_instant(now, off)), q(ov.headline.word()), ov.unknown, ov.check_due, ov.all_normal_confirmed, incident::live_open(&built).len(), rejected, rows.join(",")
    ))
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
}
