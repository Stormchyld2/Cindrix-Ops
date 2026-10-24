//! The guide pages that ship with the program: every page reads, names a source, and gives no medicine amount.

use cindrix_core::{cards, guides};
use std::path::PathBuf;

#[test]
fn every_shipped_guide_page_reads_and_has_a_source() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/guides");
    let mut total = 0;
    for (id, _) in guides::SECTIONS {
        let dir = base.join(id);
        assert!(dir.is_dir(), "missing folder {}", id);
        let l = cards::load_dirs(&[dir]);
        assert!(l.problems.is_empty(), "{}: {:?}", id, l.problems);
        assert!(!l.cards.is_empty(), "{} has no pages", id);
        for c in &l.cards {
            assert!(c.body.lines().any(|x| x.trim_start().starts_with("Source:")), "{}/{} has no Source: line", id, c.id);
            assert_ne!(c.status, "reviewed", "{}/{} claims to be reviewed", id, c.id);
            total += 1;
        }
    }
    assert!(total >= 90, "only {} pages", total);
}
