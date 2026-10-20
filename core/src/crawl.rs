//! Saving a website for offline use: a polite crawler and the page rewriting that turns what it
//! fetched into a ZIM archive (see `zim`).
//!
//! All the logic lives here with no network code of its own: the app supplies a `Fetcher`
//! (a small wrapper around an HTTP client) and tests supply a fake one. What gets saved:
//! the pages of one site (all of it, or just one section), plus the pictures, style sheets
//! and fonts those pages use, from any host. Scripts are not saved, so sites that need
//! JavaScript to show their content come out empty; plain text sites, wikis, blogs, manuals
//! and documentation come out well.

use crate::calendar;
use crate::zim::ZimWriter;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub const USER_AGENT: &str = "CindrixOps-Archiver/0.1 (personal offline copy; respects robots.txt)";

// ---------- addresses ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Url {
    pub https: bool,
    pub host: String,
    pub port: Option<u16>,
    /// Always starts with '/'. Left percent-encoded as it came.
    pub path: String,
    pub query: Option<String>,
}

fn remove_dots(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let segs: Vec<&str> = path.split('/').collect();
    for (i, s) in segs.iter().enumerate() {
        match *s {
            "." => {
                if i == segs.len() - 1 {
                    out.push("");
                }
            }
            ".." => {
                if out.len() > 1 {
                    out.pop();
                }
                if i == segs.len() - 1 {
                    out.push("");
                }
            }
            _ => out.push(s),
        }
    }
    let joined = out.join("/");
    if joined.starts_with('/') { joined } else { format!("/{}", joined) }
}

fn split_query(rest: &str) -> (String, Option<String>) {
    match rest.find('?') {
        Some(i) => (rest[..i].to_string(), Some(rest[i + 1..].to_string()).filter(|q| !q.is_empty())),
        None => (rest.to_string(), None),
    }
}

impl Url {
    pub fn parse(s: &str) -> Option<Url> {
        let s = s.trim();
        let s = s.split('#').next().unwrap_or("");
        let lower = s.to_ascii_lowercase();
        let (https, rest) = if lower.starts_with("https://") {
            (true, &s[8..])
        } else if lower.starts_with("http://") {
            (false, &s[7..])
        } else {
            return None;
        };
        let end = rest.find(['/', '?']).unwrap_or(rest.len());
        let mut auth = &rest[..end];
        if let Some(at) = auth.rfind('@') {
            auth = &auth[at + 1..];
        }
        let (host, port) = match auth.rfind(':') {
            Some(c) if auth[c + 1..].chars().all(|d| d.is_ascii_digit()) && !auth[c + 1..].is_empty() => {
                (&auth[..c], auth[c + 1..].parse::<u16>().ok())
            }
            _ => (auth, None),
        };
        if host.is_empty() || host.contains(' ') {
            return None;
        }
        let port = port.filter(|p| !(*p == 80 && !https) && !(*p == 443 && https));
        let (path, query) = split_query(&rest[end..]);
        let path = if path.is_empty() { "/".to_string() } else { remove_dots(&path) };
        Some(Url { https, host: host.to_ascii_lowercase(), port, path, query })
    }

    pub fn to_string(&self) -> String {
        let mut s = format!("{}://{}", if self.https { "https" } else { "http" }, self.host);
        if let Some(p) = self.port {
            s.push_str(&format!(":{}", p));
        }
        s.push_str(&self.path);
        if let Some(q) = &self.query {
            s.push('?');
            s.push_str(q);
        }
        s
    }

    /// Identity of a page for "have we seen it": no scheme, no fragment.
    pub fn key(&self) -> String {
        let mut s = self.host.clone();
        if let Some(p) = self.port {
            s.push_str(&format!(":{}", p));
        }
        s.push_str(&self.path);
        if let Some(q) = &self.query {
            s.push('?');
            s.push_str(q);
        }
        s
    }

    /// The address a link points to, as seen from this page. None for links that are not
    /// web pages (mailto:, javascript:, data:, #section, empty).
    pub fn resolve(&self, href: &str) -> Option<Url> {
        let h = href.trim();
        let h = h.split('#').next().unwrap_or("");
        if h.is_empty() {
            return None;
        }
        let scheme_end = h.find([':', '/', '?']);
        if let Some(i) = scheme_end {
            if h.as_bytes()[i] == b':' {
                return Url::parse(h); // absolute; None unless http(s)
            }
        }
        let scheme = if self.https { "https" } else { "http" };
        if let Some(rest) = h.strip_prefix("//") {
            return Url::parse(&format!("{}://{}", scheme, rest));
        }
        let (path, query) = if h.starts_with('/') {
            split_query(h)
        } else if h.starts_with('?') {
            (self.path.clone(), Some(h[1..].to_string()).filter(|q| !q.is_empty()))
        } else {
            let (p, q) = split_query(h);
            let dir = &self.path[..self.path.rfind('/').map(|i| i + 1).unwrap_or(0)];
            (format!("{}{}", dir, p), q)
        };
        Some(Url { https: self.https, host: self.host.clone(), port: self.port, path: remove_dots(&path), query })
    }
}

