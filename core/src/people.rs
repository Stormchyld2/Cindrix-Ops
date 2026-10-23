//! A folder for each person under `user/people/<name>/`: their journal, their photos and their comms plan.
//! Shared photos (an album everyone may see) live in `user/shared/photos/`.
//!
//! Everything is a plain file. Names are made safe to use as a folder name first, so nobody can reach another
//! folder with a name like `..\x`. Files are never replaced silently: the earlier copy goes to the person's
//! `backups` folder first (`fsio::backup_then_write`).

use crate::fsio;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_JOURNAL_BYTES: usize = 200 * 1024;
pub const MAX_PLAN_BYTES: usize = 100 * 1024;
pub const MAX_PHOTO_BYTES: usize = 40 * 1024 * 1024;

// ---------- who ----------

/// A name made safe to be a folder name: letters, digits, spaces, `-`, `_` and `.` stay, anything else
/// becomes `_`; no leading dot; at most 40 characters. A name that is empty, or that Windows reserves
/// (CON, NUL, COM1 and so on), is refused.
pub fn folder_name(person: &str) -> Result<String, String> {
    let mut s: String = person
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    s = s.trim_start_matches('.').trim_end_matches(['.', ' ']).to_string();
    if s.chars().count() > 40 {
        s = s.chars().take(40).collect::<String>().trim_end().to_string();
    }
    if s.is_empty() || s.chars().all(|c| c == '_') {
        return Err("A person needs a name first. Add them on the Family page.".into());
    }
    let stem = s.split('.').next().unwrap_or("").to_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.chars().last().map_or(false, |c| c.is_ascii_digit()));
    if reserved {
        return Err("That name cannot be used for a folder. Add a surname or change it a little.".into());
    }
    Ok(s)
}

/// `user/people/<name>`
pub fn person_dir(user_dir: &Path, person: &str) -> Result<PathBuf, String> {
    Ok(user_dir.join("people").join(folder_name(person)?))
}

/// The album everyone may see.
pub fn shared_photos_dir(user_dir: &Path) -> PathBuf {
    user_dir.join("shared").join("photos")
}

/// Photo folders: `shared` is the shared album, `mine` is the person's own.
pub fn photo_dir(user_dir: &Path, scope: &str, person: &str) -> Result<PathBuf, String> {
    match scope {
        "shared" => Ok(shared_photos_dir(user_dir)),
        "mine" => Ok(person_dir(user_dir, person)?.join("photos")),
        _ => Err("That album is not known.".into()),
    }
}

fn io_err(what: &str, e: std::io::Error) -> String {
    format!("{}: {}", what, e)
}

// ---------- journal ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// "YYYY-MM-DD"
    pub date: String,
    pub text: String,
}

/// A date as `YYYY-MM-DD` with real-looking numbers.
pub fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| d[r].chars().all(|c| c.is_ascii_digit());
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let (m, day): (u32, u32) = (d[5..7].parse().unwrap_or(0), d[8..10].parse().unwrap_or(0));
    (1..=12).contains(&m) && (1..=31).contains(&day) && d[0..4] >= *"1900"
}

fn journal_dir(user_dir: &Path, person: &str) -> Result<PathBuf, String> {
    Ok(person_dir(user_dir, person)?.join("journal"))
}

/// All the entries, newest first. An unreadable file is reported, not skipped silently.
pub fn journal_list(user_dir: &Path, person: &str) -> Result<Vec<Entry>, String> {
    let dir = journal_dir(user_dir, person)?;
    let Ok(rd) = fs::read_dir(&dir) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(date) = name.strip_suffix(".md") else { continue };
        if !valid_date(date) {
            continue;
        }
        let text = fs::read_to_string(e.path()).map_err(|er| io_err(&format!("Could not read the entry for {}", date), er))?;
        out.push(Entry { date: date.to_string(), text });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(out)
}

/// Save one day. Empty text removes the day (the old copy is kept in `backups`).
pub fn journal_save(user_dir: &Path, person: &str, date: &str, text: &str) -> Result<(), String> {
    if !valid_date(date) {
        return Err("That date does not look right.".into());
    }
    if text.len() > MAX_JOURNAL_BYTES {
        return Err("That entry is too long. Split it over two days.".into());
    }
    let pdir = person_dir(user_dir, person)?;
    let file = pdir.join("journal").join(format!("{}.md", date));
    let backups = pdir.join("backups");
    if text.trim().is_empty() {
        if file.exists() {
            fs::create_dir_all(&backups).map_err(|e| io_err("Could not make the backups folder", e))?;
            let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            fs::copy(&file, backups.join(format!("{}-{}.md", secs, date))).map_err(|e| io_err("Could not keep a copy first", e))?;
            fs::remove_file(&file).map_err(|e| io_err("Could not remove the entry", e))?;
        }
        return Ok(());
    }
    fsio::backup_then_write(&file, &backups, text.as_bytes()).map_err(|e| io_err("Could not save the entry", e))
}

// ---------- comms plan ----------

