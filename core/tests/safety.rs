//! Runs every case in tests/safety/dosing.json against the safety check.

use cindrix_core::safety::{check_message, Verdict};
use std::fs;
use std::path::PathBuf;

fn field(obj: &str, key: &str) -> Option<String> {
    let k = format!("\"{}\"", key);
    let at = obj.find(&k)? + k.len();
    let rest = &obj[at..];
    let rest = &rest[rest.find('"')? + 1..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                other => out.push(other),
            },
            '"' => return Some(out),
            c => out.push(c),
        }
    }
    None
}

#[test]
fn dosing_cases() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/safety/dosing.json");
    let text = fs::read_to_string(&path).expect("tests/safety/dosing.json");
    let mut failures = Vec::new();
    let mut count = 0;
    for chunk in text.split("{\n      \"id\"").skip(1) {
        let obj = format!("\"id\"{}", chunk);
        let id = field(&obj, "id").unwrap();
        let msg = field(&obj, "message").unwrap();
        let expect = field(&obj, "expect").unwrap();
        count += 1;
        let v = check_message(&msg);
        let ok = match expect.as_str() {
            "refuse" => matches!(v, Verdict::Refuse(_)),
            "answer" => v == Verdict::Allow,
            other => panic!("unknown expect {}", other),
        };
        if !ok {
            failures.push(format!("{} ({}): {}", id, expect, msg));
        }
    }
    assert!(count >= 30, "expected at least 30 cases, found {}", count);
    assert!(failures.is_empty(), "safety failures:\n{}", failures.join("\n"));
}

#[test]
fn library_sample_answers_boiling_question() {
    use cindrix_core::{prompt, retrieval};
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-sample");
    let hits = retrieval::search(&dir, "How long do I boil water to make it safe?", 3);
    assert!(!hits.is_empty());
    assert_eq!(hits[0].heading, "Boiling water");
    assert!(prompt::build("x", &hits).unwrap().contains("rolling boil"));
}

#[test]
fn unknown_question_gets_not_found_not_a_guess() {
    use cindrix_core::{prompt, retrieval};
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-sample");
    let hits = retrieval::search(&dir, "How do I tune a violin?", 3);
    assert!(prompt::build("x", &hits).is_none());
}

#[test]
fn shipped_cards_parse_and_are_drafts() {
    use cindrix_core::cards;
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/cards");
    let cs = cards::load_dir(&dir);
    assert!(cs.len() >= 3);
    assert!(cs.iter().all(|c| !c.is_reviewed()), "no card may claim review before a clinician signs off");
    assert!(cs.iter().any(|c| c.id == cindrix_core::safety::FIXED_REPLY_CARD));
}