pub fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && i + 2 <= b.len() - 1 + 0 {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn pct_encode_path(s: &str) -> String {
    let mut out = String::new();
    for &c in s.as_bytes() {
        if c.is_ascii_alphanumeric() || b"-._~/@,:+=()!*".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

fn fnv(s: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// Where a fetched address lives inside the archive: `host/path`. A web address that ends in
/// a slash becomes `.../index.html`; a query string becomes a short fingerprint.
pub fn zim_path(u: &Url) -> String {
    let mut p = pct_decode(&u.path);
    if p.ends_with('/') {
        p.push_str("index.html");
    }
    let host = match u.port {
        Some(n) => format!("{}_{}", u.host, n),
        None => u.host.clone(),
    };
    let mut s = format!("{}{}", host, p);
    if let Some(q) = &u.query {
        s.push_str(&format!("@q{:08x}", fnv(q)));
    }
    s
}

/// Relative link from one archive path to another: ("a/b/c.html", "a/x.css") -> "../x.css".
pub fn relative(from: &str, to: &str) -> String {
    let f: Vec<&str> = from.split('/').collect();
    let t: Vec<&str> = to.split('/').collect();
    let fdir = &f[..f.len().saturating_sub(1)];
    let mut common = 0;
    while common < fdir.len() && common < t.len() - 1 && fdir[common] == t[common] {
        common += 1;
    }
    let mut parts: Vec<&str> = Vec::new();
    for _ in common..fdir.len() {
        parts.push("..");
    }
    parts.extend(&t[common..]);
    parts.join("/")
}

pub fn decode_html(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(semi) = tail.find(';').filter(|n| *n <= 10) {
            let ent = &tail[1..semi];
            let rep: Option<String> = match ent {
                "amp" => Some("&".into()),
                "lt" => Some("<".into()),
                "gt" => Some(">".into()),
                "quot" => Some("\"".into()),
                "apos" => Some("'".into()),
                "nbsp" => Some(" ".into()),
                "ndash" => Some("–".into()),
                "mdash" => Some("—".into()),
                "hellip" => Some("…".into()),
                "rsquo" | "lsquo" => Some("'".into()),
                "ldquo" | "rdquo" => Some("\"".into()),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32).map(|c| c.to_string())
                }
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32).map(|c| c.to_string()),
                _ => None,
            };
            if let Some(r) = rep {
                out.push_str(&r);
                rest = &tail[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

/// Bytes to text. UTF-8 when it is valid; otherwise Windows-1252/Latin-1 when the page says so.
pub fn to_utf8(bytes: &[u8], content_type: &str) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let head: String = bytes.iter().take(2048).map(|b| if b.is_ascii() { *b as char } else { ' ' }).collect::<String>().to_ascii_lowercase();
    let says = |s: &str| content_type.to_ascii_lowercase().contains(s) || head.contains(s);
    if says("8859-1") || says("latin1") || says("1252") || says("iso-8859-15") {
        const CP: [char; 32] = [
            '€', ' ', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', ' ', 'Ž', ' ', ' ', '‘', '’', '“', '”', '•', '–', '—', '˜',
            '™', 'š', '›', 'œ', ' ', 'ž', 'Ÿ',
        ];
        return bytes
            .iter()
            .map(|&b| if (0x80..0xa0).contains(&b) { CP[(b - 0x80) as usize] } else { b as char })
            .collect();
    }
    String::from_utf8_lossy(bytes).to_string()
}

// ---------- reading and rewriting pages ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub url: Url,
    /// A link to another page (`<a href>`) as opposed to something the page needs (picture, style sheet).
    pub page: bool,
}

pub struct Page {
    pub html: String,
    pub title: String,
    pub text: String,
    pub links: Vec<Link>,
}

type Map<'a> = &'a dyn Fn(&Url) -> Option<String>;

struct Ctx<'a> {
    base: &'a Url,
    from: &'a str,
    map: Map<'a>,
    links: Vec<Link>,
}

impl Ctx<'_> {
    /// Rewrite one link value. Pointing inside the archive: a relative path. Otherwise an
    /// absolute address (so it never breaks by being read against the local server).
    fn fix(&mut self, value: &str, page: bool) -> String {
        let decoded = decode_html(value);
        let frag = decoded.find('#').map(|i| decoded[i..].to_string()).unwrap_or_default();
        match self.base.resolve(&decoded) {
            None => value.to_string(),
            Some(u) => {
                self.links.push(Link { url: u.clone(), page });
                match (self.map)(&u) {
                    Some(z) if !self.from.is_empty() => format!("{}{}", pct_encode_path(&relative(self.from, &z)), frag),
                    _ => format!("{}{}", u.to_string(), frag),
                }
            }
        }
    }

    fn srcset(&mut self, value: &str) -> String {
        value
            .split(',')
            .map(|c| {
                let c = c.trim();
                match c.split_once(char::is_whitespace) {
                    Some((u, d)) => format!("{} {}", self.fix(u, false), d.trim()),
                    None => self.fix(c, false),
                }
            })
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn css(&mut self, css: &str) -> String {
        let lower = css.to_ascii_lowercase();
        let mut out = String::with_capacity(css.len());
        let mut i = 0;
        while i < css.len() {
            let u = lower[i..].find("url(").map(|x| x + i);
            let imp = lower[i..].find("@import").map(|x| x + i);
            match (u, imp) {
                (Some(a), b) if b.map_or(true, |b| a < b) => {
                    out.push_str(&css[i..a]);
                    let Some(close) = css[a..].find(')').map(|x| x + a) else {
                        out.push_str(&css[a..]);
                        return out;
                    };
                    let inner = css[a + 4..close].trim().trim_matches(|c| c == '"' || c == '\'').trim();
                    if self.base.resolve(inner).is_some() {
                        let fixed = self.fix(inner, false);
                        out.push_str(&format!("url(\"{}\")", fixed));
                    } else {
                        out.push_str(&css[a..=close]);
                    }
                    i = close + 1;
                }
                (_, Some(b)) => {
                    out.push_str(&css[i..b]);
                    let after = b + 7;
                    let rest = css[after..].trim_start();
                    let skipped = css[after..].len() - rest.len();
                    if rest.starts_with('"') || rest.starts_with('\'') {
                        let q = rest.chars().next().unwrap();
                        if let Some(end) = rest[1..].find(q) {
                            let target = &rest[1..1 + end];
                            let fixed = self.fix(target, false);
                            out.push_str(&format!("@import \"{}\"", fixed));
                            i = after + skipped + end + 2;
                            continue;
                        }
                    }
                    out.push_str("@import");
                    i = after;
                }
                _ => {
                    out.push_str(&css[i..]);
                    break;
                }
            }
        }
        out
    }
}

struct Tag {
    name: String,
    attrs: Vec<(String, Option<String>)>,
    self_closing: bool,
    end: usize,
}

fn parse_tag(src: &str, start: usize) -> Option<Tag> {
    let b = src.as_bytes();
    let mut i = start + 1;
    let ns = i;
    while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b':') {
        i += 1;
    }
    if i == ns {
        return None;
    }
    let name = src[ns..i].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while i < b.len() && (b[i].is_ascii_whitespace()) {
            i += 1;
        }
        if i >= b.len() {
            return None;
        }
        if b[i] == b'>' {
            i += 1;
            break;
        }
        if b[i] == b'/' {
            self_closing = true;
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' && b[i] != b'>' && b[i] != b'/' {
            i += 1;
        }
        if i == s {
            i += 1;
            continue;
        }
        let an = src[s..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let vs = i + 1;
                let ve = b[vs..].iter().position(|c| *c == q).map(|x| x + vs)?;
                attrs.push((an, Some(src[vs..ve].to_string())));
                i = ve + 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'>' {
                    i += 1;
                }
                attrs.push((an, Some(src[vs..i].to_string())));
            }
        } else {
            attrs.push((an, None));
        }
        self_closing = false;
    }
    Some(Tag { name, attrs, self_closing, end: i })
}

fn escape_attr(v: &str) -> String {
    v.replace('"', "&quot;")
}

fn write_tag(out: &mut String, t: &Tag) {
    out.push('<');
    out.push_str(&t.name);
    for (n, v) in &t.attrs {
        out.push(' ');
        out.push_str(n);
        if let Some(v) = v {
            out.push_str("=\"");
            out.push_str(&escape_attr(v));
            out.push('"');
        }
    }
    if t.self_closing {
        out.push_str(" /");
    }
    out.push('>');
}

const BLOCKS: &[&str] = &["p", "br", "div", "li", "h1", "h2", "h3", "h4", "h5", "h6", "tr", "section", "article", "pre", "blockquote", "dt", "dd", "table", "ul", "ol"];

fn attr<'a>(t: &'a Tag, name: &str) -> Option<&'a str> {
    t.attrs.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
}

