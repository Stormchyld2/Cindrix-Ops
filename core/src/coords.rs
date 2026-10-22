//! Ways of writing a position on the Earth, and turning one into another: decimal degrees,
//! degrees-minutes-seconds (39°29'06.5"N 80°08'21.4"W), the military grid (MGRS, for example
//! "17S LD 1234 5678": grid zone and band, 100 km square, then easting and northing) and UTM
//! ("17S 580000 4370000"). Standard library only (WGS84, Transverse Mercator series).

const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;
const K0: f64 = 0.9996;
const BANDS: &str = "CDEFGHJKLMNPQRSTUVWX";
const ROWS: &str = "ABCDEFGHJKLMNPQRSTUV";

fn e2() -> f64 {
    F * (2.0 - F)
}

/// The UTM zone of a position (with the exceptions around Norway and Svalbard).
fn zone_of(lat: f64, lon: f64) -> u32 {
    let mut z = (((lon + 180.0) / 6.0).floor() as i32 + 1).clamp(1, 60) as u32;
    if (56.0..64.0).contains(&lat) && (3.0..12.0).contains(&lon) {
        z = 32;
    }
    if (72.0..84.0).contains(&lat) {
        if (0.0..9.0).contains(&lon) {
            z = 31;
        } else if (9.0..21.0).contains(&lon) {
            z = 33;
        } else if (21.0..33.0).contains(&lon) {
            z = 35;
        } else if (33.0..42.0).contains(&lon) {
            z = 37;
        }
    }
    z
}

fn central_meridian(zone: u32) -> f64 {
    (zone as f64 - 1.0) * 6.0 - 180.0 + 3.0
}

fn band_of(lat: f64) -> Option<char> {
    if !(-80.0..=84.0).contains(&lat) {
        return None;
    }
    let i = (((lat + 80.0) / 8.0).floor() as usize).min(19);
    BANDS.chars().nth(i)
}

/// UTM easting and northing in metres for a position, in the given zone.
fn forward(lat: f64, lon: f64, zone: u32) -> (f64, f64) {
    let e2 = e2();
    let ep2 = e2 / (1.0 - e2);
    let (phi, lam) = (lat.to_radians(), (lon - central_meridian(zone)).to_radians());
    let n = A / (1.0 - e2 * phi.sin().powi(2)).sqrt();
    let t = phi.tan().powi(2);
    let c = ep2 * phi.cos().powi(2);
    let a = lam * phi.cos();
    let m = A
        * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2.powi(3) / 256.0) * phi - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * e2.powi(3) / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e2 * e2 / 256.0 + 45.0 * e2.powi(3) / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e2.powi(3) / 3072.0) * (6.0 * phi).sin());
    let x = K0 * n * (a + (1.0 - t + c) * a.powi(3) / 6.0 + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * a.powi(5) / 120.0) + 500_000.0;
    let mut y = K0 * (m + n * phi.tan() * (a * a / 2.0 + (5.0 - t + 9.0 * c + 4.0 * c * c) * a.powi(4) / 24.0 + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * a.powi(6) / 720.0));
    if lat < 0.0 {
        y += 10_000_000.0;
    }
    (x, y)
}

/// Latitude and longitude for a UTM easting and northing.
fn inverse(x: f64, y: f64, zone: u32, south: bool) -> (f64, f64) {
    let e2 = e2();
    let ep2 = e2 / (1.0 - e2);
    let x = x - 500_000.0;
    let y = if south { y - 10_000_000.0 } else { y };
    let m = y / K0;
    let mu = m / (A * (1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2.powi(3) / 256.0));
    let e1 = (1.0 - (1.0 - e2).sqrt()) / (1.0 + (1.0 - e2).sqrt());
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1 * e1 / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();
    let c1 = ep2 * phi1.cos().powi(2);
    let t1 = phi1.tan().powi(2);
    let n1 = A / (1.0 - e2 * phi1.sin().powi(2)).sqrt();
    let r1 = A * (1.0 - e2) / (1.0 - e2 * phi1.sin().powi(2)).powf(1.5);
    let d = x / (n1 * K0);
    let phi = phi1
        - (n1 * phi1.tan() / r1)
            * (d * d / 2.0 - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * ep2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1 - 252.0 * ep2 - 3.0 * c1 * c1) * d.powi(6) / 720.0);
    let lam = (d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0 + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * ep2 + 24.0 * t1 * t1) * d.powi(5) / 120.0) / phi1.cos();
    (phi.to_degrees(), central_meridian(zone) + lam.to_degrees())
}

