//! Drug labels, as the FDA published them (openFDA "drug label" files), for reading offline.
//!
//! The big download is JSON, many files, several gigabytes. `build` reads those files one record at a
//! time (never the whole file at once), keeps the few parts a person looks for, and writes one compact
//! text file. `search` and `get` read that file. This is a reference to READ: the assistant still never
//! states or changes a drug dose (see `safety.rs`); the text here is the FDA's, shown as published.

use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

/// The parts of a label that are kept, in the order they are stored: (field in the FDA file, name shown).
pub const SECTIONS: &[(&str, &str)] = &[
    ("boxed_warning", "Boxed warning"),
    ("indications_and_usage", "What it is for"),
    ("dosage_and_administration", "Dosage and how to give it (as the FDA published it)"),
    ("contraindications", "Do not use if"),
    ("warnings", "Warnings"),
    ("warnings_and_cautions", "Warnings and precautions"),
    ("drug_interactions", "Interactions with other medicines"),
];

/// Longest text kept for one part, so a store stays small. The full label is on the FDA's DailyMed site.
pub const MAX_PART: usize = 8000;

const HEADER: &str = "#cindrix-drug-labels 1";

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Label {
    pub id: String,
    pub brand: String,
    pub generic: String,
    pub maker: String,
    /// (name shown, text), only the parts this label has.
    pub parts: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub id: String,
    pub brand: String,
    pub generic: String,
    pub maker: String,
}

// ---------- a small JSON reader (the core uses no outside crates) ----------

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, x)| x),
            _ => None,
        }
    }
    /// A string, or an array of strings joined with `sep`.
    fn text(&self, sep: &str) -> String {
        match self {
            Json::Str(s) => s.clone(),
            Json::Arr(v) => v.iter().map(|x| x.text(sep)).filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(sep),
            _ => String::new(),
        }
    }
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > 64 {
            return Err("too deep".into());
        }
        self.ws();
        match self.b.get(self.i) {
            None => Err("ends too soon".into()),
            Some(b'{') => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Json::Obj(out));
                        }
                        Some(b',') => self.i += 1,
                        Some(b'"') => {
                            let k = self.string()?;
                            self.ws();
                            if self.b.get(self.i) != Some(&b':') {
                                return Err("expected a colon".into());
                            }
                            self.i += 1;
                            let v = self.value(depth + 1)?;
                            out.push((k, v));
                        }
                        _ => return Err("bad object".into()),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Json::Arr(out));
                        }
                        Some(b',') => self.i += 1,
                        None => return Err("ends too soon".into()),
                        _ => out.push(self.value(depth + 1)?),
                    }
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') if self.b[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            Some(b'f') if self.b[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            Some(b'n') if self.b[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            Some(_) => {
                let s = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                std::str::from_utf8(&self.b[s..self.i]).ok().and_then(|t| t.parse::<f64>().ok()).map(Json::Num).ok_or_else(|| "bad value".to_string())
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.b.get(self.i..self.i + 4).ok_or("bad escape")?;
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(h).map_err(|_| "bad escape")?, 16).map_err(|_| "bad escape".to_string())
    }
    fn string(&mut self) -> Result<String, String> {
        self.i += 1; // opening quote
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or("string ends too soon")?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).map_err(|_| "not text".to_string()),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("bad escape")?;
                    self.i += 1;
                    let ch = match e {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' | b'f' => ' ',
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) && self.b.get(self.i) == Some(&b'\\') && self.b.get(self.i + 1) == Some(&b'u') {
                                self.i += 2;
                                let lo = self.hex4()?;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            char::from_u32(cp).unwrap_or('\u{FFFD}')
                        }
                        other => other as char,
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
    }
}

fn parse(bytes: &[u8]) -> Result<Json, String> {
    let mut p = P { b: bytes, i: 0 };
    p.value(0)
}

// ---------- reading a big FDA file one record at a time ----------