/// The words a new comms plan starts with.
pub fn comms_plan_template() -> String {
    "My family's communication plan\n\
\n\
Who carries a radio or a phone:\n\
\n\
First channel (and a second one if it is busy):\n\
\n\
When we check in (for example, the top of every hour for 5 minutes):\n\
\n\
Where we meet if we cannot reach each other (near home, and farther away):\n\
\n\
What we say to mean \"I am safe\", \"I need help\" and \"Come home\":\n\
\n\
The one person outside the area we all call or message:\n\
\n\
Phone numbers and radio call signs:\n"
        .to_string()
}

fn plan_file(user_dir: &Path, person: &str) -> Result<PathBuf, String> {
    Ok(person_dir(user_dir, person)?.join("comms-plan.md"))
}

/// The saved plan, or the template when none is saved yet.
pub fn plan_load(user_dir: &Path, person: &str) -> Result<String, String> {
    match fs::read_to_string(plan_file(user_dir, person)?) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(comms_plan_template()),
        Err(e) => Err(io_err("Could not read your comms plan", e)),
    }
}

pub fn plan_save(user_dir: &Path, person: &str, text: &str) -> Result<(), String> {
    if text.len() > MAX_PLAN_BYTES {
        return Err("The plan is too long.".into());
    }
    let file = plan_file(user_dir, person)?;
    let backups = person_dir(user_dir, person)?.join("backups");
    fsio::backup_then_write(&file, &backups, text.as_bytes()).map_err(|e| io_err("Could not save your comms plan", e))
}

// ---------- photos ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Photo {
    pub name: String,
    pub bytes: u64,
    /// seconds since 1970, to show the newest first
    pub when: u64,
}

/// The picture type from the first bytes: "jpg", "png", "gif" or "webp".
pub fn image_kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        Some("jpg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

pub fn mime_of(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

/// A plain file name with no folder in it.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':', '\0']) && !name.starts_with('.') && name != "." && name != ".."
}

pub fn photos_list(dir: &Path) -> Vec<Photo> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Photo> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let meta = e.metadata().ok()?;
            if !meta.is_file() || !plain_name(&name) || mime_of(&name) == "application/octet-stream" {
                return None;
            }
            let when = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            Some(Photo { name, bytes: meta.len(), when })
        })
        .collect();
    out.sort_by(|a, b| b.when.cmp(&a.when).then(a.name.cmp(&b.name)));
    out
}

/// Put a picture in a folder. Only real pictures are taken (the first bytes are checked, not the name), the
/// name is cleaned, and an existing file is never replaced: a number is added to the new one instead.
pub fn photo_add(dir: &Path, original_name: &str, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_PHOTO_BYTES {
        return Err("That picture is larger than 40 MB.".into());
    }
    let kind = image_kind(bytes).ok_or("That does not look like a picture. Photos can be JPEG, PNG, GIF or WebP files.")?;
    let base = original_name.rsplit(['/', '\\']).next().unwrap_or("");
    let stem = base.rsplit_once('.').map(|x| x.0).unwrap_or(base);
    let mut stem: String = stem.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    stem = stem.trim().trim_start_matches('.').to_string();
    if stem.is_empty() {
        stem = "photo".into();
    }
    if stem.chars().count() > 60 {
        stem = stem.chars().take(60).collect();
    }
    fs::create_dir_all(dir).map_err(|e| io_err("Could not make the photo folder", e))?;
    let mut name = format!("{}.{}", stem, kind);
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("{} ({}).{}", stem, n, kind);
        n += 1;
    }
    fsio::write_atomic(&dir.join(&name), bytes).map_err(|e| io_err("Could not save the picture", e))?;
    Ok(name)
}

pub fn photo_read(dir: &Path, name: &str) -> Result<Vec<u8>, String> {
    if !plain_name(name) || mime_of(name) == "application/octet-stream" {
        return Err("That picture is not here.".into());
    }
    fs::read(dir.join(name)).map_err(|_| "That picture is not here.".to_string())
}

