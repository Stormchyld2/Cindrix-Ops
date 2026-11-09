//! Safe file writes: write to a temporary file, then rename. Keeps a dated backup first.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------- optional lock on the security records (docs/47) ----------
//
// When the person turns the lock on, every file under `user/security/` and the device sign-in files are stored sealed
// (the program supplies the sealing; `core` stays free of outside code). Reads open them, writes seal them. While the
// lock is on and not opened, a read or write of those files FAILS with `LOCKED_MSG`: it never falls back to writing
// plain text, and an unreadable file is never treated as an empty one. Files written before the lock was turned on are
// still read as they are, so turning it on is safe at any time. The guard is tied to one `user` folder, so nothing else
// (and no other test) is affected.

pub const LOCKED_MSG: &str = "The security records are locked. Unlock them with the passcode first.";
const SEAL_MAGIC: &[u8; 4] = b"CXL1";

pub trait AtRest: Send + Sync {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String>;
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String>;
}

struct Guard {
    root: PathBuf,
    /// The lock is turned on for this folder.
    on: bool,
    key: Option<Arc<dyn AtRest>>,
}
static GUARDS: Mutex<Vec<Guard>> = Mutex::new(Vec::new());

/// Say whether the lock is on for a `user` folder, and give the key while it is open (None = locked).
pub fn set_guard(user_root: &Path, on: bool, key: Option<Arc<dyn AtRest>>) {
    let mut g = GUARDS.lock().unwrap_or_else(|e| e.into_inner());
    g.retain(|x| x.root != user_root);
    if on || key.is_some() {
        g.push(Guard { root: user_root.to_path_buf(), on, key });
    }
}

/// Files the lock covers, relative to the `user` folder.
pub fn is_protected_rel(rel: &Path) -> bool {
    let mut c = rel.components();
    match c.next().map(|x| x.as_os_str().to_string_lossy().to_string()) {
        Some(first) if first == "security" => true,
        Some(first) if c.next().is_none() => matches!(first.as_str(), "devices.txt" | "devices-log.txt" | "trusted-devices.txt" | "share-pin.txt"),
        _ => false,
    }
}

fn guard_for(path: &Path) -> Option<(bool, Option<Arc<dyn AtRest>>)> {
    let g = GUARDS.lock().unwrap_or_else(|e| e.into_inner());
    g.iter().find_map(|x| path.strip_prefix(&x.root).ok().filter(|rel| is_protected_rel(rel)).map(|_| (x.on, x.key.clone())))
}

pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == SEAL_MAGIC
}

fn locked() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, LOCKED_MSG)
}

/// True if the lock is on for this folder and not opened.
pub fn is_locked(user_root: &Path) -> bool {
    let g = GUARDS.lock().unwrap_or_else(|e| e.into_inner());
    g.iter().any(|x| x.root == user_root && x.on && x.key.is_none())
}

/// Read a file, opening it if it is sealed.
pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    let raw = fs::read(path)?;
    if !is_sealed(&raw) { return Ok(raw); }
    let key = guard_for(path).and_then(|(_, k)| k).ok_or_else(locked)?;
    key.open(&raw).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
pub fn read_to_string(path: &Path) -> std::io::Result<String> {
    String::from_utf8(read(path)?).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "not text"))
}

fn encode(path: &Path, data: &[u8]) -> std::io::Result<Vec<u8>> {
    match guard_for(path) {
        Some((true, None)) => Err(locked()),
        Some((true, Some(k))) => k.seal(data).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
        _ => Ok(data.to_vec()),
    }
}

/// Add bytes to the end of a file. A locked-protected file is read, added to and sealed again (these logs are small).
pub fn append(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
    if matches!(guard_for(path), Some((true, _))) {
        let mut all = match read(path) {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        all.extend_from_slice(data);
        return write_atomic(path, &all);
    }
    let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(data)?;
    f.sync_all()
}

/// Write bytes exactly as given (no sealing). For the work of turning the lock on and off.
pub fn write_atomic_raw(path: &Path, data: &[u8]) -> std::io::Result<()> {
    write_plain(path, data)
}

/// Write `data` to `path` so a power loss never leaves a half-written file.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let data = encode(path, data)?;
    write_plain(path, &data)
}

fn write_plain(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp: PathBuf = path.to_path_buf();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    tmp.set_file_name(format!("{}.tmp", name));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Copy the current file into `backups_dir` with a timestamp before it is replaced.
pub fn backup_then_write(path: &Path, backups_dir: &Path, data: &[u8]) -> std::io::Result<()> {
    if path.exists() {
        fs::create_dir_all(backups_dir)?;
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        fs::copy(path, backups_dir.join(format!("{}-{}", secs, name)))?;
    }
    write_atomic(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-fsio-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn write_atomic_replaces_the_file_and_leaves_no_temporary_one() {
        let d = tmp("atomic");
        let f = d.join("sub").join("a.txt");
        write_atomic(&f, b"one").unwrap();
        write_atomic(&f, b"two").unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"two");
        assert!(!d.join("sub").join("a.txt.tmp").exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn backup_then_write_keeps_the_old_contents() {
        let d = tmp("backup");
        let f = d.join("a.txt");
        write_atomic(&f, b"old").unwrap();
        backup_then_write(&f, &d.join("backups"), b"new").unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"new");
        let kept: Vec<_> = fs::read_dir(d.join("backups")).unwrap().map(|e| fs::read(e.unwrap().path()).unwrap()).collect();
        assert_eq!(kept, vec![b"old".to_vec()]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn append_adds_to_the_end_and_creates_the_file() {
        let d = tmp("append");
        let f = d.join("log.txt");
        append(&f, b"a\n").unwrap();
        append(&f, b"b\n").unwrap();
        assert_eq!(read_to_string(&f).unwrap(), "a\nb\n");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn only_the_listed_files_are_covered_by_the_lock() {
        for yes in ["security/x.txt", "security/backups/y.txt", "devices.txt", "devices-log.txt", "trusted-devices.txt", "share-pin.txt"] {
            assert!(is_protected_rel(Path::new(yes)), "{} should be covered", yes);
        }
        for no in ["inventory.txt", "tasks.txt", "inbox/pocket/r.txt", "exports/security-records.txt", "people/a/lock.json", "seclock.json", "sub/devices.txt"] {
            assert!(!is_protected_rel(Path::new(no)), "{} should not be covered", no);
        }
    }

    #[test]
    fn a_sealed_marker_is_recognised_and_plain_text_is_not() {
        assert!(is_sealed(b"CXL1abc"));
        assert!(!is_sealed(b"CXL"));
        assert!(!is_sealed(b"# format: x 1"));
    }
}
