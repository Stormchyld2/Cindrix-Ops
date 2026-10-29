//! A small JSON reader, standard library only. The Bible-study files (Canonleaf's resource packs)
//! are JSON, and the core takes no outside crates, so this reads just what they need: the whole
//! document into a tree, with plain-words errors. It only reads; nothing here writes JSON.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    /// Keys in the order they were written. Objects here have a handful of keys, so a list is fine.
    Obj(Vec<(String, Json)>),
}

const MAX_DEPTH: usize = 64;

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(v) => v.iter().find(|(k, _)| k == key).map(|(_, x)| x),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
    /// A whole, non-negative number that fits in `u32` (chapter and verse numbers).
    pub fn as_u32(&self) -> Option<u32> {
        match self {
            Json::Num(n) if *n >= 0.0 && n.fract() == 0.0 && *n <= u32::MAX as f64 => Some(*n as u32),
            _ => None,
        }
    }
    /// The text under `key`, or "" when the key is missing or is not text.
    pub fn text(&self, key: &str) -> &str {
        self.get(key).and_then(|x| x.as_str()).unwrap_or("")
    }
    /// The list under `key`, or an empty list.
    pub fn list(&self, key: &str) -> &[Json] {
        self.get(key).and_then(|x| x.as_arr()).unwrap_or(&[])
    }
}

pub fn parse(s: &str) -> Result<Json, String> {
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    let mut p = Parser { b: s.as_bytes(), s, i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.err("there is more after the end"));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    s: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn err(&self, what: &str) -> String {
        format!("The file is not valid JSON ({} near byte {}).", what, self.i)
    }
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> bool {
        if self.i < self.b.len() && self.b[self.i] == c {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn word(&mut self, w: &str, v: Json) -> Result<Json, String> {
        if self.s[self.i..].starts_with(w) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(self.err("an unknown word"))
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.err("nested too deeply"));
        }
        match self.b.get(self.i) {
            None => Err(self.err("it ends too soon")),
            Some(b'{') => {
                self.i += 1;
                let mut out: Vec<(String, Json)> = Vec::new();
                self.ws();
                if self.eat(b'}') {
                    return Ok(Json::Obj(out));
                }
                loop {
                    self.ws();
                    if self.b.get(self.i) != Some(&b'"') {
                        return Err(self.err("a name was expected"));
                    }
                    let k = self.string()?;
                    self.ws();
                    if !self.eat(b':') {
                        return Err(self.err("a colon was expected"));
                    }
                    self.ws();
                    let v = self.value(depth + 1)?;
                    out.push((k, v));
                    self.ws();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b'}') {
                        return Ok(Json::Obj(out));
                    }
                    return Err(self.err("a comma or closing brace was expected"));
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut out = Vec::new();
                self.ws();
                if self.eat(b']') {
                    return Ok(Json::Arr(out));
                }
                loop {
                    self.ws();
                    out.push(self.value(depth + 1)?);
                    self.ws();
                    if self.eat(b',') {
                        continue;
                    }
                    if self.eat(b']') {
                        return Ok(Json::Arr(out));
                    }
                    return Err(self.err("a comma or closing bracket was expected"));
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(c) if *c == b'-' || c.is_ascii_digit() => {
                let start = self.i;
                self.i += 1;
                while self.i < self.b.len() && matches!(self.b[self.i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') {
                    self.i += 1;
                }
                self.s[start..self.i].parse::<f64>().map(Json::Num).map_err(|_| self.err("a bad number"))
            }
            Some(_) => Err(self.err("an unexpected character")),
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() {
            return Err(self.err("a short \\u escape"));
        }
        let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).ok().and_then(|x| u32::from_str_radix(x, 16).ok());
        match h {
            Some(v) => {
                self.i += 4;
                Ok(v)
            }
            None => Err(self.err("a bad \\u escape")),
        }
    }
    /// Reads a string; the opening quote is at `self.i`.
    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            // copy the plain run up to the next quote or backslash in one piece
            let start = self.i;
            while self.i < self.b.len() && self.b[self.i] != b'"' && self.b[self.i] != b'\\' {
                self.i += 1;
            }
            out.push_str(&self.s[start..self.i]);
            match self.b.get(self.i) {
                None => return Err(self.err("a text is not closed")),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                _ => {
                    self.i += 1;
                    let Some(&c) = self.b.get(self.i) else { return Err(self.err("a text is not closed")) };
                    self.i += 1;
                    match c {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xD800..0xDC00).contains(&hi) && self.s[self.i..].starts_with("\\u") {
                                let save = self.i;
                                self.i += 2;
                                let lo = self.hex4()?;
                                if (0xDC00..0xE000).contains(&lo) {
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    self.i = save;
                                    0xFFFD
                                }
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(self.err("a bad escape")),
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_values() {
        let v = parse(r#" {"a": [1, 2.5, -3, true, null], "b": {"c": "x"}, "d": []} "#).unwrap();
        assert_eq!(v.list("a").len(), 5);
        assert_eq!(v.list("a")[1].as_f64(), Some(2.5));
        assert_eq!(v.list("a")[0].as_u32(), Some(1));
        assert_eq!(v.list("a")[2].as_u32(), None);
        assert_eq!(v.get("b").unwrap().text("c"), "x");
        assert!(v.list("d").is_empty());
        assert_eq!(v.text("missing"), "");
    }

    #[test]
    fn reads_escapes_and_unicode() {
        let v = parse(r#"["a\"b\\c\/d\n\t", "é", "😀", "café – ¶ ἀββά"]"#).unwrap();
        let a = v.as_arr().unwrap();
        assert_eq!(a[0].as_str(), Some("a\"b\\c/d\n\t"));
        assert_eq!(a[1].as_str(), Some("é"));
        assert_eq!(a[2].as_str(), Some("😀"));
        assert_eq!(a[3].as_str(), Some("café – ¶ ἀββά"));
    }

    #[test]
    fn a_lone_surrogate_becomes_a_replacement_mark() {
        let v = parse(r#""\ud83dx""#).unwrap();
        assert_eq!(v.as_str(), Some("\u{fffd}x"));
    }

    #[test]
    fn accepts_a_byte_order_mark() {
        assert!(parse("\u{feff}{\"a\":1}").is_ok());
    }

    #[test]
    fn rejects_bad_files_in_plain_words() {
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "{\"a\":1} x", "\"open", "[1 2]", "nul", "{'a':1}"] {
            let e = parse(bad).unwrap_err();
            assert!(e.starts_with("The file is not valid JSON"), "{bad}: {e}");
        }
    }

    #[test]
    fn deep_nesting_is_refused_not_a_crash() {
        let deep = "[".repeat(500) + &"]".repeat(500);
        assert!(parse(&deep).is_err());
    }
}
