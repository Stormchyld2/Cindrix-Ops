//! Saved maps (doc 2): one folder per map under `user/saved-maps/<id>/` holding
//! `map.json` (pins, rendezvous points, routes, no-go areas, labels, written by the interface),
//! `notes.md`, an optional `picture.png` and an optional `route.gpx`. Also the distance maths and
//! the GPX writer. Standard library only. Nothing here touches the internet.

use crate::fsio;
use std::fs;
use std::path::{Path, PathBuf};

/// Earth radius in kilometres (mean).
const EARTH_KM: f64 = 6371.0088;

/// Great-circle distance between two (latitude, longitude) points in degrees, in kilometres.
pub fn haversine_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (a.0.to_radians(), a.1.to_radians(), b.0.to_radians(), b.1.to_radians());
    let h = ((la2 - la1) / 2.0).sin().powi(2) + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.sqrt().asin()
}

/// Length of a route through `points`, in kilometres.
pub fn route_km(points: &[(f64, f64)]) -> f64 {
    points.windows(2).map(|w| haversine_km(w[0], w[1])).sum()
}

/// Initial compass bearing from `a` to `b`, 0 to 360 degrees (0 is north).
pub fn bearing_deg(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2, dlo) = (a.0.to_radians(), b.0.to_radians(), (b.1 - a.1).to_radians());
    let y = dlo.sin() * la2.cos();
    let x = la1.cos() * la2.sin() - la1.sin() * la2.cos() * dlo.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

/// A point is valid when it is a real latitude and longitude.
pub fn valid_point(p: (f64, f64)) -> bool {
    p.0.is_finite() && p.1.is_finite() && (-90.0..=90.0).contains(&p.0) && (-180.0..=180.0).contains(&p.1)
}

/// Is `p` inside the polygon `poly` (a no-go area)? Planar test on degrees: fine for areas
/// of a few kilometres, which is what a flooded road or a blocked bridge is.
pub fn in_polygon(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (yi, xi) = poly[i];
        let (yj, xj) = poly[j];
        if (yi > p.0) != (yj > p.0) && p.1 < (xj - xi) * (p.0 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A GPX 1.1 file with one route, so phone map apps can open it. Points outside real
/// latitude/longitude ranges are an error rather than a file that fails later.
pub fn gpx(name: &str, points: &[(f64, f64)]) -> Result<String, String> {
    if points.len() < 2 {
        return Err("A route needs at least two points.".into());
    }
    if let Some(bad) = points.iter().find(|p| !valid_point(**p)) {
        return Err(format!("Not a real position: {}, {}.", bad.0, bad.1));
    }
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<gpx version=\"1.1\" creator=\"Cindrix Ops\" xmlns=\"http://www.topografix.com/GPX/1/1\">\n");
    s.push_str(&format!("  <rte>\n    <name>{}</name>\n", xml_escape(name)));
    for (lat, lon) in points {
        s.push_str(&format!("    <rtept lat=\"{:.6}\" lon=\"{:.6}\"/>\n", lat, lon));
    }
    s.push_str("  </rte>\n</gpx>\n");
    Ok(s)
}

/// Pull a string value out of the (flat, top-level) `"name"` field of a map file.
fn name_of(json: &str) -> String {
    let Some(i) = json.find("\"name\"") else { return String::new() };
    let rest = &json[i + 6..];
    let Some(c) = rest.find(':') else { return String::new() };
    let Some(rest) = rest[c + 1..].trim_start().strip_prefix('"') else { return String::new() };
    let mut out = String::new();
    let mut esc = false;
    for ch in rest.chars() {
        if esc {
            out.push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '"' {
            break;
        } else {
            out.push(ch);
        }
    }
    out
}

/// A map file must be a JSON object that says which version wrote it (doc 2: every JSON file
/// is versioned). This is a guard against saving garbage, not a full JSON parser.
pub fn looks_like_map_json(json: &str) -> bool {
    let t = json.trim();
    t.starts_with('{') && t.ends_with('}') && t.contains("\"version\"")
}

/// Folder-safe form of a name: lowercase letters, digits and single hyphens.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    let out: String = out.chars().take(48).collect();
    if out.is_empty() { "map".to_string() } else { out }
}

/// An id is only ever one of our own slugs, so it can never reach outside the saved-maps folder.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 60 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapMeta {
    pub id: String,
    pub name: String,
    pub has_picture: bool,
    pub has_route: bool,
}

