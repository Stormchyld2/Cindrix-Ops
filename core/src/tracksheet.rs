//! Tracks recorded by a phone's GPS (docs/45). Pocket records where its person walked, with the distance and the climb,
//! and sends the whole track as a **track sheet** to the same waiting mailbox as reports (`user/inbox/pocket/`). Nothing
//! is kept until the host accepts it. An accepted track is stored as its own file under `user/security/tracks/`
//! (so the passcode lock covers it) and never changes any other record.
//!
//! ```text
//! # format: track-sheet 1
//! # device: <the phone's code>
//! # person: <who walked it>
//! # created: 2026-11-08T10:00-05:00
//! # track: <track id, starting with the device code and a dash>
//! # outing: <outing id, optional>
//! <seconds since 1970 (UTC)> TAB <latitude> TAB <longitude> TAB <metres above sea level, or empty>
//! ```
//! Rules: at least two points; times never go backwards; latitude and longitude in range; a track is all or nothing; the
//! same file twice changes nothing; a different file claiming a track id already kept is refused.

use crate::fsio;
use crate::security::{self, valid_id};
use crate::security_pocket as pocket;
use crate::security_store as store;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_POINTS: usize = 20_000;
/// A climb or drop is counted only after the height has moved this far from the last counted height, so GPS wobble
/// does not add up to a mountain.
const CLIMB_STEP_M: f64 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub t: i64,
    pub lat: f64,
    pub lon: f64,
    pub ele: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sheet {
    pub device: String,
    pub person: String,
    pub created: String,
    pub id: String,
    pub outing: String,
    pub points: Vec<Point>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    pub points: usize,
    pub metres: f64,
    pub climb: f64,
    pub drop: f64,
    pub low: Option<f64>,
    pub high: Option<f64>,
    pub seconds: i64,
    pub start: i64,
}

pub fn is_sheet(text: &str) -> bool {
    text.lines().find_map(|l| l.strip_prefix("# format:")).map(|f| f.split_whitespace().next() == Some("track-sheet")).unwrap_or(false)
}

fn num(s: &str) -> Option<f64> {
    let n: f64 = s.trim().parse().ok()?;
    n.is_finite().then_some(n)
}

pub fn parse(text: &str) -> Result<Sheet, String> {
    let (mut ok, mut device, mut person, mut created, mut id, mut outing) = (false, String::new(), String::new(), String::new(), String::new(), String::new());
    let mut points: Vec<Point> = Vec::new();
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("# format:") {
            ok = r.split_whitespace().collect::<Vec<_>>() == ["track-sheet", "1"];
            if !ok {
                return Err("This track is from a different version of Pocket, so it was left alone.".to_string());
            }
        } else if let Some(r) = l.strip_prefix("# device:") {
            device = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# person:") {
            person = r.trim().chars().take(80).collect();
        } else if let Some(r) = l.strip_prefix("# created:") {
            created = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# track:") {
            id = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# outing:") {
            outing = r.trim().to_string();
        } else if l.starts_with('#') || l.trim().is_empty() {
        } else {
            let f: Vec<&str> = l.split('\t').collect();
            let p = (f.len() == 4)
                .then(|| {
                    let t: i64 = f[0].trim().parse().ok()?;
                    let (lat, lon) = (num(f[1])?, num(f[2])?);
                    let ele = if f[3].trim().is_empty() { None } else { Some(num(f[3])?) };
                    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) && (0..4_102_444_800).contains(&t) && ele.map(|e| (-500.0..=9000.0).contains(&e)).unwrap_or(true)).then_some(Point { t, lat, lon, ele })
                })
                .flatten();
            match p {
                Some(p) if points.last().map(|q| p.t >= q.t).unwrap_or(true) => points.push(p),
                _ => return Err("A point in this track could not be read, or the times go backwards.".to_string()),
            }
            if points.len() > MAX_POINTS {
                return Err("This track has far more points than expected.".to_string());
            }
        }
    }
    if !ok {
        return Err("This is not a track sheet.".to_string());
    }
    if !valid_id(&device) || device.len() > 12 {
        return Err("The track does not name its phone properly.".to_string());
    }
    if person.trim().is_empty() {
        return Err("The track does not say who walked it.".to_string());
    }
    if security::parse_instant(&created).is_none() {
        return Err("The track has no readable date.".to_string());
    }
    if !valid_id(&id) || !id.starts_with(&format!("{}-", device)) {
        return Err("The track's ID does not belong to the phone that sent it.".to_string());
    }
    if !outing.is_empty() && !valid_id(&outing) {
        return Err("The outing named on the track is not valid.".to_string());
    }
    if points.len() < 2 {
        return Err("The track has fewer than two points.".to_string());
    }
    Ok(Sheet { device, person, created, id, outing, points, fingerprint: pocket::fingerprint(text.as_bytes()) })
}

