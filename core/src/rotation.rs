//! Garden beds and crop rotation. Each bed keeps a history of what grew in it, and the checker warns when a
//! plant family would follow itself too soon (which builds up that family's pests and diseases) and suggests
//! what to plant next.
//!
//! The idea of tracking per-bed crop history to check rotation was inspired by how the open-source garden
//! planner Brinjel describes its features. No Brinjel code or text is used; this is written from scratch from
//! the rules in `content/guides/planting-calendar/succession-and-crop-rotation.md`.
//!
//! Stored as `user/beds.csv`: one row per bed per year.

use crate::inventory::{quote, split_record};

pub const HEADER: &str = "bed,year,crop,notes";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Family {
    Nightshade,
    Brassica,
    Legume,
    Cucurbit,
    Allium,
    Root,
    Leafy,
    Grass,
    Other,
}

impl Family {
    pub fn name(self) -> &'static str {
        match self {
            Family::Nightshade => "nightshade (tomato, pepper, potato, eggplant)",
            Family::Brassica => "cabbage family",
            Family::Legume => "bean and pea family",
            Family::Cucurbit => "squash and cucumber family",
            Family::Allium => "onion family",
            Family::Root => "root crops (carrot, beet, parsnip)",
            Family::Leafy => "leafy greens (lettuce, spinach, chard)",
            Family::Grass => "corn and grains",
            Family::Other => "other",
        }
    }
}

/// The plant family of a crop, from its common name.
pub fn family_of(crop: &str) -> Family {
    let c = crop.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| c.contains(x));
    if has(&["tomato", "pepper", "chili", "chile", "potato", "eggplant", "tomatillo"]) {
        Family::Nightshade
    } else if has(&["cabbage", "broccoli", "cauliflower", "kale", "kohlrabi", "brussels", "turnip", "radish", "mustard", "collard", "rutabaga", "bok choy"]) {
        Family::Brassica
    } else if has(&["bean", "pea", "lentil", "chickpea", "soybean", "cowpea", "fava", "peanut", "clover", "vetch"]) {
        Family::Legume
    } else if has(&["squash", "zucchini", "pumpkin", "cucumber", "melon", "gourd", "luffa"]) {
        Family::Cucurbit
    } else if has(&["onion", "garlic", "leek", "shallot", "chive"]) {
        Family::Allium
    } else if has(&["carrot", "beet", "parsnip", "celeriac", "salsify"]) {
        Family::Root
    } else if has(&["lettuce", "spinach", "chard", "arugula", "endive", "celery"]) {
        Family::Leafy
    } else if has(&["corn", "wheat", "oat", "rye", "barley", "millet", "sorghum", "rice"]) {
        Family::Grass
    } else {
        Family::Other
    }
}

/// Years a family should stay out of a bed before coming back (heavier pest and disease families wait longer).
pub fn rest_years(f: Family) -> i32 {
    match f {
        Family::Nightshade | Family::Brassica | Family::Allium => 3,
        Family::Cucurbit | Family::Root => 2,
        Family::Legume | Family::Leafy | Family::Grass => 1,
        Family::Other => 0,
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Planting {
    pub bed: String,
    pub year: i32,
    pub crop: String,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub bed: String,
    pub year: i32,
    pub crop: String,
    pub earlier_year: i32,
    pub earlier_crop: String,
    pub family: Family,
    pub wait_until: i32,
}

/// Would planting `crop` in `bed` in `year` come back to the same family too soon? Returns the clash, if any.
pub fn check(history: &[Planting], bed: &str, year: i32, crop: &str) -> Option<Conflict> {
    let fam = family_of(crop);
    let wait = rest_years(fam);
    if wait == 0 {
        return None;
    }
    history
        .iter()
        .filter(|p| p.bed.eq_ignore_ascii_case(bed) && p.year < year && family_of(&p.crop) == fam && year - p.year <= wait)
        .max_by_key(|p| p.year)
        .map(|p| Conflict {
            bed: bed.to_string(),
            year,
            crop: crop.to_string(),
            earlier_year: p.year,
            earlier_crop: p.crop.clone(),
            family: fam,
            wait_until: p.year + wait + 1,
        })
}

/// Check every planting against the ones before it.
pub fn check_all(history: &[Planting]) -> Vec<Conflict> {
    let mut sorted = history.to_vec();
    sorted.sort_by_key(|p| p.year);
    let mut out = Vec::new();
    for (i, p) in sorted.iter().enumerate() {
        if let Some(c) = check(&sorted[..i], &p.bed, p.year, &p.crop) {
            out.push(c);
        }
    }
    out
}

/// A simple next-crop suggestion after `last` crop: legumes feed hungry crops, hungry crops are followed by roots
/// or leafy greens, and so on. Returns families in order of preference.
pub fn next_after(last: Family) -> Vec<Family> {
    match last {
        Family::Legume => vec![Family::Brassica, Family::Grass, Family::Leafy],
        Family::Brassica => vec![Family::Root, Family::Allium, Family::Legume],
        Family::Nightshade => vec![Family::Legume, Family::Allium, Family::Root],
        Family::Cucurbit => vec![Family::Legume, Family::Leafy, Family::Allium],
        Family::Root => vec![Family::Legume, Family::Leafy, Family::Grass],
        Family::Allium => vec![Family::Legume, Family::Cucurbit, Family::Leafy],
        Family::Leafy => vec![Family::Nightshade, Family::Cucurbit, Family::Brassica],
        Family::Grass => vec![Family::Legume, Family::Cucurbit, Family::Root],
        Family::Other => vec![Family::Legume, Family::Leafy, Family::Root],
    }
}

/// Suggest what to plant in a bed in `year`, skipping any family still resting there.
pub fn suggest(history: &[Planting], bed: &str, year: i32) -> Vec<Family> {
    let last = history.iter().filter(|p| p.bed.eq_ignore_ascii_case(bed) && p.year < year).max_by_key(|p| p.year);
    let order = match last {
        Some(p) => next_after(family_of(&p.crop)),
        None => vec![Family::Legume, Family::Leafy, Family::Root],
    };
    let ok = |f: Family| {
        let wait = rest_years(f);
        !history.iter().any(|p| p.bed.eq_ignore_ascii_case(bed) && p.year < year && family_of(&p.crop) == f && year - p.year <= wait)
    };
    order.into_iter().filter(|f| ok(*f)).collect()
}

pub fn parse(csv: &str) -> Vec<Planting> {
    csv.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let f = split_record(l);
            let year = f.get(1)?.trim().parse().ok()?;
            Some(Planting { bed: f.first()?.trim().to_string(), year, crop: f.get(2)?.trim().to_string(), notes: f.get(3).cloned().unwrap_or_default() })
        })
        .collect()
}

