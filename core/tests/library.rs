//! The whole-drive library over the sample archives: pages for the reader, suggestions, and
//! passages for the assistant from every archive.

use cindrix_core::library::{book_name, decode_path, keywords, signature, Library};
use std::path::PathBuf;

fn zims() -> Vec<PathBuf> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zims");
    ["new-zstd.zim", "new-xz.zim", "old-zstd.zim"].iter().map(|n| d.join(n)).collect()
}

#[test]
fn books_are_named_for_addresses_and_titled_from_the_archive() {
    let lib = Library::open(&zims());
    assert_eq!(lib.books.len(), 3);
    let names: Vec<&str> = lib.books.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, vec!["new-zstd", "new-xz", "old-zstd"]);
    assert!(lib.books.iter().all(|b| b.title == "Cindrix sample"));
    assert_eq!(book_name(&PathBuf::from("/x/Wikipedia en (all) 2025.zim")), "Wikipedia_en__all__2025");
    // two files with the same name are told apart
    let both = Library::open(&[zims()[0].clone(), zims()[0].clone()]);
    assert_eq!(both.books[0].name, "new-zstd");
    assert_eq!(both.books[1].name, "new-zstd_");
    // an unreadable file is left out
    let junk = std::env::temp_dir().join(format!("junk-{}.zim", std::process::id()));
    std::fs::write(&junk, b"nope").unwrap();
    assert_eq!(Library::open(&[junk, zims()[1].clone()]).books.len(), 1);
}

#[test]
fn pages_suggestions_and_results() {
    let lib = Library::open(&zims());
    assert!(String::from_utf8_lossy(&lib.page("new-zstd", "").unwrap().unwrap().data).contains("Cindrix sample library"));
    assert!(lib.page("new-zstd", "Water").unwrap().is_some());
    assert!(lib.page("old-zstd", "A/Water").unwrap().is_some());
    assert!(lib.page("nobody", "Water").unwrap().is_none());
    let j = lib.suggest_json("new-zstd", "water p");
    assert!(j.starts_with('[') && j.contains("\"path\":\"Water_purification\"") && j.contains("\"value\":\"Water purification\""), "{}", j);
    assert_eq!(lib.suggest_json("nobody", "x"), "[]");
    let page = lib.results_page("old-zstd", "purif");
    assert!(page.contains("/content/old-zstd/A/Water_purification"), "{}", page);
    assert!(lib.results_page("new-zstd", "<script>").contains("&lt;script&gt;"));
    assert_eq!(decode_path("Caf%C3%A9/%E6%97%A5").as_deref(), Some("Café/日"));
}

#[test]
fn the_assistant_finds_passages_in_every_archive() {
    let lib = Library::open(&zims());
    let p = lib.passages("how do I purify water", 2, 10);
    assert!(!p.is_empty());
    let top = &p[0];
    assert_eq!(top.heading, "Water purification", "{:?}", p.iter().map(|x| &x.heading).collect::<Vec<_>>());
    assert!(top.text.contains("Filter cloudy water"));
    // every archive contributes
    assert_eq!(p.iter().filter(|x| x.heading == "Water purification").count(), 3);
    // a question about nothing in the archives gives nothing
    assert!(lib.passages("quantum chromodynamics lattice", 2, 5).is_empty());
    assert!(lib.passages("the and of", 2, 5).is_empty());
    assert_eq!(keywords("How do I purify the water, water?"), vec!["purify", "water"]);
}

#[test]
fn the_signature_changes_with_the_files() {
    let a = signature(&zims());
    assert_eq!(a, signature(&zims()));
    assert_ne!(a, signature(&zims()[..2]));
}
