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
//! image: morel.jpg
//! ---
//! Body in Markdown.
//! ```
//!
//! `image:` is optional. It names a picture file (just the file name) that is looked for in the `images`
//! folders (`content/images`, the drive's `images`, the person's `user/images`). The card shows the picture
//! only when the line is there and the file is found; otherwise it looks exactly as it did before.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub id: String,
    pub title: String,
    pub kind: String,
    /// "draft" until a clinician has reviewed it
    pub status: String,
    pub reviewer: String,
    pub reviewed: String,
    /// File name of the card's picture, or empty. Never a path.
    pub image: String,
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
        reviewer: String::new(), reviewed: String::new(), image: String::new(), body: body.trim().to_string(),
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
                "image" => {
                    if image_name_ok(&v) {
                        card.image = v;
                    }
                }
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

/// Picture types a card may name. SVG is left out on purpose: photos only.
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];

/// A picture name is a plain file name with a known picture ending: no folders, no `..`, nothing hidden.
pub fn image_name_ok(name: &str) -> bool {
    if name.is_empty() || name.len() > 120 || name.starts_with('.') {
        return false;
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains(':') || name.contains('\0') {
        return false;
    }
    match name.rsplit_once('.') {
        Some((stem, ext)) => !stem.is_empty() && IMAGE_EXT.iter().any(|e| e.eq_ignore_ascii_case(ext)),
        None => false,
    }
}

/// The picture file for a card: the named file in the first folder that has it, looking in the person's
/// own folder first (the last in the list), so a person can replace a shipped picture with their own.
/// None when the name is not a plain picture name or no folder has the file.
pub fn image_file(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    if !image_name_ok(name) {
        return None;
    }
    dirs.iter().rev().map(|d| d.join(name)).find(|p| p.is_file())
}

/// The picture as a `data:` address the screen can show without a server. None when it is missing, too big
/// or not a known picture type, so the card is simply shown without it.
pub fn image_data(dirs: &[PathBuf], name: &str) -> Option<String> {
    crate::academy::image_data_url(&image_file(dirs, name)?)
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
    fn a_card_names_its_picture_only_with_a_plain_file_name() {
        let c = parse("x", "---\ntitle: T\nimage: morel.jpg\n---\nB").unwrap();
        assert_eq!(c.image, "morel.jpg");
        let none = parse("x", "---\ntitle: T\n---\nB").unwrap();
        assert_eq!(none.image, "");
        for bad in ["../secret.jpg", "a/b.jpg", "a\\b.jpg", ".hidden.jpg", "morel", "morel.exe", "morel.svg", "C:morel.jpg", ".jpg", ""] {
            assert!(!image_name_ok(bad), "{}", bad);
            let t = format!("---\ntitle: T\nimage: {}\n---\nB", bad);
            assert_eq!(parse("x", &t).unwrap().image, "", "{}", bad);
        }
        assert!(image_name_ok("Morel-2.JPG") && image_name_ok("a.png") && image_name_ok("b.webp"));
    }

    #[test]
    fn the_picture_is_found_only_when_its_file_is_there_and_the_last_folder_wins() {
        let d = std::env::temp_dir().join(format!("cindrix-cardimg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let (a, b) = (d.join("shipped"), d.join("user"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("morel.jpg"), [1u8, 2, 3]).unwrap();
        fs::write(a.join("notes.txt"), "x").unwrap();
        let dirs = vec![a.clone(), b.clone(), d.join("missing")];
        // there: found, and shown as a data address
        assert_eq!(image_file(&dirs, "morel.jpg"), Some(a.join("morel.jpg")));
        assert!(image_data(&dirs, "morel.jpg").unwrap().starts_with("data:image/jpeg;base64,"));
        // no such file, wrong type, or a sneaky name: nothing, so the card shows no picture
        assert_eq!(image_file(&dirs, "ramps.jpg"), None);
        assert_eq!(image_data(&dirs, "ramps.jpg"), None);
        assert_eq!(image_file(&dirs, "notes.txt"), None);
        assert_eq!(image_file(&dirs, "../shipped/morel.jpg"), None);
        assert_eq!(image_file(&[], "morel.jpg"), None);
        // the person's own copy replaces the shipped one
        fs::write(b.join("morel.jpg"), [9u8]).unwrap();
        assert_eq!(image_file(&dirs, "morel.jpg"), Some(b.join("morel.jpg")));
        // a file that is too big is left out rather than loaded
        fs::write(b.join("huge.png"), vec![0u8; 1_600_000]).unwrap();
        assert!(image_file(&dirs, "huge.png").is_some());
        assert_eq!(image_data(&dirs, "huge.png"), None);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn rejects_missing_header() {
        assert!(parse("x", "no header").is_err());
    }
}
