//! Which topical books and Bible dictionaries cite a verse (for the Explain window).
//!
//! Only the structured reference fields of an entry are read: Nave's `lines`, Torrey's `aspects` and
//! `references`, and a dictionary's `scripture_references`. Verse numbers found in prose are never
//! used, a reference that cannot be read exactly is skipped, and a reference to a whole chapter
//! ("Isaiah 53") does not count as citing one verse. Read-only.
use super::*;

/// Most mentions kept from one book, and from all books together.
const PER_BOOK: usize = 8;
const IN_ALL: usize = 40;
const EXCERPT: usize = 320;
const POINT: usize = 160;
const POINTS: usize = 4;

/// An entry of a topical book or dictionary that cites at least one of the asked-about verses.
#[derive(Debug, Clone, PartialEq)]
pub struct Mention {
    pub module_id: String,
    pub source: String,
    /// topical, dictionary or theology, as the book declares itself
    pub kind: String,
    pub title: String,
    /// The subheadings of a topic that cite the verse (Nave: "River of Damascus").
    pub points: Vec<String>,
    /// For an article with no subheadings, the text around the citation.
    pub excerpt: String,
    /// The asked-about verses this entry cites, as `BOOK.CHAPTER.VERSE`.
    pub verses: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Target {
    book: usize,
    chapter: u32,
    verse: u32,
}

/// A run of verses: `from` to `to` inclusive, each as (chapter, verse).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    book: usize,
    from: (u32, u32),
    to: (u32, u32),
}

fn book_of(token: &str) -> Option<usize> {
    let t = token.trim().trim_end_matches('.');
    lookup_book(&[t]).or_else(|| book_index(t))
}

fn num(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.is_empty() || s.len() > 3 || !s.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok().filter(|n| *n > 0)
}

/// `Rev.1.8`, `JHN.3.16-JHN.3.18`, `1John.4.9-10`.
fn dotted(text: &str) -> Option<Span> {
    let point = |s: &str| -> Option<(usize, u32, u32)> {
        let mut it = s.trim().split('.');
        let book = book_of(it.next()?)?;
        let ch = num(it.next()?)?;
        let v = num(it.next()?)?;
        if it.next().is_some() {
            return None;
        }
        Some((book, ch, v))
    };
    let (a, b) = match text.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (text, None),
    };
    let (book, c1, v1) = point(a)?;
    let (c2, v2) = match b {
        None => (c1, v1),
        Some(b) if b.trim().chars().all(|c| c.is_ascii_digit()) => (c1, num(b)?),
        Some(b) => {
            let (b2, c2, v2) = point(b)?;
            if b2 != book {
                return None;
            }
            (c2, v2)
        }
    };
    ((c2, v2) >= (c1, v1)).then_some(Span { book, from: (c1, v1), to: (c2, v2) })
}

/// The verses a reference text names: a dotted code, or a printed list such as "Psalms 46:5,7",
/// "Isa. 41:4; 44:6" or "Rev. 1:8, 11; 21:6". Whole-chapter references are left out.
fn spans_in(text: &str) -> Vec<Span> {
    let text = text.trim();
    if let Some(s) = dotted(text) {
        return vec![s];
    }
    // the book is everything before the first " <digits>" that starts the numbers
    let mut cut = None;
    for (i, c) in text.char_indices() {
        if c == ' ' {
            let rest = &text[i + 1..];
            let n = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            if n > 0 && matches!(rest[n..].chars().next(), None | Some(':' | ',' | '-' | ';')) {
                cut = Some(i);
                break;
            }
        }
    }
    let Some(cut) = cut else { return Vec::new() };
    let words: Vec<&str> = text[..cut].split_whitespace().map(|w| w.trim_end_matches('.')).collect();
    let Some(book) = lookup_book(&words) else { return Vec::new() };
    let mut out = Vec::new();
    for group in text[cut..].split(';') {
        let Some((c, verses)) = group.trim().split_once(':') else { continue };
        let Some(ch) = num(c) else { continue };
        for item in verses.split(',') {
            let item = item.trim().trim_end_matches(|c: char| c.is_ascii_alphabetic());
            let (a, b) = match item.split_once(['-', '\u{2013}', '\u{2014}']) {
                Some((a, b)) => (num(a), num(b)),
                None => (num(item), num(item)),
            };
            if let (Some(a), Some(b)) = (a, b) {
                if b >= a {
                    out.push(Span { book, from: (ch, a), to: (ch, b) });
                }
            }
        }
    }
    out
}

