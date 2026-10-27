//! The list of archives that `scripts/get-library.ps1` can fetch: every entry needs a title, a size and an
//! https address of a .zim file, and the list says where it came from.

use std::path::PathBuf;

#[test]
fn the_library_catalog_is_well_formed() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-catalog.json");
    let whole = std::fs::read_to_string(p).unwrap();
    let (text, to_find) = whole.split_once("\"to_find\"").expect("the list needs a to_find part");
    assert!(to_find.contains("Khan"));
    let text = text.to_string();
    assert!(text.contains("Project NOMAD") && text.contains("Apache-2.0"), "the list must credit its source");
    let urls: Vec<&str> = text.lines().filter_map(|l| l.trim().strip_prefix("\"url\": \"")).collect();
    assert!(urls.len() >= 60, "only {} archives", urls.len());
    for u in &urls {
        assert!(u.starts_with("https://download.kiwix.org/") && u.trim_end_matches(&['"', ','][..]).ends_with(".zim"), "bad address {}", u);
    }
    let sizes = text.lines().filter(|l| l.trim().starts_with("\"size_mb\":")).count();
    let titles = text.lines().filter(|l| l.trim().starts_with("\"title\":")).count();
    assert_eq!(urls.len(), sizes);
    assert_eq!(urls.len(), titles);
    // every archive says what license it is under and whether the sold version may carry it
    let licenses = text.lines().filter(|l| l.trim().starts_with("\"license\":")).count();
    assert_eq!(urls.len(), licenses);
    let sell: Vec<&str> = text.lines().filter_map(|l| l.trim().strip_prefix("\"sell_ok\": \"")).collect();
    assert_eq!(urls.len(), sell.len());
    for v in &sell {
        assert!(v.starts_with("yes\"") || v.starts_with("check\"") || v.starts_with("no\""), "bad sell_ok {}", v);
    }
    // nothing known to be non-commercial may be marked as sellable
    for (i, l) in text.lines().enumerate() {
        if l.contains("NC-") || l.contains("non-commercial") || l.contains("Non-commercial") {
            let around: String = text.lines().skip(i).take(3).collect();
            assert!(!around.contains("\"sell_ok\": \"yes\""), "non-commercial marked sellable near line {}", i);
        }
    }
}