/// Read a page: find its links, title and text, and produce the version to store, with links
/// pointing inside the archive where `map` knows the target. Scripts are dropped.
pub fn process_html(src: &str, base: &Url, from: &str, map: Map) -> Page {
    let lower = src.to_ascii_lowercase();
    let b = src.as_bytes();
    let mut cx = Ctx { base, from, map, links: Vec::new() };
    let mut out = String::with_capacity(src.len());
    let mut text = String::new();
    let mut title = String::new();
    let mut in_title = false;
    let mut head_seen = false;
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            let next = src[i..].find('<').map(|x| x + i).unwrap_or(src.len());
            let t = &src[i..next];
            out.push_str(t);
            text.push_str(t);
            if in_title {
                title.push_str(t);
            }
            i = next;
            continue;
        }
        if lower[i..].starts_with("<!--") {
            i = src[i..].find("-->").map(|x| x + i + 3).unwrap_or(src.len());
            continue;
        }
        if lower[i..].starts_with("<!") || lower[i..].starts_with("<?") {
            let close = src[i..].find('>').map(|x| x + i + 1).unwrap_or(src.len());
            if lower[i..].starts_with("<!doctype") {
                out.push_str(&src[i..close]);
            }
            i = close;
            continue;
        }
        if b.get(i + 1) == Some(&b'/') {
            let close = src[i..].find('>').map(|x| x + i + 1).unwrap_or(src.len());
            let name: String = lower[i + 2..close.min(lower.len())].chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
            if name == "title" {
                in_title = false;
            }
            if BLOCKS.contains(&name.as_str()) {
                text.push('\n');
            }
            if name != "noscript" {
                out.push_str(&src[i..close]);
            }
            i = close;
            continue;
        }
        let Some(mut tag) = parse_tag(src, i) else {
            out.push_str("&lt;");
            i += 1;
            continue;
        };
        i = tag.end;
        match tag.name.as_str() {
            "script" => {
                i = lower[i..].find("</script").map(|x| x + i).and_then(|p| src[p..].find('>').map(|x| p + x + 1)).unwrap_or(src.len());
                continue;
            }
            "noscript" | "base" => continue,
            "meta" => {
                let ct = attr(&tag, "http-equiv").map(|v| v.eq_ignore_ascii_case("content-type")).unwrap_or(false);
                if attr(&tag, "charset").is_some() || ct {
                    continue;
                }
            }
            "style" => {
                let end = lower[i..].find("</style").map(|x| x + i).unwrap_or(src.len());
                let css = cx.css(&src[i..end]);
                write_tag(&mut out, &tag);
                out.push_str(&css);
                i = end;
                continue;
            }
            _ => {}
        }
        if BLOCKS.contains(&tag.name.as_str()) {
            text.push('\n');
        }
        if tag.name == "title" {
            in_title = true;
        }
        let name = tag.name.clone();
        let rel = attr(&tag, "rel").unwrap_or("").to_ascii_lowercase();
        let as_attr = attr(&tag, "as").unwrap_or("").to_ascii_lowercase();
        let wants_link = name == "link"
            && (rel.contains("stylesheet") || rel.contains("icon") || (rel.contains("preload") && matches!(as_attr.as_str(), "style" | "font" | "image")));
        let mut new_attrs = Vec::with_capacity(tag.attrs.len());
        for (n, v) in std::mem::take(&mut tag.attrs) {
            let Some(val) = v else {
                new_attrs.push((n, None));
                continue;
            };
            let nv = match (name.as_str(), n.as_str()) {
                (_, "integrity") => continue,
                ("a" | "area", "href") => cx.fix(&val, true),
                ("link", "href") if wants_link => cx.fix(&val, false),
                ("img" | "input" | "video" | "audio", "src" | "poster") if !matches!(name.as_str(), "video" | "audio") => cx.fix(&val, false),
                ("video", "poster") => cx.fix(&val, false),
                ("img" | "source", "srcset" | "data-srcset") => cx.srcset(&val),
                ("img", "data-src") => cx.fix(&val, false),
                (_, "style") => cx.css(&val),
                _ => val,
            };
            new_attrs.push((n, Some(nv)));
        }
        tag.attrs = new_attrs;
        write_tag(&mut out, &tag);
        if name == "head" && !head_seen {
            head_seen = true;
            out.push_str("<meta charset=\"utf-8\">");
        }
    }
    if !head_seen {
        out = format!("<meta charset=\"utf-8\">{}", out);
    }
    let clean = |s: &str| decode_html(s).split_whitespace().collect::<Vec<_>>().join(" ");
    let lines: Vec<String> = decode_html(&text)
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    Page { html: out, title: clean(&title), text: lines.join("\n"), links: cx.links }
}

/// Style sheet: find and rewrite its url(...) and @import references.
pub fn process_css(css: &str, base: &Url, from: &str, map: Map) -> (String, Vec<Link>) {
    let mut cx = Ctx { base, from, map, links: Vec::new() };
    let out = cx.css(css);
    (out, cx.links)
}

// ---------- robots.txt ----------

#[derive(Debug, Default, Clone)]
pub struct Robots {
    rules: Vec<(bool, String)>,
    pub delay_ms: Option<u64>,
}

fn pat_matches(pat: &str, path: &str) -> bool {
    let (pat, anchored) = match pat.strip_suffix('$') {
        Some(p) => (p, true),
        None => (pat, false),
    };
    fn go(p: &[u8], s: &[u8], anchored: bool) -> bool {
        match p.first() {
            None => !anchored || s.is_empty(),
            Some(b'*') => (0..=s.len()).any(|k| go(&p[1..], &s[k..], anchored)),
            Some(c) => s.first() == Some(c) && go(&p[1..], &s[1..], anchored),
        }
    }
    go(pat.as_bytes(), path.as_bytes(), anchored)
}

impl Robots {
    pub fn parse(text: &str) -> Robots {
        struct Group {
            agents: Vec<String>,
            rules: Vec<(bool, String)>,
            delay: Option<f64>,
        }
        let mut groups: Vec<Group> = Vec::new();
        let mut last_was_agent = false;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once(':') else { continue };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            match k.as_str() {
                "user-agent" => {
                    if !last_was_agent || groups.is_empty() {
                        groups.push(Group { agents: Vec::new(), rules: Vec::new(), delay: None });
                    }
                    groups.last_mut().unwrap().agents.push(v.to_ascii_lowercase());
                    last_was_agent = true;
                }
                "allow" | "disallow" => {
                    last_was_agent = false;
                    if let Some(g) = groups.last_mut() {
                        if !v.is_empty() {
                            g.rules.push((k == "allow", v.to_string()));
                        }
                    }
                }
                "crawl-delay" => {
                    last_was_agent = false;
                    if let Some(g) = groups.last_mut() {
                        g.delay = v.parse().ok();
                    }
                }
                _ => last_was_agent = false,
            }
        }
        let pick = groups
            .iter()
            .find(|g| g.agents.iter().any(|a| a == "cindrixops-archiver" || a == "cindrixops"))
            .or_else(|| groups.iter().find(|g| g.agents.iter().any(|a| a == "*")));
        match pick {
            Some(g) => Robots { rules: g.rules.clone(), delay_ms: g.delay.map(|d| (d * 1000.0) as u64) },
            None => Robots::default(),
        }
    }

    /// May this path (with query) be fetched? The longest matching rule wins; allow wins a tie.
    pub fn allows(&self, path_and_query: &str) -> bool {
        let mut best: Option<(usize, bool)> = None;
        for (allow, pat) in &self.rules {
            if pat_matches(pat, path_and_query) {
                let len = pat.len();
                best = match best {
                    Some((l, a)) if l > len || (l == len && a) => Some((l, a)),
                    _ => Some((len, *allow)),
                };
            }
        }
        best.map_or(true, |(_, a)| a)
    }
}

