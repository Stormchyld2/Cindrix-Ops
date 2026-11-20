//! Two-way copy of the Gear list between two devices (a Vault laptop and a Hub phone), by change sheets.
//!
//! The owner's rule (2026-11-19): a device sends a sheet, the other side accepts it, and a change made on both
//! sides is shown to a person. Nothing is merged silently and nothing is lost without being shown.
//!
//! A sheet is the sender's whole gear list (`# format: gear-sync 1`, `# from: <device code>`, then the usual gear
//! lines). Each side keeps, per other device, what that device's list looked like the last time the two met
//! (`user/sync/gear-<device>.txt`: id, TAB, fingerprint). With that, every piece is one of:
//!
//!  * the same on both sides: nothing to do;
//!  * changed (or new, or removed) only on the other side: it is taken;
//!  * changed only here: it stays, and goes across next time;
//!  * changed on both sides, or changed on one and removed on the other: a **conflict**, left exactly as it is here
//!    until a person picks mine or theirs.
//!
//! Piece ids are numbered on each device (g1, g2...). Two devices that each add a piece before ever meeting can
//! give the same id to different pieces; that shows as a conflict and the person picks one. Inventory is not done
//! here: it already merges by its own per-line ids (`sync.rs`).

use crate::fsio;
use crate::gear::{self, Piece};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub id: String,
    /// The piece here, or `None` when it was removed here.
    pub mine: Option<Piece>,
    /// The piece on the other device, or `None` when it was removed there.
    pub theirs: Option<Piece>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    pub from: String,
    pub same: usize,
    /// Pieces that are new or changed on the other device only (they will be taken).
    pub take: Vec<Piece>,
    /// Pieces the other device removed and that are unchanged here (they will be removed here).
    pub remove: Vec<Piece>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub taken: usize,
    pub removed: usize,
    pub kept_mine: usize,
    pub took_theirs: usize,
    /// Conflicts nobody has picked yet. They are untouched and will show again.
    pub waiting: usize,
}

fn print(p: &Piece) -> String {
    crate::security_pocket::fingerprint(gear::serialize(std::slice::from_ref(p)).as_bytes())
}

fn base_path(user_root: &Path, peer: &str) -> PathBuf {
    user_root.join("sync").join(format!("gear-{}.txt", peer))
}

fn has_base(user_root: &Path, peer: &str) -> bool {
    base_path(user_root, peer).exists()
}

/// What was agreed with `peer`: our own notes, or, on a first meeting, what the sender says it saw of us.
fn start_base(user_root: &Path, me: &str, peer: &str, seen: &[(String, String, String)]) -> BTreeMap<String, String> {
    if has_base(user_root, peer) {
        return load_base(user_root, peer);
    }
    seen.iter().filter(|(who, _, _)| who == me).map(|(_, id, h)| (id.clone(), h.clone())).collect()
}

fn seen_of_me(me: &str, seen: &[(String, String, String)]) -> BTreeMap<String, String> {
    seen.iter().filter(|(who, _, _)| who == me).map(|(_, id, h)| (id.clone(), h.clone())).collect()
}

fn load_base(user_root: &Path, peer: &str) -> BTreeMap<String, String> {
    fsio::read_to_string(&base_path(user_root, peer)).unwrap_or_default().lines().filter_map(|l| {
        let (a, b) = l.split_once('\t')?;
        Some((a.to_string(), b.to_string()))
    }).collect()
}

fn save_base(user_root: &Path, peer: &str, base: &BTreeMap<String, String>) -> Result<(), String> {
    let text: String = base.iter().map(|(k, v)| format!("{}\t{}\n", k, v)).collect();
    fsio::write_atomic(&base_path(user_root, peer), text.as_bytes()).map_err(|e| format!("The sync notes could not be saved: {}", e))
}