pub fn read(path: &Path) -> Result<Sheet, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "The file could not be opened.".to_string())?;
    if !meta.is_file() {
        return Err("This is not an ordinary file, so it was left alone.".to_string());
    }
    if meta.len() > 2_000_000 {
        return Err("This file is much bigger than any track should be.".to_string());
    }
    let data = fsio::read(path).map_err(|_| "The file could not be read.".to_string())?;
    parse(&String::from_utf8(data).map_err(|_| "This is not plain text.".to_string())?)
}

fn haversine(a: &Point, b: &Point) -> f64 {
    let r = 6_371_000.0_f64;
    let (p1, p2) = (a.lat.to_radians(), b.lat.to_radians());
    let (dp, dl) = ((b.lat - a.lat).to_radians(), (b.lon - a.lon).to_radians());
    let h = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

pub fn stats(points: &[Point]) -> Stats {
    let mut s = Stats { points: points.len(), ..Default::default() };
    if points.is_empty() {
        return s;
    }
    s.start = points[0].t;
    s.seconds = points[points.len() - 1].t - points[0].t;
    for w in points.windows(2) {
        s.metres += haversine(&w[0], &w[1]);
    }
    let mut reference: Option<f64> = None;
    for e in points.iter().filter_map(|p| p.ele) {
        s.low = Some(s.low.map_or(e, |l: f64| l.min(e)));
        s.high = Some(s.high.map_or(e, |h: f64| h.max(e)));
        match reference {
            None => reference = Some(e),
            Some(r) if e - r >= CLIMB_STEP_M => {
                s.climb += e - r;
                reference = Some(e);
            }
            Some(r) if r - e >= CLIMB_STEP_M => {
                s.drop += r - e;
                reference = Some(e);
            }
            _ => {}
        }
    }
    s
}

pub fn dir(user_root: &Path) -> PathBuf {
    store::dir(user_root).join("tracks")
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    pub id: String,
    pub person: String,
    pub device: String,
    pub created: String,
    pub outing: String,
    pub stats: Stats,
}

pub fn list(user_root: &Path) -> Vec<TrackInfo> {
    let mut v: Vec<TrackInfo> = match fs::read_dir(dir(user_root)) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".txt"))
            .filter_map(|e| read(&e.path()).ok())
            .map(|s| TrackInfo { stats: stats(&s.points), id: s.id, person: s.person, device: s.device, created: s.created, outing: s.outing })
            .collect(),
        Err(_) => Vec::new(),
    };
    v.sort_by(|a, b| b.stats.start.cmp(&a.stats.start).then(a.id.cmp(&b.id)));
    v
}

pub fn get(user_root: &Path, id: &str) -> Result<Sheet, String> {
    if !valid_id(id) {
        return Err("That is not a track.".to_string());
    }
    read(&dir(user_root).join(format!("{}.txt", id)))
}

