//! Reading roads out of a street map file (`.pmtiles`, Protomaps layout), standard library only.
//! Used for directions: the roads are already on the drive inside the map, so no other download
//! is needed. A map tile is a small gzip-compressed vector picture; its "roads" layer holds the
//! lines with a kind ("highway", "major_road", "minor_road", "path" ...) and a finer
//! kind_detail ("motorway", "residential", "footway" ...).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

type R<T> = Result<T, String>;

const MAX_DIR: u64 = 16 * 1024 * 1024;
const MAX_TILE: usize = 8 * 1024 * 1024;

/// The tile (x, y) that holds a position at a zoom level.
pub fn tile_xy(lat: f64, lon: f64, z: u8) -> (u32, u32) {
    let n = (1u64 << z) as f64;
    let x = ((lon + 180.0) / 360.0 * n).floor();
    let lr = lat.clamp(-85.05, 85.05).to_radians();
    let y = ((1.0 - (lr.tan() + 1.0 / lr.cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor();
    (x.clamp(0.0, n - 1.0) as u32, y.clamp(0.0, n - 1.0) as u32)
}

// ---------- the file ----------

#[derive(Clone, Debug)]
pub struct Header {
    pub root_off: u64,
    pub root_len: u64,
    pub leaf_off: u64,
    pub tile_off: u64,
    pub internal_comp: u8,
    pub tile_comp: u8,
    /// 1 vector (mvt), 2 png, 3 jpeg, 4 webp, 5 avif
    pub tile_type: u8,
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// west, south, east, north in degrees
    pub bounds: (f64, f64, f64, f64),
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    tile_id: u64,
    offset: u64,
    length: u32,
    run: u32,
}

pub fn parse_header(h: &[u8]) -> R<Header> {
    if h.len() < 127 || &h[..7] != b"PMTiles" || h[7] != 3 {
        return Err("not a PMTiles version 3 file".into());
    }
    let u = |i: usize| u64::from_le_bytes(h[i..i + 8].try_into().unwrap());
    let s = |i: usize| i32::from_le_bytes(h[i..i + 4].try_into().unwrap()) as f64 / 1e7;
    Ok(Header {
        root_off: u(8),
        root_len: u(16),
        leaf_off: u(40),
        tile_off: u(56),
        internal_comp: h[97],
        tile_comp: h[98],
        tile_type: h[99],
        min_zoom: h[100],
        max_zoom: h[101],
        bounds: (s(102), s(106), s(110), s(114)),
    })
}

/// The number the file uses for a tile (position along a Hilbert curve, zoom levels in order).
pub fn tile_id(z: u8, x: u32, y: u32) -> u64 {
    if z == 0 {
        return 0;
    }
    let mut acc: u64 = 0;
    for i in 0..z {
        acc += 1u64 << (2 * i as u32);
    }
    let n: u64 = 1u64 << z;
    let (mut x, mut y) = (x as u64 & (n - 1), y as u64 & (n - 1));
    let mut d: u64 = 0;
    let mut s = n / 2;
    while s > 0 {
        let rx = u64::from(x & s > 0);
        let ry = u64::from(y & s > 0);
        d += s * s * ((3 * rx) ^ ry);
        if ry == 0 {
            if rx == 1 {
                x = n - 1 - x;
                y = n - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s /= 2;
    }
    acc + d
}

fn varint(d: &[u8], p: &mut usize) -> R<u64> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let Some(&b) = d.get(*p) else { return Err("short data".into()) };
        *p += 1;
        if shift >= 64 {
            return Err("number too long".into());
        }
        v |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
    }
}

fn parse_dir(d: &[u8]) -> R<Vec<Entry>> {
    let mut p = 0usize;
    let n = varint(d, &mut p)? as usize;
    if n > d.len() {
        return Err("directory too big".into());
    }
    let mut es = vec![Entry { tile_id: 0, offset: 0, length: 0, run: 0 }; n];
    let mut last = 0u64;
    for e in es.iter_mut() {
        last += varint(d, &mut p)?;
        e.tile_id = last;
    }
    for e in es.iter_mut() {
        e.run = varint(d, &mut p)? as u32;
    }
    for e in es.iter_mut() {
        e.length = varint(d, &mut p)? as u32;
    }
    for i in 0..n {
        let o = varint(d, &mut p)?;
        es[i].offset = if o == 0 && i > 0 { es[i - 1].offset + es[i - 1].length as u64 } else { o.saturating_sub(1) };
    }
    Ok(es)
}

fn unpack(comp: u8, data: Vec<u8>, max: usize) -> R<Vec<u8>> {
    match comp {
        0 | 1 => Ok(data),
        2 => crate::inflate::gunzip(&data, max),
        4 => crate::zstd::decompress(&data, max),
        _ => Err("this map file uses a compression the program cannot read".into()),
    }
}

pub struct PmFile {
    path: PathBuf,
    pub header: Header,
    dirs: HashMap<u64, Rc<Vec<Entry>>>,
}

impl PmFile {
    pub fn open(path: &Path) -> R<PmFile> {
        let head = crate::maps::read_range(path, 0, 126).map_err(|e| format!("{}: {}", path.display(), e))?;
        Ok(PmFile { path: path.to_path_buf(), header: parse_header(&head)?, dirs: HashMap::new() })
    }

    fn dir(&mut self, off: u64, len: u64) -> R<Rc<Vec<Entry>>> {
        if let Some(d) = self.dirs.get(&off) {
            return Ok(d.clone());
        }
        if len == 0 || len > MAX_DIR {
            return Err("bad directory size".into());
        }
        let raw = crate::maps::read_range(&self.path, off, off + len - 1).map_err(|e| e.to_string())?;
        let es = Rc::new(parse_dir(&unpack(self.header.internal_comp, raw, 64 * 1024 * 1024)?)?);
        if self.dirs.len() > 4000 {
            self.dirs.clear();
        }
        self.dirs.insert(off, es.clone());
        Ok(es)
    }

    /// Where a tile's bytes are in the file: (offset in the tile data, length). None when the file has no such tile.
    pub fn entry(&mut self, z: u8, x: u32, y: u32) -> R<Option<(u64, u32)>> {
        if z < self.header.min_zoom || z > self.header.max_zoom {
            return Ok(None);
        }
        let id = tile_id(z, x, y);
        let (mut off, mut len) = (self.header.root_off, self.header.root_len);
        for _ in 0..4 {
            let es = self.dir(off, len)?;
            let i = es.partition_point(|e| e.tile_id <= id);
            if i == 0 {
                return Ok(None);
            }
            let e = es[i - 1];
            if e.run == 0 {
                off = self.header.leaf_off + e.offset;
                len = e.length as u64;
                continue;
            }
            if id >= e.tile_id + e.run as u64 || e.length == 0 {
                return Ok(None);
            }
            return Ok(Some((e.offset, e.length)));
        }
        Err("map file directories are nested too deep".into())
    }

    /// The tile exactly as stored in the file (still compressed if the file compresses its tiles).
    pub fn tile_raw(&mut self, z: u8, x: u32, y: u32) -> R<Option<Vec<u8>>> {
        let Some((off, len)) = self.entry(z, x, y)? else { return Ok(None) };
        crate::maps::read_range(&self.path, self.header.tile_off + off, self.header.tile_off + off + len as u64 - 1).map(Some).map_err(|e| e.to_string())
    }

    /// The tile's bytes, uncompressed; None when the file has no such tile.
    pub fn tile(&mut self, z: u8, x: u32, y: u32) -> R<Option<Vec<u8>>> {
        match self.tile_raw(z, x, y)? {
            Some(raw) => unpack(self.header.tile_comp, raw, MAX_TILE).map(Some),
            None => Ok(None),
        }
    }
}

// ---------- the roads layer of a vector tile ----------

/// One road line from a tile. Points are in tile units (0..extent, y down).
#[derive(Clone, Debug, Default)]
pub struct RoadLine {
    pub kind: String,
    pub detail: String,
    pub name: String,
    /// 0 on the ground, 1 on a bridge, -1 in a tunnel
    pub layer: i8,
    pub pts: Vec<(i32, i32)>,
}

pub struct RoadTile {
    pub extent: u32,
    pub lines: Vec<RoadLine>,
}

struct Pb<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Pb<'a> {
    fn more(&self) -> bool {
        self.p < self.d.len()
    }
    fn var(&mut self) -> R<u64> {
        varint(self.d, &mut self.p)
    }
    /// (field number, wire type)
    fn key(&mut self) -> R<(u32, u8)> {
        let k = self.var()?;
        Ok(((k >> 3) as u32, (k & 7) as u8))
    }
    fn bytes(&mut self) -> R<&'a [u8]> {
        let n = self.var()? as usize;
        if self.p + n > self.d.len() {
            return Err("short data".into());
        }
        let s = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn skip(&mut self, wt: u8) -> R<()> {
        match wt {
            0 => {
                self.var()?;
            }
            1 => self.p += 8,
            2 => {
                self.bytes()?;
            }
            5 => self.p += 4,
            _ => return Err("unknown field".into()),
        }
        if self.p > self.d.len() {
            return Err("short data".into());
        }
        Ok(())
    }
}

fn zig(v: u64) -> i32 {
    ((v >> 1) as i64 ^ -((v & 1) as i64)) as i32
}

enum Val {
    S(String),
    I(i64),
    B(bool),
}

fn parse_value(d: &[u8]) -> R<Val> {
    let mut pb = Pb { d, p: 0 };
    let mut out = Val::S(String::new());
    while pb.more() {
        let (f, wt) = pb.key()?;
        match (f, wt) {
            (1, 2) => out = Val::S(String::from_utf8_lossy(pb.bytes()?).into_owned()),
            (4, 0) | (5, 0) => out = Val::I(pb.var()? as i64),
            (6, 0) => out = Val::I(zig(pb.var()?) as i64),
            (7, 0) => out = Val::B(pb.var()? != 0),
            (_, w) => pb.skip(w)?,
        }
    }
    Ok(out)
}

fn parse_feature(d: &[u8], keys: &[String], vals: &[Val]) -> R<Vec<RoadLine>> {
    let mut pb = Pb { d, p: 0 };
    let mut tags: Vec<u32> = Vec::new();
    let mut gtype = 0u64;
    let mut geom: &[u8] = &[];
    while pb.more() {
        let (f, wt) = pb.key()?;
        match (f, wt) {
            (2, 2) => {
                let b = pb.bytes()?;
                let mut q = Pb { d: b, p: 0 };
                while q.more() {
                    tags.push(q.var()? as u32);
                }
            }
            (3, 0) => gtype = pb.var()?,
            (4, 2) => geom = pb.bytes()?,
            (_, w) => pb.skip(w)?,
        }
    }
    if gtype != 2 {
        return Ok(Vec::new());
    }
    let mut line = RoadLine::default();
    for kv in tags.chunks(2) {
        if kv.len() < 2 {
            break;
        }
        let (Some(k), Some(v)) = (keys.get(kv[0] as usize), vals.get(kv[1] as usize)) else { continue };
        match (k.as_str(), v) {
            ("kind", Val::S(s)) => line.kind = s.clone(),
            ("kind_detail", Val::S(s)) => line.detail = s.clone(),
            ("name", Val::S(s)) => line.name = s.clone(),
            ("is_bridge", Val::B(true)) => line.layer = 1,
            ("is_tunnel", Val::B(true)) => line.layer = -1,
            ("level", Val::I(n)) if *n != 0 && line.layer == 0 => line.layer = if *n > 0 { 1 } else { -1 },
            _ => {}
        }
    }
    // the geometry: commands MoveTo(1) / LineTo(2) with zig-zag deltas; each MoveTo starts a new line
    let mut g = Pb { d: geom, p: 0 };
    let (mut x, mut y) = (0i32, 0i32);
    let mut cur: Vec<(i32, i32)> = Vec::new();
    let mut parts: Vec<Vec<(i32, i32)>> = Vec::new();
    while g.more() {
        let c = g.var()?;
        let (cmd, count) = (c & 7, (c >> 3) as usize);
        match cmd {
            1 | 2 => {
                for _ in 0..count {
                    x = x.wrapping_add(zig(g.var()?));
                    y = y.wrapping_add(zig(g.var()?));
                    if cmd == 1 {
                        if cur.len() >= 2 {
                            parts.push(std::mem::take(&mut cur));
                        } else {
                            cur.clear();
                        }
                    }
                    cur.push((x, y));
                }
            }
            7 => {}
            _ => return Err("bad geometry".into()),
        }
    }
    if cur.len() >= 2 {
        parts.push(cur);
    }
    // a feature with several parts becomes one RoadLine per part
    Ok(parts.into_iter().map(|p| RoadLine { pts: p, ..line.clone() }).collect())
}

/// The road lines of one (uncompressed) vector tile. Tiles without a roads layer give no lines.
pub fn parse_roads(tile: &[u8]) -> R<RoadTile> {
    let mut out = RoadTile { extent: 4096, lines: Vec::new() };
    let mut pb = Pb { d: tile, p: 0 };
    while pb.more() {
        let (f, wt) = pb.key()?;
        if !(f == 3 && wt == 2) {
            pb.skip(wt)?;
            continue;
        }
        let layer = pb.bytes()?;
        let mut lb = Pb { d: layer, p: 0 };
        let mut name = String::new();
        let mut keys: Vec<String> = Vec::new();
        let mut vals: Vec<Val> = Vec::new();
        let mut feats: Vec<&[u8]> = Vec::new();
        let mut extent = 4096u32;
        while lb.more() {
            let (lf, lwt) = lb.key()?;
            match (lf, lwt) {
                (1, 2) => name = String::from_utf8_lossy(lb.bytes()?).into_owned(),
                (2, 2) => feats.push(lb.bytes()?),
                (3, 2) => keys.push(String::from_utf8_lossy(lb.bytes()?).into_owned()),
                (4, 2) => vals.push(parse_value(lb.bytes()?)?),
                (5, 0) => extent = lb.var()? as u32,
                (_, w) => lb.skip(w)?,
            }
        }
        if name != "roads" {
            continue;
        }
        out.extent = extent.max(1);
        for f in feats {
            out.lines.extend(parse_feature(f, &keys, &vals)?);
        }
    }
    Ok(out)
}

// ---------- for tests (and anyone who needs to make a small map file) ----------

/// Encoding helpers that make a tiny uncompressed PMTiles file with one roads layer per tile.
/// Used by the tests of this file and of `routing`.
#[cfg(test)]
pub mod testkit {
    fn put_var(o: &mut Vec<u8>, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                o.push(b);
                return;
            }
            o.push(b | 0x80);
        }
    }
    fn zz(v: i32) -> u64 {
        ((v << 1) ^ (v >> 31)) as u32 as u64
    }
    fn field(o: &mut Vec<u8>, f: u32, bytes: &[u8]) {
        put_var(o, ((f as u64) << 3) | 2);
        put_var(o, bytes.len() as u64);
        o.extend_from_slice(bytes);
    }

    /// (kind, kind_detail, name, bridge, points in tile units)
    pub type L = (&'static str, &'static str, &'static str, bool, Vec<(i32, i32)>);

    pub fn tile(lines: &[L]) -> Vec<u8> {
        let mut keys: Vec<String> = vec!["kind".into(), "kind_detail".into(), "name".into(), "is_bridge".into()];
        let mut vals: Vec<Vec<u8>> = Vec::new();
        let sval = |s: &str, vals: &mut Vec<Vec<u8>>| -> u64 {
            let mut v = Vec::new();
            field(&mut v, 1, s.as_bytes());
            vals.push(v);
            (vals.len() - 1) as u64
        };
        let mut feats: Vec<Vec<u8>> = Vec::new();
        for (k, d, n, br, pts) in lines {
            let mut tags: Vec<u64> = Vec::new();
            tags.extend([0, sval(k, &mut vals), 1, sval(d, &mut vals)]);
            if !n.is_empty() {
                tags.extend([2, sval(n, &mut vals)]);
            }
            if *br {
                vals.push(vec![(7 << 3) | 0, 1]);
                tags.extend([3, (vals.len() - 1) as u64]);
            }
            let mut f = Vec::new();
            let mut t = Vec::new();
            for x in tags {
                put_var(&mut t, x);
            }
            field(&mut f, 2, &t);
            put_var(&mut f, (3 << 3) | 0);
            put_var(&mut f, 2);
            let mut g = Vec::new();
            put_var(&mut g, (1 << 3) | 1);
            put_var(&mut g, zz(pts[0].0));
            put_var(&mut g, zz(pts[0].1));
            put_var(&mut g, (((pts.len() - 1) as u64) << 3) | 2);
            for w in pts.windows(2) {
                put_var(&mut g, zz(w[1].0 - w[0].0));
                put_var(&mut g, zz(w[1].1 - w[0].1));
            }
            field(&mut f, 4, &g);
            feats.push(f);
        }
        let mut layer = Vec::new();
        field(&mut layer, 1, b"roads");
        for f in &feats {
            field(&mut layer, 2, f);
        }
        for k in keys.drain(..) {
            field(&mut layer, 3, k.as_bytes());
        }
        for v in &vals {
            field(&mut layer, 4, v);
        }
        put_var(&mut layer, 5 << 3);
        put_var(&mut layer, 4096);
        let mut out = Vec::new();
        field(&mut out, 3, &layer);
        out
    }

    /// An uncompressed PMTiles v3 file holding these tiles `(z, x, y, tile bytes)`, all at one zoom.
    pub fn file(tiles: &[(u8, u32, u32, Vec<u8>)], bounds: (f64, f64, f64, f64)) -> Vec<u8> {
        let mut es: Vec<(u64, usize)> = tiles.iter().enumerate().map(|(i, t)| (super::tile_id(t.0, t.1, t.2), i)).collect();
        es.sort();
        let mut data = Vec::new();
        let mut offs = Vec::new();
        for (_, i) in &es {
            offs.push((data.len() as u64, tiles[*i].3.len() as u64));
            data.extend_from_slice(&tiles[*i].3);
        }
        let mut dir = Vec::new();
        put_var(&mut dir, es.len() as u64);
        let mut last = 0u64;
        for (id, _) in &es {
            put_var(&mut dir, id - last);
            last = *id;
        }
        for _ in &es {
            put_var(&mut dir, 1);
        }
        for (_, l) in &offs {
            put_var(&mut dir, *l);
        }
        for (o, _) in &offs {
            put_var(&mut dir, o + 1);
        }
        let mut h = vec![0u8; 127];
        h[..7].copy_from_slice(b"PMTiles");
        h[7] = 3;
        let root_off = 127u64;
        let tile_off = root_off + dir.len() as u64;
        h[8..16].copy_from_slice(&root_off.to_le_bytes());
        h[16..24].copy_from_slice(&(dir.len() as u64).to_le_bytes());
        h[40..48].copy_from_slice(&tile_off.to_le_bytes());
        h[56..64].copy_from_slice(&tile_off.to_le_bytes());
        h[64..72].copy_from_slice(&(data.len() as u64).to_le_bytes());
        h[97] = 1;
        h[98] = 1;
        h[99] = 1;
        h[100] = tiles.iter().map(|t| t.0).min().unwrap_or(0);
        h[101] = tiles.iter().map(|t| t.0).max().unwrap_or(0);
        let e7 = |v: f64| ((v * 1e7) as i32).to_le_bytes();
        h[102..106].copy_from_slice(&e7(bounds.0));
        h[106..110].copy_from_slice(&e7(bounds.1));
        h[110..114].copy_from_slice(&e7(bounds.2));
        h[114..118].copy_from_slice(&e7(bounds.3));
        let mut out = h;
        out.extend_from_slice(&dir);
        out.extend_from_slice(&data);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;

    #[test]
    fn tile_numbers_follow_the_curve() {
        assert_eq!(tile_id(0, 0, 0), 0);
        assert_eq!(tile_id(1, 0, 0), 1);
        assert_eq!(tile_id(1, 0, 1), 2);
        assert_eq!(tile_id(1, 1, 1), 3);
        assert_eq!(tile_id(1, 1, 0), 4);
        assert_eq!(tile_id(2, 0, 0), 5);
    }

    #[test]
    fn roads_survive_a_round_trip_through_a_tiny_map_file() {
        let t1 = tile(&[
            ("major_road", "primary", "Main St", false, vec![(0, 2048), (4096, 2048)]),
            ("minor_road", "residential", "", true, vec![(100, 100), (200, 300), (500, 300)]),
        ]);
        let dir = std::env::temp_dir().join(format!("cindrix-roadtiles-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.pmtiles");
        std::fs::write(&p, file(&[(14, 3, 5, t1), (14, 4, 5, tile(&[("path", "footway", "", false, vec![(0, 0), (10, 10)])]))], (-80.0, 39.0, -79.9, 39.1))).unwrap();
        let mut f = PmFile::open(&p).unwrap();
        assert_eq!(f.header.max_zoom, 14);
        let raw = f.tile(14, 3, 5).unwrap().unwrap();
        let rt = parse_roads(&raw).unwrap();
        assert_eq!(rt.extent, 4096);
        assert_eq!(rt.lines.len(), 2);
        assert_eq!(rt.lines[0].kind, "major_road");
        assert_eq!(rt.lines[0].detail, "primary");
        assert_eq!(rt.lines[0].name, "Main St");
        assert_eq!(rt.lines[0].pts, vec![(0, 2048), (4096, 2048)]);
        assert_eq!(rt.lines[1].layer, 1);
        assert_eq!(rt.lines[1].pts, vec![(100, 100), (200, 300), (500, 300)]);
        assert_eq!(parse_roads(&f.tile(14, 4, 5).unwrap().unwrap()).unwrap().lines[0].detail, "footway");
        // a tile the file does not have, and a zoom it does not have
        assert!(f.tile(14, 9, 9).unwrap().is_none());
        assert!(f.tile(10, 0, 0).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_is_not_a_map_is_refused_and_junk_tiles_are_errors() {
        assert!(parse_header(b"hello").is_err());
        assert!(parse_roads(&[0x1a, 0xff, 0xff, 0xff]).is_err());
        assert_eq!(parse_roads(&[]).unwrap().lines.len(), 0);
    }
}
