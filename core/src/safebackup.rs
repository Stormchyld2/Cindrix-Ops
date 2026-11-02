//! The mailbox for backups sent by Cindrix Safe (the phone's encrypted document vault).
//!
//! Safe leaves a `.csbackup` file in `user/inbox/safe/`. Ops cannot read it and does not try to: every
//! file inside is encrypted with a key that only the person's Safe passphrase (or recovery code) opens.
//! Ops only checks that the file really is a Safe backup, keeps it, and lists what it holds.
//! Nothing is ever opened, merged or sent anywhere. See docs/40-safe-backups.md.
//!
//! A `.csbackup` is an ordinary zip of `vault.bin` and `items/<16 hex>.item|.att`; the header starts with the
//! four bytes 0x43 0x53 0x56 0x31 ("CSV1"). The check reads the zip's file list and the start of `vault.bin`.

use crate::fsio;
use crate::inflate;
use std::fs;
use std::path::{Path, PathBuf};

const EXT: &str = ".csbackup";
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024 + 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 5000;
const MAGIC: [u8; 4] = [0x43, 0x53, 0x56, 0x31];

#[derive(Debug, Clone, PartialEq)]
pub struct BackupInfo {
    pub name: String,
    pub size: u64,
    pub modified: u64,
    /// Number of saved items (documents, cards, notes) inside.
    pub items: usize,
    /// Empty when the file is fine; otherwise a plain-words reason it cannot be used.
    pub problem: String,
}

pub fn inbox_dir(root: &Path) -> PathBuf {
    root.join("inbox").join("safe")
}
fn done_dir(root: &Path) -> PathBuf {
    inbox_dir(root).join("done")
}
pub fn store_dir(root: &Path) -> PathBuf {
    root.join("safe-backups")
}

fn safe_name(name: &str) -> bool {
    name.ends_with(EXT) && name.len() <= 150 && !name.contains(['/', '\\', ':']) && !name.starts_with('.')
}

fn le16(d: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(d.get(at..at + 2)?.try_into().ok()?) as usize)
}
fn le32(d: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?) as usize)
}

fn allowed_entry(n: &str) -> bool {
    if n == "vault.bin" {
        return true;
    }
    let Some(rest) = n.strip_prefix("items/") else { return false };
    let Some((id, ext)) = rest.split_once('.') else { return false };
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) && (ext == "item" || ext == "att")
}

/// Look inside the zip bytes. Returns the number of items, or a plain-words problem.
pub fn check(data: &[u8]) -> Result<usize, String> {
    let bad = || "This is not a Cindrix Safe backup.".to_string();
    if data.len() < 22 {
        return Err(bad());
    }
    // End-of-central-directory record: search back from the end (the comment is at most 65535 bytes).
    let lowest = data.len().saturating_sub(22 + 65535);
    let mut eocd = None;
    let mut i = data.len() - 22;
    loop {
        if data[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
            eocd = Some(i);
            break;
        }
        if i == lowest {
            break;
        }
        i -= 1;
    }
    let eocd = eocd.ok_or_else(bad)?;
    let count = le16(data, eocd + 10).ok_or_else(bad)?;
    let cd_at = le32(data, eocd + 16).ok_or_else(bad)?;
    if count == 0 || count > MAX_ENTRIES {
        return Err(bad());
    }
    let mut at = cd_at;
    let mut items = 0usize;
    let mut vault_header: Option<(usize, usize, usize)> = None; // method, compressed size, local header offset
    for _ in 0..count {
        if data.get(at..at + 4) != Some(&[0x50, 0x4b, 0x01, 0x02][..]) {
            return Err(bad());
        }
        let method = le16(data, at + 10).ok_or_else(bad)?;
        let csize = le32(data, at + 20).ok_or_else(bad)?;
        let nlen = le16(data, at + 28).ok_or_else(bad)?;
        let xlen = le16(data, at + 30).ok_or_else(bad)?;
        let clen = le16(data, at + 32).ok_or_else(bad)?;
        let local = le32(data, at + 42).ok_or_else(bad)?;
        let name = std::str::from_utf8(data.get(at + 46..at + 46 + nlen).ok_or_else(bad)?).map_err(|_| bad())?;
        if !allowed_entry(name) {
            return Err("This backup holds files that Safe would not make. It was not used.".into());
        }
        if name.ends_with(".item") {
            items += 1;
        }
        if name == "vault.bin" {
            vault_header = Some((method, csize, local));
        }
        at += 46 + nlen + xlen + clen;
    }
    let (method, csize, local) = vault_header.ok_or_else(bad)?;
    if data.get(local..local + 4) != Some(&[0x50, 0x4b, 0x03, 0x04][..]) {
        return Err(bad());
    }
    let nlen = le16(data, local + 26).ok_or_else(bad)?;
    let xlen = le16(data, local + 28).ok_or_else(bad)?;
    let start = local + 30 + nlen + xlen;
    let body = data.get(start..start.saturating_add(csize).min(data.len())).ok_or_else(bad)?;
    let head = match method {
        0 => body.to_vec(),
        8 => inflate::inflate(body, 1 << 20).map_err(|_| bad())?,
        _ => return Err(bad()),
    };
    if head.len() < 4 || head[..4] != MAGIC {
        return Err("This is not a Cindrix Safe backup.".into());
    }
    Ok(items)
}

