//! Builds a small sample archive with the website saver, using a made-up website instead of the
//! internet. Used by the CI check that opens the result with the real Kiwix tools.
//!   cargo run --example make_zim -- <output folder>

use cindrix_core::crawl::{self, Fetcher, Options, Response};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

struct Site(HashMap<String, (&'static str, Vec<u8>)>);

impl Fetcher for Site {
    fn get(&self, url: &str, _max: usize) -> Result<Response, String> {
        match self.0.get(url) {
            Some((ct, body)) => Ok(Response { status: 200, final_url: url.to_string(), content_type: ct.to_string(), body: body.clone() }),
            None => Ok(Response { status: 404, final_url: url.to_string(), content_type: String::new(), body: vec![] }),
        }
    }
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output folder"));
    std::fs::create_dir_all(&out).unwrap();
    // A 1x1 PNG.
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63,
        0xf8, 0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
        0xae, 0x42, 0x60, 0x82,
    ];
    let mut m: HashMap<String, (&'static str, Vec<u8>)> = HashMap::new();
    m.insert(
        "https://example.test/".into(),
        ("text/html", b"<html><head><title>Sample home</title><link rel=stylesheet href=/site.css></head><body><h1>Water</h1><p>Boil water for one minute.</p><img src=/pic.png alt=pic><a href=/guides/water%20storage.html>Water storage</a></body></html>".to_vec()),
    );
    m.insert(
        "https://example.test/guides/water%20storage.html".into(),
        ("text/html", "<html><head><title>Water storage \u{2013} guide</title></head><body><p>Store one gallon per person per day.</p><a href=\"/\">Home</a></body></html>".as_bytes().to_vec()),
    );
    m.insert("https://example.test/site.css".into(), ("text/css", b"body{background:url(/pic.png)}".to_vec()));
    m.insert("https://example.test/pic.png".into(), ("image/png", png));
    let mut o = Options::new("https://example.test/", "Sample site");
    o.delay_ms = 0;
    o.respect_robots = false;
    let cancel = AtomicBool::new(false);
    let s = crawl::run(&o, &Site(m), &out.join("sample.zim"), &out.join("sample.md"), &out.join("spool"), &cancel, &mut |_| {}).expect("crawl failed");
    println!("{} pages, {} files, {} bytes -> {}", s.pages, s.files, s.bytes, s.zim.display());
}
