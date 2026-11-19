//! The Gear list: tents, radios, walking poles and other things that are not food (`user/gear.txt`).
//!
//! It is a separate list from the food Inventory on purpose. Each piece has a name, a kind, a make, a model, a
//! serial number, a place, a count, a condition, the date it was bought and notes. The list starts empty: nothing is
//! added for the person. A file that cannot be read in full is never saved over, and every save is backed up first.

use crate::fsio;
use crate::ops::{esc, next_id, unesc};
use std::path::{Path, PathBuf};

pub const KINDS: &[(&str, &str)] = &[
    ("shelter", "Shelter and sleeping"),
    ("clothing", "Clothing and boots"),
    ("tools", "Tools and repair"),
    ("navigation", "Navigation and walking"),
    ("radio", "Radios and electronics"),
    ("power", "Light and power"),
    ("water", "Water and cooking gear"),
    ("medical", "Medical kits and supplies"),
    ("security", "Security"),
    ("transport", "Vehicles and carrying"),
    ("other", "Other"),
];
pub const CONDITIONS: &[(&str, &str)] = &[("good", "Good"), ("worn", "Worn"), ("repair", "Needs repair"), ("out", "Out of use")];

#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    pub location: String,
    pub qty: u32,
    pub condition: String,
    /// `YYYY-MM-DD` or empty.
    pub bought: String,
    pub notes: String,
}

pub struct Loaded {
    pub items: Vec<Piece>,
    /// Lines that could not be read. While there are any, nothing is saved.
    pub rejected: Vec<String>,
}

pub fn path(user_root: &Path) -> PathBuf {
    user_root.join("gear.txt")
}

/// One line only: tabs and line breaks become single spaces.
fn one_line(s: &str, max: usize) -> String {
    s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect::<String>().trim().to_string()
}

fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect::<String>().trim().to_string()
}

pub fn parse(text: &str) -> Loaded {
    let (mut items, mut rejected) = (Vec::new(), Vec::new());
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let ok = f.len() == 11 && f[0].starts_with('g') && f[0][1..].parse::<u64>().is_ok() && !f[1].trim().is_empty() && f[7].parse::<u32>().is_ok() && !items.iter().any(|x: &Piece| x.id == f[0]);
        if !ok {
            rejected.push(l.to_string());
            continue;
        }
        items.push(Piece { id: f[0].into(), name: unesc(f[1]), kind: f[2].into(), make: unesc(f[3]), model: unesc(f[4]), serial: unesc(f[5]), location: unesc(f[6]), qty: f[7].parse().unwrap(), condition: f[8].into(), bought: f[9].into(), notes: unesc(f[10]) });
    }
    Loaded { items, rejected }
}

pub fn serialize(items: &[Piece]) -> String {
    let mut o = String::from("# format: gear 1\n# id, name, kind, make, model, serial number, place, count, condition, bought, notes\n");
    for p in items {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", p.id, esc(&p.name), p.kind, esc(&p.make), esc(&p.model), esc(&p.serial), esc(&p.location), p.qty, p.condition, p.bought, esc(&p.notes)));
    }
    o
}