fn mtime(p: &Path) -> u64 {
    fs::metadata(p)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn describe(dir: &Path, name: &str) -> BackupInfo {
    let path = dir.join(name);
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let (items, problem) = if size > MAX_FILE {
        (0, "This file is too big to be a Safe backup.".to_string())
    } else {
        match fs::read(&path).map_err(|_| "The file could not be read.".to_string()).and_then(|d| check(&d)) {
            Ok(n) => (n, String::new()),
            Err(e) => (0, e),
        }
    };
    BackupInfo { name: name.to_string(), size, modified: mtime(&path), items, problem }
}

fn list_dir(dir: &Path) -> Vec<BackupInfo> {
    let mut v = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if safe_name(&name) && e.path().is_file() {
                v.push(describe(dir, &name));
            }
        }
    }
    v.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    v
}

/// Backups that arrived and have not been filed yet, newest first.
pub fn waiting(root: &Path) -> Vec<BackupInfo> {
    list_dir(&inbox_dir(root))
}

/// Backups Ops is keeping, newest first.
pub fn stored(root: &Path) -> Vec<BackupInfo> {
    list_dir(&store_dir(root))
}

fn unique(dir: &Path, name: &str) -> PathBuf {
    let mut to = dir.join(name);
    let mut n = 1;
    while to.exists() {
        to = dir.join(format!("{}-{}{}", name.trim_end_matches(EXT), n, EXT));
        n += 1;
    }
    to
}

fn move_file(from: &Path, to_dir: &Path, name: &str) -> Result<(), String> {
    fs::create_dir_all(to_dir).map_err(|e| e.to_string())?;
    fs::rename(from, unique(to_dir, name)).map_err(|e| e.to_string())
}

/// Keep a waiting backup: check it, then move it into `user/safe-backups/`. A damaged file stays where it is.
pub fn keep(root: &Path, name: &str) -> Result<BackupInfo, String> {
    if !safe_name(name) {
        return Err("That file could not be found.".into());
    }
    let info = describe(&inbox_dir(root), name);
    if !info.problem.is_empty() {
        return Err(info.problem);
    }
    move_file(&inbox_dir(root).join(name), &store_dir(root), name)?;
    Ok(info)
}

/// Set a waiting file aside without keeping it (it moves to `done/`; nothing is deleted).
pub fn set_aside(root: &Path, name: &str) -> Result<(), String> {
    if !safe_name(name) {
        return Err("That file could not be found.".into());
    }
    move_file(&inbox_dir(root).join(name), &done_dir(root), name)
}

/// Delete a kept backup. Only the person asking for it should reach this.
pub fn remove_kept(root: &Path, name: &str) -> Result<(), String> {
    if !safe_name(name) {
        return Err("That file could not be found.".into());
    }
    fs::remove_file(store_dir(root).join(name)).map_err(|_| "That file could not be found.".to_string())
}

