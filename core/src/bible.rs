//! Bible study: reads the files of Canonleaf's resource library straight from the drive.
//!
//! Canonleaf (the owner's separate Bible-study program) keeps every Bible, commentary, dictionary,
//! topical index and cross-reference list as ordinary JSON files in a folder per resource, each with
//! a `manifest.json` (schema version 2). This module finds those folders, and reads them without
//! Canonleaf running and without writing anything. Standard library only, like the rest of the core.
//!
//! Rules it keeps (see `docs/39-bible-study.md`):
//! - Read only. Nothing here changes a resource.
//! - Text is shown exactly as the resource has it. A passage a resource does not have is reported
//!   as missing, never filled in.
//! - Every answer says which resource it came from, so the assistant can name its source.
//! - A resource path in a manifest can never leave the resource's own folder.
//!
//! Where it looks: any folder below `library/bible` (on each library drive) that holds a
//! `manifest.json` of a supported kind. That covers Canonleaf's own library folder (pointed at the
//! drive), its three built-in Bibles copied over, and a `.theopack` file unzipped into a folder.

use crate::json::{self, Json};
use crate::retrieval::Passage;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ---------- the books ----------

pub struct Book {
    /// The three-character code Canonleaf's Bible files use (`GEN`, `1CO`, `JHN`).
    pub code: &'static str,
    pub name: &'static str,
    pub chapters: u32,
    aliases: &'static [&'static str],
}

macro_rules! book {
    ($code:expr, $name:expr, $ch:expr, [$($a:expr),*]) => {
        Book { code: $code, name: $name, chapters: $ch, aliases: &[$($a),*] }
    };
}

/// The 66 books in the usual order, with the chapters each has.
pub static BOOKS: [Book; 66] = [
    book!("GEN", "Genesis", 50, ["gen", "ge", "gn"]),
    book!("EXO", "Exodus", 40, ["exod", "ex"]),
    book!("LEV", "Leviticus", 27, ["lev", "le", "lv"]),
    book!("NUM", "Numbers", 36, ["num", "nu", "nm", "nb"]),
    book!("DEU", "Deuteronomy", 34, ["deut", "dt"]),
    book!("JOS", "Joshua", 24, ["josh", "jsh"]),
    book!("JDG", "Judges", 21, ["judg", "jg", "jdgs"]),
    book!("RUT", "Ruth", 4, ["ru"]),
    book!("1SA", "1 Samuel", 31, ["1sam", "1sm"]),
    book!("2SA", "2 Samuel", 24, ["2sam", "2sm"]),
    book!("1KI", "1 Kings", 22, ["1kgs", "1kg"]),
    book!("2KI", "2 Kings", 25, ["2kgs", "2kg"]),
    book!("1CH", "1 Chronicles", 29, ["1chron", "1chr"]),
    book!("2CH", "2 Chronicles", 36, ["2chron", "2chr"]),
    book!("EZR", "Ezra", 10, []),
    book!("NEH", "Nehemiah", 13, ["ne"]),
    book!("EST", "Esther", 10, ["esth", "es"]),
    book!("JOB", "Job", 42, ["jb"]),
    book!("PSA", "Psalms", 150, ["psalm", "ps", "pslm", "psm"]),
    book!("PRO", "Proverbs", 31, ["prov", "prv", "pr"]),
    book!("ECC", "Ecclesiastes", 12, ["eccl", "eccles", "ec"]),
    book!("SNG", "Song of Solomon", 8, ["songofsongs", "song", "sos", "ss", "canticles", "cant", "sg"]),
    book!("ISA", "Isaiah", 66, ["is"]),
    book!("JER", "Jeremiah", 52, ["je", "jr"]),
    book!("LAM", "Lamentations", 5, ["la"]),
    book!("EZK", "Ezekiel", 48, ["ezek", "eze"]),
    book!("DAN", "Daniel", 12, ["da", "dn"]),
    book!("HOS", "Hosea", 14, ["ho"]),
    book!("JOL", "Joel", 3, ["jl"]),
    book!("AMO", "Amos", 9, ["am"]),
    book!("OBA", "Obadiah", 1, ["obad", "ob"]),
    book!("JON", "Jonah", 4, ["jnh"]),
    book!("MIC", "Micah", 7, ["mc"]),
    book!("NAM", "Nahum", 3, ["nah", "na"]),
    book!("HAB", "Habakkuk", 3, ["hb"]),
    book!("ZEP", "Zephaniah", 3, ["zeph", "zp"]),
    book!("HAG", "Haggai", 2, ["hg"]),
    book!("ZEC", "Zechariah", 14, ["zech", "zc"]),
    book!("MAL", "Malachi", 4, ["ml"]),
    book!("MAT", "Matthew", 28, ["matt", "mt"]),
    book!("MRK", "Mark", 16, ["mk", "mr"]),
    book!("LUK", "Luke", 24, ["lk"]),
    book!("JHN", "John", 21, ["jn"]),
    book!("ACT", "Acts", 28, ["ac"]),
    book!("ROM", "Romans", 16, ["ro", "rm"]),
    book!("1CO", "1 Corinthians", 16, ["1cor"]),
    book!("2CO", "2 Corinthians", 13, ["2cor"]),
    book!("GAL", "Galatians", 6, ["ga"]),
    book!("EPH", "Ephesians", 6, ["ephes"]),
    book!("PHP", "Philippians", 4, ["phil", "pp"]),
    book!("COL", "Colossians", 4, []),
    book!("1TH", "1 Thessalonians", 5, ["1thess", "1thes"]),
    book!("2TH", "2 Thessalonians", 3, ["2thess", "2thes"]),
    book!("1TI", "1 Timothy", 6, ["1tim"]),
    book!("2TI", "2 Timothy", 4, ["2tim"]),
    book!("TIT", "Titus", 3, []),
    book!("PHM", "Philemon", 1, ["philem", "phlm"]),
    book!("HEB", "Hebrews", 13, []),
    book!("JAS", "James", 5, ["jm", "jam"]),
    book!("1PE", "1 Peter", 5, ["1pet", "1pt"]),
    book!("2PE", "2 Peter", 3, ["2pet", "2pt"]),
    book!("1JN", "1 John", 5, ["1jo", "1jhn"]),
    book!("2JN", "2 John", 1, ["2jo", "2jhn"]),
    book!("3JN", "3 John", 1, ["3jo", "3jhn"]),
    book!("JUD", "Jude", 1, ["jd"]),
    book!("REV", "Revelation", 22, ["revelations", "rv", "apocalypse"]),
];

pub fn book_by_code(code: &str) -> Option<&'static Book> {
    BOOKS.iter().find(|b| b.code.eq_ignore_ascii_case(code))
}

fn book_index(code: &str) -> Option<usize> {
    BOOKS.iter().position(|b| b.code.eq_ignore_ascii_case(code))
}

/// Lower-case letters and digits only: "Song of Solomon" and "song-of-solomon" meet at the same key.
fn key(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

fn alias_map() -> &'static HashMap<String, usize> {
    static MAP: OnceLock<HashMap<String, usize>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for (i, b) in BOOKS.iter().enumerate() {
            let mut keys = vec![key(b.name), key(b.code)];
            // "1 Samuel" is also written "1Samuel"; the digit stays in the key either way
            keys.extend(b.aliases.iter().map(|a| key(a)));
            // the bare name of a numbered book ("1 John" -> "1john") is already its key; add the short name too
            for k in keys {
                m.entry(k).or_insert(i);
            }
        }
        m
    })
}

/// The book a few words name: "John", "1 Cor", "I Corinthians", "First John", "Song of Solomon".
fn lookup_book(words: &[&str]) -> Option<usize> {
    if words.is_empty() || words.len() > 4 {
        return None;
    }
    let mut parts: Vec<String> = words.iter().map(|w| key(w)).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    if parts.len() > 1 {
        let n = match parts[0].as_str() {
            "i" | "1st" | "first" => Some("1"),
            "ii" | "2nd" | "second" => Some("2"),
            "iii" | "3rd" | "third" => Some("3"),
            _ => None,
        };
        if let Some(n) = n {
            parts[0] = n.to_string();
        }
    }
    alias_map().get(&parts.concat()).copied()
}

// ---------- references ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Ref {
    pub book: &'static str,
    pub chapter: u32,
    pub verse: Option<u32>,
    pub verse_end: Option<u32>,
}

impl Ref {
    pub fn book_name(&self) -> &'static str {
        book_by_code(self.book).map(|b| b.name).unwrap_or(self.book)
    }
    /// The form Canonleaf's files use: `JHN.3.16` (or `JHN.3` for a whole chapter).
    pub fn code(&self) -> String {
        match self.verse {
            Some(v) => format!("{}.{}.{}", self.book, self.chapter, v),
            None => format!("{}.{}", self.book, self.chapter),
        }
    }
    /// The form a person reads: `John 3:16-18`.
    pub fn label(&self) -> String {
        match (self.verse, self.verse_end) {
            (Some(v), Some(e)) if e > v => format!("{} {}:{}\u{2013}{}", self.book_name(), self.chapter, v, e),
            (Some(v), _) => format!("{} {}:{}", self.book_name(), self.chapter, v),
            _ => format!("{} {}", self.book_name(), self.chapter),
        }
    }
}

