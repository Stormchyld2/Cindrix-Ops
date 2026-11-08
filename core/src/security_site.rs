//! Site records for the Security tab (docs/44): what protects a site (zones, barriers, alarms) and how often each is
//! inspected, planned patrol routes, and who is available. Plain tab-separated files under `user/security/`.
//!
//! The asset list and the route list are small lists that are rewritten with a backup first. Inspections and
//! availability are append-only logs: a mistake or a change is a later line, nothing is edited or erased, and a log that
//! cannot be read is never added to. Nothing here lowers a posture or closes anything by itself.

use crate::fsio;
use crate::ops::{esc, unesc};
use crate::security::{check_format, format_instant, parse_instant, valid_id, Parsed};
use crate::security_comms::{clean, read_text, rows};
use crate::security_schedule::{Watch, WatchState};
use crate::security_store as store;
use std::path::{Path, PathBuf};

// ---------- assets: zones, barriers, alarms ----------

pub const KINDS: &[(&str, &str)] = &[("zone", "Zone"), ("barrier", "Barrier"), ("alarm", "Alarm or sensor"), ("other", "Other")];

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub id: String,
    pub site: String,
    pub kind: String,
    pub name: String,
    /// 0 = no regular inspection
    pub every_days: u32,
    pub notes: String,
}
pub fn assets_path(r: &Path) -> PathBuf {
    store::dir(r).join("assets.txt")
}
pub fn parse_assets(text: &str) -> Result<Parsed<Asset>, String> {
    check_format(text, "security-assets", 1)?;
    let mut p = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let a = (f.len() == 6 && valid_id(f[0]) && valid_id(f[1]) && KINDS.iter().any(|k| k.0 == f[2]) && !unesc(f[3]).trim().is_empty() && f[4].parse::<u32>().map(|d| d <= 3650).unwrap_or(false))
            .then(|| Asset { id: f[0].into(), site: f[1].into(), kind: f[2].into(), name: unesc(f[3]), every_days: f[4].parse().unwrap_or(0), notes: unesc(f[5]) });
        match a {
            Some(a) if !p.items.iter().any(|x: &Asset| x.id == a.id) => p.items.push(a),
            _ => p.rejected.push(l.to_string()),
        }
    }
    Ok(p)
}
pub fn load_assets(r: &Path) -> Result<Parsed<Asset>, String> {
    parse_assets(&read_text(&assets_path(r))?)
}
/// Add a thing (empty `id`) or change one. The code of an existing one never changes. Returns the code.
pub fn save_asset(r: &Path, id: &str, site: &str, kind: &str, name: &str, every_days: u32, notes: &str) -> Result<String, String> {
    let sites = store::load_sites(r)?;
    if !sites.items.iter().any(|s| s.id == site) { return Err("That site is not on the list.".to_string()); }
    if !KINDS.iter().any(|k| k.0 == kind) { return Err("Pick zone, barrier, alarm or other.".to_string()); }
    let name = clean(name, 80);
    if name.is_empty() { return Err("Give it a name first.".to_string()); }
    if every_days > 3650 { return Err("That is a very long time between inspections.".to_string()); }
    let mut cur = load_assets(r)?;
    if !cur.rejected.is_empty() { return Err("Some lines in the list could not be read, so nothing was changed.".to_string()); }
    let id = if id.is_empty() { crate::ops::next_id('a', cur.items.iter().map(|a| a.id.clone())) } else { id.to_string() };
    let rec = Asset { id: id.clone(), site: site.into(), kind: kind.into(), name, every_days, notes: clean(notes, 300) };
    match cur.items.iter_mut().find(|a| a.id == id) {
        Some(slot) => { if slot.site != rec.site { return Err("A thing cannot move to another site. Add it there instead.".to_string()); } *slot = rec }
        None => cur.items.push(rec),
    }
    let mut t = String::from("# format: security-assets 1\n# id, site, kind, name, inspect every N days (0 = never), notes\n");
    for a in &cur.items { t.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\n", a.id, a.site, a.kind, esc(&a.name), a.every_days, esc(&a.notes))); }
    fsio::backup_then_write(&assets_path(r), &store::dir(r).join("backups"), t.as_bytes()).map_err(|_| "The list could not be saved.".to_string())?;
    Ok(id)
}

// ---------- inspections ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Inspection {
    pub id: String,
    pub asset: String,
    pub at: String,
    pub recorded: String,
    pub by: String,
    /// ok or needs-work
    pub result: String,
    pub note: String,
}
pub fn inspections_path(r: &Path) -> PathBuf {
    store::dir(r).join("inspection-log.txt")
}
pub fn parse_inspections(text: &str) -> Result<Parsed<Inspection>, String> {
    let (ok, mut rejected) = rows(text, "inspection-log", 7)?;
    let mut items: Vec<Inspection> = Vec::new();
    for f in ok {
        let i = Inspection { id: f[0].clone(), asset: f[1].clone(), at: f[2].clone(), recorded: f[3].clone(), by: unesc(&f[4]), result: f[5].clone(), note: unesc(&f[6]) };
        if valid_id(&i.asset) && matches!(i.result.as_str(), "ok" | "needs-work") && !items.iter().any(|x| x.id == i.id) { items.push(i) } else { rejected.push(f[0].clone()) }
    }
    Ok(Parsed { items, rejected })
}
pub fn load_inspections(r: &Path) -> Result<Parsed<Inspection>, String> {
    parse_inspections(&read_text(&inspections_path(r))?)
}
pub fn inspect(r: &Path, asset: &str, result: &str, by: &str, note: &str, off: i64, now: i64) -> Result<String, String> {
    let assets = load_assets(r)?;
    if !assets.items.iter().any(|a| a.id == asset) { return Err("That is not on the list.".to_string()); }
    if !matches!(result, "ok" | "needs-work") { return Err("The result has to be ok or needs work.".to_string()); }
    let by = clean(by, 80);
    if by.is_empty() { return Err("Say who looked.".to_string()); }
    let note = clean(note, 300);
    if result == "needs-work" && note.is_empty() { return Err("Say what needs work.".to_string()); }
    let log = load_inspections(r)?;
    if !log.rejected.is_empty() { return Err("Some lines in the inspection log could not be read, so nothing was added. Look at those first.".to_string()); }
    let at = format_instant(now, off);
    let id = crate::ops::next_id('i', log.items.iter().map(|x| x.id.clone()));
    let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", id, asset, at, at, esc(&by), result, esc(&note));
    store::append_line(r, &inspections_path(r), "# format: inspection-log 1\n# id, thing, time, recorded, by, result (ok or needs-work), note (never changed; the newest line for a thing counts)\n", &line, |t| parse_inspections(t).map(|_| ()))?;
    Ok(id)
}
/// The newest inspection of one thing.
pub fn last_inspection<'a>(log: &'a [Inspection], asset: &str) -> Option<&'a Inspection> {
    log.iter().filter(|i| i.asset == asset).max_by_key(|i| parse_instant(&i.at).unwrap_or(0))
}