/// Accept a waiting track. Returns 1 when it was kept, 0 when this very file was kept before.
pub fn accept(user_root: &Path, name: &str) -> Result<usize, String> {
    if !pocket::safe_name(name) {
        return Err("That is not a file name Pocket would make.".to_string());
    }
    let path = pocket::inbox_dir(user_root).join(name);
    let sheet = read(&path)?;
    let before = pocket::accepted(user_root).iter().any(|(n, f)| n == name && *f == sheet.fingerprint);
    let mut kept = 0;
    if !before {
        let target = dir(user_root).join(format!("{}.txt", sheet.id));
        if target.exists() {
            return Err("A different track with this ID is already kept. Set this one aside.".to_string());
        }
        fs::create_dir_all(dir(user_root)).map_err(|_| "The tracks folder could not be made.".to_string())?;
        let bytes = fsio::read(&path).map_err(|_| "The file could not be read.".to_string())?;
        fsio::write_atomic(&target, &bytes).map_err(|e| format!("The track could not be kept: {}", e))?;
        let now = security::format_instant(crate::security_api::now_minutes(), 0);
        let line = format!("{}\t{}\t{}\t{}\t{}\n", name, sheet.fingerprint, sheet.device, sheet.points.len(), now);
        fsio::append(&pocket::ledger_path(user_root), line.as_bytes()).map_err(|_| "The ledger could not be written.".to_string())?;
        kept = 1;
    }
    pocket::move_into(&pocket::done_dir(user_root), &path, name)?;
    Ok(kept)
}

pub fn render(device: &str, person: &str, created: &str, id: &str, outing: &str, points: &[Point]) -> String {
    let mut o = format!("# format: track-sheet 1\n# device: {}\n# person: {}\n# created: {}\n# track: {}\n# outing: {}\n", device, person, created, id, outing);
    for p in points {
        o.push_str(&format!("{}\t{:.6}\t{:.6}\t{}\n", p.t, p.lat, p.lon, p.ele.map(|e| format!("{:.1}", e)).unwrap_or_default()));
    }
    o
}

