//! Builds what the model is asked, and the reply when the library has no answer.

use crate::retrieval::Passage;

pub const NOT_FOUND: &str = "I couldn't find that in the library on this drive, so I won't guess. \
Try different words, or open the Library and browse.";

const SYSTEM: &str = "You are the Cindrix Ops assistant. Answer only from the numbered passages below. \
If the passages do not answer the question, say so. Use plain words. \
Start with one sentence that answers the question directly. Then give the details the passages contain: \
numbered steps for a how-to, and the exact amounts, times, temperatures and warnings as the passages state them. \
Cover every passage that helps, in about 5 to 10 lines, and add [1], [2] after a fact to show which passage it came from. \
Do not add anything the passages do not say. Never give a medication amount or dose, and never tell anyone to change a dose; tell them to use \
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

/// The words each source gave (the first passage under each label), in the same order as `source_refs`,
/// cut to a readable length.
pub fn source_excerpts(passages: &[Passage]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut v: Vec<String> = Vec::new();
    for p in passages {
        let label = format!("{} · {}", p.source, p.heading);
        if seen.contains(&label) {
            continue;
        }
        seen.push(label);
        let t = p.text.trim();
        let mut end = t.len().min(1500);
        while !t.is_char_boundary(end) {
            end -= 1;
        }
        v.push(if end < t.len() { format!("{}...", t[..end].trim_end()) } else { t.to_string() });
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
        assert_eq!(source_excerpts(&[p("Wiki", "Water", "x"), p("Wiki", "Water", "x"), p("Notes", "Boil", "")]).len(), refs.len());
    }
}
