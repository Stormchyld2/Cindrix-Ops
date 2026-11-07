//! Canonleaf add-on: reads the Bible text that the owner's separate Canonleaf program keeps in its
//! own folder (`resources/bundled/<edition>/manifest.json` and `books/<CODE>.json`), so Ask can quote
//! and cite verses. Read-only: this never writes into Canonleaf's folder and never runs anything from it.
//! Standard library only. Book codes are the three-letter USFM codes Canonleaf uses (GEN, JHN, ...);
//! file names are always built from that fixed table, never from text a person typed.
//! See `docs/44-canonleaf-addon.md`.

use crate::retrieval::{self, Passage};
use std::fs;
use std::path::{Path, PathBuf};

/// (code, name, other ways people write it). Order is Bible order.
const BOOKS: &[(&str, &str, &[&str])] = &[
    ("GEN", "Genesis", &["gen"]), ("EXO", "Exodus", &["exod", "ex"]), ("LEV", "Leviticus", &["lev"]),
    ("NUM", "Numbers", &["num"]), ("DEU", "Deuteronomy", &["deut", "deu"]), ("JOS", "Joshua", &["josh"]),
    ("JDG", "Judges", &["judg"]), ("RUT", "Ruth", &[]), ("1SA", "1 Samuel", &["1 sam"]),
    ("2SA", "2 Samuel", &["2 sam"]), ("1KI", "1 Kings", &["1 kgs"]), ("2KI", "2 Kings", &["2 kgs"]),
    ("1CH", "1 Chronicles", &["1 chron"]), ("2CH", "2 Chronicles", &["2 chron"]), ("EZR", "Ezra", &[]),
    ("NEH", "Nehemiah", &["neh"]), ("EST", "Esther", &["esth"]), ("JOB", "Job", &[]),
    ("PSA", "Psalms", &["psalm", "ps", "psa"]), ("PRO", "Proverbs", &["prov"]), ("ECC", "Ecclesiastes", &["eccl"]),
    ("SNG", "Song of Solomon", &["song of songs", "song"]), ("ISA", "Isaiah", &[]), ("JER", "Jeremiah", &["jer"]),
    ("LAM", "Lamentations", &["lam"]), ("EZK", "Ezekiel", &["ezek"]), ("DAN", "Daniel", &["dan"]),
    ("HOS", "Hosea", &[]), ("JOL", "Joel", &[]), ("AMO", "Amos", &[]), ("OBA", "Obadiah", &["obad"]),
    ("JON", "Jonah", &[]), ("MIC", "Micah", &[]), ("NAM", "Nahum", &["nah"]), ("HAB", "Habakkuk", &["hab"]),
    ("ZEP", "Zephaniah", &["zeph"]), ("HAG", "Haggai", &[]), ("ZEC", "Zechariah", &["zech"]), ("MAL", "Malachi", &[]),
    ("MAT", "Matthew", &["matt"]), ("MRK", "Mark", &[]), ("LUK", "Luke", &[]), ("JHN", "John", &[]),
    ("ACT", "Acts", &[]), ("ROM", "Romans", &["rom"]), ("1CO", "1 Corinthians", &["1 cor"]),
    ("2CO", "2 Corinthians", &["2 cor"]), ("GAL", "Galatians", &["gal"]), ("EPH", "Ephesians", &["eph"]),
    ("PHP", "Philippians", &["phil"]), ("COL", "Colossians", &["col"]), ("1TH", "1 Thessalonians", &["1 thess"]),
    ("2TH", "2 Thessalonians", &["2 thess"]), ("1TI", "1 Timothy", &["1 tim"]), ("2TI", "2 Timothy", &["2 tim"]),
    ("TIT", "Titus", &[]), ("PHM", "Philemon", &["philem"]), ("HEB", "Hebrews", &["heb"]), ("JAS", "James", &["jam"]),
    ("1PE", "1 Peter", &["1 pet"]), ("2PE", "2 Peter", &["2 pet"]), ("1JN", "1 John", &[]), ("2JN", "2 John", &[]),
    ("3JN", "3 John", &[]), ("JUD", "Jude", &[]), ("REV", "Revelation", &["rev"]),
];