/// The military grid reference of a position, for example `17S LD 1234 5678` with `digits` = 4
/// (10 m squares). `digits` is 1 to 5 for each of the easting and the northing. None outside 80°S to 84°N.
pub fn to_mgrs(lat: f64, lon: f64, digits: usize) -> Option<String> {
    let band = band_of(lat)?;
    let zone = zone_of(lat, lon);
    let (x, y) = forward(lat, lon, zone);
    let col_set = ((zone - 1) % 3) as usize;
    let col_idx = (x / 100_000.0).floor() as i64 - 1;
    if !(0..8).contains(&col_idx) {
        return None;
    }
    let col = "ABCDEFGHJKLMNPQRSTUVWXYZ".chars().nth(col_set * 8 + col_idx as usize)?;
    let row_idx = ((y / 100_000.0).floor() as i64 + if zone % 2 == 0 { 5 } else { 0 }).rem_euclid(20) as usize;
    let row = ROWS.chars().nth(row_idx)?;
    let d = digits.clamp(1, 5);
    let scale = 10f64.powi(5 - d as i32);
    let e = ((x % 100_000.0) / scale).floor() as u64;
    let n = ((y % 100_000.0) / scale).floor() as u64;
    Some(format!("{}{} {}{} {:0w$} {:0w$}", zone, band, col, row, e, n, w = d))
}

/// Latitude and longitude of the middle of the square a military grid reference names.
pub fn from_mgrs(text: &str) -> Option<(f64, f64)> {
    let s: String = text.to_uppercase().chars().filter(|c| !c.is_whitespace()).collect();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == 0 || i > 2 || b.len() < i + 3 {
        return None;
    }
    let zone: u32 = s[..i].parse().ok()?;
    if !(1..=60).contains(&zone) {
        return None;
    }
    let band = b[i] as char;
    let band_i = BANDS.find(band)?;
    let (c, r) = (b[i + 1] as char, b[i + 2] as char);
    let digits = &s[i + 3..];
    if !digits.chars().all(|c| c.is_ascii_digit()) || digits.len() % 2 != 0 || digits.len() > 10 {
        return None;
    }
    let half = digits.len() / 2;
    let scale = 10f64.powi(5 - half as i32);
    let (e_part, n_part) = if half == 0 { (0.0, 0.0) } else { (digits[..half].parse::<f64>().ok()?, digits[half..].parse::<f64>().ok()?) };
    // the middle of the square the digits name
    let (e_in, n_in) = (e_part * scale + if half == 0 { 50_000.0 } else { scale / 2.0 }, n_part * scale + if half == 0 { 50_000.0 } else { scale / 2.0 });
    let col_all = "ABCDEFGHJKLMNPQRSTUVWXYZ";
    let col_pos = col_all.find(c)?;
    if col_pos / 8 != ((zone - 1) % 3) as usize {
        return None;
    }
    let easting = ((col_pos % 8) as f64 + 1.0) * 100_000.0 + e_in;
    let row_pos = ROWS.find(r)? as i64;
    let row_100k = (row_pos - if zone % 2 == 0 { 5 } else { 0 }).rem_euclid(20) as f64;
    let n0 = row_100k * 100_000.0 + n_in;
    // the row letters repeat every 2,000 km: take the one at or above the bottom of the latitude band
    let band_south = -80.0 + 8.0 * band_i as f64;
    let (_, min_n) = forward(band_south, central_meridian(zone), zone);
    let mut northing = n0;
    while northing < min_n - 1.0 {
        northing += 2_000_000.0;
    }
    let south = band < 'N';
    let (lat, lon) = inverse(easting, northing, zone, south);
    // the answer must fall in (or beside, for a big square) the band that was named
    let got = BANDS.find(band_of(lat)?)? as i64;
    if (got - band_i as i64).abs() > 1 {
        return None;
    }
    Some((lat, lon))
}

/// UTM written as "17S 580000 4370000" (zone and band, easting, northing).
pub fn from_utm(text: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    let zb = parts[0].to_uppercase();
    let band = zb.chars().last()?;
    BANDS.find(band)?;
    let zone: u32 = zb[..zb.len() - 1].parse().ok()?;
    if !(1..=60).contains(&zone) {
        return None;
    }
    let (e, n): (f64, f64) = (parts[1].trim_end_matches(['m', 'M', 'E', 'e']).parse().ok()?, parts[2].trim_end_matches(['m', 'M', 'N', 'n']).parse().ok()?);
    if !(100_000.0..900_000.0).contains(&e) || !(0.0..10_000_000.0).contains(&n) {
        return None;
    }
    Some(inverse(e, n, zone, band < 'N'))
}

