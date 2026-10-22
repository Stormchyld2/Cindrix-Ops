//! A phone map: a snapshot of the part of the map on screen, in one file (`.cmap`) that a phone can
//! carry. It holds the map tiles of that area (street, topo and aerial, as many as the person
//! chooses and at the detail they choose), the lettering the street map needs, and the person's own
//! pins, drawings, areas to avoid, notes and directions. Standard library only.
//!
//! The file is written in one pass: tiles first, then the tile lists, then a description of it all
//! (JSON), then the description's length and an end mark. A reader opens the last 12 bytes, finds
//! the description, and from it the place of every tile. The format is described in
//! `docs/32-phone-map.md`.

use crate::roadtiles::{tile_xy, PmFile};
use std::collections::HashSet;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

type R<T> = Result<T, String>;

pub const MAGIC: &[u8; 8] = b"CMAP0001";
pub const END_MAGIC: &[u8; 8] = b"CMAPEND1";
/// The most tile squares looked at for one zoom level; beyond this the level is "too big" to offer.
const MAX_CELLS: u64 = 2_000_000;

#[derive(Clone, Copy, Debug)]
pub struct Bbox {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl Bbox {
    pub fn valid(&self) -> bool {
        self.west < self.east && self.south < self.north && self.south >= -85.06 && self.north <= 85.06 && self.west >= -180.0 && self.east <= 180.0
    }
}

/// One kind of picture and the map files that hold it (the sharpest first).
#[derive(Clone, Debug)]
pub struct Source {
    /// "street", "topo" or "aerial"
    pub kind: String,
    pub files: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZoomInfo {
    pub zoom: u8,
    pub tiles: u64,
    pub bytes: u64,
    /// More tile squares than are looked at; not offered.
    pub too_big: bool,
}

#[derive(Clone, Debug)]
pub struct Estimate {
    pub kind: String,
    pub zooms: Vec<ZoomInfo>,
}

fn open_all(files: &[PathBuf]) -> R<Vec<PmFile>> {
    let mut out = Vec::new();
    let mut first_err = None;
    for f in files {
        match PmFile::open(f) {
            Ok(p) => out.push(p),
            Err(e) => first_err = first_err.or(Some(e)),
        }
    }
    if out.is_empty() {
        return Err(first_err.unwrap_or_else(|| "no map file".to_string()));
    }
    Ok(out)
}

fn tile_range(b: &Bbox, z: u8) -> (u32, u32, u32, u32) {
    let (x0, y0) = tile_xy(b.north, b.west, z);
    let (x1, y1) = tile_xy(b.south, b.east, z);
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}

fn overlaps(f: &PmFile, b: &Bbox) -> bool {
    let (w, s, e, n) = f.header.bounds;
    !(e < b.west || w > b.east || n < b.south || s > b.north)
}

/// For every zoom level the files have, how many tiles lie in the area and how many bytes they take.
pub fn estimate(src: &Source, b: &Bbox) -> R<Estimate> {
    if !b.valid() {
        return Err("The area is not valid.".into());
    }
    let mut files = open_all(&src.files)?;
    let lo = files.iter().map(|f| f.header.min_zoom).min().unwrap_or(0);
    let hi = files.iter().map(|f| f.header.max_zoom).max().unwrap_or(0);
    let mut zooms = Vec::new();
    for z in lo..=hi {
        let (x0, y0, x1, y1) = tile_range(b, z);
        let cells = (x1 - x0 + 1) as u64 * (y1 - y0 + 1) as u64;
        if cells > MAX_CELLS {
            zooms.push(ZoomInfo { zoom: z, tiles: 0, bytes: 0, too_big: true });
            continue;
        }
        let (mut tiles, mut bytes) = (0u64, 0u64);
        for x in x0..=x1 {
            for y in y0..=y1 {
                for f in files.iter_mut() {
                    if !overlaps(f, b) {
                        continue;
                    }
                    if let Some((_, len)) = f.entry(z, x, y)? {
                        tiles += 1;
                        bytes += len as u64;
                        break;
                    }
                }
            }
        }
        zooms.push(ZoomInfo { zoom: z, tiles, bytes, too_big: false });
    }
    Ok(Estimate { kind: src.kind.clone(), zooms })
}

/// A kind of picture to carry, and how sharp (the highest zoom level to include).
#[derive(Clone, Debug)]
pub struct Choice {
    pub source: Source,
    pub max_zoom: u8,
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub bytes: u64,
    pub layers: Vec<(String, u64)>,
}

fn jstr(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

struct Out {
    w: BufWriter<fs::File>,
    pos: u64,
}

impl Out {
    fn put(&mut self, d: &[u8]) -> R<()> {
        self.w.write_all(d).map_err(|e| e.to_string())?;
        self.pos += d.len() as u64;
        Ok(())
    }
}

fn type_name(t: u8) -> &'static str {
    match t {
        1 => "mvt",
        2 => "png",
        3 => "jpeg",
        4 => "webp",
        5 => "avif",
        _ => "unknown",
    }
}

fn comp_name(c: u8) -> &'static str {
    match c {
        2 => "gzip",
        4 => "zstd",
        _ => "none",
    }
}

/// The lettering files (`fonts/<font>/0-255.pbf` and `256-511.pbf`) as (font, range, bytes).
fn glyph_files(fonts: &Path) -> Vec<(String, String, Vec<u8>)> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(fonts) else { return out };
    let mut dirs: Vec<_> = rd.flatten().filter(|e| e.path().is_dir()).collect();
    dirs.sort_by_key(|e| e.file_name());
    for d in dirs {
        let font = d.file_name().to_string_lossy().to_string();
        for range in ["0-255", "256-511"] {
            let p = d.path().join(format!("{}.pbf", range));
            if let Ok(m) = fs::metadata(&p) {
                if m.len() > 0 && m.len() < 4 * 1024 * 1024 {
                    if let Ok(b) = fs::read(&p) {
                        out.push((font.clone(), range.to_string(), b));
                    }
                }
            }
        }
    }
    out
}