/// Every book as (code, name), in Bible order, for the reader's book list.
pub fn books() -> Vec<(&'static str, &'static str)> {
    BOOKS.iter().map(|b| (b.0, b.1)).collect()
}

pub fn book_name(code: &str) -> Option<&'static str> {
    BOOKS.iter().find(|b| b.0 == code).map(|b| b.1)
}

/// One installed Bible edition inside a Canonleaf folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Bible {
    pub id: String,
    pub title: String,
    pub dir: PathBuf,
    pub is_default: bool,
}

/// A verse reference: book code, chapter, first and last verse (0 and 0 mean the whole chapter).
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    pub book: &'static str,
    pub chapter: u32,
    pub from: u32,
    pub to: u32,
}

/// Editions found under `<canonleaf>/resources/bundled/`. Empty when the folder is not Canonleaf's
/// or has no Bible text; the caller says so in plain words.
pub fn find_bibles(canonleaf: &Path) -> Vec<Bible> {
    let root = canonleaf.join("resources").join("bundled");
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(&root) else { return out };
    for e in rd.flatten() {
        let dir = e.path();
        if !dir.join("books").is_dir() {
            continue;
        }
        let Ok(m) = fs::read_to_string(dir.join("manifest.json")) else { continue };
        if string_field(&m, "kind").as_deref().is_some_and(|k| k != "bible") {
            continue;
        }
        let id = e.file_name().to_string_lossy().to_string();
        let title = string_field(&m, "title").unwrap_or_else(|| id.to_uppercase());
        out.push(Bible { id, title, dir, is_default: m.replace(' ', "").contains("\"default\":true") });
    }
    out.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(a.id.cmp(&b.id)));
    out
}

/// The edition to quote from: the one Canonleaf marks as default, else the first found.
pub fn default_bible(canonleaf: &Path) -> Option<Bible> {
    find_bibles(canonleaf).into_iter().next()
}

/// All verses of one chapter, as (verse number, text). Empty when it is not there.
pub fn chapter(bible: &Bible, book: &str, chapter: u32) -> Vec<(String, String)> {
    let Some(text) = read_book(bible, book) else { return Vec::new() };
    verses_of(&text).into_iter().filter(|v| v.chapter == chapter).map(|v| (v.number, v.text)).collect()
}

/// How many chapters a book has in this edition (0 if the book is missing).
pub fn chapter_count(bible: &Bible, book: &str) -> u32 {
    read_book(bible, book).map(|t| verses_of(&t).iter().map(|v| v.chapter).max().unwrap_or(0)).unwrap_or(0)
}

