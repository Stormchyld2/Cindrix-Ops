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

#[test]
fn searching_every_archive_at_once_alternates_between_them() {
    let lib = Library::open(&zims());
    let hits = lib.suggest_all("water", 2, 10);
    assert!(!hits.is_empty());
    // every archive is represented and the first three results are one from each
    let first: Vec<&str> = hits.iter().take(3).map(|h| h.book.as_str()).collect();
    assert_eq!(first, vec!["new-zstd", "new-xz", "old-zstd"], "{:?}", hits);
    assert!(hits.iter().all(|h| h.book_title == "Cindrix sample" && !h.address.is_empty() && !h.title.is_empty()));
    // at most `per_book` from each archive, and at most `total` overall
    assert!(hits.iter().filter(|h| h.book == "new-zstd").count() <= 2);
    assert_eq!(lib.suggest_all("water", 2, 2).len(), 2);
    // too short, or nothing there
    assert!(lib.suggest_all("w", 2, 10).is_empty());
    assert!(lib.suggest_all("zzzzqqqq", 2, 10).is_empty());
    // a title that only contains the word is found too
    assert!(lib.suggest_all("purification", 3, 10).iter().any(|h| h.title == "Water purification"));
}

#[test]
fn the_library_server_restarts_only_once_new_archives_have_settled() {
    use cindrix_core::library::SettleWatch;
    let dir = std::env::temp_dir().join(format!("cindrix-settle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = zims()[0].clone();
    let b = dir.join("new.zim");
    std::fs::write(&b, b"partial").unwrap();
    let running = vec![a.clone()];
    let mut w = SettleWatch::default();
    // first look: nothing to compare with yet
    assert!(!w.should_restart(&[a.clone(), b.clone()], &running));
    // the file is still growing (a copy in progress): wait
    std::fs::write(&b, b"partial and more").unwrap();
    assert!(!w.should_restart(&[a.clone(), b.clone()], &running));
    // unchanged since the last look: restart
    assert!(w.should_restart(&[a.clone(), b.clone()], &running));
    // once the server runs with both, no more restarts
    assert!(!w.should_restart(&[a.clone(), b.clone()], &[a.clone(), b.clone()]));
    // a removed archive counts as a change too, after it settles
    let _ = w.should_restart(std::slice::from_ref(&a), &[a.clone(), b.clone()]);
    assert!(w.should_restart(std::slice::from_ref(&a), &[a.clone(), b.clone()]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folders_below_the_library_give_categories_and_narrow_the_search() {
    use cindrix_core::library::category_for;
    let root = std::env::temp_dir().join(format!("cx-cats-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Medicine")).unwrap();
    std::fs::create_dir_all(root.join("Survival/Water")).unwrap();
    let z = zims();
    std::fs::copy(&z[0], root.join("Medicine/a.zim")).unwrap();
    std::fs::copy(&z[1], root.join("Survival/Water/b.zim")).unwrap();
    std::fs::copy(&z[2], root.join("c.zim")).unwrap();
    let files = vec![root.join("Medicine/a.zim"), root.join("Survival/Water/b.zim"), root.join("c.zim")];
    let roots = vec![root.clone()];
    assert_eq!(category_for(&files[0], &roots), "Medicine");
    assert_eq!(category_for(&files[1], &roots), "Survival");
    assert_eq!(category_for(&files[2], &roots), "");
    assert_eq!(category_for(&PathBuf::from("/elsewhere/x.zim"), &roots), "");
    let lib = Library::open_in(&files, &roots);
    assert_eq!(lib.categories(), vec!["Medicine".to_string(), "Survival".to_string()]);
    let all = lib.suggest_in(None, "water", 2, 10);
    let med = lib.suggest_in(Some("medicine"), "water", 2, 10);
    assert!(!med.is_empty() && med.iter().all(|h| h.book == "a"));
    assert!(all.len() > med.len());
    assert!(lib.suggest_in(Some("Nothing"), "water", 2, 10).is_empty());
    std::fs::remove_dir_all(&root).ok();
}