/// Chapter, then optionally `:verse` and `-end`, at the start of `s`. Anything after a comma or a
/// semicolon ("1:2,8,11") is ignored; any other trailing text means this is not a reference.
fn parse_spec(s: &str) -> Option<(u32, Option<u32>, Option<u32>)> {
    let s = s.trim_start_matches('(').trim_end_matches(|c: char| matches!(c, '.' | ')' | '"' | '\'' | '?' | '!' | ':'));
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let num = |i: &mut usize| -> Option<u32> {
        let st = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == st || *i - st > 3 {
            return None;
        }
        b[st..*i].iter().collect::<String>().parse().ok()
    };
    let chapter = num(&mut i)?;
    let (mut verse, mut end) = (None, None);
    if i < b.len() && b[i] == ':' {
        i += 1;
        verse = Some(num(&mut i)?);
        if i < b.len() && matches!(b[i], '-' | '\u{2013}' | '\u{2014}') {
            let save = i;
            i += 1;
            match num(&mut i) {
                Some(e) => end = Some(e),
                None => i = save,
            }
        }
    }
    if i < b.len() && !matches!(b[i], ',' | ';') {
        return None;
    }
    if chapter == 0 || verse == Some(0) {
        return None;
    }
    Some((chapter, verse, end))
}

fn make_ref(idx: usize, spec: (u32, Option<u32>, Option<u32>)) -> Option<Ref> {
    let b = &BOOKS[idx];
    if spec.0 > b.chapters {
        return None;
    }
    Some(Ref { book: b.code, chapter: spec.0, verse: spec.1, verse_end: spec.2 })
}

/// A whole string that is one reference: "John 3:16", "1 Cor 13", "Ps 23:1-3", "Rev 1:2,8,11".
pub fn parse_reference(s: &str) -> Option<Ref> {
    let words: Vec<&str> = s.split_whitespace().collect();
    if words.len() < 2 {
        return None;
    }
    let (spec, book) = words.split_last()?;
    let idx = lookup_book(book)?;
    make_ref(idx, parse_spec(spec)?)
}

/// `JHN.3.16` or `JHN.3` (Canonleaf's form).
pub fn parse_osis(s: &str) -> Option<Ref> {
    let mut it = s.trim().split('.');
    let idx = book_index(it.next()?)?;
    let chapter: u32 = it.next()?.parse().ok()?;
    let verse: Option<u32> = match it.next() {
        Some(v) => Some(v.parse().ok()?),
        None => None,
    };
    if it.next().is_some() {
        return None;
    }
    make_ref(idx, (chapter, verse, None))
}

/// Names that are also everyday words or names: in running text they count only with a verse
/// ("Job 3:2", not "job 3").
const COMMON_WORDS: &[&str] = &[
    "job", "mark", "acts", "numbers", "ruth", "judges", "amos", "jude", "james", "luke", "joel", "titus", "esther", "daniel",
];
/// Two-letter forms safe to take as a book in running text.
const SAFE_SHORT: &[&str] = &["ps", "jn", "mt", "mk", "lk"];

/// The references written in a piece of text ("what does John 3:16 mean", "compare Rom 8 and Gal 5").
pub fn find_references(text: &str) -> Vec<Ref> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<Ref> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut hit = None;
        for len in (1..=4usize).rev() {
            if i + len >= words.len() {
                continue;
            }
            let book: Vec<&str> = words[i..i + len].iter().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric())).collect();
            let Some(idx) = lookup_book(&book) else { continue };
            let Some(spec) = parse_spec(words[i + len]) else { continue };
            let k = key(&book.concat());
            if k.len() < 3 && !SAFE_SHORT.contains(&k.as_str()) {
                continue;
            }
            if spec.1.is_none() && COMMON_WORDS.contains(&key(BOOKS[idx].name).as_str()) {
                continue;
            }
            if let Some(r) = make_ref(idx, spec) {
                hit = Some((r, len + 1));
                break;
            }
        }
        match hit {
            Some((r, used)) => {
                if !out.contains(&r) {
                    out.push(r);
                }
                i += used;
            }
            None => i += 1,
        }
    }
    out
}

// ---------- finding the resources ----------

/// What a resource can be. Others (and old schema-1 test files) are left alone.
mod mentions;
pub use mentions::Mention;

pub const SUPPORTED_KINDS: &[&str] = &["bible", "commentary", "dictionary", "topical", "concordance", "theology"];

const MAX_SCAN_DEPTH: usize = 5;
const MAX_MODULES: usize = 200;
const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
struct MFile {
    path: String,
    role: String,
    book: String,
}

/// One installed resource: a Bible, a commentary, a dictionary and so on.
#[derive(Debug, Clone)]
pub struct Module {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub license: String,
    pub attribution: String,
    dir: PathBuf,
    files: Vec<MFile>,
}

impl Module {
    /// A file of this resource, by the path its manifest gives. Nothing outside the resource's own
    /// folder, no links, no odd paths.
    fn member(&self, rel: &str) -> Option<PathBuf> {
        if rel.is_empty() || rel.len() > 300 || rel.starts_with('/') || rel.contains('\\') || rel.contains(':') {
            return None;
        }
        if rel.split('/').any(|p| p.is_empty() || p == "." || p == "..") {
            return None;
        }
        let p = self.dir.join(rel);
        let md = fs::symlink_metadata(&p).ok()?;
        if md.file_type().is_symlink() || !md.is_file() {
            return None;
        }
        Some(p)
    }
    fn role_files<'a>(&'a self, role: &'a str) -> impl Iterator<Item = &'a MFile> + 'a {
        self.files.iter().filter(move |f| f.role == role)
    }
}

fn read_json(p: &Path) -> Result<Json, String> {
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let len = fs::metadata(p).map_err(|_| format!("The Bible study file {} could not be opened.", name))?.len();
    if len > MAX_FILE {
        return Err(format!("The Bible study file {} is too large to open.", name));
    }
    let s = fs::read_to_string(p).map_err(|_| format!("The Bible study file {} could not be read.", name))?;
    json::parse(&s).map_err(|e| format!("{} ({})", e, name))
}

/// The text of a file that is read before it is parsed, with the same size limit as `read_json`.
fn read_text(p: &Path) -> Option<String> {
    if fs::metadata(p).ok()?.len() > MAX_FILE {
        return None;
    }
    fs::read_to_string(p).ok()
}

/// The ways a word may be written in a file: as typed, lower case, Capitalised, UPPER CASE.
fn spellings(term: &str) -> Vec<String> {
    let t = term.trim();
    let lower = t.to_lowercase();
    let mut cap = lower.clone();
    if let Some(c) = cap.chars().next() {
        cap = c.to_uppercase().collect::<String>() + &cap[c.len_utf8()..];
    }
    let mut v = vec![t.to_string(), lower, cap, t.to_uppercase()];
    v.dedup();
    v
}

fn safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 160 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn read_manifest(dir: &Path) -> Option<Module> {
    let path = dir.join("manifest.json");
    let md = fs::symlink_metadata(&path).ok()?;
    if md.file_type().is_symlink() || !md.is_file() || md.len() > MAX_MANIFEST {
        return None;
    }
    let j = json::parse(&fs::read_to_string(&path).ok()?).ok()?;
    if j.get("schema_version").and_then(|v| v.as_u32()) != Some(2) {
        return None;
    }
    let id = j.text("module_id").to_string();
    let kind = j.text("kind").to_string();
    if !safe_id(&id) || !SUPPORTED_KINDS.contains(&kind.as_str()) {
        return None;
    }
    let files: Vec<MFile> = j
        .list("files")
        .iter()
        .map(|f| MFile { path: f.text("path").to_string(), role: f.text("role").to_string(), book: f.text("book").to_string() })
        .filter(|f| !f.path.is_empty())
        .collect();
    let title = match j.text("title") {
        "" => id.clone(),
        t => t.to_string(),
    };
    Some(Module { id, title, kind, license: j.text("license").to_string(), attribution: j.text("attribution").to_string(), dir: dir.to_path_buf(), files })
}

fn scan(dir: &Path, depth: usize, out: &mut Vec<Module>) {
    if depth > MAX_SCAN_DEPTH || out.len() >= MAX_MODULES {
        return;
    }
    if let Some(m) = read_manifest(dir) {
        if !out.iter().any(|x| x.id == m.id) {
            out.push(m);
        }
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut subs: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir() && !t.is_symlink()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    subs.sort();
    for s in subs {
        let name = s.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if name.starts_with('.') || name == "cache" || name == "node_modules" {
            continue;
        }
        scan(&s, depth + 1, out);
    }
}

fn kind_rank(kind: &str) -> usize {
    SUPPORTED_KINDS.iter().position(|k| *k == kind).unwrap_or(99)
}

/// What a library drive offers: every resource found below the folders given.
pub struct Library {
    pub modules: Vec<Module>,
}

/// One verse of a chapter.
#[derive(Debug, Clone, PartialEq)]
pub struct Verse {
    pub number: String,
    pub text: String,
}

/// A verse found by a search.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub book: &'static str,
    pub chapter: u32,
    pub verse: String,
    pub text: String,
}

/// A commentary note: where it belongs ("Verse 16", "Introduction") and its words.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub label: String,
    pub text: String,
}

/// Cross-references for one verse: the words they hang on, and the places they point to.
#[derive(Debug, Clone, PartialEq)]
pub struct XRef {
    pub anchor: String,
    pub refs: Vec<String>,
}

