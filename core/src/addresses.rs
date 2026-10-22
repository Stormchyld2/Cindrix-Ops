//! Street address search with no internet: plain text files of address points kept on the drive.
//!
//! Layout (made by `scripts/get-addresses.ps1` from the Overture Maps addresses data):
//! ```text
//! maps/addresses/<pack>/<X>.tsv      X = first letter or digit of the street's key (A-Z, 0-9, or _)
//! ```
//! A pack is one download: `us` for the whole country, or a name of your own for one area. Every pack is searched.
//!
//! One line: `street TAB number TAB zip TAB state TAB latitude TAB longitude`.
//! * `street` is the street name in the **key form** made by [`norm_street`] (capitals, no punctuation, usual short
//!   words such as ST for STREET and N for NORTH). The download script writes it the same way, and
//!   `core/tests/address_keys.json` is checked against both, so a search finds what the download wrote.
//! * Lines are sorted by street, then by the house number's digits, then by the whole number text, so a search jumps
//!   to the right place in a file of millions of lines without reading the rest (a binary search by file position).
//!
//! Coverage is partial: an address that is not in the data is not found, and that does not mean the house is not there.

use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Words that are written the short way in a street key: (long form, key form).
pub const SHORT_WORDS: &[(&str, &str)] = &[
    ("STREET", "ST"), ("AVENUE", "AVE"), ("AV", "AVE"), ("ROAD", "RD"), ("DRIVE", "DR"), ("LANE", "LN"), ("COURT", "CT"),
    ("CIRCLE", "CIR"), ("BOULEVARD", "BLVD"), ("PLACE", "PL"), ("HIGHWAY", "HWY"), ("PARKWAY", "PKWY"), ("TRAIL", "TRL"),
    ("TERRACE", "TER"), ("SQUARE", "SQ"), ("ALLEY", "ALY"), ("EXPRESSWAY", "EXPY"), ("FREEWAY", "FWY"), ("TURNPIKE", "TPKE"),
    ("CROSSING", "XING"), ("MOUNTAIN", "MTN"), ("HEIGHTS", "HTS"), ("POINT", "PT"), ("RIDGE", "RDG"), ("VALLEY", "VLY"),
    ("CREEK", "CRK"), ("NORTH", "N"), ("SOUTH", "S"), ("EAST", "E"), ("WEST", "W"), ("NORTHEAST", "NE"), ("NORTHWEST", "NW"),
    ("SOUTHEAST", "SE"), ("SOUTHWEST", "SW"), ("SAINT", "ST"), ("FORT", "FT"), ("MOUNT", "MT"),
];

/// A street name in key form: capitals and digits only, one space between words, the usual short words.
/// "North Main Street" and "N. MAIN ST" both become "N MAIN ST".
pub fn norm_street(s: &str) -> String {
    let clean: String = s.to_uppercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' }).collect();
    let words: Vec<&str> = clean
        .split_whitespace()
        .map(|w| SHORT_WORDS.iter().find(|(long, _)| *long == w).map(|(_, short)| *short).unwrap_or(w))
        .collect();
    words.join(" ")
}

/// The part of the file name that says which file a street is in: its first letter or digit.
pub fn file_letter(key: &str) -> char {
    match key.chars().next() {
        Some(c) if c.is_ascii_uppercase() || c.is_ascii_digit() => c,
        _ => '_',
    }
}

/// What was typed, picked apart.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Query {
    pub number: String,
    /// The street in key form (it may be only the start of a name).
    pub street: String,
    pub zip: String,
    pub state: String,
    /// Anything else after the street (a town), when there is some.
    pub town: String,
}

const STATE_CODES: &[&str] = &[
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "DC", "FL", "GA", "HI", "ID", "IL", "IN", "IA", "KS", "KY", "LA", "ME", "MD", "MA", "MI", "MN",
    "MS", "MO", "MT", "NE", "NV", "NH", "NJ", "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC", "SD", "TN", "TX", "UT", "VT", "VA", "WA",
    "WV", "WI", "WY", "PR", "GU", "VI", "AS", "MP",
];

fn is_house_number(w: &str) -> bool {
    let w = w.trim_end_matches(',');
    let digits = w.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0 && digits <= 8 && w.len() - digits <= 2 && w[digits..].chars().all(|c| c.is_ascii_alphabetic() || c == '-')
}