/// Calls `each` with the bytes of every object in the top-level `"results"` array, without holding the file in memory.
fn for_each_record<R: Read>(r: R, mut each: impl FnMut(&[u8])) -> std::io::Result<()> {
    let mut depth = 0usize;
    let mut in_str = false;
    let mut esc = false;
    let mut cur_str: Vec<u8> = Vec::new();
    let mut last_str: Vec<u8> = Vec::new();
    let mut top_is_results = false;
    let mut rec: Vec<u8> = Vec::new();
    let mut in_rec = false;
    let mut rec_depth = 0usize;
    for byte in BufReader::with_capacity(1 << 20, r).bytes() {
        let c = byte?;
        if in_rec {
            rec.push(c);
        }
        if in_str {
            if esc {
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                in_str = false;
                last_str = std::mem::take(&mut cur_str);
            }
            if in_str && depth == 1 {
                cur_str.push(c);
            }
            continue;
        }
        match c {
            b'"' => {
                in_str = true;
                cur_str.clear();
            }
            b':' if depth == 1 => top_is_results = last_str == b"results",
            b'{' | b'[' => {
                depth += 1;
                if c == b'{' && top_is_results && depth == 3 && !in_rec {
                    in_rec = true;
                    rec_depth = depth;
                    rec.clear();
                    rec.push(c);
                }
            }
            b'}' | b']' => {
                if in_rec && depth == rec_depth && c == b'}' {
                    in_rec = false;
                    each(&rec);
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    Ok(())
}

fn cap(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() <= MAX_PART {
        return t.to_string();
    }
    let cut: String = t.chars().take(MAX_PART).collect();
    format!("{}... (shortened: the full label is on DailyMed)", cut.trim_end())
}

fn label_from(rec: &Json) -> Option<Label> {
    let fda = rec.get("openfda");
    let names = |k: &str| fda.and_then(|f| f.get(k)).map(|x| x.text("; ")).unwrap_or_default();
    let brand = names("brand_name");
    let generic = names("generic_name");
    if brand.trim().is_empty() && generic.trim().is_empty() {
        return None;
    }
    let id = rec.get("set_id").or_else(|| rec.get("id")).map(|x| x.text("")).unwrap_or_default();
    if id.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for (field, shown) in SECTIONS {
        if let Some(v) = rec.get(field) {
            let t = v.text("\n\n");
            if !t.trim().is_empty() {
                parts.push((shown.to_string(), cap(&t)));
            }
        }
    }
    Some(Label { id, brand, generic, maker: names("manufacturer_name"), parts })
}

// ---------- the compact store ----------

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\r', "").replace('\n', "\\n")
}

fn unesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn line_for(l: &Label) -> String {
    let mut f: Vec<String> = vec![esc(&l.id), esc(&l.brand), esc(&l.generic), esc(&l.maker)];
    for (_, shown) in SECTIONS {
        f.push(esc(l.parts.iter().find(|(n, _)| n == shown).map(|(_, t)| t.as_str()).unwrap_or("")));
    }
    f.join("\t")
}

fn label_from_line(line: &str) -> Option<Label> {
    let f: Vec<&str> = line.split('\t').collect();
    if f.len() < 4 + SECTIONS.len() {
        return None;
    }
    let mut parts = Vec::new();
    for (i, (_, shown)) in SECTIONS.iter().enumerate() {
        let t = unesc(f[4 + i]);
        if !t.is_empty() {
            parts.push((shown.to_string(), t));
        }
    }
    Some(Label { id: unesc(f[0]), brand: unesc(f[1]), generic: unesc(f[2]), maker: unesc(f[3]), parts })
}

/// Reads every `*.json` in `raw_dir` (the FDA's label files, unzipped) and writes the compact store to `out`.
/// Returns how many labels were kept. A damaged file is skipped and counted in the second number.
pub fn build(raw_dir: &Path, out: &Path) -> Result<(usize, usize), String> {
    let mut files: Vec<_> = fs::read_dir(raw_dir)
        .map_err(|_| "The drug label files are not on the drive yet. Run Get-DrugLabels first.".to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("json")).unwrap_or(false))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err("There are no drug label files in that folder yet. Run Get-DrugLabels first.".into());
    }
    let mut text = String::from(HEADER);
    text.push('\n');
    let (mut kept, mut skipped) = (0usize, 0usize);
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        let file = match fs::File::open(f) {
            Ok(x) => x,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        let r = for_each_record(file, |bytes| match parse(bytes) {
            Ok(j) => {
                if let Some(l) = label_from(&j) {
                    if seen.insert(l.id.clone()) {
                        text.push_str(&line_for(&l));
                        text.push('\n');
                        kept += 1;
                    }
                }
            }
            Err(_) => skipped += 1,
        });
        if r.is_err() {
            skipped += 1;
        }
    }
    if kept == 0 {
        return Err("No drug labels could be read from those files.".into());
    }
    crate::fsio::write_atomic(out, text.as_bytes()).map_err(|_| "The drug label list could not be saved. Check that the drive has room.".to_string())?;
    Ok((kept, skipped))
}

/// How many labels the store holds (0 when there is none or it is damaged).
pub fn count(store: &str) -> usize {
    if !store.starts_with(HEADER) {
        return 0;
    }
    store.lines().skip(1).filter(|l| !l.is_empty()).count()
}

fn norm(s: &str) -> Vec<String> {
    crate::retrieval::tokens(s)
}

/// Labels whose brand or generic name matches the words typed, best first. Names count most; the
/// "what it is for" text counts a little, so "heartburn" finds the medicines for it.
pub fn search(store: &str, query: &str, limit: usize) -> Vec<Summary> {
    let q = norm(query);
    if q.is_empty() || !store.starts_with(HEADER) {
        return Vec::new();
    }
    let mut hits: Vec<(f64, Summary)> = Vec::new();
    for line in store.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 + SECTIONS.len() {
            continue;
        }
        let names = norm(&format!("{} {}", unesc(f[1]), unesc(f[2])));
        let name_text = format!("{} {}", unesc(f[1]), unesc(f[2])).to_lowercase();
        let uses_idx = 4 + SECTIONS.iter().position(|(k, _)| *k == "indications_and_usage").unwrap_or(1);
        let uses = unesc(f[uses_idx]).to_lowercase();
        let mut score = 0.0;
        let mut all_found = true;
        for w in &q {
            let mut got = 0.0;
            if names.iter().any(|n| n == w) {
                got = 5.0;
            } else if names.iter().any(|n| n.starts_with(w.as_str())) {
                got = 3.0;
            } else if name_text.contains(w.as_str()) {
                got = 1.0;
            } else if uses.contains(w.as_str()) {
                got = 0.5;
            }
            if got == 0.0 {
                all_found = false;
            }
            score += got;
        }
        if all_found && score > 0.0 {
            hits.push((score, Summary { id: unesc(f[0]), brand: unesc(f[1]), generic: unesc(f[2]), maker: unesc(f[3]) }));
        }
    }
    hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.1.brand.cmp(&b.1.brand)));
    hits.truncate(limit);
    hits.into_iter().map(|(_, s)| s).collect()
}

