//! The list of archives that `scripts/get-library.ps1` can fetch: every entry needs a title, a size and an
//! https address of a .zim file, and the list says where it came from.

use std::path::PathBuf;

#[test]
fn the_library_catalog_is_well_formed() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-catalog.json");
    let text = std::fs::read_to_string(p).unwrap();
    assert!(text.contains("Project NOMAD") && text.contains("Apache-2.0"), "the list must credit its source");
    let urls: Vec<&str> = text.lines().filter_map(|l| l.trim().strip_prefix("\"url\": \"")).collect();
    assert!(urls.len() >= 50, "only {} archives", urls.len());
    for u in &urls {
        assert!(u.starts_with("https://download.kiwix.org/") && u.trim_end_matches(&['"', ','][..]).ends_with(".zim"), "bad address {}", u);
    }
    let sizes = text.lines().filter(|l| l.trim().starts_with("\"size_mb\":")).count();
    let titles = text.lines().filter(|l| l.trim().starts_with("\"title\":")).count();
    assert_eq!(urls.len(), sizes);
    assert_eq!(urls.len(), titles);
}