/// The ways a typed address could be read, most likely first. "123 Main St Denver CO 80202" is first read as the street
/// "MAIN ST DENVER", then as "MAIN ST" in the town "DENVER".
pub fn parse_query(q: &str) -> Vec<Query> {
    let q = q.trim();
    // house number first
    let mut words = q.split_whitespace();
    let Some(first) = words.next() else { return vec![] };
    if !is_house_number(first) {
        return vec![];
    }
    let number = first.trim_end_matches(',').to_uppercase();
    let rest: String = words.collect::<Vec<_>>().join(" ");
    if rest.is_empty() {
        return vec![];
    }
    // after a comma: the town, state and ZIP
    let (street_part, tail) = match rest.split_once(',') {
        Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
        None => (rest.clone(), String::new()),
    };
    let mut toks: Vec<String> = street_part.split_whitespace().map(|s| s.to_string()).collect();
    let mut tail_toks: Vec<String> = tail.replace(',', " ").split_whitespace().map(|s| s.to_string()).collect();
    let mut zip = String::new();
    let mut state = String::new();
    // a ZIP and a state code can be at the end of either part
    for (is_street_part, part) in [(false, &mut tail_toks), (true, &mut toks)] {
        if let Some(z) = part.last() {
            let z5: String = z.chars().take(5).collect();
            if z5.len() == 5 && z5.chars().all(|c| c.is_ascii_digit()) && (z.len() == 5 || z.chars().nth(5) == Some('-')) {
                zip = z5;
                part.pop();
            }
        }
        if let Some(s) = part.last() {
            let up = s.to_uppercase();
            // a state code alone after the comma is a state; at the end of the street part it needs a street word before it
            if STATE_CODES.contains(&up.as_str()) && (!is_street_part || part.len() > 1) {
                state = up;
                part.pop();
            }
        }
    }
    let mut out = Vec::new();
    if toks.is_empty() {
        return out;
    }
    let town_tail = tail_toks.join(" ").to_uppercase();
    if !tail_toks.is_empty() || toks.len() == 1 {
        out.push(Query { number: number.clone(), street: norm_street(&toks.join(" ")), zip: zip.clone(), state: state.clone(), town: town_tail.clone() });
        return out;
    }
    // no comma: the last words may be a town
    let n = toks.len();
    let lowest = n.saturating_sub(4).max(1);
    for k in (lowest..=n).rev() {
        out.push(Query {
            number: number.clone(),
            street: norm_street(&toks[..k].join(" ")),
            zip: zip.clone(),
            state: state.clone(),
            town: toks[k..].join(" ").to_uppercase(),
        });
    }
    out
}

/// One address found.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub number: String,
    pub street: String,
    pub zip: String,
    pub state: String,
    pub lat: f64,
    pub lon: f64,
    /// True when this house number was not in the data and a neighbouring one on the same street is shown instead.
    pub near: bool,
}

impl Hit {
    /// "123 Main St, CO 80202".
    pub fn label(&self) -> String {
        let street: Vec<String> = self
            .street
            .split(' ')
            .map(|w| {
                let mut cs = w.chars();
                match cs.next() {
                    Some(f) if f.is_ascii_alphabetic() && w.len() > 1 && w.chars().all(|c| c.is_ascii_alphabetic()) && !matches!(w, "NE" | "NW" | "SE" | "SW") => {
                        f.to_string() + &cs.as_str().to_lowercase()
                    }
                    _ => w.to_string(),
                }
            })
            .collect();
        let mut s = format!("{} {}", self.number, street.join(" "));
        let place = [self.state.as_str(), self.zip.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" ");
        if !place.is_empty() {
            s.push_str(", ");
            s.push_str(&place);
        }
        s
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Row {
    street: String,
    number: String,
    zip: String,
    state: String,
    lat: f64,
    lon: f64,
}

fn parse_row(line: &str) -> Option<Row> {
    let mut f = line.split('\t');
    let street = f.next()?.to_string();
    let number = f.next()?.to_string();
    let zip = f.next()?.to_string();
    let state = f.next()?.to_string();
    let lat: f64 = f.next()?.trim().parse().ok()?;
    let lon: f64 = f.next()?.trim().parse().ok()?;
    if street.is_empty() || number.is_empty() || !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    Some(Row { street, number, zip, state, lat, lon })
}

/// The digits a house number starts with, if any. Numbers without digits sort first.
fn digits_of(n: &str) -> Option<u64> {
    let d: String = n.chars().take_while(|c| c.is_ascii_digit()).take(18).collect();
    d.parse().ok()
}

fn cmp_key(street: &str, number: &str, t_street: &str, t_number: &str) -> Ordering {
    street.as_bytes().cmp(t_street.as_bytes()).then_with(|| {
        if t_number == AFTER_ALL_NUMBERS {
            return Ordering::Less; // every number on the street is below the end marker
        }
        digits_of(number).cmp(&digits_of(t_number)).then_with(|| number.as_bytes().cmp(t_number.as_bytes()))
    })
}

/// The start of the first line at or after byte `off` (the end of the file when there is none).
fn line_start_at_or_after<R: Read + Seek>(r: &mut R, off: u64, len: u64) -> std::io::Result<u64> {
    if off == 0 {
        return Ok(0);
    }
    if off >= len {
        return Ok(len);
    }
    r.seek(SeekFrom::Start(off - 1))?;
    let mut buf = [0u8; 4096];
    let mut pos = off - 1;
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            return Ok(len);
        }
        if let Some(i) = buf[..n].iter().position(|&b| b == b'\n') {
            return Ok(pos + i as u64 + 1);
        }
        pos += n as u64;
    }
}

