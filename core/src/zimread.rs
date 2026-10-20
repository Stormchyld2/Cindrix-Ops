//! Reading ZIM archives directly, without Kiwix's program: look up an article by address,
//! follow redirects, find articles by the start of their title, and read the archive's own
//! description. Clusters may be stored, Zstandard (every Kiwix archive since 2021) or XZ (older
//! ones); see `zstd` and `xz`. Full-text search needs Kiwix's search index and is not here.
//!
//! One `Archive` can be shared between threads. The most recently used clusters are kept
//! decompressed in memory (a few megabytes), so reading several articles of one cluster is fast.

use crate::{xz, zstd};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Mutex};

const MAGIC: u32 = 72173914;
const REDIRECT: u16 = 0xffff;
const NONE: u32 = 0xffff_ffff;
/// A cluster that decompresses to more than this is refused.
const MAX_CLUSTER: usize = 192 * 1024 * 1024;
/// Decompressed clusters kept for reuse, in total bytes (at least one is always kept).
const CACHE_BYTES: usize = 48 * 1024 * 1024;

fn bad<T>(m: &str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, m.to_string()))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Redirect(u32),
    Content { mime: u16, cluster: u32, blob: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub ns: u8,
    pub url: String,
    /// Empty when the title is the same as the address.
    pub title: String,
    pub kind: Kind,
}

impl Entry {
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() { &self.url } else { &self.title }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub mime: String,
    pub data: Vec<u8>,
    /// The address (namespace byte and path) it was finally read from, after redirects.
    pub ns: u8,
    pub url: String,
}

struct Inner {
    f: File,
    cache: VecDeque<(u32, Arc<Vec<u8>>)>,
}

pub struct Archive {
    inner: Mutex<Inner>,
    pub major: u16,
    pub minor: u16,
    pub articles: u32,
    pub clusters: u32,
    url_ptr: u64,
    title_ptr: u64,
    cluster_ptr: u64,
    checksum_pos: u64,
    main_page: u32,
    mimes: Vec<String>,
    len: u64,
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

impl Archive {
    pub fn open(path: &Path) -> io::Result<Archive> {
        let mut f = File::open(path)?;
        let len = f.metadata()?.len();
        let mut h = [0u8; 80];
        f.read_exact(&mut h).or_else(|_| bad("too short to be a ZIM file"))?;
        if u32_at(&h, 0) != MAGIC {
            return bad("not a ZIM file");
        }
        let (major, minor) = (u16_at(&h, 4), u16_at(&h, 6));
        if !(5..=6).contains(&major) {
            return bad("unsupported ZIM version");
        }
        let articles = u32_at(&h, 24);
        let clusters = u32_at(&h, 28);
        let (url_ptr, title_ptr, cluster_ptr, mime_pos) = (u64_at(&h, 32), u64_at(&h, 40), u64_at(&h, 48), u64_at(&h, 56));
        let checksum_pos = u64_at(&h, 72);
        let ok = |p: u64, n: u64| p.checked_add(n).map(|e| e <= len).unwrap_or(false);
        if !ok(url_ptr, 8 * articles as u64) || !ok(cluster_ptr, 8 * clusters as u64) || !ok(mime_pos, 1) || checksum_pos > len || url_ptr < mime_pos {
            return bad("the ZIM header points outside the file");
        }
        let mime_len = (url_ptr - mime_pos).min(64 * 1024) as usize;
        f.seek(SeekFrom::Start(mime_pos))?;
        let mut raw = vec![0u8; mime_len];
        f.read_exact(&mut raw)?;
        let mimes: Vec<String> = raw.split(|b| *b == 0).take_while(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).to_string()).collect();
        Ok(Archive {
            inner: Mutex::new(Inner { f, cache: VecDeque::new() }),
            major,
            minor,
            articles,
            clusters,
            url_ptr,
            title_ptr,
            cluster_ptr,
            checksum_pos,
            main_page: u32_at(&h, 64),
            mimes,
            len,
        })
    }

