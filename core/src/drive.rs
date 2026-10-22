//! Finding the Cindrix drive. The drive letter (or mount point) changes from one computer to
//! the next, so nothing is remembered by letter. A Cindrix drive carries a small marker file,
//! `cindrix-drive.json`, in its top folder (or in a `Cindrix` folder there). We look for it.

use std::fs;
use std::path::{Path, PathBuf};

pub const MARKER: &str = "cindrix-drive.json";

#[derive(Debug, Clone, PartialEq)]
pub struct DriveInfo {
    /// The folder that holds engines/, models/, library/ and user/.
    pub root: PathBuf,
    pub name: String,
    /// "go", "vault", "hub", or "" when the marker does not say.
    pub edition: String,
    /// What the program remembers the drive by (instead of its drive letter, which changes).
    /// The marker's `id` field, which for the owner's drives is simply `vault`, `go` or `hub`.
    /// A marker without one is known by its edition word, and failing that by `cindrix`.
    pub id: String,
}

impl DriveInfo {
    /// Which product this drive is. A marker that says nothing counts as Go.
    pub fn product(&self) -> crate::product::Product {
        crate::product::Product::from_edition(&self.edition)
    }
}

/// Pull a string value out of the marker (a tiny flat JSON object). Not a general JSON parser:
/// the marker is written by us, and a damaged one just yields empty fields.
fn field(json: &str, key: &str) -> String {
    let pat = format!("\"{}\"", key);
    let Some(i) = json.find(&pat) else { return String::new() };
    let rest = &json[i + pat.len()..];
    let Some(c) = rest.find(':') else { return String::new() };
    let rest = rest[c + 1..].trim_start();
    let Some(rest) = rest.strip_prefix('"') else { return String::new() };
    let mut out = String::new();
    let mut esc = false;
    for ch in rest.chars() {
        if esc {
            out.push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else if ch == '"' {
            break;
        } else {
            out.push(ch);
        }
    }
    out
}

/// A drive's id: lower case letters, digits, `-` and `_` only. Falls back to the edition word,
/// then to `cindrix`, so every drive has one.
pub fn clean_id(raw: &str, edition: &str) -> String {
    let tidy = |s: &str| -> String {
        s.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .take(40)
            .collect()
    };
    let id = tidy(raw);
    if !id.is_empty() {
        return id;
    }
    let id = tidy(edition);
    if !id.is_empty() { id } else { "cindrix".to_string() }
}

/// Is `dir` itself, or its `Cindrix` subfolder, a Cindrix drive?
pub fn check(dir: &Path) -> Option<DriveInfo> {
    for root in [dir.to_path_buf(), dir.join("Cindrix")] {
        let marker = root.join(MARKER);
        if let Ok(text) = fs::read_to_string(&marker) {
            let name = field(&text, "name");
            let edition = field(&text, "edition").to_lowercase();
            return Some(DriveInfo {
                name: if name.is_empty() { "Cindrix drive".to_string() } else { name },
                id: clean_id(&field(&text, "id"), &edition),
                edition,
                root,
            });
        }
    }
    None
}

/// Every place a removable drive might appear on this computer.
pub fn candidate_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if cfg!(windows) {
        for letter in b'A'..=b'Z' {
            let p = PathBuf::from(format!("{}:\\", letter as char));
            if p.exists() {
                v.push(p);
            }
        }
    } else {
        for base in ["/media", "/run/media", "/mnt", "/Volumes"] {
            let base = PathBuf::from(base);
            let Ok(rd) = fs::read_dir(&base) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if !p.is_dir() {
                    continue;
                }
                v.push(p.clone());
                // /media/<user>/<drive> and /run/media/<user>/<drive>
                if let Ok(rd2) = fs::read_dir(&p) {
                    for e2 in rd2.flatten() {
                        if e2.path().is_dir() {
                            v.push(e2.path());
                        }
                    }
                }
            }
        }
    }
    v
}

/// All Cindrix drives among `candidates`, in the order given.
pub fn find_in(candidates: &[PathBuf]) -> Vec<DriveInfo> {
    candidates.iter().filter_map(|c| check(c)).collect()
}

/// All Cindrix drives currently attached to this computer.
pub fn find_all() -> Vec<DriveInfo> {
    find_in(&candidate_roots())
}

