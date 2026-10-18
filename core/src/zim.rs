//! Writing (and, for checking, reading) ZIM files: the archive format Kiwix uses.
//!
//! The writer makes version 6.1 files (the "C", "M" and "W" namespaces) with uncompressed
//! clusters. Uncompressed is a legal ZIM cluster type that every Kiwix reader understands; it
//! makes the file bigger than a compressed one but keeps this code free of outside libraries.
//! The writer does not build the full-text search index; the app keeps its own text copy of
//! saved sites for the assistant (see `crawl`).
//!
//! Layout written: header, MIME list, URL pointers, title pointers, cluster pointers,
//! directory entries, clusters, then the 16-byte MD5 checksum of everything before it.

use crate::md5::Md5;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const MAGIC: u32 = 72173914;
const HEADER_LEN: u64 = 80;
const NO_PAGE: u32 = 0xffff_ffff;
const REDIRECT: u16 = 0xffff;
/// A cluster is closed once it holds this much.
const CLUSTER_TARGET: usize = 2 * 1024 * 1024;
/// One saved file bigger than this is refused (cluster offsets are 32-bit).
pub const MAX_BLOB: usize = 3_500_000_000;

enum Kind {
    Content { mime: u16, cluster: u32, blob: u32 },
    Redirect { to_ns: u8, to_url: String },
}

struct Entry {
    ns: u8,
    url: String,
    title: String,
    kind: Kind,
}

pub struct Stats {
    pub entries: u32,
    pub clusters: u32,
    pub bytes: u64,
}

pub struct ZimWriter {
    path: PathBuf,
    tmp_path: PathBuf,
    clusters: BufWriter<File>,
    cluster_ends: Vec<u64>,
    written: u64,
    cur: Vec<Vec<u8>>,
    cur_size: usize,
    entries: Vec<Entry>,
    seen: HashSet<(u8, String)>,
    mimes: Vec<String>,
    mime_ids: HashMap<String, u16>,
    cluster_count: u32,
    main_page: Option<String>,
}

fn io_err(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.to_string())
}

