//! "What fails next": which supplies run out first, from the real inventory.
//! An estimate from the numbers entered, not a prediction.

use crate::inventory::{days_left, months_until, Item};

#[derive(Debug, Clone, PartialEq)]
pub struct RunsOut {
    pub name: String,
    pub location: String,
    pub days: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expiring {
    pub name: String,
    pub months: i32,
}

/// Items used up daily, soonest to run out first. Same-name items in different places are added together;
/// the daily use is the largest value entered on any of those rows.
pub fn runs_out_first(items: &[Item]) -> Vec<RunsOut> {
    let mut groups: Vec<(String, f64, f64)> = Vec::new(); // name, quantity, use per day
    for i in items.iter().filter(|i| i.use_per_day > 0.0) {
        match groups.iter_mut().find(|g| g.0 == i.name) {
            Some(g) => {
                g.1 += i.quantity;
                // The household's daily use is entered on any row of the item; the largest counts.
                if i.use_per_day > g.2 {
                    g.2 = i.use_per_day;
                }
            }
            None => groups.push((i.name.clone(), i.quantity, i.use_per_day)),
        }
    }
    let mut v: Vec<RunsOut> = groups
        .into_iter()
        .map(|(name, q, per)| RunsOut { name, location: String::new(), days: q / per })
        .collect();
    // Single-location items keep their place name.
    for r in v.iter_mut() {
        let places: Vec<&str> = items.iter().filter(|i| i.name == r.name).map(|i| i.location.as_str()).collect();
        r.location = if places.windows(2).all(|w| w[0] == w[1]) { places.first().copied().unwrap_or("").to_string() } else { "several places".to_string() };
    }
    v.sort_by(|a, b| a.days.partial_cmp(&b.days).unwrap_or(std::cmp::Ordering::Equal));
    v
}

/// Items past or within `within_months` of their use-by month.
pub fn expiring(items: &[Item], year: i32, month: u32, within_months: i32) -> Vec<Expiring> {
    let mut v: Vec<Expiring> = items
        .iter()
        .filter_map(|i| months_until(i, year, month).map(|m| Expiring { name: i.name.clone(), months: m }))
        .filter(|e| e.months <= within_months)
        .collect();
    v.sort_by_key(|e| e.months);
    v
}

/// Days of one category's supplies at the current use rate (all its daily-use items added).
pub fn category_days(items: &[Item], category: &str) -> Option<f64> {
    let (mut q, mut u) = (0.0, 0.0);
    for i in items.iter().filter(|i| i.category == category && days_left(i).is_some()) {
        q += i.quantity;
        u += i.use_per_day;
    }
    if u > 0.0 { Some(q / u) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(name: &str, cat: &str, q: f64, per: f64, use_by: &str, loc: &str) -> Item {
        Item { id: "1".into(), category: cat.into(), name: name.into(), location: loc.into(), quantity: q,
               unit: "x".into(), use_per_day: per, use_by: use_by.into(), notes: "".into() }
    }

    #[test]
    fn soonest_first_and_places_added() {
        let items = vec![
            it("Water", "Water", 6.0, 2.0, "", "Pantry"),
            it("Water", "Water", 3.0, 2.0, "", "Garage"),
            it("Strips", "Medical", 30.0, 4.0, "", "Pantry"),
        ];
        let r = runs_out_first(&items);
        assert_eq!(r[0].name, "Water");
        assert_eq!(r[0].days, 4.5);
        assert_eq!(r[0].location, "several places");
        assert_eq!(r[1].name, "Strips");
        assert_eq!(r[1].days, 7.5);
    }

    #[test]
    fn expiring_list() {
        let items = vec![it("A", "x", 1.0, 0.0, "2026-11", ""), it("B", "x", 1.0, 0.0, "2030-01", "")];
        let e = expiring(&items, 2026, 10, 3);
        assert_eq!(e, vec![Expiring { name: "A".into(), months: 1 }]);
    }

    #[test]
    fn category_total() {
        let items = vec![it("W", "Water", 9.0, 3.0, "", ""), it("F", "Food", 1.0, 1.0, "", "")];
        assert_eq!(category_days(&items, "Water"), Some(3.0));
        assert_eq!(category_days(&items, "Tools"), None);
    }
}