/// Pick a reference such as "John 3:16", "Romans 8:28-30" or "Psalm 23" out of a question.
pub fn find_reference(question: &str) -> Option<Reference> {
    let q = question.to_lowercase().replace(['\u{2013}', '\u{2014}'], "-");
    let q = q.replace("first ", "1 ").replace("second ", "2 ").replace("third ", "3 ");
    let mut names: Vec<(String, &'static str)> = Vec::new();
    for (code, name, alts) in BOOKS {
        names.push((name.to_lowercase(), *code));
        for a in *alts {
            names.push((a.to_string(), *code));
        }
    }
    names.sort_by(|a, b| b.0.len().cmp(&a.0.len())); // "1 john" before "john"
    for (name, code) in &names {
        let mut from = 0;
        while let Some(i) = q[from..].find(name.as_str()) {
            let start = from + i;
            let end = start + name.len();
            from = end;
            let before_ok = start == 0 || !q[..start].chars().last().is_some_and(|c| c.is_alphanumeric());
            if !before_ok {
                continue;
            }
            let rest = q[end..].trim_start_matches(|c: char| c == '.' || c == ' ');
            if let Some((chapter, v1, v2)) = parse_numbers(rest) {
                let book = BOOKS.iter().find(|b| b.0 == *code).map(|b| b.0)?;
                return Some(Reference { book, chapter, from: v1, to: v2 });
            }
        }
    }
    None
}

fn parse_numbers(s: &str) -> Option<(u32, u32, u32)> {
    let (c, rest) = take_number(s)?;
    if let Some(r) = rest.strip_prefix(':') {
        if let Some((v1, r2)) = take_number(r) {
            if let Some(r3) = r2.strip_prefix('-') {
                if let Some((v2, _)) = take_number(r3) {
                    return Some((c, v1, v2.max(v1)));
                }
            }
            return Some((c, v1, v1));
        }
    }
    Some((c, 0, 0))
}

fn take_number(s: &str) -> Option<(u32, &str)> {
    let n = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if n == 0 || n > 3 {
        return None;
    }
    s[..n].parse().ok().map(|v| (v, &s[n..]))
}

/// Passages for Ask: the verses of a named reference, else the verses that best match the words
/// of the question. Always names the edition. Nothing is made up: no match gives an empty list.
pub fn search(canonleaf: &Path, question: &str, limit: usize) -> Vec<Passage> {
    let Some(bible) = default_bible(canonleaf) else { return Vec::new() };
    let source = format!("Canonleaf: {}", bible.title);
    if let Some(r) = find_reference(question) {
        let all = chapter(&bible, r.book, r.chapter);
        let picked: Vec<&(String, String)> = all
            .iter()
            .filter(|(n, _)| r.from == 0 || n.parse::<u32>().map(|n| n >= r.from && n <= r.to).unwrap_or(false))
            .take(30)
            .collect();
        if !picked.is_empty() {
            let name = book_name(r.book).unwrap_or(r.book);
            let heading = match (r.from, r.to) {
                (0, _) => format!("{} {}", name, r.chapter),
                (a, b) if a == b => format!("{} {}:{}", name, r.chapter, a),
                (a, b) => format!("{} {}:{}-{}", name, r.chapter, a, b),
            };
            let text = picked.iter().map(|(n, t)| format!("{} {}", n, t)).collect::<Vec<_>>().join(" ");
            let link = format!("canonleaf:{}.{}.{}", r.book, r.chapter, r.from.max(1));
            return vec![Passage { source, heading, text, score: 100.0, link }];
        }
    }
    // words people wrap round a Bible question that are not what to look for
    const FILLER: &[&str] = &["bible", "scripture", "verse", "say", "said", "about", "anything", "something", "tell", "mean", "pass", "passage"];
    let q: Vec<String> = retrieval::query_tokens(question).into_iter().filter(|w| !FILLER.contains(&w.as_str())).collect();
    if q.is_empty() {
        return Vec::new();
    }
    // pairs of neighbouring words in the question ("love your", "your enemies"): a verse that has the
    // pair in a row is a much better match than one that merely has both words somewhere
    let plain = |s: &str| -> Vec<String> {
        s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_string()).collect()
    };
    let asked = plain(question);
    let pairs: Vec<String> = asked.windows(2).map(|w| format!(" {} {} ", w[0], w[1])).collect();
    let mut scored: Vec<Passage> = Vec::new();
    for (code, name, _) in BOOKS {
        let Some(text) = read_book(&bible, code) else { continue };
        for v in verses_of(&text) {
            // a topic question needs at least two of its words in the verse, so one common word
            // ("person") cannot pull in an unrelated verse as a source
            let words = retrieval::tokens(&v.text);
            let hits = q.iter().filter(|w| words.contains(w)).count();
            if hits < q.len().min(2) {
                continue;
            }
            let flat = format!(" {} ", plain(&v.text).join(" "));
            let in_a_row = pairs.iter().filter(|p| flat.contains(p.as_str())).count();
            // more matching words first, pairs in a row count double, shorter verses win ties
            let s = hits as f64 + 2.0 * in_a_row as f64 + 1.0 / (1.0 + words.len() as f64);
            scored.push(Passage {
                source: source.clone(),
                heading: format!("{} {}:{}", name, v.chapter, v.number),
                text: v.text.clone(),
                score: s,
                link: format!("canonleaf:{}.{}.{}", code, v.chapter, v.number),
            });
        }
    }
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

// ---- reading Canonleaf's plain book files (no outside JSON crate) ----

struct Verse {
    chapter: u32,
    number: String,
    text: String,
}

fn read_book(bible: &Bible, code: &str) -> Option<String> {
    if !BOOKS.iter().any(|b| b.0 == code) {
        return None;
    }
    fs::read_to_string(bible.dir.join("books").join(format!("{}.json", code))).ok()
}

/// Walks the file in order: each `"chapter":N` starts a chapter; each verse has `"number"` then `"text"`.
/// Works with absolute offsets and `find`, so it never cuts inside a multi-byte character.
fn verses_of(json: &str) -> Vec<Verse> {
    const CH: &str = "\"chapter\":";
    const NU: &str = "\"number\":";
    const TX: &str = "\"text\":";
    // start of the value after a key, skipping spaces
    fn value_at(json: &str, from: usize) -> usize {
        from + (json[from..].len() - json[from..].trim_start().len())
    }
    let mut out = Vec::new();
    let mut chapter = 0u32;
    let mut pos = 0usize;
    while pos < json.len() {
        let c = json[pos..].find(CH).map(|i| pos + i);
        let n = json[pos..].find(NU).map(|i| pos + i);
        match (c, n) {
            (None, None) => break,
            (Some(ci), ni) if ni.map_or(true, |x| ci < x) => {
                let v = value_at(json, ci + CH.len());
                if let Some((num, _)) = take_number_loose(&json[v..]) {
                    chapter = num;
                }
                pos = ci + CH.len();
            }
            (_, Some(ni)) => {
                let v = value_at(json, ni + NU.len());
                pos = ni + NU.len();
                let Some((number, used)) = read_json_string(&json[v..]) else { continue };
                let after = v + used;
                let Some(t) = json[after..].find(TX) else { continue };
                let tv = value_at(json, after + t + TX.len());
                if let Some((text, used2)) = read_json_string(&json[tv..]) {
                    out.push(Verse { chapter, number, text });
                    pos = tv + used2;
                }
            }
            _ => break,
        }
    }
    out
}

fn take_number_loose(s: &str) -> Option<(u32, usize)> {
    let n = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if n == 0 { None } else { s[..n].parse().ok().map(|v| (v, n)) }
}

fn string_field(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let i = json.find(&pat)? + pat.len();
    let rest = json[i..].trim_start().strip_prefix(':')?.trim_start();
    read_json_string(rest).map(|(s, _)| s)
}

/// Reads one JSON string starting at its opening quote; returns the text and the bytes used.
fn read_json_string(s: &str) -> Option<(String, usize)> {
    let mut chars = s.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, i + 1)),
            '\\' => match chars.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => {
                    let hex: String = (0..4).filter_map(|_| chars.next().map(|x| x.1)).collect();
                    let mut code = u32::from_str_radix(&hex, 16).ok()?;
                    if (0xD800..0xDC00).contains(&code) {
                        let rest: String = chars.clone().take(6).map(|x| x.1).collect();
                        if let Some(low) = rest.strip_prefix("\\u").and_then(|h| u32::from_str_radix(&h[..4.min(h.len())], 16).ok()) {
                            for _ in 0..6 {
                                chars.next();
                            }
                            code = 0x10000 + ((code - 0xD800) << 10) + (low.wrapping_sub(0xDC00) & 0x3FF);
                        }
                    }
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-canonleaf-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let ed = d.join("resources/bundled/test");
        fs::create_dir_all(ed.join("books")).unwrap();
        fs::write(ed.join("manifest.json"), "{\"kind\": \"bible\", \"title\": \"Test Edition\", \"default\": true}").unwrap();
        fs::write(
            ed.join("books/JHN.json"),
            "{\"book\":\"JHN\",\"chapters\":[{\"chapter\":3,\"verses\":[{\"reference\":\"JHN.3.16\",\"number\":\"16\",\"text\":\"For God so loved the world \\u201cthat\\u201d he gave.\"},{\"reference\":\"JHN.3.17\",\"number\":\"17\",\"text\":\"Not to condemn the world.\"}]}]}",
        )
        .unwrap();
        fs::write(
            ed.join("books/PRO.json"),
            "{\"book\":\"PRO\",\"chapters\":[{\"chapter\":15,\"verses\":[{\"reference\":\"PRO.15.1\",\"number\":\"1\",\"text\":\"A gentle answer turns away wrath.\"}]}]}",
        )
        .unwrap();
        d
    }

    #[test]
    fn finds_editions_and_ignores_other_folders() {
        let d = fixture("find");
        let b = find_bibles(&d);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].title, "Test Edition");
        assert!(b[0].is_default);
        assert!(find_bibles(&std::env::temp_dir().join("no-such-canonleaf-folder")).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn references_are_read_from_a_question() {
        assert_eq!(find_reference("What does John 3:16 mean?").unwrap(), Reference { book: "JHN", chapter: 3, from: 16, to: 16 });
        assert_eq!(find_reference("read Romans 8:28-30").unwrap(), Reference { book: "ROM", chapter: 8, from: 28, to: 30 });
        assert_eq!(find_reference("explain 1 John 1:9").unwrap().book, "1JN");
        assert_eq!(find_reference("Psalm 23").unwrap(), Reference { book: "PSA", chapter: 23, from: 0, to: 0 });
        assert_eq!(find_reference("first corinthians 13").unwrap().book, "1CO");
        assert!(find_reference("how do I purify water").is_none());
        // a word that merely contains a book name is not a reference
        assert!(find_reference("the jobless numbers 2024").is_none());
    }

    #[test]
    fn a_named_verse_is_quoted_with_its_edition_and_link() {
        let d = fixture("quote");
        let p = search(&d, "what does John 3:16 say", 3);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].heading, "John 3:16");
        assert_eq!(p[0].source, "Canonleaf: Test Edition");
        assert_eq!(p[0].link, "canonleaf:JHN.3.16");
        assert!(p[0].text.contains("loved the world \u{201c}that\u{201d}"), "{}", p[0].text);
        assert!(!p[0].text.contains("condemn"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_topic_question_finds_matching_verses_and_nothing_is_invented() {
        let d = fixture("topic");
        let p = search(&d, "how do I answer someone who is angry, a gentle answer", 3);
        assert_eq!(p.first().map(|x| x.heading.as_str()), Some("Proverbs 15:1"));
        // one shared word is not enough
        assert!(search(&d, "person", 3).len() <= 1);
        assert!(search(&d, "angry person world", 3).iter().all(|x| x.heading != "John 3:17" || x.text.contains("world")));
        assert!(search(&d, "zzzz qqqq", 3).is_empty());
        assert!(search(&std::env::temp_dir().join("no-such-canonleaf-folder"), "love", 3).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn chapters_can_be_read_and_counted() {
        let d = fixture("chapter");
        let b = default_bible(&d).unwrap();
        assert_eq!(chapter(&b, "JHN", 3).len(), 2);
        assert_eq!(chapter_count(&b, "JHN"), 3);
        assert!(chapter(&b, "JHN", 4).is_empty());
        // only the fixed book table makes file names
        assert!(chapter(&b, "../../etc/passwd", 1).is_empty());
        let _ = fs::remove_dir_all(&d);
    }
}