/// A copy of a kept backup in the outbox, so it can be sent back to the phone (AirLink, or by hand).
pub fn copy_to_outbox(root: &Path, name: &str) -> Result<PathBuf, String> {
    if !safe_name(name) {
        return Err("That file could not be found.".into());
    }
    let data = fs::read(store_dir(root).join(name)).map_err(|_| "That file could not be found.".to_string())?;
    let dir = root.join("outbox").join("safe");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let to = dir.join(name);
    fsio::write_atomic(&to, &data).map_err(|e| e.to_string())?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(tag: &str) -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-safebackup-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
            }
        }
        !c
    }

    /// A stored (uncompressed) zip, built by hand so the test needs no outside code.
    fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, body) in entries {
            let off = out.len() as u32;
            let crc = crc32(body);
            out.extend([0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend(crc.to_le_bytes());
            out.extend((body.len() as u32).to_le_bytes());
            out.extend((body.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend([0, 0]);
            out.extend(name.as_bytes());
            out.extend(*body);
            central.extend([0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend(crc.to_le_bytes());
            central.extend((body.len() as u32).to_le_bytes());
            central.extend((body.len() as u32).to_le_bytes());
            central.extend((name.len() as u16).to_le_bytes());
            central.extend([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend(off.to_le_bytes());
            central.extend(name.as_bytes());
        }
        let cd_at = out.len() as u32;
        out.extend(&central);
        out.extend([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((central.len() as u32).to_le_bytes());
        out.extend(cd_at.to_le_bytes());
        out.extend([0, 0]);
        out
    }

    fn header() -> Vec<u8> {
        let mut h = MAGIC.to_vec();
        h.extend([0u8; 40]);
        h
    }

    fn good() -> Vec<u8> {
        zip(&[("vault.bin", &header()), ("items/0123456789abcdef.item", b"x"), ("items/0123456789abcdef.att", b"y"), ("items/fedcba9876543210.item", b"z")])
    }

    #[test]
    fn a_real_looking_backup_is_counted() {
        assert_eq!(check(&good()), Ok(2));
    }

    #[test]
    fn other_zips_and_odd_files_are_refused() {
        assert!(check(b"hello").is_err());
        assert!(check(&zip(&[("vault.bin", b"NOPE and more bytes")])).is_err()); // wrong header
        assert!(check(&zip(&[("items/0123456789abcdef.item", b"x")])).is_err()); // no vault.bin
        assert!(check(&zip(&[("vault.bin", &header()), ("../evil.txt", b"x")])).is_err()); // name Safe never makes
        assert!(check(&zip(&[("vault.bin", &header()), ("items/ZZ.item", b"x")])).is_err());
    }

    #[test]
    fn a_waiting_backup_is_kept_and_a_bad_one_stays() {
        let root = tmp("flow");
        let inbox = inbox_dir(&root);
        fs::create_dir_all(&inbox).unwrap();
        fs::write(inbox.join("CindrixSafe-20261101-120000.csbackup"), good()).unwrap();
        fs::write(inbox.join("CindrixSafe-bad.csbackup"), b"junk").unwrap();
        fs::write(inbox.join("notes.txt"), b"ignored").unwrap();
        let w = waiting(&root);
        assert_eq!(w.len(), 2);
        assert!(w.iter().any(|b| b.name.contains("bad") && !b.problem.is_empty()));
        assert!(keep(&root, "CindrixSafe-bad.csbackup").is_err());
        assert!(inbox.join("CindrixSafe-bad.csbackup").exists());
        let k = keep(&root, "CindrixSafe-20261101-120000.csbackup").unwrap();
        assert_eq!(k.items, 2);
        assert_eq!(stored(&root).len(), 1);
        assert_eq!(waiting(&root).len(), 1);
    }

    #[test]
    fn same_name_twice_keeps_both() {
        let root = tmp("dupe");
        let inbox = inbox_dir(&root);
        fs::create_dir_all(&inbox).unwrap();
        let n = "CindrixSafe-1.csbackup";
        fs::write(inbox.join(n), good()).unwrap();
        keep(&root, n).unwrap();
        fs::write(inbox.join(n), good()).unwrap();
        keep(&root, n).unwrap();
        assert_eq!(stored(&root).len(), 2);
    }

    #[test]
    fn names_that_climb_out_are_refused() {
        let root = tmp("names");
        assert!(keep(&root, "../x.csbackup").is_err());
        assert!(set_aside(&root, "a/b.csbackup").is_err());
        assert!(remove_kept(&root, "..\\x.csbackup").is_err());
        assert!(copy_to_outbox(&root, "x.txt").is_err());
    }

    #[test]
    fn set_aside_and_remove_and_outbox_copy() {
        let root = tmp("misc");
        let inbox = inbox_dir(&root);
        fs::create_dir_all(&inbox).unwrap();
        fs::write(inbox.join("a.csbackup"), good()).unwrap();
        set_aside(&root, "a.csbackup").unwrap();
        assert!(done_dir(&root).join("a.csbackup").exists());
        fs::write(inbox.join("b.csbackup"), good()).unwrap();
        keep(&root, "b.csbackup").unwrap();
        let out = copy_to_outbox(&root, "b.csbackup").unwrap();
        assert!(out.exists());
        remove_kept(&root, "b.csbackup").unwrap();
        assert!(stored(&root).is_empty());
    }
}