/// The line that starts at `at`: (its text, where the next line starts).
fn line_at<R: Read + Seek>(r: &mut R, at: u64) -> std::io::Result<(String, u64)> {
    r.seek(SeekFrom::Start(at))?;
    let mut br = BufReader::new(&mut *r);
    let mut s = String::new();
    let n = br.read_line(&mut s)?;
    Ok((s.trim_end_matches(['\n', '\r']).to_string(), at + n as u64))
}

/// Where the first line whose key is not below (street, number) starts. The file must be sorted.
fn lower_bound<R: Read + Seek>(r: &mut R, len: u64, street: &str, number: &str) -> std::io::Result<u64> {
    let (mut lo, mut hi) = (0u64, len);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let ls = line_start_at_or_after(r, mid, len)?;
        let at_or_above = if ls >= len {
            true
        } else {
            let (text, _) = line_at(r, ls)?;
            match parse_row(&text) {
                Some(row) => cmp_key(&row.street, &row.number, street, number) != Ordering::Less,
                // a damaged line never decides anything: treat it as small so the search moves on past it
                None => false,
            }
        };
        if at_or_above { hi = mid } else { lo = mid + 1 }
    }
    line_start_at_or_after(r, lo, len)
}

/// The row on the line just before the one that starts at `at` (not reading earlier than `floor`).
fn row_before<R: Read + Seek>(r: &mut R, at: u64, floor: u64) -> std::io::Result<Option<(Row, u64)>> {
    let from = at.saturating_sub(2048).max(floor);
    r.seek(SeekFrom::Start(from))?;
    let mut buf = vec![0u8; (at - from) as usize];
    r.read_exact(&mut buf)?;
    // drop the newline that ends the previous line, then take what follows the one before it
    let body = if buf.last() == Some(&b'\n') { &buf[..buf.len() - 1] } else { &buf[..] };
    let begin = match body.iter().rposition(|&b| b == b'\n') {
        Some(i) => i + 1,
        None if from == floor => 0,
        None => return Ok(None),
    };
    Ok(parse_row(String::from_utf8_lossy(&body[begin..]).trim_end()).map(|row| (row, from + begin as u64)))
}

/// A key just after every number on `street`, so the search can step to the next street.
const AFTER_ALL_NUMBERS: &str = "\u{10FFFF}";

