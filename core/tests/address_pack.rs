//! Checks a pack made by scripts/get-addresses.ps1. Skipped unless CINDRIX_ADDRESS_PACK names the folder
//! (the build makes a small one for Denver). The point: the files the download writes are sorted the way
//! the program's search expects, and the search finds an address read from them.
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

fn digits(n: &str) -> Option<u128> {
    let d: String = n.chars().take_while(|c| c.is_ascii_digit()).take(18).collect();
    d.parse().ok()
}

#[test]
fn a_downloaded_pack_is_sorted_and_searchable() {
    let Ok(dir) = std::env::var("CINDRIX_ADDRESS_PACK") else {
        eprintln!("no pack to check; skipped");
        return;
    };
    let pack = PathBuf::from(&dir);
    let mut total = 0usize;
    let mut sample: Option<(String, String)> = None;
    for e in fs::read_dir(&pack).unwrap().flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("tsv") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        let mut prev: Option<(String, String)> = None;
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            assert_eq!(f.len(), 6, "{}: {:?}", path.display(), line);
            assert!(f[4].parse::<f64>().is_ok() && f[5].parse::<f64>().is_ok(), "{:?}", line);
            assert_eq!(cindrix_core::addresses::norm_street(f[0]), f[0], "key not in key form: {:?}", line);
            if let Some((ps, pn)) = &prev {
                let ord = ps
                    .as_bytes()
                    .cmp(f[0].as_bytes())
                    .then_with(|| digits(pn).cmp(&digits(f[1])))
                    .then_with(|| pn.as_bytes().cmp(f[1].as_bytes()));
                assert_ne!(ord, Ordering::Greater, "{} out of order near {:?}", path.display(), line);
            }
            prev = Some((f[0].to_string(), f[1].to_string()));
            if sample.is_none() && f[1].chars().all(|c| c.is_ascii_digit()) {
                sample = Some((f[1].to_string(), f[0].to_string()));
            }
            total += 1;
        }
    }
    assert!(total > 100, "only {} addresses in the pack", total);
    let (n, s) = sample.unwrap();
    let hits = cindrix_core::addresses::search(pack.parent().unwrap(), &format!("{} {}", n, s), None, 8);
    assert!(hits.iter().any(|h| h.number == n && !h.near), "{} {} not found", n, s);
}