pub fn to_csv(history: &[Planting]) -> String {
    let mut s = String::from(HEADER);
    s.push('\n');
    for p in history {
        s.push_str(&format!("{},{},{},{}\n", quote(&p.bed), p.year, quote(&p.crop), quote(&p.notes)));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(bed: &str, year: i32, crop: &str) -> Planting {
        Planting { bed: bed.into(), year, crop: crop.into(), notes: String::new() }
    }

    #[test]
    fn sorts_crops_into_families() {
        assert_eq!(family_of("Roma Tomato"), Family::Nightshade);
        assert_eq!(family_of("Kale"), Family::Brassica);
        assert_eq!(family_of("Pole bean"), Family::Legume);
        assert_eq!(family_of("Sugar pumpkin"), Family::Cucurbit);
        assert_eq!(family_of("Yellow onion"), Family::Allium);
        assert_eq!(family_of("Danvers carrot"), Family::Root);
        assert_eq!(family_of("Romaine lettuce"), Family::Leafy);
        assert_eq!(family_of("Sweet corn"), Family::Grass);
        assert_eq!(family_of("Marigold"), Family::Other);
    }

    #[test]
    fn tomato_after_pepper_next_year_is_a_clash() {
        let h = vec![p("A", 2024, "Pepper")];
        let c = check(&h, "A", 2025, "Tomato").expect("clash");
        assert_eq!(c.earlier_crop, "Pepper");
        assert_eq!(c.wait_until, 2028);
    }

    #[test]
    fn same_family_is_fine_after_the_rest_period() {
        let h = vec![p("A", 2020, "Tomato")];
        assert!(check(&h, "A", 2024, "Tomato").is_none());
        assert!(check(&h, "A", 2023, "Tomato").is_some());
    }

    #[test]
    fn another_bed_does_not_matter() {
        let h = vec![p("A", 2024, "Tomato")];
        assert!(check(&h, "B", 2025, "Tomato").is_none());
    }

    #[test]
    fn bed_names_ignore_case() {
        let h = vec![p("bed 1", 2024, "Cabbage")];
        assert!(check(&h, "Bed 1", 2025, "Broccoli").is_some());
    }

    #[test]
    fn check_all_finds_every_clash_in_order() {
        let h = vec![p("A", 2024, "Tomato"), p("A", 2025, "Pepper"), p("A", 2026, "Beans")];
        let c = check_all(&h);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].year, 2025);
    }

    #[test]
    fn suggestions_skip_resting_families() {
        let h = vec![p("A", 2024, "Beans"), p("A", 2023, "Kale")];
        let s = suggest(&h, "A", 2025);
        assert!(!s.contains(&Family::Brassica));
        assert!(s.contains(&Family::Grass));
    }

    #[test]
    fn empty_bed_starts_with_a_legume() {
        assert_eq!(suggest(&[], "New", 2026)[0], Family::Legume);
    }

    #[test]
    fn csv_round_trips() {
        let h = vec![p("A", 2024, "Tomato"), Planting { bed: "B, east".into(), year: 2025, crop: "Beans".into(), notes: "needs \"stakes\"".into() }];
        assert_eq!(parse(&to_csv(&h)), h);
    }
}
