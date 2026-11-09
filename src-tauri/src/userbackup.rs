//! Tauri side of the backup of the person's own files (core/src/userbackup.rs). Host-only: none of these commands
//! is in the share allowlist. The place the copies go is remembered in `user/backup-destination.txt`.

use cindrix_core::userbackup as ub;
use serde::Serialize;
use std::path::PathBuf;

fn user_root() -> PathBuf {
    crate::root().join("user")
}
fn dest_file() -> PathBuf {
    user_root().join("backup-destination.txt")
}
fn dest() -> Option<PathBuf> {
    std::fs::read_to_string(dest_file()).ok().map(|t| PathBuf::from(t.trim())).filter(|p| !p.as_os_str().is_empty())
}

#[derive(Serialize)]
pub struct BackupFound {
    name: String,
    stamp: String,
    files: usize,
}
#[derive(Serialize)]
pub struct BackupStatus {
    destination: String,
    reachable: bool,
    backups: Vec<BackupFound>,
    /// Whole days since the newest copy, or null when there is none.
    days_since: Option<i64>,
}

pub fn days_since_last(today: &str) -> Option<Option<i64>> {
    let d = dest()?;
    Some(ub::list(&d).first().and_then(|f| ub::days_between(&f.stamp, today)))
}

#[tauri::command]
pub fn backup_status(today: String) -> BackupStatus {
    match dest() {
        None => BackupStatus { destination: String::new(), reachable: false, backups: vec![], days_since: None },
        Some(d) => {
            let backups: Vec<BackupFound> = ub::list(&d).into_iter().map(|f| BackupFound { name: f.name, stamp: f.stamp, files: f.files }).collect();
            let days_since = backups.first().and_then(|f| ub::days_between(&f.stamp, &today));
            BackupStatus { destination: d.to_string_lossy().to_string(), reachable: d.is_dir(), backups, days_since }
        }
    }
}

#[tauri::command]
pub fn backup_set_destination(path: String) -> Result<(), String> {
    let p = PathBuf::from(path.trim());
    if !p.is_dir() { return Err("That folder could not be found.".to_string()); }
    let (a, b) = (p.canonicalize().map_err(|_| "That folder could not be opened.".to_string())?, user_root().canonicalize().unwrap_or(user_root()));
    if a.starts_with(&b) { return Err("Pick a place outside your files folder, ideally on another drive.".to_string()); }
    std::fs::create_dir_all(user_root()).map_err(|_| "Your files folder could not be made.".to_string())?;
    cindrix_core::fsio::write_atomic(&dest_file(), p.to_string_lossy().as_bytes()).map_err(|_| "The choice could not be saved.".to_string())
}

#[tauri::command]
pub fn backup_now(stamp: String) -> Result<String, String> {
    let d = dest().ok_or("Pick where to keep the copy first.")?;
    let r = ub::backup(&user_root(), &d, &stamp)?;
    let mb = (r.bytes as f64 / 1_048_576.0 * 10.0).round() / 10.0;
    Ok(format!("Copied {} files ({} MB) and checked every one. {}", r.files, mb, if r.skipped.is_empty() { String::new() } else { format!("{} shortcut{} could not be copied.", r.skipped.len(), if r.skipped.len() == 1 { "" } else { "s" }) }).trim().to_string())
}

#[tauri::command]
pub fn backup_check(name: String) -> Result<String, String> {
    let d = dest().ok_or("Pick where the copies are kept first.")?;
    if name.contains('/') || name.contains('\\') || name.contains("..") { return Err("That is not a backup name.".to_string()); }
    let bad = ub::verify(&d.join(&name))?;
    if bad.is_empty() { Ok("Every file in this copy reads back correctly.".to_string()) } else { Err(format!("{} file{} in this copy are missing or damaged: {}.", bad.len(), if bad.len() == 1 { "" } else { "s" }, bad.join(", "))) }
}

#[tauri::command]
pub fn backup_bring_back(name: String) -> Result<String, String> {
    let d = dest().ok_or("Pick where the copies are kept first.")?;
    if name.contains('/') || name.contains('\\') || name.contains("..") { return Err("That is not a backup name.".to_string()); }
    let (added, differ) = ub::bring_back_missing(&d.join(&name), &user_root())?;
    Ok(format!("{} missing file{} put back. {} file{} here differ from the copy and were left as they are.", added, if added == 1 { "" } else { "s" }, differ, if differ == 1 { "" } else { "s" }))
}