/// A dictionary, topical or theology entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub module_id: String,
    pub kind: String,
    pub source: String,
    pub title: String,
    pub body: String,
    pub refs: Vec<String>,
}

/// A Hebrew or Greek lexicon entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Strongs {
    pub number: String,
    pub lemma: String,
    pub transliteration: String,
    pub gloss: String,
    pub morphology: String,
    pub source: String,
}

impl Library {
    /// Looks below each folder in `roots` (the `library/bible` folders of the drives that may be
    /// read). Folders that are not there are skipped.
    pub fn open(roots: &[PathBuf]) -> Library {
        let mut modules = Vec::new();
        for r in roots {
            scan(r, 0, &mut modules);
        }
        modules.sort_by(|a, b| kind_rank(&a.kind).cmp(&kind_rank(&b.kind)).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
        Library { modules }
    }

    pub fn module(&self, id: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }
    pub fn of_kind(&self, kind: &str) -> Vec<&Module> {
        self.modules.iter().filter(|m| m.kind == kind).collect()
    }
    /// The Bible to use when the person has not picked one: the Berean Standard Bible if it is
    /// there, else the first Bible found.
    pub fn default_bible(&self) -> Option<&Module> {
        self.module("bible-bsb").or_else(|| self.modules.iter().find(|m| m.kind == "bible"))
    }

    fn typed(&self, id: &str, kind: &str) -> Result<&Module, String> {
        match self.module(id) {
            Some(m) if m.kind == kind => Ok(m),
            _ => Err("That Bible study book is not on the drive any more.".to_string()),
        }
    }

    /// The books a Bible has, in the usual order.
    pub fn books_in(&self, bible: &Module) -> Vec<&'static Book> {
        BOOKS.iter().filter(|b| bible.role_files("book").any(|f| f.book.eq_ignore_ascii_case(b.code))).collect()
    }

    // ----- reading -----

    pub fn chapter(&self, bible_id: &str, book: &str, chapter: u32) -> Result<Vec<Verse>, String> {
        let m = self.typed(bible_id, "bible")?;
        let b = book_by_code(book).ok_or("That is not a book of the Bible.")?;
        let f = m
            .role_files("book")
            .find(|f| f.book.eq_ignore_ascii_case(b.code))
            .ok_or_else(|| format!("{} does not have {}.", m.title, b.name))?;
        let path = m.member(&f.path).ok_or_else(|| format!("The file for {} is missing from {}.", b.name, m.title))?;
        let j = read_json(&path)?;
        let c = j
            .list("chapters")
            .iter()
            .find(|c| c.get("chapter").and_then(|n| n.as_u32()) == Some(chapter))
            .ok_or_else(|| format!("{} does not have {} {}.", m.title, b.name, chapter))?;
        let verses = c
            .list("verses")
            .iter()
            .map(|v| Verse {
                number: match v.get("number") {
                    Some(Json::Str(s)) => s.clone(),
                    Some(Json::Num(n)) => format!("{}", *n as u64),
                    _ => String::new(),
                },
                text: v.text("text").trim_start_matches('\u{b6}').trim().to_string(),
            })
            .filter(|v| !v.text.is_empty())
            .collect();
        Ok(verses)
    }

