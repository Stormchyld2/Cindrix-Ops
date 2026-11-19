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

fn read_recovery() -> Option<String> {
    let text = std::fs::read_to_string(lock_file()).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("recovery").and_then(|k| k.as_str()).map(|s| s.to_string())
}

/// Save the passcode's wrap of the key. The recovery wrap, if there is one, is kept as it is.
fn write_wrapped(wrapped: &str) -> Result<(), String> {
    write_both(wrapped, read_recovery().as_deref())
}

fn write_both(wrapped: &str, recovery: Option<&str>) -> Result<(), String> {
    let mut v = serde_json::json!({ "v": 1, "key": wrapped });
    if let Some(r) = recovery { v["recovery"] = serde_json::Value::String(r.to_string()); }
    let body = v.to_string();
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
    has_recovery: bool,
}

#[tauri::command]
pub fn seclock_status() -> Status {
    Status { on: has_lock(), unlocked: !locked(), has_recovery: read_recovery().is_some() }
}

/// A recovery code: 20 letters and digits in groups of four, without the look-alikes (0, O, 1, I). Shown once.
fn new_recovery_code() -> Result<String, String> {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let bytes = cindrix_crypto::new_key().map_err(|e| e.to_string())?;
    let chars: Vec<char> = bytes.iter().take(20).map(|b| ALPHABET[(*b & 31) as usize] as char).collect();
    Ok(chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-"))
}

/// What the person typed, as the key's passphrase: capitals only, no spaces or dashes.
fn recovery_passphrase(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

#[derive(Serialize)]
pub struct SetResult {
    sealed: usize,
    recovery: String,
}

/// Make a new recovery code (the old one stops working). Needs the current passcode.
#[tauri::command]
pub fn seclock_make_recovery(passcode: String) -> Result<String, String> {
    let key = open_key(&passcode)?;
    let code = new_recovery_code()?;
    let wrapped = cindrix_crypto::wrap_key(&key, &recovery_passphrase(&code)).map_err(|e| e.to_string())?;
    let current = read_wrapped()?;
    write_both(&current, Some(&wrapped))?;
    Ok(code)
}

/// The host's way back in after a forgotten passcode: the recovery code opens the key, and a new passcode is set.
/// Nothing in the records is touched. The recovery code stays valid.
#[tauri::command]
pub fn seclock_reset(recovery_code: String, new_passcode: String, app: tauri::AppHandle) -> Result<(), String> {
    check_new(&new_passcode)?;
    let wrapped = read_recovery().ok_or("There is no recovery code for this lock. Make one while the records are open.")?;
    let key = cindrix_crypto::unwrap_key(&wrapped, &recovery_passphrase(&recovery_code)).map_err(|_| "That recovery code is not right.".to_string())?;
    let new_wrap = cindrix_crypto::wrap_key(&key, &new_passcode).map_err(|e| e.to_string())?;
    write_both(&new_wrap, Some(&wrapped))?;
    open_now(key);
    crate::share::reload_after_lock_change(&app);
    Ok(())
}

/// Turn the passcode on. Every record is sealed, with a checked round trip first. The host stays unlocked.
#[tauri::command]
pub fn seclock_set(passcode: String, app: tauri::AppHandle) -> Result<SetResult, String> {
    check_new(&passcode)?;
    if has_lock() {
        return Err("The security records already have a passcode.".into());
    }
    let key = cindrix_crypto::new_key().map_err(|e| e.to_string())?;
    let wrapped = cindrix_crypto::wrap_key(&key, &passcode).map_err(|e| e.to_string())?;
    let code = new_recovery_code()?;
    let recovery = cindrix_crypto::wrap_key(&key, &recovery_passphrase(&code)).map_err(|e| e.to_string())?;
    write_both(&wrapped, Some(&recovery))?;
    open_now(key);
    let n = seclock::seal_all(&user_dir(), &Key(key));
    crate::share::reload_after_lock_change(&app);
    Ok(SetResult { sealed: n?, recovery: code })
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
