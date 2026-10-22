//! Directions between two places, by car and on foot, worked out from the roads inside the street
//! map file on the drive (no internet, no extra download). Standard library only.
//!
//! How it works: the map tiles along the way are read (see `roadtiles`), their road lines are cut
//! into pieces and joined wherever they cross or meet (map tiles drop the corner points of a
//! junction, so crossings are found by geometry), the two places are attached to the nearest
//! road, and the quickest way is found with A*. Long trips read a coarser, bigger-roads-only
//! picture of the map so they stay quick. The map file has no one-way or turn information, so
//! the answer is a good estimate of the way and the time, not a promise.

use crate::roadtiles::{parse_roads, PmFile, RoadTile};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;

type R<T> = Result<T, String>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Mode {
    Car,
    Foot,
}

#[derive(Clone, Debug)]
pub struct Step {
    /// The road, or a plain word such as "a path" when the map gives no name.
    pub name: String,
    pub km: f64,
    pub minutes: f64,
    /// Index in the route's points where this stretch begins.
    pub at: usize,
}

#[derive(Clone, Debug)]
pub struct Route {
    /// (latitude, longitude) along the way, from the start to the end.
    pub points: Vec<(f64, f64)>,
    pub km: f64,
    pub minutes: f64,
    pub steps: Vec<Step>,
    /// Plain words about how rough the answer is, empty when there is nothing to say.
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Directions {
    pub car: Result<Route, String>,
    pub foot: Result<Route, String>,
    /// The map zoom the roads were read at (14 is every street; lower is bigger roads only).
    pub zoom: u8,
}

// ---------- what each kind of road allows ----------

/// Speeds in km/h; 0 means that way of travelling is not allowed on this road.
#[derive(Clone, Copy, Debug)]
struct Cls {
    car: f32,
    foot: f32,
}

fn classify(kind: &str, detail: &str) -> Option<Cls> {
    let k = kind.to_lowercase();
    let d = detail.to_lowercase();
    if matches!(k.as_str(), "rail" | "ferry" | "aeroway" | "runway" | "taxiway" | "" | "other") && d.is_empty() && k != "other" {
        return None;
    }
    if matches!(k.as_str(), "rail" | "ferry" | "aeroway") {
        return None;
    }
    const FOOT: f32 = 5.0;
    // roads only people use
    if matches!(d.as_str(), "footway" | "pedestrian" | "path" | "cycleway" | "bridleway" | "corridor" | "sidewalk" | "crossing" | "pier" | "platform" | "steps") || k == "path" {
        let s = if d == "steps" { 3.0 } else { FOOT };
        return Some(Cls { car: 0.0, foot: s });
    }
    let car = match d.as_str() {
        "motorway" => 100.0,
        "motorway_link" => 50.0,
        "trunk" => 80.0,
        "trunk_link" => 45.0,
        "primary" => 65.0,
        "primary_link" => 40.0,
        "secondary" => 55.0,
        "secondary_link" => 35.0,
        "tertiary" => 48.0,
        "tertiary_link" => 30.0,
        "unclassified" | "road" => 45.0,
        "residential" => 35.0,
        "living_street" => 15.0,
        "service" | "driveway" | "parking_aisle" => 20.0,
        "track" => 15.0,
        _ => match k.as_str() {
            "highway" => 90.0,
            "major_road" => 60.0,
            "medium_road" => 48.0,
            "minor_road" => 35.0,
            _ => 20.0,
        },
    };
    let foot = if d.starts_with("motorway") || (k == "highway" && d.is_empty()) { 0.0 } else { FOOT };
    Some(Cls { car, foot })
}

/// Short stretches (a link or a join between two roads) are folded into the stretch before them, and
/// neighbours with the same name are joined, so the list reads as a handful of real turns.
fn tidy_steps(steps: Vec<Step>) -> Vec<Step> {
    let mut out: Vec<Step> = Vec::new();
    for s in steps {
        match out.last_mut() {
            Some(prev) if prev.name == s.name || (s.km < 0.4 && !s.name.starts_with("straight")) => {
                prev.km += s.km;
                prev.minutes += s.minutes;
            }
            _ => out.push(s),
        }
    }
    // a short stretch left at the very start is folded forward too
    let mut j = 0;
    while j + 1 < out.len() {
        if out[j].km < 0.4 && !out[j].name.starts_with("straight") && out[j + 1].name == out[j].name {
            j += 1;
            continue;
        }
        if out[j + 1].name == out[j].name {
            let n = out.remove(j + 1);
            out[j].km += n.km;
            out[j].minutes += n.minutes;
        } else {
            j += 1;
        }
    }
    out
}

/// The name (and number) of the named road nearest a point, read from the map at zoom `z`.
fn name_near(src: &mut Source, lat: f64, lon: f64, z: u8) -> Option<String> {
    let (tx, ty) = tile_of(lat, lon, z);
    let t = src.tile(z, tx, ty).ok()??;
    let n = (1u64 << z) as f64;
    let extent = t.extent as f64;
    let px = ((lon + 180.0) / 360.0 * n - tx as f64) * extent;
    let lr = lat.to_radians();
    let py = ((1.0 - (lr.tan() + 1.0 / lr.cos()).ln() / std::f64::consts::PI) / 2.0 * n - ty as f64) * extent;
    let mpu = 40_075_016.686 * lr.cos() / (n * extent);
    let mut best: Option<(f64, &crate::roadtiles::RoadLine)> = None;
    for l in &t.lines {
        if (l.name.is_empty() && l.refname.is_empty()) || classify(&l.kind, &l.detail).is_none() {
            continue;
        }
        for w in l.pts.windows(2) {
            let (ax, ay, bx, by) = (w[0].0 as f64, w[0].1 as f64, w[1].0 as f64, w[1].1 as f64);
            let (dx, dy) = (bx - ax, by - ay);
            let len2 = dx * dx + dy * dy;
            let f = if len2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let d = ((ax + f * dx - px).powi(2) + (ay + f * dy - py).powi(2)).sqrt() * mpu;
            if d < 100.0 && best.map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, l));
            }
        }
    }
    best.map(|(_, l)| match (l.name.is_empty(), l.refname.is_empty()) {
        (false, false) if l.name != l.refname => format!("{} ({})", l.name, l.refname),
        (false, _) => l.name.clone(),
        _ => l.refname.clone(),
    })
}

