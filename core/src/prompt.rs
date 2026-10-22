//! Builds what the model is asked, and the reply when the library has no answer.

use crate::retrieval::Passage;

pub const NOT_FOUND: &str = "I couldn't find that in the library on this drive, so I won't guess. \
Try different words, or open the Library and browse.";

const SYSTEM: &str = "You are the Cindrix Ops assistant. Answer only from the numbered passages below. \
If the passages do not answer the question, say so. Be short and plain. Number your steps. \
Never give a medication amount or dose, and never tell anyone to change a dose; tell them to use \
their doctor's written plan. For emergencies, say to call 911 first.";

/// The system and user parts of the question for the model.
/// None when there are no passages: the caller replies NOT_FOUND and does not call the model.
pub fn build_parts(question: &str, passages: &[Passage]) -> Option<(String, String)> {
    if passages.is_empty() {
        return None;
    }
    let mut user = String::from("Passages:\n");
    for (i, ps) in passages.iter().enumerate() {
        user.push_str(&format!("[{}] ({} · {})\n{}\n\n", i + 1, ps.source, ps.heading, ps.text));
    }
    user.push_str(&format!("Question: {}", question));
    Some((SYSTEM.to_string(), user))
}

/// Each source label with where to open it ("" when it has no page to open), without repeats.
pub fn source_refs(passages: &[Passage]) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for p in passages {
        let label = format!("{} · {}", p.source, p.heading);
        if !v.iter().any(|(l, _)| *l == label) {
            v.push((label, p.link.clone()));
        }
    }
    v
}

/// Source labels to show under the answer.
pub fn source_labels(passages: &[Passage]) -> Vec<String> {
    let mut v: Vec<String> = passages.iter().map(|p| format!("{} · {}", p.source, p.heading)).collect();
    v.dedup();
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_source_keeps_its_link_and_repeats_are_dropped() {
        use crate::retrieval::Passage;
        let p = |s: &str, h: &str, l: &str| Passage { source: s.into(), heading: h.into(), text: String::new(), score: 5.0, link: l.into() };
        let refs = source_refs(&[p("Wiki", "Water", "/content/wiki/A/Water"), p("Wiki", "Water", "/content/wiki/A/Water"), p("Notes", "Boil", "")]);
        assert_eq!(refs, vec![("Wiki · Water".to_string(), "/content/wiki/A/Water".to_string()), ("Notes · Boil".to_string(), String::new())]);
    }
}
