//! A plain guess at how long food lasts, used to fill in "use by" from the month it was bought or packed.
//!
//! The numbers are in `content/shelf-life.csv` (store-bought, kept in a cupboard around 70°F, from Kansas State
//! University Extension; sealed long-term storage, from a published long-term food storage chart). The shorter
//! end of every range is used, so the guess leans toward "use it sooner". It is a guess to plan with, never a
//! promise: the date printed on a package always wins. Medicine is not in the table and never will be.
//! See docs/42.

use crate::inventory::split_record;
use std::sync::OnceLock;

const TABLE: &str = include_str!("../../content/shelf-life.csv");

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Lower case words that point to this food. The longest one found in the item's name wins.
    pub words: Vec<String>,
    /// Canned goods only match when the name says "canned" (so "black beans" and "canned black beans" differ).
    pub canned: bool,
    pub food: String,
    /// Months, store-bought and kept in the cupboard. None when the table has no such figure.
    pub store: Option<u32>,
    /// Months, sealed for long-term storage (see `is_long_term`). None when the table has no such figure.
    pub long: Option<u32>,
    pub note: String,
}

/// The ways of packing that the screen offers, as (code, plain words).
pub const PACKAGING: &[(&str, &str)] = &[
    ("mylar", "Foil (mylar) pouch"),
    ("o2", "Oxygen absorber inside"),
    ("vacuum", "Vacuum sealed"),
    ("sealed-can", "Sealed metal can"),
    ("jar", "Glass jar with a gasket lid"),
    ("bucket", "Food-grade bucket with a gasket lid"),
    ("desiccant", "Desiccant packet"),
];

pub fn table() -> &'static [Entry] {
    static T: OnceLock<Vec<Entry>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = Vec::new();
        for (n, line) in TABLE.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if n == 0 || line.trim().is_empty() {
                continue;
            }
            let f = split_record(line);
            if f.len() < 6 {
                continue;
            }
            let months = |s: &str| s.trim().parse::<u32>().ok().filter(|m| *m > 0);
            out.push(Entry {
                words: f[0].split('|').map(|w| w.trim().to_lowercase()).filter(|w| !w.is_empty()).collect(),
                canned: f[1].trim() == "canned",
                food: f[2].trim().to_string(),
                store: months(&f[3]),
                long: months(&f[4]),
                note: f[5].trim().to_string(),
            });
        }
        out
    })
}

/// True when `needle` appears in `hay` as whole words ("rice" is in "white rice" but not in "price list").
fn has_word(hay: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(at) = hay[from..].find(needle) {
        let s = from + at;
        let e = s + needle.len();
        let before = hay[..s].chars().next_back().map_or(true, |c| !c.is_alphanumeric());
        let after = hay[e..].chars().next().map_or(true, |c| !c.is_alphanumeric());
        if before && after {
            return true;
        }
        from = s + needle.len().max(1);
        if from >= hay.len() {
            break;
        }
    }
    false
}

fn says_canned(name: &str) -> bool {
    ["canned", "tinned", "can of", "cans of"].iter().any(|w| has_word(name, w))
}

/// The table row that fits an item's name best, or None.
pub fn find(name: &str) -> Option<&'static Entry> {
    let name = name.to_lowercase();
    // A name that says "canned" is looked up among the canned rows first, so "canned black beans" is not "dry beans".
    if says_canned(&name) {
        if let Some(e) = best_match(&name, true) {
            return Some(e);
        }
    }
    best_match(&name, false)
}

/// The row with the longest matching word. Canned rows are used only when `canned_only` is set.
fn best_match(name: &str, canned_only: bool) -> Option<&'static Entry> {
    let mut best: Option<(&Entry, usize)> = None;
    for e in table() {
        if e.canned != canned_only {
            continue;
        }
        for w in &e.words {
            if has_word(name, w) && best.map_or(true, |(_, len)| w.chars().count() > len) {
                best = Some((e, w.chars().count()));
            }
        }
    }
    best.map(|(e, _)| e)
}

/// The packaging codes in a saved value (commas, semicolons or spaces between them), lower case.
pub fn codes(packaging: &str) -> Vec<String> {
    packaging.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty()).collect()
}

/// Known codes only, once each, in the order the screen lists them. Used when saving.
pub fn normalize_packaging(packaging: &str) -> String {
    let have = codes(packaging);
    PACKAGING.iter().filter(|(c, _)| have.iter().any(|h| h == c)).map(|(c, _)| *c).collect::<Vec<_>>().join(",")
}

/// Sealed so that oxygen is kept out: a sealed can, a vacuum seal, or an oxygen absorber in a foil pouch or glass jar.
/// (An oxygen absorber does not work in a plain bucket, so a bucket alone does not count.)
pub fn is_long_term(packaging: &str) -> bool {
    let c = codes(packaging);
    let has = |k: &str| c.iter().any(|x| x == k);
    has("sealed-can") || has("vacuum") || (has("o2") && (has("mylar") || has("jar")))
}

