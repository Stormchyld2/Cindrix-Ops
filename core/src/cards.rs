//! Emergency cards: Markdown files with a small header. No model needed to show them.
//!
//! File format:
//! ```text
//! ---
//! title: Low blood sugar
//! kind: emergency
//! status: draft
//! reviewer:
//! reviewed:
//! ---
//! Body in Markdown.
//! ```

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub kind: String,
    /// "draft" until a clinician has reviewed it
    pub status: String,
    pub reviewer: String,
    pub reviewed: String,
    pub body: String,
}

impl Card {
    pub fn is_reviewed(&self) -> bool {
        self.status == "reviewed" && !self.reviewer.is_empty() && !self.reviewed.is_empty()
    }
}

pub fn parse(id: &str, text: &str) -> Result<Card, String> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or("card must start with ---")?;
    let (head, body) = rest.split_once("\n---\n").ok_or("card header is not closed with ---")?;
    let mut card = Card {
        id: id.to_string(), title: String::new(), kind: String::new(), status: "draft".into(),
        reviewer: String::new(), reviewed: String::new(), body: body.trim().to_string(),
    };
    for line in head.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().to_string();
            match k.trim() {
                "title" => card.title = v,
                "kind" => card.kind = v,
                "status" => card.status = v,
                "reviewer" => card.reviewer = v,
                "reviewed" => card.reviewed = v,
                _ => {}
            }
        }
    }
    if card.title.is_empty() {
        return Err("card has no title".into());
    }
    // Never present a card as reviewed unless it names who and when.
    if card.status == "reviewed" && !card.is_reviewed() {
        card.status = "draft".into();
    }
    Ok(card)
}

pub fn load_dir(dir: &Path) -> Vec<Card> {
    let mut cards = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("md") {
                let id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Ok(t) = fs::read_to_string(&p) {
                    if let Ok(c) = parse(&id, &t) {
                        cards.push(c);
                    }
                }
            }
        }
    }
    cards.sort_by(|a, b| a.title.cmp(&b.title));
    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_card() {
        let c = parse("x", "---\ntitle: T\nkind: emergency\nstatus: draft\n---\nBody").unwrap();
        assert_eq!(c.title, "T");
        assert_eq!(c.body, "Body");
        assert!(!c.is_reviewed());
    }

    #[test]
    fn reviewed_without_names_falls_back_to_draft() {
        let c = parse("x", "---\ntitle: T\nstatus: reviewed\n---\nB").unwrap();
        assert_eq!(c.status, "draft");
    }

    #[test]
    fn rejects_missing_header() {
        assert!(parse("x", "no header").is_err());
    }
}
