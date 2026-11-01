//! Merging the inventory copies of the two devices that may change it (the Windows computer and the Android
//! phone that hosts the server).
//!
//! Nothing here depends on the order the files travel in, on a file arriving twice, or on a reply getting lost:
//!
//!  * Every line has a `uid` that is the same on every device.
//!  * Stock is counted per device: each line carries, for every device, the total it has ever added and the total
//!    it has ever taken out. The quantity is all adds minus all take-outs (never shown below zero). Merging takes the
//!    larger total for each device, so adding six on the laptop and taking out one on the phone gives +5 whichever
//!    meets first, and merging the same file again changes nothing.
//!  * Other fields (name, place, use-by, notes...): the newer change to the line wins, and the person is told if
//!    the other side's differed.
//!  * A deleted line is remembered with the time it was deleted. It stays deleted unless it was changed afterwards.

use crate::inventory::{self, Item};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A merge id that will not repeat on any device: time in nanoseconds, the process number and a counter.
pub fn new_uid() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", n, std::process::id(), c)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MergeReport {
    pub added: usize,
    pub removed: usize,
    /// Lines whose quantity changed because the other device also changed it.
    pub quantities_combined: usize,
    /// Plain-words notes about lines where both devices changed the same thing.
    pub notes: Vec<String>,
}

pub type Counts = BTreeMap<String, (f64, f64)>;

pub fn parse_counts(s: &str) -> Counts {
    let mut m = Counts::new();
    for part in s.split(';').filter(|p| !p.trim().is_empty()) {
        let f: Vec<&str> = part.split(':').collect();
        if f.len() == 3 {
            if let (Ok(a), Ok(r)) = (f[1].parse::<f64>(), f[2].parse::<f64>()) {
                m.insert(f[0].to_string(), (a, r));
            }
        }
    }
    m
}

pub fn format_counts(c: &Counts) -> String {
    c.iter().map(|(d, (a, r))| format!("{}:{}:{}", d, a, r)).collect::<Vec<_>>().join(";")
}

fn net(c: &Counts) -> f64 {
    c.values().map(|(a, r)| a - r).sum::<f64>().max(0.0)
}

/// The counts of a line, or for an older line that has none yet, its quantity credited to `device`.
fn counts_of(i: &Item, device: &str) -> Counts {
    let c = parse_counts(&i.counts);
    if c.is_empty() && i.quantity > 0.0 {
        let mut m = Counts::new();
        m.insert(device.to_string(), (i.quantity, 0.0));
        m
    } else {
        c
    }
}

/// Called when this device saves a line: whatever the quantity moved by is recorded against this device.
/// `old` is the line as it was on disk, if it was there.
pub fn record_local_change(old: Option<&Item>, new: &mut Item, device: &str) {
    let mut c = match old {
        Some(o) => counts_of(o, device),
        None => Counts::new(),
    };
    let before = old.map(|o| o.quantity).unwrap_or(0.0);
    let delta = new.quantity - before;
    if delta != 0.0 {
        let e = c.entry(device.to_string()).or_insert((0.0, 0.0));
        if delta > 0.0 { e.0 += delta } else { e.1 += -delta }
    }
    new.counts = format_counts(&c);
}

fn merge_counts(a: &Counts, b: &Counts) -> Counts {
    let mut out = a.clone();
    for (d, (x, y)) in b {
        let e = out.entry(d.clone()).or_insert((0.0, 0.0));
        e.0 = e.0.max(*x);
        e.1 = e.1.max(*y);
    }
    out
}

/// Deleted lines: uid and the time (YYYY-MM-DDTHH:MM:SSZ) they were deleted.
pub type Tombstones = BTreeMap<String, String>;

pub fn parse_tombstones(text: &str) -> Tombstones {
    text.lines()
        .filter_map(|l| {
            let (u, t) = l.trim().split_once(',')?;
            if u.is_empty() || u == "uid" { None } else { Some((u.to_string(), t.to_string())) }
        })
        .collect()
}

pub fn format_tombstones(t: &Tombstones) -> String {
    let mut s = String::from("uid,deleted\n");
    for (u, at) in t {
        s.push_str(&format!("{},{}\n", u, at));
    }
    s
}

fn union_tombs(a: &Tombstones, b: &Tombstones) -> Tombstones {
    let mut out = a.clone();
    for (u, t) in b {
        let e = out.entry(u.clone()).or_insert_with(|| t.clone());
        if t > e {
            *e = t.clone();
        }
    }
    out
}

