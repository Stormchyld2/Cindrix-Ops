//! The inbox for files sent by Cindrix Scan (the phone barcode scanner).
//!
//! Scan leaves a small CSV in `user/inbox/inventory/`. The file waits there, untouched, until the person
//! opens Inventory and accepts it, so it does not matter whether the phone and the base camp computer
//! are on at the same time. Accepted files move to `done/` and every line's `scan_id` is remembered,
//! so a file that arrives twice never counts twice.
//!
//! File format: first line `#cindrix-inventory-import v1` or `v2`, then a header row, then one row per
//! line. v2 adds an `action` column (`add` or `remove`); v1 files are all adds. See docs/39.

use crate::fsio;
use crate::inventory::{self, Item};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct ImportLine {
    pub scan_id: String,
    pub remove: bool,
    pub name: String,
    pub barcode: String,
    pub qty: f64,
    pub unit: String,
    /// "YYYY-MM" (the day, if Scan sent one, is dropped), or empty.
    pub use_by: String,
    pub category: String,
    pub location: String,
    pub scanned_at: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub added: usize,
    pub removed: usize,
    /// Lines already applied from an earlier file.
    pub repeated: usize,
    /// Names of things asked to be removed that are not in the inventory (or not enough of them).
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InboxFile {
    pub name: String,
    pub from: String,
    pub adds: usize,
    pub removes: usize,
    pub modified: u64,
}

pub fn inbox_dir(root: &Path) -> PathBuf {
    root.join("inbox").join("inventory")
}

fn done_dir(root: &Path) -> PathBuf {
    inbox_dir(root).join("done")
}

fn applied_path(root: &Path) -> PathBuf {
    root.join("inbox").join("applied-scan-ids.txt")
}

fn safe_name(name: &str) -> bool {
    name.ends_with(".csv") && name.len() <= 150 && !name.contains(['/', '\\', ':']) && !name.starts_with('.')
}

/// Scan's categories, in the names Inventory uses.
pub fn map_category(scan: &str) -> String {
    match scan.trim() {
        "Food" => "Food",
        "Water" => "Water",
        "Medical" => "Medical and diabetes",
        "Hygiene" => "Hygiene and sanitation",
        "Fuel" => "Fuel and heat",
        "Tools" => "Tools and repair",
        "Power" => "Power and light",
        "" => "",
        "Other" => "Safety and protection",
        other => other,
    }
    .to_string()
}

fn split(line: &str) -> Vec<String> {
    inventory::split_record(line)
}

pub fn parse(text: &str) -> Result<Vec<ImportLine>, String> {
    let mut lines = text.lines().map(|l| l.trim_end_matches('\r'));
    let first = lines.next().unwrap_or("");
    let v2 = if first.starts_with("#cindrix-inventory-import v2") {
        true
    } else if first.starts_with("#cindrix-inventory-import v1") {
        false
    } else {
        return Err("This is not a Cindrix Scan file.".into());
    };
    let header = split(lines.next().ok_or("The file has no header row.")?);
    let col = |n: &str| header.iter().position(|h| h == n);
    let need = |n: &str| col(n).ok_or_else(|| format!("The file has no \"{}\" column.", n));
    let c_id = need("scan_id")?;
    let c_item = need("item")?;
    let c_qty = need("qty")?;
    let c_action = if v2 { col("action") } else { None };
    let mut out = Vec::new();
    for (n, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let f = split(line);
        let get = |c: Option<usize>| c.and_then(|i| f.get(i)).cloned().unwrap_or_default();
        let qty: f64 = f
            .get(c_qty)
            .map(|s| s.trim())
            .unwrap_or("")
            .parse()
            .map_err(|_| format!("Line {} has a quantity that is not a number.", n + 3))?;
        if !(qty > 0.0) || qty > 100_000.0 {
            return Err(format!("Line {} has a quantity that cannot be used.", n + 3));
        }
        let name = f.get(c_item).cloned().unwrap_or_default();
        let scan_id = f.get(c_id).cloned().unwrap_or_default();
        if name.trim().is_empty() || scan_id.trim().is_empty() {
            return Err(format!("Line {} is missing a name or an id.", n + 3));
        }
        let action = get(c_action);
        let use_by_full = get(col("use_by"));
        out.push(ImportLine {
            scan_id,
            remove: action.eq_ignore_ascii_case("remove"),
            name: name.trim().to_string(),
            barcode: get(col("barcode")),
            qty,
            unit: get(col("unit")),
            use_by: use_by_full.chars().take(7).collect(),
            category: get(col("category")),
            location: get(col("location")),
            scanned_at: get(col("scanned_at")),
            source: get(col("source")),
        });
    }
    Ok(out)
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// Apply lines to the inventory. Lines whose `scan_id` is in `applied` are skipped; the rest are added to it.
pub fn apply(items: &mut Vec<Item>, lines: &[ImportLine], applied: &mut HashSet<String>) -> Report {
    let mut r = Report::default();
    for l in lines {
        if applied.contains(&l.scan_id) {
            r.repeated += 1;
            continue;
        }
        if l.remove {
            let mut left = l.qty;
            // Use up the line that goes out of date first.
            let mut idx: Vec<usize> = (0..items.len())
                .filter(|&i| same(&items[i].name, &l.name) && items[i].quantity > 0.0)
                .filter(|&i| l.location.is_empty() || same(&items[i].location, &l.location) || items[i].location.is_empty())
                .collect();
            idx.sort_by_key(|&i| {
                let u = items[i].use_by.clone();
                (u.is_empty(), u)
            });
            for i in idx {
                if left <= 0.0 {
                    break;
                }
                let take = left.min(items[i].quantity);
                items[i].quantity -= take;
                left -= take;
            }
            if left > 0.0 {
                r.missing.push(l.name.clone());
            }
            r.removed += 1;
        } else {
            let hit = items.iter().position(|i| {
                same(&i.name, &l.name) && same(&i.unit, &l.unit) && i.use_by == l.use_by && same(&i.location, &l.location)
            });
            match hit {
                Some(i) => items[i].quantity += l.qty,
                None => {
                    let bought: String = l.scanned_at.chars().take(7).collect();
                    let id = crate::userdata::next_id(items);
                    items.push(Item {
                        id,
                        category: map_category(&l.category),
                        name: l.name.clone(),
                        location: l.location.clone(),
                        quantity: l.qty,
                        unit: if l.unit.is_empty() { "ea".into() } else { l.unit.clone() },
                        use_by: l.use_by.clone(),
                        bought: if bought.len() == 7 { bought } else { String::new() },
                        notes: if l.barcode.is_empty() { String::new() } else { format!("Barcode {}", l.barcode) },
                        ..Default::default()
                    });
                }
            }
            r.added += 1;
        }
        applied.insert(l.scan_id.clone());
    }
    r
}

/// The files waiting, oldest first.
pub fn list(root: &Path) -> Vec<InboxFile> {
    let mut v = Vec::new();
    if let Ok(rd) = fs::read_dir(inbox_dir(root)) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !safe_name(&name) {
                continue;
            }
            let Ok(text) = fs::read_to_string(e.path()) else { continue };
            let Ok(lines) = parse(&text) else { continue };
            let modified = e
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            v.push(InboxFile {
                from: lines.first().map(|l| l.source.clone()).unwrap_or_default(),
                adds: lines.iter().filter(|l| !l.remove).count(),
                removes: lines.iter().filter(|l| l.remove).count(),
                name,
                modified,
            });
        }
    }
    v.sort_by(|a, b| a.modified.cmp(&b.modified).then(a.name.cmp(&b.name)));
    v
}

