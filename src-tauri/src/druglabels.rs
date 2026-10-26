//! Tauri side of the drug labels reference (core/src/druglabels.rs). The FDA files are fetched by
//! `scripts/get-druglabels.ps1` into `<drive>/drug-labels/raw`; "Prepare" builds one compact list next to them.
//! The labels are the FDA's text, to read. These commands are host-only (not in the share allowlist).

use cindrix_core::druglabels as dl;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;

fn folder() -> PathBuf {
    super::root().join("drug-labels")
}
fn store_path() -> PathBuf {
    folder().join("drug-labels.txt")
}

#[derive(Default, Clone)]
struct Work {
    running: bool,
    message: String,
}
static WORK: Mutex<Work> = Mutex::new(Work { running: false, message: String::new() });

#[derive(Serialize)]
pub struct StatusDto {
    /// FDA files are on the drive, waiting to be prepared
    has_files: bool,
    /// the searchable list exists
    ready: bool,
    count: usize,
    preparing: bool,
    message: String,
}

#[derive(Serialize)]
pub struct SummaryDto {
    id: String,
    brand: String,
    generic: String,
    maker: String,
}

#[derive(Serialize)]
pub struct LabelDto {
    id: String,
    brand: String,
    generic: String,
    maker: String,
    parts: Vec<(String, String)>,
}

#[tauri::command]
pub fn druglabels_status() -> StatusDto {
    let raw = folder().join("raw");
    let has_files = std::fs::read_dir(&raw)
        .map(|rd| rd.flatten().any(|e| e.path().extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("json")).unwrap_or(false)))
        .unwrap_or(false);
    let store = std::fs::read_to_string(store_path()).unwrap_or_default();
    let count = dl::count(&store);
    let w = WORK.lock().unwrap().clone();
    StatusDto { has_files, ready: count > 0, count, preparing: w.running, message: w.message }
}

/// Reads the FDA files and writes the searchable list, in the background (it can take a few minutes).
#[tauri::command]
pub fn druglabels_prepare() -> Result<(), String> {
    {
        let mut w = WORK.lock().unwrap();
        if w.running {
            return Ok(());
        }
        w.running = true;
        w.message = String::new();
    }
    std::thread::spawn(|| {
        let r = dl::build(&folder().join("raw"), &store_path());
        let mut w = WORK.lock().unwrap();
        w.running = false;
        w.message = match r {
            Ok((n, 0)) => format!("Ready: {} drug labels.", n),
            Ok((n, bad)) => format!("Ready: {} drug labels. {} file(s) could not be read; run Get-DrugLabels again to fetch them.", n, bad),
            Err(e) => e,
        };
    });
    Ok(())
}

#[tauri::command]
pub fn druglabels_search(query: String) -> Result<Vec<SummaryDto>, String> {
    let store = std::fs::read_to_string(store_path()).map_err(|_| "The drug labels are not ready yet.".to_string())?;
    Ok(dl::search(&store, &query, 40).into_iter().map(|s| SummaryDto { id: s.id, brand: s.brand, generic: s.generic, maker: s.maker }).collect())
}

#[tauri::command]
pub fn druglabels_get(id: String) -> Result<LabelDto, String> {
    let store = std::fs::read_to_string(store_path()).map_err(|_| "The drug labels are not ready yet.".to_string())?;
    dl::get(&store, &id)
        .map(|l| LabelDto { id: l.id, brand: l.brand, generic: l.generic, maker: l.maker, parts: l.parts })
        .ok_or_else(|| "That label could not be found.".to_string())
}
