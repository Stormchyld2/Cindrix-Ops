//! Full-text search inside ZIM archives, with no Kiwix program: the program reads each article once, makes its own
//! word index in a folder on the drive, and searches that.
//!
//! One index is one folder (`docs.dat`, `docs.off`, `post.dat`, `dict.dat`, `sparse.dat`, `meta.txt`). It is built in
//! pieces: articles are read in order, and every few million words the pieces so far are saved as a sorted segment and
//! progress is noted, so a build that is stopped (or the computer is turned off) carries on where it left off. When
//! every article has been read the segments are merged and `meta.txt` is written last: a folder without it is not an
//! index. The index is tied to the archive it was made from (its size and its own id), so a replaced archive makes it
//! "out of date" instead of giving wrong answers.
//!
//! Words are lower-cased, split at anything that is not a letter or digit, stripped of a few common endings (plurals,
//! "ing", "ed") so "boiling" finds "boiled", and the commonest English words are left out. A word in a title counts for
//! much more than one in the text. The depth says how much of each article is read: the opening part (`Lead`, small
//! and fast, right for huge archives) or nearly all of it (`Full`).

use crate::kiwix::html_to_text;
use crate::zimread::{Archive, Kind};
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Words held in memory before a segment is saved (a few hundred megabytes at most; small enough for a phone).
pub const DEFAULT_BUDGET: usize = 3_000_000;
const SPARSE_EVERY: u64 = 64;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    Lead,
    Full,
}
impl Depth {
    /// How many characters of each article's text are read.
    pub fn chars(self) -> usize {
        match self {
            Depth::Lead => 4_000,
            Depth::Full => 60_000,
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            Depth::Lead => "lead",
            Depth::Full => "full",
        }
    }
    pub fn parse(s: &str) -> Option<Depth> {
        match s {
            "lead" => Some(Depth::Lead),
            "full" => Some(Depth::Full),
            _ => None,
        }
    }
    /// A rough guess of the index size per article, for telling the person before they start.
    pub fn bytes_per_article(self) -> u64 {
        match self {
            Depth::Lead => 700,
            Depth::Full => 2_500,
        }
    }
}

// ---------- words ----------

const STOP: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "had", "her", "was", "one", "our", "out", "has", "have", "that", "this", "with", "from", "they", "will", "would", "there", "their",
    "what", "when", "which", "were", "been", "than", "then", "them", "these", "those", "into", "also", "its", "who", "how", "about", "other", "some", "such", "more", "most", "over", "only", "very", "your", "just",
];

fn stem(w: &str) -> String {
    let n = w.chars().count();
    let w = w.strip_suffix("ies").filter(|_| n > 4).map(|x| format!("{}y", x)).unwrap_or_else(|| w.to_string());
    let n = w.chars().count();
    if n > 3 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") && !w.ends_with("is") {
        return w[..w.len() - 1].to_string();
    }
    if n >= 7 && w.ends_with("ing") {
        return w[..w.len() - 3].to_string();
    }
    if n >= 6 && w.ends_with("ed") {
        return w[..w.len() - 2].to_string();
    }
    w
}

/// The words of a text as they are indexed and searched for.
pub fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut flush = |cur: &mut String, out: &mut Vec<String>| {
        if cur.is_empty() {
            return;
        }
        let n = cur.chars().count();
        if (2..=32).contains(&n) && !STOP.contains(&cur.as_str()) {
            out.push(stem(cur));
        }
        cur.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

// ---------- small binary helpers ----------

fn put(v: &mut Vec<u8>, mut n: u64) {
    while n >= 0x80 {
        v.push((n as u8 & 0x7f) | 0x80);
        n >>= 7;
    }
    v.push(n as u8);
}
fn get(b: &[u8], pos: &mut usize) -> Option<u64> {
    let (mut n, mut shift) = (0u64, 0u32);
    loop {
        let x = *b.get(*pos)?;
        *pos += 1;
        if shift > 63 {
            return None;
        }
        n |= ((x & 0x7f) as u64) << shift;
        if x & 0x80 == 0 {
            return Some(n);
        }
        shift += 7;
    }
}
fn read_varint<R: Read>(r: &mut R) -> io::Result<Option<u64>> {
    let (mut n, mut shift) = (0u64, 0u32);
    let mut first = true;
    loop {
        let mut b = [0u8; 1];
        match r.read(&mut b)? {
            0 if first => return Ok(None),
            0 => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "cut short")),
            _ => {}
        }
        first = false;
        if shift > 63 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "number too long"));
        }
        n |= ((b[0] & 0x7f) as u64) << shift;
        if b[0] & 0x80 == 0 {
            return Ok(Some(n));
        }
        shift += 7;
    }
}
fn bad<T>(m: &str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, m.to_string()))
}

fn decode_postings(bytes: &[u8]) -> Vec<(u32, u16)> {
    let (mut pos, mut doc) = (0usize, 0u64);
    let mut out = Vec::new();
    while pos < bytes.len() {
        let (Some(d), Some(t)) = (get(bytes, &mut pos), get(bytes, &mut pos)) else { break };
        doc += d;
        if doc > u32::MAX as u64 {
            break;
        }
        out.push((doc as u32, t.min(u16::MAX as u64) as u16));
    }
    out
}
fn encode_postings(list: &[(u32, u16)]) -> Vec<u8> {
    let mut v = Vec::with_capacity(list.len() * 3);
    let mut prev = 0u64;
    for (d, t) in list {
        put(&mut v, *d as u64 - prev);
        put(&mut v, *t as u64);
        prev = *d as u64;
    }
    v
}

// ---------- names and fingerprints ----------