/// Write the phone map. `map_json` is the person's own map (pins, drawings, areas to avoid, notes,
/// directions) as one JSON object; it is carried as it is. `fonts` is the lettering folder, if any.
pub fn build(out: &Path, name: &str, b: &Bbox, view: ((f64, f64), f64), map_json: &str, choices: &[Choice], fonts: Option<&Path>) -> R<Summary> {
    if !b.valid() {
        return Err("The area is not valid.".into());
    }
    let mj = map_json.trim();
    if !(mj.starts_with('{') && mj.ends_with('}')) {
        return Err("The map's own data is not in the form expected.".into());
    }
    if choices.is_empty() {
        return Err("Choose at least one kind of map to carry.".into());
    }
    if let Some(dir) = out.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Could not make the folder {}: {}", dir.display(), e))?;
    }
    let tmp = out.with_extension("cmap.part");
    let file = fs::File::create(&tmp).map_err(|e| format!("Could not write {}: {}", tmp.display(), e))?;
    let mut o = Out { w: BufWriter::with_capacity(1 << 20, file), pos: 0 };
    let result = (|| -> R<Summary> {
        o.put(MAGIC)?;
        let mut layer_json: Vec<String> = Vec::new();
        let mut summary = Summary { bytes: 0, layers: Vec::new() };
        let mut have_street = false;
        for ch in choices {
            let mut files = open_all(&ch.source.files)?;
            let lo = files.iter().map(|f| f.header.min_zoom).min().unwrap_or(0);
            let hi = files.iter().map(|f| f.header.max_zoom).max().unwrap_or(0).min(ch.max_zoom);
            // all files of one kind must hold the same kind of tile
            let (ttype, tcomp) = (files[0].header.tile_type, files[0].header.tile_comp);
            let data_start = o.pos;
            let mut index: Vec<[u32; 5]> = Vec::new();
            let mut seen: HashSet<(u8, u32, u32)> = HashSet::new();
            for z in lo..=hi {
                let (x0, y0, x1, y1) = tile_range(b, z);
                if (x1 - x0 + 1) as u64 * (y1 - y0 + 1) as u64 > MAX_CELLS {
                    return Err(format!("The area is too large to cut at zoom {}. Pick a smaller part of the map or less detail.", z));
                }
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        for f in files.iter_mut() {
                            if !overlaps(f, b) || f.header.tile_type != ttype || f.header.tile_comp != tcomp {
                                continue;
                            }
                            if let Some(raw) = f.tile_raw(z, x, y)? {
                                if seen.insert((z, x, y)) {
                                    let off = o.pos - data_start;
                                    if off + raw.len() as u64 > u32::MAX as u64 {
                                        return Err("This map layer is bigger than 4 GB; choose less detail or a smaller area.".into());
                                    }
                                    index.push([z as u32, x, y, off as u32, raw.len() as u32]);
                                    o.put(&raw)?;
                                }
                                break;
                            }
                        }
                    }
                }
            }
            if index.is_empty() {
                return Err(format!("The {} map has nothing in this area.", ch.source.kind));
            }
            let data_len = o.pos - data_start;
            let index_offset = o.pos;
            for e in &index {
                for v in e {
                    o.put(&v.to_le_bytes())?;
                }
            }
            if ch.source.kind == "street" {
                have_street = true;
            }
            layer_json.push(format!(
                "{{\"kind\":{},\"tile_type\":{},\"compression\":{},\"min_zoom\":{},\"max_zoom\":{},\"tiles\":{},\"data_offset\":{},\"data_length\":{},\"index_offset\":{},\"index_count\":{}}}",
                jstr(&ch.source.kind),
                jstr(type_name(ttype)),
                jstr(comp_name(tcomp)),
                index.iter().map(|e| e[0]).min().unwrap_or(0),
                index.iter().map(|e| e[0]).max().unwrap_or(0),
                index.len(),
                data_start,
                data_len,
                index_offset,
                index.len()
            ));
            summary.layers.push((ch.source.kind.clone(), data_len + index.len() as u64 * 20));
        }
        let mut glyph_json: Vec<String> = Vec::new();
        if have_street {
            if let Some(f) = fonts {
                for (font, range, bytes) in glyph_files(f) {
                    let off = o.pos;
                    o.put(&bytes)?;
                    glyph_json.push(format!("{{\"font\":{},\"range\":{},\"offset\":{},\"length\":{}}}", jstr(&font), jstr(&range), off, bytes.len()));
                }
            }
        }
        let manifest = format!(
            "{{\"format\":1,\"name\":{},\"created\":{},\"bbox\":[{},{},{},{}],\"view\":{{\"center\":[{},{}],\"zoom\":{}}},\"layers\":[{}],\"glyphs\":[{}],\"map\":{}}}",
            jstr(name),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            b.west, b.south, b.east, b.north,
            (view.0).0, (view.0).1, view.1,
            layer_json.join(","),
            glyph_json.join(","),
            mj
        );
        o.put(manifest.as_bytes())?;
        o.put(&(manifest.len() as u32).to_le_bytes())?;
        o.put(END_MAGIC)?;
        summary.bytes = o.pos;
        Ok(summary)
    })();
    let flushed = o.w.flush().map_err(|e| e.to_string());
    drop(o);
    match (result, flushed) {
        (Ok(s), Ok(())) => {
            if out.exists() {
                let _ = fs::remove_file(out);
            }
            fs::rename(&tmp, out).map_err(|e| format!("Could not finish {}: {}", out.display(), e))?;
            Ok(s)
        }
        (Err(e), _) | (_, Err(e)) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// The description at the end of a `.cmap` file, as text (for checking a file and for tests).
pub fn read_manifest(path: &Path) -> R<String> {
    let len = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len < 28 {
        return Err("not a phone map".into());
    }
    let tail = crate::maps::read_range(path, len - 12, len - 1).map_err(|e| e.to_string())?;
    if &tail[4..] != END_MAGIC {
        return Err("not a phone map (or it was not finished)".into());
    }
    let n = u32::from_le_bytes(tail[..4].try_into().unwrap()) as u64;
    if n + 12 + 8 > len {
        return Err("damaged phone map".into());
    }
    let m = crate::maps::read_range(path, len - 12 - n, len - 13).map_err(|e| e.to_string())?;
    String::from_utf8(m).map_err(|_| "damaged phone map".to_string())
}

/// A place a phone map can be copied to: a drive or card that is plugged in.
#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub path: PathBuf,
    /// A short name for the list ("Drive E:", "Looks like a phone").
    pub label: String,
    pub looks_like_phone: bool,
    /// The drive the phone map is already on.
    pub here: bool,
}

