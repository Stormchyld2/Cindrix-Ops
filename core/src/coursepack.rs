//! Course packs: an Academy course (its folders, lesson files and pictures) as ONE text file, so a
//! course can be handed to another household and dropped in. Pictures are stored as base64 lines.
//!
//! Format: the first line is `CINDRIX-COURSE-PACK 1`. Then each file starts with a line
//! `@@@ file <path>` (text) or `@@@ binary <path>` (picture, base64), followed by its content.
//! Paths are below the Academy folder, such as `my-course/course.md` or `my-course/m1/knot.png`.
//!
//! Nothing is written until the whole pack has been checked: plain path pieces only (no `..`, no
//! drive letters, no hidden names), allowed file types only, size limits, and no course folder that
//! is already installed (an installed course is never overwritten). Standard library only.

use crate::fsio;
use std::fs;
use std::path::{Path, PathBuf};

pub const HEADER: &str = "CINDRIX-COURSE-PACK 1";
const MARK: &str = "@@@ ";
const MAX_FILES: usize = 600;
const MAX_TOTAL: usize = 24_000_000;
const MAX_DEPTH: usize = 4;
const TEXT_TYPES: &[&str] = &["md", "txt"];
const PICTURE_TYPES: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg"];

/// One file of a pack: the path below the Academy folder (with `/`) and its bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub bytes: Vec<u8>,
}

fn ext_of(path: &str) -> String {
    path.rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default()
}

fn is_text(path: &str) -> bool {
    TEXT_TYPES.contains(&ext_of(path).as_str())
}

/// A path that is safe to create below the Academy folder.
pub fn path_ok(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() < 2 || parts.len() > MAX_DEPTH + 1 {
        return false; // a file always sits inside a course folder
    }
    let piece_ok = |p: &&str| {
        !p.is_empty() && p.len() <= 80 && !p.starts_with('.') && p.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ' | '(' | ')'))
    };
    if !parts.iter().all(piece_ok) {
        return false;
    }
    let e = ext_of(path);
    TEXT_TYPES.contains(&e.as_str()) || PICTURE_TYPES.contains(&e.as_str())
}

/// Every course folder in `academy_dir` as pack entries (text and pictures, nothing else).
pub fn collect(academy_dir: &Path) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(academy_dir) else { return Ok(out) };
    let mut tops: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    tops.sort();
    for top in tops {
        let name = top.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        walk(&top, &name, 1, &mut out)?;
    }
    Ok(out)
}

fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<Entry>) -> Result<(), String> {
    let Ok(rd) = fs::read_dir(dir) else { return Ok(()) };
    let mut items: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    items.sort();
    for p in items {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let here = format!("{}/{}", rel, name);
        if p.is_dir() {
            if depth < MAX_DEPTH && !name.starts_with('.') && !name.starts_with('_') {
                walk(&p, &here, depth + 1, out)?;
            }
        } else if p.is_file() && path_ok(&here) {
            let bytes = fs::read(&p).map_err(|e| format!("Could not read {}: {}", here, e))?;
            out.push(Entry { path: here, bytes });
        }
    }
    Ok(())
}

pub fn pack(entries: &[Entry]) -> String {
    let mut s = String::from(HEADER);
    s.push('\n');
    for e in entries {
        if is_text(&e.path) {
            s.push_str(&format!("{}file {}\n", MARK, e.path));
            s.push_str(String::from_utf8_lossy(&e.bytes).trim_end_matches('\n'));
            s.push('\n');
        } else {
            s.push_str(&format!("{}binary {}\n", MARK, e.path));
            let b64 = crate::academy::base64(&e.bytes);
            for chunk in b64.as_bytes().chunks(76) {
                s.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                s.push('\n');
            }
        }
    }
    s
}

