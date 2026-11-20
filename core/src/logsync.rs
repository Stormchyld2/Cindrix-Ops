//! Copying the Security logs between two devices without losing or mixing up a record (docs/61 phase 6).
//!
//! These logs only ever grow, one tab-separated line at a time. Two devices that each add lines while apart can
//! number them the same (`c5` on both, for different events). So the copy never renumbers and never overwrites:
//!
//!  * a line the other side has and this side lacks is added at the end (earlier lines are never touched);
//!  * a line both sides have, exactly the same, is left alone;
//!  * a line whose number is already used here for something different is **held**, with the reason, and shown to
//!    a person. So is any later line from the other side that points at a held number (a "closes" or "supersedes"
//!    would otherwise land on the wrong record);
//!  * lines with no number of their own (the posture log) are compared as whole lines.
//!
//! The incident log is not copied this way: an incident and its pinned plan text belong to one device, and a
//! phone's notes for it travel as reports (`security_pocket.rs`). Snapshot files (sites, watchbill, channels,
//! plans) are not copied either; they are edited in place, not grown.

use crate::fsio;
use std::path::{Path, PathBuf};

/// The logs that are copied, as file names under `user/security/`.
pub const LOGS: &[&str] = &[
    "posture-log.txt",
    "comms-log.txt",
    "coverage-log.txt",
    "radio-training.txt",
    "review-log.txt",
    "outing-log.txt",
    "equipment-log.txt",
    "roles-log.txt",
    "inspection-log.txt",
    "availability-log.txt",
];

pub fn is_log(name: &str) -> bool {
    LOGS.contains(&name)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Held {
    pub file: String,
    pub id: String,
    pub line: String,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Merged {
    pub text: String,
    pub added: usize,
    pub held: Vec<Held>,
}

fn is_number(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(l) if l.is_ascii_lowercase()) && s.len() > 1 && c.all(|d| d.is_ascii_digit())
}

fn format_line(text: &str) -> Option<&str> {
    text.lines().find(|l| l.starts_with("# format:")).map(|l| l.trim())
}

fn data_lines(text: &str) -> Vec<&str> {
    text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).collect()
}

/// Merge the other side's log into this side's. Nothing is written here; the caller saves `text` when `added > 0`.
pub fn merge(file: &str, mine: &str, theirs: &str) -> Result<Merged, String> {
    if !is_log(file) {
        return Err("That file is not one of the logs that are copied.".to_string());
    }
    if theirs.trim().is_empty() {
        return Ok(Merged { text: mine.to_string(), ..Default::default() });
    }
    if let (Some(a), Some(b)) = (format_line(mine), format_line(theirs)) {
        if a != b {
            return Err("The two devices keep this log in different versions, so it was left alone.".to_string());
        }
    }
    if format_line(theirs).is_none() {
        return Err("The other device's copy of this log does not say what it is, so it was left alone.".to_string());
    }
    let mine_lines = data_lines(mine);
    let mut out = Merged::default();
    let mut candidates: Vec<&str> = Vec::new();
    let mut held_ids: Vec<String> = Vec::new();
    for l in data_lines(theirs) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 2 {
            out.held.push(Held { file: file.into(), id: String::new(), line: l.to_string(), why: "A line in the other copy could not be read.".to_string() });
            continue;
        }
        if is_number(f[0]) {
            match mine_lines.iter().find(|m| m.split('\t').next() == Some(f[0])) {
                Some(m) if *m == l => {}
                Some(_) => {
                    held_ids.push(f[0].to_string());
                    out.held.push(Held { file: file.into(), id: f[0].into(), line: l.to_string(), why: format!("Number {} is already used here for something different.", f[0]) });
                }
                None => candidates.push(l),
            }
        } else if !mine_lines.contains(&l) {
            candidates.push(l);
        }
    }
    // a line that points at a held number would land on the wrong record here: hold it too, until nothing more moves
    loop {
        let mut moved = false;
        let mut keep = Vec::new();
        for l in candidates {
            let f: Vec<&str> = l.split('\t').collect();
            if f.iter().skip(1).any(|x| held_ids.iter().any(|h| h == x)) {
                if is_number(f[0]) {
                    held_ids.push(f[0].to_string());
                }
                out.held.push(Held { file: file.into(), id: f[0].into(), line: l.to_string(), why: "It points at an entry that is being held for you to look at.".to_string() });
                moved = true;
            } else {
                keep.push(l);
            }
        }
        candidates = keep;
        if !moved {
            break;
        }
    }
    let mut text = mine.to_string();
    if candidates.is_empty() {
        out.text = text;
        return Ok(out);
    }
    if text.trim().is_empty() {
        // an empty log starts with the other side's heading
        text = theirs.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{}\n", l)).collect();
    } else if !text.ends_with('\n') {
        text.push('\n');
    }
    for l in &candidates {
        text.push_str(l);
        text.push('\n');
    }
    out.added = candidates.len();
    out.text = text;
    Ok(out)
}

fn path(user_root: &Path, file: &str) -> PathBuf {
    crate::security_store::dir(user_root).join(file)
}

