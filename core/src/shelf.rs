//! The bookshelf. Books are files (PDF, Markdown or text) plus a small CSV list.
//! One shelf ships with the drive (read-only); the user's own shelf lives in `user/my-library/`
//! and takes any PDF the person adds, such as one they downloaded themselves.
//!
//! Layout of a shelf folder:
//!   books.csv            id,title,author,category,file,kind,added,license,fingerprint
//!   books/<file>         the book itself
//!   text/<id>.md         page-numbered text taken from the book, so the assistant can search it
//!   marks.csv            reading position, bookmarks, highlights and notes (user shelf only)

use crate::fsio;
use crate::inventory::{quote, split_record};
use crate::retrieval::{self, Passage};
use std::fs;
use std::path::{Path, PathBuf};

pub const HEADER: &str = "id,title,author,category,file,kind,added,license,fingerprint";
pub const MAX_BYTES: usize = 400 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: String,
    pub category: String,
    /// File name inside `books/`
    pub file: String,
    /// "pdf", "md" or "txt"
    pub kind: String,
    pub added: String,
    pub license: String,
    pub fingerprint: String,
}

pub fn parse(csv: &str) -> Result<Vec<Book>, String> {
    let mut v = Vec::new();
    for (n, line) in csv.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || (n == 0 && line.starts_with("id,")) {
            continue;
        }
        let f = split_record(line);
        if f.len() < 9 {
            return Err(format!("books.csv line {} has {} fields, expected 9", n + 1, f.len()));
        }
        v.push(Book { id: f[0].clone(), title: f[1].clone(), author: f[2].clone(), category: f[3].clone(), file: f[4].clone(),
                      kind: f[5].clone(), added: f[6].clone(), license: f[7].clone(), fingerprint: f[8].clone() });
    }
    Ok(v)
}

pub fn serialize(books: &[Book]) -> String {
    let mut out = String::from(HEADER);
    out.push('\n');
    for b in books {
        out.push_str(&format!("{},{},{},{},{},{},{},{},{}\n", quote(&b.id), quote(&b.title), quote(&b.author), quote(&b.category),
                              quote(&b.file), quote(&b.kind), quote(&b.added), quote(&b.license), quote(&b.fingerprint)));
    }
    out
}

/// "where_there_is_no-doctor (1).pdf" -> "where there is no doctor (1)"
pub fn title_from_filename(name: &str) -> String {
    let stem = match name.rsplit_once('.') {
        Some((s, ext)) if ext.len() <= 4 => s,
        _ => name,
    };
    let t: String = stem.chars().map(|c| if c == '_' || c == '-' { ' ' } else { c }).collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return "Untitled".to_string();
    }
    if t.chars().any(|c| c.is_uppercase()) {
        return t;
    }
    // all lower case file name: make it read like a title
    t.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Only these kinds can go on the shelf. Returns the lower-case extension.
pub fn kind_of(name: &str, bytes: &[u8]) -> Result<&'static str, String> {
    let ext = name.rsplit_once('.').map(|x| x.1.to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "pdf" => {
            if bytes.len() >= 5 && &bytes[..5] == b"%PDF-" { Ok("pdf") } else { Err("That file says it is a PDF but does not look like one.".into()) }
        }
        "md" => Ok("md"),
        "txt" => Ok("txt"),
        _ => Err("Only PDF, Markdown (.md) and text (.txt) books can be added.".into()),
    }
}

/// A short fingerprint of the file's bytes (FNV-1a, 64 bit) to spot the same book added twice.
pub fn fingerprint(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}-{}", h, bytes.len())
}

/// File names that sit on a drive but are not books: the small notes this program's own drive
/// builder writes, and license texts.
fn is_not_a_book(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with("readme") || n.starts_with("licen") || n.starts_with("copying") || n.starts_with("notice") || n.starts_with("source") || n.ends_with(".part") || n.ends_with(".tmp") || n.starts_with('.')
}