fn good_peer(s: &str) -> bool {
    !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

/// This device's gear list as a sheet for the other device. `device` is this device's code.
pub fn export(user_root: &Path, device: &str) -> Result<String, String> {
    if !good_peer(device) {
        return Err("This device has no usable code yet.".to_string());
    }
    let l = gear::load(user_root)?;
    if !l.rejected.is_empty() {
        return Err("Some lines in the gear list could not be read, so no sheet was made.".to_string());
    }
    let body = gear::serialize(&l.items);
    let mut head = format!("# format: gear-sync 1\n# from: {}\n", device);
    if let Ok(rd) = std::fs::read_dir(user_root.join("sync")) {
        let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        for n in names {
            if let Some(peer) = n.strip_prefix("gear-").and_then(|r| r.strip_suffix(".txt")) {
                if good_peer(peer) {
                    for (id, h) in load_base(user_root, peer) {
                        head.push_str(&format!("# seen: {}\t{}\t{}\n", peer, id, h));
                    }
                }
            }
        }
    }
    Ok(body.replacen("# format: gear 1\n", &head, 1))
}

/// Read a sheet: the sender's code and their pieces. Anything unreadable refuses the whole sheet.
pub fn parse_sheet(text: &str) -> Result<(String, Vec<Piece>), String> {
    parse_full(text).map(|(f, p, _)| (f, p))
}

/// A sheet's sender, pieces, and the `seen` notes: for each device the sender has met, what it last saw of that
/// device's pieces. They let a device that has never kept notes about the sender start from what they both agreed.
fn parse_full(text: &str) -> Result<(String, Vec<Piece>, Vec<(String, String, String)>), String> {
    let mut lines = text.lines();
    if lines.next().map(|l| l.trim()) != Some("# format: gear-sync 1") {
        return Err("This is not a gear sheet from this version of the program, so it was left alone.".to_string());
    }
    let from = lines.next().and_then(|l| l.strip_prefix("# from:")).map(|s| s.trim().to_string()).unwrap_or_default();
    if !good_peer(&from) {
        return Err("The sheet does not say which device it came from.".to_string());
    }
    let seen: Vec<(String, String, String)> = text.lines().filter_map(|l| l.strip_prefix("# seen:")).filter_map(|r| {
        let f: Vec<&str> = r.trim().split('\t').collect();
        (f.len() == 3 && good_peer(f[0])).then(|| (f[0].to_string(), f[1].to_string(), f[2].to_string()))
    }).collect();
    let body = text.replacen("# format: gear-sync 1\n", "# format: gear 1\n", 1);
    let l = gear::parse(&body);
    if !l.rejected.is_empty() {
        return Err("Some lines in the sheet could not be read, so none of it was used.".to_string());
    }
    Ok((from, l.items, seen))
}

/// `seen` is what the sender says it last saw of our pieces: when it matches our piece as it is now, the sender has
/// already looked at our version and its own differs on purpose, so its version is taken without a conflict.
fn compare(base: &BTreeMap<String, String>, seen: &BTreeMap<String, String>, mine: &[Piece], theirs: &[Piece], from: &str) -> Plan {
    let m: BTreeMap<&str, &Piece> = mine.iter().map(|p| (p.id.as_str(), p)).collect();
    let t: BTreeMap<&str, &Piece> = theirs.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut ids: Vec<&str> = m.keys().chain(t.keys()).copied().collect();
    ids.sort_by_key(|i| i[1..].parse::<u64>().unwrap_or(0));
    ids.dedup();
    let mut plan = Plan { from: from.to_string(), ..Default::default() };
    for id in ids {
        let b = base.get(id);
        match (m.get(id), t.get(id)) {
            (Some(a), Some(c)) if a == c => plan.same += 1,
            (Some(a), Some(c)) => {
                let (mine_same, theirs_same) = (b == Some(&print(a)), b == Some(&print(c)));
                if mine_same || seen.get(id) == Some(&print(a)) && !theirs_same {
                    plan.take.push((*c).clone());
                } else if theirs_same {
                    // changed only here: stays, goes across next time
                } else {
                    plan.conflicts.push(Conflict { id: id.to_string(), mine: Some((*a).clone()), theirs: Some((*c).clone()) });
                }
            }
            (None, Some(c)) => match b {
                None => plan.take.push((*c).clone()),
                Some(h) if *h == print(c) => {} // removed here, untouched there: stays removed
                Some(_) => plan.conflicts.push(Conflict { id: id.to_string(), mine: None, theirs: Some((*c).clone()) }),
            },
            (Some(a), None) => match b {
                None => {} // new here
                Some(h) if *h == print(a) || seen.get(id) == Some(&print(a)) => plan.remove.push((*a).clone()),
                Some(_) => plan.conflicts.push(Conflict { id: id.to_string(), mine: Some((*a).clone()), theirs: None }),
            },
            (None, None) => {}
        }
    }
    plan
}

/// What accepting this sheet would do. Changes nothing.
pub fn preview(user_root: &Path, me: &str, sheet: &str) -> Result<Plan, String> {
    let (from, theirs, seen) = parse_full(sheet)?;
    let mine = gear::load(user_root)?;
    if !mine.rejected.is_empty() {
        return Err("Some lines in the gear list could not be read, so nothing was compared. Your file has not been changed.".to_string());
    }
    Ok(compare(&start_base(user_root, me, &from, &seen), &seen_of_me(me, &seen), &mine.items, &theirs, &from))
}

/// Accept the sheet. `picks` holds the person's choice for each conflict they have decided (`true` = theirs).
/// Conflicts without a pick are left untouched and show again next time.
pub fn accept(user_root: &Path, me: &str, sheet: &str, picks: &[(String, bool)]) -> Result<Report, String> {
    let (from, theirs, seen) = parse_full(sheet)?;
    let loaded = gear::load(user_root)?;
    if !loaded.rejected.is_empty() {
        return Err("Some lines in the gear list could not be read, so nothing was changed. Your file has not been changed.".to_string());
    }
    let mut base = start_base(user_root, me, &from, &seen);
    let plan = compare(&base, &seen_of_me(me, &seen), &loaded.items, &theirs, &from);
    let mut items = loaded.items.clone();
    let mut rep = Report::default();
    for p in &plan.take {
        match items.iter_mut().find(|x| x.id == p.id) {
            Some(slot) => *slot = p.clone(),
            None => items.push(p.clone()),
        }
        rep.taken += 1;
    }
    for p in &plan.remove {
        items.retain(|x| x.id != p.id);
        rep.removed += 1;
    }
    let mut undecided: Vec<&str> = Vec::new();
    for c in &plan.conflicts {
        match picks.iter().find(|(id, _)| *id == c.id) {
            Some((_, true)) => {
                items.retain(|x| x.id != c.id);
                if let Some(t) = &c.theirs {
                    items.push(t.clone());
                }
                rep.took_theirs += 1;
            }
            Some((_, false)) => rep.kept_mine += 1,
            None => {
                undecided.push(c.id.as_str());
                rep.waiting += 1;
            }
        }
    }
    items.sort_by_key(|p| p.id[1..].parse::<u64>().unwrap_or(0));
    if items != loaded.items {
        gear::replace_all(user_root, &items)?;
    }
    // what we now remember as "last seen on the other device": its list, except for conflicts still undecided
    for p in &theirs {
        if !undecided.contains(&p.id.as_str()) {
            base.insert(p.id.clone(), print(p));
        }
    }
    let their_ids: Vec<&str> = theirs.iter().map(|p| p.id.as_str()).collect();
    base.retain(|id, _| their_ids.contains(&id.as_str()) || undecided.contains(&id.as_str()));
    save_base(user_root, &from, &base)?;
    Ok(rep)
}

// ---------- the waiting folder ----------

fn inbox(user_root: &Path) -> PathBuf {
    user_root.join("inbox").join("sync")
}
fn name_ok(n: &str) -> bool {
    n.starts_with("gear-sync") && n.ends_with(".txt") && n.len() <= 120 && !n.contains(['/', '\\', ':']) && !n.starts_with('.')
}

/// Gear sheets waiting in `user/inbox/sync/`.
pub fn waiting(user_root: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(inbox(user_root)).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| name_ok(n)).collect()).unwrap_or_default();
    v.sort();
    v
}