fn read_log(user_root: &Path, file: &str) -> Result<String, String> {
    match fsio::read(&path(user_root, file)) {
        Ok(b) => String::from_utf8(b).map_err(|_| "A security file is damaged (it is not plain text). It was left alone.".to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(_) => Err("A security file could not be opened. It was left alone.".to_string()),
    }
}

/// This device's copy of a log, for sending.
pub fn export(user_root: &Path, file: &str) -> Result<String, String> {
    if !is_log(file) {
        return Err("That file is not one of the logs that are copied.".to_string());
    }
    read_log(user_root, file)
}

/// Add what is new from the other side to this device's log. Returns what was added and what is held.
pub fn accept(user_root: &Path, file: &str, theirs: &str) -> Result<Merged, String> {
    let mine = read_log(user_root, file)?;
    let m = merge(file, &mine, theirs)?;
    if m.added > 0 {
        let p = path(user_root, file);
        fsio::backup_then_write(&p, &crate::security_store::dir(user_root).join("backups"), m.text.as_bytes()).map_err(|_| "The log could not be saved. Nothing was changed.".to_string())?;
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "# format: comms-log 1\n# id, site\n";
    fn log(lines: &[&str]) -> String {
        format!("{}{}", H, lines.iter().map(|l| format!("{}\n", l)).collect::<String>())
    }

    #[test]
    fn new_lines_are_added_at_the_end_and_nothing_earlier_changes() {
        let mine = log(&["c1\thome\tA", "c2\thome\tB"]);
        let theirs = log(&["c1\thome\tA", "c2\thome\tB", "c3\thome\tC"]);
        let m = merge("comms-log.txt", &mine, &theirs).unwrap();
        assert_eq!((m.added, m.held.len()), (1, 0));
        assert!(m.text.starts_with(&mine) && m.text.ends_with("c3\thome\tC\n"));
        // the same again changes nothing
        let again = merge("comms-log.txt", &m.text, &theirs).unwrap();
        assert_eq!(again.added, 0);
        assert_eq!(again.text, m.text);
    }

    #[test]
    fn the_same_number_for_a_different_line_is_held_not_overwritten_and_so_is_anything_pointing_at_it() {
        let mine = log(&["c1\thome\tmine"]);
        let theirs = log(&["c1\thome\ttheirs", "c2\thome\tclose c1\tc1", "c3\thome\tunrelated"]);
        let m = merge("comms-log.txt", &mine, &theirs).unwrap();
        assert_eq!(m.added, 1, "only the unrelated line");
        assert!(m.text.contains("c1\thome\tmine") && !m.text.contains("theirs") && m.text.contains("c3\thome\tunrelated"));
        assert_eq!(m.held.len(), 2);
        assert!(m.held[0].why.contains("already used") && m.held[1].why.contains("points at"));
        assert!(!m.text.contains("close c1"), "the closing line must not land on my c1");
    }

    #[test]
    fn lines_without_their_own_number_are_compared_whole() {
        let h = "# format: security-posture 1\n";
        let mine = format!("{}home\televated\tDad\tr\tmanual\t2026-11-20T10:00-05:00\t\t\n", h);
        let theirs = format!("{}home\televated\tDad\tr\tmanual\t2026-11-20T10:00-05:00\t\t\nhome\tnormal\tMom\t\tmanual\t2026-11-20T12:00-05:00\t\t\n", h);
        let m = merge("posture-log.txt", &mine, &theirs).unwrap();
        assert_eq!((m.added, m.held.len()), (1, 0));
        assert_eq!(m.text.lines().filter(|l| l.starts_with("home")).count(), 2);
    }

    #[test]
    fn an_empty_side_takes_the_other_whole_and_odd_files_are_refused() {
        let theirs = log(&["c1\thome\tA"]);
        let m = merge("comms-log.txt", "", &theirs).unwrap();
        assert_eq!(m.text, theirs);
        assert_eq!(merge("comms-log.txt", &theirs, "").unwrap().added, 0);
        assert!(merge("comms-log.txt", &log(&["c1\thome\tA"]), "# format: comms-log 2\nc9\tx\ty\n").is_err(), "different version");
        assert!(merge("comms-log.txt", &theirs, "c1\thome\tA\n").is_err(), "no heading");
        assert!(merge("incident-log.txt", "", &theirs).is_err(), "the incident log is not copied this way");
        assert!(merge("../x.txt", "", &theirs).is_err());
        let bad = merge("comms-log.txt", &theirs, &format!("{}one field only\n", H)).unwrap();
        assert_eq!((bad.added, bad.held.len()), (0, 1));
    }

    #[test]
    fn accepting_writes_the_log_with_a_backup_and_leaves_a_damaged_file_alone() {
        let r = std::env::temp_dir().join(format!("cindrix-logsync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        let dir = crate::security_store::dir(&r);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("comms-log.txt"), log(&["c1\thome\tA"])).unwrap();
        let m = accept(&r, "comms-log.txt", &log(&["c1\thome\tA", "c2\thome\tB"])).unwrap();
        assert_eq!(m.added, 1);
        assert!(std::fs::read_to_string(dir.join("comms-log.txt")).unwrap().ends_with("c2\thome\tB\n"));
        assert!(dir.join("backups").exists());
        std::fs::write(dir.join("coverage-log.txt"), [0xff, 0xfe]).unwrap();
        assert!(accept(&r, "coverage-log.txt", "# format: coverage-log 1\nv1\ta\n").is_err());
        assert_eq!(export(&r, "comms-log.txt").unwrap(), std::fs::read_to_string(dir.join("comms-log.txt")).unwrap());
        assert_eq!(export(&r, "radio-training.txt").unwrap(), "");
    }
}
