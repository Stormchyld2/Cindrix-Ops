//! Turning the lock on the security records on and off (docs/47). The sealing itself is supplied by the program
//! (`fsio::AtRest`); this module walks the files. Every file is sealed, then opened again and compared before the plain
//! one is replaced, so a failure can never leave a record unreadable. Stopping half way is safe: a folder with some
//! sealed and some plain files reads fine while the lock is open, and doing it again finishes the job.

use crate::fsio::{self, AtRest};
use std::fs;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        let mut v: Vec<_> = rd.flatten().collect();
        v.sort_by_key(|e| e.file_name());
        for e in v {
            let p = e.path();
            match fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => walk(&p, out),
                Ok(m) if m.is_file() => out.push(p),
                _ => {}
            }
        }
    }
}

/// Every file the lock covers, including old copies in `security/backups`.
pub fn protected_files(user_root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    walk(&user_root.join("security"), &mut v);
    for n in ["devices.txt", "devices-log.txt", "trusted-devices.txt", "share-pin.txt"] {
        let p = user_root.join(n);
        if p.is_file() { v.push(p); }
    }
    v.retain(|p| !p.extension().map(|e| e == "tmp").unwrap_or(false));
    v
}

/// Seal every plain file. Returns how many were sealed.
pub fn seal_all(user_root: &Path, key: &dyn AtRest) -> Result<usize, String> {
    let mut n = 0;
    for p in protected_files(user_root) {
        let raw = fs::read(&p).map_err(|_| "A record could not be read, so the lock was not finished. Nothing was lost.".to_string())?;
        if fsio::is_sealed(&raw) { continue; }
        let sealed = key.seal(&raw)?;
        if key.open(&sealed).map(|b| b != raw).unwrap_or(true) { return Err("Sealing did not check out, so the lock was not finished. Nothing was lost.".to_string()); }
        fsio::write_atomic_raw(&p, &sealed).map_err(|_| "A record could not be saved, so the lock was not finished. Nothing was lost.".to_string())?;
        n += 1;
    }
    Ok(n)
}

