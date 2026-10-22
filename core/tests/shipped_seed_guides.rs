//! The seed vault guides that ship must load, name a source, and use a kind the Seed vault page shows.

use cindrix_core::cards;
use std::path::PathBuf;

#[test]
fn shipped_seed_guides_load_and_carry_a_source() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/seed-guides");
    let l = cards::load_dirs(&[dir]);
    assert!(l.problems.is_empty(), "problems: {:?}", l.problems);
    assert!(l.cards.len() >= 12, "only {} guides", l.cards.len());
    let mut kinds = std::collections::HashSet::new();
    for c in &l.cards {
        assert!(["buy", "store", "grow", "save"].contains(&c.kind.as_str()), "{}: kind {}", c.id, c.kind);
        assert!(c.body.contains("\nSource: "), "{} has no Source line", c.id);
        kinds.insert(c.kind.clone());
    }
    assert_eq!(kinds.len(), 4, "every kind should have at least one guide");
}

#[test]
fn the_seed_vault_course_loads_with_quizzes() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/academy");
    let l = cindrix_core::academy::load_dir(&dir);
    assert!(l.problems.is_empty(), "{:?}", l.problems);
    let c = l.courses.iter().find(|c| c.id == "seed-vault").expect("seed-vault course");
    assert_eq!(c.modules.len(), 3);
    for m in &c.modules {
        for les in &m.lessons {
            assert!(les.quiz.len() >= 3, "{}", les.id);
            assert!(!les.source.is_empty(), "{}", les.id);
        }
    }
}
