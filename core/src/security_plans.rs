//! Security plans (docs/44 section 6): written plans kept as plain files with a copy for every revision, and the step
//! that pins one revision to an incident.
//!
//! `user/security/plans/<id>-r<n>.md`: a revision is never changed or removed. Saving a plan makes the next revision;
//! the newest is "current". Activating a plan in an incident saves a copy of the exact text beside the incident log
//! (through `security_store::activate_plan`), so later edits to the plan can never change what was in force.

use crate::fsio;
use crate::security::{format_instant, valid_id};
use crate::security_incident as incident;
use crate::security_store as store;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_TEXT: usize = 60_000;

pub fn dir(r: &Path) -> PathBuf {
    store::dir(r).join("plans")
}
fn rev_path(r: &Path, id: &str, n: u32) -> PathBuf {
    dir(r).join(format!("{}-r{}.md", id, n))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanInfo {
    pub id: String,
    pub title: String,
    pub current: u32,
    pub revisions: Vec<u32>,
}

/// Revision numbers of one plan, oldest first.
pub fn revisions(r: &Path, id: &str) -> Vec<u32> {
    let mut v: Vec<u32> = Vec::new();
    if let Ok(rd) = fs::read_dir(dir(r)) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".md") {
                if let Some((pid, n)) = stem.rsplit_once("-r") {
                    if pid == id {
                        if let Ok(n) = n.parse::<u32>() { if n >= 1 { v.push(n) } }
                    }
                }
            }
        }
    }
    v.sort();
    v
}

pub fn title_of(text: &str) -> String {
    text.lines().map(|l| l.trim().trim_start_matches('#').trim()).find(|l| !l.is_empty()).unwrap_or("").chars().take(100).collect()
}

pub fn read_revision(r: &Path, id: &str, n: u32) -> Result<String, String> {
    if !valid_id(id) { return Err("That plan name is not valid.".to_string()); }
    fs::read_to_string(rev_path(r, id, n)).map_err(|_| "That version of the plan could not be found.".to_string())
}

/// All plans, by name. A plan whose file cannot be read still shows, with an empty title.
pub fn list(r: &Path) -> Vec<PlanInfo> {
    let mut ids: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir(dir(r)) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_suffix(".md") {
                if let Some((pid, n)) = stem.rsplit_once("-r") {
                    if valid_id(pid) && n.parse::<u32>().is_ok() && !ids.contains(&pid.to_string()) { ids.push(pid.to_string()) }
                }
            }
        }
    }
    ids.sort();
    ids.into_iter()
        .filter_map(|id| {
            let revs = revisions(r, &id);
            let cur = *revs.last()?;
            Some(PlanInfo { title: fs::read_to_string(rev_path(r, &id, cur)).map(|t| title_of(&t)).unwrap_or_default(), id, current: cur, revisions: revs })
        })
        .collect()
}

/// Save a plan. A new name starts at revision 1; an existing one gets the next. Text identical to the current
/// revision is refused (so a revision always means something changed). Returns the revision number.
pub fn save(r: &Path, id: &str, text: &str) -> Result<u32, String> {
    if !valid_id(id) { return Err("A plan name uses lowercase letters, numbers and dashes, like evacuation-camp.".to_string()); }
    let text = text.replace("\r\n", "\n");
    if text.trim().is_empty() { return Err("The plan is empty.".to_string()); }
    if text.len() > MAX_TEXT { return Err("That plan is too long.".to_string()); }
    let revs = revisions(r, id);
    if let Some(&last) = revs.last() {
        if fs::read_to_string(rev_path(r, id, last)).map(|t| t == text).unwrap_or(false) { return Err("Nothing has changed since the last version.".to_string()); }
    }
    let n = revs.last().copied().unwrap_or(0) + 1;
    let p = rev_path(r, id, n);
    if p.exists() { return Err("That version already exists. Nothing was changed.".to_string()); }
    fsio::write_atomic(&p, text.as_bytes()).map_err(|_| "The plan could not be saved.".to_string())?;
    Ok(n)
}

