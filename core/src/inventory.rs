//! Inventory stored as CSV. Medicine rows hold counts only, never doses.

use std::fmt;

pub const HEADER: &str = "id,category,name,location,quantity,unit,use_per_day,use_by,notes";

#[derive(Debug, Clone, PartialEq)]
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

fn split_record(line: &str) -> Vec<String> {
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

fn quote(s: &str) -> String {
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
        if f.len() < 9 {
            return Err(ParseError(format!("line {} has {} fields, expected 9", n + 1, f.len())));
        }
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
        });
    }
    Ok(items)
}

pub fn serialize(items: &[Item]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for i in items {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{}\n",
            quote(&i.id), quote(&i.category), quote(&i.name), quote(&i.location), i.quantity,
            quote(&i.unit), i.use_per_day, quote(&i.use_by), quote(&i.notes)
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

/// Status for the Inventory screen.
pub fn status(item: &Item, year: i32, month: u32) -> Status {
    let expiry = months_until(item, year, month);
    let days = days_left(item);
    match (expiry, days) {
        (Some(e), _) if e < 0 => Status::ActNow,
        (_, Some(d)) if d < 7.0 => Status::ActNow,
        (Some(e), _) if e <= 3 => Status::Check,
        (_, Some(d)) if d < 14.0 => Status::Check,
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
               use_by: "2027-11".into(), notes: "a, b".into() }
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
    fn delete_removes_selected_only() {
        let mut b = sample();
        b.id = "2".into();
        let left = delete_ids(vec![sample(), b], &["1".to_string()]);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, "2");
    }

    #[test]
    fn bad_row_is_an_error() {
        assert!(parse("id,category\n1,Water\n").is_err());
    }
}
