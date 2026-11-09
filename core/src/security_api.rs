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
use crate::security_comms as comms;
use crate::security_outing as outing;
use crate::security_site as siterec;
use crate::security_equipment as equip;
use crate::security_plans as plans;

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
    "security_comms_list",
    "security_comms_add",
    "security_coverage_list",
    "security_coverage_add",
    "security_channels_get",
    "security_channels_save",
    "security_training_list",
    "security_training_record",
    "security_outing_list",
    "security_outing_start",
    "security_outing_event",
    "security_assets_list",
    "security_asset_save",
    "security_asset_inspect",
    "security_schedule_get",
    "security_schedule_save",
    "security_equipment_list",
    "security_equipment_add",
    "security_routes_get",
    "security_routes_save",
    "security_availability_list",
    "security_availability_set",
    "security_plans_list",
    "security_plan_read",
    "security_plan_save",
    "security_plan_activate",
    "security_weather_list",
    "security_weather_set",
    "security_export_records",
    "security_tracks_list",
    "security_track_export",
    "security_pocket_list",
    "security_pocket_sheet",
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
    let outing_log = outing::load(user_root)?;
    let outings = outing::build(&outing_log.items);
    let comms_log = comms::load_comms(user_root)?;
    let cov_log = comms::load_coverage(user_root)?;
    let train_log = comms::load_training(user_root)?;
    let channel_file = comms::load_channels(user_root)?;
    let asset_file = siterec::load_assets(user_root)?;
    let insp_log = siterec::load_inspections(user_root)?;
    let avail_log = siterec::load_availability(user_root)?;
    let equip_log = equip::load(user_root)?;
    let inventory = crate::userdata::UserDir::new(user_root).load_inventory().unwrap_or_default();
    let gear = equip::gear(&equip_log.items, &inventory);
    let rules_file = siterec::load_schedule(user_root)?;
    let recs = Recs { gear, rules: rules_file.items.clone(), outings: outings.clone(), comms: comms_log.items.clone(), coverage: cov_log.items.clone(), training: train_log.items.clone(), assets: asset_file.items.clone(), inspections: insp_log.items.clone(), avail: avail_log.items.clone() };
    let built = incident::build(&incidents.items);
    let ov = security::overview(&sites.items, &log.items, now);
    let mut rows = Vec::new();
    let mut look_total = 0usize;
    // what the program tells the host about while it is open: a key (so the same thing is announced once), the words, and how urgent
    let mut alerts: Vec<String> = Vec::new();
    let alert = |key: String, text: String, urgent: bool| format!("{{\"key\":{},\"text\":{},\"urgent\":{}}}", q(&key), q(&text), urgent);
    for (s, st) in sites.items.iter().zip(ov.sites.iter()) {
        let cur = security::current_entry(&log.items, &s.id);
        let on: Vec<String> = sched::on_watch_now(&watches.items, &s.id, now).iter().map(|w| q(&w.person)).collect();
        let live = incident::live_open(&built).iter().filter(|i| i.site == s.id).count();
        let looks = site_looks(&s.id, st, cur, &watches.items, &recs, off, now);
        let out_now = outings.iter().filter(|o| o.site == s.id && o.state == outing::State::Out).count();
        let primary = comms::channels_for(&channel_file.items, &s.id).0.into_iter().find(|c| c.role == "primary").map(|c| format!("CH {}", c.channel)).unwrap_or_default();
        let last_patrol = outing::last_patrol(&outings, &s.id).map(|m| format_instant(m, off)).unwrap_or_default();
        look_total += looks.len();
        if live > 0 {
            alerts.push(alert(format!("incident|{}|{}", s.id, live), format!("{}: {} incident{} open.", s.name, live, if live == 1 { "" } else { "s" }), true));
        }
        for l in &looks {
            alerts.push(alert(format!("look|{}|{}", s.id, l), format!("{}: {}", s.name, l), false));
        }
        rows.push(format!(
            "{{\"id\":{},\"name\":{},\"tz\":{},\"posture\":{},\"check_due\":{},\"never_assessed\":{},\"set_by\":{},\"reason\":{},\"at\":{},\"review_by\":{},\"on_watch\":[{}],\"live_incidents\":{},\"out_now\":{},\"last_patrol\":{},\"primary_channel\":{},\"looks\":[{}]}}",
            q(&s.id), q(&s.name), q(&s.tz), q(st.posture.word()), st.check_due, st.never_assessed,
            q(cur.map(|e| e.set_by.as_str()).unwrap_or("")), q(cur.map(|e| e.reason.as_str()).unwrap_or("")), q(cur.map(|e| e.at.as_str()).unwrap_or("")), q(cur.map(|e| e.review_by.as_str()).unwrap_or("")),
            on.join(","), live, out_now, q(&last_patrol), q(&primary), looks.iter().map(|l| q(l)).collect::<Vec<_>>().join(",")
        ));
    }
    let sites_on_watch = sites.items.iter().filter(|s| !sched::on_watch_now(&watches.items, &s.id, now).is_empty()).count();
    let rejected = sites.rejected.len() + log.rejected.len() + watches.rejected.len() + incidents.rejected.len() + outing_log.rejected.len() + comms_log.rejected.len() + cov_log.rejected.len() + train_log.rejected.len() + channel_file.rejected.len() + asset_file.rejected.len() + insp_log.rejected.len() + avail_log.rejected.len() + rules_file.rejected.len() + equip_log.rejected.len();
    if rejected > 0 {
        look_total += 1;
        alerts.push(alert(format!("unreadable|{}", rejected), format!("{} line{} in the security records could not be read. Nothing was changed.", rejected, if rejected == 1 { "" } else { "s" }), false));
    }
    let pocket = crate::security_pocket::waiting_count(user_root);
    if pocket > 0 {
        look_total += 1;
        alerts.push(alert(format!("pocket|{}", pocket), format!("{} report{} from a phone {} waiting for you to accept.", pocket, if pocket == 1 { "" } else { "s" }, if pocket == 1 { "is" } else { "are" }), false));
    }
    Ok(format!(
        "{{\"now\":{},\"headline\":{},\"unknown\":{},\"check_due\":{},\"all_normal_confirmed\":{},\"live_incidents\":{},\"unreadable_lines\":{},\"pocket_waiting\":{},\"needs_a_look\":{},\"sites_on_watch\":{},\"sites\":[{}],\"alerts\":[{}]}}",
        q(&format_instant(now, off)), q(ov.headline.word()), ov.unknown, ov.check_due, ov.all_normal_confirmed, incident::live_open(&built).len(), rejected, pocket, look_total, sites_on_watch, rows.join(","), alerts.join(",")
    ))
}