/// Tells one archive from another by its size and its own id (from the header), so a replaced file is noticed.
pub fn fingerprint(archive_file: &Path) -> io::Result<String> {
    let mut f = File::open(archive_file)?;
    let len = f.metadata()?.len();
    let mut h = [0u8; 24];
    f.read_exact(&mut h)?;
    Ok(format!("{:x}-{}", len, h[8..24].iter().map(|b| format!("{:02x}", b)).collect::<String>()))
}

/// The folder an archive's index lives in, below `base`.
pub fn index_dir_for(base: &Path, archive_file: &Path, fp: &str) -> PathBuf {
    let stem: String = archive_file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(60).collect();
    base.join(format!("{}-{}", if stem.is_empty() { "archive".to_string() } else { stem }, fp.rsplit('-').next().unwrap_or("x").chars().take(8).collect::<String>()))
}

// ---------- status ----------

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    NotBuilt,
    /// Started and stopped (or interrupted): this many entries read of this many.
    Partial { done: u32, total: u32 },
    Ready { docs: u32, terms: u64, depth: Depth, bytes: u64 },
    /// Made from a different archive file than the one there now.
    OutOfDate,
}

fn kv(text: &str) -> HashMap<String, String> {
    text.lines().filter_map(|l| l.split_once(' ')).map(|(k, v)| (k.to_string(), v.trim().to_string())).collect()
}
fn dir_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir).map(|d| d.filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0)
}

pub fn status(dir: &Path, fp: &str) -> Status {
    if let Ok(t) = fs::read_to_string(dir.join("meta.txt")) {
        let m = kv(&t);
        if m.get("fp").map(|x| x.as_str()) != Some(fp) {
            return Status::OutOfDate;
        }
        let num = |k: &str| m.get(k).and_then(|v| v.parse::<u64>().ok());
        if let (Some(docs), Some(terms), Some(depth)) = (num("docs"), num("terms"), m.get("depth").and_then(|d| Depth::parse(d))) {
            return Status::Ready { docs: docs as u32, terms, depth, bytes: dir_bytes(dir) };
        }
        return Status::NotBuilt;
    }
    if let Ok(t) = fs::read_to_string(dir.join("progress.txt")) {
        let m = kv(&t);
        if m.get("fp").map(|x| x.as_str()) != Some(fp) {
            return Status::OutOfDate;
        }
        let num = |k: &str| m.get(k).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        return Status::Partial { done: num("next"), total: num("articles") };
    }
    Status::NotBuilt
}

/// Remove an index (or a half-made one).
pub fn delete(dir: &Path) -> io::Result<()> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

// ---------- building ----------

/// Shared with the screen: how far a build has got, and a way to ask it to stop.
#[derive(Default)]
pub struct Progress {
    pub done: AtomicU64,
    pub total: AtomicU64,
    /// 0 not running, 1 reading articles, 2 joining the pieces.
    pub phase: AtomicU64,
    pub stop: AtomicBool,
}
impl Progress {
    pub fn new() -> Arc<Progress> {
        Arc::new(Progress::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Done,
    Stopped,
}

struct Seg {
    path: PathBuf,
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp-w");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

fn wipe(dir: &Path) -> io::Result<()> {
    if dir.exists() {
        for e in fs::read_dir(dir)? {
            let p = e?.path();
            if p.is_file() {
                fs::remove_file(p)?;
            }
        }
    }
    fs::create_dir_all(dir)
}

fn flush_segment(map: &mut HashMap<String, Vec<(u32, u16)>>, path: &Path) -> io::Result<()> {
    let mut keys: Vec<(String, Vec<(u32, u16)>)> = map.drain().collect();
    keys.sort_by(|a, b| a.0.cmp(&b.0));
    let tmp = path.with_extension("part");
    let mut w = BufWriter::new(File::create(&tmp)?);
    for (term, list) in keys {
        let bytes = encode_postings(&list);
        let mut head = Vec::new();
        put(&mut head, term.len() as u64);
        head.extend_from_slice(term.as_bytes());
        put(&mut head, list.len() as u64);
        put(&mut head, bytes.len() as u64);
        w.write_all(&head)?;
        w.write_all(&bytes)?;
    }
    w.flush()?;
    w.get_ref().sync_data()?;
    drop(w);
    fs::rename(tmp, path)
}

struct SegReader {
    r: BufReader<File>,
    cur: Option<(String, Vec<u8>)>,
}
impl SegReader {
    fn open(p: &Path) -> io::Result<SegReader> {
        let mut s = SegReader { r: BufReader::with_capacity(256 * 1024, File::open(p)?), cur: None };
        s.advance()?;
        Ok(s)
    }
    fn advance(&mut self) -> io::Result<()> {
        self.cur = None;
        let Some(tl) = read_varint(&mut self.r)? else { return Ok(()) };
        if tl == 0 || tl > 256 {
            return bad("a segment is damaged");
        }
        let mut t = vec![0u8; tl as usize];
        self.r.read_exact(&mut t)?;
        let _df = read_varint(&mut self.r)?.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "cut short"))?;
        let bl = read_varint(&mut self.r)?.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "cut short"))?;
        if bl > 1 << 31 {
            return bad("a segment is damaged");
        }
        let mut b = vec![0u8; bl as usize];
        self.r.read_exact(&mut b)?;
        self.cur = Some((String::from_utf8(t).or_else(|_| bad("a segment is damaged"))?, b));
        Ok(())
    }
}