/// Everything one file says about a typed address: rows on streets that start with `q.street`.
fn search_file(path: &Path, q: &Query, max_streets: usize, per_street: usize, out: &mut Vec<Hit>) -> std::io::Result<()> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    let mut street_key = q.street.clone();
    let mut streets = 0;
    let mut pos = lower_bound(&mut f, len, &street_key, "")?;
    while pos < len && streets < max_streets {
        let (text, next) = line_at(&mut f, pos)?;
        let Some(first) = parse_row(&text) else {
            pos = next;
            continue;
        };
        if !first.street.starts_with(&q.street) {
            break;
        }
        streets += 1;
        street_key = first.street.clone();
        // the block of this street: from `start` up to the first line of the next street
        let start = lower_bound(&mut f, len, &street_key, "")?;
        let end = lower_bound(&mut f, len, &street_key, AFTER_ALL_NUMBERS)?;
        // the first row at or after the typed number, then a few either side
        let at = lower_bound(&mut f, end, &street_key, &q.number)?.max(start);
        let mut exact = Vec::new();
        let mut around: Vec<Row> = Vec::new();
        // rows from `at` going forward
        let mut p = at;
        let mut count = 0;
        while p < end && count < per_street {
            let (t, n) = line_at(&mut f, p)?;
            if let Some(r) = parse_row(&t) {
                if r.number == q.number {
                    exact.push(r);
                } else if around.len() < 2 {
                    around.push(r);
                }
                count += 1;
            }
            p = n;
        }
        // when the house number is there, only it is shown; otherwise its neighbours, marked as near
        let shown: Vec<&Row> = if exact.is_empty() { around.iter().collect() } else { exact.iter().collect() };
        for r in shown {
            let near = r.number != q.number;
            out.push(Hit { number: r.number.clone(), street: r.street.clone(), zip: r.zip.clone(), state: r.state.clone(), lat: r.lat, lon: r.lon, near });
        }
        // the number just before, for a neighbour on the other side, when nothing was exact
        if exact.is_empty() {
            let mut back = at;
            for _ in 0..3 {
                if back <= start {
                    break;
                }
                let Some((r, from)) = row_before(&mut f, back, start)? else { break };
                back = from;
                if r.street == street_key {
                    out.push(Hit { number: r.number, street: r.street, zip: r.zip, state: r.state, lat: r.lat, lon: r.lon, near: true });
                }
            }
        }
        // on to the next street
        pos = end;
    }
    Ok(())
}

/// Every pack folder under `maps/addresses` (a missing folder is just no packs).
pub fn packs(addresses_dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(addresses_dir)
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// True when at least one address file exists.
pub fn has_addresses(addresses_dir: &Path) -> bool {
    packs(addresses_dir).iter().any(|p| {
        std::fs::read_dir(p).map(|rd| rd.flatten().any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("tsv"))).unwrap_or(false)
    })
}