/// What a waiting file would do, for the person to read before accepting.
pub fn preview(root: &Path, name: &str) -> Result<Vec<ImportLine>, String> {
    if !safe_name(name) {
        return Err("That file could not be found.".into());
    }
    let text = fs::read_to_string(inbox_dir(root).join(name)).map_err(|_| "That file could not be found.".to_string())?;
    parse(&text)
}

fn load_applied(root: &Path) -> HashSet<String> {
    fs::read_to_string(applied_path(root)).map(|t| t.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

fn move_to_done(root: &Path, name: &str) -> Result<(), String> {
    let from = inbox_dir(root).join(name);
    let dir = done_dir(root);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut to = dir.join(name);
    let mut n = 1;
    while to.exists() {
        to = dir.join(format!("{}-{}.csv", name.trim_end_matches(".csv"), n));
        n += 1;
    }
    fs::rename(&from, &to).map_err(|e| e.to_string())
}

/// Apply a waiting file to the inventory, remember its ids, and move it to `done/`.
/// The inventory is backed up first. If anything fails, the file stays where it is.
pub fn accept(root: &Path, backups: &Path, name: &str) -> Result<Report, String> {
    let lines = preview(root, name)?;
    let inv_path = root.join("inventory.csv");
    let mut items = match fs::read_to_string(&inv_path) {
        Ok(t) => inventory::parse(&t).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut applied = load_applied(root);
    let report = apply(&mut items, &lines, &mut applied);
    fsio::backup_then_write(&inv_path, backups, inventory::serialize(&items).as_bytes()).map_err(|e| format!("The inventory could not be saved: {}", e))?;
    let mut ids: Vec<&String> = applied.iter().collect();
    ids.sort();
    let text: String = ids.iter().map(|i| format!("{}\n", i)).collect();
    fsio::write_atomic(&applied_path(root), text.as_bytes()).map_err(|e| e.to_string())?;
    move_to_done(root, name)?;
    Ok(report)
}

/// Put a waiting file aside without using it.
pub fn dismiss(root: &Path, name: &str) -> Result<(), String> {
    if !safe_name(name) {
        return Err("That file could not be found.".into());
    }
    move_to_done(root, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(tag: &str) -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-scaninbox-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }

    const V2: &str = "#cindrix-inventory-import v2\n\
scan_id,action,item,barcode,qty,unit,use_by,category,location,scanned_at,source\n\
a1,add,\"Beans, black\",0123,6,can,2028-03-15,Food,Pantry,2026-11-01T12:00:00Z,Falcon-27\n\
a2,add,\"Beans, black\",0123,2,can,2028-03-20,Food,Pantry,2026-11-01T12:01:00Z,Falcon-27\n\
a3,remove,\"Beans, black\",0123,1,can,,Food,Pantry,2026-11-01T12:02:00Z,Falcon-27\n";

    #[test]
    fn reads_v2_with_quotes_and_drops_the_day() {
        let l = parse(V2).unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].name, "Beans, black");
        assert_eq!(l[0].use_by, "2028-03");
        assert!(l[2].remove);
        assert_eq!(l[0].source, "Falcon-27");
    }

    #[test]
    fn v1_lines_are_all_adds() {
        let t = "#cindrix-inventory-import v1\nscan_id,item,barcode,qty,unit,use_by,category,location,scanned_at,source\nx1,Rice,,2,bag,,Food,,2026-11-01T01:00:00Z,S\n";
        let l = parse(t).unwrap();
        assert_eq!(l.len(), 1);
        assert!(!l[0].remove);
    }

    #[test]
    fn rejects_other_files_and_bad_quantities() {
        assert!(parse("id,category\n1,Water\n").is_err());
        let bad = "#cindrix-inventory-import v2\nscan_id,action,item,qty\nq,add,Rice,abc\n";
        assert!(parse(bad).is_err());
        let zero = "#cindrix-inventory-import v2\nscan_id,action,item,qty\nq,add,Rice,0\n";
        assert!(parse(zero).is_err());
    }

    #[test]
    fn add_merges_matching_lines_and_remove_takes_soonest_date_first() {
        let lines = parse(V2).unwrap();
        let mut items = Vec::new();
        let mut applied = HashSet::new();
        let r = apply(&mut items, &lines, &mut applied);
        assert_eq!((r.added, r.removed), (2, 1));
        // 2028-03 merges: 6 + 2 = 8, then one removed = 7
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, 7.0);
        assert_eq!(items[0].category, "Food");
        assert_eq!(items[0].bought, "2026-11");
        assert!(items[0].notes.contains("0123"));
    }

    #[test]
    fn remove_uses_the_line_that_expires_first_and_reports_shortfalls() {
        let mk = |id: &str, ub: &str, q: f64| Item { id: id.into(), name: "Soup".into(), unit: "can".into(), use_by: ub.into(), quantity: q, ..Default::default() };
        let mut items = vec![mk("1", "2029-01", 4.0), mk("2", "2027-06", 2.0)];
        let l = ImportLine { scan_id: "r".into(), remove: true, name: "soup".into(), barcode: "".into(), qty: 3.0, unit: "can".into(), use_by: "".into(), category: "".into(), location: "".into(), scanned_at: "".into(), source: "".into() };
        let mut applied = HashSet::new();
        apply(&mut items, &[l.clone()], &mut applied);
        assert_eq!(items[1].quantity, 0.0);
        assert_eq!(items[0].quantity, 3.0);
        let big = ImportLine { scan_id: "r2".into(), qty: 10.0, ..l };
        let r = apply(&mut items, &[big], &mut applied);
        assert_eq!(items[0].quantity, 0.0);
        assert_eq!(r.missing, vec!["soup".to_string()]);
    }

    #[test]
    fn the_same_line_never_counts_twice() {
        let lines = parse(V2).unwrap();
        let mut items = Vec::new();
        let mut applied = HashSet::new();
        apply(&mut items, &lines, &mut applied);
        let r = apply(&mut items, &lines, &mut applied);
        assert_eq!(r.repeated, 3);
        assert_eq!(items[0].quantity, 7.0);
    }

    #[test]
    fn a_waiting_file_is_listed_accepted_and_moved() {
        let root = tmp("flow");
        let dir = inbox_dir(&root);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("inventory-Falcon-27-20261101-120000.csv"), V2).unwrap();
        fs::write(dir.join("notes.txt"), "ignore me").unwrap();
        fs::write(dir.join("broken.csv"), "nonsense").unwrap();
        let l = list(&root);
        assert_eq!(l.len(), 1);
        assert_eq!((l[0].adds, l[0].removes), (2, 1));
        assert_eq!(l[0].from, "Falcon-27");

        let r = accept(&root, &root.join("backups"), &l[0].name).unwrap();
        assert_eq!(r.added, 2);
        assert!(list(&root).is_empty());
        assert!(done_dir(&root).join(&l[0].name).exists());
        let inv = inventory::parse(&fs::read_to_string(root.join("inventory.csv")).unwrap()).unwrap();
        assert_eq!(inv[0].quantity, 7.0);

        // the same file arrives again under another name: nothing is counted twice
        fs::write(dir.join("again.csv"), V2).unwrap();
        let r2 = accept(&root, &root.join("backups"), "again.csv").unwrap();
        assert_eq!(r2.repeated, 3);
        let inv = inventory::parse(&fs::read_to_string(root.join("inventory.csv")).unwrap()).unwrap();
        assert_eq!(inv[0].quantity, 7.0);
    }

    #[test]
    fn a_damaged_inventory_is_not_overwritten() {
        let root = tmp("damaged");
        fs::create_dir_all(inbox_dir(&root)).unwrap();
        fs::write(inbox_dir(&root).join("f.csv"), V2).unwrap();
        fs::write(root.join("inventory.csv"), "id,category\n1,Water\n").unwrap();
        assert!(accept(&root, &root.join("backups"), "f.csv").is_err());
        assert!(inbox_dir(&root).join("f.csv").exists());
        assert!(fs::read_to_string(root.join("inventory.csv")).unwrap().contains("1,Water"));
    }

    #[test]
    fn unsafe_names_are_refused() {
        let root = tmp("names");
        assert!(preview(&root, "../inventory.csv").is_err());
        assert!(dismiss(&root, "a/b.csv").is_err());
    }

    #[test]
    fn categories_use_inventory_names() {
        assert_eq!(map_category("Medical"), "Medical and diabetes");
        assert_eq!(map_category("Food"), "Food");
        assert_eq!(map_category(""), "");
    }
}