fn merge(dir: &Path, segs: &[Seg], progress: &Progress) -> io::Result<(u64, u64)> {
    let mut readers: Vec<SegReader> = Vec::new();
    for s in segs {
        readers.push(SegReader::open(&s.path)?);
    }
    let mut heap: BinaryHeap<Reverse<(String, usize)>> = BinaryHeap::new();
    for (i, r) in readers.iter().enumerate() {
        if let Some((t, _)) = &r.cur {
            heap.push(Reverse((t.clone(), i)));
        }
    }
    let mut post = BufWriter::new(File::create(dir.join("post.dat"))?);
    let mut dict = BufWriter::new(File::create(dir.join("dict.dat"))?);
    let mut sparse = BufWriter::new(File::create(dir.join("sparse.dat"))?);
    let (mut post_pos, mut dict_pos, mut terms) = (0u64, 0u64, 0u64);
    while let Some(Reverse((term, first))) = heap.pop() {
        let mut parts = vec![first];
        while let Some(Reverse((t, _))) = heap.peek() {
            if *t != term {
                break;
            }
            let Reverse((_, i)) = heap.pop().unwrap();
            parts.push(i);
        }
        let mut all: Vec<(u32, u16)> = Vec::new();
        for i in parts {
            let (_, bytes) = readers[i].cur.take().unwrap_or_default();
            // each segment's first number is counted from zero, so its postings are decoded and joined
            all.extend(decode_postings(&bytes));
            readers[i].advance()?;
            if let Some((t, _)) = &readers[i].cur {
                heap.push(Reverse((t.clone(), i)));
            }
        }
        let bytes = encode_postings(&all);
        if terms % SPARSE_EVERY == 0 {
            let mut s = Vec::new();
            put(&mut s, term.len() as u64);
            s.extend_from_slice(term.as_bytes());
            s.extend_from_slice(&dict_pos.to_le_bytes());
            sparse.write_all(&s)?;
        }
        let mut e = Vec::new();
        put(&mut e, term.len() as u64);
        e.extend_from_slice(term.as_bytes());
        put(&mut e, all.len() as u64);
        put(&mut e, post_pos);
        put(&mut e, bytes.len() as u64);
        dict.write_all(&e)?;
        post.write_all(&bytes)?;
        dict_pos += e.len() as u64;
        post_pos += bytes.len() as u64;
        terms += 1;
        if terms % 50_000 == 0 && progress.stop.load(Ordering::Relaxed) {
            // joining is quick; a stop request here is honoured by throwing the half-joined files away (the segments stay)
            return Err(io::Error::new(io::ErrorKind::Interrupted, "stopped while joining"));
        }
    }
    for w in [&mut post, &mut dict, &mut sparse] {
        w.flush()?;
        w.get_ref().sync_data()?;
    }
    Ok((terms, dict_pos))
}

/// Read every article of `archive` and make (or finish making) its index in `dir`. A build that was stopped carries on
/// from where it was. `budget` is how many words are held in memory before a piece is saved.
pub fn build(archive: &Archive, dir: &Path, fp: &str, depth: Depth, budget: usize, progress: &Progress) -> Result<Outcome, String> {
    build_inner(archive, dir, fp, depth, budget, progress, None).map_err(|e| format!("The search index could not be made: {}", e))
}