/// Plain-words lines for one site: things that need work, are overdue for inspection, or have never been inspected.
pub fn asset_looks(site: &str, assets: &[Asset], log: &[Inspection], now: i64) -> Vec<String> {
    let mut out = Vec::new();
    for a in assets.iter().filter(|a| a.site == site) {
        let last = last_inspection(log, &a.id);
        if let Some(l) = last.filter(|l| l.result == "needs-work") {
            out.push(format!("{} needs work: {}", a.name, l.note));
        }
        if a.every_days > 0 {
            match last.and_then(|l| parse_instant(&l.at)) {
                None => out.push(format!("{} has never been inspected (it is meant to be every {} day{}).", a.name, a.every_days, if a.every_days == 1 { "" } else { "s" })),
                Some(t) if now > t + a.every_days as i64 * 1440 => out.push(format!("{} is overdue for inspection (last looked at {} days ago, meant to be every {}).", a.name, (now - t) / 1440, a.every_days)),
                _ => {}
            }
        }
    }
    out
}

// ---------- patrol routes ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub site: String,
    pub name: String,
    pub checkpoints: Vec<String>,
}
pub fn routes_path(r: &Path) -> PathBuf {
    store::dir(r).join("routes.txt")
}
pub fn parse_routes(text: &str) -> Result<Parsed<Route>, String> {
    check_format(text, "patrol-routes", 1)?;
    let mut p = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let r = (f.len() == 3 && valid_id(f[0]) && !unesc(f[1]).trim().is_empty()).then(|| Route { site: f[0].into(), name: unesc(f[1]), checkpoints: unesc(f[2]).split('|').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect() });
        match r { Some(r) => p.items.push(r), None => p.rejected.push(l.to_string()) }
    }
    Ok(p)
}
pub fn load_routes(r: &Path) -> Result<Parsed<Route>, String> {
    parse_routes(&read_text(&routes_path(r))?)
}
/// Replace one site's routes, keeping every other site's.
pub fn save_routes(r: &Path, site: &str, list: &[Route]) -> Result<(), String> {
    if !store::load_sites(r)?.items.iter().any(|s| s.id == site) { return Err("That site is not on the list.".to_string()); }
    let cur = load_routes(r)?;
    if !cur.rejected.is_empty() { return Err("Some lines in the route list could not be read, so nothing was changed.".to_string()); }
    let mut seen: Vec<String> = Vec::new();
    for l in list {
        let n = clean(&l.name, 60).to_lowercase();
        if n.is_empty() { return Err("Every route needs a name.".to_string()); }
        if seen.contains(&n) { return Err("Two routes have the same name.".to_string()); }
        seen.push(n);
        if l.checkpoints.iter().any(|c| c.contains('|') || c.len() > 60) { return Err("A checkpoint name cannot contain | or be that long.".to_string()); }
    }
    let mut t = String::from("# format: patrol-routes 1\n# site, route name, checkpoints in order separated by |\n");
    for l in cur.items.iter().filter(|x| x.site != site).chain(list.iter().map(|x| x).filter(|_| true)) {
        let site = if l.site.is_empty() { site } else { l.site.as_str() };
        t.push_str(&format!("{}\t{}\t{}\n", site, esc(&clean(&l.name, 60)), esc(&l.checkpoints.iter().map(|c| c.trim()).filter(|c| !c.is_empty()).collect::<Vec<_>>().join("|"))));
    }
    fsio::backup_then_write(&routes_path(r), &store::dir(r).join("backups"), t.as_bytes()).map_err(|_| "The route list could not be saved.".to_string())
}