pub fn photo_delete(dir: &Path, name: &str) -> Result<(), String> {
    if !plain_name(name) || mime_of(name) == "application/octet-stream" {
        return Err("That picture is not here.".into());
    }
    fs::remove_file(dir.join(name)).map_err(|e| io_err("Could not remove the picture", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-people-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn png() -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(b"fake body");
        v
    }

    #[test]
    fn names_become_safe_folders_and_cannot_climb_out() {
        assert_eq!(folder_name("Anna").unwrap(), "Anna");
        assert_eq!(folder_name("  Mary   Ann ").unwrap(), "Mary Ann");
        assert_eq!(folder_name("José").unwrap(), "José");
        assert_eq!(folder_name("..\\..\\x").unwrap(), "_.._x");
        for sneaky in ["../x", "..\\x", "a/b", "C:\\Windows", "a\0b"] {
            let f = folder_name(sneaky).unwrap();
            assert!(!f.contains(['/', '\\', ':', '\0']), "{} -> {}", sneaky, f);
            assert!(!f.starts_with('.'), "{} -> {}", sneaky, f);
        }
        assert!(folder_name("").is_err());
        assert!(folder_name("   ").is_err());
        assert!(folder_name("...").is_err());
        assert!(folder_name("???").is_err());
        for r in ["CON", "nul", "Com1", "LPT9", "aux.txt"] {
            assert!(folder_name(r).is_err(), "{}", r);
        }
        assert_eq!(folder_name(&"x".repeat(80)).unwrap().len(), 40);
        let u = tmp("dirs");
        assert!(person_dir(&u, "Anna").unwrap().starts_with(u.join("people")));
    }

    #[test]
    fn dates_are_checked() {
        assert!(valid_date("2026-10-23"));
        for bad in ["2026-13-01", "2026-10-32", "2026-1-5", "20261023", "../../x.md", "2026-10-2a", "1800-01-01", ""] {
            assert!(!valid_date(bad), "{}", bad);
        }
    }

    #[test]
    fn the_journal_keeps_a_copy_before_it_changes_a_day() {
        let u = tmp("journal");
        assert!(journal_list(&u, "Anna").unwrap().is_empty());
        journal_save(&u, "Anna", "2026-10-22", "First day.").unwrap();
        journal_save(&u, "Anna", "2026-10-23", "Second day.").unwrap();
        journal_save(&u, "Anna", "2026-10-23", "Second day, longer.").unwrap();
        let l = journal_list(&u, "Anna").unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], Entry { date: "2026-10-23".into(), text: "Second day, longer.".into() });
        let backups = person_dir(&u, "Anna").unwrap().join("backups");
        assert_eq!(fs::read_dir(&backups).unwrap().count(), 1, "the earlier text was kept");
        // another person does not see it
        assert!(journal_list(&u, "Ben").unwrap().is_empty());
        // empty text removes the day but keeps a copy
        journal_save(&u, "Anna", "2026-10-22", "  ").unwrap();
        assert_eq!(journal_list(&u, "Anna").unwrap().len(), 1);
        assert_eq!(fs::read_dir(&backups).unwrap().count(), 2);
        assert!(journal_save(&u, "Anna", "../x", "no").is_err());
        assert!(journal_save(&u, "Anna", "2026-10-22", &"x".repeat(MAX_JOURNAL_BYTES + 1)).is_err());
    }

    #[test]
    fn the_comms_plan_starts_from_a_template_and_is_private_to_the_person() {
        let u = tmp("plan");
        assert!(plan_load(&u, "Anna").unwrap().contains("check in"));
        plan_save(&u, "Anna", "Channel 3, hourly.").unwrap();
        assert_eq!(plan_load(&u, "Anna").unwrap(), "Channel 3, hourly.");
        assert!(plan_load(&u, "Ben").unwrap().contains("check in"));
        plan_save(&u, "Anna", "Channel 5.").unwrap();
        assert_eq!(fs::read_dir(person_dir(&u, "Anna").unwrap().join("backups")).unwrap().count(), 1);
    }

    #[test]
    fn photos_are_checked_named_safely_and_never_overwritten() {
        let u = tmp("photos");
        let mine = photo_dir(&u, "mine", "Anna").unwrap();
        let shared = photo_dir(&u, "shared", "Anna").unwrap();
        assert_ne!(mine, shared);
        assert!(photo_dir(&u, "everyone", "Anna").is_err());
        assert!(photo_add(&mine, "notes.png", b"just some words").is_err(), "a name alone does not make a picture");
        assert_eq!(photo_add(&mine, "Family picnic.PNG", &png()).unwrap(), "Family picnic.png");
        assert_eq!(photo_add(&mine, "Family picnic.png", &png()).unwrap(), "Family picnic (2).png");
        // a picture's real type decides the extension
        assert_eq!(photo_add(&mine, "x.txt", &png()).unwrap(), "x.png");
        // a path in the name is ignored
        assert_eq!(photo_add(&mine, "..\\..\\sneaky.png", &png()).unwrap(), "sneaky.png");
        assert_eq!(photo_add(&mine, "", &png()).unwrap(), "photo.png");
        assert!(photo_add(&mine, "big.png", &vec![0u8; MAX_PHOTO_BYTES + 1]).is_err());
        let l = photos_list(&mine);
        assert_eq!(l.len(), 5);
        assert!(photos_list(&shared).is_empty());
        assert_eq!(photo_read(&mine, "sneaky.png").unwrap(), png());
        for bad in ["../x.png", "..\\x.png", "a/b.png", ".hidden.png", "readme.txt", ""] {
            assert!(photo_read(&mine, bad).is_err(), "{}", bad);
            assert!(photo_delete(&mine, bad).is_err(), "{}", bad);
        }
        photo_delete(&mine, "sneaky.png").unwrap();
        assert_eq!(photos_list(&mine).len(), 4);
    }

    #[test]
    fn picture_types_come_from_the_first_bytes() {
        assert_eq!(image_kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        assert_eq!(image_kind(b"GIF89a...."), Some("gif"));
        assert_eq!(image_kind(b"RIFF\0\0\0\0WEBPVP8 "), Some("webp"));
        assert_eq!(image_kind(b"<svg>"), None, "svg can carry scripts, so it is not taken");
        assert_eq!(image_kind(b""), None);
        assert_eq!(mime_of("a.JPEG"), "image/jpeg");
    }
}
