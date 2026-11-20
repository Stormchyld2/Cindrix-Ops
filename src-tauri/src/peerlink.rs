//! Tauri side of the link to the other device (core/src/peer.rs, peersync.rs). Host-only commands, plus the
//! background round that runs by itself while the program is open. Nothing here reaches past the local network.

use cindrix_core::{peer, peersync};
use serde::Serialize;
use std::sync::Mutex;

/// One round at a time, whether the person pressed the button or the timer fired.
static ROUND: Mutex<()> = Mutex::new(());

fn user_root() -> std::path::PathBuf {
    crate::root().join("user")
}

fn last_path() -> std::path::PathBuf {
    user_root().join("sync").join("last.txt")
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[derive(Serialize, Clone)]
pub struct HeldDto {
    pub file: String,
    pub id: String,
    pub why: String,
    pub line: String,
}

#[derive(Serialize)]
pub struct LinkStatusDto {
    pub linked: bool,
    pub address: String,
    pub name: String,
    /// When the last round finished (seconds since 1970), 0 when none has.
    pub last_at: u64,
    pub last_notes: Vec<String>,
    pub held: Vec<HeldDto>,
    pub gear_waiting: usize,
}

#[derive(Serialize)]
pub struct RoundDto {
    pub gear_taken: usize,
    pub gear_waiting: usize,
    pub inventory_changes: usize,
    pub log_lines: usize,
    pub notes: Vec<String>,
}

fn status() -> Result<LinkStatusDto, String> {
    let link = peer::load(&user_root())?;
    let last = std::fs::read_to_string(last_path()).unwrap_or_default();
    let mut it = last.lines();
    let last_at = it.next().and_then(|l| l.parse().ok()).unwrap_or(0);
    let last_notes: Vec<String> = it.map(|l| l.to_string()).collect();
    Ok(LinkStatusDto {
        linked: link.is_some(),
        address: link.as_ref().map(|l| l.base.clone()).unwrap_or_default(),
        name: link.map(|l| l.name).unwrap_or_default(),
        last_at,
        last_notes,
        held: peersync::held(&user_root()).into_iter().map(|h| HeldDto { file: h.file, id: h.id, why: h.why, line: h.line }).collect(),
        gear_waiting: cindrix_core::gearsync::waiting(&user_root()).len(),
    })
}

/// One round with the other device. Returns what happened in plain words.
pub fn round() -> Result<RoundDto, String> {
    let _one = ROUND.lock().map_err(|_| "A copy is already running.".to_string())?;
    let link = peer::load(&user_root())?.ok_or("This device is not linked to another one yet.")?;
    let s = peersync::run(&user_root(), &peer::Http(link));
    let mut text = format!("{}\n", now());
    for n in &s.notes {
        text.push_str(&format!("{}\n", n.replace('\n', " ")));
    }
    let _ = cindrix_core::fsio::write_atomic(&last_path(), text.as_bytes());
    Ok(RoundDto { gear_taken: s.gear_taken, gear_waiting: s.gear_waiting, inventory_changes: s.inventory_changes, log_lines: s.log_lines, notes: s.notes })
}

/// Starts the timer that makes a round every few minutes while the program is open and a link exists.
pub fn start_loop() {
    let _ = std::thread::Builder::new().name("peer-link".into()).spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(45));
        loop {
            if !crate::seclock::locked() && matches!(peer::load(&user_root()), Ok(Some(_))) {
                let _ = round();
            }
            std::thread::sleep(std::time::Duration::from_secs(300));
        }
    });
}

#[tauri::command]
pub fn link_join(url: String, name: String) -> Result<LinkStatusDto, String> {
    let l = peer::join(&url, &name)?;
    peer::save(&user_root(), &l)?;
    status()
}

#[tauri::command]
pub fn link_status() -> Result<LinkStatusDto, String> {
    status()
}

#[tauri::command]
pub fn link_unlink() -> Result<LinkStatusDto, String> {
    peer::unlink(&user_root())?;
    status()
}

#[tauri::command]
pub async fn link_sync_now() -> Result<RoundDto, String> {
    tauri::async_runtime::spawn_blocking(round).await.map_err(|_| "The copy could not be run. Please try again.".to_string())?
}

#[tauri::command]
pub fn link_dismiss(file: String, line: String) -> Result<LinkStatusDto, String> {
    peersync::dismiss(&user_root(), &file, &line)?;
    status()
}