fn walk_for_books(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        // `file_type` does not follow links, so a link back up the tree cannot loop
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if ft.is_dir() {
            if depth > 0 && !name.starts_with('.') {
                walk_for_books(&path, depth - 1, out);
            }
        } else if ft.is_file() && !is_not_a_book(&name) {
            let ext = name.rsplit_once('.').map(|x| x.1.to_lowercase()).unwrap_or_default();
            if matches!(ext.as_str(), "pdf" | "md" | "txt") {
                out.push(path);
            }
        }
    }
}

/// PDF, Markdown and text files in the drive's book folders: `library` (and its subfolders, such
/// as `library/manuals`), `books` and `manuals`, up to four levels deep. `.zim` archives are not
/// books here (they have their own screen).
pub fn drive_book_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for d in ["library", "books", "manuals"] {
        walk_for_books(&root.join(d), 4, &mut out);
    }
    out
}

/// What one pass of putting the drive's books on the shelf did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImportReport {
    /// Books newly put on the shelf.
    pub added: Vec<String>,
    /// Files that were already on the shelf (the same book), or were looked at before.
    pub known: usize,
    /// Files that could not be used, with the reason, one line each.
    pub problems: Vec<String>,
}

pub struct Shelf {
    pub dir: PathBuf,
    pub prefix: &'static str,
}

impl Shelf {
    pub fn new(dir: &Path, prefix: &'static str) -> Shelf {
        Shelf { dir: dir.to_path_buf(), prefix }
    }
    fn csv(&self) -> PathBuf { self.dir.join("books.csv") }
    fn books_dir(&self) -> PathBuf { self.dir.join("books") }
    fn text_dir(&self) -> PathBuf { self.dir.join("text") }