// ---------- the crawl ----------

pub struct Response {
    pub status: u16,
    /// Where the fetch ended up after redirects.
    pub final_url: String,
    pub content_type: String,
    pub body: Vec<u8>,
}

pub trait Fetcher: Sync {
    /// Fetch one address, following redirects. Err when the body is bigger than `max_bytes` or the request fails.
    fn get(&self, url: &str, max_bytes: usize) -> Result<Response, String>;
}

#[derive(Debug, Clone)]
pub struct Options {
    pub start: String,
    pub name: String,
    pub max_pages: usize,
    pub max_mb: u64,
    /// Stay inside the start page's folder instead of the whole site.
    pub section_only: bool,
    pub delay_ms: u64,
    pub respect_robots: bool,
    pub max_depth: usize,
    /// How many addresses are fetched at once (1 to 8). Forced to 1 when the site's robots.txt
    /// asks for a crawl delay, so a site that asks for slow service gets it.
    pub workers: usize,
}

impl Options {
    pub fn new(start: &str, name: &str) -> Options {
        Options {
            start: start.to_string(),
            name: name.to_string(),
            max_pages: 500,
            max_mb: 1000,
            section_only: false,
            delay_ms: 600,
            respect_robots: true,
            max_depth: 12,
            workers: 1,
        }
    }

    /// The "Fast" setting: several requests at once and a short pause between rounds.
    /// Still obeys robots.txt, including any crawl delay it asks for.
    pub fn fast(mut self) -> Options {
        self.delay_ms = 60;
        self.workers = 6;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    /// "crawling", "writing" or "done"
    pub phase: String,
    pub pages: usize,
    pub files: usize,
    pub mb: f64,
    pub queued: usize,
    pub current: String,
    pub failed: usize,
}

#[derive(Debug)]
pub struct Summary {
    pub zim: PathBuf,
    pub pages: usize,
    pub files: usize,
    pub bytes: u64,
    pub failed: Vec<String>,
    pub blocked_by_robots: usize,
    pub hit_limit: bool,
}

const FILE_CAP: usize = 25 * 1024 * 1024;

#[derive(PartialEq, Clone, Copy)]
enum RecKind {
    Html,
    Css,
    Other,
}

struct Rec {
    url: Url,
    zim: String,
    mime: String,
    file: PathBuf,
    kind: RecKind,
}

pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// A readable default name for a site: its host without "www.".
pub fn default_name(url: &str) -> String {
    Url::parse(url).map(|u| u.host.trim_start_matches("www.").to_string()).unwrap_or_default()
}

fn guess_mime(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

fn bare_mime(content_type: &str, path: &str) -> String {
    let m = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if m.is_empty() { guess_mime(path).to_string() } else { m }
}

/// Addresses on this computer or its own network. Links found on a website are never followed
/// there, so a page cannot make the app poke at things on the user's machine or home network.
pub fn is_private_host(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") || h.ends_with(".internal") || h.starts_with('[') {
        return true;
    }
    let parts: Vec<u8> = h.split('.').filter_map(|p| p.parse::<u8>().ok()).collect();
    if parts.len() == 4 && h.split('.').count() == 4 {
        return matches!(parts[0], 0 | 10 | 127)
            || (parts[0] == 169 && parts[1] == 254)
            || (parts[0] == 172 && (16..=31).contains(&parts[1]))
            || (parts[0] == 192 && parts[1] == 168);
    }
    false
}

fn start_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..=i].to_string(),
        None => "/".to_string(),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Cut text into ~900 character pieces at paragraph boundaries, for the assistant's search.
fn chunk_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if cur.len() + line.len() > 900 && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            if out.len() >= 40 {
                return out;
            }
        }
        if line.starts_with('#') {
            cur.push(' ');
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if !cur.trim().is_empty() && out.len() < 40 {
        out.push(cur);
    }
    out
}

/// Save a website. Writes the archive to `zim_path`, a searchable text copy to `text_path`
/// (for the assistant), and uses `spool` as scratch space (removed afterwards).
pub fn run(
    o: &Options,
    f: &dyn Fetcher,
    zim_path_out: &Path,
    text_path: &Path,
    spool: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&Progress),
) -> Result<Summary, String> {
    let result = run_inner(o, f, zim_path_out, text_path, spool, cancel, progress);
    let _ = std::fs::remove_dir_all(spool);
    if result.is_err() {
        let _ = std::fs::remove_file(zim_path_out);
        let _ = std::fs::remove_file(zim_path_out.with_extension("zim.clusters.tmp"));
        let _ = std::fs::remove_file(text_path);
    }
    result
}

