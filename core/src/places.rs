//! Finding a place on the map with no internet: a plain text list of places kept on the drive
//! (`maps/places/places.tsv`, made by `scripts/get-maps.ps1 -Places`), plus the 50 states and
//! Washington DC, which are always known. Reads no network and no other file.
//!
//! One line of the list: `name TAB state TAB kind TAB latitude TAB longitude TAB population`.
//! Kinds: `city`, `county`, `zip`, `park`, `lake`, `peak`, `airport`, `hospital`, `state`.
//! Street addresses are not in this list; they need a much larger address file.

use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub name: String,
    /// Two-letter state code, or "" when there is none.
    pub state: String,
    pub kind: String,
    pub lat: f64,
    pub lon: f64,
    pub pop: u64,
}

/// (code, name, latitude, longitude): the middle of each state, roughly.
const STATES: &[(&str, &str, f64, f64)] = &[
    ("AL", "Alabama", 32.8, -86.8), ("AK", "Alaska", 64.2, -152.3), ("AZ", "Arizona", 34.3, -111.7), ("AR", "Arkansas", 34.9, -92.4),
    ("CA", "California", 37.2, -119.5), ("CO", "Colorado", 39.0, -105.5), ("CT", "Connecticut", 41.6, -72.7), ("DE", "Delaware", 39.0, -75.5),
    ("DC", "District of Columbia", 38.9, -77.0), ("FL", "Florida", 28.6, -82.4), ("GA", "Georgia", 32.7, -83.4), ("HI", "Hawaii", 20.8, -156.3),
    ("ID", "Idaho", 44.4, -114.6), ("IL", "Illinois", 40.0, -89.2), ("IN", "Indiana", 39.9, -86.3), ("IA", "Iowa", 42.1, -93.5),
    ("KS", "Kansas", 38.5, -98.4), ("KY", "Kentucky", 37.5, -85.3), ("LA", "Louisiana", 31.1, -92.0), ("ME", "Maine", 45.4, -69.2),
    ("MD", "Maryland", 39.0, -76.8), ("MA", "Massachusetts", 42.3, -71.8), ("MI", "Michigan", 44.3, -85.4), ("MN", "Minnesota", 46.3, -94.3),
    ("MS", "Mississippi", 32.7, -89.7), ("MO", "Missouri", 38.4, -92.5), ("MT", "Montana", 47.0, -109.6), ("NE", "Nebraska", 41.5, -99.8),
    ("NV", "Nevada", 39.3, -116.6), ("NH", "New Hampshire", 43.7, -71.6), ("NJ", "New Jersey", 40.1, -74.7), ("NM", "New Mexico", 34.4, -106.1),
    ("NY", "New York", 42.9, -75.5), ("NC", "North Carolina", 35.6, -79.4), ("ND", "North Dakota", 47.5, -100.5), ("OH", "Ohio", 40.3, -82.8),
    ("OK", "Oklahoma", 35.6, -97.5), ("OR", "Oregon", 43.9, -120.6), ("PA", "Pennsylvania", 40.9, -77.8), ("RI", "Rhode Island", 41.7, -71.5),
    ("SC", "South Carolina", 33.9, -80.9), ("SD", "South Dakota", 44.4, -100.2), ("TN", "Tennessee", 35.9, -86.4), ("TX", "Texas", 31.5, -99.3),
    ("UT", "Utah", 39.3, -111.7), ("VT", "Vermont", 44.0, -72.7), ("VA", "Virginia", 37.5, -78.8), ("WA", "Washington", 47.4, -120.5),
    ("WV", "West Virginia", 38.6, -80.6), ("WI", "Wisconsin", 44.6, -89.9), ("WY", "Wyoming", 43.0, -107.6),
];

fn state_code(word: &str) -> Option<&'static str> {
    let w = word.trim().trim_end_matches('.').to_lowercase();
    STATES.iter().find(|s| s.0.to_lowercase() == w || s.1.to_lowercase() == w).map(|s| s.0)
}

#[derive(Default)]
pub struct Index {
    places: Vec<Place>,
    /// Lower-case names, one per place, so a search does not lower-case two million words each time.
    keys: Vec<String>,
}

impl Index {
    /// The built-in states and DC, then whatever the list on the drive holds. A missing file is fine.
    pub fn load(file: &Path) -> Index {
        let mut ix = Index::default();
        for (code, name, lat, lon) in STATES {
            ix.push(Place { name: name.to_string(), state: code.to_string(), kind: "state".into(), lat: *lat, lon: *lon, pop: 100_000_000 });
        }
        if let Ok(text) = std::fs::read_to_string(file) {
            for line in text.lines() {
                if let Some(p) = parse_line(line) {
                    ix.push(p);
                }
            }
        }
        ix
    }
    fn push(&mut self, p: Place) {
        self.keys.push(p.name.to_lowercase());
        self.places.push(p);
    }
    pub fn len(&self) -> usize {
        self.places.len()
    }
    pub fn is_empty(&self) -> bool {
        self.places.is_empty()
    }
    /// True when the list on the drive was found (more than the built-in states).
    pub fn has_list(&self) -> bool {
        self.places.len() > STATES.len()
    }