fn build_inner(archive: &Archive, dir: &Path, fp: &str, depth: Depth, budget: usize, progress: &Progress, stop_after: Option<u32>) -> io::Result<Outcome> {
    fs::create_dir_all(dir)?;
    progress.phase.store(1, Ordering::Relaxed);
    progress.total.store(archive.articles as u64, Ordering::Relaxed);
    let result: io::Result<Outcome> = (|| {
        if let Ok(t) = fs::read_to_string(dir.join("meta.txt")) {
            let m = kv(&t);
            if m.get("fp").map(|x| x.as_str()) == Some(fp) && m.get("depth").map(|x| x.as_str()) == Some(depth.word()) {
                progress.done.store(archive.articles as u64, Ordering::Relaxed);
                return Ok(Outcome::Done);
            }
            wipe(dir)?;
        }
        let (mut next, mut docs, mut segs_n) = (0u32, 0u32, 0u32);
        let (mut dat_len, mut off_len) = (0u64, 0u64);
        let mut resumed = false;
        if let Ok(t) = fs::read_to_string(dir.join("progress.txt")) {
            let m = kv(&t);
            let num = |k: &str| m.get(k).and_then(|v| v.parse::<u64>().ok());
            if m.get("fp").map(|x| x.as_str()) == Some(fp) && m.get("depth").map(|x| x.as_str()) == Some(depth.word()) {
                if let (Some(n), Some(d), Some(dl), Some(ol), Some(s)) = (num("next"), num("docs"), num("docdat"), num("docoff"), num("segs")) {
                    let have_segs = (1..=s).all(|i| dir.join(format!("seg-{:05}.dat", i)).exists());
                    let lens_ok = fs::metadata(dir.join("docs.dat")).map(|m| m.len() >= dl).unwrap_or(false) && fs::metadata(dir.join("docs.off")).map(|m| m.len() >= ol).unwrap_or(false);
                    if have_segs && lens_ok && ol == 8 * d {
                        (next, docs, dat_len, off_len, segs_n, resumed) = (n as u32, d as u32, dl, ol, s as u32, true);
                    }
                }
            }
        }
        if !resumed {
            wipe(dir)?;
        }
        let mut dat = OpenOptions::new().create(true).write(true).open(dir.join("docs.dat"))?;
        let mut off = OpenOptions::new().create(true).write(true).open(dir.join("docs.off"))?;
        dat.set_len(dat_len)?;
        off.set_len(off_len)?;
        dat.seek(SeekFrom::Start(dat_len))?;
        off.seek(SeekFrom::Start(off_len))?;
        let (mut dat, mut off) = (BufWriter::new(dat), BufWriter::new(off));
        let ns = archive.content_ns();
        let cap = depth.chars();
        let mut map: HashMap<String, Vec<(u32, u16)>> = HashMap::new();
        let mut held = 0usize;
        let save = |next: u32, docs: u32, segs: u32, dl: u64, ol: u64| -> io::Result<()> {
            let t = format!("fp {}\ndepth {}\nnext {}\narticles {}\ndocs {}\ndocdat {}\ndocoff {}\nsegs {}\n", fp, depth.word(), next, archive.articles, docs, dl, ol, segs);
            write_atomic(&dir.join("progress.txt"), t.as_bytes())
        };
        let mut checkpoint = |map: &mut HashMap<String, Vec<(u32, u16)>>, held: &mut usize, next: u32, docs: u32, segs_n: &mut u32, dat: &mut BufWriter<File>, off: &mut BufWriter<File>, dat_len: u64, off_len: u64| -> io::Result<()> {
            if !map.is_empty() {
                *segs_n += 1;
                flush_segment(map, &dir.join(format!("seg-{:05}.dat", *segs_n)))?;
                *held = 0;
            }
            dat.flush()?;
            off.flush()?;
            dat.get_ref().sync_data()?;
            off.get_ref().sync_data()?;
            save(next, docs, *segs_n, dat_len, off_len)
        };
        let mut i = next;
        while i < archive.articles {
            let stopping = progress.stop.load(Ordering::Relaxed) || stop_after.map(|n| i >= n).unwrap_or(false);
            if stopping {
                checkpoint(&mut map, &mut held, i, docs, &mut segs_n, &mut dat, &mut off, dat_len, off_len)?;
                return Ok(Outcome::Stopped);
            }
            let take = (|| -> Option<(String, String)> {
                let e = archive.entry(i).ok()?;
                let Kind::Content { mime, .. } = e.kind else { return None };
                if e.ns != ns || !archive.mime_of(mime).starts_with("text/html") {
                    return None;
                }
                let page = archive.read_index(i).ok()?;
                let text: String = html_to_text(&String::from_utf8_lossy(&page.data)).chars().take(cap).collect();
                Some((e.display_title().chars().take(MAX_TITLE_CHARS).collect(), text))
            })();
            if let Some((title, text)) = take {
                let id = docs;
                let tt = tokens(&title);
                let bt = tokens(&text);
                let mut local: HashMap<&str, (u32, bool)> = HashMap::new();
                for t in &tt {
                    let e = local.entry(t.as_str()).or_insert((0, true));
                    e.0 += 1;
                    e.1 = true;
                }
                for t in &bt {
                    local.entry(t.as_str()).or_insert((0, false)).0 += 1;
                }
                let mut rec = Vec::new();
                rec.extend_from_slice(&i.to_le_bytes());
                put(&mut rec, title.len() as u64);
                rec.extend_from_slice(title.as_bytes());
                put(&mut rec, (tt.len() + bt.len()) as u64);
                off.write_all(&dat_len.to_le_bytes())?;
                dat.write_all(&rec)?;
                dat_len += rec.len() as u64;
                off_len += 8;
                for (term, (tf, in_title)) in local {
                    map.entry(term.to_string()).or_default().push((id, ((tf.min(32_000) as u16) << 1) | in_title as u16));
                    held += 1;
                }
                docs += 1;
                if held >= budget {
                    checkpoint(&mut map, &mut held, i + 1, docs, &mut segs_n, &mut dat, &mut off, dat_len, off_len)?;
                }
            }
            i += 1;
            if i % 64 == 0 {
                progress.done.store(i as u64, Ordering::Relaxed);
            }
        }
        checkpoint(&mut map, &mut held, archive.articles, docs, &mut segs_n, &mut dat, &mut off, dat_len, off_len)?;
        next = archive.articles;
        progress.done.store(next as u64, Ordering::Relaxed);
        progress.phase.store(2, Ordering::Relaxed);
        let segs: Vec<Seg> = (1..=segs_n).map(|n| Seg { path: dir.join(format!("seg-{:05}.dat", n)) }).collect();
        let (terms, dict_len) = merge(dir, &segs, progress).inspect_err(|_| {
            for f in ["post.dat", "dict.dat", "sparse.dat"] {
                let _ = fs::remove_file(dir.join(f));
            }
        })?;
        let size = |f: &str| fs::metadata(dir.join(f)).map(|m| m.len()).unwrap_or(0);
        let meta = format!("format cfti 1\nfp {}\ndepth {}\ndocs {}\nterms {}\narticles {}\ndocdat {}\ndocoff {}\npost {}\ndict {}\nsparse {}\n", fp, depth.word(), docs, terms, archive.articles, size("docs.dat"), size("docs.off"), size("post.dat"), dict_len, size("sparse.dat"));
        write_atomic(&dir.join("meta.txt"), meta.as_bytes())?;
        for s in &segs {
            let _ = fs::remove_file(&s.path);
        }
        let _ = fs::remove_file(dir.join("progress.txt"));
        Ok(Outcome::Done)
    })();
    progress.phase.store(0, Ordering::Relaxed);
    match result {
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(Outcome::Stopped),
        r => r,
    }
}

// ---------- searching ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The article's entry number in the archive.
    pub entry: u32,
    pub title: String,
    pub score: f64,
}

struct Term {
    df: u64,
    off: u64,
    len: u64,
}

pub struct Index {
    docs: u32,
    depth: Depth,
    sparse: Vec<(String, u64)>,
    dict_len: u64,
    f_dat: Mutex<File>,
    f_off: Mutex<File>,
    f_post: Mutex<File>,
    f_dict: Mutex<File>,
}

fn read_at(f: &Mutex<File>, pos: u64, buf: &mut [u8]) -> io::Result<()> {
    let mut g = f.lock().unwrap_or_else(|e| e.into_inner());
    g.seek(SeekFrom::Start(pos))?;
    g.read_exact(buf)
}

