//! Security records written out as one plain, readable text file in `user/exports/` (docs/44). The logs themselves are
//! never changed or moved; this is a copy to print or hand over. Lines that could not be read are counted at the top and
//! left out of the copy, so a copy never claims to be complete when it is not.

use crate::security::format_instant;
use crate::security_comms as comms;
use crate::security_equipment as equip;
use crate::security_incident as incident;
use crate::security_outing as outing;
use crate::security_site as site;
use crate::security_store as store;
use crate::userdata::UserDir;
use std::path::{Path, PathBuf};

fn clock(s: &str) -> String {
    s.replace('T', " ")
}

pub fn render(r: &Path, off: i64, now: i64) -> Result<String, String> {
    let sites = store::load_sites(r)?;
    let name = |id: &str| sites.items.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_else(|| id.to_string());
    let inc = store::load_incident_log(r)?;
    let out_log = outing::load(r)?;
    let outs = outing::build(&out_log.items);
    let comm = comms::load_comms(r)?;
    let cov = comms::load_coverage(r)?;
    let insp = site::load_inspections(r)?;
    let assets = site::load_assets(r)?;
    let avail = site::load_availability(r)?;
    let eq = equip::load(r)?;
    let unreadable = inc.rejected.len() + out_log.rejected.len() + comm.rejected.len() + cov.rejected.len() + insp.rejected.len() + avail.rejected.len() + eq.rejected.len();

    let mut t = format!("SECURITY RECORDS\nWritten {}\n", clock(&format_instant(now, off)));
    if unreadable > 0 {
        t.push_str(&format!("WARNING: {} line{} in the logs could not be read and {} left out of this copy. The logs themselves were not touched.\n", unreadable, if unreadable == 1 { "" } else { "s" }, if unreadable == 1 { "is" } else { "are" }));
    }

    t.push_str("\n== INCIDENTS ==\n");
    let built = incident::build(&inc.items);
    if built.is_empty() { t.push_str("None recorded.\n"); }
    for i in &built {
        t.push_str(&format!("\n{} at {}: {} (priority {}, {}{})\nOpened {} by {}\n", i.id, name(&i.site), i.title, i.priority, i.status.word(), if i.exercise { ", exercise" } else { "" }, clock(&i.opened_at), i.opened_by));
        for p in &i.plans {
            t.push_str(&format!("  Plan in force: {} version {} (fingerprint {}) put in use by {} at {}\n", p.plan, p.revision, p.fingerprint, p.activated_by, clock(&p.at)));
        }
        for l in incident::timeline(&inc.items, &i.id) {
            let e = l.entry;
            let fixed = l.superseded_by.as_deref().map(|b| format!(" [corrected by {}]", b)).unwrap_or_default();
            t.push_str(&format!("  {} {} ({}, {}): {}{}\n", clock(&e.happened_at), e.kind.word(), e.by, e.source.word(), if e.text.is_empty() { e.arg1.clone() } else { e.text.clone() }, fixed));
        }
    }

    t.push_str("\n== OUTINGS AND PATROLS ==\n");
    if outs.is_empty() { t.push_str("None recorded.\n"); }
    for o in &outs {
        t.push_str(&format!("{} {} at {}: {} ({}) left {} due back {} - {:?}{}\n", o.id, if o.patrol { "patrol" } else { "outing" }, name(&o.site), o.people.join(", "), o.task, clock(&format_instant(o.left, off)), clock(&format_instant(o.back, off)), o.state, o.ended.map(|e| format!(", ended {}", clock(&format_instant(e, off)))).unwrap_or_default()));
        for (at, st, what) in &o.checkpoints {
            t.push_str(&format!("    {} {} {}\n", clock(&format_instant(*at, off)), st, what));
        }
    }

    t.push_str("\n== COMMUNICATIONS LOG ==\n");
    if comm.items.is_empty() { t.push_str("None recorded.\n"); }
    for c in &comm.items {
        t.push_str(&format!("{} {} {} {} [{}] {} - {} ({}){}\n", c.id, clock(&c.at), name(&c.site), c.station, c.category, c.event, c.action, c.status, if c.closes.is_empty() { String::new() } else { format!(" closes {}", c.closes) }));
    }

    t.push_str("\n== RADIO COVERAGE TESTS ==\n");
    if cov.items.is_empty() { t.push_str("None recorded.\n"); }
    let rate = |v: Option<u8>| v.map(|n| n.to_string()).unwrap_or_else(|| "-".to_string());
    for c in &cov.items {
        t.push_str(&format!("{} {} {}: primary {}/{} alternate {}/{} by {} {}\n", clock(&c.at), name(&c.site), c.place, rate(c.p_in), rate(c.p_out), rate(c.a_in), rate(c.a_out), c.by, c.note));
    }

    t.push_str("\n== INSPECTIONS ==\n");
    if insp.items.is_empty() { t.push_str("None recorded.\n"); }
    for i in &insp.items {
        let a = assets.items.iter().find(|a| a.id == i.asset);
        t.push_str(&format!("{} {}{}: {} by {} {}\n", clock(&i.at), a.map(|a| name(&a.site) + " - ").unwrap_or_default(), a.map(|a| a.name.as_str()).unwrap_or(&i.asset), i.result, i.by, i.note));
    }

    t.push_str("\n== AVAILABILITY ==\n");
    if avail.items.is_empty() { t.push_str("None recorded.\n"); }
    for a in &avail.items {
        t.push_str(&format!("{} {}: {}{} {} (written by {})\n", clock(&a.from), a.person, a.status, if a.until.is_empty() { String::new() } else { format!(" until {}", clock(&a.until)) }, a.reason, a.by));
    }
    t.push_str("\n== COMMUNICATIONS GEAR ==\n");
    if eq.items.is_empty() { t.push_str("None recorded.\n"); }
    let inv = UserDir::new(r).load_inventory().unwrap_or_default();
    for e in &eq.items {
        let nm = inv.iter().find(|i| i.uid == e.uid).map(|i| i.name.clone()).unwrap_or_else(|| e.uid.clone());
        t.push_str(&format!("{} {}: {}{}{} ({})\n", clock(&e.at), nm, e.ev, if e.kind.is_empty() { String::new() } else { format!(" as {}", e.kind) }, if e.note.is_empty() { String::new() } else { format!(" - {}", e.note) }, e.by));
    }
    Ok(t)
}

