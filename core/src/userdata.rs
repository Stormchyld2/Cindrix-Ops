//! The user's own files, in the `user/` folder next to the program.
//! Plain files only: inventory.csv and family.json (JSON text, possibly an encrypted envelope).

use crate::fsio;
use crate::inventory::{self, Item};
use crate::ops::{self, Event, Project, Task};
use std::fs;
use std::path::{Path, PathBuf};

pub struct UserDir {
    pub root: PathBuf,
}

impl UserDir {
    pub fn new(root: &Path) -> UserDir {
        UserDir { root: root.to_path_buf() }
    }
    pub fn inventory_path(&self) -> PathBuf {
        self.root.join("inventory.csv")
    }
    pub fn family_path(&self) -> PathBuf {
        self.root.join("family.json")
    }
    fn safe_txt(name: &str) -> bool {
        name.ends_with(".txt") && name.len() > 4 && name.len() <= 120 && !name.contains(['/', '\\', ':']) && !name.starts_with('.')
    }
    fn write_txt(&self, dir: &str, name: &str, text: &str) -> Result<PathBuf, String> {
        if !Self::safe_txt(name) {
            return Err("That file name cannot be used.".into());
        }
        let path = self.root.join(dir).join(name);
        fsio::write_atomic(&path, text.as_bytes()).map_err(|e| format!("The copy could not be saved: {}", e))?;
        Ok(path)
    }
    /// Save a readable plain-text copy in `exports/` (never locked). The name must be a plain file name ending in .txt.
    pub fn write_export(&self, name: &str, text: &str) -> Result<PathBuf, String> {
        self.write_txt("exports", name, text)
    }
    /// Save a past Ask chat as a text file in `chats/`.
    pub fn write_chat(&self, name: &str, text: &str) -> Result<PathBuf, String> {
        if text.len() > 400_000 {
            return Err("That chat is too long to save.".into());
        }
        self.write_txt("chats", name, text)
    }
    /// The saved chats, newest first (at most 200): file name and modified time in seconds.
    pub fn list_chats(&self) -> Vec<(String, u64)> {
        let mut v: Vec<(String, u64)> = fs::read_dir(self.root.join("chats"))
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        if !Self::safe_txt(&n) {
                            return None;
                        }
                        let t = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
                        Some((n, t))
                    })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
        v.truncate(200);
        v
    }
    pub fn read_chat(&self, name: &str) -> Result<String, String> {
        if !Self::safe_txt(name) {
            return Err("That chat could not be found.".into());
        }
        fs::read_to_string(self.root.join("chats").join(name)).map_err(|_| "That chat could not be found.".to_string())
    }
    pub fn delete_chat(&self, name: &str) -> Result<(), String> {
        if !Self::safe_txt(name) {
            return Err("That chat could not be found.".into());
        }
        fs::remove_file(self.root.join("chats").join(name)).map_err(|e| e.to_string())
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    /// Missing file means an empty inventory. A damaged file is an error and is never overwritten silently.
    pub fn load_inventory(&self) -> Result<Vec<Item>, String> {
        match fs::read_to_string(self.inventory_path()) {
            Ok(t) => inventory::parse(&t).map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn save_inventory(&self, items: &[Item]) -> Result<(), String> {
        let csv = inventory::serialize(items);
        fsio::backup_then_write(&self.inventory_path(), &self.backups(), csv.as_bytes()).map_err(|e| e.to_string())
    }

    // ---------- tasks, calendar events, projects and the change log (core/src/ops.rs) ----------

    fn read_text(&self, name: &str) -> Result<String, String> {
        match fs::read_to_string(self.root.join(name)) {
            Ok(t) => Ok(t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(format!("{} could not be read: {}", name, e)),
        }
    }
    fn write_text(&self, name: &str, text: &str) -> Result<(), String> {
        fsio::backup_then_write(&self.root.join(name), &self.backups(), text.as_bytes()).map_err(|e| format!("{} could not be saved: {}", name, e))
    }
    pub fn load_tasks(&self) -> Result<Vec<Task>, String> {
        Ok(ops::parse_tasks(&self.read_text("tasks.txt")?))
    }
    pub fn save_tasks(&self, v: &[Task]) -> Result<(), String> {
        self.write_text("tasks.txt", &ops::serialize_tasks(v))
    }
    pub fn load_events(&self) -> Result<Vec<Event>, String> {
        Ok(ops::parse_events(&self.read_text("events.txt")?))
    }
    pub fn save_events(&self, v: &[Event]) -> Result<(), String> {
        self.write_text("events.txt", &ops::serialize_events(v))
    }
    pub fn load_projects(&self) -> Result<Vec<Project>, String> {
        Ok(ops::parse_projects(&self.read_text("projects.txt")?))
    }
    pub fn save_projects(&self, v: &[Project]) -> Result<(), String> {
        self.write_text("projects.txt", &ops::serialize_projects(v))
    }
    // ----- documents copied into a project (user/projects/<id>/) -----

    /// The most one copied document may weigh.
    pub const MAX_UPLOAD: usize = 25_000_000;

    fn project_folder(&self, id: &str) -> Result<PathBuf, String> {
        let ok = id.len() > 1 && id.len() < 12 && id.starts_with('p') && id[1..].chars().all(|c| c.is_ascii_digit());
        if !ok {
            return Err("There is no such project.".into());
        }
        Ok(self.root.join("projects").join(id))
    }
    /// The names of the files stored for a project.
    pub fn upload_names(&self, id: &str) -> Vec<String> {
        let Ok(d) = self.project_folder(id) else { return vec![] };
        let mut v: Vec<String> = fs::read_dir(d).map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
        v.sort();
        v
    }
    /// Copy a document into the project's folder. Returns the name it was stored under (a number is
    /// added when that name is taken, so nothing is ever overwritten).
    pub fn save_upload(&self, id: &str, name: &str, bytes: &[u8]) -> Result<String, String> {
        let dir = self.project_folder(id)?;
        let file = ops::safe_upload_name(name).ok_or("That kind of file cannot be added. Pictures, PDFs, text and office documents can.")?;
        if bytes.is_empty() {
            return Err("That file is empty.".into());
        }
        if bytes.len() > Self::MAX_UPLOAD {
            return Err("That file is too big (the limit is 25 MB). Link to it instead.".into());
        }
        let stored = ops::unique_upload_name(&self.upload_names(id), &file);
        fsio::write_atomic(&dir.join(&stored), bytes).map_err(|e| format!("Could not save the file: {}", e))?;
        Ok(stored)
    }
    /// Where a stored document is, when it exists. The name must be a plain file name.
    pub fn upload_path(&self, id: &str, stored: &str) -> Option<PathBuf> {
        if stored.contains(['/', '\\']) || stored.starts_with('.') {
            return None;
        }
        let p = self.project_folder(id).ok()?.join(stored);
        if p.is_file() { Some(p) } else { None }
    }
    pub fn delete_upload(&self, id: &str, stored: &str) {
        if let Some(p) = self.upload_path(id, stored) {
            let _ = fs::remove_file(p);
        }
    }

    /// Add one line to the change log (best effort: a failure here never stops the change itself).
    pub fn note_change(&self, now_secs: u64, kind: &str, id: &str, action: &str) {
        let mut text = self.read_text("changes.log").unwrap_or_default();
        text.push_str(&ops::change_line(now_secs, kind, id, action));
        text.push('\n');
        let _ = fsio::write_atomic(&self.root.join("changes.log"), ops::trim_log(&text, 300).as_bytes());
    }
    pub fn changes(&self, n: usize) -> Vec<(u64, String, String, String)> {
        ops::recent_changes(&self.read_text("changes.log").unwrap_or_default(), n)
    }

    pub fn read_family(&self) -> Result<Option<String>, String> {
        match fs::read_to_string(self.family_path()) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn write_family(&self, text: &str) -> Result<(), String> {
        fsio::backup_then_write(&self.family_path(), &self.backups(), text.as_bytes()).map_err(|e| e.to_string())
    }
}

/// Next free numeric id.
pub fn next_id(items: &[Item]) -> String {
    let max = items.iter().filter_map(|i| i.id.parse::<u64>().ok()).max().unwrap_or(0);
    (max + 1).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(tag: &str) -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-test-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn item(id: &str) -> Item {
        Item { id: id.into(), category: "Water".into(), name: "Jug".into(), location: "Pantry".into(),
               quantity: 3.0, unit: "gal".into(), use_per_day: 1.0, use_by: "".into(), notes: "".into(), ..Default::default() }
    }

    #[test]
    fn empty_when_missing() {
        let u = UserDir::new(&tmp("a"));
        assert!(u.load_inventory().unwrap().is_empty());
    }

    #[test]
    fn save_load_and_backup() {
        let dir = tmp("b");
        let u = UserDir::new(&dir);
        u.save_inventory(&[item("1")]).unwrap();
        u.save_inventory(&[item("1"), item("2")]).unwrap();
        assert_eq!(u.load_inventory().unwrap().len(), 2);
        let backups = fs::read_dir(u.backups()).unwrap().count();
        assert_eq!(backups, 1, "second save keeps the first version");
    }

    #[test]
    fn damaged_file_is_an_error_not_wiped() {
        let dir = tmp("c");
        let u = UserDir::new(&dir);
        fs::write(u.inventory_path(), "id,category\n1,Water\n").unwrap();
        assert!(u.load_inventory().is_err());
        assert!(fs::read_to_string(u.inventory_path()).unwrap().contains("1,Water"));
    }

    #[test]
    fn family_text_round_trip() {
        let u = UserDir::new(&tmp("d"));
        assert_eq!(u.read_family().unwrap(), None);
        u.write_family("{\"a\":1}").unwrap();
        assert_eq!(u.read_family().unwrap().as_deref(), Some("{\"a\":1}"));
    }

    #[test]
    fn ids_increase() {
        assert_eq!(next_id(&[]), "1");
        assert_eq!(next_id(&[item("4"), item("9")]), "10");
    }

    #[test]
    fn a_readable_copy_is_saved_in_exports_and_odd_names_are_refused() {
        let dir = std::env::temp_dir().join(format!("cx-export-{}", std::process::id()));
        let u = UserDir::new(&dir);
        let p = u.write_export("family-readable.txt", "Name: A\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "Name: A\n");
        assert!(p.starts_with(dir.join("exports")));
        for bad in ["../x.txt", "a/b.txt", "a\\b.txt", "x.json", ".txt", ".hidden.txt", "c:x.txt"] {
            assert!(u.write_export(bad, "x").is_err(), "{}", bad);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chats_are_saved_listed_read_and_deleted_by_plain_names_only() {
        let dir = std::env::temp_dir().join(format!("cx-chats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let u = UserDir::new(&dir);
        assert!(u.list_chats().is_empty());
        u.write_chat("2026-10-22-1100-water.txt", "YOU: water?\n").unwrap();
        u.write_chat("2026-10-22-1100-water.txt", "YOU: water?\nCINDRIX: boil it\n").unwrap();
        u.write_chat("b.txt", "x").unwrap();
        assert_eq!(u.list_chats().len(), 2);
        assert!(u.read_chat("2026-10-22-1100-water.txt").unwrap().contains("boil it"));
        assert!(u.read_chat("../family.json").is_err());
        assert!(u.write_chat("../x.txt", "x").is_err());
        u.delete_chat("b.txt").unwrap();
        assert_eq!(u.list_chats().len(), 1);
        assert!(u.delete_chat("a/b.txt").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tasks_events_and_projects_survive_a_save_and_a_second_save_keeps_a_backup() {
        let dir = std::env::temp_dir().join(format!("cx-ops-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let u = UserDir::new(&dir);
        assert!(u.load_tasks().unwrap().is_empty());
        let t = Task { id: "t1".into(), title: "Check water".into(), due: "2026-11-01".into(), ..Default::default() };
        u.save_tasks(&[t.clone()]).unwrap();
        u.save_tasks(&[t.clone(), Task { id: "t2".into(), title: "Two".into(), ..Default::default() }]).unwrap();
        assert_eq!(u.load_tasks().unwrap().len(), 2);
        assert!(fs::read_dir(u.backups()).unwrap().count() >= 1);
        u.save_events(&[Event { id: "e1".into(), title: "Call".into(), date: "2026-11-02".into(), ..Default::default() }]).unwrap();
        assert_eq!(u.load_events().unwrap()[0].title, "Call");
        u.save_projects(&[Project { id: "p1".into(), name: "Winter".into(), status: "open".into(), ..Default::default() }]).unwrap();
        assert_eq!(u.load_projects().unwrap()[0].name, "Winter");
        u.note_change(10, "task", "t1", "added");
        u.note_change(11, "task", "t2", "added");
        assert_eq!(u.changes(5)[0].2, "t2");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copied_documents_stay_in_the_project_folder_and_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("cindrix-ud-up-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let u = UserDir::new(&dir);
        let a = u.save_upload("p1", "C:\\phone\\Roof.JPG", b"one").unwrap();
        let b = u.save_upload("p1", "Roof.JPG", b"two").unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("Roof.JPG", "Roof (2).JPG"));
        assert_eq!(fs::read(u.upload_path("p1", &a).unwrap()).unwrap(), b"one");
        assert_eq!(u.upload_names("p1"), vec!["Roof (2).JPG", "Roof.JPG"]);
        assert!(u.upload_path("p2", &a).is_none()); // another project's folder
        assert!(u.upload_path("p1", "../tasks.txt").is_none());
        assert!(u.save_upload("../x", "a.pdf", b"x").is_err());
        assert!(u.save_upload("p1", "run.exe", b"x").is_err());
        assert!(u.save_upload("p1", "empty.pdf", b"").is_err());
        assert!(u.save_upload("p1", "big.pdf", &vec![0u8; UserDir::MAX_UPLOAD + 1]).is_err());
        u.delete_upload("p1", &a);
        assert!(u.upload_path("p1", &a).is_none());
        assert!(u.upload_path("p1", &b).is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