/// One label by its id.
pub fn get(store: &str, id: &str) -> Option<Label> {
    if !store.starts_with(HEADER) {
        return None;
    }
    store.lines().skip(1).find(|l| l.split('\t').next().map(unesc).as_deref() == Some(id)).and_then(label_from_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{"meta":{"disclaimer":"x","results":{"skip":0,"limit":2,"total":3}},"results":[
{"set_id":"aaa-1","openfda":{"brand_name":["Zestril"],"generic_name":["LISINOPRIL"],"manufacturer_name":["Acme Pharma"]},
 "indications_and_usage":["Treats high blood pressure."],"dosage_and_administration":["Take as published: 10 mg once a day.\nTabs \"quoted\" \u00e9"],"warnings":["Pregnancy: stop and call."]},
{"set_id":"bbb-2","openfda":{"brand_name":["Tums"],"generic_name":["CALCIUM CARBONATE"]},"indications_and_usage":["Relieves heartburn and sour stomach."]},
{"set_id":"ccc-3","openfda":{},"indications_and_usage":["No names, so it is skipped."]}
]}"#;

    fn tmp(name: &str) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("cx-dl-{}-{}-{}", name, std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn built() -> String {
        let d = tmp("b");
        fs::write(d.join("drug-label-0001.json"), FILE).unwrap();
        let out = d.join("store.txt");
        let (kept, skipped) = build(&d, &out).unwrap();
        assert_eq!((kept, skipped), (2, 0));
        let s = fs::read_to_string(&out).unwrap();
        fs::remove_dir_all(&d).ok();
        s
    }

    #[test]
    fn records_are_read_one_by_one_and_unnamed_ones_are_skipped() {
        let mut n = 0;
        for_each_record(FILE.as_bytes(), |_| n += 1).unwrap();
        assert_eq!(n, 3, "meta must not count as a record");
        assert_eq!(count(&built()), 2);
    }

    #[test]
    fn a_label_keeps_its_parts_and_text_with_quotes_and_accents() {
        let store = built();
        let l = get(&store, "aaa-1").unwrap();
        assert_eq!(l.brand, "Zestril");
        assert_eq!(l.generic, "LISINOPRIL");
        assert_eq!(l.maker, "Acme Pharma");
        let dose = &l.parts.iter().find(|(n, _)| n.starts_with("Dosage")).unwrap().1;
        assert!(dose.contains("10 mg once a day") && dose.contains("\"quoted\" é") && dose.contains('\n'));
        assert!(l.parts.iter().any(|(n, _)| n == "Warnings"));
        assert!(get(&store, "nope").is_none());
    }

    #[test]
    fn search_finds_names_and_what_a_medicine_is_for() {
        let store = built();
        assert_eq!(search(&store, "lisinopril", 5)[0].id, "aaa-1");
        assert_eq!(search(&store, "zest", 5)[0].id, "aaa-1");
        assert_eq!(search(&store, "heartburn", 5)[0].brand, "Tums");
        assert!(search(&store, "warfarin", 5).is_empty());
        assert!(search(&store, "", 5).is_empty());
        assert!(search("not a store", "tums", 5).is_empty());
    }

    #[test]
    fn long_parts_are_shortened_and_say_so() {
        let long = "a".repeat(MAX_PART + 500);
        let t = cap(&long);
        assert!(t.contains("shortened") && t.len() < MAX_PART + 100);
    }

    #[test]
    fn no_files_or_junk_files_give_plain_errors() {
        let d = tmp("e");
        assert!(build(&d, &d.join("o.txt")).unwrap_err().contains("Get-DrugLabels"));
        fs::write(d.join("x.json"), "{ this is not json").unwrap();
        assert!(build(&d, &d.join("o.txt")).unwrap_err().contains("No drug labels"));
        assert!(!d.join("o.txt").exists());
        fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn json_escapes_and_pairs_are_read() {
        let j = parse(br#"{"a":"\ud83d\ude00 \u00e9 \\ \/","b":[1,true,null]}"#).unwrap();
        assert_eq!(j.get("a").unwrap().text(""), "😀 é \\ /");
    }
}