impl ZimWriter {
    pub fn create(path: &Path) -> io::Result<ZimWriter> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp_path = path.with_extension("zim.clusters.tmp");
        Ok(ZimWriter {
            path: path.to_path_buf(),
            clusters: BufWriter::new(File::create(&tmp_path)?),
            tmp_path,
            cluster_ends: Vec::new(),
            written: 0,
            cur: Vec::new(),
            cur_size: 0,
            entries: Vec::new(),
            seen: HashSet::new(),
            mimes: Vec::new(),
            mime_ids: HashMap::new(),
            cluster_count: 0,
            main_page: None,
        })
    }

    fn mime_id(&mut self, m: &str) -> u16 {
        if let Some(i) = self.mime_ids.get(m) {
            return *i;
        }
        let id = self.mimes.len() as u16;
        self.mimes.push(m.to_string());
        self.mime_ids.insert(m.to_string(), id);
        id
    }

    fn flush_cluster(&mut self) -> io::Result<()> {
        if self.cur.is_empty() {
            return Ok(());
        }
        let n = self.cur.len();
        let table = (n + 1) * 4;
        let total: usize = table + self.cur.iter().map(|b| b.len()).sum::<usize>();
        if total > u32::MAX as usize {
            return Err(io_err("cluster too large"));
        }
        self.clusters.write_all(&[1u8])?; // uncompressed, 32-bit offsets
        let mut off = table as u32;
        self.clusters.write_all(&off.to_le_bytes())?;
        for b in &self.cur {
            off += b.len() as u32;
            self.clusters.write_all(&off.to_le_bytes())?;
        }
        for b in &self.cur {
            self.clusters.write_all(b)?;
        }
        self.written += 1 + total as u64;
        self.cluster_ends.push(self.written);
        self.cur.clear();
        self.cur_size = 0;
        self.cluster_count += 1;
        Ok(())
    }

    /// Add a file. Returns false (and does nothing) when that address already exists.
    pub fn add(&mut self, ns: u8, url: &str, title: &str, mime: &str, data: Vec<u8>) -> io::Result<bool> {
        if data.len() > MAX_BLOB {
            return Err(io_err("file too large for a ZIM archive"));
        }
        if !self.seen.insert((ns, url.to_string())) {
            return Ok(false);
        }
        if !self.cur.is_empty() && self.cur_size + data.len() > CLUSTER_TARGET {
            self.flush_cluster()?;
        }
        let mime = self.mime_id(mime);
        let kind = Kind::Content { mime, cluster: self.cluster_count, blob: self.cur.len() as u32 };
        self.cur_size += data.len();
        self.cur.push(data);
        self.entries.push(Entry { ns, url: url.to_string(), title: title.to_string(), kind });
        Ok(true)
    }

    /// Add a redirect: `ns/url` leads to `to_ns/to_url`, which must exist by the time of `finish`.
    pub fn redirect(&mut self, ns: u8, url: &str, title: &str, to_ns: u8, to_url: &str) -> bool {
        if !self.seen.insert((ns, url.to_string())) {
            return false;
        }
        self.entries.push(Entry {
            ns,
            url: url.to_string(),
            title: title.to_string(),
            kind: Kind::Redirect { to_ns, to_url: to_url.to_string() },
        });
        true
    }

    /// A line of archive information (Title, Language, Description, Creator, Publisher, Date, Name, ...).
    pub fn metadata(&mut self, key: &str, value: &str) -> io::Result<()> {
        self.add(b'M', key, "", "text/plain;charset=utf-8", value.as_bytes().to_vec()).map(|_| ())
    }

    /// The page the archive opens on, given by its address in the content ("C") namespace.
    pub fn set_main_page(&mut self, url: &str) {
        self.main_page = Some(url.to_string());
    }

    pub fn finish(mut self) -> io::Result<Stats> {
        self.flush_cluster()?;
        self.clusters.flush()?;
        let mut main_idx_url: Option<String> = None;
        if let Some(m) = self.main_page.clone() {
            if self.seen.contains(&(b'C', m.clone())) {
                self.redirect(b'W', "mainPage", "", b'C', &m);
                main_idx_url = Some(m);
            }
        }

        // Order by address.
        let mut entries = std::mem::take(&mut self.entries);
        entries.sort_by(|a, b| (a.ns, a.url.as_bytes()).cmp(&(b.ns, b.url.as_bytes())));
        let n = entries.len();
        let index: HashMap<(u8, &str), u32> =
            entries.iter().enumerate().map(|(i, e)| ((e.ns, e.url.as_str()), i as u32)).collect();

        // Title order.
        let mut by_title: Vec<u32> = (0..n as u32).collect();
        let tkey = |e: &Entry| -> (u8, Vec<u8>) {
            (e.ns, if e.title.is_empty() { e.url.as_bytes().to_vec() } else { e.title.as_bytes().to_vec() })
        };
        by_title.sort_by(|a, b| tkey(&entries[*a as usize]).cmp(&tkey(&entries[*b as usize])));

        // Sizes and positions.
        let mime_len: u64 = self.mimes.iter().map(|m| m.len() as u64 + 1).sum::<u64>() + 1;
        let url_ptr_pos = HEADER_LEN + mime_len;
        let title_ptr_pos = url_ptr_pos + 8 * n as u64;
        let cluster_ptr_pos = title_ptr_pos + 4 * n as u64;
        let entries_pos = cluster_ptr_pos + 8 * self.cluster_count as u64;
        let mut entry_offsets = Vec::with_capacity(n);
        let mut pos = entries_pos;
        for e in &entries {
            entry_offsets.push(pos);
            let t = if e.title == e.url { 0 } else { e.title.len() };
            let fixed = match e.kind {
                Kind::Content { .. } => 16,
                Kind::Redirect { .. } => 12,
            };
            pos += fixed + e.url.len() as u64 + 1 + t as u64 + 1;
        }
        let clusters_pos = pos;
        let checksum_pos = clusters_pos + self.written;

        // Write the file.
        let out = File::create(&self.path)?;
        let mut w = Hashed { inner: BufWriter::new(out), md5: Md5::new(), pos: 0 };
        let main_index = match &main_idx_url {
            Some(u) => *index.get(&(b'C', u.as_str())).unwrap_or(&NO_PAGE),
            None => NO_PAGE,
        };
        w.put(&MAGIC.to_le_bytes())?;
        w.put(&6u16.to_le_bytes())?;
        w.put(&1u16.to_le_bytes())?;
        w.put(&make_uuid(n as u64, self.written))?;
        w.put(&(n as u32).to_le_bytes())?;
        w.put(&self.cluster_count.to_le_bytes())?;
        w.put(&url_ptr_pos.to_le_bytes())?;
        w.put(&title_ptr_pos.to_le_bytes())?;
        w.put(&cluster_ptr_pos.to_le_bytes())?;
        w.put(&HEADER_LEN.to_le_bytes())?;
        w.put(&main_index.to_le_bytes())?;
        w.put(&NO_PAGE.to_le_bytes())?;
        w.put(&checksum_pos.to_le_bytes())?;
        debug_assert_eq!(w.pos, HEADER_LEN);

        for m in &self.mimes {
            w.put(m.as_bytes())?;
            w.put(&[0])?;
        }
        w.put(&[0])?;
        for off in &entry_offsets {
            w.put(&off.to_le_bytes())?;
        }
        for t in &by_title {
            w.put(&t.to_le_bytes())?;
        }
        let mut start = clusters_pos;
        let mut prev = 0u64;
        for end in &self.cluster_ends {
            w.put(&start.to_le_bytes())?;
            start += end - prev;
            prev = *end;
        }
        for e in &entries {
            match &e.kind {
                Kind::Content { mime, cluster, blob } => {
                    w.put(&mime.to_le_bytes())?;
                    w.put(&[0, e.ns])?;
                    w.put(&0u32.to_le_bytes())?;
                    w.put(&cluster.to_le_bytes())?;
                    w.put(&blob.to_le_bytes())?;
                }
                Kind::Redirect { to_ns, to_url } => {
                    let target = *index
                        .get(&(*to_ns, to_url.as_str()))
                        .ok_or_else(|| io_err(&format!("redirect target missing: {}", to_url)))?;
                    w.put(&REDIRECT.to_le_bytes())?;
                    w.put(&[0, e.ns])?;
                    w.put(&0u32.to_le_bytes())?;
                    w.put(&target.to_le_bytes())?;
                }
            }
            w.put(e.url.as_bytes())?;
            w.put(&[0])?;
            if e.title != e.url {
                w.put(e.title.as_bytes())?;
            }
            w.put(&[0])?;
        }
        debug_assert_eq!(w.pos, clusters_pos);
        let mut tmp = File::open(&self.tmp_path)?;
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let k = tmp.read(&mut buf)?;
            if k == 0 {
                break;
            }
            w.put(&buf[..k])?;
        }
        debug_assert_eq!(w.pos, checksum_pos);
        let digest = w.md5.finish();
        w.inner.write_all(&digest)?;
        w.inner.flush()?;
        drop(tmp);
        let _ = std::fs::remove_file(&self.tmp_path);
        Ok(Stats { entries: n as u32, clusters: self.cluster_count, bytes: checksum_pos + 16 })
    }

    /// Give up: remove the partial files.
    pub fn abandon(self) {
        let tmp = self.tmp_path.clone();
        drop(self);
        let _ = std::fs::remove_file(tmp);
    }
}