/// A trip read at a coarse zoom has few names. For stretches still called "the highway" or "a road",
/// look the road up in the sharper map along the way and use what most of it is called.
fn refine_names(src: &mut Source, d: &mut Directions, hi: u8) {
    let zr = hi.min(12);
    if d.zoom >= zr {
        return;
    }
    for way in [&mut d.car, &mut d.foot] {
        let Ok(r) = way else { continue };
        let ends: Vec<usize> = r.steps.iter().skip(1).map(|s| s.at).chain(std::iter::once(r.points.len().saturating_sub(1))).collect();
        for (i, end) in ends.into_iter().enumerate() {
            let generic = matches!(r.steps[i].name.as_str(), "the highway" | "a road");
            if !generic {
                continue;
            }
            let start = r.steps[i].at.min(r.points.len().saturating_sub(1));
            let end = end.max(start).min(r.points.len().saturating_sub(1));
            let mut tally: HashMap<String, usize> = HashMap::new();
            let mut since = f64::MAX;
            for k in start..=end {
                if k > start {
                    since += crate::maps::haversine_km(r.points[k - 1], r.points[k]);
                }
                if since >= 1.5 {
                    since = 0.0;
                    if let Some(nm) = name_near(src, r.points[k].0, r.points[k].1, zr) {
                        *tally.entry(nm).or_insert(0) += 1;
                    }
                }
            }
            if let Some((nm, _)) = tally.into_iter().max_by_key(|(n, c)| (*c, n.len())) {
                r.steps[i].name = nm;
            }
        }
        r.steps = tidy_steps(std::mem::take(&mut r.steps));
    }
}

fn plain_name(kind: &str, detail: &str) -> &'static str {
    let d = detail.to_lowercase();
    match d.as_str() {
        "footway" | "path" | "pedestrian" | "cycleway" | "bridleway" | "sidewalk" | "crossing" | "corridor" => "a path",
        "steps" => "steps",
        "service" | "driveway" | "parking_aisle" => "a service road",
        "track" => "a track",
        _ => match kind {
            "highway" => "the highway",
            _ => "a road",
        },
    }
}

// ---------- geometry on a small flat patch ----------

#[derive(Clone, Copy)]
struct Proj {
    lon0: f64,
    lat0: f64,
    kx: f64,
    ky: f64,
}

impl Proj {
    fn new(lat: f64, lon: f64) -> Proj {
        Proj { lon0: lon, lat0: lat, kx: 111_320.0 * lat.to_radians().cos().max(0.05), ky: 110_540.0 }
    }
    fn xy(&self, lat: f64, lon: f64) -> (f64, f64) {
        ((lon - self.lon0) * self.kx, (lat - self.lat0) * self.ky)
    }
    fn back(&self, x: f64, y: f64) -> (f64, f64) {
        (self.lat0 + y / self.ky, self.lon0 + x / self.kx)
    }
}

fn haversine_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    crate::maps::haversine_km(a, b) * 1000.0
}

fn tile_point(z: u8, x: u32, y: u32, px: f64, py: f64, extent: f64) -> (f64, f64) {
    let n = (1u64 << z) as f64;
    let fx = (x as f64 + px / extent) / n;
    let fy = (y as f64 + py / extent) / n;
    let lon = fx * 360.0 - 180.0;
    let lat = (std::f64::consts::PI * (1.0 - 2.0 * fy)).sinh().atan().to_degrees();
    (lat, lon)
}

fn tile_of(lat: f64, lon: f64, z: u8) -> (u32, u32) {
    let n = (1u64 << z) as f64;
    let x = ((lon + 180.0) / 360.0 * n).floor();
    let lr = lat.clamp(-85.05, 85.05).to_radians();
    let y = ((1.0 - (lr.tan() + 1.0 / lr.cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor();
    (x.clamp(0.0, n - 1.0) as u32, y.clamp(0.0, n - 1.0) as u32)
}

/// The part of a segment inside the box 0..=e, if any (Liang–Barsky).
fn clip(mut x0: f64, mut y0: f64, mut x1: f64, mut y1: f64, e: f64) -> Option<(f64, f64, f64, f64)> {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, x0), (dx, e - x0), (-dy, y0), (dy, e - y0)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return None;
                }
                t1 = t1.min(r);
            }
        }
    }
    let (nx0, ny0, nx1, ny1) = (x0 + t0 * dx, y0 + t0 * dy, x0 + t1 * dx, y0 + t1 * dy);
    x0 = nx0;
    y0 = ny0;
    x1 = nx1;
    y1 = ny1;
    Some((x0, y0, x1, y1))
}

/// Distance from p to the segment a-b, and how far along it the nearest point is (0 to 1).
fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 <= 1e-12 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) };
    let (qx, qy) = (a.0 + t * dx, a.1 + t * dy);
    (((p.0 - qx).powi(2) + (p.1 - qy).powi(2)).sqrt(), t)
}

// ---------- the road network ----------

#[derive(Clone, Copy)]
struct Node {
    x: f64,
    y: f64,
    lat: f64,
    lon: f64,
}

#[derive(Clone, Copy)]
struct Seg {
    a: u32,
    b: u32,
    cls: Cls,
    layer: i8,
    name: u32,
}

#[derive(Clone, Copy)]
struct Edge {
    to: u32,
    len: f32,
    cls: Cls,
    name: u32,
}

const MERGE_M: f64 = 1.5;
const CELL_M: f64 = 100.0;

