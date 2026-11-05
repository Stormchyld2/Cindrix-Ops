//! Places that sit inside places: "Garage > Shelf 2 > Black pail".
//!
//! An inventory line's `location` is still one piece of text, so every older file reads exactly as before. When the
//! person writes it as a trail with `>` between the parts, the screens can show where something is down to the bin,
//! list what is inside a shelf, and answer "where is it?". See docs/42.

use crate::inventory::Item;
use std::collections::BTreeMap;

pub const SEP: &str = " > ";

/// The parts of a place trail. Only `>`, `›`, `→` and `»` separate parts (a "/" stays part of a name,
/// as in "Go-bag / car"). Spaces are tidied and empty parts dropped.
pub fn parts(place: &str) -> Vec<String> {
    place
        .split(|c| matches!(c, '>' | '›' | '→' | '»'))
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect()
}

/// The trail written the standard way: "Garage > Shelf 2 > Black pail". Text with no separator comes back tidied
/// but otherwise unchanged.
pub fn normalize(place: &str) -> String {
    parts(place).join(SEP)
}

/// The first part: the room or building.
pub fn top(place: &str) -> String {
    parts(place).into_iter().next().unwrap_or_default()
}

fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// True when `place` is `inside` or somewhere within it (the match is by whole parts, ignoring capitals).
pub fn is_inside(place: &str, inside: &str) -> bool {
    let (p, i) = (parts(place), parts(inside));
    !i.is_empty() && p.len() >= i.len() && p.iter().zip(&i).all(|(a, b)| same(a, b))
}

/// One place in the list of everywhere something is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// The whole trail, "Garage > Shelf 2".
    pub path: String,
    /// Just the last part, "Shelf 2".
    pub name: String,
    /// 0 for a room, 1 for a shelf in it, and so on.
    pub depth: usize,
    /// Inventory lines kept right here.
    pub here: usize,
    /// Inventory lines kept here or anywhere inside.
    pub inside: usize,
}

/// Every place named in the inventory, with the places above it, sorted so a room comes before what is in it.
pub fn places(items: &[Item]) -> Vec<Place> {
    // key: lower-case trail; value: (trail as first written, here, inside)
    let mut map: BTreeMap<Vec<String>, (Vec<String>, usize, usize)> = BTreeMap::new();
    for it in items {
        let p = parts(&it.location);
        for n in 1..=p.len() {
            let key: Vec<String> = p[..n].iter().map(|s| s.to_lowercase()).collect();
            let e = map.entry(key).or_insert_with(|| (p[..n].to_vec(), 0, 0));
            e.2 += 1;
            if n == p.len() {
                e.1 += 1;
            }
        }
    }
    map.into_values()
        .map(|(trail, here, inside)| Place {
            path: trail.join(SEP),
            name: trail.last().cloned().unwrap_or_default(),
            depth: trail.len() - 1,
            here,
            inside,
        })
        .collect()
}

/// The places one step inside `inside` (the rooms when `inside` is empty), for drilling down.
pub fn children(items: &[Item], inside: &str) -> Vec<Place> {
    let depth = parts(inside).len();
    places(items).into_iter().filter(|p| p.depth == depth && (depth == 0 || is_inside(&p.path, inside))).collect()
}

/// One answer to "where is it?".
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub id: String,
    pub name: String,
    pub place: String,
    pub quantity: f64,
    pub unit: String,
}