/// Search every pack for a typed address. `near` (latitude, longitude) puts the closest results first when the same
/// street name is in many places. A ZIP, a state or a town that was typed narrows the results.
pub fn search(addresses_dir: &Path, typed: &str, near: Option<(f64, f64)>, limit: usize) -> Vec<Hit> {
    let readings = parse_query(typed);
    for q in readings {
        if q.street.is_empty() {
            continue;
        }
        let letter = file_letter(&q.street);
        let mut hits: Vec<Hit> = Vec::new();
        for pack in packs(addresses_dir) {
            let path = pack.join(format!("{}.tsv", letter));
            if path.is_file() {
                // a damaged or half-written file just gives nothing
                let _ = search_file(&path, &q, 40, 40, &mut hits);
            }
        }
        if !q.zip.is_empty() {
            hits.retain(|h| h.zip.starts_with(&q.zip) || h.zip.is_empty());
        }
        if !q.state.is_empty() {
            hits.retain(|h| h.state.is_empty() || h.state == q.state);
        }
        if hits.is_empty() {
            continue;
        }
        // the same place from two packs is one result
        hits.sort_by(|a, b| (a.number.as_str(), a.street.as_str(), a.zip.as_str()).cmp(&(b.number.as_str(), b.street.as_str(), b.zip.as_str())));
        hits.dedup_by(|a, b| a.number == b.number && a.street == b.street && a.zip == b.zip && (a.lat - b.lat).abs() < 0.0005 && (a.lon - b.lon).abs() < 0.0005);
        let dist = |h: &Hit| match near {
            Some((la, lo)) => (h.lat - la).powi(2) + ((h.lon - lo) * (la.to_radians().cos())).powi(2),
            None => 0.0,
        };
        hits.sort_by(|a, b| {
            a.near
                .cmp(&b.near)
                .then(dist(a).partial_cmp(&dist(b)).unwrap_or(Ordering::Equal))
                .then_with(|| a.street.cmp(&b.street))
                .then_with(|| a.number.cmp(&b.number))
        });
        hits.truncate(limit);
        return hits;
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cx-addr-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Rows written the way the download writes them: sorted by street, then number digits, then number text.
    fn write_sorted(path: &Path, rows: &[(&str, &str, &str, &str, f64, f64)]) {
        let mut v: Vec<_> = rows.to_vec();
        v.sort_by(|a, b| cmp_key(a.0, a.1, b.0, b.1));
        let text: String = v.iter().map(|r| format!("{}\t{}\t{}\t{}\t{}\t{}\n", r.0, r.1, r.2, r.3, r.4, r.5)).collect();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn the_shared_list_of_street_keys_matches() {
        // the same cases are run through the download's own SQL in CI (see .github/workflows/ci.yml)
        let text = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/address_keys.tsv")).unwrap();
        let mut n = 0;
        for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let (input, want) = line.split_once('\t').unwrap();
            assert_eq!(norm_street(input), want, "{:?}", input);
            n += 1;
        }
        assert!(n >= 15);
    }

    #[test]
    fn the_word_list_has_no_word_pointing_at_another_long_word() {
        for (long, short) in SHORT_WORDS {
            assert!(SHORT_WORDS.iter().all(|(l, _)| l != short), "{} -> {} is itself shortened", long, short);
        }
    }

    #[test]
    fn typed_addresses_are_read_in_the_likely_ways() {
        let q = parse_query("123 North Main Street, Denver CO 80202");
        assert_eq!(q.len(), 1);
        assert_eq!((q[0].number.as_str(), q[0].street.as_str(), q[0].zip.as_str(), q[0].state.as_str(), q[0].town.as_str()), ("123", "N MAIN ST", "80202", "CO", "DENVER"));
        let q = parse_query("55 elm st 80202");
        assert_eq!((q[0].street.as_str(), q[0].zip.as_str()), ("ELM ST", "80202"));
        // no commas: the last word may be a town
        let q = parse_query("12 oak avenue springfield");
        assert_eq!(q[0].street, "OAK AVE SPRINGFIELD");
        assert!(q.iter().any(|x| x.street == "OAK AVE" && x.town == "SPRINGFIELD"));
        assert!(parse_query("main street").is_empty()); // no house number
        assert!(parse_query("denver").is_empty());
        assert!(parse_query("123").is_empty());
        assert_eq!(parse_query("12B Pine Rd")[0].number, "12B");
        // the town in front of the comma with only a state and ZIP after it
        let q = parse_query("529 Ogden Ave Fairmont, WV 26554");
        assert_eq!((q[0].street.as_str(), q[0].state.as_str(), q[0].zip.as_str()), ("OGDEN AVE FAIRMONT", "WV", "26554"));
        assert!(q.iter().any(|x| x.street == "OGDEN AVE" && x.town == "FAIRMONT" && x.state == "WV"));
    }

    #[test]
    fn the_binary_search_agrees_with_reading_every_line() {
        let d = tmp("bin");
        let mut rows: Vec<(String, String)> = Vec::new();
        for s in ["A ST", "ALDER LN", "B ST", "BIRCH RD", "C ST", "CEDAR CT", "D ST"] {
            for n in [1, 3, 5, 7, 9, 11, 13, 15, 101, 103, 105, 2001, 2003, 99999] {
                rows.push((s.to_string(), n.to_string()));
            }
            rows.push((s.to_string(), "7B".to_string()));
        }
        let borrowed: Vec<(&str, &str, &str, &str, f64, f64)> = rows.iter().map(|(s, n)| (s.as_str(), n.as_str(), "80202", "CO", 39.0, -105.0)).collect();
        let p = d.join("A.tsv");
        write_sorted(&p, &borrowed);
        let bytes = fs::read(&p).unwrap();
        let len = bytes.len() as u64;
        let mut c = Cursor::new(bytes.clone());
        let lines: Vec<(u64, Row)> = {
            let text = String::from_utf8(bytes).unwrap();
            let mut at = 0u64;
            text.split_inclusive('\n').map(|l| { let r = (at, parse_row(l.trim_end()).unwrap()); at += l.len() as u64; r }).collect()
        };
        for target_street in ["A ST", "ALDER LN", "ALDER", "B", "BIRCH RD", "Z", "A", "D ST", "CEDAR CT"] {
            for target_number in ["", "0", "1", "2", "7", "7B", "8", "100", "2002", "99999", "100000", "7C"] {
                let want = lines.iter().find(|(_, r)| cmp_key(&r.street, &r.number, target_street, target_number) != Ordering::Less).map(|(at, _)| *at).unwrap_or(len);
                let got = lower_bound(&mut c, len, target_street, target_number).unwrap();
                assert_eq!(got, want, "{} {}", target_street, target_number);
            }
        }
    }

    #[test]
    fn an_address_is_found_exactly_or_by_its_neighbour_and_can_be_narrowed() {
        let d = tmp("find");
        let root = d.join("addresses");
        write_sorted(
            &root.join("us/M.tsv"),
            &[
                ("MAIN ST", "100", "80202", "CO", 39.75, -104.99),
                ("MAIN ST", "102", "80202", "CO", 39.7501, -104.99),
                ("MAIN ST", "104", "80202", "CO", 39.7502, -104.99),
                ("MAIN ST", "200", "80203", "CO", 39.76, -104.98),
                ("MAIN ST", "100", "64101", "MO", 39.10, -94.58),
                ("MAIN AVE", "100", "80211", "CO", 39.77, -105.0),
                ("MAINE ST", "5", "04101", "ME", 43.66, -70.25),
                ("MAPLE ST", "9", "80202", "CO", 39.7, -104.9),
            ],
        );
        // exact, with the state narrowing it
        let h = search(&root, "100 Main St CO", None, 8);
        assert_eq!(h.len(), 1, "{:?}", h);
        assert_eq!((h[0].number.as_str(), h[0].zip.as_str(), h[0].near), ("100", "80202", false));
        assert!(h[0].label().starts_with("100 Main St, CO 80202"));
        // exact in two places: the one nearer the map's middle first
        let h = search(&root, "100 Main St", Some((39.1, -94.6)), 8);
        assert_eq!(h[0].zip, "64101");
        assert!(h.iter().any(|x| x.zip == "80202" && !x.near));
        // a number that is not there: its neighbours on the street, marked as near
        let h = search(&root, "101 Main St 80202", None, 8);
        assert!(!h.is_empty() && h.iter().all(|x| x.near), "{:?}", h);
        assert!(h.iter().any(|x| x.number == "100") && h.iter().any(|x| x.number == "102"));
        // the start of a street name
        let h = search(&root, "100 Mai", None, 8);
        assert!(h.iter().any(|x| x.street == "MAIN ST") && h.iter().any(|x| x.street == "MAIN AVE"));
        // no such street, no such file, no such folder
        assert!(search(&root, "100 Zebra Rd", None, 8).is_empty());
        assert!(search(&d.join("none"), "100 Main St", None, 8).is_empty());
        assert!(search(&root, "not an address", None, 8).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_town_after_the_street_is_dropped_when_it_finds_nothing_with_it() {
        let d = tmp("town");
        let root = d.join("addresses");
        write_sorted(&root.join("us/E.tsv"), &[("ELM ST", "12", "62701", "IL", 39.8, -89.65)]);
        let h = search(&root, "12 elm street springfield", None, 8);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].zip, "62701");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_full_address_with_the_town_before_the_comma_is_found() {
        let d = tmp("full");
        let root = d.join("addresses");
        write_sorted(&root.join("us/O.tsv"), &[("OGDEN AVE", "529", "26554", "WV", 39.48, -80.14), ("OGDEN AVE", "531", "26554", "WV", 39.481, -80.14)]);
        let h = search(&root, "529 Ogden Ave Fairmont, WV 26554", None, 8);
        assert_eq!(h.len(), 1, "{:?}", h);
        assert!(!h[0].near);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn two_packs_with_the_same_address_give_one_result_and_damaged_lines_are_skipped() {
        let d = tmp("packs");
        let root = d.join("addresses");
        let rows = [("ELM ST", "12", "62701", "IL", 39.8, -89.65)];
        write_sorted(&root.join("us/E.tsv"), &rows);
        write_sorted(&root.join("springfield/E.tsv"), &rows);
        assert!(has_addresses(&root));
        assert_eq!(search(&root, "12 elm st", None, 8).len(), 1);
        fs::write(root.join("us/E.tsv"), "garbage line\nELM ST\t12\t62701\tIL\t39.8\t-89.65\nELM ST\tx\n").unwrap();
        assert!(!search(&root, "12 elm st", None, 8).is_empty());
        assert!(!has_addresses(&d.join("nothing")));
        let _ = fs::remove_dir_all(&d);
    }
}
