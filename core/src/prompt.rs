//! Builds what the model is asked, and the reply when the library has no answer.

use crate::retrieval::Passage;

pub const NOT_FOUND: &str = "I couldn't find that in the library on this drive, so I won't guess. \
Try different words, or open the Library and browse.";

const SYSTEM: &str = "You are the Cindrix Ops assistant. Answer only from the numbered passages below. \
If the passages do not answer the question, say so. Be short and plain. Number your steps. \
Never give a medication amount or dose, and never tell anyone to change a dose; tell them to use \
their doctor's written plan. For emergencies, say to call 911 first.";

/// None when there are no passages: the caller replies NOT_FOUND and does not call the model.
pub fn build(question: &str, passages: &[Passage]) -> Option<String> {
    if passages.is_empty() {
        return None;
    }
    let mut p = String::from(SYSTEM);
    p.push_str("\n\nPassages:\n");
    for (i, ps) in passages.iter().enumerate() {
        p.push_str(&format!("[{}] ({} · {})\n{}\n\n", i + 1, ps.source, ps.heading, ps.text));
    }
    p.push_str(&format!("Question: {}\nAnswer:", question));
    Some(p)
}

/// Source labels to show under the answer.
pub fn source_labels(passages: &[Passage]) -> Vec<String> {
    let mut v: Vec<String> = passages.iter().map(|p| format!("{} · {}", p.source, p.heading)).collect();
    v.dedup();
    v
}