    /// Verses of one Bible that hold all the words of `query` (a word also matches longer words
    /// that start with it, from four letters: "bless" finds "blessed"). Whole phrases come first.
    pub fn search(&self, bible_id: &str, query: &str, limit: usize) -> Result<Vec<Hit>, String> {
        let m = self.typed(bible_id, "bible")?;
        let q = words_of(query);
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let phrase = q.join(" ");
        let mut scored: Vec<(u32, usize, Hit)> = Vec::new();
        let mut order = 0usize;
        for b in BOOKS.iter() {
            let Some(f) = m.role_files("book").find(|f| f.book.eq_ignore_ascii_case(b.code)) else { continue };
            let Some(path) = m.member(&f.path) else { continue };
            let Ok(j) = read_json(&path) else { continue };
            for c in j.list("chapters") {
                let Some(ch) = c.get("chapter").and_then(|n| n.as_u32()) else { continue };
                for v in c.list("verses") {
                    let text = v.text("text");
                    let toks = words_of(text);
                    if q.iter().all(|w| toks.iter().any(|t| t == w || (w.len() >= 4 && t.starts_with(w.as_str())))) {
                        let score = if toks.join(" ").contains(&phrase) { 2 } else { 1 };
                        let number = match v.get("number") {
                            Some(Json::Str(s)) => s.clone(),
                            Some(Json::Num(n)) => format!("{}", *n as u64),
                            _ => String::new(),
                        };
                        scored.push((score, order, Hit { book: b.code, chapter: ch, verse: number, text: text.trim_start_matches('\u{b6}').trim().to_string() }));
                        order += 1;
                    }
                }
            }
            if scored.len() > 4000 {
                break;
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        Ok(scored.into_iter().take(limit).map(|x| x.2).collect())
    }

    /// The file of a resource that belongs to one book, found by the book named in the file's name
    /// ("john.json", "1-john.json", "JHN.json", "1%20Chr.json").
    fn book_file(&self, m: &Module, role: &str, prefix: Option<&str>, code: &str) -> Option<PathBuf> {
        let want = book_index(code)?;
        let f = m.role_files(role).find(|f| prefix.map_or(true, |p| f.path.starts_with(p)) && book_of_file(&f.path) == Some(want))?;
        m.member(&f.path)
    }

    /// Commentary for a chapter, or for one verse of it. A commentary that does not cover the
    /// book gives an empty list (not an error).
    pub fn notes(&self, commentary_id: &str, book: &str, chapter: u32, verse: Option<u32>) -> Result<Vec<Note>, String> {
        let m = self.typed(commentary_id, "commentary")?;
        let Some(path) = self.book_file(m, "notes", None, book) else { return Ok(Vec::new()) };
        let j = read_json(&path)?;
        if j.get("data").is_some() {
            return Ok(notes_from_entries(&j, chapter, verse));
        }
        if j.get("chapters").is_some() || j.get("pages").is_some() {
            return Ok(notes_from_chapters(&j, chapter, verse));
        }
        Ok(Vec::new())
    }

    /// The cross-reference list that goes with a Bible (same edition when there is one), if any.
    pub fn crossref_module_for(&self, bible_id: &str) -> Option<&Module> {
        let edition = bible_id.rsplit('-').next().unwrap_or("");
        let lists: Vec<&Module> = self.modules.iter().filter(|m| m.kind == "concordance" && m.role_files("notes").any(|f| f.path.starts_with("crossrefs/"))).collect();
        lists.iter().find(|m| m.id.ends_with(&format!("-{}", edition))).or_else(|| lists.first()).copied()
    }

    pub fn crossrefs(&self, module_id: &str, book: &str, chapter: u32, verse: u32) -> Result<Vec<XRef>, String> {
        let m = self.typed(module_id, "concordance")?;
        let Some(path) = self.book_file(m, "notes", Some("crossrefs/"), book) else { return Ok(Vec::new()) };
        let j = read_json(&path)?;
        let rows: &[Json] = j.as_arr().unwrap_or(&[]);
        Ok(rows
            .iter()
            .filter(|r| r.get("chapter").and_then(|n| n.as_u32()) == Some(chapter) && r.get("verse").and_then(|n| n.as_u32()) == Some(verse))
            .map(|r| XRef { anchor: r.text("anchor").to_string(), refs: r.list("references").iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect() })
            .filter(|x| !x.refs.is_empty())
            .collect())
    }

    /// Dictionary, topical and theology entries whose heading is `term` (or starts with it),
    /// best matches first.
    pub fn define(&self, term: &str, limit: usize) -> Vec<Entry> {
        let q = key(term.trim_end_matches('s'));
        let q_full = key(term);
        if q_full.is_empty() {
            return Vec::new();
        }
        let first = q_full.chars().next().unwrap_or(' ');
        let alt_spellings: Vec<String> = spellings(term).into_iter().chain(spellings(term.trim_end_matches('s'))).collect();
        let mut found: Vec<(u8, Entry)> = Vec::new();
        for m in self.modules.iter().filter(|m| matches!(m.kind.as_str(), "dictionary" | "topical" | "theology")) {
            for f in m.role_files("articles") {
                // a dictionary split by letter: only the file for this word's letter
                let stem = f.path.rsplit('/').next().unwrap_or("").trim_end_matches(".json");
                let other_letter = stem.len() == 1 && stem.chars().next().map(|c| c.to_ascii_lowercase()) != Some(first);
                let Some(path) = m.member(&f.path) else { continue };
                let j = if other_letter {
                    // An entry in another letter's file can still list this word as another name for itself
                    // ("Favour" under G for Grace). Looking is cheap: the file is only opened as text, and it
                    // is read as JSON only if the word is written in it somewhere.
                    let Some(raw) = read_text(&path) else { continue };
                    if !alt_spellings.iter().any(|w| raw.contains(w.as_str())) {
                        continue;
                    }
                    match json::parse(&raw) {
                        Ok(j) => j,
                        Err(_) => continue,
                    }
                } else {
                    let Ok(j) = read_json(&path) else { continue };
                    j
                };
                for e in entries_of(&j) {
                    let title = entry_title(e);
                    if title.is_empty() {
                        continue;
                    }
                    let t = key(title);
                    let alt_hit = e.list("alt_terms").iter().filter_map(|a| a.as_str()).any(|a| key(a) == q_full || key(a) == q);
                    if other_letter && !alt_hit {
                        continue;
                    }
                    let score = if t == q_full || t == q || alt_hit {
                        3
                    } else if t.starts_with(&q_full) && q_full.len() >= 3 {
                        2
                    } else {
                        0
                    };
                    if score > 0 {
                        found.push((score, Entry { module_id: m.id.clone(), kind: m.kind.clone(), source: m.title.clone(), title: title.to_string(), body: clip(&entry_body(e), 20_000), refs: entry_refs(e) }));
                    }
                }
            }
        }
        found.sort_by(|a, b| b.0.cmp(&a.0));
        found.into_iter().take(limit).map(|x| x.1).collect()
    }

    /// Hebrew and Greek lexicon entries for a Strong's number ("G5485", "h430").
    pub fn strongs(&self, number: &str) -> Vec<Strongs> {
        let Some(want) = strongs_key(number) else { return Vec::new() };
        let mut out = Vec::new();
        for m in self.modules.iter().filter(|m| m.kind == "dictionary") {
            for f in m.role_files("lexicon").filter(|f| f.path.ends_with("entries.json")) {
                let Some(path) = m.member(&f.path) else { continue };
                let Ok(j) = read_json(&path) else { continue };
                for e in j.list("entries") {
                    let ext = e.text("extended_strong");
                    let rest = ext.strip_prefix(want.as_str());
                    if matches!(rest, Some(r) if r.chars().all(|c| c.is_ascii_alphabetic())) {
                        out.push(Strongs {
                            number: ext.to_string(),
                            lemma: e.text("lemma").to_string(),
                            transliteration: e.text("transliteration").to_string(),
                            gloss: e.text("gloss").to_string(),
                            morphology: e.text("morphology").to_string(),
                            source: m.title.clone(),
                        });
                        if out.len() >= 10 {
                            return out;
                        }
                    }
                }
            }
        }
        out
    }

    // ----- for the assistant -----

    /// Passages that can answer a question: the verses of any reference written in it, a
    /// commentary note on the first one, and (only for a question that sounds like a Bible
    /// question) dictionary and topical entries for its words. Each carries the name of the book
    /// it came from, so the answer can say so.
    pub fn passages(&self, question: &str, limit: usize) -> Vec<Passage> {
        let mut out: Vec<Passage> = Vec::new();
        let refs = find_references(question);
        if let Some(bible) = self.default_bible() {
            for r in refs.iter().take(2) {
                if let Some(p) = self.verse_passage(bible, r) {
                    out.push(p);
                }
            }
        }
        if let (Some(r), true) = (refs.first(), refs.first().map_or(false, |r| r.verse.is_some())) {
            if let Some(p) = self.commentary_passage(r) {
                out.push(p);
            }
        }
        if refs.is_empty() && looks_biblical(question) {
            // "What does the Bible say about forgiveness?": the topical index first (it lists the
            // passages), then the dictionaries. The words that only say "Bible question" are not looked up.
            let mut words: Vec<String> = crate::retrieval::tokens(question).into_iter().filter(|w| w.len() > 2 && !FILLER.contains(&w.as_str())).collect();
            words.sort_by(|a, b| b.len().cmp(&a.len()));
            let mut found: Vec<Entry> = Vec::new();
            for w in words.into_iter().take(4) {
                for e in self.define(&w, 8).into_iter().filter(|e| key(&e.title) == key(&w) || key(&e.title) == key(w.trim_end_matches('s'))) {
                    if !found.iter().any(|x| x.module_id == e.module_id && x.title == e.title) {
                        found.push(e);
                    }
                }
            }
            let rank = |k: &str| match k {
                "topical" => 0,
                "theology" => 1,
                _ => 2,
            };
            found.sort_by_key(|e| rank(&e.kind));
            for e in found.into_iter().take(3) {
                let mut text = clip(&e.body, 1400);
                if !e.refs.is_empty() {
                    text.push_str(&format!("\nPassages: {}", e.refs.iter().take(12).cloned().collect::<Vec<_>>().join("; ")));
                }
                out.push(Passage { source: e.source.clone(), heading: e.title.clone(), text, score: 20.0, link: format!("bible:term:{}", e.title) });
            }
        }
        out.truncate(limit);
        out
    }

    fn verse_passage(&self, bible: &Module, r: &Ref) -> Option<Passage> {
        let verses = self.chapter(&bible.id, r.book, r.chapter).ok()?;
        let picked: Vec<&Verse> = match r.verse {
            Some(from) => {
                let to = r.verse_end.unwrap_or(from).max(from).min(from + 14);
                verses.iter().filter(|v| v.number.parse::<u32>().map_or(false, |n| n >= from && n <= to)).collect()
            }
            // a whole chapter: the first dozen verses it has
            None => verses.iter().take(12).collect(),
        };
        if picked.is_empty() {
            return None;
        }
        let text = picked.iter().map(|v| format!("{} {}", v.number, v.text)).collect::<Vec<_>>().join(" ");
        Some(Passage { source: bible.title.clone(), heading: r.label(), text, score: 50.0, link: format!("bible:{}", r.code()) })
    }

    fn commentary_passage(&self, r: &Ref) -> Option<Passage> {
        for m in self.of_kind("commentary") {
            if let Ok(notes) = self.notes(&m.id, r.book, r.chapter, r.verse) {
                if let Some(n) = notes.first() {
                    return Some(Passage {
                        source: m.title.clone(),
                        heading: format!("{} \u{2013} {}", r.label(), n.label),
                        text: clip(&n.text, 1500),
                        score: 40.0,
                        link: format!("bible:{}", r.code()),
                    });
                }
            }
        }
        None
    }
}

/// Does a question sound like it is about the Bible? Only then do dictionary and topical entries
/// join the assistant's sources, so "how do I purify water" never pulls in a Bible dictionary.
pub fn looks_biblical(question: &str) -> bool {
    const WORDS: &[&str] = &[
        "bible", "biblical", "scripture", "scriptures", "verse", "verses", "gospel", "gospels", "psalm", "psalms", "jesus", "christ", "god",
        "lord", "holy", "spirit", "prayer", "pray", "sin", "sins", "faith", "commentary", "testament", "apostle", "apostles", "prophet",
        "prophets", "covenant", "sabbath", "tithe", "baptism", "resurrection", "salvation", "righteous", "righteousness", "heaven",
        "church", "scriptural", "theology", "theological", "christian", "christians", "disciple", "disciples", "angel", "angels",
    ];
    words_of(question).iter().any(|w| WORDS.contains(&w.as_str()))
}

// ---------- helpers ----------

/// Words of a Bible question that say what kind of question it is, not what it is about.
const FILLER: &[&str] = &[
    "bible", "biblical", "scripture", "scriptures", "scriptural", "verse", "verses", "say", "says", "said", "teach", "teaches", "tell", "tells",
    "mean", "means", "meaning", "according", "about", "chapter", "book", "passage", "passages", "christian", "christians",
];

fn words_of(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase()).collect()
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max).collect();
    t.push('\u{2026}');
    t
}

/// The book a file name stands for.
fn book_of_file(path: &str) -> Option<usize> {
    let name = path.rsplit('/').next()?;
    let stem = name.strip_suffix(".json").unwrap_or(name);
    let decoded = crate::share::url_decode(&stem.replace('+', "%2B"));
    let words: Vec<&str> = decoded.split(|c: char| c == ' ' || c == '-' || c == '_').filter(|w| !w.is_empty()).collect();
    lookup_book(&words)
}

/// "g5485" -> "G5485"; numbers are four digits wide in the lexicons.
fn strongs_key(s: &str) -> Option<String> {
    let s = s.trim();
    let letter = s.chars().next()?.to_ascii_uppercase();
    if letter != 'G' && letter != 'H' {
        return None;
    }
    let digits = &s[1..];
    if digits.is_empty() || digits.len() > 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}{:0>4}", letter, digits))
}

enum Range {
    Intro,
    Verses(u32, u32),
    Whole,
}

fn parse_range(r: &str) -> Range {
    let r = r.trim();
    if r.eq_ignore_ascii_case("intro") || r.eq_ignore_ascii_case("introduction") {
        return Range::Intro;
    }
    let parts: Vec<&str> = r.split(|c| c == '-' || c == '\u{2013}').collect();
    match parts.as_slice() {
        [a] => a.trim().parse().map(|n| Range::Verses(n, n)).unwrap_or(Range::Whole),
        [a, b] => match (a.trim().parse(), b.trim().parse()) {
            (Ok(a), Ok(b)) => Range::Verses(a, b),
            _ => Range::Whole,
        },
        _ => Range::Whole,
    }
}

fn span_label(a: u32, b: u32) -> String {
    if a == b {
        format!("Verse {}", a)
    } else {
        format!("Verses {}\u{2013}{}", a, b)
    }
}

/// Commentaries written as one entry per verse range (Clarke, Gill, Jamieson-Fausset-Brown...).
fn notes_from_entries(j: &Json, chapter: u32, verse: Option<u32>) -> Vec<Note> {
    let mut rows: Vec<(u32, Note)> = Vec::new();
    for e in j.list("data") {
        if e.get("chapter").and_then(|c| c.as_u32()) != Some(chapter) {
            continue;
        }
        let text = e.text("commentary_text").trim();
        if text.is_empty() {
            continue;
        }
        let (label, start, shown) = match parse_range(e.text("verse_range")) {
            Range::Intro => ("Introduction".to_string(), 0, verse.is_none()),
            Range::Whole => ("Whole chapter".to_string(), 0, verse.is_none()),
            Range::Verses(a, b) => (span_label(a, b), a, verse.map_or(true, |v| a <= v && v <= b)),
        };
        if shown {
            rows.push((start, Note { label, text: text.replace("\r\n", "\n") }));
        }
    }
    rows.sort_by_key(|r| r.0);
    rows.into_iter().map(|r| r.1).collect()
}

