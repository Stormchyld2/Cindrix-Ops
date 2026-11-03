//! Compares our own ZIM reader with the real kiwix-serve, article by article:
//!   cargo run --example cross_check -- <kiwix-serve port> <file.zim>
//! (kiwix-serve must be serving only that one file.)
use cindrix_core::kiwix;
use cindrix_core::zimread::{Archive, Kind};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let port: u16 = a[1].parse().unwrap();
    let ar = Archive::open(std::path::Path::new(&a[2])).expect("open");
    let cat = kiwix::get_full(port, "/catalog/v2/entries").expect("catalog");
    let book = kiwix::parse_catalog(&String::from_utf8_lossy(&cat.body)).remove(0).name;
    let (mut same, mut diff, mut skipped) = (0, 0, 0);
    for i in 0..ar.articles {
        let e = ar.entry(i).unwrap();
        // only the content namespaces a reader asks for
        let shown = if ar.new_layout() { e.ns == b'C' } else { matches!(e.ns, b'A' | b'I' | b'-') };
        if !shown {
            skipped += 1;
            continue;
        }
        let addr = ar.address_of(e.ns, &e.url);
        let want = ar.read_index(i).unwrap();
        let path = format!("/content/{}/{}", book, cindrix_core::library::encode_path(&addr));
        match kiwix::follow(port, &path) {
            Ok((r, _)) if r.status == 200 && r.body == want.data => same += 1,
            other => {
                diff += 1;
                let what = match other { Ok((r, _)) => format!("status {} {} bytes (ours {})", r.status, r.body.len(), want.data.len()), Err(e) => e };
                println!("DIFFERENT {} ({}): {}", addr, if matches!(e.kind, Kind::Redirect(_)) { "redirect" } else { "content" }, what);
            }
        }
    }
    println!("{}: {} identical, {} different, {} not compared", a[2], same, diff, skipped);
    // titles: what kiwix suggests should also be suggested by us
    let sug = kiwix::get_full(port, &format!("/suggest?content={}&term=wat", kiwix::url_encode(&book))).expect("suggest");
    println!("kiwix suggests: {}", String::from_utf8_lossy(&sug.body).chars().take(300).collect::<String>());
    println!("we suggest: {:?}", ar.suggest("wat", 5).unwrap());
    if diff > 0 {
        std::process::exit(1);
    }
}
