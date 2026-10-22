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
            let hidden = p.file_name().map(|n| n.to_string_lossy().starts_with('_') || n.to_string_lossy().starts_with('.')).unwrap_or(true);
            if !hidden && p.extension().and_then(|x| x.to_str()) == Some("md") {
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

/// A card file that was left out, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub place: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loaded {
    pub cards: Vec<Card>,
    pub problems: Vec<Problem>,
}

/// Cards from several folders, the program's own first. A card with the same file name in a later folder
/// replaces the earlier one, so a person can correct a shipped card by dropping a copy in their own folder.
/// A card that cannot be read, or that names a medicine amount, is left out and listed in `problems`.
pub fn load_dirs(dirs: &[std::path::PathBuf]) -> Loaded {
    let mut out = Loaded::default();
    for dir in dirs {
        let Ok(rd) = fs::read_dir(dir) else { continue };
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).collect();
        files.sort();
        for p in files {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with('_') || name.starts_with('.') || p.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let id = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let place = format!("{}/{}", dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), name);
            let text = match fs::read_to_string(&p) {
                Ok(t) => t,
                Err(e) => {
                    out.problems.push(Problem { place, message: e.to_string() });
                    continue;
                }
            };
            match parse(&id, &text) {
                Ok(c) => {
                    if crate::safety::output_contains_dose(&format!("{} {}", c.title, c.body)) {
                        out.problems.push(Problem { place, message: "It names a medicine amount. Cards never do that. Write \"follow the label\" or \"follow your doctor's written plan\" instead.".into() });
                        continue;
                    }
                    out.cards.retain(|x| x.id != c.id);
                    out.cards.push(c);
                }
                Err(e) => out.problems.push(Problem { place, message: e }),
            }
        }
    }
    out.cards.sort_by(|a, b| a.title.cmp(&b.title));
    out
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
    fn folders_are_merged_later_ones_win_and_bad_cards_are_listed() {
        let d = std::env::temp_dir().join(format!("cindrix-cards-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let (a, b) = (d.join("one"), d.join("two"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("x.md"), "---\ntitle: Old\nkind: medical\n---\nB").unwrap();
        fs::write(b.join("x.md"), "---\ntitle: New\nkind: medical\n---\nB").unwrap();
        fs::write(a.join("_template.md"), "---\ntitle: Template\n---\nB").unwrap();
        fs::write(a.join("broken.md"), "no header").unwrap();
        fs::write(a.join("dose.md"), "---\ntitle: Dose\n---\nGive the child 500 mg of ibuprofen.").unwrap();
        fs::write(a.join("notes.txt"), "ignored").unwrap();
        let l = load_dirs(&[a, b, d.join("missing")]);
        assert_eq!(l.cards.len(), 1);
        assert_eq!(l.cards[0].title, "New");
        assert_eq!(l.problems.len(), 2);
        assert!(l.problems.iter().any(|p| p.place == "one/broken.md"));
        assert!(l.problems.iter().any(|p| p.place == "one/dose.md" && p.message.contains("medicine amount")));
    }

    #[test]
    fn rejects_missing_header() {
        assert!(parse("x", "no header").is_err());
    }
}
