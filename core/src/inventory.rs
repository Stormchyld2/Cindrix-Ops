//! Inventory stored as CSV. Medicine rows hold counts only, never doses.

use std::fmt;

pub const HEADER: &str = "id,category,name,location,quantity,unit,use_per_day,use_by,notes,brand,size,for_whom,bought,target,condition,checked,cost,source,serial";
/// The first nine columns are the original layout; files written before the extra columns are still read.
const CORE_COLUMNS: usize = 9;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub id: String,
    pub category: String,
    pub name: String,
    pub location: String,
    pub quantity: f64,
    pub unit: String,
    pub use_per_day: f64,
    /// "YYYY-MM" or empty
    pub use_by: String,
    pub notes: String,
    /// Brand or kind, for example "Mountain House" or "AA lithium".
    pub brand: String,
    /// Size of one unit, for example "1 gal", "15 oz can", "10 ft".
    pub size: String,
    /// Who it is for: everyone, an adult, a child, a baby, a pet, or a name.
    pub for_whom: String,
    /// "YYYY-MM" it was bought or packed, or empty.
    pub bought: String,
    /// How many you want to keep. 0 means no target is set.
    pub target: f64,
    /// new, good, worn or needs repair (for gear); empty for things that get used up.
    pub condition: String,
    /// "YYYY-MM-DD" it was last looked at, or empty.
    pub checked: String,
    /// What it cost in all (not each); 0 when not known.
    pub cost: f64,
    /// Where it was bought, or where to get more.
    pub source: String,
    /// Model or serial number.
    pub serial: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Check,
    ActNow,
}

#[derive(Debug)]
pub struct ParseError(pub String);
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub(crate) fn split_record(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, in_q) {
            ('"', true) => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_q = false;
                }
            }
            ('"', false) => in_q = true,
            (',', false) => fields.push(std::mem::take(&mut cur)),
            (c, _) => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

pub(crate) fn quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn num(s: &str, what: &str) -> Result<f64, ParseError> {
    if s.trim().is_empty() {
        return Ok(0.0);
    }
    s.trim().parse::<f64>().map_err(|_| ParseError(format!("{} is not a number: {}", what, s)))
}

pub fn parse(csv: &str) -> Result<Vec<Item>, ParseError> {
    let mut items = Vec::new();
    for (n, line) in csv.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || (n == 0 && line.starts_with("id,")) {
            continue;
        }
        let f = split_record(line);
        if f.len() < CORE_COLUMNS {
            return Err(ParseError(format!("line {} has {} fields, expected at least {}", n + 1, f.len(), CORE_COLUMNS)));
        }
        let extra = |k: usize| f.get(CORE_COLUMNS + k).cloned().unwrap_or_default();
        items.push(Item {
            id: f[0].clone(),
            category: f[1].clone(),
            name: f[2].clone(),
            location: f[3].clone(),
            quantity: num(&f[4], "quantity")?,
            unit: f[5].clone(),
            use_per_day: num(&f[6], "use_per_day")?,
            use_by: f[7].clone(),
            notes: f[8].clone(),
            brand: extra(0),
            size: extra(1),
            for_whom: extra(2),
            bought: extra(3),
            target: num(&extra(4), "target")?,
            condition: extra(5),
            checked: extra(6),
            cost: num(&extra(7), "cost")?,
            source: extra(8),
            serial: extra(9),
        });
    }
    Ok(items)
}

pub fn serialize(items: &[Item]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for i in items {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            quote(&i.id), quote(&i.category), quote(&i.name), quote(&i.location), i.quantity,
            quote(&i.unit), i.use_per_day, quote(&i.use_by), quote(&i.notes),
            quote(&i.brand), quote(&i.size), quote(&i.for_whom), quote(&i.bought), i.target,
            quote(&i.condition), quote(&i.checked), i.cost, quote(&i.source), quote(&i.serial)
        ));
    }
    out
}

/// Whole days the item lasts at the current use rate, or None if it is not used up daily.
pub fn days_left(item: &Item) -> Option<f64> {
    if item.use_per_day > 0.0 {
        Some(item.quantity / item.use_per_day)
    } else {
        None
    }
}

/// Months from (year, month) to the item's use-by month; negative if past.
pub fn months_until(item: &Item, year: i32, month: u32) -> Option<i32> {
    let (y, m) = item.use_by.split_once('-')?;
    let (y, m): (i32, i32) = (y.trim().parse().ok()?, m.trim().parse().ok()?);
    if !(1..=12).contains(&m) {
        return None;
    }
    Some((y - year) * 12 + (m - month as i32))
}

/// When an item turns "check" or "act now". The defaults are what the program ships with; the
/// owner can change them in Settings (Plan).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// Fewer days left than this: act now.
    pub act_days: f64,
    /// Fewer days left than this: check.
    pub check_days: f64,
    /// Use-by month this many months away or less: check. (Already past: act now.)
    pub check_months: i32,
}

impl Default for Thresholds {
    fn default() -> Thresholds {
        Thresholds { act_days: 7.0, check_days: 14.0, check_months: 3 }
    }
}

/// Status for the Inventory screen, with the program's standard thresholds.
pub fn status(item: &Item, year: i32, month: u32) -> Status {
    status_with(item, year, month, &Thresholds::default())
}

