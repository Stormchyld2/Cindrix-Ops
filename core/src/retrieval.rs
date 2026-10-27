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
    /// Where to open the page this came from: an Intel address (`/content/<book>/<page>`), `card:<id>`, or empty.
    pub link: String,
}

const STOP: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "is", "are", "do", "does", "how", "what",
    "i", "my", "me", "can", "should", "for", "it", "with", "at", "be", "you", "we", "if", "when",
];

pub fn tokens(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !STOP.contains(w))
        .map(fold_plural)
        .collect()
}

/// The words of a question, rewritten to the meaning its context points to (see `senses`):
/// "dress a cut" is about bandaging a wound, not dressing timber. Use this for the person's
/// question; `tokens` for page text.
pub fn query_tokens(question: &str) -> Vec<String> {
    crate::senses::expand(tokens(question))
}

/// "burns" and "burn" should match: drop a plural "s" from longer words (not "ss", "us" or "is" endings).
fn fold_plural(w: &str) -> String {
    if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") && !w.ends_with("is") {
        w[..w.len() - 1].to_string()
    } else {
        w.to_string()
    }
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
    let mut all = h.clone();
    all.extend(t.iter().cloned());
    let mut s = 0.0;
    for q in query {
        if h.contains(q) {
            s += 3.0;
        }
        let hits = t.iter().filter(|w| *w == q).count() as f64;
        s += hits.min(3.0);
    }
    s * crate::senses::weight(query, &all)
}

/// The `title:` of a page's front matter (cards and guide pages have one), or empty.
pub fn front_title(md: &str) -> String {
    let mut lines = md.lines();
    if lines.next().map(|l| l.trim()) != Some("---") {
        return String::new();
    }
    for l in lines {
        if l.trim() == "---" {
            break;
        }
        if let Some(t) = l.strip_prefix("title:") {
            return t.trim().trim_matches('"').to_string();
        }
    }
    String::new()
}

/// Minimum score for a passage to count as relevant.
pub const MIN_SCORE: f64 = 3.0;

pub fn search(dir: &Path, question: &str, limit: usize) -> Vec<Passage> {
    let q = query_tokens(question);
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
                let title = front_title(&text);
                for (head, body) in sections(&text) {
                    let sc = score(&q, &format!("{} {}", title, head), &body);
                    if sc >= MIN_SCORE {
                        found.push(Passage { source: src.clone(), heading: head, text: body, score: sc, link: String::new() });
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(limit);
    found
}

/// The relevant sections of one piece of text (a document the person keeps), best first.
pub fn search_text(source: &str, text: &str, question: &str, limit: usize) -> Vec<Passage> {
    let q = query_tokens(question);
    if q.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<Passage> = sections(text)
        .into_iter()
        .filter_map(|(head, body)| {
            let sc = score(&q, &head, &body);
            (sc >= MIN_SCORE).then(|| Passage { source: source.to_string(), heading: head, text: body, score: sc, link: String::new() })
        })
        .collect();
    found.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(limit);
    found
}

#[cfg(test)]
mod tests {
    #[test]
    fn dress_a_cut_is_about_wounds_not_timber() {
        let q = query_tokens("how do i dress a cut");
        assert!(q.contains(&"wound".to_string()) && q.contains(&"bandage".to_string()));
        assert!(!q.contains(&"dress".to_string()));
        assert_eq!(query_tokens("dress a dulcimer"), tokens("dress a dulcimer"));
    }

    #[test]
    fn plural_words_match_the_single_word() {
        assert_eq!(tokens("burns"), tokens("burn"));
        assert_eq!(tokens("a bus"), vec!["bus".to_string()]);
        assert_eq!(tokens("glass"), vec!["glass".to_string()]);
    }

    #[test]
    fn a_page_title_counts_like_a_heading() {
        let md = "---\ntitle: Earthquake\nkind: disaster\n---\n**During: Drop, Cover, Hold On.**\nGet under a sturdy table.\n";
        assert_eq!(front_title(md), "Earthquake");
        assert_eq!(front_title("no front matter"), "");
        let dir = std::env::temp_dir().join(format!("cx-title-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("earthquake.md"), md).unwrap();
        let found = search(&dir, "what to do in an earthquake", 3);
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, "earthquake");
    }

    use super::*;

    #[test]
    fn one_text_is_searched_by_section() {
        let t = "# Pantry\nrice rice rice beans\n# Tools\nhammer";
        let r = search_text("list.txt", t, "how much rice beans", 3);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].heading, "Pantry");
        assert!(search_text("x", t, "", 3).is_empty());
    }

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
