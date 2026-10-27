//! Playlists: one CSV file each in `user/playlists/`, named after the playlist. A spreadsheet
//! opens them, and moving the drive keeps them. Columns: path, title, artist, album (the path is
//! under the music folder; the others are only for showing a track that has gone missing).

use super::{class_of_ext, valid_rel};
use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

pub const HEADER: &str = "path,title,artist,album";
pub const MAX_ENTRIES: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub name: String,
    pub count: usize,
}

const RESERVED: &[&str] = &["con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"];

/// Letters, digits, spaces and a little punctuation, up to 60 characters, and not a name Windows
/// reserves for devices (a file called CON.csv cannot be made there).
pub fn valid_name(name: &str) -> bool {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 60 || n != name || n.starts_with('.') || n.ends_with('.') {
        return false;
    }
    if !n.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.' | '(' | ')' | '\'')) {
        return false;
    }
    let stem = n.split('.').next().unwrap_or("").trim().to_lowercase();
    !RESERVED.contains(&stem.as_str())
}

fn entry_ok(e: &Entry) -> bool {
    valid_rel(&e.path) && e.path.rsplit_once('.').map(|(_, x)| class_of_ext(x).is_some()).unwrap_or(false)
}

fn flat(s: &str) -> String {
    quote(&s.replace(['\n', '\r'], " "))
}

pub struct Store {
    dir: PathBuf,
    backups: PathBuf,
}

impl Store {
    /// `user` is the drive's `user/` folder.
    pub fn new(user: &Path) -> Store {
        Store { dir: user.join("playlists"), backups: user.join("backups") }
    }

    /// One person's own playlists: `user/playlists/<person>/`. `Store::new` is the shared set everyone sees.
    pub fn for_person(user: &Path, person: &str) -> Result<Store, String> {
        let folder = crate::people::folder_name(person)?;
        Ok(Store { dir: user.join("playlists").join(folder), backups: user.join("backups") })
    }

    /// Every playlist of everyone: the shared ones (owner "") and each person's own (owner = their folder name).
    /// All of them can be played by anyone; only the owner changes their own.
    pub fn all(user: &Path) -> Vec<(String, Meta)> {
        let mut out: Vec<(String, Meta)> = Store::new(user).list().into_iter().map(|m| (String::new(), m)).collect();
        let mut owners: Vec<String> = fs::read_dir(user.join("playlists"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        owners.sort_by_key(|o| o.to_lowercase());
        for o in owners {
            if let Ok(s) = Store::for_person(user, &o) {
                out.extend(s.list().into_iter().map(|m| (o.clone(), m)));
            }
        }
        out
    }

    fn file(&self, name: &str) -> Result<PathBuf, String> {
        if valid_name(name) { Ok(self.dir.join(format!("{}.csv", name))) } else { Err("That is not a usable playlist name. Use letters, numbers and spaces, up to 60 characters.".into()) }
    }

    pub fn list(&self) -> Vec<Meta> {
        let Ok(rd) = fs::read_dir(&self.dir) else { return vec![] };
        let mut v: Vec<Meta> = rd
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                let name = n.strip_suffix(".csv")?.to_string();
                if !valid_name(&name) {
                    return None;
                }
                let count = fs::read_to_string(e.path()).map(|t| t.lines().count().saturating_sub(1)).unwrap_or(0);
                Some(Meta { name, count })
            })
            .collect();
        v.sort_by_key(|m| m.name.to_lowercase());
        v
    }

    /// The entries of a playlist. Lines that are not usable (a bad path, another file type) are
    /// left out rather than failing the whole list.
    pub fn load(&self, name: &str) -> Result<Vec<Entry>, String> {
        let text = fs::read_to_string(self.file(name)?).map_err(|_| "There is no such playlist.".to_string())?;
        let mut out = Vec::new();
        for line in text.lines().skip(1).take(MAX_ENTRIES) {
            let f = split_record(line);
            let get = |i: usize| f.get(i).cloned().unwrap_or_default();
            let e = Entry { path: get(0), title: get(1), artist: get(2), album: get(3) };
            if entry_ok(&e) {
                out.push(e);
            }
        }
        Ok(out)
    }

