//! Radios, batteries and chargers the security plan depends on (plan COPS-COM-001 section 10). They stay ordinary
//! Inventory items; this log only says which ones are communications gear, when each was tested and when each failed.
//! Nothing in the Inventory is changed here. Append-only: a fault is answered by a later "repaired" line, never an edit,
//! so the failure history stays. A log that cannot be read is never added to.

use crate::inventory::Item;
use crate::ops::{esc, unesc};
use crate::security::{format_instant, parse_instant, valid_id, Parsed};
use crate::security_comms::{clean, read_text, rows};
use crate::security_store as store;
use std::path::{Path, PathBuf};

pub const KINDS: &[(&str, &str)] = &[("radio", "Radio"), ("battery", "Battery or power bank"), ("charger", "Charger or solar panel"), ("other", "Other gear")];
const EVENTS: &[&str] = &["listed", "test", "fault", "repaired", "charged", "retired"];

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub id: String,
    /// The Inventory line's uid.
    pub uid: String,
    pub at: String,
    pub by: String,
    pub ev: String,
    /// listed: the kind. Others: empty.
    pub kind: String,
    /// listed: the site it belongs to (empty = all). Others: empty.
    pub site: String,
    /// listed: test it every N days (0 = never). Others: empty.
    pub test_days: String,
    pub note: String,
}

pub fn path(r: &Path) -> PathBuf {
    store::dir(r).join("equipment-log.txt")
}
pub fn parse(text: &str) -> Result<Parsed<Event>, String> {
    let (ok, mut rejected) = rows(text, "equipment-log", 9)?;
    let mut items: Vec<Event> = Vec::new();
    for f in ok {
        let e = Event { id: f[0].clone(), uid: f[1].clone(), at: f[2].clone(), by: unesc(&f[3]), ev: f[4].clone(), kind: f[5].clone(), site: f[6].clone(), test_days: f[7].clone(), note: unesc(&f[8]) };
        let good = !e.uid.is_empty() && parse_instant(&e.at).is_some() && EVENTS.contains(&e.ev.as_str()) && !items.iter().any(|x| x.id == e.id)
            && (e.ev != "listed" || (KINDS.iter().any(|k| k.0 == e.kind) && e.test_days.parse::<u32>().is_ok()));
        if good { items.push(e) } else { rejected.push(f[0].clone()) }
    }
    Ok(Parsed { items, rejected })
}
pub fn load(r: &Path) -> Result<Parsed<Event>, String> {
    parse(&read_text(&path(r))?)
}

/// What the log says about one piece of gear, joined with its Inventory line.
#[derive(Debug, Clone, PartialEq)]
pub struct Gear {
    pub uid: String,
    pub name: String,
    pub in_inventory: bool,
    pub kind: String,
    pub site: String,
    pub test_days: u32,
    pub last_test: Option<i64>,
    pub faults: usize,
    /// Some(note) while the newest fault has no later "repaired".
    pub open_fault: Option<String>,
    pub retired: bool,
}

pub fn gear(events: &[Event], inv: &[Item]) -> Vec<Gear> {
    let mut uids: Vec<&str> = Vec::new();
    for e in events { if e.ev == "listed" && !uids.contains(&e.uid.as_str()) { uids.push(e.uid.as_str()) } }
    uids.into_iter().map(|uid| {
        let mine: Vec<&Event> = events.iter().filter(|e| e.uid == uid).collect();
        let listed = mine.iter().rev().find(|e| e.ev == "listed").unwrap();
        let item = inv.iter().find(|i| i.uid == uid);
        let t = |e: &&&Event| parse_instant(&e.at).unwrap_or(0);
        let last_fault = mine.iter().filter(|e| e.ev == "fault").max_by_key(t);
        let last_fix = mine.iter().filter(|e| e.ev == "repaired").max_by_key(t);
        let open = last_fault.filter(|f| last_fix.map(|x| t(&x) < t(&f)).unwrap_or(true)).map(|f| f.note.clone());
        Gear {
            uid: uid.to_string(),
            name: item.map(|i| i.name.clone()).unwrap_or_else(|| "(not in the Inventory)".to_string()),
            in_inventory: item.is_some(),
            kind: listed.kind.clone(),
            site: listed.site.clone(),
            test_days: listed.test_days.parse().unwrap_or(0),
            last_test: mine.iter().filter(|e| e.ev == "test" || e.ev == "repaired").max_by_key(t).and_then(|e| parse_instant(&e.at)),
            faults: mine.iter().filter(|e| e.ev == "fault").count(),
            open_fault: open,
            retired: mine.last().map(|e| e.ev == "retired").unwrap_or(false),
        }
    }).collect()
}

