//! Simple keyword search over Markdown files in the library. Used to pick the passages
//! the model may answer from. The real library (Kiwix archives) plugs in behind the same shape.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    pub source: String,
    pub heading: String,
    pub text: String,
    pub score: f64,
}

const STOP: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "is", "are", "do", "does", "how", "what",
    "i", "my", "me", "can", "should", "for", "it", "with", "at", "be", "you", "we", "if", "when",
];

pub fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !STOP.contains(w))
        .map(|w| w.to_string())
        .collect()
}

/// Split a Markdown file into (heading, text) sections.
pub fn sections(md: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut head = String::new();
    let mut buf = String::new();
    for line in md.lines() {
        if line.starts_with('#') {
            if !buf.trim().is_empty() {
                out.push((head.clone(), buf.trim().to_string()));
            }
            head = line.trim_start_matches('#').trim().to_string();
            buf.clear();
        } else {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if !buf.trim().is_empty() {
        out.push((head, buf.trim().to_string()));
    }
    out
}

pub fn score(query: &[String], heading: &str, text: &str) -> f64 {
    let h = tokens(heading);
    let t = tokens(text);
    let mut s = 0.0;
    for q in query {
        if h.contains(q) {
            s += 3.0;
        }
        let hits = t.iter().filter(|w| *w == q).count() as f64;
        s += hits.min(3.0);
    }
    s
}

/// Minimum score for a passage to count as relevant.
pub const MIN_SCORE: f64 = 3.0;

pub fn search(dir: &Path, question: &str, limit: usize) -> Vec<Passage> {
    let q = tokens(question);
    let mut found = Vec::new();
    if q.is_empty() {
        return found;
    }
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let src = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if let Ok(text) = fs::read_to_string(&p) {
                for (head, body) in sections(&text) {
                    let sc = score(&q, &head, &body);
                    if sc >= MIN_SCORE {
                        found.push(Passage { source: src.clone(), heading: head, text: body, score: sc });
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(limit);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sections() {
        let s = sections("# A\none\n## B\ntwo\n");
        assert_eq!(s, vec![("A".to_string(), "one".to_string()), ("B".to_string(), "two".to_string())]);
    }

    #[test]
    fn heading_hits_count_more() {
        let q = tokens("boil water");
        assert!(score(&q, "Boil water", "text") > score(&q, "Other", "boil"));
    }
}