struct Hashed<W: Write> {
    inner: W,
    md5: Md5,
    pos: u64,
}

impl<W: Write> Hashed<W> {
    fn put(&mut self, b: &[u8]) -> io::Result<()> {
        self.md5.update(b);
        self.pos += b.len() as u64;
        self.inner.write_all(b)
    }
}

fn make_uuid(a: u64, b: u64) -> [u8; 16] {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut m = Md5::new();
    m.update(&nanos.to_le_bytes());
    m.update(&a.to_le_bytes());
    m.update(&b.to_le_bytes());
    m.update(&std::process::id().to_le_bytes());
    m.finish()
}

// ---------- reading (uncompressed clusters only; used for checks and for the archive list) ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub major: u16,
    pub minor: u16,
    pub articles: u32,
    pub clusters: u32,
    pub url_ptr_pos: u64,
    pub title_ptr_pos: u64,
    pub cluster_ptr_pos: u64,
    pub mime_pos: u64,
    pub main_page: u32,
    pub checksum_pos: u64,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn u64_at(b: &[u8], i: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(a)
}

/// Read just the header; fails if this is not a ZIM file. Works for every kind of ZIM file.
pub fn read_header(path: &Path) -> io::Result<Header> {
    let mut f = File::open(path)?;
    let mut b = [0u8; 80];
    f.read_exact(&mut b).map_err(|_| io_err("too short to be a ZIM file"))?;
    if u32_at(&b, 0) != MAGIC {
        return Err(io_err("not a ZIM file"));
    }
    Ok(Header {
        major: u16_at(&b, 4),
        minor: u16_at(&b, 6),
        articles: u32_at(&b, 24),
        clusters: u32_at(&b, 28),
        url_ptr_pos: u64_at(&b, 32),
        title_ptr_pos: u64_at(&b, 40),
        cluster_ptr_pos: u64_at(&b, 48),
        mime_pos: u64_at(&b, 56),
        main_page: u32_at(&b, 64),
        checksum_pos: u64_at(&b, 72),
    })
}

