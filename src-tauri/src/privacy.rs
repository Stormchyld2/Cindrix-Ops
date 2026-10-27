//! The passcode for one person's journal, own pictures and comms plan. The file work and its tests are in
//! `core/src/privacy.rs`; here is the key held in memory while a person is unlocked, and the commands.
//!
//! A random key seals the files. The passcode only wraps that key (stored in `user/people/<name>/lock.json`),
//! so changing the passcode never touches the files. The key lives in memory only, and is dropped after the
//! idle time set in Settings, when the person locks it, or when the program closes.

use cindrix_core::people;
use cindrix_core::privacy::{self, Sealer, LOCKED};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MIN_PASSCODE: usize = 6;

struct Session {
    key: [u8; 32],
    last: Instant,
}

static SESSIONS: Mutex<Option<HashMap<String, Session>>> = Mutex::new(None);
static IDLE_MINUTES: Mutex<u64> = Mutex::new(15);

pub struct KeySealer([u8; 32]);

impl Sealer for KeySealer {
    fn seal(&self, plain: &[u8]) -> Result<Vec<u8>, String> {
        cindrix_crypto::seal(&self.0, plain).map_err(|e| e.to_string())
    }
    fn open(&self, sealed: &[u8]) -> Result<Vec<u8>, String> {
        cindrix_crypto::open(&self.0, sealed).map_err(|_| "That file could not be opened. It may be damaged.".to_string())
    }
}

fn with_sessions<R>(f: impl FnOnce(&mut HashMap<String, Session>) -> R) -> R {
    let mut g = SESSIONS.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

fn user_dir() -> std::path::PathBuf {
    crate::root().join("user")
}

/// The sealer for this person right now: None when they are not protected, `Err("locked")` when they are
/// protected and not unlocked (or the idle time ran out).
pub fn sealer_for(person: &str) -> Result<Option<KeySealer>, String> {
    let ud = user_dir();
    if !privacy::is_protected(&ud, person) {
        return Ok(None);
    }
    let name = people::folder_name(person)?;
    let idle = *IDLE_MINUTES.lock().unwrap_or_else(|e| e.into_inner());
    with_sessions(|m| {
        let expired = match m.get(&name) {
            Some(s) => idle > 0 && s.last.elapsed() > Duration::from_secs(idle * 60),
            None => return Err(LOCKED.to_string()),
        };
        if expired {
            m.remove(&name);
            return Err(LOCKED.to_string());
        }
        let s = m.get_mut(&name).ok_or_else(|| LOCKED.to_string())?;
        s.last = Instant::now();
        Ok(Some(KeySealer(s.key)))
    })
}

/// For an album: the shared one is open to everyone; a person's own follows their lock.
pub fn sealer_for_scope(scope: &str, person: &str) -> Result<Option<KeySealer>, String> {
    if scope == "mine" { sealer_for(person) } else { Ok(None) }
}

pub fn as_dyn(k: &Option<KeySealer>) -> Option<&dyn Sealer> {
    k.as_ref().map(|k| k as &dyn Sealer)
}

fn read_wrapped(person: &str) -> Result<String, String> {
    let f = privacy::lock_file(&user_dir(), person)?;
    let text = std::fs::read_to_string(&f).map_err(|_| "This person has no passcode.".to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| "The passcode file is damaged.".to_string())?;
    v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string()).ok_or_else(|| "The passcode file is damaged.".to_string())
}

fn write_wrapped(person: &str, wrapped: &str) -> Result<(), String> {
    let f = privacy::lock_file(&user_dir(), person)?;
    if let Some(p) = f.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("Could not make the folder: {}", e))?;
    }
    let body = serde_json::json!({ "v": 1, "key": wrapped }).to_string();
    // not backup_then_write: an old copy would still open with the old passcode
    cindrix_core::fsio::write_atomic(&f, body.as_bytes()).map_err(|e| format!("Could not save the passcode: {}", e))
}

fn check_new(passcode: &str) -> Result<(), String> {
    if passcode.chars().count() < MIN_PASSCODE {
        return Err(format!("Use at least {} characters for the passcode.", MIN_PASSCODE));
    }
    Ok(())
}

fn open_key(person: &str, passcode: &str) -> Result<[u8; 32], String> {
    let wrapped = read_wrapped(person)?;
    cindrix_crypto::unwrap_key(&wrapped, passcode).map_err(|_| "That passcode is not right.".to_string())
}

fn remember(person: &str, key: [u8; 32]) -> Result<(), String> {
    let name = people::folder_name(person)?;
    with_sessions(|m| {
        m.insert(name, Session { key, last: Instant::now() });
    });
    Ok(())
}

pub fn lock_everyone() {
    with_sessions(|m| m.clear());
}

#[derive(Serialize)]
pub struct LockStatus {
    protected: bool,
    unlocked: bool,
}

#[tauri::command]
pub fn person_lock_status(person: String) -> Result<LockStatus, String> {
    let protected = privacy::is_protected(&user_dir(), &person);
    let unlocked = !protected || sealer_for(&person).is_ok();
    Ok(LockStatus { protected, unlocked })
}

/// Turn the passcode on: everything of this person's is sealed. They stay unlocked afterwards.
#[tauri::command]
pub fn person_lock_set(person: String, passcode: String) -> Result<usize, String> {
    check_new(&passcode)?;
    let ud = user_dir();
    if privacy::is_protected(&ud, &person) {
        return Err("This person already has a passcode.".into());
    }
    let key = cindrix_crypto::new_key().map_err(|e| e.to_string())?;
    let wrapped = cindrix_crypto::wrap_key(&key, &passcode).map_err(|e| e.to_string())?;
    // the lock file first: from here on every read and write treats the person as protected
    write_wrapped(&person, &wrapped)?;
    remember(&person, key)?;
    privacy::seal_all(&ud, &person, &KeySealer(key))
}

#[tauri::command]
pub fn person_lock_unlock(person: String, passcode: String) -> Result<(), String> {
    let key = open_key(&person, &passcode)?;
    // finish any sealing an earlier try left half done
    let _ = privacy::seal_all(&user_dir(), &person, &KeySealer(key));
    remember(&person, key)
}

#[tauri::command]
pub fn person_lock_relock(person: String) -> Result<(), String> {
    let name = people::folder_name(&person)?;
    with_sessions(|m| {
        m.remove(&name);
    });
    Ok(())
}

#[tauri::command]
pub fn person_lock_change(person: String, old_passcode: String, new_passcode: String) -> Result<(), String> {
    check_new(&new_passcode)?;
    let key = open_key(&person, &old_passcode)?;
    let wrapped = cindrix_crypto::wrap_key(&key, &new_passcode).map_err(|e| e.to_string())?;
    write_wrapped(&person, &wrapped)?;
    remember(&person, key)
}

/// Take the passcode off: the person's files are written back as plain files.
#[tauri::command]
pub fn person_lock_remove(person: String, passcode: String) -> Result<usize, String> {
    let key = open_key(&person, &passcode)?;
    let ud = user_dir();
    let n = privacy::unseal_all(&ud, &person, &KeySealer(key))?;
    std::fs::remove_file(privacy::lock_file(&ud, &person)?).map_err(|e| format!("Could not remove the passcode: {}", e))?;
    let name = people::folder_name(&person)?;
    with_sessions(|m| {
        m.remove(&name);
    });
    Ok(n)
}

/// Settings > "Lock private notes after" (minutes; 0 = only when asked).
#[tauri::command]
pub fn privacy_set_idle(minutes: u64) {
    *IDLE_MINUTES.lock().unwrap_or_else(|e| e.into_inner()) = minutes.min(24 * 60);
}
