//! How the program copes with a big archive and with several readers at once (roadmap: large libraries).
//! Makes a made-up archive of many articles, builds the program's own search index for it, then times searches,
//! one reader and then several at the same time. Nothing here touches the internet.
//!   cargo run --release --example probe_scale -- <work folder> <articles> <words per article> <readers>
//! The archive is written uncompressed, so this measures the index and the search, not the cost of reading
//! compressed clusters from a real Wikipedia file (that needs a real file on real hardware).

use cindrix_core::ftindex::{self, Depth, Index, Progress};
use cindrix_core::zim::ZimWriter;
use cindrix_core::zimread::Archive;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("VmHWM:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn word(i: u64) -> String {
    // pronounceable made-up words, so the tokeniser treats them like real ones
    const C: &[u8] = b"bdfgklmnprstvz";
    const V: &[u8] = b"aeiou";
    let mut n = i + 1;
    let mut s = String::new();
    while n > 0 || s.len() < 4 {
        s.push(C[(n % 14) as usize] as char);
        n /= 14;
        s.push(V[(n % 5) as usize] as char);
        n /= 5;
        if s.len() > 14 { break; }
    }
    s
}

fn pct(v: &mut Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() as f64 - 1.0) * p) as usize]
}

fn dir_size(p: &std::path::Path) -> u64 {
    std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.metadata().map(|m| if m.is_dir() { dir_size(&e.path()) } else { m.len() }).unwrap_or(0)).sum()
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let work = PathBuf::from(a.get(1).expect("work folder"));
    let articles: u64 = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(50_000);
    let words: u64 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(300);
    let readers: usize = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(8);
    // a fifth word, "reuse", keeps the archive and the first index from an earlier run
    let reuse = a.get(5).map(|x| x == "reuse").unwrap_or(false) && work.join("big.zim").exists();
    if !reuse {
        let _ = std::fs::remove_dir_all(&work);
    }
    std::fs::create_dir_all(&work).unwrap();

    // 1. the archive: a vocabulary of 60 000 words used very unevenly, as real text is
    let t = Instant::now();
    let zim = work.join("big.zim");
    let mut w = ZimWriter::create(&zim).unwrap();
    let mut r = Rng(0x9e3779b97f4a7c15);
    for i in 0..(if reuse { 0 } else { articles }) {
        let mut body = String::new();
        for _ in 0..words {
            let x = r.next() % 1_000_000;
            // squaring skews the choice towards the first words
            let k = ((x * x) / 1_000_000_000_000u64.max(1) as u64) as f64 / 1_000_000.0;
            let idx = ((x as f64 / 1_000_000.0).powi(3) * 60_000.0) as u64 + (k as u64 & 1);
            body.push_str(&word(idx));
            body.push(' ');
        }
        let title = format!("{} {}", word(r.next() % 20_000), word(r.next() % 20_000));
        let html = format!("<html><head><title>{}</title></head><body><h1>{}</h1><p>{}</p></body></html>", title, title, body);
        w.add(b'C', &format!("a{:07}", i), &title, "text/html", html.into_bytes()).unwrap();
    }
    if !reuse {
        w.set_main_page("a0000000");
        w.finish().unwrap();
    } else {
        drop(w);
    }
    let zim_mb = std::fs::metadata(&zim).unwrap().len() / 1_048_576;
    println!("archive: {} articles, {} MB, made in {:.1}s", articles, zim_mb, t.elapsed().as_secs_f64());

    // 2. the index
    let arch = Archive::open(&zim).unwrap();
    let fp = ftindex::fingerprint(&zim).unwrap();
    let dir = ftindex::index_dir_for(&work.join("idx"), &zim, &fp);
    let before = rss_mb();
    let t = Instant::now();
    let p = Progress::default();
    let out = ftindex::build(&arch, &dir, &fp, Depth::Lead, ftindex::DEFAULT_BUDGET, &p).unwrap();
    let secs = t.elapsed().as_secs_f64();
    println!("index: {:?} in {:.1}s ({:.0} articles/s), {} MB on disk ({:.0}% of the archive), peak memory {} MB (was {} MB)", out, secs, articles as f64 / secs, dir_size(&dir) / 1_048_576, 100.0 * dir_size(&dir) as f64 / (zim_mb as f64 * 1_048_576.0), rss_mb(), before);

    // 3. searches: common words, rare words, two words, a word that is not there
    let queries: Vec<String> = (0..300u64).map(|i| match i % 4 {
        0 => word(i % 50),
        1 => word(5_000 + i * 97 % 50_000),
        2 => format!("{} {}", word(i % 200), word(300 + i % 900)),
        _ => "qqqzzzxxx".to_string(),
    }).collect();
    let ix = Arc::new(Index::open(&dir, &fp).unwrap());
    let mut lat = vec![];
    for q in &queries {
        let t = Instant::now();
        let _ = ix.search(q, 20).unwrap();
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    println!("one reader: median {:.1} ms, 95th {:.1} ms, worst {:.1} ms", pct(&mut lat.clone(), 0.5), pct(&mut lat.clone(), 0.95), pct(&mut lat, 1.0));

    // 4. several readers at once, sharing the open index, each asking the same 300 questions
    let t = Instant::now();
    let handles: Vec<_> = (0..readers).map(|n| {
        let ix = ix.clone();
        let qs = queries.clone();
        std::thread::spawn(move || {
            let mut lat = vec![];
            for (j, q) in qs.iter().enumerate() {
                if (j + n) % 3 == 3 { continue; }
                let t = Instant::now();
                let _ = ix.search(q, 20).unwrap();
                lat.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            lat
        })
    }).collect();
    let mut all: Vec<f64> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    println!("{} readers at once: {} searches in {:.1}s, median {:.1} ms, 95th {:.1} ms, worst {:.1} ms, peak memory {} MB", readers, all.len(), t.elapsed().as_secs_f64(), pct(&mut all.clone(), 0.5), pct(&mut all.clone(), 0.95), pct(&mut all, 1.0), rss_mb());

    // 5. stop part way and carry on: the finished index must be the same size as an unbroken one
    let dir2 = work.join("idx2");
    let p2 = Arc::new(Progress::default());
    let stopper = p2.clone();
    let total = articles;
    let h = std::thread::spawn(move || {
        while stopper.done.load(std::sync::atomic::Ordering::SeqCst) < total / 3 { std::thread::sleep(std::time::Duration::from_millis(20)); }
        stopper.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let d2 = ftindex::index_dir_for(&dir2, &zim, &fp);
    let first = ftindex::build(&arch, &d2, &fp, Depth::Lead, ftindex::DEFAULT_BUDGET, &p2).unwrap();
    h.join().unwrap();
    let p3 = Progress::default();
    let t = Instant::now();
    let second = ftindex::build(&arch, &d2, &fp, Depth::Lead, ftindex::DEFAULT_BUDGET, &p3).unwrap();
    println!("stop and resume: first {:?}, then {:?} in {:.1}s; sizes {} MB and {} MB", first, second, t.elapsed().as_secs_f64(), dir_size(&dir) / 1_048_576, dir_size(&d2) / 1_048_576);
}