/// The plugged-in drives and cards among `roots`, leaving out the one holding `file` (the drive it came from).
pub fn devices_in(roots: &[PathBuf], file: &Path) -> Vec<Device> {
    let mut out = Vec::new();
    for r in roots {
        if fs::read_dir(r).is_err() {
            continue;
        }
        let phone = r.join("Android").is_dir() || r.join("DCIM").is_dir();
        let name = r.to_string_lossy().trim_end_matches(['\\', '/']).to_string();
        let here = file.starts_with(r);
        let label = if here { format!("{} (the file is already on this drive)", name) } else if phone { format!("{} (looks like a phone)", name) } else { name };
        out.push(Device { path: r.clone(), label, looks_like_phone: phone && !here, here });
    }
    out.sort_by_key(|d| (d.here, !d.looks_like_phone));
    out
}

/// Copy the phone map into `dest` (a folder on the chosen device). On a phone drive the file goes in
/// its `Download` folder when there is one. The copy is finished under a temporary name, then renamed,
/// and its size is checked. Returns where it went.
pub fn send(file: &Path, dest: &Path) -> R<PathBuf> {
    let name = file.file_name().ok_or("That file has no name.")?;
    let mut dir = dest.to_path_buf();
    if dir.join("Android").is_dir() && dir.join("Download").is_dir() {
        dir = dir.join("Download");
    }
    if !dir.is_dir() {
        return Err("That place cannot be reached. Check that it is still plugged in.".into());
    }
    let target = dir.join(name);
    let tmp = dir.join(format!("{}.part", name.to_string_lossy()));
    let want = fs::metadata(file).map_err(|e| format!("Could not read the phone map: {}", e))?.len();
    let done = fs::copy(file, &tmp).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Could not copy to {}: {}. There may not be enough room.", dir.display(), e)
    })?;
    if done != want {
        let _ = fs::remove_file(&tmp);
        return Err("The copy did not finish. Please try again.".into());
    }
    if target.exists() {
        let _ = fs::remove_file(&target);
    }
    fs::rename(&tmp, &target).map_err(|e| format!("Could not finish the copy: {}", e))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roadtiles::testkit::{file, tile, L};
    use crate::roadtiles::tile_id;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-phone-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn road(name: &'static str) -> L {
        ("minor_road", "residential", name, false, vec![(0, 100), (500, 100)])
    }

    /// A small map file: tiles at zoom 12, 13 and 14 around one spot, plus a far-away tile.
    fn make(d: &Path) -> (PathBuf, Bbox) {
        let (lat, lon) = (39.485, -80.139);
        let mut tiles = Vec::new();
        for z in [12u8, 13, 14] {
            let (x, y) = tile_xy(lat, lon, z);
            for dx in 0..2u32 {
                tiles.push((z, x + dx, y, tile(&[road("Main")])));
            }
        }
        // a tile that is far outside the area
        tiles.push((14, 1000, 1000, tile(&[road("Far")])));
        let p = d.join("s.pmtiles");
        fs::write(&p, file(&tiles, (-180.0, -85.0, 180.0, 85.0))).unwrap();
        (p, Bbox { west: -80.145, south: 39.480, east: -80.125, north: 39.490 })
    }

    #[test]
    fn sizes_are_counted_for_the_area_only() {
        let d = dir("est");
        let (p, b) = make(&d);
        let e = estimate(&Source { kind: "street".into(), files: vec![p] }, &b).unwrap();
        assert_eq!(e.zooms.len(), 3);
        assert!(e.zooms.iter().all(|z| z.tiles >= 1 && z.bytes > 0 && !z.too_big), "{:?}", e.zooms);
        // the far-away tile at zoom 14 is not counted
        let z14 = e.zooms.iter().find(|z| z.zoom == 14).unwrap();
        assert!(z14.tiles <= 2, "{:?}", z14);
        // a huge area at a high zoom is refused rather than walked
        let world = Bbox { west: -179.0, south: -80.0, east: 179.0, north: 80.0 };
        let e = estimate(&Source { kind: "street".into(), files: vec![make(&d).0] }, &world).unwrap();
        assert!(e.zooms.iter().any(|z| z.zoom == 14 && z.too_big));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_phone_map_is_written_and_every_tile_can_be_found_again() {
        let d = dir("build");
        let (p, b) = make(&d);
        let fonts = d.join("fonts");
        fs::create_dir_all(fonts.join("Noto Sans Regular")).unwrap();
        fs::write(fonts.join("Noto Sans Regular").join("0-255.pbf"), b"glyphs-a").unwrap();
        let out = d.join("out").join("trail.cmap");
        let s = build(
            &out,
            "Trail \"north\"",
            &b,
            ((39.485, -80.139), 14.0),
            "{\"pins\":[{\"label\":\"Camp\",\"lat\":39.48,\"lon\":-80.13}],\"notes\":\"line one\\nline two\"}",
            &[Choice { source: Source { kind: "street".into(), files: vec![p.clone()] }, max_zoom: 13 }],
            Some(&fonts),
        )
        .unwrap();
        assert!(s.bytes > 100 && s.layers.len() == 1);
        let m = read_manifest(&out).unwrap();
        assert!(m.starts_with("{\"format\":1,"), "{}", m);
        assert!(m.contains("\"name\":\"Trail \\\"north\\\"\""), "{}", m);
        assert!(m.contains("\"max_zoom\":13"), "{}", m);
        assert!(!m.contains("\"max_zoom\":14"), "detail above the chosen level is left out");
        assert!(m.contains("\"pins\":[{\"label\":\"Camp\""), "{}", m);
        assert!(m.contains("\"font\":\"Noto Sans Regular\""), "{}", m);
        // read the tile list back and compare every tile with the source
        let num = |key: &str| -> u64 {
            let i = m.find(key).unwrap() + key.len();
            m[i..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap()
        };
        let (index_offset, count, data_offset) = (num("\"index_offset\":"), num("\"index_count\":"), num("\"data_offset\":"));
        assert!(count >= 2);
        let all = fs::read(&out).unwrap();
        assert_eq!(&all[..8], MAGIC);
        assert_eq!(&all[all.len() - 8..], END_MAGIC);
        let mut src = PmFile::open(&p).unwrap();
        for i in 0..count as usize {
            let at = index_offset as usize + i * 20;
            let v: Vec<u32> = (0..5).map(|k| u32::from_le_bytes(all[at + k * 4..at + k * 4 + 4].try_into().unwrap())).collect();
            let (z, x, y, off, len) = (v[0] as u8, v[1], v[2], v[3] as usize, v[4] as usize);
            assert!(z <= 13);
            let got = &all[data_offset as usize + off..data_offset as usize + off + len];
            assert_eq!(got, &src.tile_raw(z, x, y).unwrap().unwrap()[..], "tile {}/{}/{}", z, x, y);
            let _ = tile_id(z, x, y);
        }
        // no half-written file is left behind
        assert!(!out.with_extension("cmap.part").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn bad_requests_give_plain_errors_and_leave_no_file() {
        let d = dir("bad");
        let (p, b) = make(&d);
        let out = d.join("x.cmap");
        let src = Source { kind: "street".into(), files: vec![p] };
        assert!(build(&out, "x", &b, ((0.0, 0.0), 1.0), "{}", &[], None).unwrap_err().contains("at least one"));
        assert!(build(&out, "x", &b, ((0.0, 0.0), 1.0), "not json", &[Choice { source: src.clone(), max_zoom: 14 }], None).is_err());
        let bad = Bbox { west: 1.0, south: 1.0, east: 0.0, north: 2.0 };
        assert!(build(&out, "x", &bad, ((0.0, 0.0), 1.0), "{}", &[Choice { source: src.clone(), max_zoom: 14 }], None).is_err());
        // an area with no tiles in it
        let empty = Bbox { west: 10.0, south: 10.0, east: 10.01, north: 10.01 };
        assert!(build(&out, "x", &empty, ((0.0, 0.0), 1.0), "{}", &[Choice { source: src, max_zoom: 14 }], None).unwrap_err().contains("nothing in this area"));
        assert!(!out.exists() && !out.with_extension("cmap.part").exists());
        assert!(read_manifest(&d.join("s.pmtiles")).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn sending_lists_other_drives_and_copies_safely() {
        let d = dir("send");
        let home = d.join("drive");
        let phone = d.join("phone");
        let card = d.join("card");
        for p in [&home, &phone, &card] {
            fs::create_dir_all(p).unwrap();
        }
        fs::create_dir_all(phone.join("Android")).unwrap();
        fs::create_dir_all(phone.join("Download")).unwrap();
        let f = home.join("trail.cmap");
        fs::write(&f, b"0123456789").unwrap();
        let list = devices_in(&[home.clone(), card.clone(), phone.clone(), d.join("gone")], &f);
        assert_eq!(list.len(), 3, "missing places are left out");
        assert!(list[0].looks_like_phone && list[0].path == phone, "phones come first");
        assert!(list[2].here && list[2].path == home, "the drive the file is on comes last and is marked");
        let got = send(&f, &phone).unwrap();
        assert_eq!(got, phone.join("Download").join("trail.cmap"));
        assert_eq!(fs::read(&got).unwrap(), b"0123456789");
        let got2 = send(&f, &card).unwrap();
        assert_eq!(got2, card.join("trail.cmap"));
        assert!(send(&f, &d.join("gone")).unwrap_err().contains("cannot be reached"));
        assert!(!card.join("trail.cmap.part").exists());
        let _ = fs::remove_dir_all(&d);
    }
}