/// Status for the Inventory screen, with the owner's thresholds.
pub fn status_with(item: &Item, year: i32, month: u32, t: &Thresholds) -> Status {
    let expiry = months_until(item, year, month);
    let days = days_left(item);
    match (expiry, days) {
        (Some(e), _) if e < 0 => Status::ActNow,
        (_, Some(d)) if d < t.act_days => Status::ActNow,
        (_, _) if item.target > 0.0 && item.quantity <= 0.0 => Status::ActNow,
        (Some(e), _) if e <= t.check_months => Status::Check,
        (_, Some(d)) if d < t.check_days => Status::Check,
        // fewer than you said you want to keep
        (_, _) if item.target > 0.0 && item.quantity < item.target => Status::Check,
        _ => Status::Ok,
    }
}

/// Delete items by id (the Delete selected button).
pub fn delete_ids(items: Vec<Item>, ids: &[String]) -> Vec<Item> {
    items.into_iter().filter(|i| !ids.contains(&i.id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Item {
        Item { id: "1".into(), category: "Water".into(), name: "Water, \"1 gal\" jugs".into(),
               location: "Pantry".into(), quantity: 8.0, unit: "gal".into(), use_per_day: 2.0,
               use_by: "2027-11".into(), notes: "a, b".into(), ..Default::default() }
    }

    #[test]
    fn round_trips_with_quotes_and_commas() {
        let items = vec![sample()];
        let csv = serialize(&items);
        assert_eq!(parse(&csv).unwrap(), items);
    }

    #[test]
    fn days_left_and_status() {
        let i = sample();
        assert_eq!(days_left(&i), Some(4.0));
        assert_eq!(status(&i, 2026, 10), Status::ActNow);
    }

    #[test]
    fn expiry_status() {
        let mut i = sample();
        i.use_per_day = 0.0;
        assert_eq!(status(&i, 2027, 12), Status::ActNow);
        assert_eq!(status(&i, 2027, 9), Status::Check);
        assert_eq!(status(&i, 2026, 1), Status::Ok);
    }

    #[test]
    fn owner_thresholds_move_the_lines() {
        let mut i = sample(); // 8 gal at 2 a day: 4 days
        // 4 days is "act now" by default, but only "check" if act-now is set to 3 days
        let calm = Thresholds { act_days: 3.0, check_days: 10.0, check_months: 3 };
        assert_eq!(status_with(&i, 2026, 10, &Thresholds::default()), Status::ActNow);
        assert_eq!(status_with(&i, 2026, 10, &calm), Status::Check);
        i.quantity = 40.0; // 20 days
        assert_eq!(status_with(&i, 2026, 10, &Thresholds::default()), Status::Ok);
        assert_eq!(status_with(&i, 2026, 10, &Thresholds { act_days: 7.0, check_days: 30.0, check_months: 3 }), Status::Check);
        // the use-by window: 2027-11 is 13 months from 2026-10
        i.use_per_day = 0.0;
        assert_eq!(status_with(&i, 2026, 10, &Thresholds { act_days: 7.0, check_days: 14.0, check_months: 12 }), Status::Ok);
        assert_eq!(status_with(&i, 2026, 10, &Thresholds { act_days: 7.0, check_days: 14.0, check_months: 13 }), Status::Check);
    }

    #[test]
    fn delete_removes_selected_only() {
        let mut b = sample();
        b.id = "2".into();
        let left = delete_ids(vec![sample(), b], &["1".to_string()]);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "2");
    }

    #[test]
    fn the_extra_questions_round_trip_and_old_files_still_load() {
        let mut i = sample();
        i.brand = "Acme, Inc".into();
        i.size = "1 gal".into();
        i.for_whom = "Baby".into();
        i.bought = "2026-05".into();
        i.target = 12.0;
        i.condition = "good".into();
        i.checked = "2026-10-01".into();
        i.cost = 18.5;
        i.source = "Costco".into();
        i.serial = "SN \"7\"".into();
        assert_eq!(parse(&serialize(&[i.clone()])).unwrap(), vec![i]);
        // a file from before the extra columns
        let old = "id,category,name,location,quantity,unit,use_per_day,use_by,notes\n1,Water,Jug,Pantry,3,gal,1,,\n";
        let p = parse(old).unwrap();
        assert_eq!((p[0].quantity, p[0].target, p[0].brand.as_str()), (3.0, 0.0, ""));
        assert!(parse("id\n1,Water,Jug,Pantry,3,gal,1,,,,,,,abc\n").is_err());
    }

    #[test]
    fn fewer_than_the_target_is_a_check_and_none_is_act_now() {
        let mut i = sample();
        i.use_per_day = 0.0;
        i.use_by = String::new();
        i.target = 10.0; // has 8
        assert_eq!(status(&i, 2026, 10), Status::Check);
        i.quantity = 10.0;
        assert_eq!(status(&i, 2026, 10), Status::Ok);
        i.quantity = 0.0;
        assert_eq!(status(&i, 2026, 10), Status::ActNow);
        i.target = 0.0; // no target set: nothing to compare with
        assert_eq!(status(&i, 2026, 10), Status::Ok);
    }

    #[test]
    fn bad_row_is_an_error() {
        assert!(parse("id,category\n1,Water\n").is_err());
    }
}
