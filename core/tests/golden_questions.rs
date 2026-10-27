//! Golden questions: plain questions with the page each answer should come from. They run the
//! assistant's own keyword search over the shipped pages, so a change that makes search worse fails here.
//! (Idea from Project NOMAD's eval harness, written for this code base.)

use cindrix_core::retrieval;
use std::path::PathBuf;

fn page_folders() -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content");
    let mut v = vec![base.join("library-sample"), base.join("cards"), base.join("plans")];
    if let Ok(rd) = std::fs::read_dir(base.join("guides")) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                v.push(e.path());
            }
        }
    }
    v
}

fn top_sources(question: &str, n: usize) -> Vec<String> {
    let mut all = Vec::new();
    for d in page_folders() {
        all.extend(retrieval::search(&d, question, 3));
    }
    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut out: Vec<String> = Vec::new();
    for p in all {
        if !out.contains(&p.source) {
            out.push(p.source);
        }
        if out.len() == n {
            break;
        }
    }
    out
}

#[test]
fn golden_questions_find_their_page_in_the_top_three() {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/questions.tsv")).unwrap();
    let mut misses = Vec::new();
    let mut total = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let (q, want) = line.split_once('\t').expect("question<TAB>page");
        total += 1;
        let got = top_sources(q, 3);
        if !got.iter().any(|g| g == want.trim()) {
            misses.push(format!("{:?} wanted {} but got {:?}", q, want.trim(), got));
        }
    }
    assert!(total >= 10);
    assert!(misses.is_empty(), "{} of {} golden questions missed:\n{}", misses.len(), total, misses.join("\n"));
}