/// What the table says about an item, in words a person can use.
#[derive(Debug, Clone, PartialEq)]
pub struct Guess {
    pub food: String,
    /// None when the table has no figure for how this item is packed (for example whole wheat that is not sealed).
    pub months: Option<u32>,
    pub long_term: bool,
    /// One plain sentence saying where the number comes from.
    pub basis: String,
    /// Something useful to know, or empty.
    pub hint: String,
}

/// "1 year", "18 months", "30 years".
pub fn span(months: u32) -> String {
    match months {
        12 => "1 year".to_string(),
        m if m >= 24 && m % 12 == 0 => format!("{} years", m / 12),
        1 => "1 month".to_string(),
        m => format!("{} months", m),
    }
}

pub fn guess(name: &str, packaging: &str) -> Option<Guess> {
    let e = find(name)?;
    let sealed = is_long_term(packaging);
    let mut hint = String::new();
    if !sealed && codes(packaging).iter().any(|c| c == "bucket") && codes(packaging).iter().any(|c| c == "o2") {
        hint = "Oxygen absorbers do not work in a plain bucket. Put the food in foil pouches with an absorber first, then in the bucket.".to_string();
    }
    let (months, long_term) = match (sealed, e.long, e.store) {
        (true, Some(l), _) => (Some(l), true),
        (_, _, Some(s)) => (Some(s), false),
        _ => (None, false),
    };
    let basis = match (months, long_term) {
        (Some(m), true) => format!("{}, sealed for long-term storage and kept cool (75°F or lower) and dry: about {} (long-term storage chart).", e.food, span(m)),
        (Some(m), false) => format!("{}, store-bought and kept in a cupboard: about {} (Kansas State University Extension).", e.food, span(m)),
        (None, _) => format!("{} keeps a long time only when it is sealed (a foil pouch with an oxygen absorber, or a sealed can).", e.food),
    };
    if hint.is_empty() {
        hint = if long_term {
            if e.note.is_empty() { "Keep it cool, dry and off the floor.".to_string() } else { format!("{} Keep it cool, dry and off the floor.", e.note) }
        } else if let (Some(l), Some(_)) = (e.long, e.store) {
            format!("Sealed for long-term storage (a foil pouch with an oxygen absorber) it can keep about {}.", span(l))
        } else if months.is_some() {
            "The date printed on the package comes first. Use whichever is earlier.".to_string()
        } else {
            String::new()
        };
    }
    Some(Guess { food: e.food.clone(), months, long_term, basis, hint })
}

/// "YYYY-MM" plus a number of months, as "YYYY-MM". None when the start is not a month.
pub fn add_months(ym: &str, months: u32) -> Option<String> {
    let (y, m) = ym.trim().split_once('-')?;
    let (y, m): (i64, i64) = (y.trim().parse().ok()?, m.trim().parse().ok()?);
    if !(1..=12).contains(&m) || !(1900..=3000).contains(&y) {
        return None;
    }
    let total = y * 12 + (m - 1) + months as i64;
    Some(format!("{:04}-{:02}", total / 12, total % 12 + 1))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    /// "YYYY-MM"
    pub use_by: String,
    pub guess: Guess,
}

