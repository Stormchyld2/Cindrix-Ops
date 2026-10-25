//! Guides: plain Markdown pages in themed folders, read the same way as emergency cards (`cards.rs`).
//! One folder per section: Agro (crop guides, planting calendar, food preservation, livestock, soil and
//! water) and Comms (radio reference). A person can add their own pages in `user/guides/<section>`.
//!
//! Every page is a card file: a small header (`title`, `kind`, `status`) and a body that ends in a
//! `Source:` line. A page that names a medicine amount is left out, as for cards.

use std::path::{Path, PathBuf};

/// The sections a screen may ask for: (folder name, name shown to the person).
pub const SECTIONS: &[(&str, &str)] = &[
    ("crop-guides", "Crop guides"),
    ("planting-calendar", "Planting calendar"),
    ("food-preservation", "Food preservation"),
    ("livestock", "Livestock"),
    ("soil-and-water", "Soil and water"),
    ("radio-reference", "Radio reference"),
    ("home-remedies", "Home remedies"),
];

/// Only these exact names are folders; anything else (such as `../cards`) is refused.
pub fn is_section(id: &str) -> bool {
    SECTIONS.iter().any(|(k, _)| *k == id)
}

pub fn title_of(id: &str) -> Option<&'static str> {
    SECTIONS.iter().find(|(k, _)| *k == id).map(|(_, t)| *t)
}

/// The folders to read for one section, the program's own first so a later copy of the same page wins:
/// the program's content, the drive's `guides`, each other library's, and (unless `include_user` is
/// false, as for another device on the hotspot) the person's own `user/guides`.
pub fn dirs_for(id: &str, program_guides: &Path, drive_root: &Path, other_roots: &[PathBuf], user_dir: &Path, include_user: bool) -> Vec<PathBuf> {
    if !is_section(id) {
        return Vec::new();
    }
    let mut v = vec![program_guides.join(id), drive_root.join("guides").join(id)];
    for o in other_roots {
        v.push(o.join("guides").join(id));
        v.push(o.join("content").join("guides").join(id));
    }
    if include_user {
        v.push(user_dir.join("guides").join(id));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_named_sections_are_folders() {
        assert!(is_section("crop-guides"));
        assert!(is_section("radio-reference"));
        for bad in ["", "../cards", "crop-guides/..", "Crop-Guides", "cards", "crop-guides/x", "..\\user"] {
            assert!(!is_section(bad), "{}", bad);
            assert!(dirs_for(bad, Path::new("c/guides"), Path::new("d"), &[], Path::new("u"), true).is_empty());
        }
        assert_eq!(title_of("soil-and-water"), Some("Soil and water"));
        assert_eq!(title_of("nope"), None);
    }

    #[test]
    fn folders_come_in_order_and_the_users_own_are_left_out_for_guests() {
        let others = vec![PathBuf::from("/other")];
        let own = dirs_for("livestock", Path::new("/c/guides"), Path::new("/d"), &others, Path::new("/d/user"), true);
        assert_eq!(own.first().unwrap(), &PathBuf::from("/c/guides/livestock"));
        assert_eq!(own.last().unwrap(), &PathBuf::from("/d/user/guides/livestock"));
        assert!(own.contains(&PathBuf::from("/other/content/guides/livestock")));
        let guest = dirs_for("livestock", Path::new("/c/guides"), Path::new("/d"), &others, Path::new("/d/user"), false);
        assert!(!guest.iter().any(|p| p.starts_with("/d/user")));
        assert_eq!(guest.len(), own.len() - 1);
    }

    #[test]
    fn a_guide_page_is_read_like_a_card_and_a_dose_is_refused() {
        let tmp = std::env::temp_dir().join(format!("cindrix-guides-{}", std::process::id()));
        let dir = tmp.join("guides/livestock");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hens.md"), "---\ntitle: Keeping hens\nkind: livestock\nstatus: draft\n---\nGive water daily.\n\nSource: a test.").unwrap();
        std::fs::write(dir.join("dose.md"), "---\ntitle: Dose\n---\nGive the hen 500 mg of medicine.\n\nSource: a test.").unwrap();
        let dirs = dirs_for("livestock", &tmp.join("guides"), Path::new("/nowhere"), &[], Path::new("/nowhere"), true);
        let l = crate::cards::load_dirs(&dirs);
        assert_eq!(l.cards.len(), 1);
        assert_eq!(l.cards[0].title, "Keeping hens");
        assert_eq!(l.problems.len(), 1);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
