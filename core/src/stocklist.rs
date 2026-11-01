//! The stock list: a read-only copy of the inventory for the phone apps (Cindrix Scan, Cindrix Pocket).
//!
//! Only the device that hosts the server changes the inventory. It writes this file after every change
//! to `user/outbox/stock-list.csv`; the phones read it and send their changes back as Scan files
//! (see scaninbox.rs). The phones never edit the list themselves.
//!
//! Format: first line `#cindrix-stock-list v1`, then
//! `name,barcode,unit,location,qty,use_by,category,updated`, one row per inventory line. UTF-8, LF.

use crate::inventory::{quote, Item};

pub const MARKER: &str = "#cindrix-stock-list v1";
pub const HEADER: &str = "name,barcode,unit,location,qty,use_by,category,updated";

/// The barcode Scan left in the notes ("Barcode 0123..."), or empty.
pub fn barcode_of(item: &Item) -> String {
    item.notes
        .split(|c: char| c == ';' || c == '\n')
        .filter_map(|p| p.trim().strip_prefix("Barcode "))
        .map(|b| b.trim().to_string())
        .next()
        .unwrap_or_default()
}

fn qty(q: f64) -> String {
    if q == q.floor() { format!("{}", q as i64) } else { format!("{}", q) }
}

/// `updated` is a UTC time like 2026-11-01T18:00:00Z, written into every row so a phone can tell how old its copy is.
pub fn build(items: &[Item], updated: &str) -> String {
    let mut out = format!("{}\n{}\n", MARKER, HEADER);
    for i in items.iter().filter(|i| i.quantity > 0.0) {
        let cols = [
            quote(&i.name), quote(&barcode_of(i)), quote(&i.unit), quote(&i.location),
            qty(i.quantity), quote(&i.use_by), quote(&i.category), quote(updated),
        ];
        out.push_str(&cols.join(","));
        out.push('\n');
    }
    out
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
pub fn utc_stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // civil-from-days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(name: &str, q: f64, notes: &str) -> Item {
        Item { id: "1".into(), name: name.into(), quantity: q, unit: "can".into(), location: "Pantry".into(),
               use_by: "2028-03".into(), category: "Food".into(), notes: notes.into(), ..Default::default() }
    }

    #[test]
    fn time_stamp_is_right() {
        assert_eq!(utc_stamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_stamp(1_782_950_400), "2026-07-02T00:00:00Z");
        assert_eq!(utc_stamp(951_782_400 + 3_661), "2000-02-29T01:01:01Z");
    }

    #[test]
    fn saving_the_inventory_also_writes_the_stock_list() {
        let root = std::env::temp_dir().join(format!("cx-stock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let u = crate::userdata::UserDir::new(&root);
        u.save_inventory(&[it("Rice", 3.0, "Barcode 55")]).unwrap();
        let t = std::fs::read_to_string(u.stock_list_path()).unwrap();
        assert!(t.starts_with(MARKER));
        assert!(t.contains("Rice,55,can,Pantry,3"));
    }

    #[test]
    fn writes_marker_header_and_rows() {
        let t = build(&[it("Beans, black", 7.0, "Barcode 0123")], "2026-11-01T18:00:00Z");
        let l: Vec<&str> = t.lines().collect();
        assert_eq!(l[0], MARKER);
        assert_eq!(l[1], HEADER);
        assert_eq!(l[2], "\"Beans, black\",0123,can,Pantry,7,2028-03,Food,2026-11-01T18:00:00Z");
    }

    #[test]
    fn used_up_lines_are_left_out() {
        let t = build(&[it("Soup", 0.0, ""), it("Rice", 2.5, "")], "x");
        assert_eq!(t.lines().count(), 3);
        assert!(t.contains("Rice,,can,Pantry,2.5"));
    }

    #[test]
    fn finds_the_barcode_among_other_notes() {
        assert_eq!(barcode_of(&it("A", 1.0, "keep dry; Barcode 99")), "99");
        assert_eq!(barcode_of(&it("A", 1.0, "")), "");
    }
}