/// Pin one revision (0 = the current one) of a plan to an incident. Returns the log entry's number.
pub fn activate(r: &Path, inc: &str, plan: &str, revision: u32, by: &str, off: i64, now: i64) -> Result<String, String> {
    let revs = revisions(r, plan);
    let n = if revision == 0 { *revs.last().ok_or("That plan could not be found.")? } else { revision };
    if !revs.contains(&n) { return Err("That version of the plan could not be found.".to_string()); }
    let text = read_revision(r, plan, n)?;
    let by = by.trim();
    if by.is_empty() { return Err("Say who is putting the plan into use.".to_string()); }
    let log = store::load_incident_log(r)?.items;
    let at = format_instant(now, off);
    let e = incident::Entry {
        id: incident::next_id('e', log.iter().map(|e| e.id.clone())),
        incident: inc.to_string(),
        kind: incident::Kind::Plan,
        happened_at: at.clone(),
        recorded_at: at,
        by: by.to_string(),
        source: incident::Source::Observed,
        arg1: plan.to_string(),
        arg2: n.to_string(),
        arg3: String::new(),
        text: title_of(&text),
        supersedes: String::new(),
    };
    let id = e.id.clone();
    store::activate_plan(r, e, &text)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security_incident::fingerprint;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-plans-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        store::save_sites(&p, &[crate::security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        p
    }
    const NOW: i64 = 29_000_000;

    #[test]
    fn every_save_is_a_new_revision_and_old_ones_stay() {
        let r = tmp("rev");
        assert_eq!(save(&r, "shelter", "# Shelter in place\nStay inside.\n").unwrap(), 1);
        assert!(save(&r, "shelter", "# Shelter in place\nStay inside.\n").unwrap_err().contains("Nothing has changed"));
        assert_eq!(save(&r, "shelter", "# Shelter in place\nStay inside. Close the vents.\r\n").unwrap(), 2);
        assert_eq!(read_revision(&r, "shelter", 1).unwrap(), "# Shelter in place\nStay inside.\n");
        let l = list(&r);
        assert_eq!((l.len(), l[0].current, l[0].title.as_str(), l[0].revisions.clone()), (1, 2, "Shelter in place", vec![1, 2]));
        assert!(save(&r, "Bad Name", "x").is_err());
        assert!(save(&r, "empty", "  \n").is_err());
        assert!(read_revision(&r, "../x", 1).is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn activating_pins_the_exact_text_even_if_the_plan_changes_later() {
        let r = tmp("act");
        let inc = crate::security_api::incident_open(&r, "camp", "Smoke on the ridge", 2, false, "Dana", 0).unwrap();
        save(&r, "evac", "# Evacuate\nGo to the creek road.\n").unwrap();
        assert!(activate(&r, &inc, "evac", 0, "Dana", 0, NOW).unwrap_err().contains("active incident"));
        crate::security_api::incident_entry(&r, &inc, "status", "active", "Confirmed", "observed", "Dana", "", "", 0).unwrap();
        let e = activate(&r, &inc, "evac", 0, "Dana", 0, NOW).unwrap();
        save(&r, "evac", "# Evacuate\nGo to the other road.\n").unwrap();
        let log = store::load_incident_log(&r).unwrap().items;
        let plan = log.iter().find(|x| x.id == e).unwrap();
        assert_eq!((plan.arg1.as_str(), plan.arg2.as_str()), ("evac", "1"));
        assert_eq!(plan.arg3, fingerprint("# Evacuate\nGo to the creek road.\n"));
        let copy = fs::read_to_string(store::plan_copy_path(&r, &inc, &e)).unwrap();
        assert!(copy.contains("creek road"));
        // an older revision can be pinned on purpose
        assert!(activate(&r, &inc, "evac", 2, "Dana", 0, NOW).is_ok());
        assert!(activate(&r, &inc, "evac", 9, "Dana", 0, NOW).is_err());
        assert!(activate(&r, &inc, "nope", 0, "Dana", 0, NOW).is_err());
        assert!(activate(&r, &inc, "evac", 0, " ", 0, NOW).is_err());
        assert!(activate(&r, "i99", "evac", 0, "Dana", 0, NOW).is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