fn superscript_digit(c: char) -> Option<u32> {
    match c {
        '\u{2070}' => Some(0),
        '\u{b9}' => Some(1),
        '\u{b2}' => Some(2),
        '\u{b3}' => Some(3),
        '\u{2074}'..='\u{2079}' => Some(c as u32 - 0x2070),
        '0'..='9' => c.to_digit(10),
        _ => None,
    }
}

struct MdSection {
    title: String,
    verses: Vec<u32>,
    text: String,
}

/// Matthew Henry's chapters are Markdown: a summary, then "## Heading", a quoted run of verses
/// (each number in bold superscript), and the commentary under it.
fn md_sections(md: &str) -> Vec<MdSection> {
    let mut body = md;
    if let Some(rest) = md.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            body = &rest[end + 4..];
        }
    }
    let mut out: Vec<MdSection> = Vec::new();
    let mut cur = MdSection { title: String::new(), verses: Vec::new(), text: String::new() };
    for line in body.lines() {
        let t = line.trim_end();
        if let Some(h) = t.strip_prefix("## ") {
            out.push(std::mem::replace(&mut cur, MdSection { title: h.trim().to_string(), verses: Vec::new(), text: String::new() }));
            continue;
        }
        if t.starts_with("# ") {
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            let mut rest = q;
            while let Some(i) = rest.find("**") {
                let after = &rest[i + 2..];
                let Some(j) = after.find("**") else { break };
                let digits: Option<Vec<u32>> = after[..j].chars().map(superscript_digit).collect();
                if let Some(d) = digits.filter(|d| !d.is_empty() && d.len() <= 3) {
                    cur.verses.push(d.iter().fold(0, |a, x| a * 10 + x));
                }
                rest = &after[j + 2..];
            }
            continue;
        }
        cur.text.push_str(t);
        cur.text.push('\n');
    }
    out.push(cur);
    out.into_iter()
        .map(|mut s| {
            // some commentaries carry the verses in the heading ("Verses 1\u{2013}8") instead of quoting them
            if s.verses.is_empty() {
                if let Some((a, b)) = heading_verses(&s.title) {
                    s.verses = (a..=b.min(a + 200)).collect();
                }
            }
            s.text = s.text.replace("**", "").replace('*', "");
            let mut t = String::new();
            let mut blank = 0;
            for l in s.text.lines() {
                if l.trim().is_empty() {
                    blank += 1;
                    if blank > 1 {
                        continue;
                    }
                } else {
                    blank = 0;
                }
                t.push_str(l.trim_end());
                t.push('\n');
            }
            s.text = t.trim().to_string();
            s
        })
        .filter(|s| !s.text.is_empty())
        .collect()
}

/// "Verses 1\u{2013}8" or "Verse 5" at the start of a heading.
fn heading_verses(title: &str) -> Option<(u32, u32)> {
    let t = title.trim();
    let rest = t.strip_prefix("Verses ").or_else(|| t.strip_prefix("Verse "))?;
    match parse_range(rest.trim()) {
        Range::Verses(a, b) if a >= 1 && b >= a => Some((a, b)),
        _ => None,
    }
}

