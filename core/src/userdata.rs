//! The user's own files, in the `user/` folder next to the program.
//! Plain files only: inventory.csv and family.json (JSON text, possibly an encrypted envelope).

use crate::fsio;
use crate::inventory::{self, Item};
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
    /// Save a readable plain-text copy in `exports/` (never locked). The name must be a plain file name ending in .txt.
    pub fn write_export(&self, name: &str, text: &str) -> Result<PathBuf, String> {
        let ok = name.ends_with(".txt") && name.len() > 4 && !name.contains(['/', '\\', ':']) && !name.starts_with('.');
        if !ok {
            return Err("That file name cannot be used.".into());
        }
        let path = self.root.join("exports").join(name);
        fsio::write_atomic(&path, text.as_bytes()).map_err(|e| format!("The copy could not be saved: {}", e))?;
        Ok(path)
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
}