    /// Missing list means an empty shelf. A damaged list is an error and is not overwritten.
    pub fn list(&self) -> Result<Vec<Book>, String> {
        match fs::read_to_string(self.csv()) {
            Ok(t) => parse(&t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(e.to_string()),
        }
    }

    fn save(&self, books: &[Book]) -> Result<(), String> {
        fsio::backup_then_write(&self.csv(), &self.dir.join("backups"), serialize(books).as_bytes()).map_err(|e| e.to_string())
    }

    fn next_id(&self, books: &[Book]) -> String {
        let max = books.iter().filter_map(|b| b.id.strip_prefix(self.prefix).and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0);
        format!("{}{}", self.prefix, max + 1)
    }

    /// Put a file on the shelf. `today` is "YYYY-MM-DD".
    pub fn add(&self, original_name: &str, bytes: &[u8], category: &str, today: &str) -> Result<Book, String> {
        self.add_with(original_name, bytes, category, today, "Added by you")
    }

    /// Like `add`, naming where the book came from (shown as its license line).
    pub fn add_with(&self, original_name: &str, bytes: &[u8], category: &str, today: &str, license: &str) -> Result<Book, String> {
        if bytes.is_empty() {
            return Err("That file is empty.".into());
        }
        if bytes.len() > MAX_BYTES {
            return Err("That file is larger than 400 MB.".into());
        }
        let kind = kind_of(original_name, bytes)?;
        let mut books = self.list()?;
        let fp = fingerprint(bytes);
        if let Some(b) = books.iter().find(|b| b.fingerprint == fp) {
            return Err(format!("That book is already on your shelf as \"{}\".", b.title));
        }
        let id = self.next_id(&books);
        let file = format!("{}.{}", id, kind);
        fsio::write_atomic(&self.books_dir().join(&file), bytes).map_err(|e| e.to_string())?;
        let book = Book {
            id, title: title_from_filename(original_name), author: String::new(),
            category: if category.is_empty() { "Mine".into() } else { category.into() },
            file, kind: kind.into(), added: today.into(), license: license.into(), fingerprint: fp,
        };
        books.push(book.clone());
        if let Err(e) = self.save(&books) {
            let _ = fs::remove_file(self.books_dir().join(&book.file));
            return Err(e);
        }
        Ok(book)
    }

    fn imported_file(&self) -> PathBuf { self.dir.join("drive-imported.txt") }

    /// Put the books found on the drive onto this shelf, once each. A file is remembered by its
    /// path, size and date, so a book the person later removes is not put back, and the next
    /// scan costs almost nothing. Changed files (a newer copy) count as new and are added unless
    /// the same bytes are already on the shelf.
    pub fn import_from_drive(&self, root: &Path, files: &[PathBuf], today: &str) -> ImportReport {
        let mut rep = ImportReport::default();
        let seen_text = fs::read_to_string(self.imported_file()).unwrap_or_default();
        let mut seen: Vec<String> = seen_text.lines().map(|l| l.to_string()).collect();
        let mut fresh: Vec<String> = Vec::new();
        for f in files {
            let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().replace('\\', "/");
            let Ok(meta) = fs::metadata(f) else { continue };
            let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            let key = format!("{}|{}|{}", rel, meta.len(), mtime);
            if seen.contains(&key) {
                rep.known += 1;
                continue;
            }
            let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if meta.len() as usize > MAX_BYTES {
                rep.problems.push(format!("{}: larger than 400 MB, left off the shelf", rel));
                seen.push(key.clone());
                fresh.push(key);
                continue;
            }
            let bytes = match fs::read(f) {
                Ok(b) => b,
                Err(e) => {
                    // could not be read right now (in use, drive busy): try again next time
                    rep.problems.push(format!("{}: could not be read ({})", rel, e));
                    continue;
                }
            };
            // the folder it sits in names its category: library/manuals/x.pdf -> "Manuals"
            let category = {
                let parts: Vec<&str> = rel.split('/').collect();
                let folder = if parts.len() >= 3 { parts[parts.len() - 2] } else if parts.len() == 2 && parts[0] != "library" { parts[0] } else { "" };
                if folder.is_empty() { "Drive library".to_string() } else { title_from_filename(&format!("{}.x", folder)) }
            };
            match self.add_with(&name, &bytes, &category, today, "From the drive") {
                Ok(b) => rep.added.push(b.title),
                Err(e) if e.starts_with("That book is already on your shelf") => rep.known += 1,
                Err(e) => rep.problems.push(format!("{}: {}", rel, e)),
            }
            seen.push(key.clone());
            fresh.push(key);
        }
        if !fresh.is_empty() {
            let mut all = seen_text;
            if !all.is_empty() && !all.ends_with('\n') {
                all.push('\n');
            }
            for k in &fresh {
                all.push_str(k);
                all.push('\n');
            }
            if let Err(e) = fsio::write_atomic(&self.imported_file(), all.as_bytes()) {
                rep.problems.push(format!("could not remember which books were added ({}); they may be looked at again", e));
            }
        }
        rep
    }

    pub fn update(&self, id: &str, title: &str, author: &str, category: &str) -> Result<(), String> {
        let mut books = self.list()?;
        let b = books.iter_mut().find(|b| b.id == id).ok_or("No such book.")?;
        if !title.trim().is_empty() { b.title = title.trim().to_string(); }
        b.author = author.trim().to_string();
        if !category.trim().is_empty() { b.category = category.trim().to_string(); }
        self.save(&books)
    }

    pub fn remove(&self, id: &str) -> Result<(), String> {
        let mut books = self.list()?;
        let b = books.iter().find(|b| b.id == id).cloned().ok_or("No such book.")?;
        books.retain(|x| x.id != id);
        self.save(&books)?;
        let _ = fs::remove_file(self.books_dir().join(&b.file));
        let _ = fs::remove_file(self.text_dir().join(format!("{}.md", id)));
        Ok(())
    }

    pub fn read(&self, id: &str) -> Result<Vec<u8>, String> {
        let books = self.list()?;
        let b = books.iter().find(|b| b.id == id).ok_or("No such book.")?;
        fs::read(self.books_dir().join(&b.file)).map_err(|e| e.to_string())
    }

    /// Save the text of each page (index 0 is page 1) so the assistant can search the book.
    pub fn save_pages(&self, id: &str, pages: &[String]) -> Result<(), String> {
        let books = self.list()?;
        if !books.iter().any(|b| b.id == id) {
            return Err("No such book.".into());
        }
        let mut out = String::new();
        for (i, p) in pages.iter().enumerate() {
            let text: String = p.trim().lines().map(|l| if l.trim_start().starts_with('#') { format!(" {}", l) } else { l.to_string() }).collect::<Vec<_>>().join("\n");
            if text.trim().len() < 20 {
                continue;
            }
            out.push_str(&format!("# page {}\n{}\n\n", i + 1, text));
        }
        fsio::write_atomic(&self.text_dir().join(format!("{}.md", id)), out.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn has_text(&self, id: &str) -> bool {
        self.text_dir().join(format!("{}.md", id)).exists()
    }

    /// Search the books' text. Each result names the book and the page.
    pub fn search(&self, question: &str, limit: usize) -> Vec<Passage> {
        let titles: Vec<(String, String)> = self.list().unwrap_or_default().into_iter().map(|b| (b.id, b.title)).collect();
        retrieval::search(&self.text_dir(), question, limit)
            .into_iter()
            .map(|mut p| {
                if let Some((_, t)) = titles.iter().find(|(id, _)| *id == p.source) {
                    p.source = t.clone();
                }
                p
            })
            .collect()
    }
}

// ---------- reading position, bookmarks, highlights, notes ----------

pub const MARKS_HEADER: &str = "book,kind,page,text";

#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub book: String,
    /// "last", "bookmark", "highlight" or "note"
    pub kind: String,
    pub page: u32,
    pub text: String,
}

pub struct Marks {
    pub path: PathBuf,
}

impl Marks {
    pub fn new(dir: &Path) -> Marks {
        Marks { path: dir.join("marks.csv") }
    }
    pub fn load(&self) -> Result<Vec<Mark>, String> {
        let t = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut v = Vec::new();
        for (n, line) in t.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.trim().is_empty() || (n == 0 && line.starts_with("book,")) {
                continue;
            }
            let f = split_record(line);
            if f.len() < 4 {
                return Err(format!("marks.csv line {} is damaged", n + 1));
            }
            v.push(Mark { book: f[0].clone(), kind: f[1].clone(), page: f[2].parse().unwrap_or(1), text: f[3].clone() });
        }
        Ok(v)
    }
    fn save(&self, marks: &[Mark]) -> Result<(), String> {
        let mut out = String::from(MARKS_HEADER);
        out.push('\n');
        for m in marks {
            out.push_str(&format!("{},{},{},{}\n", quote(&m.book), quote(&m.kind), m.page, quote(&m.text)));
        }
        fsio::write_atomic(&self.path, out.as_bytes()).map_err(|e| e.to_string())
    }
    pub fn for_book(&self, book: &str) -> Result<Vec<Mark>, String> {
        Ok(self.load()?.into_iter().filter(|m| m.book == book).collect())
    }
    pub fn last_page(&self, book: &str) -> u32 {
        self.for_book(book).ok().and_then(|v| v.into_iter().find(|m| m.kind == "last").map(|m| m.page)).unwrap_or(1)
    }
    pub fn set_last(&self, book: &str, page: u32) -> Result<(), String> {
        let mut all = self.load()?;
        all.retain(|m| !(m.book == book && m.kind == "last"));
        all.push(Mark { book: book.into(), kind: "last".into(), page, text: String::new() });
        self.save(&all)
    }
    pub fn add(&self, mark: Mark) -> Result<(), String> {
        let mut all = self.load()?;
        if mark.kind == "bookmark" && all.iter().any(|m| m.book == mark.book && m.kind == "bookmark" && m.page == mark.page) {
            return Ok(());
        }
        all.push(mark);
        self.save(&all)
    }
    /// Remove one mark: the n-th mark of that book and kind.
    pub fn remove(&self, book: &str, kind: &str, page: u32, text: &str) -> Result<(), String> {
        let mut all = self.load()?;
        if let Some(i) = all.iter().position(|m| m.book == book && m.kind == kind && m.page == page && m.text == text) {
            all.remove(i);
        }
        self.save(&all)
    }
    pub fn remove_book(&self, book: &str) -> Result<(), String> {
        let mut all = self.load()?;
        all.retain(|m| m.book != book);
        self.save(&all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(tag: &str) -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-shelf-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn pdf() -> Vec<u8> { b"%PDF-1.4\nfake body".to_vec() }

    fn drive_with_books() -> PathBuf {
        let root = tmp("drive");
        fs::create_dir_all(root.join("library/manuals")).unwrap();
        fs::create_dir_all(root.join("books")).unwrap();
        fs::create_dir_all(root.join("music")).unwrap();
        fs::write(root.join("library/manuals/fm_21-76-1.pdf"), pdf()).unwrap();
        fs::write(root.join("library/first aid.PDF"), b"%PDF-1.4\nother book").unwrap();
        fs::write(root.join("library/README.txt"), b"Put .zim library files here").unwrap();
        fs::write(root.join("library/big.zim"), b"not a book").unwrap();
        fs::write(root.join("library/unfinished.pdf.part"), b"%PDF-1.4 half").unwrap();
        fs::write(root.join("library/manuals/broken.pdf"), b"this is not a pdf").unwrap();
        fs::write(root.join("books/notes.md"), b"# Notes\nsome notes").unwrap();
        fs::write(root.join("music/song.txt"), b"not scanned: music folder").unwrap();
        root
    }

    #[test]
    fn the_drive_scan_finds_books_and_skips_everything_else() {
        let root = drive_with_books();
        let names: Vec<String> = drive_book_files(&root).iter().map(|p| p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(names, vec!["library/first aid.PDF", "library/manuals/broken.pdf", "library/manuals/fm_21-76-1.pdf", "books/notes.md"]);
        assert!(drive_book_files(&root.join("nothing")).is_empty());
    }

    #[test]
    fn drive_books_go_on_the_shelf_once_and_stay_removed() {
        let root = drive_with_books();
        let shelf = Shelf::new(&tmp("shelf"), "u");
        let files = drive_book_files(&root);
        let r = shelf.import_from_drive(&root, &files, "2026-10-20");
        let mut added = r.added.clone();
        added.sort();
        assert_eq!(added, vec!["First Aid", "Fm 21 76 1", "Notes"]);
        // the broken one is reported, and does not stop the others
        assert_eq!(r.problems.len(), 1, "{:?}", r.problems);
        assert!(r.problems[0].contains("broken.pdf"));
        let books = shelf.list().unwrap();
        assert_eq!(books.len(), 3);
        let fm = books.iter().find(|b| b.title == "Fm 21 76 1").unwrap();
        assert_eq!((fm.category.as_str(), fm.license.as_str()), ("Manuals", "From the drive"));
        assert_eq!(books.iter().find(|b| b.title == "First Aid").unwrap().category, "Drive library");
        assert_eq!(books.iter().find(|b| b.title == "Notes").unwrap().category, "Books");
        assert_eq!(shelf.read(&fm.id).unwrap(), pdf());

        // a second scan finds nothing new and does not report the broken file again
        let r2 = shelf.import_from_drive(&root, &drive_book_files(&root), "2026-10-21");
        assert!(r2.added.is_empty() && r2.problems.is_empty(), "{:?}", r2);
        assert_eq!(r2.known, 4);

        // a book the person removed does not come back
        shelf.remove(&fm.id).unwrap();
        let r3 = shelf.import_from_drive(&root, &drive_book_files(&root), "2026-10-22");
        assert!(r3.added.is_empty());
        assert_eq!(shelf.list().unwrap().len(), 2);

        // a new file on the drive is picked up
        fs::write(root.join("library/new guide.pdf"), b"%PDF-1.4\nnew guide").unwrap();
        let r4 = shelf.import_from_drive(&root, &drive_book_files(&root), "2026-10-23");
        assert_eq!(r4.added, vec!["New Guide"]);
    }

    #[test]
    fn a_book_already_on_the_shelf_is_not_added_twice() {
        let root = drive_with_books();
        let shelf = Shelf::new(&tmp("shelf2"), "u");
        shelf.add("my copy.pdf", &pdf(), "", "2026-10-01").unwrap();
        let r = shelf.import_from_drive(&root, &drive_book_files(&root), "2026-10-20");
        assert!(!r.added.contains(&"Fm 21 76 1".to_string()), "{:?}", r);
        assert_eq!(r.known, 1);
        assert_eq!(shelf.list().unwrap().len(), 3); // mine + first aid + notes; the manual is the same book as mine
    }

    #[test]
    fn titles_from_file_names() {
        assert_eq!(title_from_filename("where_there_is-no__doctor.pdf"), "Where There Is No Doctor");
        assert_eq!(title_from_filename("FM 21-76-1 Survival.pdf"), "FM 21 76 1 Survival");
        assert_eq!(title_from_filename("My Book (2).PDF"), "My Book (2)");
        assert_eq!(title_from_filename(".pdf"), "Untitled");
    }

    #[test]
    fn only_real_pdfs_md_and_txt() {
        assert_eq!(kind_of("a.pdf", &pdf()), Ok("pdf"));
        assert!(kind_of("a.pdf", b"not a pdf").is_err());
        assert!(kind_of("a.exe", b"MZ").is_err());
        assert_eq!(kind_of("notes.MD", b"# hi"), Ok("md"));
        assert_eq!(kind_of("x.txt", b"hi"), Ok("txt"));
    }

    #[test]
    fn csv_round_trip_with_commas_and_quotes() {
        let b = Book { id: "u1".into(), title: "Water, \"clean\"".into(), author: "A".into(), category: "Mine".into(), file: "u1.pdf".into(),
                       kind: "pdf".into(), added: "2026-10-18".into(), license: "Added by you".into(), fingerprint: "x".into() };
        let back = parse(&serialize(&[b.clone()])).unwrap();
        assert_eq!(back, vec![b]);
    }

    #[test]
    fn add_list_read_remove() {
        let s = Shelf::new(&tmp("a"), "u");
        let b = s.add("Where_There_Is_No_Doctor.pdf", &pdf(), "", "2026-10-18").unwrap();
        assert_eq!(b.id, "u1");
        assert_eq!(b.title, "Where There Is No Doctor");
        assert_eq!(b.category, "Mine");
        assert_eq!(s.list().unwrap().len(), 1);
        assert_eq!(s.read("u1").unwrap(), pdf());
        s.remove("u1").unwrap();
        assert!(s.list().unwrap().is_empty());
        assert!(!s.dir.join("books/u1.pdf").exists());
    }

    #[test]
    fn same_book_twice_is_refused_and_ids_increase() {
        let s = Shelf::new(&tmp("b"), "u");
        s.add("a.pdf", &pdf(), "", "2026-10-18").unwrap();
        let err = s.add("copy.pdf", &pdf(), "", "2026-10-18").unwrap_err();
        assert!(err.contains("already on your shelf"));
        let mut other = pdf();
        other.extend_from_slice(b"more");
        assert_eq!(s.add("b.pdf", &other, "", "2026-10-18").unwrap().id, "u2");
    }

    #[test]
    fn bad_files_are_refused_and_nothing_is_written() {
        let s = Shelf::new(&tmp("c"), "u");
        assert!(s.add("a.pdf", b"", "", "d").is_err());
        assert!(s.add("a.pdf", b"hello", "", "d").is_err());
        assert!(s.add("virus.exe", b"MZ", "", "d").is_err());
        assert!(s.list().unwrap().is_empty());
    }

    #[test]
    fn file_names_cannot_escape_the_folder() {
        let s = Shelf::new(&tmp("d"), "u");
        let b = s.add("../../evil.pdf", &pdf(), "", "d").unwrap();
        assert_eq!(b.file, "u1.pdf");
        assert!(s.dir.join("books/u1.pdf").exists());
    }

    #[test]
    fn rename_and_recategorize() {
        let s = Shelf::new(&tmp("e"), "u");
        s.add("a.pdf", &pdf(), "", "d").unwrap();
        s.update("u1", "Field Guide", "Someone", "Medical").unwrap();
        let b = &s.list().unwrap()[0];
        assert_eq!((b.title.as_str(), b.author.as_str(), b.category.as_str()), ("Field Guide", "Someone", "Medical"));
        assert!(s.update("nope", "x", "", "").is_err());
    }

    #[test]
    fn damaged_list_is_an_error_not_wiped() {
        let dir = tmp("f");
        fs::write(dir.join("books.csv"), "id,title\nu1,only two\n").unwrap();
        let s = Shelf::new(&dir, "u");
        assert!(s.list().is_err());
        assert!(s.add("a.pdf", &pdf(), "", "d").is_err());
        assert!(fs::read_to_string(dir.join("books.csv")).unwrap().contains("only two"));
    }

    #[test]
    fn search_cites_the_book_and_page() {
        let s = Shelf::new(&tmp("g"), "u");
        s.add("doc.pdf", &pdf(), "", "d").unwrap();
        s.update("u1", "Where There Is No Doctor", "", "").unwrap();
        s.save_pages("u1", &[
            "Contents and preface and nothing else useful here at all.".into(),
            "Treating a burn: cool the burn with clean running water for ten minutes. Do not use butter on a burn.".into(),
            "# a line that starts like a heading\nmore text about something else entirely here".into(),
        ]).unwrap();
        assert!(s.has_text("u1"));
        let r = s.search("how do I treat a burn with water", 3);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].source, "Where There Is No Doctor");
        assert_eq!(r[0].heading, "page 2");
        // page text starting with '#' stays inside its own page
        let r2 = s.search("heading something entirely", 3);
        assert!(r2.iter().all(|p| p.heading == "page 3"));
    }

    #[test]
    fn marks_position_bookmarks_notes() {
        let m = Marks::new(&tmp("h"));
        assert_eq!(m.last_page("u1"), 1);
        m.set_last("u1", 45).unwrap();
        m.set_last("u1", 46).unwrap();
        assert_eq!(m.last_page("u1"), 46);
        m.add(Mark { book: "u1".into(), kind: "bookmark".into(), page: 12, text: String::new() }).unwrap();
        m.add(Mark { book: "u1".into(), kind: "bookmark".into(), page: 12, text: String::new() }).unwrap();
        m.add(Mark { book: "u1".into(), kind: "note".into(), page: 14, text: "ask, \"neighbors\"".into() }).unwrap();
        m.add(Mark { book: "u2".into(), kind: "bookmark".into(), page: 3, text: String::new() }).unwrap();
        let mine = m.for_book("u1").unwrap();
        assert_eq!(mine.len(), 3); // last + 1 bookmark + 1 note
        assert!(mine.iter().any(|x| x.text == "ask, \"neighbors\""));
        m.remove("u1", "bookmark", 12, "").unwrap();
        assert_eq!(m.for_book("u1").unwrap().len(), 2);
        m.remove_book("u1").unwrap();
        assert!(m.for_book("u1").unwrap().is_empty());
        assert_eq!(m.for_book("u2").unwrap().len(), 1);
    }
}