fn mark(found: &mut Vec<String>, spans: &[Span], targets: &[Target]) -> bool {
    let mut any = false;
    for t in targets {
        if spans.iter().any(|s| s.book == t.book && (t.chapter, t.verse) >= s.from && (t.chapter, t.verse) <= s.to) {
            any = true;
            let code = format!("{}.{}.{}", BOOKS[t.book].code, t.chapter, t.verse);
            if !found.contains(&code) {
                found.push(code);
            }
        }
    }
    any
}

fn squeeze(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn clip_at_word(text: &str, max: usize) -> String {
    let text = squeeze(text);
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max).collect();
    let cut = cut.rsplit_once(' ').map(|(a, _)| a.to_string()).unwrap_or(cut);
    format!("{}\u{2026}", cut.trim_end_matches(|c: char| c.is_whitespace() || ",;:.".contains(c)))
}

fn push_point(points: &mut Vec<String>, label: &str) {
    let label = clip_at_word(label, POINT);
    if !label.is_empty() && !points.contains(&label) {
        points.push(label);
    }
}

/// A line's label without the leading dash and without the printed references that follow it.
fn line_label(text: &str, refs: &[Json]) -> String {
    let text = text.trim();
    let cut = refs.iter().map(|r| r.text("display")).filter(|d| !d.is_empty()).filter_map(|d| text.find(d)).min();
    let label = match cut {
        Some(i) => &text[..i],
        None => text,
    };
    label.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{2013}' | '\u{2014}' | '-' | '.' | ',' | ';' | ':')).to_string()
}

fn ref_spans(r: &Json) -> Vec<Span> {
    match r {
        Json::Str(s) => spans_in(s),
        o => {
            let (start, end) = (o.text("start"), o.text("end"));
            if !start.is_empty() {
                let joined = if end.is_empty() { start.to_string() } else { format!("{start}-{end}") };
                let s = spans_in(&joined);
                if !s.is_empty() {
                    return s;
                }
            }
            for k in ["osis", "passage", "display", "raw"] {
                let t = o.text(k);
                if !t.is_empty() {
                    let s = spans_in(t);
                    if !s.is_empty() {
                        return s;
                    }
                }
            }
            Vec::new()
        }
    }
}

/// About `EXCERPT` characters of the article around the printed reference that cites the verse,
/// or the start of the article when that text is not found in it.
fn excerpt_around(e: &Json, raw: &str) -> String {
    let mut blocks: Vec<&str> = e.list("definition_blocks").iter().filter_map(|b| b.as_str()).collect();
    if blocks.is_empty() {
        blocks = ["definition", "body", "text"].iter().map(|k| e.text(k)).filter(|t| !t.is_empty()).collect();
    }
    let first = raw.split([';', ',']).next().unwrap_or("").trim();
    for needle in [raw.trim(), first] {
        if needle.chars().count() < 5 {
            continue;
        }
        for block in &blocks {
            if let Some(at) = block.find(needle) {
                let half = EXCERPT / 2;
                let n = needle.chars().count();
                let before: Vec<char> = block[..at].chars().rev().take(half).collect();
                let after: String = block[at..].chars().take(half + n).collect();
                let lead = block[..at].chars().count() > half;
                let tail = block[at..].chars().count() > half + n;
                let text: String = before.into_iter().rev().chain(after.chars()).collect();
                let words: Vec<&str> = text.split_whitespace().collect();
                // begin and end at whole words
                let from = usize::from(lead);
                let to = if tail { words.len().saturating_sub(1) } else { words.len() };
                if from < to {
                    return format!("{}{}{}", if lead { "\u{2026}" } else { "" }, words[from..to].join(" "), if tail { "\u{2026}" } else { "" });
                }
            }
        }
    }
    match blocks.first() {
        Some(b) => clip_at_word(b, EXCERPT),
        None => String::new(),
    }
}