/// A GPX 1.1 file of the track, for other map programs (the standalone map viewer reads GPX).
pub fn gpx(s: &Sheet) -> String {
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut o = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Cindrix Ops\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n");
    o.push_str(&format!("<trk><name>{}</name><trkseg>\n", esc(&format!("{} {}", s.person, s.id))));
    for p in &s.points {
        o.push_str(&format!("<trkpt lat=\"{:.6}\" lon=\"{:.6}\">", p.lat, p.lon));
        if let Some(e) = p.ele {
            o.push_str(&format!("<ele>{:.1}</ele>", e));
        }
        o.push_str(&format!("<time>{}</time></trkpt>\n", utc_text(p.t)));
    }
    o.push_str("</trkseg></trk>\n</gpx>\n");
    o
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ` (civil-from-days, no outside code).
pub fn utc_text(t: i64) -> String {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, secs / 3600, (secs % 3600) / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(t: i64, lat: f64, lon: f64, ele: Option<f64>) -> Point {
        Point { t, lat, lon, ele }
    }
    fn created() -> String {
        security::format_instant(crate::security_api::now_minutes(), 0)
    }
    fn setup(name: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-trk-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(&r).unwrap();
        r
    }
    fn walk() -> Vec<Point> {
        // about 111 m north per 0.001 degree of latitude
        vec![pt(1_000, 38.000, -80.0, Some(500.0)), pt(1_060, 38.001, -80.0, Some(510.0)), pt(1_120, 38.002, -80.0, Some(506.0)), pt(1_180, 38.002, -80.001, Some(506.5))]
    }

    #[test]
    fn distance_climb_and_time_come_out_right() {
        let s = stats(&walk());
        assert!((s.metres - (111.2 * 2.0 + 87.7)).abs() < 3.0, "{}", s.metres);
        assert_eq!((s.climb, s.drop, s.seconds), (10.0, 4.0, 180));
        assert_eq!((s.low, s.high), (Some(500.0), Some(510.0)));
    }

    #[test]
    fn gps_wobble_under_three_metres_is_not_counted_as_climbing() {
        let wobble: Vec<Point> = (0..20).map(|i| pt(i, 38.0, -80.0, Some(500.0 + if i % 2 == 0 { 1.0 } else { -1.0 }))).collect();
        let s = stats(&wobble);
        assert_eq!((s.climb, s.drop), (0.0, 0.0));
    }

    #[test]
    fn a_track_with_no_heights_still_has_a_distance() {
        let s = stats(&[pt(0, 38.0, -80.0, None), pt(10, 38.001, -80.0, None)]);
        assert!(s.metres > 100.0 && s.climb == 0.0 && s.low.is_none());
    }

    #[test]
    fn a_track_survives_writing_and_reading() {
        let text = render("n10", "Sam", &created(), "n10-t1", "e3", &walk());
        let s = parse(&text).unwrap();
        assert_eq!((s.points.len(), s.id.as_str(), s.outing.as_str(), s.person.as_str()), (4, "n10-t1", "e3", "Sam"));
        assert_eq!(s.points[1].ele, Some(510.0));
    }

    #[test]
    fn bad_tracks_are_refused() {
        let ok = render("n10", "Sam", &created(), "n10-t1", "", &walk());
        assert!(parse(&ok).is_ok());
        assert!(parse(&ok.replace("38.001000", "98.001000")).is_err(), "latitude out of range");
        assert!(parse(&ok.replace("1060\t", "900\t")).is_err(), "time going backwards");
        assert!(parse(&ok.replace("# track: n10-t1", "# track: n11-t1")).is_err(), "another phone's id");
        assert!(parse(&ok.replace("# person: Sam\n", "")).is_err());
        assert!(parse(&ok.replace("track-sheet 1", "track-sheet 2")).is_err());
        let one = render("n10", "Sam", &created(), "n10-t1", "", &walk()[..1]);
        assert!(parse(&one).is_err(), "one point is not a track");
        assert!(parse("# format: track-sheet 1\n").is_err());
    }

    #[test]
    fn a_waiting_track_is_kept_once_and_listed_with_its_numbers() {
        let r = setup("keep");
        pocket::drop_file(&r, "n10-track-1.txt", &render("n10", "Sam", &created(), "n10-t1", "", &walk())).unwrap();
        assert_eq!(pocket::accept_any(&r, "n10-track-1.txt", false).unwrap(), 1);
        let l = list(&r);
        assert_eq!((l.len(), l[0].id.as_str(), l[0].stats.seconds), (1, "n10-t1", 180));
        // the very same file again changes nothing
        pocket::drop_file(&r, "n10-track-1.txt", &render("n10", "Sam", &created(), "n10-t1", "", &walk())).unwrap();
        assert_eq!(pocket::accept_any(&r, "n10-track-1.txt", false).unwrap(), 0);
        assert_eq!(list(&r).len(), 1);
        // a different file claiming the same track id is refused and the kept track is untouched
        let other: Vec<Point> = walk().into_iter().take(2).collect();
        pocket::drop_file(&r, "n10-track-2.txt", &render("n10", "Sam", &created(), "n10-t1", "", &other)).unwrap();
        assert!(pocket::accept_any(&r, "n10-track-2.txt", false).is_err());
        assert_eq!(get(&r, "n10-t1").unwrap().points.len(), 4);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn kept_tracks_live_under_the_security_folder_so_the_lock_covers_them() {
        let r = setup("where");
        assert!(dir(&r).starts_with(store::dir(&r)));
        assert!(fsio::is_protected_rel(Path::new("security/tracks/n10-t1.txt")));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_gpx_copy_has_every_point_and_a_readable_time() {
        let s = parse(&render("n10", "Sam <&>", &created(), "n10-t1", "", &walk())).unwrap();
        let g = gpx(&s);
        assert_eq!(g.matches("<trkpt").count(), 4);
        assert!(g.contains("Sam &lt;&amp;&gt;") && g.contains("<ele>510.0</ele>"));
        assert_eq!(utc_text(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_text(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(utc_text(1_709_164_800), "2024-02-29T00:00:00Z");
    }
}