// ---------- who is available ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Availability {
    pub id: String,
    pub person: String,
    /// available or unavailable
    pub status: String,
    pub from: String,
    /// empty = until someone says otherwise
    pub until: String,
    pub reason: String,
    pub by: String,
    pub recorded: String,
}
pub fn availability_path(r: &Path) -> PathBuf {
    store::dir(r).join("availability-log.txt")
}
pub fn parse_availability(text: &str) -> Result<Parsed<Availability>, String> {
    let (ok, mut rejected) = rows(text, "availability-log", 8)?;
    let mut items: Vec<Availability> = Vec::new();
    for f in ok {
        let a = Availability { id: f[0].clone(), person: unesc(&f[1]), status: f[2].clone(), from: f[3].clone(), until: f[4].clone(), reason: unesc(&f[5]), by: unesc(&f[6]), recorded: f[7].clone() };
        if !a.person.is_empty() && matches!(a.status.as_str(), "available" | "unavailable") && parse_instant(&a.from).is_some() && (a.until.is_empty() || parse_instant(&a.until).is_some()) && !items.iter().any(|x| x.id == a.id) { items.push(a) } else { rejected.push(f[0].clone()) }
    }
    Ok(Parsed { items, rejected })
}
pub fn load_availability(r: &Path) -> Result<Parsed<Availability>, String> {
    parse_availability(&read_text(&availability_path(r))?)
}
/// Say a person is unavailable from now until a time (or until further notice), or available again.
/// `until_utc` is minutes, 0 = until further notice.
pub fn set_availability(r: &Path, person: &str, status: &str, until_utc: i64, reason: &str, by: &str, off: i64, now: i64) -> Result<String, String> {
    let person = clean(person, 80);
    if person.is_empty() { return Err("Say who.".to_string()); }
    if !matches!(status, "available" | "unavailable") { return Err("The status has to be available or unavailable.".to_string()); }
    if status == "unavailable" && until_utc != 0 && until_utc <= now { return Err("The end time has to be in the future.".to_string()); }
    let reason = clean(reason, 200);
    if status == "unavailable" && reason.is_empty() { return Err("Say why they are not available (sick, away, injured, resting).".to_string()); }
    let by = clean(by, 80);
    if by.is_empty() { return Err("Say who is writing this.".to_string()); }
    let log = load_availability(r)?;
    if !log.rejected.is_empty() { return Err("Some lines in the availability log could not be read, so nothing was added. Look at those first.".to_string()); }
    let at = format_instant(now, off);
    let until = if status == "unavailable" && until_utc != 0 { format_instant(until_utc, off) } else { String::new() };
    let id = crate::ops::next_id('v', log.items.iter().map(|x| x.id.clone()));
    let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", id, esc(&person), status, at, until, esc(&reason), esc(&by), at);
    store::append_line(r, &availability_path(r), "# format: availability-log 1\n# id, person, status, from, until (empty = until said otherwise), reason, by, recorded (never changed; the newest line for a person counts)\n", &line, |t| parse_availability(t).map(|_| ()))?;
    Ok(id)
}
/// Who is unavailable at `now`: the newest line for each person, if it says unavailable and has not run out.
pub fn unavailable_now(all: &[Availability], now: i64) -> Vec<&Availability> {
    let mut people: Vec<&str> = Vec::new();
    for a in all { if !people.iter().any(|p| p.eq_ignore_ascii_case(&a.person)) { people.push(a.person.as_str()) } }
    people.into_iter().filter_map(|p| all.iter().rev().find(|a| a.person.eq_ignore_ascii_case(p))).filter(|a| a.status == "unavailable" && a.until.is_empty() || (a.status == "unavailable" && parse_instant(&a.until).map(|u| now < u).unwrap_or(false))).collect()
}
/// Watches (scheduled or under way) that fall in time someone has marked unavailable. A person who has marked themselves
/// available again, or whose time has run out, is not mentioned.
pub fn availability_looks(site: &str, watches: &[Watch], all: &[Availability], now: i64) -> Vec<String> {
    let mut out = Vec::new();
    for u in unavailable_now(all, now) {
        let from = parse_instant(&u.from).unwrap_or(now);
        let until = parse_instant(&u.until).unwrap_or(i64::MAX);
        for w in watches.iter().filter(|w| w.site == site && w.person.eq_ignore_ascii_case(&u.person) && matches!(w.state, WatchState::Scheduled | WatchState::Acknowledged | WatchState::OnDuty)) {
            if let Some((s, e)) = w.span() {
                if e > now && s < until && e > from {
                    out.push(format!("{} is on the watchbill while marked unavailable ({}).", w.person, u.reason));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-site-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        store::save_sites(&p, &[crate::security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        p
    }
    const NOW: i64 = 29_000_000; // some minute count

    #[test]
    fn things_that_protect_a_site_keep_their_code_and_must_stay_at_their_site() {
        let r = tmp("assets");
        let id = save_asset(&r, "", "camp", "barrier", "North gate", 7, "Chain and lock").unwrap();
        assert_eq!(id, "a1");
        save_asset(&r, &id, "camp", "barrier", "North gate (new lock)", 7, "").unwrap();
        let a = load_assets(&r).unwrap().items;
        assert_eq!((a.len(), a[0].id.as_str(), a[0].name.as_str()), (1, "a1", "North gate (new lock)"));
        assert!(save_asset(&r, "", "nowhere", "zone", "x", 0, "").is_err());
        assert!(save_asset(&r, "", "camp", "moat", "x", 0, "").is_err());
        assert!(save_asset(&r, "", "camp", "zone", "  ", 0, "").is_err());
        store::save_sites(&r, &[crate::security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }, crate::security::Site { id: "home".into(), name: "Home".into(), ..Default::default() }]).unwrap();
        assert!(save_asset(&r, "a1", "home", "barrier", "North gate", 7, "").unwrap_err().contains("another site"));
        fs::write(assets_path(&r), "# format: security-assets 3\n").unwrap();
        assert!(save_asset(&r, "", "camp", "zone", "x", 0, "").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn inspections_flag_overdue_never_inspected_and_needs_work_until_a_later_ok() {
        let r = tmp("inspect");
        save_asset(&r, "", "camp", "alarm", "Trip line, south", 7, "").unwrap();
        save_asset(&r, "", "camp", "zone", "Kill zone map", 0, "").unwrap();
        let assets = load_assets(&r).unwrap().items;
        let l = asset_looks("camp", &assets, &[], NOW);
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("never been inspected"));
        inspect(&r, "a1", "ok", "Sam", "", 0, NOW - 3 * 1440).unwrap();
        assert!(asset_looks("camp", &assets, &load_inspections(&r).unwrap().items, NOW).is_empty());
        let l = asset_looks("camp", &assets, &load_inspections(&r).unwrap().items, NOW + 6 * 1440);
        assert!(l[0].contains("overdue") && l[0].contains("9 days"));
        assert!(inspect(&r, "a1", "needs-work", "Sam", "", 0, NOW).unwrap_err().contains("what needs work"));
        inspect(&r, "a1", "needs-work", "Sam", "Wire is slack", 0, NOW).unwrap();
        let l = asset_looks("camp", &assets, &load_inspections(&r).unwrap().items, NOW);
        assert!(l.iter().any(|x| x.contains("needs work: Wire is slack")));
        inspect(&r, "a1", "ok", "Dana", "Tightened", 0, NOW + 10).unwrap();
        assert!(asset_looks("camp", &assets, &load_inspections(&r).unwrap().items, NOW + 10).is_empty());
        assert_eq!(load_inspections(&r).unwrap().items.len(), 3, "the old lines stay");
        assert!(inspect(&r, "a9", "ok", "Sam", "", 0, NOW).is_err());
        assert!(inspect(&r, "a1", "ok", " ", "", 0, NOW).is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_damaged_inspection_log_is_never_added_to() {
        let r = tmp("badinsp");
        save_asset(&r, "", "camp", "alarm", "Trip line", 7, "").unwrap();
        fs::write(inspections_path(&r), "# format: inspection-log 1\ngarbage\n").unwrap();
        assert!(inspect(&r, "a1", "ok", "Sam", "", 0, NOW).unwrap_err().contains("could not be read"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn routes_save_per_site_with_checkpoints_in_order() {
        let r = tmp("routes");
        let rt = |n: &str, c: &[&str]| Route { site: "camp".into(), name: n.into(), checkpoints: c.iter().map(|s| s.to_string()).collect() };
        save_routes(&r, "camp", &[rt("Perimeter loop", &["North gate", "Creek", "Barn"]), rt("Quick look", &["Gate"])]).unwrap();
        let got = load_routes(&r).unwrap().items;
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].checkpoints, vec!["North gate", "Creek", "Barn"]);
        assert!(save_routes(&r, "camp", &[rt("A", &[]), rt("a", &[])]).unwrap_err().contains("same name"));
        assert!(save_routes(&r, "camp", &[rt("", &[])]).is_err());
        assert!(save_routes(&r, "camp", &[rt("A", &["x|y"])]).is_err());
        assert!(save_routes(&r, "nowhere", &[]).is_err());
        assert_eq!(load_routes(&r).unwrap().items.len(), 2, "refused saves change nothing");
        let _ = fs::remove_dir_all(&r);
    }

    fn watch(person: &str, s: i64, e: i64, st: WatchState) -> Watch {
        Watch { id: "w1".into(), site: "camp".into(), person: person.into(), role: "watch".into(), start: format_instant(s, 0), end: format_instant(e, 0), state: st, ack_at: String::new(), entered_at: String::new(), notes: String::new() }
    }

    #[test]
    fn unavailable_people_on_the_watchbill_are_flagged_until_they_are_available_or_the_time_runs_out() {
        let r = tmp("avail");
        let w = vec![watch("Sam", NOW + 60, NOW + 420, WatchState::Scheduled), watch("Kim", NOW + 60, NOW + 420, WatchState::Scheduled), watch("Lee", NOW + 60, NOW + 420, WatchState::Proposed)];
        set_availability(&r, "Sam", "unavailable", NOW + 600, "Sick", "Dana", 0, NOW).unwrap();
        set_availability(&r, "Lee", "unavailable", 0, "Away", "Dana", 0, NOW).unwrap();
        let all = load_availability(&r).unwrap().items;
        let l = availability_looks("camp", &w, &all, NOW);
        assert_eq!(l.len(), 1, "{:?}", l);
        assert!(l[0].contains("Sam") && l[0].contains("Sick"));
        // the time runs out: no flag
        assert!(availability_looks("camp", &w, &all, NOW + 700).is_empty());
        // they say they are well again: the newest line counts
        set_availability(&r, "sam", "available", 0, "", "Dana", 0, NOW + 5).unwrap();
        let all = load_availability(&r).unwrap().items;
        assert!(availability_looks("camp", &w, &all, NOW + 6).is_empty());
        assert!(unavailable_now(&all, NOW + 6).iter().all(|a| a.person != "Sam"));
        assert_eq!(all.len(), 3, "the old lines stay");
        assert!(set_availability(&r, "Kim", "unavailable", NOW - 5, "x", "Dana", 0, NOW).unwrap_err().contains("future"));
        assert!(set_availability(&r, "Kim", "unavailable", 0, " ", "Dana", 0, NOW).unwrap_err().contains("why"));
        assert!(set_availability(&r, "Kim", "maybe", 0, "x", "Dana", 0, NOW).is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