/// What one entry says about the asked-about verses, or `None` when it does not cite them.
fn mention_of(e: &Json, targets: &[Target]) -> Option<(String, Vec<String>, String, Vec<String>)> {
    let title = ["term", "title", "headword", "topic"].iter().map(|k| e.text(k)).find(|t| !t.is_empty())?.to_string();
    let mut found: Vec<String> = Vec::new();
    let mut points: Vec<String> = Vec::new();
    let mut cited_in = String::new();

    // Nave: levelled lines; a hit on a line marks the section heading it sits under
    let mut section = String::new();
    for line in e.list("lines") {
        let refs = line.list("references");
        let label = line_label(line.text("text"), refs);
        if line.get("level").and_then(|l| l.as_u32()).unwrap_or(2) <= 2 || section.is_empty() {
            section = label.clone();
        }
        let mut hit = false;
        for r in refs {
            hit |= mark(&mut found, &ref_spans(r), targets);
        }
        if hit {
            push_point(&mut points, if section.is_empty() { &label } else { &section });
        }
    }
    // Torrey: aspects carry their own reference lists
    for aspect in e.list("aspects") {
        let mut hit = false;
        for r in aspect.list("references") {
            hit |= mark(&mut found, &ref_spans(r), targets);
        }
        if hit {
            push_point(&mut points, aspect.text("label"));
        }
    }
    // topic-level references
    if points.is_empty() {
        for r in e.list("references") {
            mark(&mut found, &ref_spans(r), targets);
        }
    }
    // a dictionary article
    if found.is_empty() {
        for r in e.list("scripture_references") {
            let raw = match r {
                Json::Str(s) => s.as_str(),
                o => o.text("raw"),
            };
            let mut spans = Vec::new();
            match r {
                Json::Str(s) => spans.extend(spans_in(s)),
                o => {
                    for x in o.list("osis").iter().filter_map(|x| x.as_str()) {
                        spans.extend(spans_in(x));
                    }
                    if spans.is_empty() {
                        spans.extend(spans_in(raw));
                    }
                }
            }
            if mark(&mut found, &spans, targets) && cited_in.is_empty() {
                cited_in = raw.to_string();
            }
        }
    }
    if found.is_empty() {
        return None;
    }
    points.truncate(POINTS);
    let excerpt = if !points.is_empty() { String::new() } else { excerpt_around(e, &cited_in) };
    Some((title, points, excerpt, found))
}

