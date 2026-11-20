//! Recording a track on the host itself (docs/55, docs/45). A phone host (the Hub) has a GPS; the screen hands each
//! reading to these functions. Nothing here talks to a sensor or the network.
//!
//! While recording, every kept reading is added at once to `user/security/tracks/<id>.part` (so a crash or a flat battery
//! loses at most the last reading). Stopping turns the part file into an ordinary track sheet (`<id>.txt`, the same kind
//! Pocket sends), so the Walked tracks list, GPX copy-out and the lock all work on it unchanged. Air pressure, when the
//! device has it, is kept beside the track in `<id>.pressure`, because the track sheet format has no column for it.
//!
//! Two filters keep the line clean: a reading whose accuracy is worse than the limit is dropped (the idea from
//! Trackbook, docs/55), and a reading too near the last kept one in both distance and time is dropped.

use crate::fsio;
use crate::security::{self, valid_id};
use crate::tracksheet::{self as ts, Point};
use std::fs;
use std::path::{Path, PathBuf};

/// Readings less accurate than this (metres) are dropped.
pub const MAX_ACCURACY_M: f64 = 30.0;
/// A reading is kept when it is at least this far (metres) from the last kept one ...
pub const MIN_STEP_M: f64 = 5.0;
/// ... or at least this long (seconds) after it.
pub const MIN_GAP_S: i64 = 60;
const MAX_POINTS: usize = 20_000;
/// The device code for tracks made on the host.
pub const DEVICE: &str = "host";

#[derive(Debug, Clone, PartialEq)]
pub enum Taken {
    Kept,
    /// Left out, with the plain reason.
    Skipped(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub id: String,
    pub person: String,
    pub outing: String,
    pub points: usize,
    pub last: Option<Point>,
}

fn part_path(user_root: &Path, id: &str) -> PathBuf {
    ts::dir(user_root).join(format!("{}.part", id))
}
fn pressure_path(user_root: &Path, id: &str) -> PathBuf {
    ts::dir(user_root).join(format!("{}.pressure", id))
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(80).collect::<String>().trim().to_string()
}

/// Begin a recording. Only one can be under way at a time.
pub fn start(user_root: &Path, person: &str, outing: &str, now_secs: i64) -> Result<String, String> {
    if current(user_root)?.is_some() {
        return Err("A track is already being recorded. Stop it first.".to_string());
    }
    let person = clean(person);
    if person.is_empty() {
        return Err("Say who is walking this track.".to_string());
    }
    if !outing.is_empty() && !valid_id(outing) {
        return Err("That outing is not valid.".to_string());
    }
    let id = format!("{}-{}", DEVICE, now_secs);
    if ts::dir(user_root).join(format!("{}.txt", id)).exists() {
        return Err("A track was already started this second. Try again.".to_string());
    }
    let head = format!("# format: track-part 1\n# person: {}\n# outing: {}\n", person, outing);
    fsio::append(&part_path(user_root, &id), head.as_bytes()).map_err(|e| format!("The track could not be started: {}", e))?;
    Ok(id)
}

struct Part {
    person: String,
    outing: String,
    points: Vec<Point>,
}

fn read_part(user_root: &Path, id: &str) -> Result<Part, String> {
    if !valid_id(id) || !id.starts_with(&format!("{}-", DEVICE)) {
        return Err("That is not a track being recorded.".to_string());
    }
    let text = fsio::read_to_string(&part_path(user_root, id)).map_err(|_| "That track is not being recorded.".to_string())?;
    let (mut person, mut outing, mut points) = (String::new(), String::new(), Vec::new());
    for l in text.lines() {
        if let Some(r) = l.strip_prefix("# person:") {
            person = r.trim().to_string();
        } else if let Some(r) = l.strip_prefix("# outing:") {
            outing = r.trim().to_string();
        } else if l.starts_with('#') || l.trim().is_empty() {
        } else {
            // a half-written last line is simply not a point
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() >= 3 {
                if let (Ok(t), Ok(lat), Ok(lon)) = (f[0].parse::<i64>(), f[1].parse::<f64>(), f[2].parse::<f64>()) {
                    let ele = f.get(3).and_then(|e| e.parse::<f64>().ok());
                    points.push(Point { t, lat, lon, ele });
                }
            }
        }
    }
    Ok(Part { person, outing, points })
}

/// The recording under way (or left by a crash), if any.
pub fn current(user_root: &Path) -> Result<Option<Recording>, String> {
    let rd = match fs::read_dir(ts::dir(user_root)) {
        Ok(rd) => rd,
        Err(_) => return Ok(None),
    };
    let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".part")).collect();
    names.sort();
    let Some(n) = names.pop() else { return Ok(None) };
    let id = n.trim_end_matches(".part").to_string();
    let p = read_part(user_root, &id)?;
    Ok(Some(Recording { id, person: p.person, outing: p.outing, points: p.points.len(), last: p.points.last().cloned() }))
}