/// The records, other than the watchbill and posture, that add lines to "needs a look".
struct Recs {
    outings: Vec<outing::Outing>,
    comms: Vec<comms::CommsEntry>,
    coverage: Vec<comms::Coverage>,
    training: Vec<comms::Training>,
    gear: Vec<equip::Gear>,
    rules: Vec<siterec::PatrolRule>,
    assets: Vec<siterec::Asset>,
    inspections: Vec<siterec::Inspection>,
    avail: Vec<siterec::Availability>,
}

/// The plain-words "needs a look" lines for one site. One place makes them, so the Dashboard count and the
/// Command list can never disagree.
fn site_looks(site: &str, st: &security::SiteStatus, cur: Option<&PostureEntry>, watches: &[sched::Watch], recs: &Recs, off: i64, now: i64) -> Vec<String> {
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
    for o in recs.outings.iter().filter(|o| o.site == site) {
        out.extend(outing::looks(o, now, off));
    }
    out.extend(comms::looks(site, &recs.comms, &recs.coverage));
    out.extend(comms::training_looks(site, watches, &recs.training));
    let patrols: Vec<(String, i64)> = recs.outings.iter().filter(|o| o.patrol && o.site == site && matches!(o.state, outing::State::Out | outing::State::Back)).map(|o| (o.task.clone(), o.ended.unwrap_or(o.left))).collect();
    out.extend(equip::looks(site, &recs.gear, now));
    out.extend(siterec::schedule_looks(site, &recs.rules, &patrols, off, now));
    out.extend(siterec::asset_looks(site, &recs.assets, &recs.inspections, now));
    out.extend(siterec::availability_looks(site, watches, &recs.avail, now));
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

// ---------- outings and patrols ----------

fn when_json(m: i64, off: i64) -> String {
    q(&format_instant(m, off))
}

/// Everyone out now (and anyone who ended in the last day), at one site or all (`site` empty), with the plain-words
/// warnings for each. Times are written with the screen's offset.
pub fn outing_list(user_root: &Path, site: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    outing_list_at(user_root, site, off, now_minutes())
}

pub fn outing_list_at(user_root: &Path, site: &str, off: i64, now: i64) -> Result<String, String> {
    let log = outing::load(user_root)?;
    let all = outing::build(&log.items);
    let mut shown: Vec<&outing::Outing> = all.iter().filter(|o| (site.is_empty() || o.site == site) && (o.state == outing::State::Out || o.ended.map(|e| now - e < 1440).unwrap_or(false))).collect();
    shown.sort_by_key(|o| (o.state != outing::State::Out, o.back));
    let rows: Vec<String> = shown
        .iter()
        .map(|o| {
            let last = o.last.as_ref().map(|(t, st, what)| format!("{{\"at\":{},\"status\":{},\"text\":{}}}", when_json(*t, off), q(st), q(what))).unwrap_or_else(|| "null".to_string());
            let cps: Vec<String> = o.checkpoints.iter().map(|(t, n, w)| format!("{{\"at\":{},\"name\":{},\"text\":{}}}", when_json(*t, off), q(n), q(w))).collect();
            format!(
                "{{\"id\":{},\"patrol\":{},\"site\":{},\"people\":[{}],\"task\":{},\"method\":{},\"left\":{},\"back\":{},\"next\":{},\"state\":{},\"ended\":{},\"last\":{},\"checkpoints\":[{}],\"looks\":[{}]}}",
                q(&o.id), o.patrol, q(&o.site), o.people.iter().map(|p| q(p)).collect::<Vec<_>>().join(","), q(&o.task), q(&o.method),
                when_json(o.left, off), when_json(o.back, off), o.next.map(|n| when_json(n, off)).unwrap_or_else(|| "\"\"".to_string()),
                q(match o.state { outing::State::Out => "out", outing::State::Back => "back", outing::State::Cancelled => "cancelled" }),
                o.ended.map(|n| when_json(n, off)).unwrap_or_else(|| "\"\"".to_string()), last, cps.join(","),
                outing::looks(o, now, off).iter().map(|l| q(l)).collect::<Vec<_>>().join(",")
            )
        })
        .collect();
    let lp = if site.is_empty() { None } else { outing::last_patrol(&all, site) };
    Ok(format!("{{\"outings\":[{}],\"last_patrol\":{},\"unreadable_lines\":{}}}", rows.join(","), lp.map(|m| when_json(m, off)).unwrap_or_else(|| "\"\"".to_string()), log.rejected.len()))
}

fn put_outing(user_root: &Path, mut e: outing::Entry, off: i64) -> Result<(), String> {
    let log = outing::load(user_root)?;
    e.id = crate::ops::next_id('e', log.items.iter().map(|x| x.id.clone()));
    e.recorded = format_instant(now_minutes(), off);
    outing::append(user_root, &e)
}

/// People go out. `back_local` and `next_local` are times typed on the screen (local, `2026-11-08T14:00`); `next_local`
/// may be empty. `kind` is `outing` or `patrol`. Returns the new outing's code.
pub fn outing_start(user_root: &Path, site: &str, kind: &str, people: &str, task: &str, back_local: &str, next_local: &str, method: &str, by: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    known_site(user_root, site)?;
    let log = outing::load(user_root)?;
    let id = crate::ops::next_id('o', log.items.iter().map(|x| x.outing.clone()));
    let now = format_instant(now_minutes(), off);
    let e = outing::Entry {
        id: String::new(), outing: id.clone(), ev: outing::Ev::Out, at: now, recorded: String::new(), by: by.trim().to_string(), source: "observed".into(),
        a1: kind.to_string(), a2: site.to_string(), a3: people.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(", "),
        back: local_instant(back_local, off)?, next: if next_local.trim().is_empty() { String::new() } else { local_instant(next_local, off)? },
        method: method.trim().to_string(), text: task.trim().to_string(),
    };
    put_outing(user_root, e, off)?;
    Ok(id)
}

/// Something happens to an outing: `contact`, `checkpoint`, `extend`, `return`, `cancel` or `note`. The time is now.
/// `a1`/`a2` are the status (contact) or the checkpoint name and status (checkpoint). `back_local`/`next_local` may be empty.
pub fn outing_event(user_root: &Path, outing_id: &str, ev: &str, a1: &str, a2: &str, text: &str, back_local: &str, next_local: &str, by: &str, offset_minutes: i64) -> Result<(), String> {
    let off = offset_ok(offset_minutes)?;
    let ev = outing::Ev::from_word(ev).filter(|e| *e != outing::Ev::Out).ok_or("That is not something that can happen to an outing.")?;
    let e = outing::Entry {
        id: String::new(), outing: outing_id.to_string(), ev, at: format_instant(now_minutes(), off), recorded: String::new(), by: by.trim().to_string(), source: "observed".into(),
        a1: a1.trim().to_string(), a2: a2.trim().to_string(), a3: String::new(),
        back: if back_local.trim().is_empty() { String::new() } else { local_instant(back_local, off)? },
        next: if next_local.trim().is_empty() { String::new() } else { local_instant(next_local, off)? },
        method: String::new(), text: text.trim().to_string(),
    };
    put_outing(user_root, e, off)
}

// ---------- communications: log, coverage, channels, training ----------

fn rec_time(off: i64) -> (String, String) {
    let n = format_instant(now_minutes(), off);
    (n.clone(), n)
}

/// The communications log for a site, newest first (at most 200 lines), with the log categories.
pub fn comms_list(user_root: &Path, site: &str) -> Result<String, String> {
    let log = comms::load_comms(user_root)?;
    let open: Vec<String> = comms::open_items(&log.items).iter().map(|e| q(&e.id)).collect();
    let rows: Vec<String> = log.items.iter().rev().filter(|e| site.is_empty() || e.site == site).take(200).map(|e| {
        format!("{{\"id\":{},\"site\":{},\"at\":{},\"author\":{},\"station\":{},\"category\":{},\"event\":{},\"action\":{},\"status\":{},\"closes\":{}}}", q(&e.id), q(&e.site), q(&e.at), q(&e.author), q(&e.station), q(&e.category), q(&e.event), q(&e.action), q(&e.status), q(&e.closes))
    }).collect();
    let cats: Vec<String> = comms::CATEGORIES.iter().map(|c| format!("{{\"id\":{},\"label\":{}}}", q(c.0), q(c.1))).collect();
    Ok(format!("{{\"entries\":[{}],\"open\":[{}],\"categories\":[{}],\"unreadable_lines\":{}}}", rows.join(","), open.join(","), cats.join(","), log.rejected.len()))
}

/// Write one line in the communications log. `status` is open, complete, or closed (with `closes` = the entry it closes).
pub fn comms_add(user_root: &Path, site: &str, category: &str, station: &str, event: &str, action: &str, status: &str, closes: &str, author: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let log = comms::load_comms(user_root)?;
    let (at, rec) = rec_time(off);
    let e = comms::new_entry(&log.items, site, &at, &rec, author, station, category, event, action, status, closes);
    comms::append_comms(user_root, &e)?;
    Ok(e.id)
}

/// The newest test of each place at a site, and how many tests are on file.
pub fn coverage_list(user_root: &Path, site: &str) -> Result<String, String> {
    let log = comms::load_coverage(user_root)?;
    let n = |v: Option<u8>| v.map(|x| x.to_string()).unwrap_or_else(|| "null".to_string());
    let rows: Vec<String> = comms::latest_coverage(&log.items, site).iter().map(|c| format!("{{\"place\":{},\"p_in\":{},\"p_out\":{},\"a_in\":{},\"a_out\":{},\"at\":{},\"by\":{},\"note\":{}}}", q(&c.place), n(c.p_in), n(c.p_out), n(c.a_in), n(c.a_out), q(&c.at), q(&c.by), q(&c.note))).collect();
    Ok(format!("{{\"places\":[{}],\"tests_on_file\":{},\"unreadable_lines\":{}}}", rows.join(","), log.items.iter().filter(|c| c.site == site).count(), log.rejected.len()))
}

/// Record a coverage test. Ratings are 0 to 5, or empty if not tested.
pub fn coverage_add(user_root: &Path, site: &str, place: &str, p_in: &str, p_out: &str, a_in: &str, a_out: &str, by: &str, note: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let log = comms::load_coverage(user_root)?;
    let r = |s: &str| -> Result<Option<u8>, String> {
        let s = s.trim();
        if s.is_empty() { return Ok(None); }
        s.parse::<u8>().ok().filter(|n| *n <= 5).map(Some).ok_or_else(|| "Ratings go from 0 to 5.".to_string())
    };
    let (at, rec) = rec_time(off);
    let c = comms::Coverage { id: crate::ops::next_id('v', log.items.iter().map(|e| e.id.clone())), site: site.into(), place: place.trim().into(), p_in: r(p_in)?, p_out: r(p_out)?, a_in: r(a_in)?, a_out: r(a_out)?, at, recorded: rec, by: by.trim().into(), note: note.trim().chars().take(300).collect() };
    comms::append_coverage(user_root, &c)?;
    Ok(c.id)
}

/// A site's channel plan (the plan's own defaults until the site saves its own).
pub fn channels_get(user_root: &Path, site: &str) -> Result<String, String> {
    known_site(user_root, site)?;
    let file = comms::load_channels(user_root)?;
    let (list, is_default) = comms::channels_for(&file.items, site);
    let rows: Vec<String> = list.iter().map(|c| format!("{{\"channel\":{},\"code\":{},\"role\":{},\"purpose\":{}}}", q(&c.channel), q(&c.code), q(&c.role), q(&c.purpose))).collect();
    Ok(format!("{{\"channels\":[{}],\"is_default\":{}}}", rows.join(","), is_default))
}

pub fn channels_save(user_root: &Path, site: &str, list: &[comms::Channel]) -> Result<(), String> {
    comms::save_channels(user_root, site, list)
}

/// The twelve skills and each person's standing. `people` also lists names with no record yet, so the screen can start them.
pub fn training_list(user_root: &Path) -> Result<String, String> {
    let log = comms::load_training(user_root)?;
    let skills: Vec<String> = comms::SKILLS.iter().map(|s| q(s)).collect();
    let state = |v: &Option<bool>| match v { Some(true) => "true", Some(false) => "false", None => "null" };
    let people: Vec<String> = comms::standing(&log.items).iter().map(|(p, sk)| format!("{{\"person\":{},\"skills\":[{}],\"qualified\":{}}}", q(p), sk.iter().map(state).collect::<Vec<_>>().join(","), sk.iter().all(|s| *s == Some(true)))).collect();
    Ok(format!("{{\"skills\":[{}],\"people\":[{}],\"unreadable_lines\":{}}}", skills.join(","), people.join(","), log.rejected.len()))
}

/// Sign off (or mark for more practice) one skill for one person. `by` is whoever watched, and cannot be the same person.
pub fn training_record(user_root: &Path, person: &str, skill: u8, result: &str, by: &str, note: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let log = comms::load_training(user_root)?;
    let t = comms::new_training(&log.items, person, skill, result, &format_instant(now_minutes(), off), by, note);
    comms::append_training(user_root, &t)?;
    Ok(t.id)
}

/// Reports waiting in the Pocket mailbox, each with the reason it cannot be used if it cannot.
pub fn pocket_list(user_root: &Path) -> Result<String, String> {
    let rows: Vec<String> = crate::security_pocket::waiting(user_root)
        .iter()
        .map(|w| format!("{{\"name\":{},\"size\":{},\"device\":{},\"person\":{},\"created\":{},\"entries\":{},\"kind\":{},\"changed\":{},\"problem\":{}}}", q(&w.name), w.size, q(&w.device), q(&w.person), q(&w.created), w.entries, q(&w.kind), w.changed, q(&w.problem)))
        .collect();
    Ok(format!("{{\"waiting\":[{}]}}", rows.join(",")))
}

/// The host accepts a report into the records. Returns how many entries were added.
pub fn pocket_accept(user_root: &Path, name: &str, allow_moved: bool) -> Result<usize, String> {
    crate::security_pocket::accept_any(user_root, name, allow_moved)
}

/// A waiting count sheet line by line, so the host can see what each number was, is, and was counted as.
pub fn pocket_sheet(user_root: &Path, name: &str) -> Result<String, String> {
    crate::countsheet::detail_json(user_root, name)
}

/// The host sets a report aside without using it. It is kept, not deleted.
pub fn pocket_set_aside(user_root: &Path, name: &str) -> Result<(), String> {
    crate::security_pocket::set_aside(user_root, name)
}

// ---------- the Security tab, routes, availability, plans, weather, export ----------

/// Zones, barriers and alarms at a site with when each was last looked at and whether it is due.
pub fn assets_list(user_root: &Path, site: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    assets_list_at(user_root, site, off, now_minutes())
}
pub fn assets_list_at(user_root: &Path, site: &str, off: i64, now: i64) -> Result<String, String> {
    known_site(user_root, site)?;
    let file = siterec::load_assets(user_root)?;
    let log = siterec::load_inspections(user_root)?;
    let rows: Vec<String> = file.items.iter().filter(|a| a.site == site).map(|a| {
        let last = siterec::last_inspection(&log.items, &a.id);
        let t = last.and_then(|l| security::parse_instant(&l.at));
        let due = if last.map(|l| l.result == "needs-work").unwrap_or(false) { "needs-work" }
            else if a.every_days == 0 { "none" }
            else { match t { None => "never", Some(t) if now > t + a.every_days as i64 * 1440 => "overdue", _ => "ok" } };
        format!("{{\"id\":{},\"kind\":{},\"name\":{},\"every_days\":{},\"notes\":{},\"last_at\":{},\"last_by\":{},\"last_result\":{},\"last_note\":{},\"due\":{}}}",
            q(&a.id), q(&a.kind), q(&a.name), a.every_days, q(&a.notes), q(last.map(|l| l.at.as_str()).unwrap_or("")), q(last.map(|l| l.by.as_str()).unwrap_or("")), q(last.map(|l| l.result.as_str()).unwrap_or("")), q(last.map(|l| l.note.as_str()).unwrap_or("")), q(due))
    }).collect();
    let kinds: Vec<String> = siterec::KINDS.iter().map(|k| format!("{{\"id\":{},\"label\":{}}}", q(k.0), q(k.1))).collect();
    let _ = off;
    Ok(format!("{{\"assets\":[{}],\"kinds\":[{}],\"unreadable_lines\":{}}}", rows.join(","), kinds.join(","), file.rejected.len() + log.rejected.len()))
}
pub fn asset_save(user_root: &Path, site: &str, id: &str, kind: &str, name: &str, every_days: u32, notes: &str) -> Result<String, String> {
    siterec::save_asset(user_root, id, site, kind, name, every_days, notes)
}
pub fn asset_inspect(user_root: &Path, asset: &str, result: &str, by: &str, note: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    siterec::inspect(user_root, asset, result, by, note, off, now_minutes())
}

pub fn routes_get(user_root: &Path, site: &str) -> Result<String, String> {
    known_site(user_root, site)?;
    let f = siterec::load_routes(user_root)?;
    let rows: Vec<String> = f.items.iter().filter(|r| r.site == site).map(|r| format!("{{\"name\":{},\"checkpoints\":[{}]}}", q(&r.name), r.checkpoints.iter().map(|c| q(c)).collect::<Vec<_>>().join(","))).collect();
    Ok(format!("{{\"routes\":[{}],\"unreadable_lines\":{}}}", rows.join(","), f.rejected.len()))
}
pub fn schedule_get(user_root: &Path, site: &str) -> Result<String, String> {
    known_site(user_root, site)?;
    let f = siterec::load_schedule(user_root)?;
    let rows: Vec<String> = f.items.iter().filter(|r| r.site == site).map(|r| format!("{{\"route\":{},\"every_hours\":{},\"from\":{},\"to\":{}}}", q(&r.route), r.every_hours, q(&siterec::clock_text(r.from)), q(&siterec::clock_text(r.to)))).collect();
    Ok(format!("{{\"rules\":[{}],\"unreadable_lines\":{}}}", rows.join(","), f.rejected.len()))
}
/// Each rule is (route, every N hours, from "HH:MM", to "HH:MM").
pub fn schedule_save(user_root: &Path, site: &str, rules: &[(String, u32, String, String)]) -> Result<(), String> {
    let mut list = Vec::new();
    for (route, h, a, b) in rules {
        let from = siterec::parse_clock(a).ok_or("A start time could not be read. Use hours and minutes, like 20:00.")?;
        let to = siterec::parse_clock(b).ok_or("An end time could not be read. Use hours and minutes, like 06:00.")?;
        list.push(siterec::PatrolRule { site: site.to_string(), route: route.trim().to_string(), every_hours: *h, from, to });
    }
    siterec::save_schedule(user_root, site, &list)
}
/// Communications gear (with its test and failure history) and the Inventory lines that could be listed as gear.
pub fn equipment_list(user_root: &Path) -> Result<String, String> {
    let log = equip::load(user_root)?;
    let inv = crate::userdata::UserDir::new(user_root).load_inventory()?;
    let gear = equip::gear(&log.items, &inv);
    let rows: Vec<String> = gear.iter().map(|g| format!("{{\"uid\":{},\"name\":{},\"in_inventory\":{},\"kind\":{},\"site\":{},\"test_days\":{},\"last_test\":{},\"faults\":{},\"open_fault\":{},\"retired\":{}}}",
        q(&g.uid), q(&g.name), g.in_inventory, q(&g.kind), q(&g.site), g.test_days, q(&g.last_test.map(|t| security::format_instant(t, 0)).unwrap_or_default()), g.faults, q(g.open_fault.as_deref().unwrap_or("")), g.retired)).collect();
    let listed: Vec<&str> = gear.iter().map(|g| g.uid.as_str()).collect();
    let choices: Vec<String> = inv.iter().filter(|i| !i.uid.is_empty() && !listed.contains(&i.uid.as_str())).map(|i| format!("{{\"uid\":{},\"name\":{},\"location\":{}}}", q(&i.uid), q(&i.name), q(&i.location))).collect();
    let kinds: Vec<String> = equip::KINDS.iter().map(|k| format!("{{\"id\":{},\"label\":{}}}", q(k.0), q(k.1))).collect();
    Ok(format!("{{\"gear\":[{}],\"choices\":[{}],\"kinds\":[{}],\"unreadable_lines\":{}}}", rows.join(","), choices.join(","), kinds.join(","), log.rejected.len()))
}
#[allow(clippy::too_many_arguments)]
pub fn equipment_add(user_root: &Path, uid: &str, ev: &str, kind: &str, site: &str, test_days: u32, by: &str, note: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let inv = crate::userdata::UserDir::new(user_root).load_inventory()?;
    equip::add(user_root, &inv, uid, ev, kind, site, test_days, by, note, off, now_minutes())
}
pub fn routes_save(user_root: &Path, site: &str, list: &[siterec::Route]) -> Result<(), String> {
    let fixed: Vec<siterec::Route> = list.iter().map(|r| siterec::Route { site: site.to_string(), ..r.clone() }).collect();
    siterec::save_routes(user_root, site, &fixed)
}

/// Who is unavailable now, and the newest few availability lines.
pub fn availability_list(user_root: &Path, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    availability_list_at(user_root, off, now_minutes())
}
pub fn availability_list_at(user_root: &Path, off: i64, now: i64) -> Result<String, String> {
    let _ = off;
    let log = siterec::load_availability(user_root)?;
    let row = |a: &siterec::Availability| format!("{{\"id\":{},\"person\":{},\"status\":{},\"from\":{},\"until\":{},\"reason\":{},\"by\":{}}}", q(&a.id), q(&a.person), q(&a.status), q(&a.from), q(&a.until), q(&a.reason), q(&a.by));
    let now_rows: Vec<String> = siterec::unavailable_now(&log.items, now).iter().map(|a| row(a)).collect();
    let recent: Vec<String> = log.items.iter().rev().take(40).map(|a| row(a)).collect();
    Ok(format!("{{\"unavailable\":[{}],\"recent\":[{}],\"unreadable_lines\":{}}}", now_rows.join(","), recent.join(","), log.rejected.len()))
}
/// `until_local` is a time typed on the screen, or empty for "until I say otherwise".
pub fn availability_set(user_root: &Path, person: &str, status: &str, until_local: &str, reason: &str, by: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let until = if until_local.trim().is_empty() { 0 } else { security::parse_instant(&local_instant(until_local, off)?).ok_or("The end time could not be read.")? };
    siterec::set_availability(user_root, person, status, until, reason, by, off, now_minutes())
}

pub fn plans_list(user_root: &Path) -> Result<String, String> {
    let rows: Vec<String> = plans::list(user_root).iter().map(|p| format!("{{\"id\":{},\"title\":{},\"current\":{},\"revisions\":[{}]}}", q(&p.id), q(&p.title), p.current, p.revisions.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(","))).collect();
    Ok(format!("{{\"plans\":[{}]}}", rows.join(",")))
}
pub fn plan_read(user_root: &Path, id: &str, revision: u32) -> Result<String, String> {
    let n = if revision == 0 { *plans::revisions(user_root, id).last().ok_or("That plan could not be found.")? } else { revision };
    Ok(format!("{{\"id\":{},\"revision\":{},\"text\":{}}}", q(id), n, q(&plans::read_revision(user_root, id, n)?)))
}
pub fn plan_save(user_root: &Path, id: &str, text: &str) -> Result<u32, String> {
    plans::save(user_root, id, text)
}
pub fn plan_activate(user_root: &Path, inc: &str, plan: &str, revision: u32, by: &str, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    plans::activate(user_root, inc, plan, revision, by, off, now_minutes())
}

pub fn weather_list() -> String {
    let rows: Vec<String> = comms::NOAA_STATIONS.iter().map(|s| format!("{{\"call_sign\":{},\"mhz\":{},\"place\":{}}}", q(s.0), q(s.1), q(s.2))).collect();
    format!("{{\"stations\":[{}],\"note\":{}}}", rows.join(","), q(comms::NOAA_TENNESSEE_NOTE))
}
pub fn weather_set(user_root: &Path, site: &str, call_sign: &str) -> Result<(), String> {
    comms::set_weather(user_root, site, call_sign)
}

/// Write the readable copy of the records. Answers with where it went (inside the user folder).
pub fn export_records(user_root: &Path, offset_minutes: i64) -> Result<String, String> {
    let off = offset_ok(offset_minutes)?;
    let p = crate::security_export::export(user_root, off, now_minutes())?;
    Ok(p.file_name().map(|n| format!("exports/{}", n.to_string_lossy())).unwrap_or_default())
}

/// The kept GPS tracks from phones, newest first, with distance, climb and time worked out (core/src/tracksheet.rs).
pub fn tracks_list(user_root: &Path) -> Result<String, String> {
    let n = |o: Option<f64>| o.map(|v| format!("{:.1}", v)).unwrap_or_else(|| "null".to_string());
    let rows: Vec<String> = crate::tracksheet::list(user_root)
        .iter()
        .map(|t| {
            format!(
                "{{\"id\":{},\"person\":{},\"device\":{},\"created\":{},\"outing\":{},\"points\":{},\"metres\":{:.0},\"climb\":{:.0},\"drop\":{:.0},\"low\":{},\"high\":{},\"seconds\":{},\"start\":{}}}",
                q(&t.id), q(&t.person), q(&t.device), q(&t.created), q(&t.outing), t.stats.points, t.stats.metres, t.stats.climb, t.stats.drop, n(t.stats.low), n(t.stats.high), t.stats.seconds, t.stats.start
            )
        })
        .collect();
    Ok(format!("{{\"tracks\":[{}]}}", rows.join(",")))
}

/// Write one kept track as a GPX file in `user/exports/` and say where (a plain copy: the lock does not cover exports).
pub fn track_export(user_root: &Path, id: &str) -> Result<String, String> {
    let sheet = crate::tracksheet::get(user_root, id)?;
    let dir = user_root.join("exports");
    std::fs::create_dir_all(&dir).map_err(|_| "The exports folder could not be made.".to_string())?;
    let name = format!("track-{}.gpx", sheet.id);
    crate::fsio::write_atomic(&dir.join(&name), crate::tracksheet::gpx(&sheet).as_bytes()).map_err(|e| format!("The file could not be saved: {}", e))?;
    Ok(format!("exports/{}", name))
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
        assert!(j.contains("\"needs_a_look\":0") && j.contains("\"looks\":[]") && j.contains("\"alerts\":[]"));
        // a site with no watchbill at all is not flagged for gaps; one with a watch and a hole is
        let day = now_minutes().div_euclid(1440) * 1440; // a watch inside today, whatever time the test runs
        watch_add(&r, "home", "Dad", "", &local_for(day + 60), &local_for(day + 120), 0).unwrap();
        let j = overview(&r, 0).unwrap();
        assert!(j.contains("Nobody is covering"));
        assert!(j.contains("\"sites_on_watch\":0"));
        // every look is also an alert with a steady key, so the screen announces it once and not at every refresh
        assert!(j.contains("\"alerts\":[{\"key\":\"look|home|") && j.contains("\"urgent\":false"));
        assert_eq!(overview(&r, 0).unwrap().split("\"alerts\":").nth(1), j.split("\"alerts\":").nth(1));
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
        assert_eq!(pocket_accept(&r, "n10-0001.txt", false).unwrap(), 1);
        assert!(overview(&r, 0).unwrap().contains("\"pocket_waiting\":0"));
        assert!(incident_list(&r).unwrap().contains("\"source\":\"reported\""));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn outings_show_up_in_needs_a_look_and_the_overview_count_agrees() {
        let r = tmp("outing");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        posture_set(&r, "camp", "normal", "Sam", "Walked the line", 0, -300).unwrap();
        let id = outing_start(&r, "camp", "outing", "Sam, Dana", "Water run", "2099-01-01T12:00", "2099-01-01T11:00", "radio", "Base", -300).unwrap();
        assert_eq!(id, "o1");
        let j = overview_at(&r, -300, now_minutes()).unwrap();
        assert!(j.contains("\"out_now\":1"));
        // in the far future they are very late: both the list and the overview say so, from the same lines
        let far = security::parse_instant("2099-01-01T20:00-00:00").or_else(|| security::parse_instant("2099-01-01T20:00+00:00")).unwrap();
        let ov = overview_at(&r, 0, far).unwrap();
        let ls = outing_list_at(&r, "camp", 0, far).unwrap();
        assert!(ov.contains("overdue back") && ls.contains("overdue back"));
        let n: usize = ov.split("\"needs_a_look\":").nth(1).unwrap().split(',').next().unwrap().parse().unwrap();
        let sites_part = ov.split("\"alerts\":").next().unwrap(); // the alerts repeat the same words
        let looks = sites_part.matches("overdue back").count() + sites_part.matches("late radio").count();
        assert!(n >= looks && looks >= 1);
        // a person says they are back: the look goes away
        outing_event(&r, "o1", "return", "", "", "Back with water", "", "", "Base", -300).unwrap();
        let ov = overview_at(&r, 0, far).unwrap();
        assert!(!ov.contains("overdue back") && ov.contains("\"out_now\":0"));
        assert!(security::parse_instant("2099-01-01T20:00Z").is_some());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn patrol_checkpoints_and_last_patrol_come_through_the_list() {
        let r = tmp("patrol");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        let id = outing_start(&r, "camp", "patrol", "Sam, Dana", "Perimeter loop", "2099-01-01T12:00", "", "radio", "Base", 0).unwrap();
        outing_event(&r, &id, "checkpoint", "North gate", "ok", "All quiet", "", "", "Base", 0).unwrap();
        assert!(outing_event(&r, &id, "checkpoint", "", "ok", "", "", "", "Base", 0).is_err());
        assert!(outing_event(&r, &id, "out", "", "", "", "", "", "Base", 0).is_err());
        let ls = outing_list_at(&r, "camp", 0, now_minutes()).unwrap();
        assert!(ls.contains("North gate") && ls.contains("\"patrol\":true") && ls.contains("\"last_patrol\":\"\""));
        outing_event(&r, &id, "return", "", "", "", "", "", "Base", 0).unwrap();
        let ls = outing_list_at(&r, "camp", 0, now_minutes()).unwrap();
        assert!(!ls.contains("\"last_patrol\":\"\"") && ls.contains("\"state\":\"back\""));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn communications_records_feed_needs_a_look_and_the_primary_channel_shows() {
        let r = tmp("comms");
        site_save(&r, "", "Camp", "", "", "").unwrap();
        let j = overview_at(&r, -300, now_minutes()).unwrap();
        assert!(j.contains("\"primary_channel\":\"CH 01\""));
        comms_add(&r, "camp", "equipment-failure", "Base", "Radio 3 will not charge", "Swapped to radio 4", "open", "", "Bravo", -300).unwrap();
        coverage_add(&r, "camp", "North boundary", "1", "2", "", "", "Sam", "Dead in the gully", -300).unwrap();
        assert!(coverage_add(&r, "camp", "Creek", "7", "", "", "", "Sam", "", -300).is_err());
        let j = overview_at(&r, -300, now_minutes()).unwrap();
        assert!(j.contains("Radio 3 will not charge") && j.contains("Radio is weak at North boundary"));
        comms_add(&r, "camp", "equipment-failure", "Base", "New cable", "", "closed", "c1", "Bravo", -300).unwrap();
        let j = overview_at(&r, -300, now_minutes()).unwrap();
        assert!(!j.contains("Radio 3 will not charge"));
        let l = comms_list(&r, "camp").unwrap();
        assert!(l.contains("\"open\":[]") && l.contains("Watch turnover"));
        // a site's own channel plan replaces the default
        let mut ch = comms::default_channels("camp");
        ch[0].channel = "07".into();
        channels_save(&r, "camp", &ch).unwrap();
        assert!(overview_at(&r, -300, now_minutes()).unwrap().contains("\"primary_channel\":\"CH 07\""));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn training_standing_comes_through_the_list() {
        let r = tmp("training");
        for sk in 1..=12 { training_record(&r, "Dana", sk, "pass", "Sam", "", -300).unwrap(); }
        training_record(&r, "Kim", 1, "practice", "Sam", "Fumbled the privacy codes", -300).unwrap();
        assert!(training_record(&r, "Kim", 2, "pass", "kim", "", -300).is_err());
        let l = training_list(&r).unwrap();
        assert!(l.contains("\"person\":\"Dana\"") && l.contains("\"qualified\":true") && l.contains("\"qualified\":false"));
        assert!(l.contains("Turn the radio on and off") && l.contains("null"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn security_tab_looks_reach_the_overview_and_availability_and_plans_work_through_the_api() {
        let r = std::env::temp_dir().join(format!("cindrix-api2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        site_save(&r, "", "Camp", "", "", "").unwrap();
        let sid = store::load_sites(&r).unwrap().items[0].id.clone();
        let now = 29_000_000;
        let a = asset_save(&r, &sid, "", "alarm", "Trip line", 7, "").unwrap();
        let ov = overview_at(&r, 0, now).unwrap();
        assert!(ov.contains("never been inspected"), "{}", ov);
        let l = assets_list_at(&r, &sid, 0, now).unwrap();
        assert!(l.contains("\"due\":\"never\""));
        asset_inspect(&r, &a, "needs-work", "Sam", "Wire slack", 0).unwrap();
        assert!(assets_list_at(&r, &sid, 0, now_minutes()).unwrap().contains("\"due\":\"needs-work\""));
        assert!(equipment_list(&r).unwrap().contains("\"gear\":[]"));
        assert!(equipment_add(&r, "nope", "listed", "radio", "", 0, "Dana", "", 0).is_err());
        assert!(routes_get(&r, "nowhere").is_err());
        schedule_save(&r, &sid, &[(String::new(), 4, "00:00".into(), "23:59".into())]).unwrap();
        assert!(schedule_get(&r, &sid).unwrap().contains("\"every_hours\":4"));
        assert!(schedule_save(&r, &sid, &[(String::new(), 4, "25:00".into(), "23:59".into())]).is_err());
        assert!(overview_at(&r, 0, now_minutes()).unwrap().contains("none is on record"));
        routes_save(&r, &sid, &[siterec::Route { site: String::new(), name: "Loop".into(), checkpoints: vec!["Gate".into(), "Creek".into()] }]).unwrap();
        assert!(routes_get(&r, &sid).unwrap().contains("\"Creek\""));
        availability_set(&r, "Kim", "unavailable", "", "Sick", "Dana", 0).unwrap();
        assert!(availability_list_at(&r, 0, now_minutes()).unwrap().contains("\"unavailable\":[{"));
        assert!(availability_set(&r, "Kim", "unavailable", "garbage", "Sick", "Dana", 0).is_err());
        assert_eq!(plan_save(&r, "evac", "# Evacuate\nGo.\n").unwrap(), 1);
        assert!(plans_list(&r).unwrap().contains("Evacuate"));
        assert!(plan_read(&r, "evac", 0).unwrap().contains("\"revision\":1"));
        assert!(weather_list().contains("KWN35"));
        weather_set(&r, &sid, "WXJ85").unwrap();
        assert!(channels_get(&r, &sid).unwrap().contains("\"weather\""));
        let f = export_records(&r, 0).unwrap();
        assert!(f.starts_with("exports/security-records-"));
        let _ = std::fs::remove_dir_all(&r);
    }

    #[test]
    fn kept_tracks_are_listed_and_can_be_copied_out_as_gpx() {
        let r = tmp("tracks");
        assert!(tracks_list(&r).unwrap().contains("\"tracks\":[]"));
        let pts: Vec<crate::tracksheet::Point> = (0..3).map(|i| crate::tracksheet::Point { t: 100 + i * 30, lat: 38.0 + i as f64 * 0.001, lon: -80.0, ele: Some(500.0 + i as f64 * 5.0) }).collect();
        crate::security_pocket::drop_file(&r, "n10-track-1.txt", &crate::tracksheet::render("n10", "Sam", &crate::security::format_instant(now_minutes(), 0), "n10-t1", "", &pts)).unwrap();
        crate::security_pocket::accept_any(&r, "n10-track-1.txt", false).unwrap();
        let j = tracks_list(&r).unwrap();
        assert!(j.contains("\"id\":\"n10-t1\"") && j.contains("\"metres\":222") && j.contains("\"climb\":10") && j.contains("\"seconds\":60"), "{}", j);
        assert_eq!(track_export(&r, "n10-t1").unwrap(), "exports/track-n10-t1.gpx");
        assert!(fs::read_to_string(r.join("exports/track-n10-t1.gpx")).unwrap().contains("<trkpt"));
        assert!(track_export(&r, "../x").is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
