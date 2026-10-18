//! "What fails next": which supplies run out first, from the real inventory.
//! An estimate from the numbers entered, not a prediction.

use crate::inventory::{months_until, Item};

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

struct Group {
    name: String,
    category: String,
    quantity: f64,
    per_day: f64,
    places: Vec<String>,
}

/// Rows with the same name are one supply kept in several places. Quantities are added;
/// the daily use is the largest value entered on any of those rows.
fn groups(items: &[Item]) -> Vec<Group> {
    let mut v: Vec<Group> = Vec::new();
    for i in items {
        match v.iter_mut().find(|g| g.name == i.name) {
            Some(g) => {
                g.quantity += i.quantity;
                if i.use_per_day > g.per_day {
                    g.per_day = i.use_per_day;
                }
                if !g.places.contains(&i.location) {
                    g.places.push(i.location.clone());
                }
            }
            None => v.push(Group {
                name: i.name.clone(),
                category: i.category.clone(),
                quantity: i.quantity,
                per_day: i.use_per_day,
                places: vec![i.location.clone()],
            }),
        }
    }
    v
}

/// Supplies used up daily, soonest to run out first.
pub fn runs_out_first(items: &[Item]) -> Vec<RunsOut> {
    let mut v: Vec<RunsOut> = groups(items)
        .into_iter()
        .filter(|g| g.per_day > 0.0)
        .map(|g| RunsOut {
            days: g.quantity / g.per_day,
            location: if g.places.len() > 1 { "several places".to_string() } else { g.places[0].clone() },
            name: g.name,
        })
        .collect();
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

/// Days a category lasts: the item in it that runs out first.
pub fn category_days(items: &[Item], category: &str) -> Option<f64> {
    groups(items)
        .into_iter()
        .filter(|g| g.category == category && g.per_day > 0.0)
        .map(|g| g.quantity / g.per_day)
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
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
    fn category_is_the_item_that_runs_out_first() {
        let items = vec![
            it("W", "Water", 9.0, 3.0, "", ""),
            it("Tablets", "Water", 1.0, 1.0, "", ""),
            it("F", "Food", 1.0, 1.0, "", ""),
        ];
        assert_eq!(category_days(&items, "Water"), Some(1.0));
        assert_eq!(category_days(&items, "Tools"), None);
    }

    #[test]
    fn rows_without_daily_use_still_count_toward_the_same_supply() {
        let items = vec![it("Water", "Water", 6.0, 2.0, "", "Pantry"), it("Water", "Water", 3.0, 0.0, "", "Garage")];
        let r = runs_out_first(&items);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].days, 4.5);
    }
}