    /// Newer archives (minor version 1 and up) keep all content in the `C` namespace.
    pub fn new_layout(&self) -> bool {
        self.minor >= 1
    }

    fn read_at(&self, pos: u64, buf: &mut [u8]) -> io::Result<()> {
        if pos.checked_add(buf.len() as u64).map(|e| e > self.len).unwrap_or(true) {
            return bad("read past the end of the file");
        }
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.f.seek(SeekFrom::Start(pos))?;
        g.f.read_exact(buf)
    }

    fn entry_pos(&self, i: u32) -> io::Result<u64> {
        if i >= self.articles {
            return bad("no such entry");
        }
        let mut b = [0u8; 8];
        self.read_at(self.url_ptr + 8 * i as u64, &mut b)?;
        Ok(u64::from_le_bytes(b))
    }

    pub fn entry(&self, i: u32) -> io::Result<Entry> {
        let pos = self.entry_pos(i)?;
        // read a little at a time until both names have ended
        let mut buf = vec![0u8; 1024];
        let avail = (self.len.saturating_sub(pos)).min(buf.len() as u64) as usize;
        buf.truncate(avail);
        self.read_at(pos, &mut buf)?;
        let mut more = 0;
        loop {
            if buf.len() >= 16 {
                let mime = u16_at(&buf, 0);
                let head = if mime == REDIRECT { 12 } else { 16 };
                let mut zeros = buf[head.min(buf.len())..].iter().enumerate().filter(|(_, b)| **b == 0).map(|(i, _)| i);
                if let (Some(a), Some(b)) = (zeros.next(), zeros.next()) {
                    let url = String::from_utf8_lossy(&buf[head..head + a]).to_string();
                    let title = String::from_utf8_lossy(&buf[head + a + 1..head + b]).to_string();
                    let kind = if mime == REDIRECT { Kind::Redirect(u32_at(&buf, 8)) } else { Kind::Content { mime, cluster: u32_at(&buf, 8), blob: u32_at(&buf, 12) } };
                    return Ok(Entry { ns: buf[3], url, title, kind });
                }
            }
            more += 1;
            let have = buf.len();
            if more > 8 || pos + have as u64 >= self.len {
                return bad("damaged directory entry");
            }
            let extra = (self.len - pos - have as u64).min(2048) as usize;
            buf.resize(have + extra, 0);
            let (a, b) = buf.split_at_mut(have);
            let _ = a;
            self.read_at(pos + have as u64, b)?;
        }
    }