/// A use-by month worked out from the month it was bought or packed. None when there is no figure for this item.
pub fn suggest_use_by(name: &str, packaging: &str, bought: &str) -> Option<Suggestion> {
    let g = guess(name, packaging)?;
    let use_by = add_months(bought, g.months?)?;
    Some(Suggestion { use_by, guess: g })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sound() {
        let t = table();
        assert!(t.len() >= 60, "{} rows", t.len());
        let mut foods = std::collections::HashSet::new();
        for e in t {
            assert!(!e.words.is_empty(), "{} has no words", e.food);
            assert!(e.store.is_some() || e.long.is_some(), "{} has no figure", e.food);
            assert!(foods.insert(e.food.clone()), "{} appears twice", e.food);
            for m in [e.store, e.long].into_iter().flatten() {
                assert!((1..=400).contains(&m), "{}: {} months", e.food, m);
            }
            // a sealed figure is never shorter than the store-bought one
            if let (Some(s), Some(l)) = (e.store, e.long) {
                assert!(l >= s, "{}: sealed {} < store {}", e.food, l, s);
            }
            for w in &e.words {
                assert_eq!(w, &w.to_lowercase());
            }
        }
    }

    #[test]
    fn medicine_is_not_in_the_table() {
        for e in table() {
            let all = format!("{} {}", e.words.join(" "), e.food).to_lowercase();
            for bad in ["insulin", "tablet", "pill", "medicine", "ibuprofen", "aspirin", "antibiotic"] {
                assert!(!all.contains(bad), "{} mentions {}", e.food, bad);
            }
        }
    }

    #[test]
    fn matches_the_longest_word_and_whole_words_only() {
        assert_eq!(find("Brown rice, 5 lb").unwrap().food, "Brown rice");
        assert_eq!(find("White Rice").unwrap().food, "White rice");
        assert_eq!(find("Rice").unwrap().food, "White rice");
        assert_eq!(find("Flavored rice mix").unwrap().food, "Flavored rice or rice mix");
        assert_eq!(find("brown sugar").unwrap().food, "Brown sugar");
        assert_eq!(find("Sugar, granulated").unwrap().food, "Sugar");
        assert_eq!(find("Powdered sugar").unwrap().food, "Powdered sugar");
        assert_eq!(find("Powdered milk").unwrap().food, "Powdered milk (nonfat)");
        assert!(find("Price list").is_none(), "rice inside price");
        assert!(find("Wrench").is_none());
        assert!(find("").is_none());
    }

    #[test]
    fn canned_goods_need_the_word_canned() {
        assert_eq!(find("Black beans").unwrap().food, "Dry beans and peas");
        assert_eq!(find("Canned black beans").unwrap().food, "Canned food (unopened)");
        assert_eq!(find("Canned fruit juice").unwrap().food, "Canned fruit juice");
        assert_eq!(find("Cans of soup").unwrap().food, "Canned food (unopened)");
    }

    #[test]
    fn sealed_long_term_only_with_oxygen_kept_out() {
        assert!(!is_long_term(""));
        assert!(!is_long_term("mylar"));
        assert!(!is_long_term("bucket,o2"));
        assert!(!is_long_term("desiccant"));
        assert!(is_long_term("mylar,o2"));
        assert!(is_long_term("jar,o2"));
        assert!(is_long_term("vacuum"));
        assert!(is_long_term("sealed-can"));
        assert!(is_long_term("MYLAR; O2"));
    }

    #[test]
    fn guess_uses_store_or_sealed_numbers() {
        let g = guess("White rice", "").unwrap();
        assert_eq!((g.months, g.long_term), (Some(12), false));
        assert!(g.basis.contains("store-bought"));
        assert!(g.hint.contains("30 years"), "{}", g.hint);
        let g = guess("White rice", "mylar,o2").unwrap();
        assert_eq!((g.months, g.long_term), (Some(360), true));
        assert!(g.basis.contains("30 years") && g.basis.contains("75°F"));
        // no store figure for whole wheat: only a sealed one
        let g = guess("Hard red wheat", "").unwrap();
        assert_eq!(g.months, None);
        assert!(g.basis.contains("sealed"));
        assert_eq!(guess("Hard red wheat", "mylar,o2").unwrap().months, Some(360));
        // a canned food is never given a 30-year figure
        assert_eq!(guess("Canned corn", "sealed-can").unwrap().months, Some(12));
        assert!(guess("Wrench", "").is_none());
    }

    #[test]
    fn bucket_with_absorber_gets_a_warning_not_a_long_guess() {
        let g = guess("Pinto beans", "bucket,o2").unwrap();
        assert_eq!((g.months, g.long_term), (Some(12), false));
        assert!(g.hint.contains("plain bucket"));
    }

    #[test]
    fn months_add_across_years() {
        assert_eq!(add_months("2026-11", 1).as_deref(), Some("2026-12"));
        assert_eq!(add_months("2026-11", 2).as_deref(), Some("2027-01"));
        assert_eq!(add_months("2026-12", 12).as_deref(), Some("2027-12"));
        assert_eq!(add_months("2026-01", 360).as_deref(), Some("2056-01"));
        assert_eq!(add_months(" 2026-03 ", 0).as_deref(), Some("2026-03"));
        assert_eq!(add_months("2026-13", 1), None);
        assert_eq!(add_months("soon", 1), None);
        assert_eq!(add_months("", 1), None);
    }

    #[test]
    fn use_by_comes_from_the_bought_month() {
        let s = suggest_use_by("Rolled oats", "mylar,o2", "2026-11").unwrap();
        assert_eq!(s.use_by, "2056-11");
        assert!(s.guess.long_term);
        let s = suggest_use_by("Flour", "", "2026-11").unwrap();
        assert_eq!(s.use_by, "2027-05");
        assert!(suggest_use_by("Flour", "", "").is_none());
        assert!(suggest_use_by("Hard red wheat", "", "2026-11").is_none());
        assert!(suggest_use_by("Wrench", "", "2026-11").is_none());
    }

    #[test]
    fn packaging_is_cleaned_when_saved() {
        assert_eq!(normalize_packaging("O2; mylar, mylar, junk"), "mylar,o2");
        assert_eq!(normalize_packaging(""), "");
        assert_eq!(normalize_packaging("bucket jar"), "jar,bucket");
    }

    #[test]
    fn spans_read_naturally() {
        assert_eq!(span(1), "1 month");
        assert_eq!(span(6), "6 months");
        assert_eq!(span(12), "1 year");
        assert_eq!(span(18), "18 months");
        assert_eq!(span(24), "2 years");
        assert_eq!(span(360), "30 years");
    }
}