/// Finds inventory lines whose name, brand, size, notes or place contains every word of `query`
/// (so "beans garage" finds beans kept in the garage, and "black pail" lists what is in that pail).
pub fn where_is(items: &[Item], query: &str) -> Vec<Found> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(|w| w.to_string()).collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Found> = items
        .iter()
        .filter(|i| {
            let hay = format!("{} {} {} {} {}", i.name, i.brand, i.size, i.notes, normalize(&i.location)).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .map(|i| Found { id: i.id.clone(), name: i.name.clone(), place: normalize(&i.location), quantity: i.quantity, unit: i.unit.clone() })
        .collect();
    out.sort_by(|a, b| a.place.to_lowercase().cmp(&b.place.to_lowercase()).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(id: &str, name: &str, place: &str) -> Item {
        Item { id: id.into(), name: name.into(), location: place.into(), quantity: 2.0, unit: "bag".into(), ..Default::default() }
    }

    fn sample() -> Vec<Item> {
        vec![
            it("1", "Pinto beans", "Garage > Shelf 2 > Black pail"),
            it("2", "White rice", "Garage > Shelf 2 > Black pail"),
            it("3", "Canned corn", "Garage > Shelf 1"),
            it("4", "AA batteries", "garage > shelf 2"),
            it("5", "Flour", "Pantry"),
            it("6", "Water jugs", ""),
            it("7", "Go-bag", "Hall closet / car"),
        ]
    }

    #[test]
    fn splits_and_tidies_trails() {
        assert_eq!(parts("Garage > Shelf 2 > Black pail"), vec!["Garage", "Shelf 2", "Black pail"]);
        assert_eq!(parts("  Garage>Shelf   2 → Pail "), vec!["Garage", "Shelf 2", "Pail"]);
        assert_eq!(normalize("Garage>Shelf 2"), "Garage > Shelf 2");
        assert_eq!(normalize("Pantry"), "Pantry");
        assert_eq!(normalize(" > > "), "");
        assert_eq!(normalize(""), "");
        // a slash stays part of a name
        assert_eq!(parts("Hall closet / car"), vec!["Hall closet / car"]);
        assert_eq!(top("Garage > Shelf 2"), "Garage");
        assert_eq!(top(""), "");
    }

    #[test]
    fn inside_means_whole_parts_ignoring_capitals() {
        assert!(is_inside("Garage > Shelf 2 > Pail", "Garage"));
        assert!(is_inside("garage > SHELF 2", "Garage > Shelf 2"));
        assert!(is_inside("Garage", "Garage"));
        assert!(!is_inside("Garage > Shelf 2", "Garage > Shelf 1"));
        assert!(!is_inside("Garage sale > Box", "Garage"));
        assert!(!is_inside("Garage", "Garage > Shelf 2"));
        assert!(!is_inside("Garage", ""));
    }

    #[test]
    fn lists_every_place_with_counts_and_merges_capitals() {
        let p = places(&sample());
        let find = |path: &str| p.iter().find(|x| x.path == path).unwrap_or_else(|| panic!("no {}: {:?}", path, p));
        assert_eq!(find("Garage").inside, 4);
        assert_eq!(find("Garage").here, 0);
        assert_eq!(find("Garage > Shelf 2").inside, 3);   // two in the pail and the batteries written in lower case
        assert_eq!(find("Garage > Shelf 2").here, 1);
        assert_eq!(find("Garage > Shelf 2 > Black pail").here, 2);
        assert_eq!(find("Garage > Shelf 2 > Black pail").depth, 2);
        assert_eq!(find("Garage > Shelf 2 > Black pail").name, "Black pail");
        assert_eq!(find("Pantry").here, 1);
        assert!(p.iter().all(|x| !x.path.is_empty()), "empty place listed");
        // a room is listed before what is inside it
        let at = |path: &str| p.iter().position(|x| x.path == path).unwrap();
        assert!(at("Garage") < at("Garage > Shelf 1") && at("Garage > Shelf 1") < at("Garage > Shelf 2"));
    }

    #[test]
    fn drilling_down_lists_one_step_at_a_time() {
        let s = sample();
        let names = |v: Vec<Place>| v.into_iter().map(|p| p.name).collect::<Vec<_>>();
        assert_eq!(names(children(&s, "")), vec!["Garage", "Hall closet / car", "Pantry"]);
        assert_eq!(names(children(&s, "Garage")), vec!["Shelf 1", "Shelf 2"]);
        assert_eq!(names(children(&s, "Garage > Shelf 2")), vec!["Black pail"]);
        assert!(children(&s, "Garage > Shelf 2 > Black pail").is_empty());
        assert!(children(&s, "Basement").is_empty());
    }

    #[test]
    fn where_is_finds_by_item_or_container() {
        let s = sample();
        let r = where_is(&s, "beans");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].place, "Garage > Shelf 2 > Black pail");
        // asking for a container lists what is in it
        let r = where_is(&s, "black pail");
        assert_eq!(r.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), vec!["Pinto beans", "White rice"]);
        // every word must match
        assert_eq!(where_is(&s, "rice garage").len(), 1);
        assert!(where_is(&s, "rice pantry").is_empty());
        assert!(where_is(&s, "   ").is_empty());
        assert!(where_is(&s, "zzz").is_empty());
        // an item with no place still turns up, with an empty place
        assert_eq!(where_is(&s, "water")[0].place, "");
    }
}