    /// Binary search over the address list.
    pub fn find(&self, ns: u8, url: &str) -> io::Result<Option<u32>> {
        let (mut lo, mut hi) = (0u32, self.articles);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let e = self.entry(mid)?;
            match (e.ns, e.url.as_bytes()).cmp(&(ns, url.as_bytes())) {
                std::cmp::Ordering::Equal => return Ok(Some(mid)),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        Ok(None)
    }

    /// Turn an address as shown in the reader into a namespace and a path. Newer archives use the
    /// path alone (`Water`); older ones start with the namespace (`A/Water`).
    pub fn split_address(&self, address: &str) -> (u8, String) {
        if self.new_layout() {
            (b'C', address.to_string())
        } else if let Some((ns, rest)) = address.split_once('/') {
            if ns.len() == 1 { (ns.as_bytes()[0], rest.to_string()) } else { (b'A', address.to_string()) }
        } else {
            (b'A', address.to_string())
        }
    }

    /// The address to show for an entry: the reverse of `split_address`.
    pub fn address_of(&self, ns: u8, url: &str) -> String {
        if self.new_layout() { url.to_string() } else { format!("{}/{}", ns as char, url) }
    }

    fn follow(&self, mut i: u32) -> io::Result<(u32, Entry)> {
        for _ in 0..16 {
            let e = self.entry(i)?;
            match e.kind {
                Kind::Redirect(t) => i = t,
                Kind::Content { .. } => return Ok((i, e)),
            }
        }
        bad("redirect loop")
    }

    /// The article at an address, following redirects.
    pub fn read(&self, address: &str) -> io::Result<Option<Page>> {
        let (ns, url) = self.split_address(address);
        let Some(i) = self.find(ns, &url)? else { return Ok(None) };
        self.read_index(i).map(Some)
    }

    pub fn read_index(&self, i: u32) -> io::Result<Page> {
        let (_, e) = self.follow(i)?;
        let Kind::Content { mime, cluster, blob } = e.kind else { return bad("not content") };
        let data = self.blob(cluster, blob)?;
        let mime = self.mimes.get(mime as usize).cloned().unwrap_or_default();
        Ok(Page { mime, data, ns: e.ns, url: e.url })
    }

    /// The archive's own start page, if it has one.
    pub fn main_page(&self) -> io::Result<Option<Page>> {
        if self.main_page != NONE && self.main_page < self.articles {
            return self.read_index(self.main_page).map(Some);
        }
        if let Some(i) = self.find(b'W', "mainPage")? {
            return self.read_index(i).map(Some);
        }
        Ok(None)
    }

    /// A text item from the archive's description (`Title`, `Description`, `Language`, `Creator`).
    pub fn metadata(&self, name: &str) -> Option<String> {
        let i = self.find(b'M', name).ok()??;
        let p = self.read_index(i).ok()?;
        Some(String::from_utf8_lossy(&p.data).trim().to_string()).filter(|s| !s.is_empty())
    }

    // ----- clusters -----

    fn cluster_bounds(&self, c: u32) -> io::Result<(u64, u64)> {
        if c >= self.clusters {
            return bad("no such cluster");
        }
        let mut b = [0u8; 8];
        self.read_at(self.cluster_ptr + 8 * c as u64, &mut b)?;
        let start = u64::from_le_bytes(b);
        let end = if c + 1 < self.clusters {
            self.read_at(self.cluster_ptr + 8 * (c as u64 + 1), &mut b)?;
            u64::from_le_bytes(b)
        } else {
            self.checksum_pos
        };
        if end <= start || end > self.len {
            return bad("damaged cluster list");
        }
        Ok((start, end))
    }

    fn blob(&self, cluster: u32, blob: u32) -> io::Result<Vec<u8>> {
        let (start, end) = self.cluster_bounds(cluster)?;
        let mut info = [0u8; 1];
        self.read_at(start, &mut info)?;
        let wide = info[0] & 0x10 != 0;
        let w = if wide { 8usize } else { 4 };
        let kind = info[0] & 0x0F;
        let off = |b: &[u8], i: usize| -> u64 { if wide { u64_at(b, i * 8) } else { u32_at(b, i * 4) as u64 } };
        if kind == 0 || kind == 1 {
            // stored: read only the two offsets and the blob itself
            let mut pair = [0u8; 16];
            self.read_at(start + 1 + blob as u64 * w as u64, &mut pair[..2 * w])?;
            let (a, b) = (off(&pair, 0), off(&pair, 1));
            if b < a || start + 1 + b > end {
                return bad("damaged cluster");
            }
            let mut data = vec![0u8; (b - a) as usize];
            self.read_at(start + 1 + a, &mut data)?;
            return Ok(data);
        }
        let all = self.cluster_data(cluster, start, end, kind)?;
        let n_first = if all.len() >= w { off(&all, 0) as usize } else { return bad("damaged cluster") };
        if n_first % w != 0 || n_first > all.len() {
            return bad("damaged cluster");
        }
        let count = n_first / w - 1;
        if blob as usize >= count {
            return bad("no such blob");
        }
        let (a, b) = (off(&all, blob as usize) as usize, off(&all, blob as usize + 1) as usize);
        if b < a || b > all.len() {
            return bad("damaged cluster");
        }
        Ok(all[a..b].to_vec())
    }

    fn cluster_data(&self, c: u32, start: u64, end: u64, kind: u8) -> io::Result<Arc<Vec<u8>>> {
        {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(p) = g.cache.iter().position(|(k, _)| *k == c) {
                let item = g.cache.remove(p).unwrap();
                let d = item.1.clone();
                g.cache.push_front(item);
                return Ok(d);
            }
        }
        let size = (end - start - 1) as usize;
        if size > MAX_CLUSTER {
            return bad("cluster is too large");
        }
        let mut comp = vec![0u8; size];
        self.read_at(start + 1, &mut comp)?;
        let data = match kind {
            5 => zstd::decompress(&comp, MAX_CLUSTER),
            4 => xz::decompress(&comp, MAX_CLUSTER),
            _ => return bad("this archive uses a compression this reader does not know"),
        }
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let data = Arc::new(data);
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.cache.push_front((c, data.clone()));
        let mut total: usize = g.cache.iter().map(|(_, d)| d.len()).sum();
        while g.cache.len() > 1 && total > CACHE_BYTES {
            if let Some((_, d)) = g.cache.pop_back() {
                total -= d.len();
            }
        }
        Ok(data)
    }

    // ----- titles -----

    fn title_entry(&self, k: u32) -> io::Result<Entry> {
        let mut b = [0u8; 4];
        self.read_at(self.title_ptr + 4 * k as u64, &mut b)?;
        self.entry(u32::from_le_bytes(b))
    }

    /// Articles whose title starts with `prefix` (tried as typed, with a capital first letter,
    /// and with a small first letter). Redirects are included but shown as their own titles.
    /// Returns (address, title), at most `limit`.
    pub fn suggest(&self, prefix: &str, limit: usize) -> io::Result<Vec<(String, String)>> {
        let mut out: Vec<(String, String)> = Vec::new();
        if prefix.is_empty() || self.title_ptr == 0 {
            return Ok(out);
        }
        let ns = if self.new_layout() { b'C' } else { b'A' };
        let mut tries: Vec<String> = vec![prefix.to_string()];
        let mut chars = prefix.chars();
        if let Some(c) = chars.next() {
            let rest: String = chars.collect();
            let up: String = c.to_uppercase().chain(rest.chars()).collect();
            let lo: String = c.to_lowercase().chain(rest.chars()).collect();
            for t in [up, lo] {
                if !tries.contains(&t) {
                    tries.push(t);
                }
            }
        }
        for t in tries {
            // first title in this namespace that is >= the prefix
            let (mut lo, mut hi) = (0u32, self.articles);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                let e = self.title_entry(mid)?;
                if (e.ns, e.display_title().as_bytes()) < (ns, t.as_bytes()) {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            let mut k = lo;
            while k < self.articles && out.len() < limit {
                let e = self.title_entry(k)?;
                if e.ns != ns || !e.display_title().starts_with(&t) {
                    break;
                }
                let addr = self.address_of(e.ns, &e.url);
                if !out.iter().any(|(a, _)| *a == addr) {
                    out.push((addr, e.display_title().to_string()));
                }
                k += 1;
            }
            if out.len() >= limit {
                break;
            }
        }
        Ok(out)
    }

    /// Titles that contain every word of `words` (checked by reading titles in order, so it is
    /// limited to the first `scan` titles of the content namespace and meant for small and
    /// medium archives). Returns (address, title).
    pub fn search_titles(&self, words: &[String], limit: usize, scan: u32) -> io::Result<Vec<(String, String)>> {
        let ns = if self.new_layout() { b'C' } else { b'A' };
        let words: Vec<String> = words.iter().map(|w| w.to_lowercase()).filter(|w| !w.is_empty()).collect();
        let mut out = Vec::new();
        if words.is_empty() {
            return Ok(out);
        }
        for k in 0..self.articles.min(scan) {
            let e = self.title_entry(k)?;
            if e.ns != ns {
                continue;
            }
            let t = e.display_title().to_lowercase();
            if words.iter().all(|w| t.contains(w.as_str())) {
                out.push((self.address_of(e.ns, &e.url), e.display_title().to_string()));
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }
}