/// Plain-words lines for one site (gear with no site belongs to every site).
pub fn looks(site: &str, gear: &[Gear], now: i64) -> Vec<String> {
    let mut out = Vec::new();
    for g in gear.iter().filter(|g| !g.retired && (g.site.is_empty() || g.site == site)) {
        if let Some(n) = &g.open_fault { out.push(format!("{} has a fault: {}", g.name, n)); }
        if !g.in_inventory { out.push(format!("A listed piece of gear is no longer in the Inventory (it was {}).", g.kind)); continue; }
        if g.test_days > 0 {
            match g.last_test {
                None => out.push(format!("{} has never been tested (it is meant to be every {} days).", g.name, g.test_days)),
                Some(t) if now > t + g.test_days as i64 * 1440 => out.push(format!("{} is overdue for a test (last tested {} days ago, meant to be every {}).", g.name, (now - t) / 1440, g.test_days)),
                _ => {}
            }
        }
    }
    out
}

/// Add a line. `uid` must be an Inventory line (or one already in the log). Returns the line's number.
#[allow(clippy::too_many_arguments)]
pub fn add(r: &Path, inv: &[Item], uid: &str, ev: &str, kind: &str, site: &str, test_days: u32, by: &str, note: &str, off: i64, now: i64) -> Result<String, String> {
    if !EVENTS.contains(&ev) { return Err("That is not something that can happen to gear.".to_string()); }
    let log = load(r)?;
    if !log.rejected.is_empty() { return Err("Some lines in the equipment log could not be read, so nothing was added. Look at those first.".to_string()); }
    let known = log.items.iter().any(|e| e.uid == uid && e.ev == "listed");
    if ev == "listed" {
        if !inv.iter().any(|i| i.uid == uid) { return Err("That is not on the Inventory list.".to_string()); }
        if !KINDS.iter().any(|k| k.0 == kind) { return Err("Pick radio, battery, charger or other.".to_string()); }
        if !site.is_empty() && (!valid_id(site) || !store::load_sites(r)?.items.iter().any(|s| s.id == site)) { return Err("That site is not on the list.".to_string()); }
        if test_days > 3650 { return Err("That is a very long time between tests.".to_string()); }
    } else if !known {
        return Err("List it as communications gear first.".to_string());
    }
    let by = clean(by, 80);
    if by.is_empty() { return Err("Say who is writing this.".to_string()); }
    let note = clean(note, 300);
    if ev == "fault" && note.is_empty() { return Err("Say what went wrong.".to_string()); }
    let at = format_instant(now, off);
    let id = crate::ops::next_id('q', log.items.iter().map(|x| x.id.clone()));
    let line = if ev == "listed" { format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", id, uid, at, esc(&by), ev, kind, site, test_days, esc(&note)) }
        else { format!("{}\t{}\t{}\t{}\t{}\t\t\t\t{}\n", id, uid, at, esc(&by), ev, esc(&note)) };
    store::append_line(r, &path(r), "# format: equipment-log 1\n# id, Inventory uid, time, by, event (listed, test, fault, repaired, charged, retired), kind, site, test every N days, note (never changed)\n", &line, |t| parse(t).map(|_| ()))?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn item(uid: &str, name: &str) -> Item {
        Item { uid: uid.into(), name: name.into(), ..Default::default() }
    }
    fn tmp(n: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-eq-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        store::save_sites(&p, &[crate::security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        p
    }
    const NOW: i64 = 29_000_000;

    #[test]
    fn gear_is_tested_on_time_and_a_fault_stays_until_a_later_repair_and_the_history_stays() {
        let r = tmp("a");
        let inv = vec![item("u1", "Baofeng handheld")];
        assert!(add(&r, &inv, "u1", "test", "", "", 0, "Sam", "", 0, NOW).unwrap_err().contains("List it"));
        add(&r, &inv, "u1", "listed", "radio", "camp", 30, "Dana", "", 0, NOW - 40 * 1440).unwrap();
        let g = gear(&load(&r).unwrap().items, &inv);
        assert!(looks("camp", &g, NOW)[0].contains("never been tested"));
        add(&r, &inv, "u1", "test", "", "", 0, "Sam", "Heard CH 01 at the gate", 0, NOW - 5 * 1440).unwrap();
        assert!(looks("camp", &gear(&load(&r).unwrap().items, &inv), NOW).is_empty());
        assert!(looks("camp", &gear(&load(&r).unwrap().items, &inv), NOW + 30 * 1440)[0].contains("overdue"));
        assert!(add(&r, &inv, "u1", "fault", "", "", 0, "Sam", "", 0, NOW).unwrap_err().contains("what went wrong"));
        add(&r, &inv, "u1", "fault", "", "", 0, "Sam", "Battery will not hold a charge", 0, NOW).unwrap();
        let g = gear(&load(&r).unwrap().items, &inv);
        assert_eq!((g[0].faults, g[0].open_fault.clone()), (1, Some("Battery will not hold a charge".to_string())));
        assert!(looks("camp", &g, NOW)[0].contains("has a fault"));
        add(&r, &inv, "u1", "repaired", "", "", 0, "Dana", "New battery", 0, NOW + 10).unwrap();
        let g = gear(&load(&r).unwrap().items, &inv);
        assert_eq!((g[0].faults, g[0].open_fault.clone()), (1, None), "the failure is still counted");
        assert!(looks("camp", &g, NOW + 10).is_empty());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn gear_belongs_to_a_site_or_all_and_retired_or_missing_gear_is_handled() {
        let r = tmp("b");
        let inv = vec![item("u1", "Radio"), item("u2", "Power bank")];
        add(&r, &inv, "u1", "listed", "radio", "camp", 0, "Dana", "", 0, NOW).unwrap();
        add(&r, &inv, "u2", "listed", "battery", "", 0, "Dana", "", 0, NOW).unwrap();
        let g = gear(&load(&r).unwrap().items, &inv);
        assert!(looks("home", &g, NOW).is_empty());
        // gone from the Inventory
        let g2 = gear(&load(&r).unwrap().items, &[item("u2", "Power bank")]);
        assert!(looks("camp", &g2, NOW)[0].contains("no longer in the Inventory"));
        add(&r, &inv, "u1", "retired", "", "", 0, "Dana", "Broke", 0, NOW + 1).unwrap();
        assert!(looks("camp", &gear(&load(&r).unwrap().items, &[item("u2", "Power bank")]), NOW + 2).is_empty());
        assert!(add(&r, &inv, "zz", "listed", "radio", "", 0, "Dana", "", 0, NOW).is_err());
        assert!(add(&r, &inv, "u1", "listed", "toaster", "", 0, "Dana", "", 0, NOW).is_err());
        assert!(add(&r, &inv, "u1", "listed", "radio", "nowhere", 0, "Dana", "", 0, NOW).is_err());
        fs::write(path(&r), "# format: equipment-log 1\nbad\n").unwrap();
        assert!(add(&r, &inv, "u2", "test", "", "", 0, "Dana", "", 0, NOW).unwrap_err().contains("could not be read"));
        let _ = fs::remove_dir_all(&r);
    }
}