/// Which folder is the Cindrix root?
///  1. The folder the program runs from, when it is itself a Cindrix drive: it travels with its data.
///  2. Otherwise the first Cindrix drive found on this computer (Vault is preferred over Go).
///  3. Otherwise the folder the program runs from.
pub fn choose(program_dir: &Path, found: &[DriveInfo]) -> (PathBuf, Option<DriveInfo>) {
    if let Some(d) = check(program_dir) {
        return (d.root.clone(), Some(d));
    }
    let best = found
        .iter()
        .find(|d| d.edition == "vault")
        .or_else(|| found.first())
        .cloned();
    match best {
        Some(d) => (d.root.clone(), Some(d)),
        None => (program_dir.to_path_buf(), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-drive-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn reads_marker_fields() {
        let j = r#"{ "name": "Family \"Vault\"", "edition": "Vault", "id": "x" }"#;
        assert_eq!(field(j, "name"), "Family \"Vault\"");
        assert_eq!(field(j, "edition"), "Vault");
        assert_eq!(field(j, "missing"), "");
        assert_eq!(field("not json", "name"), "");
    }

    #[test]
    fn a_drive_is_known_by_its_marker_id() {
        let a = tmp("id-a");
        fs::write(a.join(MARKER), r#"{"name":"Vault","edition":"vault","id":"vault"}"#).unwrap();
        assert_eq!(check(&a).unwrap().id, "vault");
        // no id: the edition word stands in; no edition either: a plain default
        let b = tmp("id-b");
        fs::write(b.join(MARKER), r#"{"name":"Go","edition":"Go"}"#).unwrap();
        assert_eq!(check(&b).unwrap().id, "go");
        let c = tmp("id-c");
        fs::write(c.join(MARKER), r#"{"name":"Old"}"#).unwrap();
        assert_eq!(check(&c).unwrap().id, "cindrix");
        // odd characters are dropped, so an id is always safe to store in a plain file
        assert_eq!(clean_id("  My Vault!\t\n#2 ", "vault"), "myvault2");
        assert_eq!(clean_id("!!!", "hub"), "hub");
    }

    #[test]
    fn finds_marker_at_top_or_in_cindrix_folder() {
        let a = tmp("a");
        fs::write(a.join(MARKER), r#"{"name":"Go","edition":"go"}"#).unwrap();
        let b = tmp("b");
        fs::create_dir_all(b.join("Cindrix")).unwrap();
        fs::write(b.join("Cindrix").join(MARKER), r#"{"name":"Vault","edition":"vault"}"#).unwrap();
        let c = tmp("c"); // no marker

        let d = check(&a).unwrap();
        assert_eq!(d.root, a);
        assert_eq!(d.edition, "go");
        let d = check(&b).unwrap();
        assert_eq!(d.root, b.join("Cindrix"));
        assert!(check(&c).is_none());
        assert_eq!(find_in(&[c.clone(), a.clone(), b.clone()]).len(), 2);
    }

    #[test]
    fn marker_edition_becomes_a_product() {
        use crate::product::Product;
        for (word, want) in [("go", Product::Go), ("vault", Product::Vault), ("hub", Product::Hub), ("", Product::Go)] {
            let d = tmp(&format!("prod-{}", if word.is_empty() { "none" } else { word }));
            fs::write(d.join(MARKER), format!(r#"{{"name":"x","edition":"{}"}}"#, word)).unwrap();
            assert_eq!(check(&d).unwrap().product(), want, "edition {:?}", word);
        }
    }

    #[test]
    fn a_different_drive_letter_still_finds_it() {
        // Same drive, mounted at two different places on two different "computers".
        let one = tmp("one");
        let two = tmp("two");
        for p in [&one, &two] {
            fs::write(p.join(MARKER), r#"{"name":"Vault","edition":"vault"}"#).unwrap();
        }
        let elsewhere = tmp("elsewhere");
        let (root, d) = choose(&elsewhere, &find_in(&[tmp("none"), one.clone()]));
        assert_eq!(root, one);
        assert!(d.is_some());
        let (root, _) = choose(&elsewhere, &find_in(std::slice::from_ref(&two)));
        assert_eq!(root, two);
    }

    #[test]
    fn program_folder_wins_when_it_is_the_drive() {
        let here = tmp("here");
        fs::write(here.join(MARKER), r#"{"name":"Go","edition":"go"}"#).unwrap();
        let other = tmp("other");
        fs::write(other.join(MARKER), r#"{"name":"Vault","edition":"vault"}"#).unwrap();
        let (root, d) = choose(&here, &find_in(&[other]));
        assert_eq!(root, here);
        assert_eq!(d.unwrap().edition, "go");
    }

    #[test]
    fn vault_preferred_then_falls_back_to_program_folder() {
        let go = tmp("go");
        fs::write(go.join(MARKER), r#"{"name":"Go","edition":"go"}"#).unwrap();
        let vault = tmp("vault");
        fs::write(vault.join(MARKER), r#"{"name":"Vault","edition":"vault"}"#).unwrap();
        let prog = tmp("prog");
        let (root, _) = choose(&prog, &find_in(&[go, vault.clone()]));
        assert_eq!(root, vault);
        let (root, d) = choose(&prog, &[]);
        assert_eq!(root, prog);
        assert!(d.is_none());
    }
}