struct Builder {
    proj: Proj,
    nodes: Vec<Node>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    segs: Vec<Seg>,
    names: Vec<String>,
    name_ix: HashMap<String, u32>,
    splits: HashMap<u32, Vec<(f64, u32)>>,
}

impl Builder {
    fn new(proj: Proj) -> Builder {
        let mut b = Builder { proj, nodes: Vec::new(), grid: HashMap::new(), segs: Vec::new(), names: Vec::new(), name_ix: HashMap::new(), splits: HashMap::new() };
        b.name("");
        b
    }

    fn name(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.name_ix.get(s) {
            return i;
        }
        self.names.push(s.to_string());
        let i = (self.names.len() - 1) as u32;
        self.name_ix.insert(s.to_string(), i);
        i
    }

    /// The node at this spot, joined with one that is already within a metre or two.
    fn node(&mut self, lat: f64, lon: f64) -> u32 {
        let (x, y) = self.proj.xy(lat, lon);
        let (cx, cy) = ((x / 3.0).floor() as i32, (y / 3.0).floor() as i32);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.grid.get(&(cx + dx, cy + dy)) {
                    for &i in v {
                        let n = &self.nodes[i as usize];
                        if (n.x - x).abs() <= MERGE_M && (n.y - y).abs() <= MERGE_M {
                            return i;
                        }
                    }
                }
            }
        }
        self.nodes.push(Node { x, y, lat, lon });
        let i = (self.nodes.len() - 1) as u32;
        self.grid.entry((cx, cy)).or_default().push(i);
        i
    }

    fn add_tile(&mut self, z: u8, tx: u32, ty: u32, t: &RoadTile) {
        let e = t.extent as f64;
        for l in &t.lines {
            let Some(cls) = classify(&l.kind, &l.detail) else { continue };
            let nm = if !l.name.is_empty() {
                l.name.clone()
            } else if !l.refname.is_empty() {
                l.refname.clone()
            } else {
                plain_name(&l.kind, &l.detail).to_string()
            };
            let name = self.name(&nm);
            for w in l.pts.windows(2) {
                let Some((x0, y0, x1, y1)) = clip(w[0].0 as f64, w[0].1 as f64, w[1].0 as f64, w[1].1 as f64, e) else { continue };
                let (la0, lo0) = tile_point(z, tx, ty, x0, y0, e);
                let (la1, lo1) = tile_point(z, tx, ty, x1, y1, e);
                let (a, b) = (self.node(la0, lo0), self.node(la1, lo1));
                if a != b {
                    self.segs.push(Seg { a, b, cls, layer: l.layer, name });
                }
            }
        }
    }

    fn pos(&self, n: u32) -> (f64, f64) {
        let n = &self.nodes[n as usize];
        (n.x, n.y)
    }

    /// Find every place where two road pieces cross or one ends on another, and note a split there.
    fn planarize(&mut self) {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let mut ranges: Vec<(i32, i32, i32, i32)> = Vec::with_capacity(self.segs.len());
        for (i, s) in self.segs.iter().enumerate() {
            let (a, b) = (self.pos(s.a), self.pos(s.b));
            let r = (
                ((a.0.min(b.0) - MERGE_M) / CELL_M).floor() as i32,
                ((a.1.min(b.1) - MERGE_M) / CELL_M).floor() as i32,
                ((a.0.max(b.0) + MERGE_M) / CELL_M).floor() as i32,
                ((a.1.max(b.1) + MERGE_M) / CELL_M).floor() as i32,
            );
            ranges.push(r);
            for cx in r.0..=r.2 {
                for cy in r.1..=r.3 {
                    cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        let mut found: Vec<(u32, f64, Option<u32>, u32, f64, Option<u32>)> = Vec::new();
        for (&(cx, cy), list) in &cells {
            if list.len() < 2 {
                continue;
            }
            for (ii, &i) in list.iter().enumerate() {
                for &j in &list[ii + 1..] {
                    let (ri, rj) = (ranges[i as usize], ranges[j as usize]);
                    if (cx, cy) != (ri.0.max(rj.0), ri.1.max(rj.1)) {
                        continue; // another cell handles this pair
                    }
                    let (si, sj) = (self.segs[i as usize], self.segs[j as usize]);
                    if si.layer != sj.layer || si.a == sj.a || si.a == sj.b || si.b == sj.a || si.b == sj.b {
                        continue;
                    }
                    if let Some(f) = self.meet(i, j) {
                        found.push(f);
                    }
                }
            }
        }
        for (i, ti, ni, j, tj, nj) in found {
            let (si, sj) = (self.segs[i as usize], self.segs[j as usize]);
            let _ = (si, sj);
            let node = match (ni, nj) {
                (Some(a), Some(_)) => Some(a),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };
            let node = match node {
                Some(n) => n,
                None => {
                    // a true crossing: a new node where the lines meet
                    let (a, b) = (self.pos(self.segs[i as usize].a), self.pos(self.segs[i as usize].b));
                    let (x, y) = (a.0 + ti * (b.0 - a.0), a.1 + ti * (b.1 - a.1));
                    let (la, lo) = self.proj.back(x, y);
                    self.node(la, lo)
                }
            };
            if ni.is_none() {
                self.splits.entry(i).or_default().push((ti, node));
            }
            if nj.is_none() {
                self.splits.entry(j).or_default().push((tj, node));
            }
        }
    }

    /// Do pieces i and j cross or touch? (i, along i, endpoint of i if the meeting is at its end, j, along j, endpoint of j)
    fn meet(&self, i: u32, j: u32) -> Option<(u32, f64, Option<u32>, u32, f64, Option<u32>)> {
        let (si, sj) = (self.segs[i as usize], self.segs[j as usize]);
        let (a, b) = (self.pos(si.a), self.pos(si.b));
        let (c, d) = (self.pos(sj.a), self.pos(sj.b));
        // an end of one lying on the other (a side street ending on a main road)
        let mut touch: Option<(u32, f64, Option<u32>, u32, f64, Option<u32>)> = None;
        for (n, p) in [(si.a, a), (si.b, b)] {
            let (dist, t) = seg_dist(p, c, d);
            if dist <= MERGE_M && t > 0.0 && t < 1.0 {
                touch = Some((i, if n == si.a { 0.0 } else { 1.0 }, Some(n), j, t, None));
                break;
            }
        }
        if touch.is_none() {
            for (n, p) in [(sj.a, c), (sj.b, d)] {
                let (dist, t) = seg_dist(p, a, b);
                if dist <= MERGE_M && t > 0.0 && t < 1.0 {
                    touch = Some((i, t, None, j, if n == sj.a { 0.0 } else { 1.0 }, Some(n)));
                    break;
                }
            }
        }
        if touch.is_some() {
            return touch;
        }
        // a proper crossing
        let (r, s) = ((b.0 - a.0, b.1 - a.1), (d.0 - c.0, d.1 - c.1));
        let den = r.0 * s.1 - r.1 * s.0;
        if den.abs() < 1e-9 {
            return None;
        }
        let t = ((c.0 - a.0) * s.1 - (c.1 - a.1) * s.0) / den;
        let u = ((c.0 - a.0) * r.1 - (c.1 - a.1) * r.0) / den;
        if t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0 {
            Some((i, t, None, j, u, None))
        } else {
            None
        }
    }

    /// The road piece nearest to a point that this way of travelling may use: (piece, how far along, metres away).
    fn nearest(&self, p: (f64, f64), mode: Mode, radius: f64, cells: &HashMap<(i32, i32), Vec<u32>>) -> Option<(u32, f64, f64)> {
        let (cx, cy) = ((p.0 / CELL_M).floor() as i32, (p.1 / CELL_M).floor() as i32);
        let maxr = (radius / CELL_M).ceil() as i32 + 1;
        let mut best: Option<(u32, f64, f64)> = None;
        for ring in 0..=maxr {
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue;
                    }
                    let Some(v) = cells.get(&(cx + dx, cy + dy)) else { continue };
                    for &i in v {
                        let s = self.segs[i as usize];
                        if (if mode == Mode::Car { s.cls.car } else { s.cls.foot }) <= 0.0 {
                            continue;
                        }
                        let (d, t) = seg_dist(p, self.pos(s.a), self.pos(s.b));
                        if d <= radius && best.map(|b| d < b.2).unwrap_or(true) {
                            best = Some((i, t, d));
                        }
                    }
                }
            }
            // once something is found, nothing in a farther ring can be closer than the ring's inner edge
            if let Some(b) = best {
                if b.2 <= ring as f64 * CELL_M {
                    break;
                }
            }
        }
        best
    }

    fn seg_cells(&self) -> HashMap<(i32, i32), Vec<u32>> {
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        for (i, s) in self.segs.iter().enumerate() {
            let (a, b) = (self.pos(s.a), self.pos(s.b));
            for cx in ((a.0.min(b.0)) / CELL_M).floor() as i32..=((a.0.max(b.0)) / CELL_M).floor() as i32 {
                for cy in ((a.1.min(b.1)) / CELL_M).floor() as i32..=((a.1.max(b.1)) / CELL_M).floor() as i32 {
                    cells.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        cells
    }

    fn split_at(&mut self, seg: u32, t: f64) -> u32 {
        let s = self.segs[seg as usize];
        if t * haversine_m((self.nodes[s.a as usize].lat, self.nodes[s.a as usize].lon), (self.nodes[s.b as usize].lat, self.nodes[s.b as usize].lon)) < 0.5 {
            return s.a;
        }
        if (1.0 - t) * haversine_m((self.nodes[s.a as usize].lat, self.nodes[s.a as usize].lon), (self.nodes[s.b as usize].lat, self.nodes[s.b as usize].lon)) < 0.5 {
            return s.b;
        }
        let (a, b) = (self.pos(s.a), self.pos(s.b));
        let (la, lo) = self.proj.back(a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        self.nodes.push(Node { x: a.0 + t * (b.0 - a.0), y: a.1 + t * (b.1 - a.1), lat: la, lon: lo });
        let n = (self.nodes.len() - 1) as u32;
        self.splits.entry(seg).or_default().push((t, n));
        n
    }

    fn edges(&self) -> Vec<Vec<Edge>> {
        let mut adj: Vec<Vec<Edge>> = vec![Vec::new(); self.nodes.len()];
        for (i, s) in self.segs.iter().enumerate() {
            let mut chain: Vec<(f64, u32)> = vec![(0.0, s.a)];
            if let Some(v) = self.splits.get(&(i as u32)) {
                let mut v = v.clone();
                v.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap_or(Ordering::Equal));
                chain.extend(v);
            }
            chain.push((1.0, s.b));
            for w in chain.windows(2) {
                let (n1, n2) = (w[0].1, w[1].1);
                if n1 == n2 {
                    continue;
                }
                let (p, q) = (&self.nodes[n1 as usize], &self.nodes[n2 as usize]);
                let len = haversine_m((p.lat, p.lon), (q.lat, q.lon)) as f32;
                adj[n1 as usize].push(Edge { to: n2, len, cls: s.cls, name: s.name });
                adj[n2 as usize].push(Edge { to: n1, len, cls: s.cls, name: s.name });
            }
        }
        adj
    }
}

#[derive(PartialEq)]
struct Q(f64, u32);
impl Eq for Q {}
impl PartialOrd for Q {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Q {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}

/// The quickest way between two nodes: the list of (node, edge used to reach it), and the seconds.
fn astar(adj: &[Vec<Edge>], nodes: &[Node], from: u32, to: u32, mode: Mode) -> Option<(Vec<(u32, Edge)>, f64)> {
    let vmax = if mode == Mode::Car { 105.0 / 3.6 } else { 5.0 / 3.6 };
    let goal = nodes[to as usize];
    let h = |n: u32| -> f64 {
        let p = &nodes[n as usize];
        ((p.x - goal.x).powi(2) + (p.y - goal.y).powi(2)).sqrt() * 0.98 / vmax
    };
    let mut g = vec![f64::INFINITY; nodes.len()];
    let mut prev: Vec<Option<(u32, Edge)>> = vec![None; nodes.len()];
    let mut heap = BinaryHeap::new();
    g[from as usize] = 0.0;
    heap.push(Q(h(from), from));
    let mut work = 0u64;
    while let Some(Q(f, n)) = heap.pop() {
        if n == to {
            break;
        }
        if f > g[n as usize] + h(n) + 1e-9 {
            continue;
        }
        work += 1;
        if work > 30_000_000 {
            return None;
        }
        for e in &adj[n as usize] {
            let speed = if mode == Mode::Car { e.cls.car } else { e.cls.foot };
            if speed <= 0.0 {
                continue;
            }
            let cost = g[n as usize] + e.len as f64 / (speed as f64 / 3.6);
            if cost < g[e.to as usize] {
                g[e.to as usize] = cost;
                prev[e.to as usize] = Some((n, *e));
                heap.push(Q(cost + h(e.to), e.to));
            }
        }
    }
    if !g[to as usize].is_finite() {
        return None;
    }
    let mut path: Vec<(u32, Edge)> = Vec::new();
    let mut at = to;
    while at != from {
        let (p, e) = prev[at as usize]?;
        path.push((at, e));
        at = p;
    }
    path.reverse();
    Some((path, g[to as usize]))
}

// ---------- reading the map ----------

struct Source {
    files: Vec<PmFile>,
    cache: HashMap<(u8, u32, u32), Option<Rc<RoadTile>>>,
}

impl Source {
    fn tile(&mut self, z: u8, x: u32, y: u32) -> R<Option<Rc<RoadTile>>> {
        if let Some(t) = self.cache.get(&(z, x, y)) {
            return Ok(t.clone());
        }
        let (la, lo) = tile_point(z, x, y, 2048.0, 2048.0, 4096.0);
        let mut found = None;
        let mut first_err: Option<String> = None;
        for f in self.files.iter_mut() {
            let (w, s, e, n) = f.header.bounds;
            if lo < w || lo > e || la < s || la > n {
                continue;
            }
            match f.tile(z, x, y) {
                Ok(Some(raw)) => {
                    match parse_roads(&raw) {
                        Ok(t) => found = Some(Rc::new(t)),
                        Err(e) => first_err = first_err.or(Some(e)),
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => first_err = first_err.or(Some(e)),
            }
        }
        if found.is_none() {
            if let Some(e) = first_err {
                return Err(e);
            }
        }
        if self.cache.len() > 6000 {
            self.cache.clear();
        }
        self.cache.insert((z, x, y), found.clone());
        Ok(found)
    }
}

fn dist_to_line(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    seg_dist(p, a, b).0
}

/// All the tiles within `half_km` of the straight line between the two places.
fn corridor(from: (f64, f64), to: (f64, f64), z: u8, half_km: f64, proj: &Proj) -> Vec<(u32, u32)> {
    let (x0, y0) = tile_of(from.0.max(to.0), from.1.min(to.1), z);
    let (x1, y1) = tile_of(from.0.min(to.0), from.1.max(to.1), z);
    let margin = ((half_km * 1000.0) / (40_075_000.0 * from.0.to_radians().cos().max(0.05) / (1u64 << z) as f64)).ceil() as u32 + 1;
    let (xa, xb) = (x0.saturating_sub(margin), (x1 + margin).min((1u32 << z) - 1));
    let (ya, yb) = (y0.saturating_sub(margin), (y1 + margin).min((1u32 << z) - 1));
    let a = proj.xy(from.0, from.1);
    let b = proj.xy(to.0, to.1);
    let tile_m = 40_075_000.0 * from.0.to_radians().cos().max(0.05) / (1u64 << z) as f64;
    let reach = half_km * 1000.0 + tile_m * 0.75;
    let mut out = Vec::new();
    if (xb - xa + 1) as u64 * (yb - ya + 1) as u64 > 400_000 {
        return out;
    }
    for x in xa..=xb {
        for y in ya..=yb {
            let (la, lo) = tile_point(z, x, y, 2048.0, 2048.0, 4096.0);
            if dist_to_line(proj.xy(la, lo), a, b) <= reach {
                out.push((x, y));
            }
        }
    }
    out
}

const MAX_TILES: usize = 2500;

/// Directions from one place to another, by car and on foot. `avoid` lists areas (each a ring of
/// (latitude, longitude) corners) the way must not go through. `packs` are the street map files.
pub fn directions(packs: &[PathBuf], from: (f64, f64), to: (f64, f64), avoid: &[Vec<(f64, f64)>]) -> R<Directions> {
    if !crate::maps::valid_point(from) || !crate::maps::valid_point(to) {
        return Err("One of the places is not a real position on the Earth.".into());
    }
    let mut files = Vec::new();
    let mut first_err = None;
    for p in packs {
        match PmFile::open(p) {
            Ok(f) => files.push(f),
            Err(e) => first_err = first_err.or(Some(e)),
        }
    }
    if files.is_empty() {
        return Err(match first_err {
            Some(e) => format!("The street map could not be opened ({}).", e),
            None => "There is no street map on this drive, so there are no roads to follow. Put one on with Get-Maps.bat.".to_string(),
        });
    }
    let straight_km = crate::maps::haversine_km(from, to);
    if straight_km > 1500.0 {
        return Err("These two places are more than 1,500 km apart. Directions this long are not worked out here; try a closer pair.".into());
    }
    let hi = files.iter().map(|f| f.header.max_zoom).max().unwrap_or(14);
    let lo = files.iter().map(|f| f.header.min_zoom).min().unwrap_or(0);
    let mut z = if straight_km <= 12.0 { 14 } else if straight_km <= 50.0 { 13 } else if straight_km <= 200.0 { 12 } else if straight_km <= 600.0 { 11 } else { 10 };
    z = z.min(hi).max(lo);
    let proj = Proj::new((from.0 + to.0) / 2.0, from.1);
    let mut src = Source { files, cache: HashMap::new() };
    let avoid_xy: Vec<Vec<(f64, f64)>> = avoid.iter().filter(|a| a.len() >= 3).map(|a| a.clone()).collect();

    let widths = [(3.0f64, 0.12f64), (8.0, 0.35), (20.0, 1.0)];
    let mut last_err = String::from("No way through was found.");
    let mut zz = z;
    loop {
        for (si, (min_km, frac)) in widths.iter().enumerate() {
            let half = (min_km).max(straight_km * frac);
            let tiles = corridor(from, to, zz, half, &proj);
            if tiles.len() > MAX_TILES && zz > lo.max(8) {
                zz -= 1;
                break;
            }
            if tiles.is_empty() {
                return Err("That trip covers too much of the map to work out.".into());
            }
            let mut b = Builder::new(proj);
            let mut got = 0usize;
            for (x, y) in &tiles {
                if let Some(t) = src.tile(zz, *x, *y)? {
                    b.add_tile(zz, *x, *y, &t);
                    got += 1;
                }
            }
            if got == 0 {
                return Err("The street map on this drive does not cover these places.".into());
            }
            match solve(&mut b, from, to, &avoid_xy, zz) {
                Ok(mut d) => {
                    refine_names(&mut src, &mut d, hi);
                    return Ok(d);
                }
                Err(e) => {
                    last_err = e;
                    if si == widths.len() - 1 {
                        return Err(last_err);
                    }
                }
            }
        }
        if zz == z {
            // the widths loop ended without a tile-count change
            return Err(last_err);
        }
        z = zz;
    }
}

fn solve(b: &mut Builder, from: (f64, f64), to: (f64, f64), avoid: &[Vec<(f64, f64)>], zoom: u8) -> R<Directions> {
    b.planarize();
    let cells = b.seg_cells();
    let fp = b.proj.xy(from.0, from.1);
    let tp = b.proj.xy(to.0, to.1);
    let radius = if zoom >= 13 { 1500.0 } else { 3500.0 };
    // attach each place to the nearest road, for each way of travelling
    let mut snaps: HashMap<(bool, Mode), (u32, f64)> = HashMap::new(); // (is start, mode) -> node, metres from the place
    let mut errs: HashMap<Mode, String> = HashMap::new();
    for mode in [Mode::Car, Mode::Foot] {
        for (is_start, p, label) in [(true, fp, "start"), (false, tp, "end")] {
            match b.nearest(p, mode, radius, &cells) {
                Some((seg, t, d)) => {
                    let n = b.split_at(seg, t);
                    snaps.insert((is_start, mode), (n, d));
                }
                None => {
                    errs.insert(mode, format!("No {} was found within {:.1} km of the {}.", if mode == Mode::Car { "road a car can use" } else { "road or path" }, radius / 1000.0, label));
                }
            }
        }
    }
    let mut adj = b.edges();
    if !avoid.is_empty() {
        // a stretch of road is out when any point along it (every 15 m or so) lies inside an area to avoid
        // (a start or end that is itself inside an area keeps its roads, or the trip could never begin)
        let ends: Vec<u32> = snaps.values().map(|v| v.0).filter(|&n| avoid.iter().any(|poly| crate::maps::in_polygon((b.nodes[n as usize].lat, b.nodes[n as usize].lon), poly))).collect();
        let boxes: Vec<(f64, f64, f64, f64)> = avoid
            .iter()
            .map(|p| (p.iter().map(|q| q.0).fold(f64::MAX, f64::min), p.iter().map(|q| q.1).fold(f64::MAX, f64::min), p.iter().map(|q| q.0).fold(f64::MIN, f64::max), p.iter().map(|q| q.1).fold(f64::MIN, f64::max)))
            .collect();
        for n in 0..adj.len() {
            let p = b.nodes[n];
            adj[n].retain(|e| {
                if ends.contains(&(n as u32)) || ends.contains(&e.to) {
                    return true;
                }
                let q = b.nodes[e.to as usize];
                let steps = ((e.len as f64 / 15.0).ceil() as usize).max(1);
                for (poly, bx) in avoid.iter().zip(&boxes) {
                    if p.lat.max(q.lat) < bx.0 || p.lat.min(q.lat) > bx.2 || p.lon.max(q.lon) < bx.1 || p.lon.min(q.lon) > bx.3 {
                        continue;
                    }
                    for k in 0..=steps {
                        let f = k as f64 / steps as f64;
                        if crate::maps::in_polygon((p.lat + f * (q.lat - p.lat), p.lon + f * (q.lon - p.lon)), poly) {
                            return false;
                        }
                    }
                }
                true
            });
        }
    }
    let mut out: HashMap<Mode, Result<Route, String>> = HashMap::new();
    for mode in [Mode::Car, Mode::Foot] {
        if let Some(e) = errs.get(&mode) {
            out.insert(mode, Err(e.clone()));
            continue;
        }
        let (sn, sd) = snaps[&(true, mode)];
        let (en, ed) = snaps[&(false, mode)];
        let Some((path, secs)) = astar(&adj, &b.nodes, sn, en, mode) else {
            out.insert(mode, Err(if avoid.is_empty() { "No way was found between these two places along the roads in the map.".to_string() } else { "No way was found that stays out of the areas you marked to avoid.".to_string() }));
            continue;
        };
        // the walk or drive from the place itself to the road and from the road to the other place
        let side_speed = if mode == Mode::Car { 20.0 } else { 5.0 } / 3.6;
        let mut points: Vec<(f64, f64)> = vec![from, (b.nodes[sn as usize].lat, b.nodes[sn as usize].lon)];
        let mut km = (sd + ed) / 1000.0;
        let minutes = (secs + (sd + ed) / side_speed) / 60.0;
        let mut steps: Vec<Step> = Vec::new();
        if sd > 30.0 {
            steps.push(Step { name: "straight across to the nearest road".to_string(), km: sd / 1000.0, minutes: sd / side_speed / 60.0, at: 0 });
        }
        let mut last_name = u32::MAX;
        for (n, e) in &path {
            points.push((b.nodes[*n as usize].lat, b.nodes[*n as usize].lon));
            let speed = if mode == Mode::Car { e.cls.car } else { e.cls.foot } as f64;
            let (k, m) = (e.len as f64 / 1000.0, e.len as f64 / (speed / 3.6) / 60.0);
            km += k;
            if e.name == last_name {
                if let Some(s) = steps.last_mut() {
                    s.km += k;
                    s.minutes += m;
                }
            } else {
                steps.push(Step { name: b.names[e.name as usize].clone(), km: k, minutes: m, at: points.len().saturating_sub(2) });
                last_name = e.name;
            }
        }
        steps = tidy_steps(steps);
        points.push(to);
        if ed > 30.0 {
            steps.push(Step { name: "straight across to the end".to_string(), km: ed / 1000.0, minutes: ed / side_speed / 60.0, at: points.len().saturating_sub(1) });
        }
        let mut note = String::new();
        if sd > 200.0 || ed > 200.0 {
            note = "The start or the end is some way from a road in the map, so that part is a straight line.".to_string();
        } else if zoom < 13 {
            note = "A long trip: only the bigger roads were used, so side streets at either end are approximate.".to_string();
        }
        out.insert(mode, Ok(Route { points, km, minutes, steps, note }));
    }
    Ok(Directions { car: out.remove(&Mode::Car).unwrap(), foot: out.remove(&Mode::Foot).unwrap(), zoom })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roadtiles::testkit::{file, tile, L};

    const Z: u8 = 14;
    const X: u32 = 4530;
    const Y: u32 = 6280;

    fn at(dx: u32, px: f64, py: f64) -> (f64, f64) {
        tile_point(Z, X + dx, Y, px, py, 4096.0)
    }

    fn write(name: &str, tiles: Vec<(u8, u32, u32, Vec<u8>)>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cindrix-routing-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("s.pmtiles");
        std::fs::write(&p, file(&tiles, (-180.0, -85.0, 180.0, 85.0))).unwrap();
        p
    }

    fn l(kind: &'static str, detail: &'static str, name: &'static str, pts: Vec<(i32, i32)>) -> L {
        (kind, detail, name, false, pts)
    }

    #[test]
    fn a_crossing_without_a_shared_corner_is_a_junction() {
        // Main St runs across; Oak St runs down and crosses it with no corner point at the crossing.
        let t = tile(&[
            l("major_road", "primary", "Main St", vec![(0, 2048), (4096, 2048)]),
            l("minor_road", "residential", "Oak St", vec![(2048, 0), (2048, 4096)]),
        ]);
        let p = write("cross", vec![(Z, X, Y, t)]);
        // from the north end of Oak St to the east end of Main St: must turn at the crossing
        let d = directions(&[p], at(0, 2048.0, 100.0), at(0, 4000.0, 2048.0), &[]).unwrap();
        let car = d.car.unwrap();
        let names: Vec<&str> = car.steps.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["Oak St", "Main St"], "{:?}", car.steps);
        assert!(car.km > 1.0 && car.km < 2.5, "{}", car.km);
    }

    #[test]
    fn a_side_street_ending_on_a_road_joins_it() {
        let t = tile(&[
            l("major_road", "secondary", "Long Rd", vec![(0, 2048), (4096, 2048)]),
            l("minor_road", "residential", "Short St", vec![(1000, 3000), (1000, 2048)]),
        ]);
        let p = write("tee", vec![(Z, X, Y, t)]);
        let d = directions(&[p], at(0, 1000.0, 3000.0), at(0, 3500.0, 2048.0), &[]).unwrap();
        let names: Vec<String> = d.car.unwrap().steps.into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["Short St".to_string(), "Long Rd".to_string()]);
    }

    #[test]
    fn a_road_that_goes_on_into_the_next_tile_is_followed() {
        // the same road in two neighbouring tiles; the second tile's copy runs a little past its edge, as real tiles do
        let a = tile(&[l("major_road", "primary", "Pike", vec![(0, 2048), (4160, 2048)])]);
        let b = tile(&[l("major_road", "primary", "Pike", vec![(-64, 2048), (4096, 2048)])]);
        let p = write("two", vec![(Z, X, Y, a), (Z, X + 1, Y, b)]);
        let d = directions(&[p], at(0, 200.0, 2048.0), at(1, 3800.0, 2048.0), &[]).unwrap();
        let car = d.car.unwrap();
        assert_eq!(car.steps.len(), 1, "{:?}", car.steps);
        // about two tiles' width less a little: z14 tiles are roughly 1.9 km wide here
        assert!(car.km > 3.0 && car.km < 4.2, "{}", car.km);
        // 65 km/h on a primary road
        assert!((car.minutes - car.km / 65.0 * 60.0).abs() < 1.0, "{} min for {} km", car.minutes, car.km);
    }

    #[test]
    fn foot_may_take_a_path_and_a_car_may_not() {
        // a long way round by road, and a short cut that is only a footpath
        let t = tile(&[
            l("minor_road", "residential", "Round Rd", vec![(200, 3800), (200, 200), (3900, 200), (3900, 3800)]),
            l("path", "footway", "", vec![(200, 3800), (3900, 3800)]),
        ]);
        let p = write("path", vec![(Z, X, Y, t)]);
        let d = directions(&[p], at(0, 200.0, 3800.0), at(0, 3900.0, 3800.0), &[]).unwrap();
        let (car, foot) = (d.car.unwrap(), d.foot.unwrap());
        assert!(foot.km < car.km * 0.6, "foot {} car {}", foot.km, car.km);
        assert_eq!(foot.steps.len(), 1);
        assert_eq!(foot.steps[0].name, "a path");
        // 5 km/h on foot
        assert!((foot.minutes - foot.km / 5.0 * 60.0).abs() < 1.0);
    }

    #[test]
    fn short_joins_do_not_clutter_the_list() {
        let st = |n: &str, km: f64| Step { name: n.to_string(), km, minutes: km, at: 0 };
        let out = tidy_steps(vec![st("the highway", 9.0), st("a road", 0.0), st("the highway", 13.0), st("a road", 0.1), st("Main St", 2.0)]);
        let names: Vec<&str> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["the highway", "Main St"]);
        assert!((out[0].km - 22.1).abs() < 1e-9);
    }

    #[test]
    fn a_long_trips_unnamed_highway_gets_its_name_from_the_sharper_map() {
        let named = tile(&[l("highway", "motorway", "Western Kentucky Parkway", vec![(0, 2048), (4096, 2048)])]);
        let p = write("refine", vec![(12, X / 4, Y / 4, named)]);
        let pts: Vec<(f64, f64)> = (1..=7).map(|i| tile_point(12, X / 4, Y / 4, i as f64 * 500.0, 2048.0, 4096.0)).collect();
        let route = Route {
            points: pts,
            km: 60.0,
            minutes: 40.0,
            steps: vec![Step { name: "the highway".into(), km: 60.0, minutes: 40.0, at: 0 }],
            note: String::new(),
        };
        let mut d = Directions { car: Ok(route.clone()), foot: Ok(route), zoom: 10 };
        let mut src = Source { files: vec![PmFile::open(&p).unwrap()], cache: HashMap::new() };
        refine_names(&mut src, &mut d, 14);
        assert_eq!(d.car.unwrap().steps[0].name, "Western Kentucky Parkway");
    }

    #[test]
    fn a_motorway_is_for_cars_only() {
        let t = tile(&[
            l("highway", "motorway", "I-79", vec![(0, 1000), (4096, 1000)]),
            l("minor_road", "residential", "Old Rd", vec![(0, 1000), (0, 1500), (4096, 1500), (4096, 1000)]),
        ]);
        let p = write("moto", vec![(Z, X, Y, t)]);
        let d = directions(&[p], at(0, 20.0, 1000.0), at(0, 4000.0, 1000.0), &[]).unwrap();
        assert!(d.car.as_ref().unwrap().steps.iter().any(|s| s.name == "I-79"));
        assert!(!d.foot.as_ref().unwrap().steps.iter().any(|s| s.name == "I-79"));
    }

    #[test]
    fn an_area_to_avoid_sends_the_way_round() {
        let t = tile(&[
            l("minor_road", "residential", "Short St", vec![(200, 2048), (3900, 2048)]),
            l("minor_road", "residential", "Detour Ave", vec![(200, 2048), (200, 300), (3900, 300), (3900, 2048)]),
        ]);
        let p = write("avoid", vec![(Z, X, Y, t)]);
        let (from, to) = (at(0, 200.0, 2048.0), at(0, 3900.0, 2048.0));
        let direct = directions(&[p.clone()], from, to, &[]).unwrap().car.unwrap();
        assert_eq!(direct.steps.len(), 1);
        // a box over the middle of Short St
        let ring = vec![at(0, 1800.0, 1900.0), at(0, 2300.0, 1900.0), at(0, 2300.0, 2200.0), at(0, 1800.0, 2200.0)];
        let around = directions(&[p], from, to, &[ring]).unwrap().car.unwrap();
        assert!(around.km > direct.km * 1.5, "{} vs {}", around.km, direct.km);
        assert!(around.steps.iter().any(|s| s.name == "Detour Ave"));
    }

    #[test]
    fn a_place_far_from_any_road_is_said_plainly() {
        let t = tile(&[l("minor_road", "residential", "Lone Rd", vec![(0, 100), (500, 100)])]);
        let p = write("far", vec![(Z, X, Y, t)]);
        let d = directions(&[p], at(0, 100.0, 100.0), at(0, 4000.0, 4000.0), &[]).unwrap();
        let e = d.car.unwrap_err();
        assert!(e.contains("No road"), "{}", e);
    }

    #[test]
    fn no_map_and_bad_places_give_plain_errors() {
        assert!(directions(&[], (39.0, -80.0), (39.1, -80.0), &[]).unwrap_err().contains("no street map"));
        assert!(directions(&[], (99.0, -80.0), (39.1, -80.0), &[]).is_err());
        let t = tile(&[l("minor_road", "residential", "A", vec![(0, 100), (500, 100)])]);
        let p = write("far2", vec![(Z, X, Y, t)]);
        assert!(directions(&[p], (39.0, -80.0), (39.0, -60.0), &[]).unwrap_err().contains("1,500"));
    }

    #[test]
    fn a_town_sized_grid_of_streets_is_quick_enough() {
        // 12 x 12 tiles, each a grid of 10 streets each way with no shared corner points
        let mut tiles = Vec::new();
        for dx in 0..12u32 {
            for dy in 0..12u32 {
                let mut lines: Vec<L> = Vec::new();
                for k in 0..10 {
                    let c = 200 + k * 410;
                    lines.push(l("minor_road", "residential", "Row St", vec![(0, c), (4096, c)]));
                    lines.push(l("minor_road", "residential", "Col St", vec![(c, 0), (c, 4096)]));
                }
                tiles.push((Z, X + dx, Y + dy, tile(&lines)));
            }
        }
        let p = write("grid", tiles);
        let t0 = std::time::Instant::now();
        let d = directions(&[p], tile_point(Z, X, Y, 205.0, 205.0, 4096.0), tile_point(Z, X + 11, Y + 11, 3500.0, 3500.0, 4096.0), &[]).unwrap();
        let car = d.car.unwrap();
        assert!(car.km > 30.0, "{}", car.km);
        assert!(t0.elapsed().as_secs() < 60, "{:?}", t0.elapsed());
        eprintln!("grid route: {} km in {:?}", car.km, t0.elapsed());
    }
}
