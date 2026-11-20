//! Tauri side of the Gear list (core/src/gear.rs). Host-only: none of these commands is in the share allowlist.

use cindrix_core::gear::{self, Piece};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct PieceDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub make: String,
    pub model: String,
    pub serial: String,
    pub location: String,
    pub qty: u32,
    pub condition: String,
    pub bought: String,
    pub notes: String,
}

#[derive(Serialize)]
pub struct GearListDto {
    pub items: Vec<PieceDto>,
    pub kinds: Vec<(String, String)>,
    pub conditions: Vec<(String, String)>,
    pub unreadable_lines: usize,
}

fn user_root() -> std::path::PathBuf {
    crate::root().join("user")
}

fn dto(p: &Piece) -> PieceDto {
    PieceDto { id: p.id.clone(), name: p.name.clone(), kind: p.kind.clone(), make: p.make.clone(), model: p.model.clone(), serial: p.serial.clone(), location: p.location.clone(), qty: p.qty, condition: p.condition.clone(), bought: p.bought.clone(), notes: p.notes.clone() }
}

fn list_of(items: &[Piece], unreadable: usize) -> GearListDto {
    GearListDto {
        items: items.iter().map(dto).collect(),
        kinds: gear::KINDS.iter().map(|k| (k.0.to_string(), k.1.to_string())).collect(),
        conditions: gear::CONDITIONS.iter().map(|k| (k.0.to_string(), k.1.to_string())).collect(),
        unreadable_lines: unreadable,
    }
}

#[tauri::command]
pub fn gear_list() -> Result<GearListDto, String> {
    let l = gear::load(&user_root())?;
    Ok(list_of(&l.items, l.rejected.len()))
}

#[tauri::command]
pub fn gear_save(piece: PieceDto) -> Result<GearListDto, String> {
    let p = Piece { id: piece.id, name: piece.name, kind: piece.kind, make: piece.make, model: piece.model, serial: piece.serial, location: piece.location, qty: piece.qty, condition: piece.condition, bought: piece.bought, notes: piece.notes };
    let items = gear::save(&user_root(), p)?;
    Ok(list_of(&items, 0))
}

#[tauri::command]
pub fn gear_delete(ids: Vec<String>) -> Result<GearListDto, String> {
    let items = gear::delete(&user_root(), &ids)?;
    Ok(list_of(&items, 0))
}

// ---------- two-way copy with the other device (core/src/gearsync.rs) ----------

#[derive(Serialize)]
pub struct SyncConflictDto {
    pub id: String,
    pub mine: Option<PieceDto>,
    pub theirs: Option<PieceDto>,
}

#[derive(Serialize)]
pub struct SyncPlanDto {
    pub from: String,
    pub same: usize,
    pub take: Vec<PieceDto>,
    pub remove: Vec<PieceDto>,
    pub conflicts: Vec<SyncConflictDto>,
}

#[derive(Serialize)]
pub struct SyncReportDto {
    pub taken: usize,
    pub removed: usize,
    pub kept_mine: usize,
    pub took_theirs: usize,
    pub waiting: usize,
}

fn my_code() -> String {
    cindrix_core::userdata::UserDir::new(&user_root()).device_id()
}

/// Write this device's gear list as a sheet for the other device and say where it is.
#[tauri::command]
pub fn gear_sync_export() -> Result<String, String> {
    let code = my_code();
    let text = cindrix_core::gearsync::export(&user_root(), &code)?;
    let name = format!("gear-sync-{}.txt", code);
    let dir = user_root().join("exports");
    std::fs::create_dir_all(&dir).map_err(|_| "The exports folder could not be made.".to_string())?;
    cindrix_core::fsio::write_atomic(&dir.join(&name), text.as_bytes()).map_err(|e| format!("The sheet could not be saved: {}", e))?;
    Ok(format!("exports/{}", name))
}

#[tauri::command]
pub fn gear_sync_waiting() -> Vec<String> {
    cindrix_core::gearsync::waiting(&user_root())
}

#[tauri::command]
pub fn gear_sync_preview(name: String) -> Result<SyncPlanDto, String> {
    let sheet = cindrix_core::gearsync::read_waiting(&user_root(), &name)?;
    let p = cindrix_core::gearsync::preview(&user_root(), &my_code(), &sheet)?;
    Ok(SyncPlanDto {
        from: p.from,
        same: p.same,
        take: p.take.iter().map(dto).collect(),
        remove: p.remove.iter().map(dto).collect(),
        conflicts: p.conflicts.iter().map(|c| SyncConflictDto { id: c.id.clone(), mine: c.mine.as_ref().map(dto), theirs: c.theirs.as_ref().map(dto) }).collect(),
    })
}

/// `picks`: for each conflict the person has decided, `true` keeps the other device's version.
#[tauri::command]
pub fn gear_sync_accept(name: String, picks: Vec<(String, bool)>) -> Result<SyncReportDto, String> {
    let sheet = cindrix_core::gearsync::read_waiting(&user_root(), &name)?;
    let r = cindrix_core::gearsync::accept(&user_root(), &my_code(), &sheet, &picks)?;
    if r.waiting == 0 {
        cindrix_core::gearsync::finish_waiting(&user_root(), &name)?;
    }
    Ok(SyncReportDto { taken: r.taken, removed: r.removed, kept_mine: r.kept_mine, took_theirs: r.took_theirs, waiting: r.waiting })
}