pub struct Reader {
    f: File,
    pub header: Header,
    mimes: Vec<String>,
}

pub struct Item {
    pub mime: String,
    pub data: Vec<u8>,
}

impl Reader {
    pub fn open(path: &Path) -> io::Result<Reader> {
        let header = read_header(path)?;
        let mut f = File::open(path)?;
        f.seek(SeekFrom::Start(header.mime_pos))?;
        let mut raw = vec![0u8; (header.url_ptr_pos - header.mime_pos) as usize];
        f.read_exact(&mut raw)?;
        let mimes: Vec<String> = raw
            .split(|b| *b == 0)
            .take_while(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        Ok(Reader { f, header, mimes })
    }

    fn entry_offset(&mut self, i: u32) -> io::Result<u64> {
        let mut b = [0u8; 8];
        self.f.seek(SeekFrom::Start(self.header.url_ptr_pos + 8 * i as u64))?;
        self.f.read_exact(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    /// (namespace, url, title, redirect target index or content location) of entry `i`.
    fn entry(&mut self, i: u32) -> io::Result<(u8, String, RawKind)> {
        let off = self.entry_offset(i)?;
        self.f.seek(SeekFrom::Start(off))?;
        let mut buf = vec![0u8; 2048];
        let k = self.f.read(&mut buf)?;
        buf.truncate(k);
        let mime = u16_at(&buf, 0);
        let ns = buf[3];
        let (kind, rest) = if mime == REDIRECT {
            (RawKind::Redirect(u32_at(&buf, 8)), 12)
        } else {
            (RawKind::Content { mime, cluster: u32_at(&buf, 8), blob: u32_at(&buf, 12) }, 16)
        };
        let end = buf[rest..].iter().position(|b| *b == 0).ok_or_else(|| io_err("bad entry"))?;
        let url = String::from_utf8_lossy(&buf[rest..rest + end]).to_string();
        Ok((ns, url, kind))
    }

    /// Look an address up (binary search over the URL list); follows redirects.
    pub fn get(&mut self, ns: u8, url: &str) -> io::Result<Option<Item>> {
        let (mut lo, mut hi) = (0u32, self.header.articles);
        let mut found = None;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (n, u, _) = self.entry(mid)?;
            match (n, u.as_bytes()).cmp(&(ns, url.as_bytes())) {
                std::cmp::Ordering::Equal => {
                    found = Some(mid);
                    break;
                }
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        let Some(mut i) = found else { return Ok(None) };
        for _ in 0..8 {
            let (_, _, kind) = self.entry(i)?;
            match kind {
                RawKind::Redirect(t) => i = t,
                RawKind::Content { mime, cluster, blob } => {
                    let data = self.blob(cluster, blob)?;
                    let mime = self.mimes.get(mime as usize).cloned().unwrap_or_default();
                    return Ok(Some(Item { mime, data }));
                }
            }
        }
        Err(io_err("redirect loop"))
    }

    fn blob(&mut self, cluster: u32, blob: u32) -> io::Result<Vec<u8>> {
        let mut b8 = [0u8; 8];
        self.f.seek(SeekFrom::Start(self.header.cluster_ptr_pos + 8 * cluster as u64))?;
        self.f.read_exact(&mut b8)?;
        let start = u64::from_le_bytes(b8);
        self.f.seek(SeekFrom::Start(start))?;
        let mut info = [0u8; 1];
        self.f.read_exact(&mut info)?;
        if info[0] & 0x0f != 1 {
            return Err(io_err("compressed cluster (this reader only handles uncompressed)"));
        }
        let wide = info[0] & 0x10 != 0;
        let width = if wide { 8 } else { 4 };
        let mut pair = vec![0u8; width * 2];
        self.f.seek(SeekFrom::Start(start + 1 + (blob as u64) * width as u64))?;
        self.f.read_exact(&mut pair)?;
        let (a, b) = if wide { (u64_at(&pair, 0), u64_at(&pair, 8)) } else { (u32_at(&pair, 0) as u64, u32_at(&pair, 4) as u64) };
        self.f.seek(SeekFrom::Start(start + 1 + a))?;
        let mut data = vec![0u8; (b - a) as usize];
        self.f.read_exact(&mut data)?;
        Ok(data)
    }
}

enum RawKind {
    Redirect(u32),
    Content { mime: u16, cluster: u32, blob: u32 },
}

/// Does the file's trailing checksum match its contents? (Reads the whole file.)
pub fn verify_checksum(path: &Path) -> io::Result<bool> {
    let h = read_header(path)?;
    let mut f = File::open(path)?;
    let mut md5 = Md5::new();
    let mut left = h.checksum_pos;
    let mut buf = vec![0u8; 1 << 20];
    while left > 0 {
        let want = (buf.len() as u64).min(left) as usize;
        f.read_exact(&mut buf[..want])?;
        md5.update(&buf[..want]);
        left -= want as u64;
    }
    let mut stored = [0u8; 16];
    f.read_exact(&mut stored)?;
    Ok(md5.finish() == stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-zim-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn writes_and_reads_back() {
        let d = tmp("rt");
        let p = d.join("t.zim");
        let mut w = ZimWriter::create(&p).unwrap();
        w.add(b'C', "example.com/index.html", "Home", "text/html", b"<h1>Hello</h1>".to_vec()).unwrap();
        w.add(b'C', "example.com/a.css", "", "text/css", b"body{}".to_vec()).unwrap();
        w.add(b'C', "example.com/logo.png", "", "image/png", vec![0x89, b'P', b'N', b'G', 1, 2, 3]).unwrap();
        w.metadata("Title", "Test").unwrap();
        w.metadata("Language", "eng").unwrap();
        w.set_main_page("example.com/index.html");
        let st = w.finish().unwrap();
        assert_eq!(st.entries, 6); // 3 content + 2 metadata + main page redirect

        let h = read_header(&p).unwrap();
        assert_eq!((h.major, h.minor), (6, 1));
        assert_eq!(h.articles, 6);
        assert!(verify_checksum(&p).unwrap());
        assert_eq!(std::fs::metadata(&p).unwrap().len(), st.bytes);

        let mut r = Reader::open(&p).unwrap();
        let it = r.get(b'C', "example.com/index.html").unwrap().unwrap();
        assert_eq!(it.mime, "text/html");
        assert_eq!(it.data, b"<h1>Hello</h1>");
        let it = r.get(b'C', "example.com/logo.png").unwrap().unwrap();
        assert_eq!(it.data, vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
        assert_eq!(r.get(b'M', "Title").unwrap().unwrap().data, b"Test");
        // the main page redirect leads to the home page
        assert_eq!(r.get(b'W', "mainPage").unwrap().unwrap().data, b"<h1>Hello</h1>");
        assert!(r.get(b'C', "example.com/missing").unwrap().is_none());
    }

    #[test]
    fn many_files_across_clusters() {
        let d = tmp("many");
        let p = d.join("m.zim");
        let mut w = ZimWriter::create(&p).unwrap();
        for i in 0..300u32 {
            let body = vec![(i % 251) as u8; 20_000 + i as usize];
            assert!(w.add(b'C', &format!("s/page{:03}.html", i), &format!("Page {}", i), "text/html", body).unwrap());
        }
        // a duplicate address is ignored
        assert!(!w.add(b'C', "s/page001.html", "dup", "text/html", vec![1]).unwrap());
        let st = w.finish().unwrap();
        assert_eq!(st.entries, 300);
        assert!(st.clusters > 1);
        assert!(verify_checksum(&p).unwrap());
        let mut r = Reader::open(&p).unwrap();
        for i in [0u32, 1, 137, 299] {
            let it = r.get(b'C', &format!("s/page{:03}.html", i)).unwrap().unwrap();
            assert_eq!(it.data.len(), 20_000 + i as usize);
            assert!(it.data.iter().all(|b| *b == (i % 251) as u8));
        }
    }

    #[test]
    fn a_big_file_gets_its_own_cluster() {
        let d = tmp("big");
        let p = d.join("b.zim");
        let mut w = ZimWriter::create(&p).unwrap();
        w.add(b'C', "a", "", "text/plain", b"small".to_vec()).unwrap();
        w.add(b'C', "b", "", "application/octet-stream", vec![7u8; 3 * 1024 * 1024]).unwrap();
        w.add(b'C', "c", "", "text/plain", b"after".to_vec()).unwrap();
        w.finish().unwrap();
        let mut r = Reader::open(&p).unwrap();
        assert_eq!(r.get(b'C', "b").unwrap().unwrap().data.len(), 3 * 1024 * 1024);
        assert_eq!(r.get(b'C', "c").unwrap().unwrap().data, b"after");
    }

    #[test]
    fn rejects_other_files_and_missing_redirect_targets() {
        let d = tmp("bad");
        let junk = d.join("junk.zim");
        std::fs::write(&junk, vec![0u8; 200]).unwrap();
        assert!(read_header(&junk).is_err());
        let p = d.join("r.zim");
        let mut w = ZimWriter::create(&p).unwrap();
        w.redirect(b'C', "x", "", b'C', "nowhere");
        assert!(w.finish().is_err());
    }

    #[test]
    fn a_damaged_file_fails_the_checksum() {
        let d = tmp("sum");
        let p = d.join("s.zim");
        let mut w = ZimWriter::create(&p).unwrap();
        w.add(b'C', "a", "", "text/plain", b"hello world".to_vec()).unwrap();
        w.finish().unwrap();
        let mut bytes = std::fs::read(&p).unwrap();
        let n = bytes.len();
        bytes[n - 20] ^= 0xff;
        std::fs::write(&p, bytes).unwrap();
        assert!(!verify_checksum(&p).unwrap());
    }
}