fn dms_part(v: f64, pos: char, neg: char) -> String {
    let h = if v < 0.0 { neg } else { pos };
    let tenths = (v.abs() * 3600.0 * 10.0).round() as u64;
    let (d, rest) = (tenths / 36000, tenths % 36000);
    let (m, s10) = (rest / 600, rest % 600);
    format!("{}°{:02}'{:02}.{}\"{}", d, m, s10 / 10, s10 % 10, h)
}

/// Degrees, minutes and seconds: `39°29'06.5"N 80°08'21.4"W`.
pub fn to_dms(lat: f64, lon: f64) -> String {
    format!("{} {}", dms_part(lat, 'N', 'S'), dms_part(lon, 'E', 'W'))
}

/// Pull the numbers and the compass letters out of a written position.
fn tokens(s: &str) -> Option<Vec<Result<f64, char>>> {
    let mut out = Vec::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c.is_ascii_digit() || ((c == '-' || c == '+' || c == '−') && cs.get(i + 1).map(|d| d.is_ascii_digit() || *d == '.').unwrap_or(false)) || (c == '.' && cs.get(i + 1).map(|d| d.is_ascii_digit()).unwrap_or(false)) {
            let mut t = String::new();
            if c == '−' {
                t.push('-');
            } else {
                t.push(c);
            }
            i += 1;
            while i < cs.len() && (cs[i].is_ascii_digit() || cs[i] == '.') {
                t.push(cs[i]);
                i += 1;
            }
            out.push(Ok(t.parse::<f64>().ok()?));
        } else if matches!(c.to_ascii_uppercase(), 'N' | 'S' | 'E' | 'W') && c.is_ascii() {
            // a word such as "North" or "West" counts as its first letter; a letter inside a longer word does not
            let mut j = i;
            while j < cs.len() && cs[j].is_ascii_alphabetic() {
                j += 1;
            }
            let word: String = cs[i..j].iter().collect::<String>().to_uppercase();
            if word.len() == 1 || matches!(word.as_str(), "NORTH" | "SOUTH" | "EAST" | "WEST") {
                out.push(Err(word.chars().next()?));
            } else {
                return None;
            }
            i = j;
        } else if c.is_ascii_alphabetic() {
            return None;
        } else {
            i += 1; // degree, minute, second marks, commas, spaces and the like
        }
    }
    Some(out)
}

