//! Follow-up questions. "How long does it last?" means nothing on its own: the words that find the right page ("preserved
//! meat") were in the question before. This notices a question that leans on an earlier one and carries the earlier question's
//! words into the search, and lets the model see the earlier question too. No model is needed to do it.
//!
//! A question that stands on its own is left exactly as it was, so a new subject never gets the old one mixed in.

use crate::retrieval::tokens;

/// Words that point back at something said before.
const POINTERS: &[&str] = &[
    "it", "its", "they", "them", "their", "theirs", "those", "these", "that", "this", "he", "she", "him", "her", "there",
    "same", "also", "another", "else", "instead", "again",
];

/// Ways a follow-up often begins.
const OPENERS: &[&str] = &["and ", "what about", "how about", "what if", "but ", "so ", "or "];

/// The most earlier questions looked at, and the longest a question may be (a long one is cut, not trusted).
const MAX_EARLIER: usize = 2;
const MAX_LEN: usize = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    /// What to search the library with.
    pub search: String,
    /// What the model and the safety check are shown as the question: the question itself, plus the earlier one for a follow-up.
    pub asked: String,
    pub follow_up: bool,
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_LEN).collect()
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_string()).collect()
}

/// Does this question lean on an earlier one? It has a pointer word, begins like a follow-up, or has too few subject words
/// of its own to search with.
pub fn is_follow_up(question: &str) -> bool {
    let q = clean(question).to_lowercase();
    if q.is_empty() { return false; }
    if OPENERS.iter().any(|o| q.starts_with(o)) { return true; }
    let w = words(&q);
    if w.iter().any(|x| POINTERS.contains(&x.as_str())) { return true; }
    tokens(&q).len() < 2
}

/// `earlier` is the person's own earlier questions in this chat, oldest first.
pub fn resolve(question: &str, earlier: &[String]) -> Resolved {
    let q = clean(question);
    let prev: Vec<String> = earlier.iter().map(|e| clean(e)).filter(|e| !e.is_empty()).rev().take(MAX_EARLIER).collect();
    if prev.is_empty() || !is_follow_up(&q) {
        return Resolved { search: q.clone(), asked: q, follow_up: false };
    }
    // search words: this question's, then the most recent earlier question's (once each)
    let mut seen: Vec<String> = Vec::new();
    let mut search = String::new();
    for w in tokens(&q).into_iter().chain(tokens(&prev[0])) {
        if !seen.contains(&w) {
            seen.push(w.clone());
            if !search.is_empty() { search.push(' '); }
            search.push_str(&w);
        }
    }
    let asked = format!("{}\n(This follows an earlier question in the same chat: \"{}\")", q, prev[0]);
    Resolved { search, asked, follow_up: true }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earlier(v: &[&str]) -> Vec<String> { v.iter().map(|s| s.to_string()).collect() }

    #[test]
    fn a_question_that_leans_on_the_last_one_borrows_its_words() {
        let r = resolve("How long does it last?", &earlier(&["What is the best way to preserve meat?"]));
        assert!(r.follow_up);
        assert!(r.search.contains("preserve") && r.search.contains("meat") && r.search.contains("last"));
        assert!(r.asked.starts_with("How long does it last?"));
        assert!(r.asked.contains("preserve meat"));
    }

    #[test]
    fn a_question_that_stands_alone_is_left_alone() {
        let r = resolve("How do I make water safe to drink?", &earlier(&["What is the best way to preserve meat?"]));
        assert!(!r.follow_up);
        assert_eq!(r.search, "How do I make water safe to drink?");
        assert_eq!(r.asked, r.search);
    }

    #[test]
    fn the_first_question_of_a_chat_has_nothing_to_follow() {
        let r = resolve("What about babies?", &[]);
        assert!(!r.follow_up);
        assert_eq!(r.asked, "What about babies?");
    }

    #[test]
    fn short_and_opening_phrases_count_as_follow_ups() {
        assert!(is_follow_up("And for a baby?"));
        assert!(is_follow_up("What about winter?"));
        assert!(is_follow_up("Why?"));
        assert!(!is_follow_up("Boil water for one minute"));
        assert!(!is_follow_up(""));
    }

    #[test]
    fn only_the_most_recent_earlier_question_is_used_and_words_are_not_repeated() {
        let r = resolve("How much of that do we need?", &earlier(&["How do I store rice?", "How long does stored rice last?"]));
        assert!(r.follow_up);
        assert!(r.asked.contains("How long does stored rice last?"));
        assert!(!r.asked.contains("How do I store rice?"));
        let n = r.search.split(' ').filter(|w| *w == "rice").count();
        assert_eq!(n, 1);
    }

    #[test]
    fn long_or_messy_questions_are_cut_and_tidied() {
        let long = "x ".repeat(500);
        let r = resolve("and that?", &earlier(&[&long]));
        assert!(r.asked.chars().count() < 450);
        let r2 = resolve("  how   long  does it last  ", &earlier(&["preserve   meat"]));
        assert!(r2.asked.starts_with("how long does it last"));
    }

    #[test]
    fn the_safety_check_sees_the_earlier_question_too() {
        // "how much of it" after a question about insulin must not slip through as a harmless short question
        let r = resolve("How much of it should I take?", &earlier(&["What is insulin?"]));
        assert!(r.follow_up);
        assert!(crate::safety::gate(&r.asked).is_some());
    }
}
