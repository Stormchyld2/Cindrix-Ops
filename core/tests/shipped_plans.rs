//! Every plan that ships with the program must load, and its screen and check names must be ones the program knows.

use cindrix_core::plans;
use std::path::PathBuf;

const SCREENS: &[&str] = &["Dashboard", "Ask", "Library", "Intel", "Academy", "Maps", "Music", "Inventory", "Cards", "Drills", "Family", "Binder", "What fails", "Start", "Plans", "Seed vault"];
const CHECKS: &[&str] = &["water", "food", "family", "inventory", "drill", "maps", "model", "library"];

#[test]
fn shipped_plans_load_and_are_well_formed() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/plans");
    let l = plans::load_dir(&dir);
    assert!(l.problems.is_empty(), "problems: {:?}", l.problems);
    assert!(l.plans.len() >= 30, "only {} plans", l.plans.len());
    for k in ["checks", "learning", "scenario", "prevent", "planning"] {
        assert!(l.plans.iter().any(|p| p.kind == k), "no plan of kind {}", k);
    }
    let academy = cindrix_core::academy::load_all(&[PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/academy")]);
    let lessons: Vec<String> = academy.courses.iter().flat_map(|c| c.modules.iter().flat_map(|m| m.lessons.iter().map(|x| x.id.clone()))).collect();
    for p in &l.plans {
        assert!(!p.summary.is_empty(), "{} needs a summary", p.id);
        for s in &p.steps {
            if !s.go.is_empty() {
                assert!(SCREENS.contains(&s.go.as_str()), "{}: unknown screen {}", p.id, s.go);
                assert!(!s.button.is_empty(), "{} / {}: a go: needs a button:", p.id, s.id);
            }
            if let Some(id) = s.check.strip_prefix("lesson:") {
                assert!(lessons.iter().any(|x| x == id), "{}: no such lesson {}", p.id, id);
            } else if !s.check.is_empty() {
                assert!(CHECKS.contains(&s.check.as_str()), "{}: unknown check {}", p.id, s.check);
            }
            assert!(!(s.every_days.is_some() && !s.check.is_empty()), "{} / {}: every and check do not go together", p.id, s.id);
        }
    }
}