/// A position written as degrees-minutes-seconds (or degrees and decimal minutes), with the
/// compass letters before or after each part: `39°29'06.5"N 80°08'21.4"W`, `N 39 29 06.5, W 80 08 21.4`,
/// `39 29.108 N 80 08.357 W`.
pub fn parse_dms(text: &str) -> Option<(f64, f64)> {
    let toks = tokens(text)?;
    let has_marks = text.chars().any(|c| matches!(c, '°' | '\'' | '′' | '’' | '"' | '″' | '”' | 'º'));
    let has_letters = toks.iter().any(|t| t.is_err());
    let count = toks.iter().filter(|t| t.is_ok()).count();
    if !(has_marks || has_letters || count >= 4) {
        return None;
    }
    // split into the two parts: a compass letter before the numbers starts a part, one after them ends it
    let mut groups: Vec<(Vec<f64>, Option<char>)> = Vec::new();
    let mut nums: Vec<f64> = Vec::new();
    let mut lead: Option<char> = None;
    for t in toks {
        match t {
            Err(h) => {
                if nums.is_empty() {
                    lead = Some(h);
                } else if lead.is_some() {
                    groups.push((std::mem::take(&mut nums), lead.take()));
                    lead = Some(h);
                } else {
                    groups.push((std::mem::take(&mut nums), Some(h)));
                }
            }
            Ok(v) => {
                if nums.len() == 3 {
                    groups.push((std::mem::take(&mut nums), lead.take()));
                }
                nums.push(v);
            }
        }
        if groups.len() > 2 {
            return None;
        }
    }
    if !nums.is_empty() {
        groups.push((nums, lead));
    }
    if groups.len() != 2 {
        return None;
    }
    let mut vals: Vec<(f64, Option<char>)> = Vec::new();
    for (n, h) in &groups {
        if n.is_empty() || n.len() > 3 {
            return None;
        }
        let (d, m, s) = (n[0], n.get(1).copied().unwrap_or(0.0), n.get(2).copied().unwrap_or(0.0));
        if !(0.0..60.0).contains(&m) || !(0.0..60.0).contains(&s) {
            return None;
        }
        let neg = d < 0.0 || matches!(h, Some('S') | Some('W'));
        let v = d.abs() + m / 60.0 + s / 3600.0;
        vals.push((if neg { -v } else { v }, *h));
    }
    let (mut a, mut b) = (vals[0], vals[1]);
    let is_lon = |h: Option<char>| matches!(h, Some('E') | Some('W'));
    let is_lat = |h: Option<char>| matches!(h, Some('N') | Some('S'));
    if is_lon(a.1) || is_lat(b.1) {
        std::mem::swap(&mut a, &mut b);
    }
    if is_lat(a.1) && is_lat(b.1) || is_lon(a.1) && is_lon(b.1) {
        return None;
    }
    let (lat, lon) = (a.0, b.0);
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

/// A position typed in any of the ways: decimal degrees ("39.4, -80.1"), degrees-minutes-seconds,
/// a military grid reference, or UTM. None when the text is not a position.
pub fn parse_any(text: &str) -> Option<(f64, f64)> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(p) = crate::places::parse_coordinates(t) {
        return Some(p);
    }
    if let Some(p) = from_utm(t) {
        return Some(p);
    }
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let starts_zone = compact.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
    if starts_zone && compact.chars().any(|c| c.is_ascii_alphabetic()) {
        if let Some(p) = from_mgrs(t) {
            return Some(p);
        }
    }
    parse_dms(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: (f64, f64), b: (f64, f64), m: f64) -> bool {
        crate::maps::haversine_km(a, b) * 1000.0 <= m
    }

    #[test]
    fn known_places_have_the_known_grid_references() {
        // The Pentagon: 38.8719 N, 77.0563 W is in zone 18S, square UJ (the example in the notes, "UU", is only a sample of the shape).
        let m = to_mgrs(38.8719, -77.0563, 4).unwrap();
        assert!(m.starts_with("18S UJ "), "{}", m);
        // the Statue of Liberty is 18T WL
        assert!(to_mgrs(40.6892, -74.0445, 4).unwrap().starts_with("18T WL "));
        // Sydney Opera House: 56H LH
        assert!(to_mgrs(-33.8568, 151.2153, 4).unwrap().starts_with("56H LH "), "{:?}", to_mgrs(-33.8568, 151.2153, 4));
        // the equator and the prime meridian: 31N AA
        assert!(to_mgrs(0.0, 0.0, 5).unwrap().starts_with("31N AA "));
        // outside the grid
        assert!(to_mgrs(85.0, 0.0, 4).is_none());
    }

    #[test]
    fn a_grid_reference_round_trips_to_within_its_square() {
        for (lat, lon) in [(39.48513, -80.13929), (38.8719, -77.0563), (-33.8568, 151.2153), (64.15, -21.94), (47.6, 7.6), (-54.8, -68.3), (71.0, 25.0)] {
            for d in [5usize, 4, 3] {
                let m = to_mgrs(lat, lon, d).unwrap();
                let back = from_mgrs(&m).unwrap_or_else(|| panic!("{}", m));
                let tol = 10f64.powi(5 - d as i32) * 1.0;
                assert!(close(back, (lat, lon), tol), "{} {} -> {} -> {:?} ({} m)", lat, lon, m, back, crate::maps::haversine_km(back, (lat, lon)) * 1000.0);
            }
        }
    }

    #[test]
    fn utm_round_trips_very_closely() {
        let (x, y) = forward(39.48513, -80.13929, 17);
        let back = inverse(x, y, 17, false);
        assert!(close(back, (39.48513, -80.13929), 0.01), "{:?}", back);
        let (x, y) = forward(-33.8568, 151.2153, 56);
        assert!(close(inverse(x, y, 56, true), (-33.8568, 151.2153), 0.01));
        // a known point: the middle of zone 17 at the equator is easting 500000, northing 0
        let (x, y) = forward(0.0, -81.0, 17);
        assert!((x - 500_000.0).abs() < 0.01 && y.abs() < 0.01);
    }

    #[test]
    fn degrees_minutes_seconds_are_written_and_read() {
        assert_eq!(to_dms(39.48513, -80.13929), "39°29'06.5\"N 80°08'21.4\"W");
        assert_eq!(to_dms(-33.8568, 151.2153), "33°51'24.5\"S 151°12'55.1\"E");
        let want = (39.0 + 29.0 / 60.0 + 6.5 / 3600.0, -(80.0 + 8.0 / 60.0 + 21.4 / 3600.0));
        for s in [
            "39°29'06.5\"N 80°08'21.4\"W",
            "39°29'06.5\"N, 80°08'21.4\"W",
            "N 39 29 06.5, W 80 08 21.4",
            "39 29 06.5 N 80 08 21.4 W",
            "39°29′06.5″N 80°08′21.4″W",
            "N39°29'06.5\" W80°08'21.4\"",
            "39 29 6.5 N 80 8 21.4 W",
            "39d 29' 06.5\" North 80d 08' 21.4\" West".replace('d', "°").as_str(),
        ] {
            let got = parse_dms(s).unwrap_or_else(|| panic!("{}", s));
            assert!(close(got, want, 0.5), "{} -> {:?}", s, got);
        }
        // degrees and decimal minutes
        let g = parse_dms("39°29.108'N 80°08.357'W").unwrap();
        assert!(close(g, want, 2.0), "{:?}", g);
        // east/west first is fine; two latitudes is not
        assert!(close(parse_dms("80°08'21.4\"W 39°29'06.5\"N").unwrap(), want, 0.5));
        assert!(parse_dms("39°29'06.5\"N 40°00'00\"N").is_none());
        assert!(parse_dms("39°70'06.5\"N 80°08'21.4\"W").is_none());
        assert!(parse_dms("Denver, CO").is_none());
    }

    #[test]
    fn the_one_box_finds_any_of_them() {
        let want = (39.48513, -80.13929);
        assert!(close(parse_any("39.48513, -80.13929").unwrap(), want, 1.0));
        assert!(close(parse_any("39°29'06.5\"N 80°08'21.4\"W").unwrap(), want, 3.0));
        let m = to_mgrs(want.0, want.1, 5).unwrap();
        assert!(close(parse_any(&m).unwrap(), want, 2.0), "{}", m);
        assert!(close(parse_any(&m.to_lowercase()).unwrap(), want, 2.0));
        assert!(close(parse_any(&m.replace(' ', "")).unwrap(), want, 2.0));
        let (x, y) = forward(want.0, want.1, 17);
        assert!(close(parse_any(&format!("17S {:.0} {:.0}", x, y)).unwrap(), want, 2.0));
        // ordinary words and addresses are not positions
        for s in ["Denver, CO", "529 Ogden Ave, Fairmont, WV 26554", "26554", "17 Main St", "", "hello"] {
            assert!(parse_any(s).is_none(), "{}", s);
        }
    }

    #[test]
    fn a_grid_reference_with_the_wrong_letters_for_its_zone_is_refused() {
        // zone 17 uses columns J to R; 'A' belongs to zones 16 and 19
        assert!(from_mgrs("17S AA 1234 5678").is_none());
        assert!(from_mgrs("17S LD 123 45").is_none()); // odd number of digits
        assert!(from_mgrs("99S LD 1234 5678").is_none());
    }

    #[test]
    fn the_screen_shows_the_same_text_as_the_core_gives() {
        // These strings are what src/lib/coords.ts produces for the same positions (checked by running it).
        for (lat, lon, mgrs, dms) in [
            (39.48513, -80.13929, "17S ND 7401 7096", "39°29'06.5\"N 80°08'21.4\"W"),
            (38.8719, -77.0563, "18S UJ 2161 0457", "38°52'18.8\"N 77°03'22.7\"W"),
            (-33.8568, 151.2153, "56H LH 3490 5228", "33°51'24.5\"S 151°12'55.1\"E"),
            (64.15, -21.94, "27W VM 5427 1406", "64°09'00.0\"N 21°56'24.0\"W"),
            (0.0, 0.0, "31N AA 6602 0000", "0°00'00.0\"N 0°00'00.0\"E"),
            (-54.8, -68.3, "19F EV 4500 2723", "54°48'00.0\"S 68°18'00.0\"W"),
        ] {
            assert_eq!(to_mgrs(lat, lon, 4).unwrap(), mgrs);
            assert_eq!(to_dms(lat, lon), dms);
        }
    }
}