    /// Save (replace) a playlist. The old file is copied to `backups/` first.
    pub fn save(&self, name: &str, entries: &[Entry]) -> Result<(), String> {
        let file = self.file(name)?;
        if entries.len() > MAX_ENTRIES {
            return Err(format!("A playlist can hold up to {} tracks.", MAX_ENTRIES));
        }
        if let Some(bad) = entries.iter().find(|e| !entry_ok(e)) {
            return Err(format!("\"{}\" is not a track in the music folder.", bad.path));
        }
        let mut out = String::from(HEADER);
        out.push('\n');
        for e in entries {
            out.push_str(&format!("{},{},{},{}\n", flat(&e.path), flat(&e.title), flat(&e.artist), flat(&e.album)));
        }
        fsio::backup_then_write(&file, &self.backups, out.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn delete(&self, name: &str) -> Result<(), String> {
        let f = self.file(name)?;
        if !f.is_file() {
            return Err("There is no such playlist.".into());
        }
        fs::remove_file(f).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-pl-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn e(path: &str, title: &str) -> Entry {
        Entry { path: path.into(), title: title.into(), artist: "A, B".into(), album: "The \"Album\"".into() }
    }

    #[test]
    fn each_person_has_their_own_playlists_beside_the_shared_ones() {
        let u = tmp("people");
        let shared = Store::new(&u);
        let anna = Store::for_person(&u, "Anna").unwrap();
        let ben = Store::for_person(&u, "Ben").unwrap();
        anna.save("Road trip", &[e("Rock/a.mp3", "A"), e("Folk/b.flac", "B")]).unwrap();
        ben.save("Road trip", &[e("Jazz/c.ogg", "C")]).unwrap();
        shared.save("Everyone", &[e("Rock/a.mp3", "A")]).unwrap();
        assert_eq!(anna.load("Road trip").unwrap().len(), 2);
        assert_eq!(ben.load("Road trip").unwrap().len(), 1);
        assert_eq!(anna.list().iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["Road trip"]);
        assert_eq!(shared.list().iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["Everyone"]);
        assert!(anna.load("Everyone").is_err());
        anna.delete("Road trip").unwrap();
        assert_eq!(ben.load("Road trip").unwrap().len(), 1);
        
        assert!(Store::for_person(&u, "  ").is_err());
        // everyone can see everyone's lists, and a list can be read back through its owner's folder name
        ben.save("Evening", &[e("Jazz/c.ogg", "C")]).unwrap();
        let all: Vec<(String, String)> = Store::all(&u).into_iter().map(|(o, m)| (o, m.name)).collect();
        assert_eq!(all, vec![("".to_string(), "Everyone".to_string()), ("Ben".to_string(), "Evening".to_string()), ("Ben".to_string(), "Road trip".to_string())]);
        assert_eq!(Store::for_person(&u, "Ben").unwrap().load("Evening").unwrap().len(), 1);
    }

    #[test]
    fn names_are_safe_file_names() {
        for ok in ["Road trip", "Evening (calm)", "Mom's mix", "90s - best of", "Música", "a"] {
            assert!(valid_name(ok), "{:?}", ok);
        }
        for bad in ["", " ", " lead", "trail ", ".hidden", "x.", "a/b", "a\\b", "a:b", "CON", "con.txt", "Nul", "LPT1", "x*", "q?", "a\"b", &"x".repeat(61)] {
            assert!(!valid_name(bad), "{:?}", bad);
        }
    }

    #[test]
    fn save_load_list_delete_round_trip() {
        let user = tmp("rt");
        let s = Store::new(&user);
        assert!(s.list().is_empty());
        let items = vec![e("A/B/01.mp3", "One"), e("Loose, file.wma", "Two\nlines")];
        s.save("Road trip", &items).unwrap();
        s.save("Alpha", &[]).unwrap();
        let l = s.list();
        assert_eq!(l, vec![Meta { name: "Alpha".into(), count: 0 }, Meta { name: "Road trip".into(), count: 2 }]);
        let back = s.load("Road trip").unwrap();
        assert_eq!(back[0], items[0]);
        assert_eq!(back[1].path, "Loose, file.wma");
        assert_eq!(back[1].title, "Two lines"); // a line break cannot live in a CSV row
        assert_eq!(back[1].artist, "A, B");
        s.delete("Alpha").unwrap();
        assert!(s.delete("Alpha").is_err());
        assert!(s.load("Alpha").is_err());
        assert_eq!(s.list().len(), 1);
    }

    #[test]
    fn replacing_keeps_a_backup() {
        let user = tmp("bak");
        let s = Store::new(&user);
        s.save("Mix", &[e("a.mp3", "x")]).unwrap();
        s.save("Mix", &[e("a.mp3", "x"), e("b.mp3", "y")]).unwrap();
        assert_eq!(fs::read_dir(user.join("backups")).unwrap().count(), 1);
        assert_eq!(s.load("Mix").unwrap().len(), 2);
    }

    #[test]
    fn bad_paths_are_refused_on_save_and_skipped_on_load() {
        let user = tmp("bad");
        let s = Store::new(&user);
        for p in ["../x.mp3", "/abs.mp3", "a\\b.mp3", "notes.txt", "noext", "C:/x.mp3"] {
            assert!(s.save("Mix", &[e(p, "t")]).is_err(), "{:?}", p);
        }
        assert!(s.save("../evil", &[]).is_err());
        // a hand-edited file with a bad line
        fs::create_dir_all(user.join("playlists")).unwrap();
        fs::write(user.join("playlists/Hand.csv"), "path,title,artist,album\n../x.mp3,t,,\nok/a.flac,t,,\nnotes.txt,t,,\n").unwrap();
        let l = s.load("Hand").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].path, "ok/a.flac");
    }

    #[test]
    fn too_many_tracks_are_refused() {
        let s = Store::new(&tmp("big"));
        let many: Vec<Entry> = (0..=MAX_ENTRIES).map(|i| e(&format!("{}.mp3", i), "t")).collect();
        assert!(s.save("Big", &many).is_err());
    }
}