/// Write the copy to `exports/security-records-<date>.txt` (a second one the same day replaces the first; it is only a copy).
pub fn export(r: &Path, off: i64, now: i64) -> Result<PathBuf, String> {
    let text = render(r, off, now)?;
    let day = format_instant(now, off);
    UserDir::new(r).write_export(&format!("security-records-{}.txt", &day[..10]), &text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_copy_shows_every_record_in_plain_words_and_leaves_the_logs_alone() {
        let r = std::env::temp_dir().join(format!("cindrix-exp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(&r).unwrap();
        store::save_sites(&r, &[crate::security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        let now = 29_000_000;
        let inc = crate::security_api::incident_open(&r, "camp", "Smoke on the ridge", 2, false, "Dana", 0).unwrap();
        crate::security_api::incident_entry(&r, &inc, "note", "", "Smell of smoke from the north", "observed", "Dana", "", "", 0).unwrap();
        site::save_asset(&r, "", "camp", "alarm", "Trip line", 7, "").unwrap();
        site::inspect(&r, "a1", "needs-work", "Sam", "Wire slack", 0, now).unwrap();
        site::set_availability(&r, "Kim", "unavailable", 0, "Sick", "Dana", 0, now).unwrap();
        let before = fs::read(site::inspections_path(&r)).unwrap();
        let p = export(&r, 0, now).unwrap();
        let t = fs::read_to_string(&p).unwrap();
        for want in ["SECURITY RECORDS", "Smoke on the ridge", "Smell of smoke", "Camp - Trip line: needs-work by Sam Wire slack", "Kim: unavailable Sick"] {
            assert!(t.contains(want), "missing {:?} in\n{}", want, t);
        }
        assert!(!t.contains("WARNING"));
        assert!(p.starts_with(r.join("exports")));
        assert_eq!(fs::read(site::inspections_path(&r)).unwrap(), before);
        // a damaged line is counted, not hidden
        fs::write(site::availability_path(&r), "# format: availability-log 1\nbroken\n").unwrap();
        let t = render(&r, 0, now).unwrap();
        assert!(t.contains("WARNING: 1 line"));
        let _ = fs::remove_dir_all(&r);
    }
}
