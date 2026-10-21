//! Libraries in the app: which Cindrix drives are plugged in, which of them the person has said
//! may be read, and the answers the screens give (the question when a drive appears, and the
//! list in Settings). The rules and the remembered answers are in `core/src/libraries.rs`.
//!
//! The primary library is still the one `root()` points at (your own files, settings, models and
//! engines live there). Other libraries that may be read add their archives to it, and the
//! folders Ask searches for loose files. Shelf books, music and maps still come from the primary
//! library only.

use super::{library_dir, rescan_now, AppState};
use cindrix_core::drive::DriveInfo;
use cindrix_core::kiwix;
use cindrix_core::libraries::{self, Answer, Library, Registry, State};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

#[derive(Default)]
pub struct LibState {
    reg: Registry,
    loaded: bool,
    /// Folders answered "just this time" in this run.
    once: HashSet<PathBuf>,
    list: Vec<Library>,
}

impl LibState {
    fn ensure(&mut self) {
        if !self.loaded {
            self.reg = Registry::load(&libraries::registry_path());
            self.loaded = true;
        }
    }

    /// Look at the drives plugged in now. Returns the ones that may be read.
    pub fn refresh(&mut self, prog: &Path) -> Vec<DriveInfo> {
        self.ensure();
        self.list = libraries::scan(prog, &self.reg, &self.once);
        let enabled = libraries::enabled(&self.list);
        *ENABLED.lock().unwrap_or_else(|e| e.into_inner()) = enabled.iter().map(|d| (d.root.clone(), d.name.clone())).collect();
        enabled
    }
}

/// The folder and name of every library that may be read, set by each scan.
static ENABLED: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

fn enabled_roots() -> Vec<(PathBuf, String)> {
    ENABLED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Every archive that may be read: the primary library's first, then those of the other
/// libraries. A file name never appears twice (the first library's copy wins).
pub fn all_zim_files() -> Vec<PathBuf> {
    let others: Vec<PathBuf> = enabled_roots().into_iter().map(|(r, _)| r.join("library")).collect();
    libraries::archive_files(&library_dir(), &others, kiwix::zim_files)
}

/// The roots of the other libraries that may be read (not the primary one).
pub fn other_roots(primary: &Path) -> Vec<PathBuf> {
    enabled_roots().into_iter().map(|(r, _)| r).filter(|r| r != primary).collect()
}

/// Which library an archive file is in ("" when it is not in a named library).
pub fn library_name_of(file: &Path) -> String {
    enabled_roots().into_iter().find(|(r, _)| file.starts_with(r)).map(|(_, n)| n).unwrap_or_default()
}

#[derive(Serialize)]
pub struct LibraryDto {
    id: String,
    name: String,
    edition: String,
    path: String,
    /// "own", "always", "once", "never" or "ask"
    state: &'static str,
    enabled: bool,
    /// False for a remembered answer about a drive that is not plugged in now.
    present: bool,
    /// The question should suggest "Always" (the main drives).
    suggest_always: bool,
}

/// Every drive plugged in, and every answer remembered about drives that are not.
#[tauri::command]
pub fn libraries_list(state: tauri::State<'_, AppState>) -> Vec<LibraryDto> {
    let mut g = state.libs.lock().unwrap();
    g.ensure();
    let mut out: Vec<LibraryDto> = g
        .list
        .iter()
        .map(|l| LibraryDto {
            id: l.info.id.clone(),
            name: l.info.name.clone(),
            edition: l.info.edition.clone(),
            path: l.info.root.to_string_lossy().to_string(),
            state: l.state.word(),
            enabled: l.state.enabled(),
            present: true,
            suggest_always: l.suggest_always,
        })
        .collect();
    for (id, r) in g.reg.items() {
        if !g.list.iter().any(|l| &l.info.id == id) {
            out.push(LibraryDto {
                id: id.clone(),
                name: r.name.clone(),
                edition: String::new(),
                path: String::new(),
                state: if r.answer == Answer::Always { State::Always.word() } else { State::Never.word() },
                enabled: false,
                present: false,
                suggest_always: false,
            });
        }
    }
    out
}

/// The person's answer for the drive at `path`: "always", "once" or "never".
#[tauri::command]
pub fn library_answer(path: String, answer: String, app: tauri::AppHandle) -> Result<(), String> {
    let st = app.state::<AppState>();
    let saved = {
        let mut g = st.libs.lock().unwrap();
        g.ensure();
        let root = PathBuf::from(&path);
        let lib = g.list.iter().find(|l| l.info.root == root).cloned().ok_or("That drive is not plugged in any more.")?;
        if lib.state == State::Own {
            return Err("This is the drive the program is running from, so it is always used.".into());
        }
        let (id, name) = (lib.info.id.clone(), lib.info.name.clone());
        match answer.as_str() {
            "always" => {
                g.once.remove(&root);
                g.reg.set(&id, &name, Answer::Always)
            }
            "never" => {
                g.once.remove(&root);
                g.reg.set(&id, &name, Answer::Never)
            }
            "once" => {
                g.once.insert(root);
                g.reg.forget(&id)
            }
            _ => return Err("That is not one of the choices.".into()),
        }
    };
    std::thread::spawn(move || rescan_now(&app));
    saved.map_err(|_| "Your answer is used for now, but it could not be saved, so you will be asked again next time.".to_string())
}

/// Forget what was said about a drive, so the question is asked again the next time it is seen.
#[tauri::command]
pub fn library_forget(id: String, app: tauri::AppHandle) -> Result<(), String> {
    let st = app.state::<AppState>();
    let saved = {
        let mut g = st.libs.lock().unwrap();
        g.ensure();
        let roots: Vec<PathBuf> = g.list.iter().filter(|l| l.info.id == id && l.state != State::Own).map(|l| l.info.root.clone()).collect();
        for r in roots {
            g.once.remove(&r);
        }
        g.reg.forget(&id)
    };
    std::thread::spawn(move || rescan_now(&app));
    saved.map_err(|_| "That could not be saved. It will be asked again the next time the program starts.".to_string())
}