pub fn load(user_root: &Path) -> Result<Loaded, String> {
    match fsio::read_to_string(&path(user_root)) {
        Ok(t) => Ok(parse(&t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Loaded { items: vec![], rejected: vec![] }),
        Err(_) => Err("The gear list could not be read.".to_string()),
    }
}

fn store(user_root: &Path, items: &[Piece]) -> Result<(), String> {
    fsio::backup_then_write(&path(user_root), &user_root.join("backups"), serialize(items).as_bytes()).map_err(|e| format!("The gear list could not be saved: {}", e))
}

/// Looks at what was typed and returns the tidy piece, or a plain-words reason it cannot be saved.
fn check(mut p: Piece) -> Result<Piece, String> {
    p.name = one_line(&p.name, 120);
    p.make = one_line(&p.make, 80);
    p.model = one_line(&p.model, 80);
    p.serial = one_line(&p.serial, 80);
    p.location = one_line(&p.location, 120);
    p.notes = clean(&p.notes, 600);
    p.bought = p.bought.trim().to_string();
    if p.name.is_empty() {
        return Err("Give the piece a name, like Tent or Radio.".to_string());
    }
    if !KINDS.iter().any(|k| k.0 == p.kind) {
        p.kind = "other".to_string();
    }
    if !CONDITIONS.iter().any(|c| c.0 == p.condition) {
        p.condition = "good".to_string();
    }
    if p.qty == 0 || p.qty > 9999 {
        return Err("The count must be between 1 and 9999.".to_string());
    }
    if !p.bought.is_empty() && crate::security_review::date_days(&p.bought).is_none() {
        return Err("Enter the date bought as year-month-day, like 2026-05-14, or leave it empty.".to_string());
    }
    Ok(p)
}

/// Add a piece (empty id) or change one. Two pieces may not have the same make, model and serial number.
pub fn save(user_root: &Path, piece: Piece) -> Result<Vec<Piece>, String> {
    let mut l = load(user_root)?;
    if !l.rejected.is_empty() {
        return Err("Some lines in the gear list could not be read, so nothing was saved. Your file has not been changed.".to_string());
    }
    let mut p = check(piece)?;
    if !p.serial.is_empty() && l.items.iter().any(|x| x.id != p.id && x.serial.eq_ignore_ascii_case(&p.serial) && x.make.eq_ignore_ascii_case(&p.make) && x.model.eq_ignore_ascii_case(&p.model)) {
        return Err("A piece with that make, model and serial number is already listed.".to_string());
    }
    if p.id.is_empty() {
        p.id = next_id('g', l.items.iter().map(|x| x.id.clone()));
        l.items.push(p);
    } else {
        match l.items.iter_mut().find(|x| x.id == p.id) {
            Some(slot) => *slot = p,
            None => return Err("That piece is not in the list any more.".to_string()),
        }
    }
    store(user_root, &l.items)?;
    Ok(l.items)
}

pub fn delete(user_root: &Path, ids: &[String]) -> Result<Vec<Piece>, String> {
    let mut l = load(user_root)?;
    if !l.rejected.is_empty() {
        return Err("Some lines in the gear list could not be read, so nothing was removed. Your file has not been changed.".to_string());
    }
    l.items.retain(|x| !ids.contains(&x.id));
    store(user_root, &l.items)?;
    Ok(l.items)
}

/// Pieces that match every word typed (name, kind, make, model, serial number, place, notes), newest id last.
pub fn search(items: &[Piece], query: &str) -> Vec<Piece> {
    let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
    items
        .iter()
        .filter(|p| {
            let hay = format!("{} {} {} {} {} {} {}", p.name, KINDS.iter().find(|k| k.0 == p.kind).map(|k| k.1).unwrap_or(""), p.make, p.model, p.serial, p.location, p.notes).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-gear-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn piece(name: &str, make: &str, model: &str, serial: &str) -> Piece {
        Piece { id: String::new(), name: name.into(), kind: "shelter".into(), make: make.into(), model: model.into(), serial: serial.into(), location: "Garage".into(), qty: 1, condition: "good".into(), bought: String::new(), notes: String::new() }
    }

    #[test]
    fn the_list_starts_empty_and_nothing_is_added_for_the_person() {
        let d = tmp("empty");
        assert!(load(&d).unwrap().items.is_empty());
        assert!(!path(&d).exists(), "no file is made until the person saves something");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_piece_keeps_every_field_through_a_save_and_a_reload() {
        let d = tmp("round");
        let mut p = piece("Tent", "Acme", "T-2 Trail", "SN\t12\n34");
        p.notes = "Poles in the\nblue bag".into();
        p.bought = "2026-05-14".into();
        p.qty = 2;
        let items = save(&d, p).unwrap();
        assert_eq!(items[0].id, "g1");
        let back = load(&d).unwrap();
        assert!(back.rejected.is_empty());
        assert_eq!(back.items[0].model, "T-2 Trail");
        assert_eq!(back.items[0].serial, "SN 12 34", "tabs and line breaks never get into a serial number");
        assert_eq!(back.items[0].notes, "Poles in the\nblue bag");
        assert_eq!((back.items[0].qty, back.items[0].bought.as_str()), (2, "2026-05-14"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_change_replaces_the_piece_and_ids_are_not_reused() {
        let d = tmp("edit");
        save(&d, piece("Tent", "A", "1", "s1")).unwrap();
        save(&d, piece("Radio", "B", "2", "s2")).unwrap();
        let mut r = load(&d).unwrap().items[1].clone();
        r.location = "Go bag".into();
        save(&d, r).unwrap();
        assert_eq!(load(&d).unwrap().items[1].location, "Go bag");
        delete(&d, &["g2".to_string()]).unwrap();
        let again = save(&d, piece("Poles", "C", "3", "s3")).unwrap();
        assert_eq!(again.last().unwrap().id, "g2", "the next id follows the highest one left");
        let mut ghost = piece("Ghost", "", "", "");
        ghost.id = "g99".into();
        assert!(save(&d, ghost).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_same_make_model_and_serial_is_listed_only_once_but_blank_serials_are_fine() {
        let d = tmp("dup");
        save(&d, piece("Radio", "Baofeng", "UV-5R", "A100")).unwrap();
        assert!(save(&d, piece("Radio again", "baofeng", "uv-5r", "a100")).unwrap_err().contains("already listed"));
        save(&d, piece("Radio 2", "Baofeng", "UV-5R", "A101")).unwrap();
        save(&d, piece("Cord", "", "", "")).unwrap();
        save(&d, piece("Cord", "", "", "")).unwrap();
        assert_eq!(load(&d).unwrap().items.len(), 4);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn bad_entries_are_refused_in_plain_words() {
        let d = tmp("bad");
        assert!(save(&d, piece("  ", "", "", "")).unwrap_err().contains("name"));
        let mut p = piece("Tent", "", "", "");
        p.qty = 0;
        assert!(save(&d, p.clone()).unwrap_err().contains("count"));
        p.qty = 1;
        p.bought = "last spring".into();
        assert!(save(&d, p.clone()).unwrap_err().contains("year-month-day"));
        p.bought = String::new();
        p.kind = "nonsense".into();
        p.condition = "nonsense".into();
        save(&d, p).unwrap();
        let got = &load(&d).unwrap().items[0];
        assert_eq!((got.kind.as_str(), got.condition.as_str()), ("other", "good"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_file_with_unreadable_lines_is_never_saved_over() {
        let d = tmp("damaged");
        fs::write(path(&d), "# format: gear 1\ng1\tTent\tshelter\t\t\t\tGarage\t1\tgood\t\t\nthis line is damaged\n").unwrap();
        let before = fs::read_to_string(path(&d)).unwrap();
        assert_eq!(load(&d).unwrap().rejected.len(), 1);
        assert!(save(&d, piece("Radio", "", "", "")).is_err());
        assert!(delete(&d, &["g1".to_string()]).is_err());
        assert_eq!(fs::read_to_string(path(&d)).unwrap(), before);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn every_save_keeps_a_backup_of_the_old_file() {
        let d = tmp("backup");
        save(&d, piece("Tent", "", "", "")).unwrap();
        save(&d, piece("Radio", "", "", "")).unwrap();
        assert!(d.join("backups").exists(), "the earlier file was copied into backups first");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn search_matches_every_word_across_the_fields() {
        let d = tmp("search");
        let mut r = piece("Radio", "Baofeng", "UV-5R", "A100");
        r.kind = "radio".into();
        save(&d, r).unwrap();
        save(&d, piece("Tent", "Acme", "T-2", "Z9")).unwrap();
        let all = load(&d).unwrap().items;
        assert_eq!(search(&all, "baofeng a100").len(), 1);
        assert_eq!(search(&all, "garage").len(), 2);
        assert_eq!(search(&all, "electronics").len(), 1, "the kind's wording is searched too");
        assert_eq!(search(&all, "tent baofeng").len(), 0);
        assert_eq!(search(&all, "").len(), 2);
        let _ = fs::remove_dir_all(&d);
    }
}