pub struct MapStore {
    root: PathBuf,
    backups: PathBuf,
}

impl MapStore {
    /// `user` is the drive's `user/` folder.
    pub fn new(user: &Path) -> MapStore {
        MapStore { root: user.join("saved-maps"), backups: user.join("backups") }
    }

    fn dir(&self, id: &str) -> Result<PathBuf, String> {
        if valid_id(id) { Ok(self.root.join(id)) } else { Err("That is not a valid map name.".into()) }
    }

    /// Every saved map, by name. A folder without a readable map.json is skipped, not an error.
    pub fn list(&self) -> Vec<MapMeta> {
        let Ok(rd) = fs::read_dir(&self.root) else { return vec![] };
        let mut v: Vec<MapMeta> = rd
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let id = e.file_name().to_string_lossy().to_string();
                if !valid_id(&id) {
                    return None;
                }
                let text = fs::read_to_string(e.path().join("map.json")).ok()?;
                let name = name_of(&text);
                Some(MapMeta {
                    name: if name.is_empty() { id.clone() } else { name },
                    has_picture: e.path().join("picture.png").is_file(),
                    has_route: e.path().join("route.gpx").is_file(),
                    id,
                })
            })
            .collect();
        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        v
    }

    /// The map file and its notes. A missing notes file is empty notes.
    pub fn load(&self, id: &str) -> Result<(String, String), String> {
        let d = self.dir(id)?;
        let json = fs::read_to_string(d.join("map.json")).map_err(|e| format!("Could not read the map: {}", e))?;
        let notes = fs::read_to_string(d.join("notes.md")).unwrap_or_default();
        Ok((json, notes))
    }

    /// Save a map. `id` None makes a new folder named from `name`; an existing id is replaced,
    /// with the old map.json copied to `backups/` first. Returns the id.
    pub fn save(&self, id: Option<&str>, name: &str, json: &str, notes: &str) -> Result<String, String> {
        if !looks_like_map_json(json) {
            return Err("That is not a saved map (missing version).".into());
        }
        let id = match id {
            Some(i) => {
                self.dir(i)?;
                i.to_string()
            }
            None => {
                let base = slug(name);
                let mut cand = base.clone();
                let mut n = 2;
                while self.root.join(&cand).exists() {
                    cand = format!("{}-{}", base, n);
                    n += 1;
                }
                cand
            }
        };
        let d = self.dir(&id)?;
        fsio::backup_then_write(&d.join("map.json"), &self.backups, json.as_bytes()).map_err(|e| e.to_string())?;
        fsio::backup_then_write(&d.join("notes.md"), &self.backups, notes.as_bytes()).map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// Write the exported picture. Only a real PNG is accepted.
    pub fn save_picture(&self, id: &str, png: &[u8]) -> Result<(), String> {
        if png.len() < 8 || png[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
            return Err("That is not a PNG picture.".into());
        }
        let d = self.dir(id)?;
        if !d.join("map.json").is_file() {
            return Err("Save the map before adding a picture.".into());
        }
        fsio::write_atomic(&d.join("picture.png"), png).map_err(|e| e.to_string())
    }

    /// Write `route.gpx` for a saved map from its route points.
    pub fn export_gpx(&self, id: &str, name: &str, points: &[(f64, f64)]) -> Result<PathBuf, String> {
        let d = self.dir(id)?;
        if !d.join("map.json").is_file() {
            return Err("Save the map before exporting its route.".into());
        }
        let text = gpx(name, points)?;
        let p = d.join("route.gpx");
        fsio::write_atomic(&p, text.as_bytes()).map_err(|e| e.to_string())?;
        Ok(p)
    }

    /// Remove a saved map. The folder is copied nowhere: this is the person's explicit choice.
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let d = self.dir(id)?;
        if !d.join("map.json").is_file() {
            return Err("There is no such saved map.".into());
        }
        fs::remove_dir_all(d).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-maps-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    const MAP: &str = r#"{ "version": 1, "name": "Home to \"Camp\"", "pins": [] }"#;

    #[test]
    fn distances_are_right() {
        // One degree of latitude is about 111.2 km.
        let d = haversine_km((0.0, 0.0), (1.0, 0.0));
        assert!((d - 111.195).abs() < 0.1, "{}", d);
        // Equator, one degree of longitude is the same.
        assert!((haversine_km((0.0, 0.0), (0.0, 1.0)) - 111.195).abs() < 0.1);
        // New York to Los Angeles is about 3936 km.
        let ny_la = haversine_km((40.7128, -74.0060), (34.0522, -118.2437));
        assert!((ny_la - 3936.0).abs() < 15.0, "{}", ny_la);
        assert_eq!(haversine_km((10.0, 10.0), (10.0, 10.0)), 0.0);
    }

    #[test]
    fn route_length_adds_the_legs() {
        let pts = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)];
        let want = haversine_km(pts[0], pts[1]) + haversine_km(pts[1], pts[2]);
        assert!((route_km(&pts) - want).abs() < 1e-9);
        assert_eq!(route_km(&[(1.0, 1.0)]), 0.0);
        assert_eq!(route_km(&[]), 0.0);
    }

    #[test]
    fn bearings_point_the_right_way() {
        assert!((bearing_deg((0.0, 0.0), (1.0, 0.0)) - 0.0).abs() < 1e-6); // north
        assert!((bearing_deg((0.0, 0.0), (0.0, 1.0)) - 90.0).abs() < 1e-6); // east
        assert!((bearing_deg((0.0, 0.0), (-1.0, 0.0)) - 180.0).abs() < 1e-6); // south
        assert!((bearing_deg((0.0, 0.0), (0.0, -1.0)) - 270.0).abs() < 1e-6); // west
    }

    #[test]
    fn no_go_polygon_test() {
        let square = [(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0)];
        assert!(in_polygon((0.5, 0.5), &square));
        assert!(!in_polygon((1.5, 0.5), &square));
        assert!(!in_polygon((0.5, -0.1), &square));
        assert!(!in_polygon((0.5, 0.5), &square[..2])); // not an area
    }

    #[test]
    fn positions_are_checked() {
        assert!(valid_point((39.0, -80.0)));
        assert!(!valid_point((91.0, 0.0)));
        assert!(!valid_point((0.0, 181.0)));
        assert!(!valid_point((f64::NAN, 0.0)));
    }

    #[test]
    fn gpx_is_well_formed_and_escaped() {
        let g = gpx("Home & <Camp>", &[(39.1, -80.2), (39.2, -80.3)]).unwrap();
        assert!(g.starts_with("<?xml"));
        assert!(g.contains("<name>Home &amp; &lt;Camp&gt;</name>"));
        assert!(g.contains("<rtept lat=\"39.100000\" lon=\"-80.200000\"/>"));
        assert_eq!(g.matches("<rtept").count(), 2);
        assert!(g.trim_end().ends_with("</gpx>"));
        assert!(gpx("x", &[(1.0, 1.0)]).is_err());
        assert!(gpx("x", &[(1.0, 1.0), (95.0, 1.0)]).is_err());
    }

    #[test]
    fn slugs_and_ids_cannot_escape_the_folder() {
        assert_eq!(slug("Home to Camp!"), "home-to-camp");
        assert_eq!(slug("  --  "), "map");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert!(valid_id("home-to-camp"));
        for bad in ["", "..", "../x", "a/b", "a\\b", "Home", "a b", "x.json"] {
            assert!(!valid_id(bad), "{:?}", bad);
        }
        let s = MapStore::new(&tmp("escape"));
        assert!(s.load("../x").is_err());
        assert!(s.delete("..").is_err());
        assert!(s.save(Some("../x"), "n", MAP, "").is_err());
    }

    #[test]
    fn save_list_load_round_trip() {
        let user = tmp("rt");
        let s = MapStore::new(&user);
        assert!(s.list().is_empty());
        let id = s.save(None, "Home to Camp", MAP, "Take the back road.").unwrap();
        assert_eq!(id, "home-to-camp");
        let l = s.list();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "Home to \"Camp\"");
        assert!(!l[0].has_picture && !l[0].has_route);
        let (json, notes) = s.load(&id).unwrap();
        assert_eq!(json, MAP);
        assert_eq!(notes, "Take the back road.");
        // The same name makes a second map, not an overwrite.
        let id2 = s.save(None, "Home to Camp", MAP, "").unwrap();
        assert_eq!(id2, "home-to-camp-2");
        assert_eq!(s.list().len(), 2);
    }

    #[test]
    fn replacing_a_map_keeps_a_backup() {
        let user = tmp("bak");
        let s = MapStore::new(&user);
        let id = s.save(None, "A", MAP, "one").unwrap();
        let newer = r#"{ "version": 1, "name": "A2" }"#;
        s.save(Some(&id), "A", newer, "two").unwrap();
        assert_eq!(s.load(&id).unwrap(), (newer.to_string(), "two".to_string()));
        let n = fs::read_dir(user.join("backups")).unwrap().count();
        assert!(n >= 2, "backups: {}", n); // map.json and notes.md
    }

    #[test]
    fn garbage_is_not_saved_as_a_map() {
        let s = MapStore::new(&tmp("garbage"));
        assert!(s.save(None, "x", "hello", "").is_err());
        assert!(s.save(None, "x", "{ \"name\": \"no version\" }", "").is_err());
        assert!(s.list().is_empty());
    }

    #[test]
    fn picture_and_route_need_a_saved_map_and_real_data() {
        let user = tmp("files");
        let s = MapStore::new(&user);
        let png = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0];
        assert!(s.save_picture("nope", &png).is_err());
        let id = s.save(None, "M", MAP, "").unwrap();
        assert!(s.save_picture(&id, b"GIF89a....").is_err());
        s.save_picture(&id, &png).unwrap();
        assert!(s.export_gpx(&id, "M", &[(1.0, 1.0)]).is_err());
        let p = s.export_gpx(&id, "M", &[(39.0, -80.0), (39.1, -80.1)]).unwrap();
        assert!(p.ends_with("route.gpx"));
        let m = &s.list()[0];
        assert!(m.has_picture && m.has_route);
    }

    #[test]
    fn delete_removes_only_that_map() {
        let s = MapStore::new(&tmp("del"));
        let a = s.save(None, "A", MAP, "").unwrap();
        let b = s.save(None, "B", MAP, "").unwrap();
        s.delete(&a).unwrap();
        assert_eq!(s.list().iter().map(|m| m.id.clone()).collect::<Vec<_>>(), vec![b]);
        assert!(s.delete(&a).is_err());
    }

    #[test]
    fn a_damaged_folder_is_skipped_in_the_list() {
        let user = tmp("skip");
        let s = MapStore::new(&user);
        s.save(None, "Good", MAP, "").unwrap();
        fs::create_dir_all(user.join("saved-maps/empty-folder")).unwrap();
        assert_eq!(s.list().len(), 1);
    }
}
