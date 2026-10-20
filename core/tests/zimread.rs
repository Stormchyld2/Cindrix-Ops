//! Reading real-layout ZIM files (made by tests/fixtures/make_zims.py) with our own reader:
//! new and old namespace layouts; stored, Zstandard and XZ clusters; 32- and 64-bit offsets.

use cindrix_core::zimread::Archive;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zims").join(name)
}

const ALL: [(&str, bool); 4] = [("new-zstd.zim", true), ("new-xz.zim", true), ("new-ext-stored.zim", true), ("old-zstd.zim", false)];

fn addr(new: bool, ns: &str, path: &str) -> String {
    if new { path.to_string() } else { format!("{}/{}", ns, path) }
}

#[test]
fn every_layout_and_compression_reads_the_same_content() {
    for (name, new) in ALL {
        let a = Archive::open(&fixture(name)).unwrap_or_else(|e| panic!("{}: {}", name, e));
        assert_eq!(a.new_layout(), new, "{}", name);
        assert_eq!(a.metadata("Title").as_deref(), Some("Cindrix sample"), "{}", name);
        assert_eq!(a.metadata("Language").as_deref(), Some("eng"), "{}", name);
        assert!(a.metadata("Nothing").is_none());

        let main = a.main_page().unwrap().unwrap_or_else(|| panic!("{}: no main page", name));
        assert!(String::from_utf8_lossy(&main.data).contains("Cindrix sample library"), "{}", name);
        assert_eq!(main.mime, "text/html");

        let w = a.read(&addr(new, "A", "Water")).unwrap().unwrap();
        assert!(String::from_utf8_lossy(&w.data).contains("Boil water for one minute"), "{}", name);
        let s = a.read(&addr(new, "A", "Shelter")).unwrap().unwrap();
        assert!(String::from_utf8_lossy(&s.data).contains("Zürich naïve 日本語"), "{}", name);

        // redirects lead to the real page and say where it really is
        let r = a.read(&addr(new, "A", "Water_alias")).unwrap().unwrap();
        assert_eq!(r.url, "Water");
        assert_eq!(r.data, w.data);
        let r2 = a.read(&addr(new, "A", "Fire_redirect")).unwrap().unwrap();
        assert!(String::from_utf8_lossy(&r2.data).contains("Keep tinder dry"));

        let css = a.read(&addr(new, "-", "style.css")).unwrap().unwrap();
        assert_eq!(css.mime, "text/css");
        assert!(css.data.starts_with(b"body{"));
        let png = a.read(&addr(new, "I", "pic.png")).unwrap().unwrap();
        assert_eq!(png.mime, "image/png");
        assert!(png.data.starts_with(&[0x89, b'P', b'N', b'G']));

        assert!(a.read(&addr(new, "A", "No_such_page")).unwrap().is_none());
        assert!(a.read("").unwrap().is_none());

        // the binary search finds every entry, including the first and last in each group
        for i in 0..60 {
            let p = a.read(&addr(new, "A", &format!("Topic_{:03}", i))).unwrap().unwrap_or_else(|| panic!("{} missing topic {}", name, i));
            assert!(String::from_utf8_lossy(&p.data).contains(&format!("Topic {:03}", i)));
        }
    }
}

#[test]
fn title_search_finds_prefixes_and_words() {
    for (name, new) in ALL {
        let a = Archive::open(&fixture(name)).unwrap();
        let titles = |p: &str| a.suggest(p, 20).unwrap().into_iter().map(|(_, t)| t).collect::<Vec<_>>();
        let t = titles("wat");
        assert!(t.contains(&"Water".to_string()) && t.contains(&"Water purification".to_string()) && t.contains(&"Water alias".to_string()), "{} {:?}", name, t);
        assert!(titles("first").contains(&"first aid".to_string()), "{}", name);
        assert!(titles("First").contains(&"first aid".to_string()), "{}", name); // small first letter tried too
        assert!(titles("éc").contains(&"Éclair".to_string()), "{}", name);
        assert!(titles("zzz").is_empty());
        assert!(titles("").is_empty());
        assert_eq!(a.suggest("Topic", 5).unwrap().len(), 5);
        // suggested addresses can be opened
        let (address, _) = a.suggest("Water pur", 3).unwrap().remove(0);
        assert_eq!(address, addr(new, "A", "Water_purification"));
        assert!(a.read(&address).unwrap().is_some());

        let hits = a.search_titles(&["purif".to_string(), "water".to_string()], 10, 100_000).unwrap();
        assert_eq!(hits.len(), 1, "{} {:?}", name, hits);
    }
}

#[test]
fn damaged_files_give_errors_not_crashes() {
    let good = std::fs::read(fixture("new-zstd.zim")).unwrap();
    let dir = std::env::temp_dir().join(format!("cindrix-zr-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut rng: u64 = 0xDEAD_BEEF_1234_5678;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for round in 0..120 {
        let mut d = good.clone();
        if round % 3 == 0 {
            d.truncate((next() as usize) % d.len());
        } else {
            for _ in 0..(1 + round % 7) {
                let i = (next() as usize) % d.len();
                d[i] ^= (next() as u8) | 1;
            }
        }
        let p = dir.join(format!("d{}.zim", round));
        std::fs::write(&p, &d).unwrap();
        if let Ok(a) = Archive::open(&p) {
            let _ = a.main_page();
            let _ = a.metadata("Title");
            for name in ["Water", "Topic_007", "style.css", "pic.png", "Water_alias"] {
                let _ = a.read(name);
            }
            let _ = a.suggest("w", 5);
            let _ = a.search_titles(&["x".to_string()], 5, 1000);
        }
    }
    assert!(Archive::open(&dir.join("missing.zim")).is_err());
    std::fs::write(dir.join("junk.zim"), b"this is not a zim file at all, just text").unwrap();
    assert!(Archive::open(&dir.join("junk.zim")).is_err());
}

#[test]
fn the_archive_can_be_shared_between_threads() {
    let a = std::sync::Arc::new(Archive::open(&fixture("new-xz.zim")).unwrap());
    let mut hs = Vec::new();
    for t in 0..4 {
        let a = a.clone();
        hs.push(std::thread::spawn(move || {
            for i in 0..60 {
                let n = (i + t * 7) % 60;
                let p = a.read(&format!("Topic_{:03}", n)).unwrap().unwrap();
                assert!(String::from_utf8_lossy(&p.data).contains(&format!("Topic {:03}", n)));
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
}