/// Open every sealed file back to plain text. Returns how many were opened.
pub fn unseal_all(user_root: &Path, key: &dyn AtRest) -> Result<usize, String> {
    let mut n = 0;
    for p in protected_files(user_root) {
        let raw = fs::read(&p).map_err(|_| "A record could not be read, so the lock was not removed. Nothing was lost.".to_string())?;
        if !fsio::is_sealed(&raw) { continue; }
        let plain = key.open(&raw)?;
        fsio::write_atomic_raw(&p, &plain).map_err(|_| "A record could not be saved, so the lock was not removed. Nothing was lost.".to_string())?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A toy lock for tests only: the magic, then every byte flipped.
    struct Toy(u8);
    impl AtRest for Toy {
        fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
            let mut v = b"CXL1".to_vec();
            v.extend(plain.iter().map(|b| b ^ self.0));
            Ok(v)
        }
        fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
            if !fsio::is_sealed(sealed) { return Err("not sealed".into()); }
            let plain: Vec<u8> = sealed[4..].iter().map(|b| b ^ self.0).collect();
            if sealed.len() > 4 && plain.iter().all(|b| *b == 0) { return Err("wrong key".into()); }
            Ok(plain)
        }
    }

    fn setup(n: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!("cindrix-lock-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&r);
        fs::create_dir_all(r.join("security/backups")).unwrap();
        fs::write(r.join("security/incident-log.txt"), "# format: incident-log 1\nsecret\n").unwrap();
        fs::write(r.join("security/backups/1-sites.txt"), "old site list\n").unwrap();
        fs::write(r.join("trusted-devices.txt"), "id\ttoken123\tSam\t1\n").unwrap();
        fs::write(r.join("tasks.txt"), "not part of the lock\n").unwrap();
        r
    }

    #[test]
    fn sealing_covers_security_old_copies_and_sign_in_files_but_nothing_else_and_opens_again() {
        let r = setup("a");
        assert_eq!(seal_all(&r, &Toy(7)).unwrap(), 3);
        for f in ["security/incident-log.txt", "security/backups/1-sites.txt", "trusted-devices.txt"] {
            let raw = fs::read(r.join(f)).unwrap();
            assert!(fsio::is_sealed(&raw) && !String::from_utf8_lossy(&raw).contains("secret") && !String::from_utf8_lossy(&raw).contains("token123"), "{}", f);
        }
        assert_eq!(fs::read_to_string(r.join("tasks.txt")).unwrap(), "not part of the lock\n");
        assert_eq!(seal_all(&r, &Toy(7)).unwrap(), 0, "doing it again changes nothing");
        assert_eq!(unseal_all(&r, &Toy(7)).unwrap(), 3);
        assert_eq!(fs::read_to_string(r.join("security/incident-log.txt")).unwrap(), "# format: incident-log 1\nsecret\n");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn while_locked_reads_and_writes_fail_and_never_fall_back_to_plain_text() {
        let r = setup("b");
        seal_all(&r, &Toy(7)).unwrap();
        fsio::set_guard(&r, true, None);
        let log = r.join("security/incident-log.txt");
        let before = fs::read(&log).unwrap();
        assert!(fsio::is_locked(&r));
        assert_eq!(fsio::read_to_string(&log).unwrap_err().to_string(), fsio::LOCKED_MSG);
        assert!(fsio::write_atomic(&log, b"plain").is_err());
        assert!(fsio::append(&log, b"more\n").is_err());
        assert!(fsio::write_atomic(&r.join("security/new.txt"), b"plain").is_err(), "even a new file is not written as plain text");
        assert!(!r.join("security/new.txt").exists());
        assert_eq!(fs::read(&log).unwrap(), before, "nothing changed");
        // a file outside the lock is untouched by it
        fsio::write_atomic(&r.join("tasks.txt"), b"fine\n").unwrap();
        fsio::set_guard(&r, false, None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn when_opened_files_read_write_and_append_sealed_and_a_wrong_key_reads_nothing() {
        let r = setup("c");
        seal_all(&r, &Toy(7)).unwrap();
        fsio::set_guard(&r, true, Some(Arc::new(Toy(7))));
        let log = r.join("security/incident-log.txt");
        assert!(!fsio::is_locked(&r));
        assert!(fsio::read_to_string(&log).unwrap().contains("secret"));
        fsio::append(&log, b"added\n").unwrap();
        assert!(fsio::is_sealed(&fs::read(&log).unwrap()), "an append stays sealed");
        assert!(fsio::read_to_string(&log).unwrap().ends_with("secret\nadded\n"));
        fsio::write_atomic(&r.join("security/new.txt"), b"hello").unwrap();
        assert!(fsio::is_sealed(&fs::read(r.join("security/new.txt")).unwrap()), "new files are sealed");
        // a plain file written before the lock still reads
        fs::write(r.join("security/old-plain.txt"), "plain\n").unwrap();
        assert_eq!(fsio::read_to_string(&r.join("security/old-plain.txt")).unwrap(), "plain\n");
        // a different key cannot open them
        fsio::set_guard(&r, true, Some(Arc::new(Toy(9))));
        let wrong = fsio::read_to_string(&log);
        assert!(wrong.is_err() || !wrong.unwrap().contains("secret"));
        fsio::set_guard(&r, false, None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn a_failed_check_leaves_every_file_as_it_was() {
        struct Bad;
        impl AtRest for Bad {
            fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> { let mut v = b"CXL1".to_vec(); v.extend_from_slice(plain); Ok(v) }
            fn open(&self, _: &[u8]) -> Result<Vec<u8>, String> { Ok(b"different".to_vec()) }
        }
        let r = setup("d");
        assert!(seal_all(&r, &Bad).is_err());
        assert_eq!(fs::read_to_string(r.join("security/incident-log.txt")).unwrap(), "# format: incident-log 1\nsecret\n");
        let _ = fs::remove_dir_all(&r);
    }
}