fn other_fields_differ(a: &Item, b: &Item) -> bool {
    inventory::same_content(&Item { quantity: 0.0, ..a.clone() }, &Item { quantity: 0.0, ..b.clone() }) == false
}

/// Merge two copies. Lines without a `uid` on `mine` are kept as they are.
/// The result is the same whichever copy is called `mine`, and merging a copy with itself changes nothing.
pub fn merge(
    mine: &[Item],
    theirs: &[Item],
    my_tombs: &Tombstones,
    their_tombs: &Tombstones,
    device: &str,
) -> (Vec<Item>, Tombstones, MergeReport) {
    let mut r = MergeReport::default();
    let tombs = union_tombs(my_tombs, their_tombs);
    let their_by: BTreeMap<&str, &Item> = theirs.iter().filter(|i| !i.uid.is_empty()).map(|i| (i.uid.as_str(), i)).collect();
    let mine_uids: std::collections::HashSet<&str> = mine.iter().map(|i| i.uid.as_str()).collect();
    let gone = |i: &Item| tombs.get(&i.uid).map(|t| t.as_str() >= i.changed.as_str()).unwrap_or(false);
    let mut out: Vec<Item> = Vec::new();

    for m in mine {
        if m.uid.is_empty() {
            out.push(m.clone());
            continue;
        }
        match their_by.get(m.uid.as_str()) {
            Some(t) => {
                if gone(m) && gone(t) {
                    r.removed += 1;
                    continue;
                }
                let counts = merge_counts(&counts_of(m, device), &counts_of(t, device));
                let theirs_newer = t.changed > m.changed;
                let mut line = if theirs_newer { Item { id: m.id.clone(), ..(*t).clone() } } else { m.clone() };
                if other_fields_differ(m, t) && m.changed != t.changed {
                    r.notes.push(format!("{}: changed on both; the newer change was kept.", m.name));
                }
                line.quantity = net(&counts);
                if (line.quantity - m.quantity).abs() > f64::EPSILON && (line.quantity - t.quantity).abs() > f64::EPSILON {
                    r.quantities_combined += 1;
                }
                line.counts = format_counts(&counts);
                line.changed = if m.changed >= t.changed { m.changed.clone() } else { t.changed.clone() };
                line.uid = m.uid.clone();
                out.push(line);
            }
            None => {
                if gone(m) {
                    r.removed += 1;
                } else {
                    out.push(m.clone());
                }
            }
        }
    }
    for t in theirs {
        if t.uid.is_empty() || mine_uids.contains(t.uid.as_str()) || gone(t) {
            continue;
        }
        let id = crate::userdata::next_id(&out);
        let mut line = Item { id, ..t.clone() };
        line.counts = format_counts(&counts_of(t, device));
        line.quantity = net(&parse_counts(&line.counts));
        out.push(line);
        r.added += 1;
    }
    (out, tombs, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(id: &str, uid: &str, name: &str, counts: &str, loc: &str, changed: &str) -> Item {
        let q = net(&parse_counts(counts));
        Item { id: id.into(), uid: uid.into(), name: name.into(), quantity: q, unit: "can".into(),
               location: loc.into(), changed: changed.into(), counts: counts.into(), ..Default::default() }
    }
    fn none() -> Tombstones { Tombstones::new() }

    #[test]
    fn local_changes_are_recorded_per_device() {
        let mut a = Item { id: "1".into(), name: "Beans".into(), quantity: 10.0, ..Default::default() };
        record_local_change(None, &mut a, "laptop");
        assert_eq!(a.counts, "laptop:10:0");
        let old = a.clone();
        a.quantity = 4.0;
        record_local_change(Some(&old), &mut a, "phone");
        assert_eq!(parse_counts(&a.counts)["phone"], (0.0, 6.0));
        assert_eq!(net(&parse_counts(&a.counts)), 4.0);
    }

    #[test]
    fn older_lines_without_counts_are_credited_to_this_device() {
        let old = Item { id: "1".into(), quantity: 5.0, ..Default::default() };
        let mut new = Item { quantity: 7.0, ..old.clone() };
        record_local_change(Some(&old), &mut new, "laptop");
        assert_eq!(parse_counts(&new.counts)["laptop"], (7.0, 0.0));
    }

    #[test]
    fn add_six_here_and_take_one_there_is_plus_five_in_any_order() {
        // both started from the same 10 (added on the laptop), then split
        let laptop = vec![it("1", "u1", "Beans", "laptop:16:0", "Pantry", "t1")];
        let phone = vec![it("7", "u1", "Beans", "laptop:10:0;phone:0:1", "Pantry", "t2")];
        let (a, _, ra) = merge(&laptop, &phone, &none(), &none(), "laptop");
        let (b, _, _) = merge(&phone, &laptop, &none(), &none(), "phone");
        assert_eq!(a[0].quantity, 15.0);
        assert_eq!(b[0].quantity, 15.0);
        assert_eq!(a[0].counts, b[0].counts);
        assert_eq!(ra.quantities_combined, 1);
    }

    #[test]
    fn merging_the_same_copy_again_changes_nothing_even_with_a_lost_reply() {
        let laptop = vec![it("1", "u1", "Beans", "laptop:16:0", "Pantry", "t1")];
        let phone = vec![it("7", "u1", "Beans", "laptop:10:0;phone:0:1", "Pantry", "t2")];
        let (m1, t1, _) = merge(&laptop, &phone, &none(), &none(), "laptop");
        let (m2, _, r2) = merge(&m1, &phone, &t1, &none(), "laptop");
        assert_eq!(m1, m2);
        assert!(r2.notes.is_empty());
        // the phone sends another copy after taking out one more; the laptop never sent its reply
        let phone2 = vec![it("7", "u1", "Beans", "laptop:10:0;phone:0:2", "Pantry", "t3")];
        let (m3, _, _) = merge(&m2, &phone2, &t1, &none(), "laptop");
        assert_eq!(m3[0].quantity, 14.0);
    }

    #[test]
    fn newer_edit_to_other_fields_wins_and_is_reported() {
        let a = vec![it("1", "u1", "Beans", "d:5:0", "Pantry", "t1")];
        let b = vec![it("4", "u1", "Beans", "d:5:0", "Garage", "t3")];
        let (m, _, r) = merge(&a, &b, &none(), &none(), "d");
        assert_eq!(m[0].location, "Garage");
        assert_eq!(m[0].id, "1");
        assert_eq!(r.notes.len(), 1);
        let (m2, _, _) = merge(&b, &a, &none(), &none(), "d");
        assert_eq!(m2[0].location, "Garage");
    }

    #[test]
    fn new_lines_on_either_side_are_added_with_a_free_local_number() {
        let a = vec![it("1", "u1", "Rice", "d:1:0", "P", "t1")];
        let b = vec![it("1", "u2", "Salt", "e:2:0", "P", "t1")];
        let (m, _, r) = merge(&a, &b, &none(), &none(), "d");
        assert_eq!(m.len(), 2);
        assert_eq!(r.added, 1);
        assert_ne!(m[0].id, m[1].id);
        assert_eq!(m[1].quantity, 2.0);
    }

    #[test]
    fn a_deleted_line_stays_deleted_unless_changed_afterwards() {
        let line = it("1", "u1", "Beans", "d:5:0", "P", "t1");
        let mut gone = Tombstones::new();
        gone.insert("u1".into(), "t2".into());
        // the other device still has the old line: it is dropped
        let (m, tombs, r) = merge(&[], &[line.clone()], &gone, &none(), "d");
        assert!(m.is_empty());
        assert_eq!(r.added, 0);
        assert!(tombs.contains_key("u1"));
        // and when the deleting side merges the other way
        let (m2, _, r2) = merge(&[line.clone()], &[], &none(), &gone, "d");
        assert!(m2.is_empty());
        assert_eq!(r2.removed, 1);
        // a change made after the delete brings it back
        let later = it("1", "u1", "Beans", "d:6:0", "P", "t9");
        let (m3, _, _) = merge(&[], &[later], &gone, &none(), "d");
        assert_eq!(m3.len(), 1);
    }

    #[test]
    fn quantity_is_never_shown_below_zero() {
        let a = vec![it("1", "u1", "Beans", "d:2:0;e:0:5", "P", "t1")];
        assert_eq!(a[0].quantity, 0.0);
    }

    #[test]
    fn tombstone_files_round_trip() {
        let mut t = Tombstones::new();
        t.insert("u1".into(), "2026-11-01T10:00:00Z".into());
        assert_eq!(parse_tombstones(&format_tombstones(&t)), t);
    }

    #[test]
    fn uids_are_not_repeated() {
        assert_ne!(new_uid(), new_uid());
    }
}
