//! Keeping two linked devices (a Vault laptop and a Hub phone) in step by themselves (docs/61 phase 6, doc 34).
//!
//! One side is the **caller**: its program reaches the other over the local network (`peer.rs`). Each time it asks for
//! the other side's gear sheet, inventory copy and security logs, takes them in, and sends its own. The other side
//! (the **server**, the share server) takes in what arrives with the same functions. Both ends use `apply`, so the
//! rules are the same whichever way the copy runs, and the tests drive two folders against each other with no network.
//!
//!  * Gear: `gearsync.rs`. A sheet with no conflict is taken at once; one with a conflict is kept waiting on the Gear page.
//!  * Inventory: `sync.rs` (per-line ids and per-device stock counts), which merges by itself.
//!  * Security logs: `logsync.rs`. Lines that clash are held and listed for a person (`user/sync/held.txt`).
//!
//! Nothing here is sent anywhere but to the device the person linked.

use crate::fsio;
use crate::gearsync;
use crate::logsync;
use crate::userdata::UserDir;
use std::path::{Path, PathBuf};

/// What a caller may ask the other end for, and send it.
pub trait Remote {
    fn get(&self, kind: &str) -> Result<String, String>;
    fn put(&self, kind: &str, body: &str) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Summary {
    pub gear_taken: usize,
    pub gear_waiting: usize,
    pub inventory_changes: usize,
    pub log_lines: usize,
    /// Plain-words things the person should look at, and anything that failed.
    pub notes: Vec<String>,
}

/// Every kind this pair of devices exchanges, in the order they are copied.
pub fn kinds() -> Vec<String> {
    let mut v = vec!["gear".to_string(), "inventory".to_string()];
    v.extend(logsync::LOGS.iter().map(|l| format!("log:{}", l)));
    v
}

fn held_path(user_root: &Path) -> PathBuf {
    user_root.join("sync").join("held.txt")
}

/// Lines held for a person across all logs: `file, id, why, line`.
pub fn held(user_root: &Path) -> Vec<logsync::Held> {
    fsio::read_to_string(&held_path(user_root)).unwrap_or_default().lines().filter_map(|l| {
        let f: Vec<&str> = l.splitn(4, '\t').collect();
        (f.len() == 4).then(|| logsync::Held { file: f[0].into(), id: f[1].into(), why: f[2].into(), line: f[3].into() })
    }).collect()
}

fn save_held(user_root: &Path, file: &str, now_held: &[logsync::Held]) -> Result<(), String> {
    let mut all: Vec<logsync::Held> = held(user_root).into_iter().filter(|h| h.file != file).collect();
    all.extend(now_held.iter().filter(|h| !is_dismissed(user_root, h)).cloned());
    let text: String = all.iter().map(|h| format!("{}\t{}\t{}\t{}\n", h.file, h.id, h.why.replace(['\t', '\n'], " "), h.line)).collect();
    fsio::write_atomic(&held_path(user_root), text.as_bytes()).map_err(|e| format!("The list of held lines could not be saved: {}", e))
}

fn dismissed_path(user_root: &Path) -> PathBuf {
    user_root.join("sync").join("dismissed.txt")
}

fn is_dismissed(user_root: &Path, h: &logsync::Held) -> bool {
    fsio::read_to_string(&dismissed_path(user_root)).unwrap_or_default().lines().any(|l| l.split_once('\t') == Some((h.file.as_str(), h.line.as_str())))
}

/// The person has looked at a held line and wants it gone from the list for good (the log itself is not changed,
/// and the same line is not held again).
pub fn dismiss(user_root: &Path, file: &str, line: &str) -> Result<(), String> {
    fsio::append(&dismissed_path(user_root), format!("{}\t{}\n", file, line).as_bytes()).map_err(|e| format!("That could not be saved: {}", e))?;
    let left: Vec<logsync::Held> = held(user_root).into_iter().filter(|h| !(h.file == file && h.line == line)).collect();
    let text: String = left.iter().map(|h| format!("{}\t{}\t{}\t{}\n", h.file, h.id, h.why.replace(['\t', '\n'], " "), h.line)).collect();
    fsio::write_atomic(&held_path(user_root), text.as_bytes()).map_err(|e| format!("The list could not be saved: {}", e))
}

fn my_code(user_root: &Path) -> String {
    UserDir::new(user_root).device_id()
}

/// This device's copy of one kind, for sending.
pub fn export(user_root: &Path, kind: &str) -> Result<String, String> {
    if kind == "gear" {
        return gearsync::export(user_root, &my_code(user_root));
    }
    if kind == "inventory" {
        let u = UserDir::new(user_root);
        let csv = std::fs::read_to_string(u.sync_export_path()).unwrap_or_default();
        let del = std::fs::read_to_string(user_root.join("outbox").join("inventory-deleted.csv")).unwrap_or_default();
        return Ok(format!("{}\n{}", del.lines().count(), format!("{}{}", del, csv)));
    }
    if let Some(f) = kind.strip_prefix("log:") {
        return logsync::export(user_root, f);
    }
    Err("That is not something the two devices copy.".to_string())
}

/// Take in what the other device sent for one kind. Returns a short plain-words result.
pub fn apply(user_root: &Path, kind: &str, body: &str, sum: &mut Summary) -> Result<(), String> {
    if kind == "gear" {
        let me = my_code(user_root);
        let plan = gearsync::preview(user_root, &me, body)?;
        if plan.conflicts.is_empty() {
            let r = gearsync::accept(user_root, &me, body, &[])?;
            sum.gear_taken += r.taken + r.removed;
        } else {
            gearsync::hold(user_root, body)?;
            sum.gear_waiting += plan.conflicts.len();
            sum.notes.push(format!("{} gear piece(s) changed on both devices are waiting for you on the Gear page.", plan.conflicts.len()));
        }
        return Ok(());
    }
    if kind == "inventory" {
        // the first line says how many lines of "deleted" come before the inventory copy
        let (n, rest) = body.split_once('\n').ok_or("The inventory copy could not be read.")?;
        let n: usize = n.trim().parse().map_err(|_| "The inventory copy could not be read.".to_string())?;
        let mut idx = 0;
        for _ in 0..n {
            idx += rest[idx..].find('\n').map(|i| i + 1).ok_or("The inventory copy could not be read.")?;
        }
        let (del, csv) = rest.split_at(idx);
        if csv.trim().is_empty() {
            return Ok(());
        }
        let r = UserDir::new(user_root).merge_in(csv, del)?;
        sum.inventory_changes += r.added + r.removed + r.quantities_combined;
        sum.notes.extend(r.notes);
        return Ok(());
    }
    if let Some(f) = kind.strip_prefix("log:") {
        let m = logsync::accept(user_root, f, body)?;
        sum.log_lines += m.added;
        save_held(user_root, f, &m.held)?;
        let m_held: Vec<&logsync::Held> = m.held.iter().filter(|h| !is_dismissed(user_root, h)).collect();
        if !m_held.is_empty() {
            sum.notes.push(format!("{} line(s) in {} clash with this device's and are held for you to look at.", m_held.len(), f));
        }
        return Ok(());
    }
    Err("That is not something the two devices copy.".to_string())
}

/// One full round from the caller's side: for every kind, take in the other side's copy and send ours.
/// One kind failing never stops the rest; the failure is added to the notes.
pub fn run(user_root: &Path, remote: &dyn Remote) -> Summary {
    let mut sum = Summary::default();
    for kind in kinds() {
        match remote.get(&kind) {
            Ok(body) if !body.trim().is_empty() => {
                if let Err(e) = apply(user_root, &kind, &body, &mut sum) {
                    sum.notes.push(format!("{}: {}", kind, e));
                }
            }
            Ok(_) => {}
            Err(e) => {
                sum.notes.push(format!("{}: {}", kind, e));
                continue;
            }
        }
        match export(user_root, &kind) {
            Ok(mine) if !mine.trim().is_empty() => {
                if let Err(e) = remote.put(&kind, &mine) {
                    sum.notes.push(format!("{}: {}", kind, e));
                }
            }
            Ok(_) => {}
            Err(e) => sum.notes.push(format!("{}: {}", kind, e)),
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gear::{self, Piece};

    fn root(n: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-peer-{}-{}", n, std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }
    fn piece(name: &str) -> Piece {
        Piece { id: String::new(), name: name.into(), kind: "tools".into(), make: String::new(), model: String::new(), serial: String::new(), location: "Shed".into(), qty: 1, condition: "good".into(), bought: String::new(), notes: String::new() }
    }
    /// The other device, reached by calling its own functions (what the share server does with a request).
    struct Local(PathBuf);
    impl Remote for Local {
        fn get(&self, kind: &str) -> Result<String, String> {
            export(&self.0, kind)
        }
        fn put(&self, kind: &str, body: &str) -> Result<String, String> {
            let mut s = Summary::default();
            apply(&self.0, kind, body, &mut s)?;
            Ok("ok".into())
        }
    }
    fn site_and_log(r: &Path, line: &str) {
        let dir = crate::security_store::dir(r);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("comms-log.txt");
        let old = std::fs::read_to_string(&p).unwrap_or_else(|_| "# format: comms-log 1\n".into());
        std::fs::write(p, format!("{}{}\n", old, line)).unwrap();
    }

    #[test]
    fn one_round_brings_both_devices_level_on_gear_and_logs() {
        let (v, h) = (root("v1"), root("h1"));
        gear::save(&v, piece("Tent")).unwrap();
        gear::save(&h, piece("Tent")).unwrap(); // the same first piece both sides: identical, no conflict
        site_and_log(&v, "c1\thome\tfrom the laptop");
        site_and_log(&h, "c1\thome\tfrom the laptop");
        site_and_log(&h, "c2\thome\tonly on the phone");
        gear::save(&h, piece("Radio")).unwrap();
        let s = run(&v, &Local(h.clone()));
        assert_eq!(s.notes, Vec::<String>::new(), "{:?}", s.notes);
        let names = |r: &Path| gear::load(r).unwrap().items.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&v), vec!["Tent", "Radio"]);
        assert_eq!(names(&h), names(&v));
        assert!(std::fs::read_to_string(crate::security_store::dir(&v).join("comms-log.txt")).unwrap().contains("only on the phone"));
        // another round with nothing new changes nothing
        let again = run(&v, &Local(h.clone()));
        assert_eq!((again.gear_taken, again.log_lines, again.gear_waiting), (0, 0, 0));
    }

    #[test]
    fn a_clash_is_held_for_a_person_and_never_overwrites_either_side() {
        let (v, h) = (root("v2"), root("h2"));
        site_and_log(&v, "c1\thome\tlaptop version");
        site_and_log(&h, "c1\thome\tphone version");
        let s = run(&v, &Local(h.clone()));
        assert!(s.notes.iter().any(|n| n.contains("held for you")), "{:?}", s.notes);
        assert!(std::fs::read_to_string(crate::security_store::dir(&v).join("comms-log.txt")).unwrap().contains("laptop version"));
        assert!(std::fs::read_to_string(crate::security_store::dir(&h).join("comms-log.txt")).unwrap().contains("phone version"));
        let hv = held(&v);
        assert_eq!(hv.len(), 1);
        assert!(hv[0].line.contains("phone version"));
        dismiss(&v, &hv[0].file, &hv[0].line).unwrap();
        assert!(held(&v).is_empty());
        let again = run(&v, &Local(h.clone()));
        assert!(held(&v).is_empty() && !again.notes.iter().any(|n| n.contains("held for you")), "a dismissed line is not held again");
    }

    #[test]
    fn a_gear_conflict_waits_on_the_page_and_one_failure_does_not_stop_the_rest() {
        let (v, h) = (root("v3"), root("h3"));
        gear::save(&v, piece("Axe")).unwrap();
        gear::save(&h, piece("Radio")).unwrap();
        site_and_log(&h, "c1\thome\tnews");
        let s = run(&v, &Local(h.clone()));
        assert_eq!(s.gear_waiting, 1);
        assert_eq!(gearsync::waiting(&v).len(), 1, "kept on the Gear page for a person");
        assert!(std::fs::read_to_string(crate::security_store::dir(&v).join("comms-log.txt")).unwrap().contains("news"), "logs still copied");
        // a remote that fails for one kind
        struct Flaky(PathBuf);
        impl Remote for Flaky {
            fn get(&self, kind: &str) -> Result<String, String> {
                if kind == "gear" { Err("The other device did not answer.".into()) } else { export(&self.0, kind) }
            }
            fn put(&self, _: &str, _: &str) -> Result<String, String> {
                Ok(String::new())
            }
        }
        let s = run(&v, &Flaky(h));
        assert!(s.notes.iter().any(|n| n.starts_with("gear:")));
    }

    #[test]
    fn inventory_copies_travel_and_odd_kinds_are_refused() {
        let (v, h) = (root("v4"), root("h4"));
        let uv = UserDir::new(&v);
        let item = crate::inventory::Item { name: "Rice".into(), category: "food".into(), location: "Pantry".into(), quantity: 5.0, unit: "kg".into(), ..Default::default() };
        uv.save_inventory(&[item]).unwrap();
        let s = run(&h, &Local(v.clone()));
        assert!(s.notes.is_empty(), "{:?}", s.notes);
        assert_eq!(UserDir::new(&h).load_inventory().unwrap().len(), 1);
        let mut sum = Summary::default();
        assert!(apply(&h, "../etc", "x", &mut sum).is_err());
        assert!(export(&h, "log:incident-log.txt").is_err());
        assert!(apply(&h, "inventory", "nonsense", &mut sum).is_err());
    }
}
