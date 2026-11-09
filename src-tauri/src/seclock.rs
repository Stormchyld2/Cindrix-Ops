//! The passcode for the security records and the device sign-in files (docs/47). The sealing and its tests are in
//! `core/src/fsio.rs` and `core/src/seclock.rs`; here is the key, held in memory only while the records are open.
//!
//! A random key seals the files. The passcode only wraps that key (stored in `user/seclock.json`), so changing the
//! passcode never touches the records. The key is dropped when the host locks it, or when the program closes.
//! While locked, every security command fails with a plain message and the share server answers nobody.

use cindrix_core::fsio::{self, AtRest};
use cindrix_core::seclock;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

const MIN_PASSCODE: usize = 6;

struct Key([u8; 32]);

impl AtRest for Key {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        cindrix_crypto::seal(&self.0, plain).map_err(|e| e.to_string())
    }
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        cindrix_crypto::open(&self.0, sealed).map_err(|_| "That file could not be opened. It may be damaged.".to_string())
    }
}

fn user_dir() -> PathBuf {
    crate::root().join("user")
}

fn lock_file() -> PathBuf {
    user_dir().join("seclock.json")
}

fn has_lock() -> bool {
    lock_file().is_file()
}

/// Call once at start (and again after the drive changes): a lock that is on starts out closed.
pub fn install() {
    fsio::set_guard(&user_dir(), has_lock(), None);
}

/// True when the lock is on and not opened. Every security command and the share server ask this first.
pub fn locked() -> bool {
    fsio::is_locked(&user_dir())
}

/// For the command wrappers: Ok when the records may be used.
pub fn require_open() -> Result<(), String> {
    if locked() { Err(fsio::LOCKED_MSG.to_string()) } else { Ok(()) }
}

fn read_wrapped() -> Result<String, String> {
    let text = std::fs::read_to_string(lock_file()).map_err(|_| "There is no passcode on the security records.".to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| "The passcode file is damaged.".to_string())?;
    v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string()).ok_or_else(|| "The passcode file is damaged.".to_string())
}

fn write_wrapped(wrapped: &str) -> Result<(), String> {
    let body = serde_json::json!({ "v": 1, "key": wrapped }).to_string();
    // not backup_then_write: an old copy would still open with the old passcode
    fsio::write_atomic_raw(&lock_file(), body.as_bytes()).map_err(|e| format!("Could not save the passcode: {}", e))
}

fn check_new(passcode: &str) -> Result<(), String> {
    if passcode.chars().count() < MIN_PASSCODE {
        return Err(format!("Use at least {} characters for the passcode.", MIN_PASSCODE));
    }
    Ok(())
}

fn open_key(passcode: &str) -> Result<[u8; 32], String> {
    cindrix_crypto::unwrap_key(&read_wrapped()?, passcode).map_err(|_| "That passcode is not right.".to_string())
}

fn open_now(key: [u8; 32]) {
    fsio::set_guard(&user_dir(), true, Some(Arc::new(Key(key))));
}

/// Forget the key (program closing, or the host pressing Lock).
pub fn lock_now() {
    if has_lock() {
        fsio::set_guard(&user_dir(), true, None);
    }
}

#[derive(Serialize)]
pub struct Status {
    on: bool,
    unlocked: bool,
}

#[tauri::command]
pub fn seclock_status() -> Status {
    Status { on: has_lock(), unlocked: !locked() }
}

/// Turn the passcode on. Every record is sealed, with a checked round trip first. The host stays unlocked.
#[tauri::command]
pub fn seclock_set(passcode: String, app: tauri::AppHandle) -> Result<usize, String> {
    check_new(&passcode)?;
    if has_lock() {
        return Err("The security records already have a passcode.".into());
    }
    let key = cindrix_crypto::new_key().map_err(|e| e.to_string())?;
    let wrapped = cindrix_crypto::wrap_key(&key, &passcode).map_err(|e| e.to_string())?;
    write_wrapped(&wrapped)?;
    open_now(key);
    let n = seclock::seal_all(&user_dir(), &Key(key));
    crate::share::reload_after_lock_change(&app);
    n
}

#[tauri::command]
pub fn seclock_unlock(passcode: String, app: tauri::AppHandle) -> Result<(), String> {
    let key = open_key(&passcode)?;
    open_now(key);
    // finish any sealing an earlier try left half done
    let _ = seclock::seal_all(&user_dir(), &Key(key));
    crate::share::reload_after_lock_change(&app);
    Ok(())
}

#[tauri::command]
pub fn seclock_relock(app: tauri::AppHandle) -> Result<(), String> {
    lock_now();
    crate::share::reload_after_lock_change(&app);
    Ok(())
}

#[tauri::command]
pub fn seclock_change(old_passcode: String, new_passcode: String) -> Result<(), String> {
    check_new(&new_passcode)?;
    let key = open_key(&old_passcode)?;
    let wrapped = cindrix_crypto::wrap_key(&key, &new_passcode).map_err(|e| e.to_string())?;
    write_wrapped(&wrapped)?;
    open_now(key);
    Ok(())
}

/// Take the passcode off: the records are written back as plain files.
#[tauri::command]
pub fn seclock_remove(passcode: String, app: tauri::AppHandle) -> Result<usize, String> {
    let key = open_key(&passcode)?;
    let n = seclock::unseal_all(&user_dir(), &Key(key))?;
    std::fs::remove_file(lock_file()).map_err(|e| format!("Could not remove the passcode: {}", e))?;
    fsio::set_guard(&user_dir(), false, None);
    crate::share::reload_after_lock_change(&app);
    Ok(n)
}
