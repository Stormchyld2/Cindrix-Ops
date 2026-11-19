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