/// Whether a reading should be kept, given the last kept point. Pure, so it is easy to test.
pub fn decide(last: Option<&Point>, now: &Point, accuracy: Option<f64>) -> Taken {
    if !(-90.0..=90.0).contains(&now.lat) || !(-180.0..=180.0).contains(&now.lon) || !now.lat.is_finite() || !now.lon.is_finite() {
        return Taken::Skipped("The position was not a real place.");
    }
    if let Some(a) = accuracy {
        if !a.is_finite() || a > MAX_ACCURACY_M {
            return Taken::Skipped("The signal was too weak to trust.");
        }
    }
    if let Some(l) = last {
        if now.t < l.t {
            return Taken::Skipped("The time went backwards.");
        }
        if ts::distance(l, now) < MIN_STEP_M && now.t - l.t < MIN_GAP_S {
            return Taken::Skipped("Not moved far enough yet.");
        }
    }
    Taken::Kept
}

/// Offer one reading. `pressure_hpa` is optional (a phone barometer).
pub fn add_point(user_root: &Path, id: &str, t: i64, lat: f64, lon: f64, ele: Option<f64>, accuracy: Option<f64>, pressure_hpa: Option<f64>) -> Result<Taken, String> {
    let part = read_part(user_root, id)?;
    if part.points.len() >= MAX_POINTS {
        return Err("This track is very long. Stop it and start a new one.".to_string());
    }
    let ele = ele.filter(|e| e.is_finite() && (-500.0..=9000.0).contains(e));
    let p = Point { t, lat, lon, ele };
    if !(0..4_102_444_800).contains(&t) {
        return Ok(Taken::Skipped("The time was not readable."));
    }
    let verdict = decide(part.points.last(), &p, accuracy);
    if verdict == Taken::Kept {
        let line = format!("{}\t{:.6}\t{:.6}\t{}\n", t, lat, lon, ele.map(|e| format!("{:.1}", e)).unwrap_or_default());
        fsio::append(&part_path(user_root, id), line.as_bytes()).map_err(|e| format!("The point could not be saved: {}", e))?;
    }
    // air pressure is worth keeping even when the position was dropped
    if let Some(h) = pressure_hpa.filter(|h| h.is_finite() && (300.0..=1100.0).contains(h)) {
        let _ = fsio::append(&pressure_path(user_root, id), format!("{}\t{:.1}\n", t, h).as_bytes());
    }
    Ok(verdict)
}

/// Stop and keep the track. With fewer than two kept points there is no track: the part file is removed and the
/// answer says so.
pub fn stop(user_root: &Path, id: &str, now_minutes: i64) -> Result<String, String> {
    let part = read_part(user_root, id)?;
    let pf = part_path(user_root, id);
    if part.points.len() < 2 {
        let _ = fs::remove_file(&pf);
        let _ = fs::remove_file(pressure_path(user_root, id));
        return Err("The track had fewer than two points, so nothing was kept.".to_string());
    }
    let created = security::format_instant(now_minutes, 0);
    let text = ts::render(DEVICE, &part.person, &created, id, &part.outing, &part.points);
    // make sure what we are about to keep is one the rest of the program will read
    ts::parse(&text)?;
    let target = ts::dir(user_root).join(format!("{}.txt", id));
    if target.exists() {
        return Err("A track with this ID is already kept.".to_string());
    }
    fsio::write_atomic(&target, text.as_bytes()).map_err(|e| format!("The track could not be kept: {}", e))?;
    let _ = fs::remove_file(&pf);
    Ok(id.to_string())
}

/// Throw a recording away (for example one left by a crash that the person does not want).
pub fn discard(user_root: &Path, id: &str) -> Result<(), String> {
    read_part(user_root, id)?;
    let _ = fs::remove_file(part_path(user_root, id));
    let _ = fs::remove_file(pressure_path(user_root, id));
    Ok(())
}

/// Air pressure readings `(seconds, hPa)` kept for a track.
pub fn pressure(user_root: &Path, id: &str) -> Vec<(i64, f64)> {
    if !valid_id(id) {
        return Vec::new();
    }
    let text = fsio::read_to_string(&pressure_path(user_root, id)).unwrap_or_default();
    text.lines().filter_map(|l| {
        let (a, b) = l.split_once('\t')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    }).collect()
}

