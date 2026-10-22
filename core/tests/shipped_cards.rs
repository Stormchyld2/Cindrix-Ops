//! Every card that ships with the program must load, say where it came from, and never name a medicine amount.

use cindrix_core::cards;
use std::path::PathBuf;

#[test]
fn shipped_cards_load_are_grouped_and_carry_a_source() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/cards");
    let l = cards::load_dirs(&[dir]);
    assert!(l.problems.is_empty(), "problems: {:?}", l.problems);
    assert!(l.cards.len() >= 45, "only {} cards", l.cards.len());
    let mut titles = std::collections::HashSet::new();
    for c in &l.cards {
        assert!(titles.insert(c.title.clone()), "two cards called {}", c.title);
        assert!(["medical", "disaster", "home"].contains(&c.kind.as_str()), "{}: kind {}", c.id, c.kind);
        assert!(c.body.contains("\nSource: "), "{} has no Source line", c.id);
        assert!(c.body.contains("911") || c.body.contains("988"), "{} never says when to call for help", c.id);
        assert!(c.status == "draft" || c.is_reviewed(), "{}", c.id);
    }
}