impl Library {
    /// Entries of the topical books, Bible dictionaries and theology books that cite any of
    /// `verses` (`BOOK.CHAPTER.VERSE`), those citing the most of them first.
    pub fn mentions(&self, verses: &[String]) -> Vec<Mention> {
        let mut targets: Vec<Target> = Vec::new();
        for v in verses.iter().take(30) {
            if let Some(Ref { book, chapter, verse: Some(verse), .. }) = parse_osis(v) {
                if let Some(idx) = book_index(book) {
                    let t = Target { book: idx, chapter, verse };
                    if !targets.contains(&t) {
                        targets.push(t);
                    }
                }
            }
        }
        let mut out: Vec<Mention> = Vec::new();
        if targets.is_empty() {
            return out;
        }
        for m in self.modules.iter().filter(|m| matches!(m.kind.as_str(), "topical" | "dictionary" | "theology")) {
            let mut here: Vec<Mention> = Vec::new();
            for f in m.role_files("articles") {
                let Some(path) = m.member(&f.path) else { continue };
                let Ok(j) = read_json(&path) else { continue };
                for e in entries_of(&j) {
                    if let Some((title, points, excerpt, verses)) = mention_of(e, &targets) {
                        here.push(Mention { module_id: m.id.clone(), source: m.title.clone(), kind: m.kind.clone(), title, points, excerpt, verses });
                    }
                }
            }
            here.sort_by(|a, b| b.verses.len().cmp(&a.verses.len()).then(b.points.len().cmp(&a.points.len())));
            here.truncate(PER_BOOK);
            out.extend(here);
            if out.len() >= IN_ALL {
                out.truncate(IN_ALL);
                break;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn t(code: &str) -> Vec<Target> {
        let r = parse_osis(code).unwrap();
        vec![Target { book: book_index(r.book).unwrap(), chapter: r.chapter, verse: r.verse.unwrap() }]
    }

    fn covers(text: &str, code: &str) -> bool {
        mark(&mut Vec::new(), &spans_in(text), &t(code))
    }

    #[test]
    fn printed_lists_name_each_verse() {
        assert!(covers("Psalms 46:5,7", "PSA.46.7"));
        assert!(covers("Psalms 46:5,7", "PSA.46.5"));
        assert!(!covers("Psalms 46:5,7", "PSA.46.6"));
        assert!(covers("2 Corinthians 1:4,5", "2CO.1.5"));
        assert!(covers("Rev. 1:8, 11; 21:6; 22:13", "REV.21.6"));
        assert!(covers("Rev. 1:8, 11; 21:6; 22:13", "REV.1.11"));
        assert!(!covers("Rev. 1:8, 11; 21:6; 22:13", "REV.21.11"));
        assert!(covers("Isa. 41:4; 44:6", "ISA.44.6"));
        assert!(covers("John 3:16-18", "JHN.3.17"));
        assert!(!covers("John 3:16-18", "JHN.3.19"));
        assert!(covers("Song of Solomon 2:1", "SNG.2.1"));
    }

    #[test]
    fn dotted_codes_in_any_common_spelling() {
        assert!(covers("2KI.5.12", "2KI.5.12"));
        assert!(covers("Rev.1.8", "REV.1.8"));
        assert!(covers("Ps.23.1", "PSA.23.1"));
        assert!(covers("Matt.5.3", "MAT.5.3"));
        assert!(covers("John.3.16-John.3.18", "JHN.3.17"));
        assert!(covers("1John.4.9-10", "1JN.4.10"));
        assert!(!covers("1John.4.9-10", "JHN.4.10"));
    }

    #[test]
    fn a_whole_chapter_is_not_a_mention_of_one_verse() {
        assert!(!covers("Isaiah 53", "ISA.53.5"));
        assert!(!covers("ISA.53", "ISA.53.5"));
        assert!(!covers("Isa 53; 54", "ISA.54.1"));
    }

    #[test]
    fn the_wrong_book_or_unreadable_text_never_matches() {
        assert!(!covers("1 John 3:16", "JHN.3.16"));
        assert!(!covers("Foo 99:1", "JHN.3.16"));
        assert!(!covers("see above", "JHN.3.16"));
        assert!(spans_in("").is_empty());
    }

    fn tmp() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!("cindrix-mentions-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn install(root: &Path, id: &str, kind: &str, articles: &str) {
        let dir = root.join(id);
        fs::create_dir_all(dir.join("articles")).unwrap();
        fs::write(
            dir.join("manifest.json"),
            format!(r#"{{"schema_version":2,"module_id":"{id}","title":"Book {id}","kind":"{kind}","license":"Public Domain","files":[{{"path":"articles/a.json","role":"articles","book":null}}]}}"#),
        )
        .unwrap();
        fs::write(dir.join("articles/a.json"), articles).unwrap();
    }

    const NAVE: &str = r#"{"data":[
      {"entry_id":"nave:abana","term":"ABANA","lines":[{"level":2,"text":"–River of Damascus 2Ki 5:12","references":[{"display":"2Ki 5:12","start":"2KI.5.12","end":null}]}]},
      {"entry_id":"nave:love","term":"LOVE","lines":[
         {"level":2,"text":"–Of God for the world John 3:16","references":[{"display":"John 3:16","start":"JHN.3.16","end":null}]},
         {"level":2,"text":"–Of Christ John 15:9","references":[{"display":"John 15:9","start":"JHN.15.9","end":null}]}]}]}"#;
    const TORREY: &str = r#"{"entries":[
      {"entry_id":"t-comfort","title":"Comfort","references":[{"display":"Isaiah 49:13"},{"display":"John 14:18"}],
       "aspects":[{"label":"God comforts","references":["Isaiah 49:13"]},{"label":"Christ is with","references":["John 14:18"]}]}]}"#;
    const EASTON: &str = r#"{"data":[
      {"entry_id":"e.abaddon","term":"Abaddon","definition_blocks":["Destruction, the Hebrew name of the angel of the bottomless pit (Rev. 9:11)."],"scripture_references":[{"raw":"Rev. 9:11","osis":["Rev.9.11"]}]},
      {"entry_id":"e.none","term":"None","definition_blocks":["No references."],"scripture_references":[]}]}"#;

    fn library() -> Library {
        let root = tmp();
        install(&root, "nave", "topical", NAVE);
        install(&root, "torrey", "theology", TORREY);
        install(&root, "easton", "dictionary", EASTON);
        Library::open(&[root])
    }

    #[test]
    fn finds_the_topics_and_articles_that_cite_a_verse() {
        let lib = library();
        let love = lib.mentions(&["JHN.3.16".into()]);
        assert_eq!(love.len(), 1);
        assert_eq!(love[0].title, "LOVE");
        assert_eq!(love[0].points, vec!["Of God for the world".to_string()]);
        assert_eq!(love[0].verses, vec!["JHN.3.16".to_string()]);
        assert_eq!(love[0].source, "Book nave");

        let comfort = lib.mentions(&["JHN.14.18".into()]);
        assert_eq!(comfort.len(), 1);
        assert_eq!(comfort[0].points, vec!["Christ is with".to_string()], "only the aspect that cites the verse");

        let abaddon = lib.mentions(&["REV.9.11".into()]);
        assert_eq!(abaddon.len(), 1);
        assert!(abaddon[0].excerpt.contains("Rev. 9:11"), "{}", abaddon[0].excerpt);
        assert!(abaddon[0].points.is_empty());

        assert!(lib.mentions(&["GEN.1.1".into()]).is_empty(), "nothing cites it, so nothing is shown");
        assert!(lib.mentions(&["JHN.3".into(), "nonsense".into()]).is_empty(), "only verses are looked up");
    }

    #[test]
    fn several_verses_are_searched_in_one_pass_and_ranked() {
        let lib = library();
        let both = lib.mentions(&["JHN.3.16".into(), "JHN.15.9".into(), "2KI.5.12".into()]);
        let by: Vec<(&str, usize)> = both.iter().map(|m| (m.title.as_str(), m.verses.len())).collect();
        assert_eq!(by, vec![("LOVE", 2), ("ABANA", 1)]);
    }

    #[test]
    fn the_text_around_the_citation_is_the_excerpt() {
        let filler = "and so the text goes on ".repeat(30);
        let e = crate::json::parse(&format!(
            r#"{{"term":"Faith","definition_blocks":["{filler}as in Heb. 11:1, 6 where faith is described. {filler}"],"scripture_references":[{{"raw":"Heb. 11:1, 6","osis":["Heb.11.1","Heb.11.6"]}}]}}"#
        ))
        .unwrap();
        let (_, _, excerpt, found) = mention_of(&e, &t("HEB.11.6")).unwrap();
        assert_eq!(found, vec!["HEB.11.6".to_string()]);
        assert!(excerpt.contains("Heb. 11:1, 6 where faith is described"), "{excerpt}");
        assert!(excerpt.starts_with('\u{2026}') && excerpt.ends_with('\u{2026}'));
        assert!(excerpt.chars().count() < 420);
    }

    #[test]
    fn long_text_is_clipped_at_a_word() {
        let c = clip_at_word(&"word ".repeat(200), 20);
        assert!(c.ends_with('\u{2026}') && c.chars().count() <= 21, "{c}");
        assert_eq!(clip_at_word("  short   text ", 50), "short text");
    }
}
