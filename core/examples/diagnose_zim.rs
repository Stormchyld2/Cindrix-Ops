//! Read-only diagnosis of one ZIM archive against our own reader and (optionally) the real kiwix-serve.
//!   cargo run --release --example diagnose_zim -- <file.zim> [path-to-kiwix-serve]
//! Never writes to the archive. Prints facts only; nothing is fixed here.
use cindrix_core::kiwix;
use cindrix_core::library::{self, Library};
use cindrix_core::zimread::{Archive, Kind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let file = PathBuf::from(a.get(1).expect("give a .zim file"));
    println!("== file: {} ({} bytes)", file.display(), std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0));
    let ar = match Archive::open(&file) {
        Ok(x) => x,
        Err(e) => {
            println!("OUR READER CANNOT OPEN IT: {}", e);
            println!("  (the app would silently leave this archive out of its list; see Library::open_in)");
            std::process::exit(2);
        }
    };
    let ours = library::book_name(&file);
    println!("== header: ZIM {}.{} (new layout: {}), {} entries, {} clusters", ar.major, ar.minor, ar.new_layout(), ar.articles, ar.clusters);
    for k in ["Name", "Title", "Date", "Flavour", "Language", "Creator", "Publisher", "Scraper", "Counter"] {
        println!("   metadata {:<9} = {:?}", k, ar.metadata(k).map(|s| s.chars().take(80).collect::<String>()));
    }
    println!("== our address name (filename stem): {}", ours);

    // main page
    match ar.main_page() {
        Ok(Some(p)) => println!("== main page (ours): {}/{} mime={} bytes={}", p.ns as char, p.url, p.mime, p.data.len()),
        Ok(None) => println!("== main page (ours): NONE (no header entry and no W/mainPage)"),
        Err(e) => println!("== main page (ours): ERROR {}", e),
    }
    for (ns, u) in [(b'W', "mainPage"), (b'C', "index.html"), (b'A', "index.html")] {
        println!("   find {}/{} -> {:?}", ns as char, u, ar.find(ns, u).ok().flatten());
    }

    // sample entries
    let n = ar.articles;
    let step = (n / 20000).max(1);
    let mut ns_count: BTreeMap<char, u32> = BTreeMap::new();
    let (mut redirects, mut content, mut with_query, mut read_ok) = (0u32, 0u32, 0u32, 0u32);
    let mut errs: BTreeMap<String, u32> = BTreeMap::new();
    let mut mimes: BTreeMap<String, u32> = BTreeMap::new();
    let mut query_examples: Vec<String> = Vec::new();
    let mut i = 0;
    while i < n {
        match ar.entry(i) {
            Ok(e) => {
                *ns_count.entry(e.ns as char).or_default() += 1;
                if e.url.contains('?') {
                    with_query += 1;
                    if query_examples.len() < 3 { query_examples.push(format!("{}/{}", e.ns as char, e.url)); }
                }
                match e.kind {
                    Kind::Redirect(_) => redirects += 1,
                    Kind::Content { .. } => content += 1,
                }
                match ar.read_index(i) {
                    Ok(p) => { read_ok += 1; *mimes.entry(p.mime).or_default() += 1; }
                    Err(x) => *errs.entry(x.to_string()).or_default() += 1,
                }
            }
            Err(x) => *errs.entry(format!("entry: {}", x)).or_default() += 1,
        }
        i += step;
    }
    println!("== sampled every {}th entry: namespaces {:?}", step, ns_count);
    println!("   content {} redirects {} readable {} with '?' in address {} {:?}", content, redirects, read_ok, with_query, query_examples);
    println!("   mime types read (top): {:?}", mimes.iter().take(8).collect::<Vec<_>>());
    println!("   read errors: {:?}", errs);

    // what our in-app server would do for the archive root and for the main page's own address
    let lib = Library::open(&[file.clone()]);
    match lib.page(&ours, "") {
        Ok(Some(p)) => println!("== in-app /content/{} -> 200 (main page {} bytes, {})", ours, p.data.len(), p.mime),
        Ok(None) => println!("== in-app /content/{} -> 404 'That page is not in this archive.' (book known: {})", ours, lib.book(&ours).is_some()),
        Err(e) => println!("== in-app /content/{} -> 500 {}", ours, e),
    }

    // the real kiwix-serve
    let Some(ks) = a.get(2) else { return };
    let port = 18000 + (std::process::id() % 1000) as u16;
    let mut child = Command::new(ks)
        .args(["--address", "127.0.0.1", "--port", &port.to_string(), file.to_str().unwrap()])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("start kiwix-serve");
    let mut ready = false;
    for _ in 0..120 {
        if kiwix::get(port, "/").map(|(s, _)| s == 200).unwrap_or(false) { ready = true; break; }
        std::thread::sleep(Duration::from_millis(500));
    }
    println!("== kiwix-serve ready: {}", ready);
    if ready {
        let cat = kiwix::get_full(port, "/catalog/v2/entries").map(|r| String::from_utf8_lossy(&r.body).to_string()).unwrap_or_default();
        let books = kiwix::parse_catalog(&cat);
        println!("   kiwix book names: {:?}", books.iter().map(|b| b.name.clone()).collect::<Vec<_>>());
        let kname = books.first().map(|b| b.name.clone()).unwrap_or_default();
        println!("   SAME NAME AS OURS? {}  (ours {:?}, kiwix {:?})", kname == ours, ours, kname);
        for (label, name) in [("kiwix's own name", kname.as_str()), ("OUR name", ours.as_str())] {
            let path = format!("/content/{}", kiwix::url_encode(name));
            match kiwix::follow(port, &path) {
                Ok((r, fp)) => println!("   kiwix-serve GET {} ({}) -> {} final {} bytes {} type {}", path, label, r.status, fp, r.body.len(), r.content_type),
                Err(e) => println!("   kiwix-serve GET {} ({}) -> error {}", path, label, e),
            }
            // and without following redirects, to see the raw answer
            match kiwix::get(port, &path) { Ok((s, b)) => println!("      raw first answer: {} body {} bytes", s, b.len()), Err(e) => println!("      raw: {}", e) }
        }
        // compare sampled entries
        let (mut same, mut diff, mut shown) = (0, 0, 0);
        let step2 = (n / 400).max(1);
        let mut j = 0;
        while j < n {
            if let Ok(e) = ar.entry(j) {
                let shown_ns = if ar.new_layout() { e.ns == b'C' } else { matches!(e.ns, b'A' | b'I' | b'-') };
                if shown_ns {
                    let addr = ar.address_of(e.ns, &e.url);
                    if let Ok(want) = ar.read_index(j) {
                        let path = format!("/content/{}/{}", kname, library::encode_path(&addr));
                        match kiwix::follow(port, &path) {
                            Ok((r, _)) if r.status == 200 && r.body == want.data => same += 1,
                            other => {
                                diff += 1;
                                if shown < 12 {
                                    shown += 1;
                                    let what = match other { Ok((r, fp)) => format!("status {} final {} {} bytes (ours {})", r.status, fp, r.body.len(), want.data.len()), Err(e) => e };
                                    println!("   DIFFERENT {} : {}", addr, what);
                                }
                            }
                        }
                    }
                }
            }
            j += step2;
        }
        println!("   sampled comparison: {} identical, {} different", same, diff);
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = Path::new("");
}