impl Index {
    /// Open a finished index. Fails (rather than guessing) when it is missing, damaged, or made from another archive.
    pub fn open(dir: &Path, fp: &str) -> io::Result<Index> {
        let m = kv(&fs::read_to_string(dir.join("meta.txt"))?);
        if m.get("fp").map(|x| x.as_str()) != Some(fp) {
            return bad("the index was made from a different archive");
        }
        let num = |k: &str| m.get(k).and_then(|v| v.parse::<u64>().ok()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "the index description is damaged"));
        let (docs, post, dict, sparse_len) = (num("docs")?, num("post")?, num("dict")?, num("sparse")?);
        let depth = m.get("depth").and_then(|d| Depth::parse(d)).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "the index description is damaged"))?;
        let size = |f: &str| fs::metadata(dir.join(f)).map(|m| m.len()).unwrap_or(u64::MAX);
        if size("docs.dat") != num("docdat")? || size("docs.off") != num("docoff")? || size("post.dat") != post || size("dict.dat") != dict || size("sparse.dat") != sparse_len || size("docs.off") != 8 * docs {
            return bad("the index files are damaged or incomplete");
        }
        let raw = fs::read(dir.join("sparse.dat"))?;
        let (mut pos, mut sparse) = (0usize, Vec::new());
        while pos < raw.len() {
            let tl = get(&raw, &mut pos).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "damaged"))? as usize;
            if pos + tl + 8 > raw.len() {
                return bad("the index files are damaged");
            }
            let t = String::from_utf8_lossy(&raw[pos..pos + tl]).to_string();
            pos += tl;
            let mut o = [0u8; 8];
            o.copy_from_slice(&raw[pos..pos + 8]);
            pos += 8;
            sparse.push((t, u64::from_le_bytes(o)));
        }
        Ok(Index { docs: docs as u32, depth, sparse, dict_len: dict, f_dat: Mutex::new(File::open(dir.join("docs.dat"))?), f_off: Mutex::new(File::open(dir.join("docs.off"))?), f_post: Mutex::new(File::open(dir.join("post.dat"))?), f_dict: Mutex::new(File::open(dir.join("dict.dat"))?) })
    }

    pub fn docs(&self) -> u32 {
        self.docs
    }
    pub fn depth(&self) -> Depth {
        self.depth
    }

    fn block(&self, b: usize) -> io::Result<Vec<(String, Term)>> {
        let start = self.sparse[b].1;
        let end = self.sparse.get(b + 1).map(|s| s.1).unwrap_or(self.dict_len);
        if end < start || end > self.dict_len {
            return bad("the index files are damaged");
        }
        let mut raw = vec![0u8; (end - start) as usize];
        read_at(&self.f_dict, start, &mut raw)?;
        let (mut pos, mut out) = (0usize, Vec::new());
        while pos < raw.len() {
            let tl = get(&raw, &mut pos).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "damaged"))? as usize;
            if pos + tl > raw.len() {
                return bad("the index files are damaged");
            }
            let t = String::from_utf8_lossy(&raw[pos..pos + tl]).to_string();
            pos += tl;
            let (Some(df), Some(off), Some(len)) = (get(&raw, &mut pos), get(&raw, &mut pos), get(&raw, &mut pos)) else { return bad("the index files are damaged") };
            out.push((t, Term { df, off, len }));
        }
        Ok(out)
    }

    /// The block that could hold `term` (the last whose first term is not after it).
    fn block_for(&self, term: &str) -> usize {
        match self.sparse.binary_search_by(|(t, _)| t.as_str().cmp(term)) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        }
    }

    fn lookup(&self, term: &str, prefix: bool, max: usize) -> io::Result<Vec<(String, Term)>> {
        if self.sparse.is_empty() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        let mut b = self.block_for(term);
        while b < self.sparse.len() {
            for (t, e) in self.block(b)? {
                if t.as_str() < term {
                    continue;
                }
                let hit = if prefix { t.starts_with(term) } else { t == term };
                if hit {
                    out.push((t, e));
                    if out.len() >= max {
                        return Ok(out);
                    }
                } else if t.as_str() > term && !(prefix && t.starts_with(term)) {
                    return Ok(out);
                }
            }
            b += 1;
        }
        Ok(out)
    }

    fn postings(&self, t: &Term) -> io::Result<Vec<(u32, u16)>> {
        let mut raw = vec![0u8; t.len as usize];
        read_at(&self.f_post, t.off, &mut raw)?;
        let v = decode_postings(&raw);
        if v.len() as u64 != t.df {
            return bad("the index files are damaged");
        }
        Ok(v)
    }

    fn doc(&self, id: u32) -> io::Result<(u32, String, u64)> {
        if id >= self.docs {
            return bad("no such article in the index");
        }
        let mut o = [0u8; 8];
        read_at(&self.f_off, 8 * id as u64, &mut o)?;
        let pos = u64::from_le_bytes(o);
        let mut buf = vec![0u8; 4 + 10 + MAX_TITLE_CHARS * 4 + 10];
        let len = {
            let mut g = self.f_dat.lock().unwrap_or_else(|e| e.into_inner());
            g.seek(SeekFrom::Start(pos))?;
            let mut n = 0;
            while n < buf.len() {
                let r = g.read(&mut buf[n..])?;
                if r == 0 {
                    break;
                }
                n += r;
            }
            n
        };
        buf.truncate(len);
        if buf.len() < 5 {
            return bad("the index files are damaged");
        }
        let entry = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let mut p = 4usize;
        let tl = get(&buf, &mut p).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "damaged"))? as usize;
        if p + tl > buf.len() {
            return bad("the index files are damaged");
        }
        let title = String::from_utf8_lossy(&buf[p..p + tl]).to_string();
        p += tl;
        let words = get(&buf, &mut p).unwrap_or(0);
        Ok((entry, title, words))
    }

    /// Articles that contain every word of the query, best first. When too few contain every word, articles with most
    /// of them follow. The last word also matches words that start with it, so a search can run while it is typed.
    pub fn search(&self, query: &str, limit: usize) -> io::Result<Vec<Found>> {
        let mut words: Vec<String> = Vec::new();
        for t in tokens(query) {
            if !words.contains(&t) {
                words.push(t);
            }
        }
        words.truncate(8);
        if words.is_empty() || limit == 0 || self.docs == 0 {
            return Ok(vec![]);
        }
        let n = self.docs as f64;
        // one list per word: (article, score), sorted by article
        let mut groups: Vec<Vec<(u32, f64)>> = Vec::new();
        for (wi, w) in words.iter().enumerate() {
            let last = wi + 1 == words.len();
            let mut merged: HashMap<u32, f64> = HashMap::new();
            let mut found_exact = false;
            let mut terms = self.lookup(w, false, 1)?;
            found_exact |= !terms.is_empty();
            if last && w.chars().count() >= 3 {
                for (t, e) in self.lookup(w, true, 12)? {
                    if t != *w {
                        terms.push((t, e));
                    }
                }
            }
            for (t, e) in terms {
                let weight = if t == *w { 1.0 } else { 0.6 };
                let idf = (1.0 + n / e.df.max(1) as f64).ln();
                if words.len() > 1 && n >= 1000.0 && e.df as f64 > 0.3 * n && found_exact {
                    continue; // so common it says nothing next to the other words
                }
                for (d, t2) in self.postings(&e)? {
                    let tf = (t2 >> 1).max(1) as f64;
                    let s = idf * (1.0 + tf.ln()) * if t2 & 1 == 1 { 4.0 } else { 1.0 } * weight;
                    *merged.entry(d).or_insert(0.0) += s;
                }
            }
            if merged.is_empty() && words.len() == 1 {
                return Ok(vec![]);
            }
            let mut v: Vec<(u32, f64)> = merged.into_iter().collect();
            v.sort_by_key(|x| x.0);
            groups.push(v);
        }
        groups.retain(|g| !g.is_empty());
        if groups.is_empty() {
            return Ok(vec![]);
        }
        let need = groups.len();
        let mut score: HashMap<u32, (usize, f64)> = HashMap::new();
        // every word
        let smallest = (0..groups.len()).min_by_key(|i| groups[*i].len()).unwrap_or(0);
        for (d, s0) in &groups[smallest] {
            let mut total = *s0;
            let mut all = true;
            for (gi, g) in groups.iter().enumerate() {
                if gi == smallest {
                    continue;
                }
                match g.binary_search_by_key(d, |x| x.0) {
                    Ok(i) => total += g[i].1,
                    Err(_) => {
                        all = false;
                        break;
                    }
                }
            }
            if all {
                score.insert(*d, (need, total));
            }
        }
        // most of the words, only when every word gave too little
        if score.len() < limit && need >= 2 && groups.iter().map(|g| g.len()).sum::<usize>() < 3_000_000 {
            for g in &groups {
                for (d, s) in g {
                    if !score.get(d).map(|x| x.0 == need).unwrap_or(false) {
                        let e = score.entry(*d).or_insert((0, 0.0));
                        e.0 += 1;
                        e.1 += s;
                    }
                }
            }
            score.retain(|_, v| v.0 >= need.div_ceil(2) || v.0 == need);
        }
        let mut cands: Vec<(u32, usize, f64)> = score.into_iter().map(|(d, (c, s))| (d, c, s)).collect();
        cands.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)).then(a.0.cmp(&b.0)));
        cands.truncate(limit * 4);
        let mut out = Vec::new();
        for (d, c, s) in cands {
            let (entry, title, words) = self.doc(d)?;
            // a long article does not win just by being long
            let norm = 1.0 / (1.0 + 0.15 * (1.0 + words as f64 / 200.0).ln());
            out.push((c, Found { entry, title, score: s * norm }));
        }
        out.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.score.partial_cmp(&a.1.score).unwrap_or(std::cmp::Ordering::Equal)).then(a.1.entry.cmp(&b.1.entry)));
        out.truncate(limit);
        Ok(out.into_iter().map(|x| x.1).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zim::ZimWriter;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-ft-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn page(body: &str) -> Vec<u8> {
        format!("<html><head><title>x</title><style>p{{color:red}}</style></head><body>{}</body></html>", body).into_bytes()
    }

    /// An archive of small articles: (title, text).
    fn make(dir: &Path, name: &str, arts: &[(&str, &str)]) -> PathBuf {
        let p = dir.join(name);
        let mut w = ZimWriter::create(&p).unwrap();
        for (i, (t, b)) in arts.iter().enumerate() {
            w.add(b'C', &format!("a{:03}", i), t, "text/html", page(&format!("<h1>{}</h1><p>{}</p>", t, b))).unwrap();
        }
        w.add(b'C', "logo.png", "logo", "image/png", vec![1, 2, 3, 4]).unwrap();
        if !arts.is_empty() {
            w.redirect(b'C', "old", "Old name", b'C', "a000");
            w.set_main_page("a000");
        }
        w.finish().unwrap();
        p
    }

    const ARTS: &[(&str, &str)] = &[
        ("Boiling water", "To purify river water bring it to a rolling boil for one minute. Boiled water kills germs."),
        ("Water filters", "A ceramic filter removes dirt and many germs. Replace the cartridge every year."),
        ("Tents", "Pitch your tent on flat ground away from dead trees. Tents need guy lines in wind."),
        ("Fire starting", "Dry tinder, kindling and fuel wood. A ferro rod throws sparks even when wet."),
        ("Salt", "Salt preserves meat. Salt curing needs cold weather."),
        ("First aid", "Press on a bleeding wound with clean cloth. Water can rinse dirt from a cut."),
    ];

    fn open(d: &Path, arts: &[(&str, &str)], depth: Depth, budget: usize) -> (Archive, PathBuf, String, Index) {
        let zim = make(d, "book.zim", arts);
        let a = Archive::open(&zim).unwrap();
        let fp = fingerprint(&zim).unwrap();
        let dir = index_dir_for(&d.join("idx"), &zim, &fp);
        assert_eq!(build(&a, &dir, &fp, depth, budget, &Progress::default()).unwrap(), Outcome::Done);
        let ix = Index::open(&dir, &fp).unwrap();
        (a, dir, fp, ix)
    }

    fn titles(ix: &Index, q: &str) -> Vec<String> {
        ix.search(q, 10).unwrap().into_iter().map(|f| f.title).collect()
    }

    #[test]
    fn words_are_lower_cased_stemmed_and_common_words_dropped() {
        assert_eq!(tokens("The Boiling of Tents, boiled & BOILS!"), vec!["boil", "of", "tent", "boil", "boil"]);
        assert_eq!(tokens("the and for"), Vec::<String>::new());
        assert_eq!(tokens("Straße-Café 2026"), vec!["straße", "café", "2026"]);
        assert_eq!(tokens("batteries"), vec!["battery"]);
        assert_eq!(tokens("a x"), Vec::<String>::new(), "one-letter words say nothing");
        assert_eq!(tokens("speed need"), vec!["speed", "need"], "a short word is not cut down to nothing");
    }

    #[test]
    fn numbers_survive_a_round_trip() {
        for n in [0u64, 1, 127, 128, 300, 16_384, u32::MAX as u64, u64::MAX >> 1] {
            let mut v = Vec::new();
            put(&mut v, n);
            let mut p = 0;
            assert_eq!(get(&v, &mut p), Some(n));
            assert_eq!(read_varint(&mut &v[..]).unwrap(), Some(n));
        }
        assert_eq!(get(&[0x80], &mut 0), None, "a cut-short number is refused");
        let list = vec![(3u32, 5u16), (9, 2), (1000, 65_000)];
        assert_eq!(decode_postings(&encode_postings(&list)), list);
    }

    #[test]
    fn an_article_is_found_by_a_word_in_its_text_and_titles_count_for_more() {
        let d = tmp("find");
        let (_a, _dir, _fp, ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        assert_eq!(ix.docs(), 6, "the picture and the redirect are not articles");
        assert_eq!(titles(&ix, "germs").len(), 2, "two articles mention germs");
        let w = titles(&ix, "water");
        assert_eq!(w[0], "Boiling water", "a title word and many mentions come first: {:?}", w);
        assert!(w.contains(&"First aid".to_string()), "a word only in the text is found: {:?}", w);
        assert!(titles(&ix, "ferro rod").contains(&"Fire starting".to_string()));
        assert!(titles(&ix, "zebra").is_empty());
        assert!(titles(&ix, "the and").is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn every_word_must_match_and_a_partly_matching_article_comes_after() {
        let d = tmp("and");
        let (_a, _dir, _fp, ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        let r = titles(&ix, "water germs");
        assert_eq!(r[0], "Boiling water", "{:?}", r);
        assert!(r.contains(&"Water filters".to_string()));
        let r = titles(&ix, "tent salt");
        assert!(r.contains(&"Tents".to_string()) && r.contains(&"Salt".to_string()), "each matches one of two words: {:?}", r);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_last_word_matches_a_start_so_a_search_can_run_while_typing() {
        let d = tmp("prefix");
        let (_a, _dir, _fp, ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        assert!(titles(&ix, "purif").contains(&"Boiling water".to_string()));
        assert!(titles(&ix, "river purif").contains(&"Boiling water".to_string()));
        assert!(titles(&ix, "purif river").is_empty() || !titles(&ix, "purif river").contains(&"Salt".to_string()));
        assert!(titles(&ix, "pu").is_empty(), "two letters is too short to guess a start");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn plurals_and_endings_find_each_other() {
        let d = tmp("stem");
        let (_a, _dir, _fp, ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        assert!(titles(&ix, "boiled").contains(&"Boiling water".to_string()));
        assert!(titles(&ix, "tent").contains(&"Tents".to_string()));
        assert!(titles(&ix, "filters").contains(&"Water filters".to_string()));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn many_small_pieces_give_the_same_answers_as_one_big_piece() {
        let d = tmp("pieces");
        let (_a, _dir, _fp, big) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        let d2 = tmp("pieces2");
        let (_b, dir2, _fp2, small) = open(&d2, ARTS, Depth::Full, 12);
        assert!(fs::read_dir(&dir2).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with("seg-")), "the pieces are cleaned up");
        for q in ["water", "water germs", "purif", "tent salt", "wound cloth", "ferro rod"] {
            let a: Vec<(u32, String)> = big.search(q, 10).unwrap().into_iter().map(|f| (f.entry, f.title)).collect();
            let b: Vec<(u32, String)> = small.search(q, 10).unwrap().into_iter().map(|f| (f.entry, f.title)).collect();
            assert_eq!(a, b, "{}", q);
        }
        assert_eq!(big.docs(), small.docs());
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&d2);
    }

    #[test]
    fn a_stopped_build_carries_on_and_ends_the_same_as_an_unbroken_one() {
        let d = tmp("resume");
        let zim = make(&d, "book.zim", ARTS);
        let a = Archive::open(&zim).unwrap();
        let fp = fingerprint(&zim).unwrap();
        let dir = index_dir_for(&d.join("idx"), &zim, &fp);
        assert_eq!(status(&dir, &fp), Status::NotBuilt);
        let p = Progress::default();
        assert_eq!(build_inner(&a, &dir, &fp, Depth::Full, 10, &p, Some(3)).unwrap(), Outcome::Stopped);
        match status(&dir, &fp) {
            Status::Partial { done, total } => assert_eq!((done, total), (3, a.articles)),
            s => panic!("{:?}", s),
        }
        assert!(Index::open(&dir, &fp).is_err(), "a half-made index is not an index");
        assert_eq!(build_inner(&a, &dir, &fp, Depth::Full, 10, &p, Some(5)).unwrap(), Outcome::Stopped);
        assert_eq!(build(&a, &dir, &fp, Depth::Full, 10, &p).unwrap(), Outcome::Done);
        let resumed = Index::open(&dir, &fp).unwrap();
        let d2 = tmp("resume-whole");
        let (_b, _dir2, _fp2, whole) = open(&d2, ARTS, Depth::Full, DEFAULT_BUDGET);
        assert_eq!(resumed.docs(), whole.docs());
        for q in ["water", "water germs", "tent", "fire"] {
            let x: Vec<u32> = resumed.search(q, 10).unwrap().into_iter().map(|f| f.entry).collect();
            let y: Vec<u32> = whole.search(q, 10).unwrap().into_iter().map(|f| f.entry).collect();
            assert_eq!(x, y, "{}", q);
        }
        assert!(matches!(status(&dir, &fp), Status::Ready { docs: 6, .. }));
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&d2);
    }

    #[test]
    fn asking_a_running_build_to_stop_saves_where_it_got_to() {
        let d = tmp("stopflag");
        let zim = make(&d, "book.zim", ARTS);
        let a = Archive::open(&zim).unwrap();
        let fp = fingerprint(&zim).unwrap();
        let dir = index_dir_for(&d.join("idx"), &zim, &fp);
        let p = Progress::default();
        p.stop.store(true, Ordering::Relaxed);
        assert_eq!(build(&a, &dir, &fp, Depth::Lead, DEFAULT_BUDGET, &p).unwrap(), Outcome::Stopped);
        assert_eq!(p.phase.load(Ordering::Relaxed), 0);
        p.stop.store(false, Ordering::Relaxed);
        assert_eq!(build(&a, &dir, &fp, Depth::Lead, DEFAULT_BUDGET, &p).unwrap(), Outcome::Done);
        assert_eq!(p.done.load(Ordering::Relaxed), a.articles as u64);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_lead_depth_reads_only_the_opening_and_the_full_depth_reads_on() {
        let d = tmp("depth");
        let long = format!("{} zyzzyva", "filler words here ".repeat(600));
        let arts = [("Long page", long.as_str()), ("Short", "zyzzyva too")];
        let (_a, _dir, _fp, lead) = open(&d, &arts, Depth::Lead, DEFAULT_BUDGET);
        assert_eq!(titles(&lead, "zyzzyva"), vec!["Short".to_string()]);
        let d2 = tmp("depth2");
        let (_b, _dir2, _fp2, full) = open(&d2, &arts, Depth::Full, DEFAULT_BUDGET);
        assert_eq!(titles(&full, "zyzzyva").len(), 2);
        assert_eq!(lead.depth(), Depth::Lead);
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&d2);
    }

    #[test]
    fn an_index_made_from_another_archive_is_out_of_date_and_not_used() {
        let d = tmp("stale");
        let (_a, dir, fp, _ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        assert!(matches!(status(&dir, &fp), Status::Ready { .. }));
        assert_eq!(status(&dir, "other-fingerprint"), Status::OutOfDate);
        assert!(Index::open(&dir, "other-fingerprint").is_err());
        // a different archive file has a different fingerprint
        let other = make(&d, "other.zim", &ARTS[..3]);
        assert_ne!(fingerprint(&other).unwrap(), fp);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_damaged_index_is_refused_or_answers_nothing_and_never_crashes() {
        let d = tmp("damage");
        let (_a, dir, fp, _ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        let dict = fs::read(dir.join("dict.dat")).unwrap();
        fs::write(dir.join("dict.dat"), &dict[..dict.len() / 2]).unwrap();
        assert!(Index::open(&dir, &fp).is_err(), "a cut-short file is noticed when it is opened");
        fs::write(dir.join("dict.dat"), &dict).unwrap();
        let mut bytes = fs::read(dir.join("post.dat")).unwrap();
        for b in bytes.iter_mut() {
            *b = 0xff;
        }
        fs::write(dir.join("post.dat"), &bytes).unwrap();
        let ix = Index::open(&dir, &fp).unwrap();
        assert!(ix.search("water", 5).is_err() || ix.search("water", 5).unwrap().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn deleting_removes_the_folder_and_a_missing_one_is_fine() {
        let d = tmp("delete");
        let (_a, dir, fp, ix) = open(&d, ARTS, Depth::Full, DEFAULT_BUDGET);
        drop(ix);
        delete(&dir).unwrap();
        assert!(!dir.exists());
        assert_eq!(status(&dir, &fp), Status::NotBuilt);
        delete(&dir).unwrap();
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_empty_archive_and_odd_queries_are_safe() {
        let d = tmp("empty");
        let (_a, _dir, _fp, ix) = open(&d, &[], Depth::Full, DEFAULT_BUDGET);
        assert_eq!(ix.docs(), 0);
        assert!(ix.search("water", 5).unwrap().is_empty());
        let d2 = tmp("odd");
        let (_b, _dir2, _fp2, ix2) = open(&d2, ARTS, Depth::Full, DEFAULT_BUDGET);
        assert!(ix2.search("", 5).unwrap().is_empty());
        assert!(ix2.search("   !!! ", 5).unwrap().is_empty());
        assert!(ix2.search("water", 0).unwrap().is_empty());
        assert!(ix2.search(&"water ".repeat(100), 5).unwrap().len() <= 5);
        let _ = fs::remove_dir_all(&d);
        let _ = fs::remove_dir_all(&d2);
    }
}