    /// Places matching what the person typed: a "latitude, longitude" pair, a 5-digit ZIP code, or words
    /// ("denver", "denver, co", "pikes peak", "texas"). Best first, at most `limit`.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Place> {
        let q = query.trim();
        if q.is_empty() || limit == 0 {
            return vec![];
        }
        if let Some((lat, lon)) = parse_coordinates(q) {
            return vec![Place { name: format!("{:.5}, {:.5}", lat, lon), state: String::new(), kind: "point".into(), lat, lon, pop: 0 }];
        }
        if q.len() == 5 && q.bytes().all(|b| b.is_ascii_digit()) {
            return self.places.iter().filter(|p| p.kind == "zip" && p.name == q).take(limit).cloned().collect();
        }
        // "name, state" or "name state": the last word or the part after a comma may be a state.
        let lower = q.to_lowercase();
        let (mut name, mut region) = (lower.clone(), None);
        if let Some((a, b)) = lower.rsplit_once(',') {
            if let Some(c) = state_code(b) {
                name = a.trim().to_string();
                region = Some(c);
            }
        } else if let Some((a, b)) = lower.rsplit_once(' ') {
            if let Some(c) = state_code(b) {
                // "new york" is a state itself: only split when something is left and it is not a state name on its own
                if state_code(&lower).is_none() {
                    name = a.trim().to_string();
                    region = Some(c);
                }
            }
        }
        if name.is_empty() {
            return vec![];
        }
        let mut scored: Vec<(i64, usize)> = Vec::new();
        for (i, key) in self.keys.iter().enumerate() {
            let p = &self.places[i];
            if p.kind == "zip" {
                continue;
            }
            if let Some(r) = region {
                if p.state != r {
                    continue;
                }
            }
            let score = if *key == name {
                3000
            } else if key.starts_with(&name) {
                2000
            } else if key.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&name)) {
                1000
            } else {
                continue;
            };
            let weight = match p.kind.as_str() {
                "state" => 400,
                "city" => 300,
                "county" => 200,
                _ => 100,
            };
            let pop = (p.pop.max(1) as f64).log10() as i64 * 10;
            scored.push((score + weight + pop - key.len() as i64 / 4, i));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().take(limit).map(|(_, i)| self.places[i].clone()).collect()
    }
}

fn parse_line(line: &str) -> Option<Place> {
    let mut f = line.split('\t');
    let name = f.next()?.trim();
    let state = f.next()?.trim();
    let kind = f.next()?.trim();
    let lat: f64 = f.next()?.trim().parse().ok()?;
    let lon: f64 = f.next()?.trim().parse().ok()?;
    let pop: u64 = f.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
    if name.is_empty() || kind.is_empty() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some(Place { name: name.to_string(), state: state.to_string(), kind: kind.to_string(), lat, lon, pop })
}

/// "39.7392, -104.9903" or "39.7392 -104.9903" (latitude first).
pub fn parse_coordinates(q: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = q.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()).collect();
    if parts.len() != 2 {
        return None;
    }
    let lat: f64 = parts[0].parse().ok()?;
    let lon: f64 = parts[1].parse().ok()?;
    if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
        Some((lat, lon))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Index {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("cindrix-places-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("places.tsv");
        std::fs::write(
            &f,
            "Denver\tCO\tcity\t39.7392\t-104.9903\t715000\n\
             Denver\tNC\tcity\t35.5\t-81.0\t3000\n\
             Denver County\tCO\tcounty\t39.76\t-104.88\t715000\n\
             80202\tCO\tzip\t39.75\t-105.0\t0\n\
             Pikes Peak\tCO\tpeak\t38.8409\t-105.0423\t0\n\
             not a line\n\
             Bad\tCO\tcity\tabc\t1\t1\n",
        )
        .unwrap();
        Index::load(&f)
    }

    #[test]
    fn states_are_known_with_no_file() {
        let ix = Index::load(Path::new("/no/such/file.tsv"));
        assert!(!ix.has_list());
        let r = ix.search("texas", 5);
        assert_eq!(r[0].name, "Texas");
        assert_eq!(r[0].state, "TX");
        assert_eq!(ix.search("new york", 3)[0].name, "New York");
    }

    #[test]
    fn a_city_is_found_by_name_and_the_bigger_one_comes_first() {
        let ix = sample();
        assert!(ix.has_list());
        let r = ix.search("denver", 5);
        assert_eq!(r[0].state, "CO");
        assert_eq!(r[0].kind, "city");
        assert!(r.iter().any(|p| p.state == "NC"));
    }

    #[test]
    fn a_state_narrows_the_search() {
        let ix = sample();
        for q in ["denver, nc", "denver nc", "Denver, North Carolina"] {
            let r = ix.search(q, 5);
            assert_eq!(r.len(), 1, "{}", q);
            assert_eq!(r[0].state, "NC");
        }
    }

    #[test]
    fn a_zip_code_and_coordinates_work() {
        let ix = sample();
        assert_eq!(ix.search("80202", 3)[0].state, "CO");
        assert!(ix.search("99999", 3).is_empty());
        let c = ix.search("39.7392, -104.9903", 3);
        assert_eq!(c[0].kind, "point");
        assert!((c[0].lat - 39.7392).abs() < 1e-9);
        assert_eq!(parse_coordinates("200, 10"), None);
        assert_eq!(parse_coordinates("1 2 3"), None);
    }

    #[test]
    fn a_word_inside_a_name_matches_and_bad_lines_are_skipped() {
        let ix = sample();
        assert_eq!(ix.search("peak", 3)[0].name, "Pikes Peak");
        assert_eq!(ix.len(), 51 + 5);
        assert!(ix.search("", 3).is_empty());
    }
}