pub fn read_waiting(user_root: &Path, name: &str) -> Result<String, String> {
    if !name_ok(name) {
        return Err("That file could not be found.".to_string());
    }
    let p = inbox(user_root).join(name);
    let meta = std::fs::symlink_metadata(&p).map_err(|_| "That file could not be found.".to_string())?;
    if !meta.is_file() || meta.len() > 2_000_000 {
        return Err("That is not a gear sheet.".to_string());
    }
    std::fs::read_to_string(&p).map_err(|_| "That file could not be read.".to_string())
}

/// After a sheet has been accepted with no conflict left waiting, move it to `done/`.
pub fn finish_waiting(user_root: &Path, name: &str) -> Result<(), String> {
    if !name_ok(name) {
        return Err("That file could not be found.".to_string());
    }
    let done = inbox(user_root).join("done");
    std::fs::create_dir_all(&done).map_err(|e| e.to_string())?;
    std::fs::rename(inbox(user_root).join(name), done.join(name)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(n: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-gsync-{}-{}", n, std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }
    fn piece(name: &str) -> Piece {
        Piece { id: String::new(), name: name.into(), kind: "tools".into(), make: String::new(), model: String::new(), serial: String::new(), location: "Shed".into(), qty: 1, condition: "good".into(), bought: String::new(), notes: String::new() }
    }
    fn names(r: &Path) -> Vec<String> {
        gear::load(r).unwrap().items.iter().map(|p| format!("{}={}", p.id, p.name)).collect()
    }
    /// Send everything from `a` to `b` and accept it with these picks.
    fn send(a: &Path, a_code: &str, b: &Path, picks: &[(String, bool)]) -> Report {
        let me = if a_code == "vault" { "hub" } else { "vault" };
        accept(b, me, &export(a, a_code).unwrap(), picks).unwrap()
    }

    #[test]
    fn a_change_on_one_side_reaches_the_other_and_back() {
        let (v, h) = (root("v1"), root("h1"));
        gear::save(&v, piece("Tent")).unwrap();
        gear::save(&v, piece("Saw")).unwrap();
        let r = send(&v, "vault", &h, &[]);
        assert_eq!((r.taken, r.waiting), (2, 0));
        assert_eq!(names(&h), vec!["g1=Tent", "g2=Saw"]);
        // the phone changes the saw and adds a radio; the laptop gets both
        let mut saw = gear::load(&h).unwrap().items[1].clone();
        saw.location = "Truck".into();
        gear::save(&h, saw).unwrap();
        gear::save(&h, piece("Radio")).unwrap();
        let r = send(&h, "hub", &v, &[]);
        assert_eq!((r.taken, r.waiting), (2, 0));
        assert_eq!(gear::load(&v).unwrap().items[1].location, "Truck");
        assert_eq!(names(&v), vec!["g1=Tent", "g2=Saw", "g3=Radio"]);
        // sending the same sheet again changes nothing
        let again = send(&h, "hub", &v, &[]);
        assert_eq!((again.taken, again.removed, again.waiting), (0, 0, 0));
    }

    #[test]
    fn a_change_on_both_sides_waits_for_a_person_and_nothing_is_lost() {
        let (v, h) = (root("v2"), root("h2"));
        gear::save(&v, piece("Stove")).unwrap();
        send(&v, "vault", &h, &[]);
        send(&h, "hub", &v, &[]);
        let mut on_v = gear::load(&v).unwrap().items[0].clone();
        on_v.notes = "new burner".into();
        gear::save(&v, on_v).unwrap();
        let mut on_h = gear::load(&h).unwrap().items[0].clone();
        on_h.notes = "lost the key".into();
        gear::save(&h, on_h).unwrap();
        let sheet = export(&h, "hub").unwrap();
        let plan = preview(&v, "vault", &sheet).unwrap();
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].mine.as_ref().unwrap().notes, "new burner");
        assert_eq!(plan.conflicts[0].theirs.as_ref().unwrap().notes, "lost the key");
        // no pick: untouched, and it keeps showing
        let r = accept(&v, "vault", &sheet, &[]).unwrap();
        assert_eq!(r.waiting, 1);
        assert_eq!(gear::load(&v).unwrap().items[0].notes, "new burner");
        assert_eq!(preview(&v, "vault", &sheet).unwrap().conflicts.len(), 1);
        // the person picks theirs
        let r = accept(&v, "vault", &sheet, &[("g1".into(), true)]).unwrap();
        assert_eq!((r.took_theirs, r.waiting), (1, 0));
        assert_eq!(gear::load(&v).unwrap().items[0].notes, "lost the key");
        assert!(preview(&v, "vault", &sheet).unwrap().conflicts.is_empty());
    }

    #[test]
    fn keeping_mine_sends_it_across_next_time() {
        let (v, h) = (root("v3"), root("h3"));
        gear::save(&v, piece("Lantern")).unwrap();
        send(&v, "vault", &h, &[]);
        send(&h, "hub", &v, &[]);
        let mut a = gear::load(&v).unwrap().items[0].clone();
        a.notes = "mine".into();
        gear::save(&v, a).unwrap();
        let mut b = gear::load(&h).unwrap().items[0].clone();
        b.notes = "theirs".into();
        gear::save(&h, b).unwrap();
        let r = send(&h, "hub", &v, &[("g1".into(), false)]);
        assert_eq!(r.kept_mine, 1);
        assert_eq!(gear::load(&v).unwrap().items[0].notes, "mine");
        // later the laptop's version goes to the phone with no conflict
        let r = send(&v, "vault", &h, &[]);
        assert_eq!((r.taken, r.waiting), (1, 0));
        assert_eq!(gear::load(&h).unwrap().items[0].notes, "mine");
    }

    #[test]
    fn a_removal_travels_but_a_removal_against_a_change_is_a_conflict() {
        let (v, h) = (root("v4"), root("h4"));
        gear::save(&v, piece("Rope")).unwrap();
        gear::save(&v, piece("Tarp")).unwrap();
        send(&v, "vault", &h, &[]);
        send(&h, "hub", &v, &[]);
        gear::delete(&v, &["g1".to_string()]).unwrap();
        let r = send(&v, "vault", &h, &[]);
        assert_eq!(r.removed, 1);
        assert_eq!(names(&h), vec!["g2=Tarp"]);
        // phone removes the tarp while the laptop changes it
        gear::delete(&h, &["g2".to_string()]).unwrap();
        let mut t = gear::load(&v).unwrap().items[0].clone();
        t.notes = "patched".into();
        gear::save(&v, t).unwrap();
        let plan = preview(&v, "vault", &export(&h, "hub").unwrap()).unwrap();
        assert_eq!(plan.conflicts.len(), 1);
        assert!(plan.conflicts[0].theirs.is_none() && plan.conflicts[0].mine.is_some());
        let r = send(&h, "hub", &v, &[]);
        assert_eq!(r.waiting, 1);
        assert_eq!(names(&v), vec!["g2=Tarp"], "still there until a person says");
    }

    #[test]
    fn two_new_pieces_with_the_same_number_are_a_conflict_not_a_quiet_overwrite() {
        let (v, h) = (root("v5"), root("h5"));
        gear::save(&v, piece("Axe")).unwrap();
        gear::save(&h, piece("Radio")).unwrap();
        let plan = preview(&v, "vault", &export(&h, "hub").unwrap()).unwrap();
        assert_eq!(plan.conflicts.len(), 1);
        let r = send(&h, "hub", &v, &[]);
        assert_eq!(r.waiting, 1);
        assert_eq!(names(&v), vec!["g1=Axe"]);
    }

    #[test]
    fn bad_sheets_are_refused_whole_and_the_list_is_not_touched() {
        let v = root("v6");
        gear::save(&v, piece("Tent")).unwrap();
        for bad in ["", "# format: gear 1\n", "# format: gear-sync 1\n# from: ../x\n", "# format: gear-sync 1\n# from: hub\nnot a line\n"] {
            assert!(accept(&v, "vault", bad, &[]).is_err(), "{:?}", bad);
        }
        assert_eq!(names(&v), vec!["g1=Tent"]);
        assert!(export(&v, "Bad Code").is_err());
        assert!(read_waiting(&v, "../gear.txt").is_err());
    }

    #[test]
    fn waiting_sheets_are_listed_and_moved_when_done() {
        let v = root("v7");
        std::fs::create_dir_all(inbox(&v)).unwrap();
        std::fs::write(inbox(&v).join("gear-sync-hub.txt"), "x").unwrap();
        std::fs::write(inbox(&v).join("other.txt"), "x").unwrap();
        assert_eq!(waiting(&v), vec!["gear-sync-hub.txt"]);
        finish_waiting(&v, "gear-sync-hub.txt").unwrap();
        assert!(waiting(&v).is_empty() && inbox(&v).join("done").join("gear-sync-hub.txt").exists());
    }
}