/// A plain hint from the air pressure over the last three hours of readings. It is a hint, never a forecast.
pub fn weather_hint(samples: &[(i64, f64)]) -> Option<String> {
    let (&(t_end, p_end), rest) = samples.split_last()?;
    let window = 3 * 3600;
    let first = rest.iter().find(|(t, _)| t_end - t <= window)?;
    let hours = (t_end - first.0) as f64 / 3600.0;
    if hours < 1.0 {
        return None;
    }
    let change = p_end - first.1;
    // scale to three hours so a short stretch is not over-read
    let per3 = change * 3.0 / hours;
    let words = if per3 <= -3.0 {
        "The air pressure is falling fast. Weather may turn worse soon."
    } else if per3 <= -1.0 {
        "The air pressure is slowly falling. Weather may be changing."
    } else if per3 >= 3.0 {
        "The air pressure is rising fast. Weather is likely clearing, often with wind."
    } else {
        return None;
    };
    Some(format!("{} It changed by {:+.1} hPa over {:.1} hours. This is a hint, not a forecast.", words, change, hours))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(n: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-rec-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(&r).unwrap();
        r
    }

    #[test]
    fn weak_and_nearby_readings_are_dropped() {
        let a = Point { t: 1000, lat: 36.0, lon: -84.0, ele: None };
        assert_eq!(decide(None, &a, Some(10.0)), Taken::Kept);
        assert!(matches!(decide(None, &a, Some(80.0)), Taken::Skipped(_)));
        let near = Point { t: 1010, lat: 36.00001, lon: -84.0, ele: None };
        assert!(matches!(decide(Some(&a), &near, Some(5.0)), Taken::Skipped(_)));
        let later = Point { t: 1100, ..near.clone() };
        assert_eq!(decide(Some(&a), &later, Some(5.0)), Taken::Kept, "standing still for a minute is kept");
        let far = Point { t: 1010, lat: 36.001, lon: -84.0, ele: None };
        assert_eq!(decide(Some(&a), &far, None), Taken::Kept);
        let back = Point { t: 900, lat: 36.1, lon: -84.0, ele: None };
        assert!(matches!(decide(Some(&a), &back, None), Taken::Skipped(_)));
        let nowhere = Point { t: 2000, lat: 99.0, lon: 0.0, ele: None };
        assert!(matches!(decide(None, &nowhere, None), Taken::Skipped(_)));
    }

    #[test]
    fn a_recording_is_saved_as_it_goes_and_becomes_a_normal_track() {
        let r = root("full");
        let id = start(&r, "Sam", "", 1_700_000_000).unwrap();
        assert_eq!(id, "host-1700000000");
        assert!(start(&r, "Sam", "", 1_700_000_001).is_err(), "only one at a time");
        for i in 0..4 {
            let t = 1_700_000_000 + i * 30;
            let v = add_point(&r, &id, t, 36.0 + i as f64 * 0.001, -84.0, Some(300.0 + i as f64 * 5.0), Some(8.0), Some(1000.0)).unwrap();
            assert_eq!(v, Taken::Kept);
        }
        // a weak fix is dropped and does not reach the file
        assert!(matches!(add_point(&r, &id, 1_700_000_200, 37.0, -84.0, None, Some(500.0), None).unwrap(), Taken::Skipped(_)));
        // crash: the recording is found again with its points
        let cur = current(&r).unwrap().unwrap();
        assert_eq!((cur.id.as_str(), cur.points, cur.person.as_str()), (id.as_str(), 4, "Sam"));
        let kept = stop(&r, &id, 28_333_333).unwrap();
        assert!(current(&r).unwrap().is_none());
        let sheet = ts::get(&r, &kept).unwrap();
        assert_eq!((sheet.points.len(), sheet.person.as_str(), sheet.device.as_str()), (4, "Sam", "host"));
        assert!(ts::list(&r).iter().any(|t| t.id == kept && t.stats.metres > 300.0), "it shows in the Walked tracks list");
        assert!(ts::gpx(&sheet).contains("<trkpt"));
        assert_eq!(pressure(&r, &id).len(), 4);
    }

    #[test]
    fn a_half_written_last_line_is_ignored_and_a_short_track_keeps_nothing() {
        let r = root("torn");
        let id = start(&r, "Pat", "", 1_700_000_000).unwrap();
        add_point(&r, &id, 1_700_000_000, 36.0, -84.0, None, None, None).unwrap();
        fs::OpenOptions::new().append(true).open(part_path(&r, &id)).unwrap().write_all_for_test(b"1700000100\t36.0");
        assert_eq!(current(&r).unwrap().unwrap().points, 1);
        let e = stop(&r, &id, 28_333_333).unwrap_err();
        assert!(e.contains("fewer than two"));
        assert!(current(&r).unwrap().is_none() && ts::list(&r).is_empty());
        assert!(discard(&r, "host-nothing").is_err());
        assert!(start(&r, "  ", "", 5).is_err());
        assert!(start(&r, "Pat", "../x", 5).is_err());
    }

    #[test]
    fn the_weather_hint_needs_a_real_fall_over_enough_time() {
        let h = 3600;
        assert!(weather_hint(&[]).is_none());
        assert!(weather_hint(&[(0, 1013.0), (600, 1010.0)]).is_none(), "ten minutes is too short to say");
        let fall = [(0, 1013.0), (h, 1011.5), (2 * h, 1010.0), (3 * h, 1009.5)];
        assert!(weather_hint(&fall).unwrap().contains("falling fast"));
        let slow = [(0, 1013.0), (3 * h, 1011.5)];
        assert!(weather_hint(&slow).unwrap().contains("slowly falling"));
        let steady = [(0, 1013.0), (3 * h, 1013.4)];
        assert!(weather_hint(&steady).is_none());
        let rise = [(0, 1005.0), (3 * h, 1009.0)];
        assert!(weather_hint(&rise).unwrap().contains("rising"));
    }

    trait W {
        fn write_all_for_test(self, b: &[u8]);
    }
    impl W for fs::File {
        fn write_all_for_test(mut self, b: &[u8]) {
            use std::io::Write;
            self.write_all(b).unwrap();
        }
    }
}