fn run_inner(
    o: &Options,
    f: &dyn Fetcher,
    zim_out: &Path,
    text_path: &Path,
    spool: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(&Progress),
) -> Result<Summary, String> {
    let start = Url::parse(&o.start).ok_or("That does not look like a web address. It should start with http:// or https://")?;
    std::fs::create_dir_all(spool).map_err(|e| format!("Cannot use the scratch folder: {}", e))?;
    let site = start.host.trim_start_matches("www.").to_string();
    let prefix = if o.section_only { Some(start_dir(&start.path)) } else { None };
    let in_scope = |u: &Url| {
        u.host.trim_start_matches("www.") == site && prefix.as_ref().map_or(true, |p| u.path.starts_with(p.as_str()))
    };

    let robots = if o.respect_robots {
        let ru = format!("{}://{}{}/robots.txt", if start.https { "https" } else { "http" }, start.host, start.port.map(|p| format!(":{}", p)).unwrap_or_default());
        match f.get(&ru, 512 * 1024) {
            Ok(r) if r.status == 200 => Robots::parse(&to_utf8(&r.body, &r.content_type)),
            _ => Robots::default(),
        }
    } else {
        Robots::default()
    };
    let delay = o.delay_ms.max(robots.delay_ms.unwrap_or(0).min(10_000));

    let max_bytes = o.max_mb * 1024 * 1024;
    let mut queue: VecDeque<(Url, usize, bool)> = VecDeque::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut recs: Vec<Rec> = Vec::new();
    let mut aliases: Vec<(Url, String)> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let (mut pages, mut files, mut total, mut blocked) = (0usize, 0usize, 0u64, 0usize);
    let mut hit_limit = false;
    let mut p = Progress { phase: "crawling".into(), ..Default::default() };
    seen.insert(start.key());
    queue.push_back((start.clone(), 0, true));

    let workers = if robots.delay_ms.unwrap_or(0) > 0 { 1 } else { o.workers.clamp(1, 8) };
    'rounds: loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        // Take the next few addresses that are allowed, then fetch them at the same time.
        let mut batch: Vec<(Url, usize)> = Vec::new();
        let mut batch_pages = 0usize;
        while batch.len() < workers {
            let Some((u, depth, is_page)) = queue.pop_front() else { break };
            if is_page && pages + batch_pages >= o.max_pages {
                hit_limit = true;
                continue;
            }
            if total > max_bytes {
                hit_limit = true;
                break 'rounds;
            }
            if is_private_host(&u.host) && u.host != start.host {
                continue;
            }
            let pq = format!("{}{}", u.path, u.query.as_ref().map(|q| format!("?{}", q)).unwrap_or_default());
            if o.respect_robots && u.host == start.host && !robots.allows(&pq) {
                blocked += 1;
                continue;
            }
            if is_page {
                batch_pages += 1;
            }
            batch.push((u, depth));
        }
        if batch.is_empty() {
            break;
        }
        p.current = batch[0].0.to_string();
        p.queued = queue.len();
        progress(&p);
        if delay > 0 && !recs.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
        let results: Vec<Result<Response, String>> = if batch.len() == 1 {
            vec![f.get(&batch[0].0.to_string(), FILE_CAP)]
        } else {
            std::thread::scope(|sc| {
                let handles: Vec<_> = batch.iter().map(|(u, _)| sc.spawn(move || f.get(&u.to_string(), FILE_CAP))).collect();
                handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err("the fetch stopped unexpectedly".into()))).collect()
            })
        };
        for ((u, depth), resp) in batch.into_iter().zip(results) {
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                if recs.is_empty() {
                    return Err(format!("Could not reach {}: {}", u.to_string(), e));
                }
                failed.push(format!("{} ({})", u.to_string(), e));
                p.failed = failed.len();
                continue;
            }
        };
        if resp.status != 200 {
            if recs.is_empty() {
                return Err(format!("{} answered with status {}", u.to_string(), resp.status));
            }
            failed.push(format!("{} (status {})", u.to_string(), resp.status));
            p.failed = failed.len();
            continue;
        }
        let fin = Url::parse(&resp.final_url).unwrap_or_else(|| u.clone());
        if is_private_host(&fin.host) && fin.host != start.host {
            continue;
        }
        if fin.key() != u.key() {
            seen.insert(fin.key());
            aliases.push((u.clone(), fin.key()));
        }
        let mime = bare_mime(&resp.content_type, &fin.path);
        let kind = if mime == "text/html" || mime == "application/xhtml+xml" {
            RecKind::Html
        } else if mime == "text/css" {
            RecKind::Css
        } else {
            RecKind::Other
        };
        // A redirect to a page we already have: nothing more to store.
        if recs.iter().any(|r| r.url.key() == fin.key()) {
            continue;
        }
        let file = spool.join(format!("{}.bin", recs.len()));
        std::fs::write(&file, &resp.body).map_err(|e| format!("Cannot write to the scratch folder: {}", e))?;
        total += resp.body.len() as u64;
        let mut found: Vec<Link> = Vec::new();
        match kind {
            RecKind::Html => {
                pages += 1;
                let text = to_utf8(&resp.body, &resp.content_type);
                found = process_html(&text, &fin, "", &|_| None).links;
            }
            RecKind::Css => {
                files += 1;
                let text = to_utf8(&resp.body, &resp.content_type);
                found = process_css(&text, &fin, "", &|_| None).1;
            }
            RecKind::Other => files += 1,
        }
        recs.push(Rec { zim: zim_path(&fin), url: fin, mime, file, kind });
        for l in found {
            if !seen.insert(l.url.key()) {
                continue;
            }
            if l.page {
                if in_scope(&l.url) && depth < o.max_depth {
                    queue.push_back((l.url, depth + 1, true));
                }
            } else {
                queue.push_back((l.url, depth + 1, false));
            }
        }
        p.pages = pages;
        p.files = files;
        p.mb = total as f64 / 1048576.0;
        }
    }
    if recs.is_empty() {
        return Err("Nothing could be saved from that address.".into());
    }

    // ----- write the archive -----
    p.phase = "writing".into();
    p.current = String::new();
    progress(&p);
    let mut index: HashMap<String, String> = recs.iter().map(|r| (r.url.key(), r.zim.clone())).collect();
    for (alias, target_key) in &aliases {
        if let Some(z) = index.get(target_key).cloned() {
            index.insert(alias.key(), z);
        }
    }
    let map = |u: &Url| index.get(&u.key()).cloned();
    let mut zw = ZimWriter::create(zim_out).map_err(|e| format!("Cannot create the archive: {}", e))?;
    let mut text_out = String::new();
    let mut listing: Vec<(String, String)> = Vec::new();
    let mut written = 0usize;
    for r in &recs {
        if cancel.load(Ordering::Relaxed) {
            zw.abandon();
            return Err("Cancelled".into());
        }
        let body = std::fs::read(&r.file).map_err(|e| format!("Scratch file lost: {}", e))?;
        let (data, title) = match r.kind {
            RecKind::Html => {
                let page = process_html(&to_utf8(&body, ""), &r.url, &r.zim, &map);
                let title = if page.title.is_empty() { r.url.path.trim_matches('/').rsplit('/').next().unwrap_or("").to_string() } else { page.title.clone() };
                if !page.text.is_empty() {
                    for (n, c) in chunk_text(&page.text).into_iter().enumerate() {
                        text_out.push_str(&format!("## {}\n", title.replace('\n', " ")));
                        if n == 0 {
                            text_out.push_str(&format!("Source: {}\n", r.url.to_string()));
                        }
                        text_out.push_str(&c);
                        text_out.push('\n');
                    }
                }
                listing.push((title.clone(), r.zim.clone()));
                (page.html.into_bytes(), title)
            }
            RecKind::Css => (process_css(&to_utf8(&body, ""), &r.url, &r.zim, &map).0.into_bytes(), String::new()),
            RecKind::Other => (body, String::new()),
        };
        let mime = if r.kind == RecKind::Html { "text/html" } else { r.mime.as_str() };
        if zw.add(b'C', &r.zim, &title, mime, data).map_err(|e| format!("Cannot write the archive: {}", e))? {
            written += 1;
        }
        p.current = r.url.to_string();
        progress(&p);
    }
    for (alias, target_key) in &aliases {
        if let Some(target) = index.get(target_key) {
            let z = zim_path(alias);
            if &z != target {
                zw.redirect(b'C', &z, "", b'C', target);
            }
        }
    }

    // Front page listing everything that was saved.
    let today = calendar::today_string();
    let start_zim = index.get(&start.key()).cloned().unwrap_or_default();
    let home = "_cindrix/index.html";
    listing.sort();
    let mut idx = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{n}</title></head><body style=\"font-family:sans-serif;max-width:48em;margin:2em auto;padding:0 1em\">\
<h1>{n}</h1><p>A copy of <b>{u}</b> saved on {d} for reading without internet. Pages belong to their authors.</p>",
        n = esc(&o.name),
        u = esc(&start.to_string()),
        d = today
    );
    if !start_zim.is_empty() {
        idx.push_str(&format!("<p><a href=\"{}\">Open the start page</a></p>", pct_encode_path(&relative(home, &start_zim))));
    }
    idx.push_str(&format!("<h2>All pages ({})</h2><ul>", listing.len()));
    for (t, z) in listing.iter().take(5000) {
        idx.push_str(&format!("<li><a href=\"{}\">{}</a></li>", pct_encode_path(&relative(home, z)), esc(if t.is_empty() { z } else { t })));
    }
    idx.push_str("</ul></body></html>");
    zw.add(b'C', home, &o.name, "text/html", idx.into_bytes()).map_err(|e| e.to_string())?;
    zw.set_main_page(home);
    let host = start.host.clone();
    for (k, v) in [
        ("Title", o.name.as_str()),
        ("Description", &format!("Saved copy of {} ({})", start.to_string(), today)),
        ("Language", "eng"),
        ("Creator", &host),
        ("Publisher", "Cindrix Ops"),
        ("Date", &today),
        ("Name", &format!("cindrix_{}", slug(&o.name))),
        ("Source", &start.to_string()),
        ("Counter", &format!("text/html={}", pages + 1)),
    ] {
        zw.metadata(k, v).map_err(|e| e.to_string())?;
    }
    zw.add(b'M', "Illustration_48x48@1", "", "image/png", crate::png::archive_icon()).map_err(|e| e.to_string())?;
    let stats = zw.finish().map_err(|e| format!("Cannot finish the archive: {}", e))?;
    if let Some(d) = text_path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(text_path, text_out).map_err(|e| format!("Cannot write the search text: {}", e))?;
    p.phase = "done".into();
    progress(&p);
    let _ = written;
    Ok(Summary { zim: zim_out.to_path_buf(), pages, files, bytes: stats.bytes, failed, blocked_by_robots: blocked, hit_limit })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zim::Reader;

    #[test]
    fn urls_parse_and_resolve() {
        let u = Url::parse("HTTPS://User@Example.COM:443/a/b/c.html?x=1#frag").unwrap();
        assert_eq!(u.to_string(), "https://example.com/a/b/c.html?x=1");
        assert_eq!(u.key(), "example.com/a/b/c.html?x=1");
        assert_eq!(u.resolve("d.html").unwrap().to_string(), "https://example.com/a/b/d.html");
        assert_eq!(u.resolve("../d.html").unwrap().to_string(), "https://example.com/a/d.html");
        assert_eq!(u.resolve("/top").unwrap().to_string(), "https://example.com/top");
        assert_eq!(u.resolve("//cdn.test/x.png").unwrap().to_string(), "https://cdn.test/x.png");
        assert_eq!(u.resolve("?page=2").unwrap().to_string(), "https://example.com/a/b/c.html?page=2");
        assert_eq!(u.resolve("http://other.org").unwrap().to_string(), "http://other.org/");
        for bad in ["#top", "", "mailto:a@b.c", "javascript:void(0)", "data:image/png;base64,AAA", "tel:555"] {
            assert!(u.resolve(bad).is_none(), "{}", bad);
        }
        assert!(Url::parse("ftp://x").is_none());
        assert_eq!(Url::parse("http://x.org:8080").unwrap().to_string(), "http://x.org:8080/");
    }

    #[test]
    fn archive_paths() {
        let u = Url::parse("https://example.com/docs/").unwrap();
        assert_eq!(zim_path(&u), "example.com/docs/index.html");
        let u = Url::parse("https://example.com/a%20b.html").unwrap();
        assert_eq!(zim_path(&u), "example.com/a b.html");
        assert_eq!(pct_encode_path("example.com/a b.html"), "example.com/a%20b.html");
        let q = Url::parse("https://example.com/s?id=3").unwrap();
        assert!(zim_path(&q).starts_with("example.com/s@q"));
        assert_ne!(zim_path(&q), zim_path(&Url::parse("https://example.com/s?id=4").unwrap()));
        assert_eq!(relative("a/b/c.html", "a/x.css"), "../x.css");
        assert_eq!(relative("a/b/c.html", "a/b/d.html"), "d.html");
        assert_eq!(relative("a/index.html", "b/y.png"), "../b/y.png");
        assert_eq!(relative("_cindrix/index.html", "e.com/a.html"), "../e.com/a.html");
        assert_eq!(pct_decode("a%20b%2"), "a b%2");
    }

    #[test]
    fn page_rewriting() {
        let base = Url::parse("https://example.com/docs/guide.html").unwrap();
        let html = r##"<!DOCTYPE html><html><head><title>My &amp; Guide</title>
<meta charset="iso-8859-1"><base href="https://evil/"><script src="a.js"></script>
<link rel="stylesheet" href="/style.css" integrity="sha384-x"><link rel="canonical" href="https://example.com/docs/guide.html">
<style>body{background:url(bg.png)}</style></head>
<body onload="x()"><!-- comment --><script>var a="<b>";</script>
<p>Hello <a href="other.html#part">other</a> and <a href="https://elsewhere.org/x">away</a> and <a href="#top">top</a> <a href="mailto:a@b.c">mail</a></p>
<img src="/img/a b.png" srcset="small.png 1x, big.png 2x" alt="x"><noscript><img src="ns.png"></noscript>
<div style="background:url('//cdn.test/b.jpg')">text</div></body></html>"##;
        let mut known = HashMap::new();
        known.insert("example.com/docs/other.html".to_string(), "example.com/docs/other.html".to_string());
        known.insert("example.com/style.css".to_string(), "example.com/style.css".to_string());
        known.insert("example.com/img/a b.png".to_string(), "example.com/img/a b.png".to_string());
        known.insert("cdn.test/b.jpg".to_string(), "cdn.test/b.jpg".to_string());
        let map = |u: &Url| known.get(&u.key()).cloned();
        // The path in the map is decoded, the key is not: use the real key for the space.
        let mut known2 = known.clone();
        known2.insert("example.com/img/a%20b.png".to_string(), "example.com/img/a b.png".to_string());
        let map2 = |u: &Url| known2.get(&u.key()).cloned();
        let _ = map;

        let page = process_html(html, &base, "example.com/docs/guide.html", &map2);
        assert_eq!(page.title, "My & Guide");
        assert!(!page.html.contains("<script"));
        assert!(!page.html.contains("var a"));
        assert!(!page.html.contains("evil"));
        assert!(!page.html.contains("integrity"));
        assert!(!page.html.contains("iso-8859-1"));
        assert!(page.html.contains("<head><meta charset=\"utf-8\">"));
        assert!(page.html.contains("href=\"other.html#part\""), "{}", page.html);
        assert!(page.html.contains("href=\"https://elsewhere.org/x\""));
        assert!(page.html.contains("href=\"#top\""));
        assert!(page.html.contains("href=\"../style.css\""));
        assert!(page.html.contains("src=\"../img/a%20b.png\""), "{}", page.html);
        assert!(page.html.contains("url(&quot;../../cdn.test/b.jpg&quot;)"), "{}", page.html);
        // not in the archive: made absolute so it cannot break
        assert!(page.html.contains("url(\"https://example.com/docs/bg.png\")"));
        assert!(page.html.contains("srcset=\"https://example.com/docs/small.png 1x, https://example.com/docs/big.png 2x\""));
        assert!(page.html.contains("ns.png")); // noscript content kept, wrapper dropped
        assert!(!page.html.contains("noscript"));
        assert!(page.text.contains("Hello other and away"));
        assert!(!page.text.contains("var a"));
        let pages: Vec<_> = page.links.iter().filter(|l| l.page).map(|l| l.url.to_string()).collect();
        assert_eq!(pages, vec!["https://example.com/docs/other.html", "https://elsewhere.org/x"]);
        let assets = page.links.iter().filter(|l| !l.page).count();
        assert!(assets >= 6);
    }

    #[test]
    fn style_sheets() {
        let base = Url::parse("https://example.com/css/site.css").unwrap();
        let css = "@import \"base.css\"; @font-face{src:url(../fonts/f.woff2) format('woff2')} a{background:url(data:image/png;base64,AAA)}";
        let map = |u: &Url| if u.path == "/fonts/f.woff2" { Some("example.com/fonts/f.woff2".to_string()) } else { None };
        let (out, links) = process_css(css, &base, "example.com/css/site.css", &map);
        assert!(out.contains("@import \"https://example.com/css/base.css\""), "{}", out);
        assert!(out.contains("url(\"../fonts/f.woff2\")"), "{}", out);
        assert!(out.contains("url(data:image/png;base64,AAA)"));
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn robots_rules() {
        let r = Robots::parse("User-agent: *\nDisallow: /private/\nAllow: /private/ok.html\nDisallow: /*.pdf$\nCrawl-delay: 2\n\nUser-agent: badbot\nDisallow: /\n");
        assert!(r.allows("/"));
        assert!(r.allows("/docs/a.html"));
        assert!(!r.allows("/private/x.html"));
        assert!(r.allows("/private/ok.html"));
        assert!(!r.allows("/files/manual.pdf"));
        assert!(r.allows("/files/manual.pdf.html"));
        assert_eq!(r.delay_ms, Some(2000));
        let none = Robots::parse("User-agent: *\nDisallow:\n");
        assert!(none.allows("/anything"));
        let ours = Robots::parse("User-agent: CindrixOps-Archiver\nDisallow: /\nUser-agent: *\nAllow: /\n");
        assert!(!ours.allows("/x"));
    }

    #[test]
    fn text_from_latin1() {
        assert_eq!(to_utf8(b"caf\xe9", "text/html; charset=ISO-8859-1"), "café");
        assert_eq!(to_utf8("café".as_bytes(), ""), "café");
        assert_eq!(decode_html("a&#8217;b &#x41; &amp;amp; &bogus;"), "a\u{2019}b A &amp; &bogus;");
    }

    struct Fake {
        pages: HashMap<String, (String, Vec<u8>)>,
        redirects: HashMap<String, String>,
        asked: std::sync::Mutex<Vec<String>>,
    }
    impl Fetcher for Fake {
        fn get(&self, url: &str, _max: usize) -> Result<Response, String> {
            self.asked.lock().unwrap().push(url.to_string());
            let target = self.redirects.get(url).cloned().unwrap_or_else(|| url.to_string());
            match self.pages.get(&target) {
                Some((ct, body)) => Ok(Response { status: 200, final_url: target, content_type: ct.clone(), body: body.clone() }),
                None => Ok(Response { status: 404, final_url: target, content_type: String::new(), body: vec![] }),
            }
        }
    }

    fn site() -> Fake {
        let mut pages = HashMap::new();
        let mut add = |u: &str, ct: &str, body: &str| {
            pages.insert(u.to_string(), (ct.to_string(), body.as_bytes().to_vec()));
        };
        add("https://example.com/robots.txt", "text/plain", "User-agent: *\nDisallow: /secret/\n");
        add(
            "https://example.com/",
            "text/html",
            "<html><head><title>Home</title><link rel=stylesheet href=/s.css></head><body><h1>Water</h1><p>Boil water for one minute to make it safe to drink in an emergency situation.</p>\
<a href=/water/>Water</a> <a href=/secret/x.html>secret</a> <a href=//example.com/old>old</a> <a href=https://other.org/z>away</a> <img src=https://cdn.test/p.png><a href=/deep/1.html>deep</a></body></html>",
        );
        add("https://example.com/s.css", "text/css", "body{background:url(/bg.png)}");
        add("https://example.com/bg.png", "image/png", "PNGDATA");
        add("https://cdn.test/p.png", "image/png", "PNG2");
        add("https://example.com/water/", "text/html", "<title>Water page</title><p>Store one gallon per person per day. # not a heading</p><a href=\"/\">home</a>");
        add("https://example.com/new", "text/html", "<title>New</title><p>moved page</p>");
        add("https://example.com/secret/x.html", "text/html", "<title>S</title>no");
        add("https://example.com/deep/1.html", "text/html", "<title>D1</title><a href=/deep/2.html>2</a>");
        add("https://example.com/deep/2.html", "text/html", "<title>D2</title>two");
        let mut redirects = HashMap::new();
        redirects.insert("https://example.com/old".to_string(), "https://example.com/new".to_string());
        Fake { pages, redirects, asked: std::sync::Mutex::new(Vec::new()) }
    }

    fn tmpdir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-crawl-{}-{}", n, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn opts() -> Options {
        let mut o = Options::new("https://example.com/", "Example");
        o.delay_ms = 0;
        o
    }

    #[test]
    fn saves_a_small_site_end_to_end() {
        let d = tmpdir("e2e");
        let fake = site();
        let cancel = AtomicBool::new(false);
        let mut last = Progress::default();
        let s = run(&opts(), &fake, &d.join("out.zim"), &d.join("text/example.md"), &d.join("spool"), &cancel, &mut |p| last = p.clone()).unwrap();
        assert_eq!(last.phase, "done");
        assert!(s.pages >= 5, "{:?}", s);
        assert_eq!(s.blocked_by_robots, 1);
        assert!(s.failed.is_empty(), "{:?}", s.failed);
        assert!(!d.join("spool").exists());
        assert!(!fake.asked.lock().unwrap().iter().any(|u| u.contains("/secret/x.html")));

        assert!(crate::zim::verify_checksum(&s.zim).unwrap());
        let mut r = Reader::open(&s.zim).unwrap();
        let home = String::from_utf8(r.get(b'C', "example.com/index.html").unwrap().unwrap().data).unwrap();
        assert!(home.contains("href=\"water/index.html\""), "{}", home);
        assert!(home.contains("href=\"s.css\""), "{}", home);
        assert!(home.contains("src=\"../cdn.test/p.png\""), "{}", home);
        assert!(home.contains("href=\"https://other.org/z\""));
        assert!(home.contains("href=\"https://example.com/secret/x.html\"")); // blocked: stays a web link
        let css = String::from_utf8(r.get(b'C', "example.com/s.css").unwrap().unwrap().data).unwrap();
        assert!(css.contains("url(\"bg.png\")"), "{}", css);
        assert_eq!(r.get(b'C', "cdn.test/p.png").unwrap().unwrap().data, b"PNG2");
        assert_eq!(r.get(b'C', "example.com/bg.png").unwrap().unwrap().mime, "image/png");
        // the redirected link works from the old address too
        assert_eq!(String::from_utf8(r.get(b'C', "example.com/old").unwrap().unwrap().data).unwrap().contains("moved page"), true);
        assert!(home.contains("href=\"new\""), "{}", home);
        // front page and information
        let front = String::from_utf8(r.get(b'W', "mainPage").unwrap().unwrap().data).unwrap();
        assert!(front.contains("All pages"));
        assert!(front.contains("../example.com/water/index.html"));
        assert_eq!(r.get(b'M', "Title").unwrap().unwrap().data, b"Example");
        // the assistant's text copy
        let text = std::fs::read_to_string(d.join("text/example.md")).unwrap();
        assert!(text.contains("## Water page"));
        assert!(text.contains("Store one gallon per person per day."));
        assert!(text.contains("Source: https://example.com/water/"));
        let found = crate::retrieval::search(&d.join("text"), "how much water do I store per person", 3);
        assert!(!found.is_empty());
        assert_eq!(found[0].heading, "Water page");
    }


    /// Wraps a fetcher and records how many requests were in flight at once.
    struct Probe<'a> {
        inner: &'a Fake,
        now: std::sync::atomic::AtomicUsize,
        most: std::sync::atomic::AtomicUsize,
    }
    impl Fetcher for Probe<'_> {
        fn get(&self, url: &str, max: usize) -> Result<Response, String> {
            let n = self.now.fetch_add(1, Ordering::SeqCst) + 1;
            self.most.fetch_max(n, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(25));
            let r = self.inner.get(url, max);
            self.now.fetch_sub(1, Ordering::SeqCst);
            r
        }
    }

    fn probe(f: &Fake) -> Probe<'_> {
        Probe { inner: f, now: std::sync::atomic::AtomicUsize::new(0), most: std::sync::atomic::AtomicUsize::new(0) }
    }

    #[test]
    fn fast_mode_saves_the_same_archive_as_normal_mode() {
        let cancel = AtomicBool::new(false);
        let d1 = tmpdir("slow");
        let f1 = site();
        let p1 = probe(&f1);
        let s1 = run(&opts(), &p1, &d1.join("a.zim"), &d1.join("a.md"), &d1.join("sp"), &cancel, &mut |_| {}).unwrap();
        let d2 = tmpdir("fast");
        let f2 = site();
        let p2 = probe(&f2);
        let mut o = opts().fast();
        o.delay_ms = 0;
        let s2 = run(&o, &p2, &d2.join("a.zim"), &d2.join("a.md"), &d2.join("sp"), &cancel, &mut |_| {}).unwrap();
        assert_eq!(p1.most.load(Ordering::SeqCst), 1, "normal mode must fetch one at a time");
        assert!(p2.most.load(Ordering::SeqCst) > 1, "fast mode should fetch several at once");
        assert_eq!((s1.pages, s1.files, s1.bytes, s1.blocked_by_robots), (s2.pages, s2.files, s2.bytes, s2.blocked_by_robots));
        assert_eq!(std::fs::read(&s1.zim).unwrap().len(), std::fs::read(&s2.zim).unwrap().len());
        assert_eq!(std::fs::read_to_string(d1.join("a.md")).unwrap(), std::fs::read_to_string(d2.join("a.md")).unwrap());
        assert!(!f2.asked.lock().unwrap().iter().any(|u| u.contains("/secret/x.html")), "robots.txt must still be obeyed in fast mode");
    }

    #[test]
    fn fast_mode_still_stops_at_the_page_limit() {
        let cancel = AtomicBool::new(false);
        let d = tmpdir("fastlimit");
        let f = site();
        let mut o = opts().fast();
        o.delay_ms = 0;
        o.max_pages = 2;
        let s = run(&o, &f, &d.join("a.zim"), &d.join("a.md"), &d.join("sp"), &cancel, &mut |_| {}).unwrap();
        assert_eq!(s.pages, 2);
        assert!(s.hit_limit);
    }

    #[test]
    fn a_site_that_asks_for_a_crawl_delay_is_fetched_one_at_a_time_even_in_fast_mode() {
        let cancel = AtomicBool::new(false);
        let d = tmpdir("delay");
        let mut f = site();
        f.pages.insert("https://example.com/robots.txt".into(), ("text/plain".into(), b"User-agent: *\nCrawl-delay: 0.01\nDisallow: /secret/\n".to_vec()));
        let p = probe(&f);
        let mut o = opts().fast();
        o.delay_ms = 0;
        run(&o, &p, &d.join("a.zim"), &d.join("a.md"), &d.join("sp"), &cancel, &mut |_| {}).unwrap();
        assert_eq!(p.most.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn limits_depth_and_section() {
        let d = tmpdir("limits");
        let fake = site();
        let cancel = AtomicBool::new(false);
        let mut o = opts();
        o.max_pages = 2;
        let s = run(&o, &fake, &d.join("a.zim"), &d.join("a.md"), &d.join("sp"), &cancel, &mut |_| {}).unwrap();
        assert_eq!(s.pages, 2);
        assert!(s.hit_limit);

        let mut o = opts();
        o.start = "https://example.com/deep/1.html".into();
        o.section_only = true;
        let s = run(&o, &fake, &d.join("b.zim"), &d.join("b.md"), &d.join("sp"), &cancel, &mut |_| {}).unwrap();
        assert_eq!(s.pages, 2); // 1.html and 2.html, nothing outside /deep/

        let mut o = opts();
        o.max_depth = 0;
        let s = run(&o, &fake, &d.join("c.zim"), &d.join("c.md"), &d.join("sp"), &cancel, &mut |_| {}).unwrap();
        assert_eq!(s.pages, 1);
    }

    #[test]
    fn failures_and_cancel_leave_nothing_behind() {
        let d = tmpdir("fail");
        let fake = site();
        let cancel = AtomicBool::new(false);
        let mut o = opts();
        o.start = "https://nowhere.test/".into();
        assert!(run(&o, &fake, &d.join("x.zim"), &d.join("x.md"), &d.join("sp"), &cancel, &mut |_| {}).is_err());
        assert!(!d.join("x.zim").exists());
        o.start = "not a url".into();
        assert!(run(&o, &fake, &d.join("x.zim"), &d.join("x.md"), &d.join("sp"), &cancel, &mut |_| {}).is_err());

        let cancel = AtomicBool::new(true);
        let e = run(&opts(), &fake, &d.join("y.zim"), &d.join("y.md"), &d.join("sp"), &cancel, &mut |_| {}).unwrap_err();
        assert_eq!(e, "Cancelled");
        assert!(!d.join("y.zim").exists());
        assert!(!d.join("sp").exists());
    }

    #[test]
    fn private_addresses_are_not_followed() {
        for h in ["localhost", "127.0.0.1", "192.168.1.5", "10.0.0.2", "172.20.1.1", "169.254.1.1", "printer.local", "[::1]"] {
            assert!(is_private_host(h), "{}", h);
        }
        for h in ["example.com", "8.8.8.8", "172.32.0.1", "11.0.0.1", "192.169.0.1"] {
            assert!(!is_private_host(h), "{}", h);
        }
    }

    #[test]
    fn names() {
        assert_eq!(slug("My Site: Part 1!"), "my-site-part-1");
        assert_eq!(default_name("https://www.ready.gov/water"), "ready.gov");
    }
}