/// Plain base64 text back into bytes (None when it is not valid base64).
pub fn unbase64(text: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let clean: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    if clean.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for q in clean.chunks(4) {
        let pad = q.iter().rev().take_while(|c| **c == b'=').count();
        if pad > 2 || q[..4 - pad].contains(&b'=') {
            return None;
        }
        let mut n = 0u32;
        for (i, c) in q.iter().enumerate() {
            n = n << 6 | if i >= 4 - pad { 0 } else { val(*c)? };
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

/// Read and check a pack. Any problem refuses the whole pack, with a plain sentence.
pub fn unpack(text: &str) -> Result<Vec<Entry>, String> {
    let mut lines = text.lines();
    if lines.next().map(|l| l.trim_start_matches('\u{feff}').trim()) != Some(HEADER) {
        return Err("This is not a course pack file.".into());
    }
    let mut out: Vec<Entry> = Vec::new();
    let mut cur: Option<(bool, String, Vec<&str>)> = None;
    let mut total = 0usize;
    fn finish(cur: Option<(bool, String, Vec<&str>)>, out: &mut Vec<Entry>, total: &mut usize) -> Result<(), String> {
        let Some((binary, path, body)) = cur else { return Ok(()) };
        let joined = body.join("\n");
        let bytes = if binary {
            unbase64(&joined).ok_or_else(|| format!("The picture {} in the pack is damaged.", path))?
        } else {
            let mut b = joined.into_bytes();
            b.push(b'\n');
            b
        };
        *total += bytes.len();
        if *total > MAX_TOTAL || out.len() >= MAX_FILES {
            return Err("The pack is too big to be a course.".into());
        }
        if out.iter().any(|e| e.path.eq_ignore_ascii_case(&path)) {
            return Err(format!("{} is in the pack twice.", path));
        }
        out.push(Entry { path, bytes });
        Ok(())
    }
    for l in lines {
        if let Some(rest) = l.strip_prefix(MARK) {
            finish(cur.take(), &mut out, &mut total)?;
            let (kind, path) = rest.split_once(' ').ok_or("A file line in the pack is damaged.")?;
            let path = path.trim().to_string();
            if !path_ok(&path) {
                return Err(format!("\"{}\" is not a usable file name for a course.", path));
            }
            let binary = match kind {
                "file" => false,
                "binary" => true,
                _ => return Err("A file line in the pack is damaged.".into()),
            };
            if binary == is_text(&path) {
                return Err(format!("\"{}\" is the wrong kind of file for its place in the pack.", path));
            }
            cur = Some((binary, path, Vec::new()));
        } else if let Some((_, _, body)) = cur.as_mut() {
            body.push(l);
        }
    }
    finish(cur.take(), &mut out, &mut total)?;
    if out.is_empty() {
        return Err("The pack has no files in it.".into());
    }
    Ok(out)
}

/// The course folders a pack would add.
pub fn courses_in(entries: &[Entry]) -> Vec<String> {
    let mut v: Vec<String> = entries.iter().filter_map(|e| e.path.split('/').next().map(|s| s.to_string())).collect();
    v.sort();
    v.dedup();
    v
}

/// Install a checked pack into `academy_dir`. A course folder that already exists is refused
/// (nothing is written at all), so an installed course is never overwritten. Returns the courses added.
pub fn install(entries: &[Entry], academy_dir: &Path) -> Result<Vec<String>, String> {
    let courses = courses_in(entries);
    for c in &courses {
        if academy_dir.join(c).exists() {
            return Err(format!("You already have a course called \"{}\". Rename or remove it first, then install the pack again.", c));
        }
    }
    for e in entries {
        if !path_ok(&e.path) {
            return Err("The pack holds a file name that cannot be used.".into());
        }
        let dest = academy_dir.join(e.path.replace('/', std::path::MAIN_SEPARATOR_STR));
        fsio::write_atomic(&dest, &e.bytes).map_err(|x| format!("Could not save {}: {}", e.path, x))?;
    }
    Ok(courses)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-cp-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_course_with_a_picture_goes_into_a_pack_and_comes_out_the_same() {
        let a = tmp("src");
        fs::create_dir_all(a.join("knots/m1")).unwrap();
        fs::write(a.join("knots/course.md"), "---\ntitle: Knots\n---\nAbout knots.\n").unwrap();
        fs::write(a.join("knots/m1/l1.md"), "# Reef knot\n\n![the knot](knot.png)\n").unwrap();
        let png = vec![0x89, b'P', b'N', b'G', 0, 255, 10, 13, 7];
        fs::write(a.join("knots/m1/knot.png"), &png).unwrap();
        fs::write(a.join("knots/m1/notes.exe"), b"nope").unwrap(); // not a course file type
        let entries = collect(&a).unwrap();
        assert_eq!(entries.len(), 3);
        let text = pack(&entries);
        assert!(text.starts_with(HEADER));
        let back = unpack(&text).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back.iter().find(|e| e.path == "knots/m1/knot.png").unwrap().bytes, png);
        assert_eq!(courses_in(&back), vec!["knots"]);
        let b = tmp("dest");
        assert_eq!(install(&back, &b).unwrap(), vec!["knots"]);
        assert_eq!(fs::read(b.join("knots/m1/knot.png")).unwrap(), png);
        assert!(fs::read_to_string(b.join("knots/course.md")).unwrap().contains("About knots."));
        // installing again is refused and changes nothing
        fs::write(b.join("knots/course.md"), "mine").unwrap();
        assert!(install(&back, &b).unwrap_err().contains("already have"));
        assert_eq!(fs::read_to_string(b.join("knots/course.md")).unwrap(), "mine");
        let _ = fs::remove_dir_all(&a);
        let _ = fs::remove_dir_all(&b);
    }

    #[test]
    fn a_bad_pack_is_refused_whole() {
        let ok = format!("{}\n{}file c/course.md\nhello\n", HEADER, MARK);
        assert!(unpack(&ok).is_ok());
        assert!(unpack("hello").is_err());
        assert!(unpack(HEADER).is_err());
        for bad in ["../x/course.md", "c/../../x.md", "C:/x/y.md", "/abs/x.md", "c/.hidden.md", "c/run.exe", "course.md", "c/a/b/c/d/e/f.md"] {
            let t = format!("{}\n{}file {}\nx\n", HEADER, MARK, bad);
            assert!(unpack(&t).is_err(), "{} should be refused", bad);
        }
        // a picture marked as text, text marked as binary, a damaged picture, a repeated file
        assert!(unpack(&format!("{}\n{}file c/a.png\nx\n", HEADER, MARK)).is_err());
        assert!(unpack(&format!("{}\n{}binary c/a.md\nAAAA\n", HEADER, MARK)).is_err());
        assert!(unpack(&format!("{}\n{}binary c/a.png\n!!!\n", HEADER, MARK)).is_err());
        assert!(unpack(&format!("{}\n{}file c/a.md\nx\n{}file c/A.md\ny\n", HEADER, MARK, MARK)).unwrap_err().contains("twice"));
    }

    #[test]
    fn base64_round_trips_every_length() {
        for n in 0..40usize {
            let data: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(unbase64(&crate::academy::base64(&data)).unwrap(), data, "length {}", n);
        }
        assert!(unbase64("AB=D").is_none());
        assert!(unbase64("ABC").is_none());
    }
}