fn chapter_of_name(name: &str) -> Option<u32> {
    let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn notes_from_chapters(j: &Json, chapter: u32, verse: Option<u32>) -> Vec<Note> {
    let mut out = Vec::new();
    // Most packs list `chapters` named by `source_name`; Song of Solomon lists `pages` named by `path`
    // (with an `_index.md` front page that has no chapter number and is skipped).
    for c in j.list("chapters").iter().chain(j.list("pages")) {
        let name = if c.get("source_name").is_some() { c.text("source_name") } else { c.text("path") };
        if chapter_of_name(name) != Some(chapter) {
            continue;
        }
        for s in md_sections(c.text("markdown")) {
            let label = if s.title.is_empty() { "Summary of the chapter".to_string() } else { s.title.clone() };
            let (lo, hi) = (s.verses.iter().min().copied(), s.verses.iter().max().copied());
            if let Some(v) = verse {
                if !s.verses.contains(&v) {
                    continue;
                }
            }
            let label = match (lo, hi) {
                // a heading that already says "Verses 1\u{2013}8" needs nothing added
                (Some(a), Some(b)) if !label.starts_with("Verse") => format!("{} ({})", label, span_label(a, b).to_lowercase()),
                _ => label,
            };
            out.push(Note { label, text: s.text });
        }
    }
    out
}

fn entries_of(j: &Json) -> &[Json] {
    if let Some(a) = j.as_arr() {
        return a;
    }
    for k in ["data", "entries"] {
        if let Some(a) = j.get(k).and_then(|x| x.as_arr()) {
            return a;
        }
    }
    &[]
}

fn entry_title(e: &Json) -> &str {
    for k in ["term", "title", "name"] {
        let t = e.text(k);
        if !t.is_empty() {
            return t;
        }
    }
    ""
}

fn entry_body(e: &Json) -> String {
    let join = |v: &[Json]| v.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n\n");
    let blocks = join(e.list("definition_blocks"));
    if !blocks.is_empty() {
        return blocks;
    }
    for k in ["definition", "body", "text", "commentary_text"] {
        let t = e.text(k);
        if !t.is_empty() {
            return t.to_string();
        }
    }
    // Nave's: a list of lines, each with its own words
    let lines: Vec<&str> = e.list("lines").iter().map(|l| l.text("text")).filter(|t| !t.is_empty()).collect();
    lines.join("\n")
}

fn entry_refs(e: &Json) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |s: &str| {
        if !s.is_empty() && !out.iter().any(|x| x == s) && out.len() < 60 {
            out.push(s.to_string());
        }
    };
    for r in e.list("scripture_references") {
        if let Some(s) = r.as_str() {
            add(s);
        }
    }
    for r in e.list("references") {
        match r {
            Json::Str(s) => add(s),
            o => add(o.text("display")),
        }
    }
    for l in e.list("lines") {
        for r in l.list("references") {
            add(r.text("display"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);

    /// A fresh empty folder for one test.
    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-bible-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(dir: &Path, rel: &str, text: &str) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    fn manifest(id: &str, kind: &str, files: &[(&str, &str, &str)]) -> String {
        let f: Vec<String> = files
            .iter()
            .map(|(p, r, b)| format!(r#"{{"path":"{}","role":"{}","book":{}}}"#, p, r, if b.is_empty() { "null".to_string() } else { format!("\"{}\"", b) }))
            .collect();
        format!(r#"{{"schema_version":2,"module_id":"{}","title":"Test {}","kind":"{}","license":"Public Domain","files":[{}]}}"#, id, id, kind, f.join(","))
    }

    const JOHN: &str = r#"{"module_id":"bible-test","book":"JHN","chapters":[{"chapter":3,"verses":[
        {"reference":"JHN.3.16","number":"16","text":"For God so loved the world, that he gave his only begotten Son."},
        {"reference":"JHN.3.17","number":"17","text":"\u00b6 For God sent not his Son into the world to condemn the world."},
        {"reference":"JHN.3.18","number":"18","text":"He that believeth on him is not condemned."}]}]}"#;

    /// A drive's `library/bible` folder holding one of each kind of resource.
    fn sample() -> PathBuf {
        let root = tmp();
        let b = root.join("Canonleaf Library/library/resources");
        put(&b.join("bible-test"), "manifest.json", &manifest("bible-test", "bible", &[("books/JHN.json", "book", "JHN")]));
        put(&b.join("bible-test"), "books/JHN.json", JOHN);
        // a commentary written per verse range
        put(&b.join("clarke"), "manifest.json", &manifest("clarke", "commentary", &[("notes/john.json", "notes", "")]));
        put(
            &b.join("clarke"),
            "notes/john.json",
            r#"{"data":[
              {"chapter":3,"verse_range":"16","commentary_text":"God loved the world."},
              {"chapter":3,"verse_range":"intro","commentary_text":"The talk with Nicodemus."},
              {"chapter":3,"verse_range":"14-17","commentary_text":"The serpent and the Son."},
              {"chapter":2,"verse_range":"1","commentary_text":"Another chapter."}]}"#,
        );
        // a commentary written as Markdown chapters
        put(&b.join("henry"), "manifest.json", &manifest("henry", "commentary", &[("notes/1-john.json", "notes", ""), ("notes/john.json", "notes", "")]));
        put(
            &b.join("henry"),
            "notes/john.json",
            "{\"book\":\"john\",\"chapters\":[{\"source_name\":\"chapter-3.md\",\"markdown\":\"---\\ntitle: \\\"John 3\\\"\\n---\\n\\n# John 3\\n\\nNicodemus comes by night.\\n\\n## The New Birth\\n\\n> **\\u00b9\\u2076** For God so loved *the world*.  \\n> **\\u00b9\\u2077** For God sent not.\\n\\nHere is **the gospel** in short.\\n\\n## Belief\\n\\n> **\\u00b9\\u2078** He that believeth.\\n\\nBelieve and live.\"}]}",
        );
        put(&b.join("henry"), "notes/1-john.json", r#"{"book":"1-john","chapters":[]}"#);
        // cross-references, named the way that pack names them
        put(&b.join("tsk-test"), "manifest.json", &manifest("crossrefs-tsk-test", "concordance", &[("crossrefs/John.json", "notes", ""), ("crossrefs/1%20John.json", "notes", "")]));
        put(
            &b.join("tsk-test"),
            "crossrefs/John.json",
            r#"[{"book":"John","chapter":3,"verse":16,"anchor":"loved","references":["Rom 5:8","1 John 4:9-10"]},
                {"book":"John","chapter":3,"verse":17,"anchor":"world","references":["Luke 19:10"]}]"#,
        );
        put(&b.join("tsk-test"), "crossrefs/1%20John.json", "[]");
        // dictionary split by letter, and a Nave-style topical index
        put(&b.join("dict"), "manifest.json", &manifest("dict", "dictionary", &[("articles/a.json", "articles", ""), ("articles/g.json", "articles", "")]));
        put(&b.join("dict"), "articles/a.json", r#"{"data":[{"term":"Grace","definition_blocks":["WRONG FILE"]}]}"#);
        put(
            &b.join("dict"),
            "articles/g.json",
            r#"{"data":[{"term":"Grace","alt_terms":["Gracia"],"definition_blocks":["Unmerited favour of God.","Second block."],"scripture_references":["Eph 2:8","Titus 2:11"]},
                        {"term":"Graceful","definition_blocks":["Other."]}]}"#,
        );
        put(&b.join("nave"), "manifest.json", &manifest("nave", "topical", &[("articles/f.json", "articles", "")]));
        put(
            &b.join("nave"),
            "articles/f.json",
            r#"{"data":[{"term":"FORGIVENESS","lines":[{"level":1,"text":"-Of sins","references":[{"display":"Mt 6:14"},{"display":"Ps 32:1"}]}]}]}"#,
        );
        // lexicon
        put(&b.join("greek"), "manifest.json", &manifest("step-greek-structured", "dictionary", &[("lexicon/entries.json", "lexicon", "")]));
        put(
            &b.join("greek"),
            "lexicon/entries.json",
            r#"{"language":"greek","entries":[{"extended_strong":"G5485","lemma":"χάρις","transliteration":"charis","morphology":"G:N-F","gloss":"grace","source_line":1},
              {"extended_strong":"G54850","lemma":"x"},{"extended_strong":"G5485A","lemma":"χάρις","gloss":"favour"}]}"#,
        );
        root
    }

    // ----- books and references -----

    #[test]
    fn the_book_table_is_complete_and_unambiguous() {
        assert_eq!(BOOKS.iter().map(|b| b.chapters).sum::<u32>(), 1189);
        let mut seen: HashMap<String, &str> = HashMap::new();
        for b in BOOKS.iter() {
            let mut keys = vec![key(b.name), key(b.code)];
            keys.extend(b.aliases.iter().map(|a| key(a)));
            for k in keys {
                if let Some(other) = seen.insert(k.clone(), b.code) {
                    assert_eq!(other, b.code, "{} means both {} and {}", k, other, b.code);
                }
            }
        }
    }

    #[test]
    fn references_are_read_in_the_common_forms() {
        let r = parse_reference("John 3:16").unwrap();
        assert_eq!((r.book, r.chapter, r.verse), ("JHN", 3, Some(16)));
        assert_eq!(r.code(), "JHN.3.16");
        assert_eq!(r.label(), "John 3:16");
        assert_eq!(parse_reference("1 Cor 13").unwrap().code(), "1CO.13");
        assert_eq!(parse_reference("I Corinthians 13:4").unwrap().book, "1CO");
        assert_eq!(parse_reference("First John 1:9").unwrap().book, "1JN");
        assert_eq!(parse_reference("Song of Solomon 2:1").unwrap().book, "SNG");
        assert_eq!(parse_reference("Ps 23:1-3").unwrap().label(), "Psalms 23:1\u{2013}3");
        assert_eq!(parse_reference("Rev 1:2,8,11").unwrap().verse, Some(2));
        assert_eq!(parse_reference("1 Chr 29:11").unwrap().book, "1CH");
        assert_eq!(parse_reference("Phlm 1:4").unwrap().book, "PHM");
        assert_eq!(parse_reference("Matt 5:3").unwrap().book, "MAT");
    }

    #[test]
    fn things_that_are_not_references_are_refused() {
        for s in ["John", "John 99", "John 3:0", "Genesis 51", "Hello 3:16", "3:16", "John 3:x", "Obad 2", "", "Job"] {
            assert!(parse_reference(s).is_none(), "{s}");
        }
    }

    #[test]
    fn osis_references_are_read() {
        assert_eq!(parse_osis("JHN.3.16").unwrap().label(), "John 3:16");
        assert_eq!(parse_osis("GEN.1").unwrap().verse, None);
        assert!(parse_osis("JHN.99.1").is_none());
        assert!(parse_osis("XYZ.1.1").is_none());
        assert!(parse_osis("JHN.3.16.1").is_none());
    }

    #[test]
    fn references_are_found_in_running_text() {
        let r = find_references("What does John 3:16 mean, and how does it fit with Rom 5:8?");
        assert_eq!(r.iter().map(|x| x.code()).collect::<Vec<_>>(), vec!["JHN.3.16", "ROM.5.8"]);
        let r = find_references("compare Ps 23 and 1 John 1:9 please");
        assert_eq!(r.iter().map(|x| x.code()).collect::<Vec<_>>(), vec!["PSA.23", "1JN.1.9"]);
        assert_eq!(find_references("see (Song of Solomon 2:1).").len(), 1);
    }

    #[test]
    fn everyday_words_are_not_taken_for_books() {
        assert!(find_references("I have a job 5 days a week").is_empty());
        assert!(find_references("mark 3 on the map and call James 2 times").is_empty());
        assert!(find_references("is 5 enough? am 3 enough? ex 4").is_empty());
        // with a verse they are references
        assert_eq!(find_references("read Job 3:2").len(), 1);
        assert_eq!(find_references("read Mark 4:39").len(), 1);
        // a plain chapter of an unmistakable book is fine
        assert_eq!(find_references("summarise Genesis 1").len(), 1);
    }

    // ----- finding resources -----

    #[test]
    fn resources_are_found_below_the_folder_whatever_the_depth() {
        let root = sample();
        let lib = Library::open(&[root.clone()]);
        let ids: Vec<&str> = lib.modules.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(lib.modules.len(), 7, "{ids:?}");
        assert!(ids.contains(&"bible-test") && ids.contains(&"clarke") && ids.contains(&"crossrefs-tsk-test"));
        // Bibles first, then commentaries
        assert_eq!(lib.modules[0].kind, "bible");
        assert_eq!(lib.default_bible().unwrap().id, "bible-test");
    }

    #[test]
    fn a_missing_folder_means_no_resources() {
        let lib = Library::open(&[PathBuf::from("/no/such/folder/anywhere")]);
        assert!(lib.modules.is_empty());
        assert!(lib.default_bible().is_none());
        assert!(lib.passages("what does John 3:16 say", 3).is_empty());
    }

    #[test]
    fn odd_manifests_and_folders_are_skipped() {
        let root = tmp();
        // old schema, unknown kind, unsafe id, a cache folder and a hidden folder
        put(&root, "old/manifest.json", r#"{"schema_version":1,"module_id":"old","kind":"bible","files":[]}"#);
        put(&root, "weird/manifest.json", r#"{"schema_version":2,"module_id":"weird","kind":"tract","files":[]}"#);
        put(&root, "unsafe/manifest.json", r#"{"schema_version":2,"module_id":"../escape","kind":"bible","files":[]}"#);
        put(&root, "cache/x/manifest.json", &manifest("incache", "bible", &[]));
        put(&root, ".stage-1/manifest.json", &manifest("staged", "bible", &[]));
        put(&root, "broken/manifest.json", "{not json");
        put(&root, "good/manifest.json", &manifest("good", "bible", &[]));
        let lib = Library::open(&[root]);
        assert_eq!(lib.modules.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["good"]);
    }

    #[test]
    fn the_first_copy_of_a_resource_wins() {
        let (a, b) = (tmp(), tmp());
        put(&a, "x/manifest.json", &manifest("same", "bible", &[]));
        put(&b, "y/manifest.json", &manifest("same", "bible", &[]));
        let lib = Library::open(&[a.clone(), b]);
        assert_eq!(lib.modules.len(), 1);
        assert!(lib.modules[0].dir.starts_with(&a));
    }

    #[test]
    fn a_manifest_cannot_point_outside_its_folder() {
        let root = tmp();
        put(&root, "secret.json", JOHN);
        let dir = root.join("lib/bible-evil");
        put(&dir, "manifest.json", &manifest("bible-evil", "bible", &[("../../secret.json", "book", "JHN")]));
        let lib = Library::open(&[root.join("lib")]);
        assert_eq!(lib.modules.len(), 1);
        let e = lib.chapter("bible-evil", "JHN", 3).unwrap_err();
        assert!(e.contains("missing"), "{e}");
        for bad in ["/etc/passwd", "a/../b", "C:/x", "a\\b", "", "./x", "a//b"] {
            assert!(lib.modules[0].member(bad).is_none(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_inside_a_resource_is_not_followed() {
        let root = tmp();
        put(&root, "outside.json", JOHN);
        let dir = root.join("lib/bible-link");
        put(&dir, "manifest.json", &manifest("bible-link", "bible", &[("books/JHN.json", "book", "JHN")]));
        fs::create_dir_all(dir.join("books")).unwrap();
        std::os::unix::fs::symlink(root.join("outside.json"), dir.join("books/JHN.json")).unwrap();
        let lib = Library::open(&[root.join("lib")]);
        assert!(lib.chapter("bible-link", "JHN", 3).is_err());
    }

    // ----- reading -----

    #[test]
    fn a_chapter_is_read_as_the_resource_has_it() {
        let lib = Library::open(&[sample()]);
        let v = lib.chapter("bible-test", "JHN", 3).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], Verse { number: "16".into(), text: "For God so loved the world, that he gave his only begotten Son.".into() });
        // the paragraph mark is dropped, the words are not changed
        assert_eq!(v[1].text, "For God sent not his Son into the world to condemn the world.");
        assert_eq!(lib.books_in(lib.module("bible-test").unwrap()).iter().map(|b| b.code).collect::<Vec<_>>(), vec!["JHN"]);
    }

    #[test]
    fn missing_passages_are_reported_not_invented() {
        let lib = Library::open(&[sample()]);
        assert!(lib.chapter("bible-test", "JHN", 9).unwrap_err().contains("does not have John 9"));
        assert!(lib.chapter("bible-test", "GEN", 1).unwrap_err().contains("does not have Genesis"));
        assert!(lib.chapter("bible-test", "XXX", 1).is_err());
        assert!(lib.chapter("clarke", "JHN", 3).is_err(), "a commentary is not a Bible");
        assert!(lib.chapter("nothing", "JHN", 3).is_err());
    }

    #[test]
    fn search_finds_words_and_puts_phrases_first() {
        let lib = Library::open(&[sample()]);
        let h = lib.search("bible-test", "world condemn", 10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!((h[0].book, h[0].chapter, h[0].verse.as_str()), ("JHN", 3, "17"));
        // the verse with the whole phrase comes before the one that only has both words
        let h = lib.search("bible-test", "his Son", 10).unwrap();
        assert_eq!(h.iter().map(|x| x.verse.as_str()).collect::<Vec<_>>(), vec!["17", "16"]);
        // a start of a word matches from four letters
        assert_eq!(lib.search("bible-test", "begot", 10).unwrap().len(), 1);
        assert!(lib.search("bible-test", "xyzzy", 10).unwrap().is_empty());
        assert!(lib.search("bible-test", "   ", 10).unwrap().is_empty());
        assert_eq!(lib.search("bible-test", "god", 1).unwrap().len(), 1);
    }

    // ----- commentary -----

    #[test]
    fn verse_range_commentary_is_picked_by_verse() {
        let lib = Library::open(&[sample()]);
        let n = lib.notes("clarke", "JHN", 3, Some(16)).unwrap();
        assert_eq!(n.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), vec!["Verses 14\u{2013}17", "Verse 16"]);
        assert_eq!(n[1].text, "God loved the world.");
        // verse 18 has none
        assert!(lib.notes("clarke", "JHN", 3, Some(18)).unwrap().is_empty());
        // the whole chapter: introduction first, then by verse, and not another chapter's
        let all = lib.notes("clarke", "JHN", 3, None).unwrap();
        assert_eq!(all.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), vec!["Introduction", "Verses 14\u{2013}17", "Verse 16"]);
    }

    #[test]
    fn markdown_commentary_is_split_by_heading_and_verse() {
        let lib = Library::open(&[sample()]);
        let n = lib.notes("henry", "JHN", 3, None).unwrap();
        assert_eq!(n.len(), 3);
        assert_eq!(n[0].label, "Summary of the chapter");
        assert_eq!(n[0].text, "Nicodemus comes by night.");
        assert_eq!(n[1].label, "The New Birth (verses 16\u{2013}17)");
        assert_eq!(n[1].text, "Here is the gospel in short.");
        let v = lib.notes("henry", "JHN", 3, Some(18)).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].text, "Believe and live.");
        assert!(lib.notes("henry", "JHN", 3, Some(99)).unwrap().is_empty());
    }

    #[test]
    fn commentary_with_the_verses_in_the_heading_is_picked_by_verse() {
        let root = tmp();
        put(&root, "concise/manifest.json", &manifest("concise", "commentary", &[("notes/john.json", "notes", "")]));
        put(
            &root,
            "concise/notes/john.json",
            "{\"book\":\"john\",\"chapters\":[{\"source_name\":\"chapter-3.md\",\"markdown\":\"---\\ntitle: x\\n---\\n\\n# John 3\\n\\n## Chapter Outline\\n\\n- Nicodemus. (1\\u201321)\\n\\n## Verses 1\\u20138\\n\\nThe new birth.\\n\\n## Verses 9\\u201321\\n\\nGod so loved.\"}]}",
        );
        let lib = Library::open(&[root]);
        let all = lib.notes("concise", "JHN", 3, None).unwrap();
        assert_eq!(all.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(), vec!["Chapter Outline", "Verses 1\u{2013}8", "Verses 9\u{2013}21"]);
        let v = lib.notes("concise", "JHN", 3, Some(16)).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].text, "God so loved.");
    }

    #[test]
    fn commentary_listed_as_pages_is_read_like_chapters() {
        let root = tmp();
        put(&root, "mh-song/manifest.json", &manifest("mh-song", "commentary", &[("notes/song.json", "notes", "SNG")]));
        put(
            &root,
            "mh-song/notes/song.json",
            "{\"book\":\"song\",\"pages\":[{\"path\":\"_index.md\",\"markdown\":\"---\\ntitle: x\\n---\\n\\nFront page 2.\"},{\"path\":\"chapter-2.md\",\"markdown\":\"---\\ntitle: x\\n---\\n\\n# Song 2\\n\\n## The Church\\n\\n> **\\u00b9** I am the rose of Sharon.  \\n> **\\u00b2** As the lily among thorns.\\n\\nChrist speaks.\"}]}",
        );
        let lib = Library::open(&[root]);
        let all = lib.notes("mh-song", "SNG", 2, None).unwrap();
        assert_eq!(all.len(), 1, "the front page is not a chapter");
        assert_eq!(all[0].text, "Christ speaks.");
        assert_eq!(lib.notes("mh-song", "SNG", 2, Some(2)).unwrap().len(), 1);
        assert!(lib.notes("mh-song", "SNG", 3, None).unwrap().is_empty());
    }

    #[test]
    fn a_commentary_that_lacks_the_book_gives_nothing() {
        let lib = Library::open(&[sample()]);
        assert!(lib.notes("clarke", "GEN", 1, None).unwrap().is_empty());
        assert!(lib.notes("henry", "1JN", 1, None).unwrap().is_empty());
        assert!(lib.notes("bible-test", "JHN", 3, None).is_err());
    }

    #[test]
    fn another_name_in_a_different_letters_file_is_found() {
        let root = tmp();
        put(&root, "dict/manifest.json", &manifest("dict", "dictionary", &[("articles/f.json", "articles", ""), ("articles/g.json", "articles", "")]));
        put(&root, "dict/articles/f.json", r#"{"data":[{"term":"Faith","definition_blocks":["Trust."]}]}"#);
        put(&root, "dict/articles/g.json", r#"{"data":[{"term":"Grace","alt_terms":["Favour"],"definition_blocks":["Unmerited favour."]}]}"#);
        let lib = Library::open(&[root]);
        let e = lib.define("favour", 5);
        assert_eq!(e.len(), 1, "{e:?}");
        assert_eq!(e[0].title, "Grace");
        // the word is written in g.json, but the entry in f.json is not dragged in by it
        assert!(lib.define("faith", 5).iter().all(|x| x.title == "Faith"));
        assert!(lib.define("nothingatall", 5).is_empty());
    }

    #[test]
    fn canonleafs_own_folder_can_be_the_root() {
        // Canonleaf keeps installed packs in <its folder>/library/resources/<id> and its bundled Bibles
        // beside them; choosing that top folder must find them all where they are.
        let root = tmp();
        put(&root, "Canonleaf Library/library/resources/bible-xyz/manifest.json", &manifest("bible-xyz", "bible", &[("books/JHN.json", "book", "JHN")]));
        put(&root, "Canonleaf Library/library/resources/bible-xyz/books/JHN.json", JOHN);
        put(&root, "Canonleaf Library/bundled/bsb/manifest.json", &manifest("bible-bsb", "bible", &[("books/JHN.json", "book", "JHN")]));
        put(&root, "Canonleaf Library/bundled/bsb/books/JHN.json", JOHN);
        let lib = Library::open(&[root.clone()]);
        let mut ids: Vec<&str> = lib.modules.iter().map(|m| m.id.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["bible-bsb", "bible-xyz"]);
        assert_eq!(lib.default_bible().unwrap().id, "bible-bsb");
        assert_eq!(lib.chapter("bible-xyz", "JHN", 3).unwrap().len(), 3);
    }

    // ----- cross-references, dictionaries, lexicon -----

    #[test]
    fn cross_references_are_listed_for_the_verse() {
        let lib = Library::open(&[sample()]);
        let m = lib.crossref_module_for("bible-test").unwrap();
        assert_eq!(m.id, "crossrefs-tsk-test");
        let x = lib.crossrefs(&m.id, "JHN", 3, 16).unwrap();
        assert_eq!(x, vec![XRef { anchor: "loved".into(), refs: vec!["Rom 5:8".into(), "1 John 4:9-10".into()] }]);
        assert!(lib.crossrefs(&m.id, "JHN", 3, 1).unwrap().is_empty());
        // the listed places can be opened
        assert_eq!(parse_reference(&x[0].refs[1]).unwrap().book, "1JN");
        assert!(lib.crossrefs(&m.id, "GEN", 1, 1).unwrap().is_empty());
    }

    #[test]
    fn a_cross_reference_list_of_the_same_edition_is_preferred() {
        let root = tmp();
        for ed in ["asv", "kjv"] {
            let id = format!("crossrefs-tsk-{}", ed);
            put(&root, &format!("{}/manifest.json", ed), &manifest(&id, "concordance", &[("crossrefs/John.json", "notes", "")]));
            put(&root, &format!("{}/crossrefs/John.json", ed), "[]");
        }
        let lib = Library::open(&[root]);
        assert_eq!(lib.crossref_module_for("bible-kjv").unwrap().id, "crossrefs-tsk-kjv");
        assert_eq!(lib.crossref_module_for("bible-bsb").unwrap().id, "crossrefs-tsk-asv", "falls back to the first");
    }

    #[test]
    fn dictionary_headwords_are_looked_up_by_letter() {
        let lib = Library::open(&[sample()]);
        let e = lib.define("grace", 5);
        assert_eq!(e[0].title, "Grace");
        assert_eq!(e[0].body, "Unmerited favour of God.\n\nSecond block.");
        assert_eq!(e[0].refs, vec!["Eph 2:8", "Titus 2:11"]);
        assert_eq!(e[0].source, "Test dict");
        // the file for "a" was never opened, and a longer headword comes after the exact one
        assert!(e.iter().all(|x| x.body != "WRONG FILE"));
        assert_eq!(e[1].title, "Graceful");
        // an alternative name finds it too (in the file for its own first letter), and a plural
        assert_eq!(lib.define("Gracia", 5)[0].title, "Grace");
        assert_eq!(lib.define("graces", 5)[0].title, "Grace");
        assert!(lib.define("zebra", 5).is_empty());
        assert!(lib.define("", 5).is_empty());
    }

    #[test]
    fn topical_entries_give_their_lines_and_passages() {
        let lib = Library::open(&[sample()]);
        let e = lib.define("forgiveness", 5);
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].body, "-Of sins");
        assert_eq!(e[0].refs, vec!["Mt 6:14", "Ps 32:1"]);
    }

    #[test]
    fn strongs_numbers_are_looked_up() {
        let lib = Library::open(&[sample()]);
        let s = lib.strongs("g5485");
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!((s[0].number.as_str(), s[0].transliteration.as_str(), s[0].gloss.as_str()), ("G5485", "charis", "grace"));
        assert_eq!(s[1].number, "G5485A");
        assert_eq!(lib.strongs("G548").len(), 0, "a number is four digits: G0548 is another word");
        assert!(lib.strongs("X12").is_empty());
        assert!(lib.strongs("G").is_empty());
        assert!(lib.strongs("G12345").is_empty());
    }

    // ----- for the assistant -----

    #[test]
    fn a_written_reference_brings_its_verses_and_a_note() {
        let lib = Library::open(&[sample()]);
        let p = lib.passages("What does John 3:16 mean?", 4);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].source.as_str(), p[0].heading.as_str(), p[0].link.as_str()), ("Test bible-test", "John 3:16", "bible:JHN.3.16"));
        assert_eq!(p[0].text, "16 For God so loved the world, that he gave his only begotten Son.");
        assert!(p[0].score > p[1].score);
        assert_eq!(p[1].source, "Test clarke");
        assert!(p[1].heading.starts_with("John 3:16 \u{2013} "));
        // a range, and a chapter alone
        assert_eq!(lib.passages("explain John 3:16-17", 4)[0].text.matches("For God").count(), 2);
        assert!(lib.passages("summarise John 3", 4)[0].text.contains("18 He that believeth"));
        // a reference in a chapter that is not there brings nothing
        assert!(lib.passages("read John 9:1", 4).is_empty());
    }

    #[test]
    fn bible_words_only_join_questions_that_sound_biblical() {
        let lib = Library::open(&[sample()]);
        // not a Bible question: nothing, even though "grace" is in the dictionary
        assert!(lib.passages("how much grace period on a loan", 4).is_empty());
        assert!(lib.passages("how do I purify water", 4).is_empty());
        // a Bible question
        let p = lib.passages("What does the Bible say about grace?", 4);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].heading, "Grace");
        assert_eq!(p[0].link, "bible:term:Grace");
        assert!(p[0].text.contains("Passages: Eph 2:8; Titus 2:11"));
        let p = lib.passages("what does scripture teach about forgiveness", 4);
        assert_eq!(p[0].heading, "FORGIVENESS");
    }

    #[test]
    fn the_topical_index_comes_before_the_dictionaries_and_filler_words_are_not_looked_up() {
        let root = sample();
        // a dictionary entry for the word "Bible" itself, and a dictionary entry for forgiveness
        put(&root, "Canonleaf Library/library/resources/dict/articles/b.json", r#"{"data":[{"term":"Bible","definition_blocks":["The book."]}]}"#);
        let manifest_path = root.join("Canonleaf Library/library/resources/dict/manifest.json");
        fs::write(&manifest_path, manifest("dict", "dictionary", &[("articles/a.json", "articles", ""), ("articles/b.json", "articles", ""), ("articles/f.json", "articles", ""), ("articles/g.json", "articles", "")])).unwrap();
        put(&root, "Canonleaf Library/library/resources/dict/articles/f.json", r#"{"data":[{"term":"Forgiveness","definition_blocks":["Pardon."]}]}"#);
        let lib = Library::open(&[root]);
        let p = lib.passages("What does the Bible say about forgiveness?", 4);
        assert_eq!(p.iter().map(|x| (x.source.as_str(), x.heading.as_str())).collect::<Vec<_>>(), vec![("Test nave", "FORGIVENESS"), ("Test dict", "Forgiveness")]);
    }

    #[test]
    fn the_passage_limit_is_kept() {
        let lib = Library::open(&[sample()]);
        assert_eq!(lib.passages("What does John 3:16 mean?", 1).len(), 1);
        assert!(lib.passages("What does John 3:16 mean?", 0).is_empty());
    }

    /// Run by hand against a real Canonleaf library: `CINDRIX_BIBLE_TEST=<folder> cargo test real_library -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_library_smoke() {
        let root = PathBuf::from(std::env::var("CINDRIX_BIBLE_TEST").expect("set CINDRIX_BIBLE_TEST to a folder of Canonleaf resources"));
        let lib = Library::open(&[root]);
        for m in &lib.modules {
            println!("{:<34} {:<11} {}", m.id, m.kind, m.title);
        }
        assert!(!lib.modules.is_empty());
        if let Some(b) = lib.default_bible() {
            let v = lib.chapter(&b.id, "JHN", 3).unwrap();
            println!("{} John 3: {} verses; v16: {}", b.title, v.len(), v.iter().find(|x| x.number == "16").map(|x| x.text.as_str()).unwrap_or(""));
            let t = std::time::Instant::now();
            let h = lib.search(&b.id, "love one another", 5).unwrap();
            println!("search 'love one another': {} hits in {:?}; first {:?}", h.len(), t.elapsed(), h.first());
            if let Some(x) = lib.crossref_module_for(&b.id) {
                println!("crossrefs {}: {:?}", x.id, lib.crossrefs(&x.id, "JHN", 3, 16).unwrap().iter().take(3).collect::<Vec<_>>());
            }
        }
        for m in lib.of_kind("commentary") {
            let n = lib.notes(&m.id, "JHN", 3, Some(16)).unwrap_or_default();
            println!("commentary {:<30} John 3:16 -> {} notes {}", m.id, n.len(), n.first().map(|x| x.label.as_str()).unwrap_or("-"));
        }
        for t in ["Aaron", "grace", "forgiveness", "Abana"] {
            let e = lib.define(t, 3);
            println!("define {:<12} -> {:?}", t, e.iter().map(|x| format!("{}: {} ({} chars, {} refs)", x.source, x.title, x.body.len(), x.refs.len())).collect::<Vec<_>>());
        }
        for m in lib.of_kind("commentary").into_iter().filter(|m| m.id.contains("song")) {
            let n = lib.notes(&m.id, "SNG", 2, Some(1)).unwrap_or_default();
            println!("commentary {} Song 2:1 -> {} notes {}", m.id, n.len(), n.first().map(|x| x.label.as_str()).unwrap_or("-"));
            assert!(!n.is_empty(), "the Song of Solomon pages were not read");
        }
        println!("strongs G5485 -> {:?}", lib.strongs("G5485").first());
        println!("strongs H0430 -> {:?}", lib.strongs("H0430").first());
        for q in ["What does John 3:16 mean?", "What does the Bible say about forgiveness?"] {
            for p in lib.passages(q, 4) {
                println!("ask {:?}: [{}] {} ({}) {} chars -> {}", q, p.source, p.heading, p.score, p.text.len(), p.link);
            }
        }
    }
}
