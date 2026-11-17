//! The phone maps (`.cmap` files, docs/32) the Hub keeps in `user/exports/`, so a trusted phone (Cindrix Pocket)
//! can list them and copy one over the Hub's Wi-Fi instead of being carried across by hand.
//!
//! Only a phone that joined with an invite may ask: a map carries the owner's own pins, drawings and areas to avoid
//! (see `share::PERSON_COMMANDS`). A name is checked here so it can never reach another folder.
use crate::share::parse_range;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct PhoneMap {
    /// The file name, including `.cmap`.
    pub name: String,
    pub bytes: u64,
    /// Seconds since 1970 when the file was last written (0 when the drive does not say).
    pub made: u64,
}

/// A plain file name ending in `.cmap`: no folders, no hidden or half-written files.
pub fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && name.ends_with(".cmap")
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.contains("..")
}

/// Every finished phone map in `dir`, newest first.
pub fn list(dir: &Path) -> Vec<PhoneMap> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name_ok(&name) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() || meta.len() < 20 {
            continue;
        }
        let made = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
        out.push(PhoneMap { name, bytes: meta.len(), made });
    }
    out.sort_by(|a, b| b.made.cmp(&a.made).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

/// The file for a name the phone sent, or a plain reason it cannot be given.
pub fn resolve(dir: &Path, name: &str) -> Result<PathBuf, String> {
    if !name_ok(name) {
        return Err("That map is not known.".into());
    }
    let p = dir.join(name);
    if p.is_file() {
        Ok(p)
    } else {
        Err("That map is no longer on the Hub.".into())
    }
}

/// What to send for a download request over a file of `len` bytes: the status, the first and last byte, and
/// the `Content-Range` header when only a part goes. A phone resumes a broken download with a `Range` header.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub status: u16,
    pub start: u64,
    pub end: u64,
    pub content_range: Option<String>,
}

impl Plan {
    pub fn length(&self) -> u64 {
        if self.status == 416 || self.end < self.start { 0 } else { self.end - self.start + 1 }
    }
}

pub fn plan(len: u64, range: Option<&str>) -> Plan {
    match range.map(|h| parse_range(h, len)) {
        Some(Err(())) => Plan { status: 416, start: 0, end: 0, content_range: Some(format!("bytes */{}", len)) },
        Some(Ok(Some((a, b)))) => Plan { status: 206, start: a, end: b, content_range: Some(format!("bytes {}-{}/{}", a, b, len)) },
        _ => Plan { status: 200, start: 0, end: len.saturating_sub(1), content_range: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cx-phonemaps-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn only_plain_cmap_names_are_allowed() {
        assert!(name_ok("Camp-area.cmap"));
        assert!(name_ok("Camp area.cmap"));
        for bad in ["", "x.txt", "../x.cmap", "a/b.cmap", "a\\b.cmap", ".hidden.cmap", "x.cmap.part", "c:x.cmap", "a..b.cmap"] {
            assert!(!name_ok(bad), "{bad}");
        }
        assert!(!name_ok(&format!("{}.cmap", "a".repeat(130))));
    }

    #[test]
    fn the_list_has_finished_maps_only() {
        let d = tmp("list");
        std::fs::write(d.join("a.cmap"), vec![1u8; 40]).unwrap();
        std::fs::write(d.join("b.cmap.part"), vec![1u8; 40]).unwrap();
        std::fs::write(d.join("notes.txt"), "x").unwrap();
        std::fs::write(d.join("tiny.cmap"), "x").unwrap();
        std::fs::create_dir_all(d.join("dir.cmap")).unwrap();
        let l = list(&d);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "a.cmap");
        assert_eq!(l[0].bytes, 40);
        assert!(list(&d.join("missing")).is_empty());
    }

    #[test]
    fn a_name_that_leaves_the_folder_is_refused() {
        let d = tmp("resolve");
        std::fs::write(d.join("a.cmap"), vec![1u8; 40]).unwrap();
        assert!(resolve(&d, "a.cmap").is_ok());
        assert_eq!(resolve(&d, "../a.cmap").unwrap_err(), "That map is not known.");
        assert_eq!(resolve(&d, "gone.cmap").unwrap_err(), "That map is no longer on the Hub.");
    }

    #[test]
    fn a_broken_download_can_carry_on_where_it_stopped() {
        assert_eq!(plan(1000, None), Plan { status: 200, start: 0, end: 999, content_range: None });
        let p = plan(1000, Some("bytes=400-"));
        assert_eq!((p.status, p.start, p.end, p.length()), (206, 400, 999, 600));
        assert_eq!(p.content_range.as_deref(), Some("bytes 400-999/1000"));
        let past = plan(1000, Some("bytes=2000-"));
        assert_eq!((past.status, past.length()), (416, 0));
        assert_eq!(plan(1000, Some("nonsense")).status, 200);
    }
}
