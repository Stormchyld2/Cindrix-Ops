//! The big offline library (Wikipedia, medical guides, repair manuals ...) lives in Kiwix ZIM files.
//! The `kiwix-serve` program serves them on this computer only (127.0.0.1); this module starts it,
//! searches it and turns the matching articles into plain-text passages for the answer path.

use crate::retrieval::{self, Passage};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const SHARE_PORT: u16 = 8787;

pub struct KiwixHost {
    child: Child,
    pub port: u16,
}

/// All .zim files in a folder.
pub fn zim_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|x| x.to_str()) == Some("zim")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// `share` false: only this computer can connect (127.0.0.1). `share` true: other devices on the
/// same network can read the library too (they cannot change anything).
pub fn args(zims: &[PathBuf], port: u16, share: bool) -> Vec<String> {
    let addr = if share { "0.0.0.0" } else { "127.0.0.1" };
    let mut a = vec!["--address".to_string(), addr.into(), "--port".into(), port.to_string()];
    a.extend(zims.iter().map(|z| z.to_string_lossy().to_string()));
    a
}

impl KiwixHost {
    pub fn start(engine: &Path, zims: &[PathBuf], share: bool) -> Result<KiwixHost, String> {
        if zims.is_empty() {
            return Err("no .zim library files found".into());
        }
        if !engine.exists() {
            return Err(format!("the library server was not found at {}", engine.display()));
        }
        // A steady port when sharing, so other devices can bookmark the address.
        let port = if share && std::net::TcpListener::bind(("0.0.0.0", SHARE_PORT)).is_ok() { SHARE_PORT } else { crate::modelhost::free_port()? };
        let mut cmd = Command::new(engine);
        cmd.args(args(zims, port, share)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(dir) = engine.parent() {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let child = cmd.spawn().map_err(|e| format!("could not start the library server: {}", e))?;
        Ok(KiwixHost { child, port })
    }

    pub fn wait_ready(&mut self, timeout: Duration) -> Result<(), String> {
        let t0 = Instant::now();
        loop {
            if get(self.port, "/").map(|(s, _)| s == 200).unwrap_or(false) {
                return Ok(());
            }
            if let Ok(Some(st)) = self.child.try_wait() {
                return Err(format!("the library server stopped while starting ({})", st));
            }
            if t0.elapsed() > timeout {
                return Err("the library server took too long to start".into());
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for KiwixHost {
    fn drop(&mut self) {
        self.stop();
    }
}

/// This computer's address on the local network, if it has one (for telling other devices where to connect).
/// Nothing is sent: the socket only asks the system which network address it would use.
pub fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    if ip.is_loopback() || ip.is_unspecified() { None } else { Some(ip.to_string()) }
}

/// Minimal HTTP GET to 127.0.0.1. Returns (status, body).
pub fn get(port: u16, path: &str) -> Result<(u16, Vec<u8>), String> {
    let addr = ("127.0.0.1", port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(15))).ok();
    let req = format!("GET {} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", path);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("bad response")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let status: u16 = head.split_whitespace().nth(1).and_then(|x| x.parse().ok()).ok_or("bad status")?;
    Ok((status, raw[split + 4..].to_vec()))
}

pub fn url_encode(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(&s[a..b])
}

pub fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

/// (title, link) for each result in kiwix-serve's XML search output.
pub fn parse_search(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<item>") {
        let after = &rest[i + 6..];
        let end = after.find("</item>").unwrap_or(after.len());
        let item = &after[..end];
        if let (Some(t), Some(l)) = (between(item, "<title>", "</title>"), between(item, "<link>", "</link>")) {
            out.push((decode_entities(t).trim().to_string(), decode_entities(l).trim().to_string()));
        }
        rest = &after[end..];
    }
    out
}

/// Plain text from an article's HTML: drops scripts, styles and tags, keeps paragraph breaks.
pub fn html_to_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0;
    let b = html.as_bytes();
    while i < b.len() {
        if b[i] == b'<' {
            let skip_to = ["script", "style"].iter().find_map(|tag| {
                if lower[i..].starts_with(&format!("<{}", tag)) {
                    lower[i..].find(&format!("</{}>", tag)).map(|e| i + e + tag.len() + 3)
                } else {
                    None
                }
            });
            if let Some(j) = skip_to {
                i = j;
                continue;
            }
            let close = match html[i..].find('>') {
                Some(c) => i + c,
                None => break,
            };
            let tag = lower[i + 1..close].trim_start_matches('/');
            let name: String = tag.chars().take_while(|c| c.is_alphanumeric()).collect();
            if matches!(name.as_str(), "p" | "br" | "div" | "li" | "h1" | "h2" | "h3" | "h4" | "tr") {
                out.push('\n');
            }
            i = close + 1;
        } else {
            let next = html[i..].find('<').map(|x| i + x).unwrap_or(html.len());
            out.push_str(&html[i..next]);
            i = next;
        }
    }
    let text = decode_entities(&out);
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        if !l.is_empty() {
            lines.push(l);
        }
    }
    lines.join("\n\n")
}

/// The part of an article that best matches the question: paragraphs are grouped into chunks of
/// about 1200 characters, each chunk is scored against the question, and the best one is returned.
/// A long article is therefore not cut off at its introduction when the answer is further down.
pub fn best_chunk(title: &str, text: &str, question: &str) -> String {
    let q = retrieval::tokens(question);
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();
    for para in text.split("\n\n") {
        if !cur.is_empty() && cur.len() + para.len() > 1200 {
            chunks.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(para);
    }
    if !cur.is_empty() {
        chunks.push(cur);
    }
    let mut best = (f64::MIN, 0usize);
    for (i, c) in chunks.iter().enumerate() {
        // the first chunk gets a small bonus: introductions are usually the right summary
        let sc = retrieval::score(&q, title, c) + if i == 0 { 0.5 } else { 0.0 };
        if sc > best.0 {
            best = (sc, i);
        }
    }
    let chosen = chunks.get(best.1).cloned().unwrap_or_default();
    chosen.chars().take(1500).collect()
}

/// Search the archives and return up to `limit` passages. Each is the best-matching part of its article.
pub fn search(port: u16, question: &str, limit: usize) -> Vec<Passage> {
    let path = format!("/search?pattern={}&pageLength={}&format=xml", url_encode(question), limit);
    let xml = match get(port, &path) {
        Ok((200, body)) => String::from_utf8_lossy(&body).to_string(),
        _ => return vec![],
    };
    let mut passages = Vec::new();
    for (rank, (title, link)) in parse_search(&xml).into_iter().take(limit).enumerate() {
        let link = if link.starts_with('/') { link } else { format!("/{}", link) };
        if let Ok((200, body)) = get(port, &link) {
            let text = html_to_text(&String::from_utf8_lossy(&body));
            if text.len() < 80 {
                continue;
            }
            let text = best_chunk(&title, &text, question);
            // /content/<book>/A/<article>
            let book = link.trim_start_matches("/content/").split('/').next().unwrap_or("library").replace('_', " ");
            passages.push(Passage { source: book, heading: title, text, score: 6.0 - rank as f64 });
        }
    }
    passages
}

// ---------- the in-app reader: a safe window onto kiwix-serve ----------

/// What kiwix-serve answered, with the headers the reader needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub content_type: String,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

/// Biggest page or picture handed to the reader.
pub const MAX_BODY: u64 = 64 * 1024 * 1024;

fn parse_head(head: &str) -> Option<(u16, String, Option<String>)> {
    let mut lines = head.lines();
    let status: u16 = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let (mut ct, mut loc) = (String::new(), None);
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-type" => ct = v.trim().to_string(),
                "location" => loc = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    Some((status, ct, loc))
}

/// Like `get`, but keeps the content type and any redirect, and caps the size.
pub fn get_full(port: u16, path: &str) -> Result<Reply, String> {
    let addr = ("127.0.0.1", port).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(20))).ok();
    let req = format!("GET {} HTTP/1.0\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n", path);
    s.write_all(req.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    s.take(MAX_BODY + 65536).read_to_end(&mut raw).map_err(|e| e.to_string())?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("bad response")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_string();
    let (status, content_type, location) = parse_head(&head).ok_or("bad status")?;
    let body = raw[split + 4..].to_vec();
    if body.len() as u64 > MAX_BODY {
        return Err("that file is too large to show here".into());
    }
    Ok(Reply { status, content_type, location, body })
}

/// Only these parts of the library server may be reached from the reader, and only with plain
/// printable characters (so a request line cannot be split or smuggled).
pub fn proxy_path_ok(path: &str) -> bool {
    const ALLOWED: [&str; 5] = ["/content/", "/skin/", "/suggest?", "/search?", "/catalog/v2/"];
    path.len() <= 2048
        && path.bytes().all(|b| (0x21..=0x7e).contains(&b))
        && !path.contains("/../")
        && !path.contains("%2e%2e")
        && !path.contains("%2E%2E")
        && ALLOWED.iter().any(|p| path.starts_with(p))
}

/// Fetch a path, following up to four redirects that stay on this server. Returns the reply and
/// the path it finally came from (kiwix-serve redirects a book to its main page).
pub fn follow(port: u16, path: &str) -> Result<(Reply, String), String> {
    let mut cur = path.to_string();
    for _ in 0..5 {
        if !proxy_path_ok(&cur) {
            return Err("that address is not part of the library".into());
        }
        let r = get_full(port, &cur)?;
        if (300..400).contains(&r.status) {
            let Some(loc) = r.location else { return Ok((Reply { location: None, ..r }, cur)) };
            let own = format!("http://127.0.0.1:{}", port);
            let next = if let Some(rest) = loc.strip_prefix(&own) { rest.to_string() } else { loc };
            if next.starts_with('/') {
                cur = next;
                continue;
            }
            return Err("the library tried to send you to another site".into());
        }
        return Ok((r, cur));
    }
    Err("too many redirects".into())
}

/// One archive known to kiwix-serve.
#[derive(Debug, Clone, PartialEq)]
pub struct Book {
    /// The name used in addresses: `/content/<name>/...`
    pub name: String,
    pub title: String,
}

/// Read kiwix-serve's catalog (`/catalog/v2/entries`) for the books it serves.
pub fn parse_catalog(xml: &str) -> Vec<Book> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<entry>") {
        let after = &rest[i + 7..];
        let end = after.find("</entry>").unwrap_or(after.len());
        let e = &after[..end];
        // the address in the text/html link is the dependable source of the name
        let mut name = None;
        let mut scan = e;
        while let Some(l) = scan.find("<link ") {
            let tag_end = scan[l..].find('>').map(|x| l + x).unwrap_or(scan.len());
            let tag = &scan[l..tag_end];
            if tag.contains("text/html") {
                if let Some(h) = between(tag, "href=\"", "\"") {
                    if let Some(n) = h.strip_prefix("/content/") {
                        name = Some(n.trim_end_matches('/').split('/').next().unwrap_or("").to_string());
                        break;
                    }
                }
            }
            scan = &scan[tag_end.min(scan.len())..];
        }
        let name = name.or_else(|| between(e, "<name>", "</name>").map(|n| n.trim().to_string()));
        if let Some(name) = name.filter(|n| !n.is_empty()) {
            let title = between(e, "<title>", "</title>").map(|t| decode_entities(t).trim().to_string()).filter(|t| !t.is_empty()).unwrap_or_else(|| name.replace('_', " "));
            out.push(Book { name, title });
        }
        rest = &after[end..];
    }
    out
}

/// The small script put into every page shown in the reader. It tells the reader which page
/// is open (for Back, Forward and bookmarks), keeps links that leave the library from opening,
/// and handles the text-size setting.
pub const READER_SCRIPT: &str = "<script>(function(){\
var O=location.origin;\
function tell(){try{parent.postMessage({cx:'page',url:document.baseURI,title:document.title||''},'*')}catch(e){}}\
document.addEventListener('click',function(e){var a=e.target&&e.target.closest?e.target.closest('a[href]'):null;if(!a)return;var h=a.href;\
if(/^(https?|ftp|mailto|tel):/i.test(h)&&h.indexOf(O)!==0){e.preventDefault();try{parent.postMessage({cx:'external',url:h},'*')}catch(x){}}},true);\
addEventListener('message',function(e){var d=e.data;if(d&&d.cx==='zoom'){document.documentElement.style.zoom=d.v}});\
if(document.readyState==='complete')tell();else addEventListener('load',tell);\
})();</script>";

/// Put a snippet into an HTML page, right after `<head>` (or at the very start).
pub fn inject_head(html: &str, snippet: &str) -> String {
    let lower = html.to_ascii_lowercase();
    if let Some(i) = lower.find("<head") {
        if let Some(j) = lower[i..].find('>') {
            let at = i + j + 1;
            return format!("{}{}{}", &html[..at], snippet, &html[at..]);
        }
    }
    format!("{}{}", snippet, html)
}

/// Put the reader script into an HTML page.
pub fn inject_reader_script(html: &str) -> String {
    inject_head(html, READER_SCRIPT)
}

/// A `<base>` tag, so a page that was reached through a redirect finds its pictures and links.
pub fn base_tag(url: &str) -> String {
    let safe: String = url.chars().map(|c| match c { '"' => "%22".to_string(), '<' => "%3C".to_string(), '>' => "%3E".to_string(), '&' => "&amp;".to_string(), c => c.to_string() }).collect();
    format!("<base href=\"{}\">", safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn parses_results() {
        let xml = "<rss><channel><item><title>Water &amp; health</title><link>/content/wiki/A/Water</link></item><item><title>B</title><link>/content/wiki/A/B</link></item></channel></rss>";
        let r = parse_search(xml);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0], ("Water & health".to_string(), "/content/wiki/A/Water".to_string()));
    }

    #[test]
    fn html_becomes_text() {
        let t = html_to_text("<html><head><style>p{}</style></head><body><h1>Title</h1><p>One <b>two</b> &amp; three</p><script>x()</script><p>Four</p></body></html>");
        assert_eq!(t, "Title\n\nOne two & three\n\nFour");
    }

    #[test]
    fn long_articles_give_the_matching_part_not_just_the_start() {
        let intro = "Water is a clear liquid that covers most of the earth and is used for many things every day. ".repeat(10);
        let answer = "To purify river water, filter it through cloth and then boil it for one minute so germs are killed. ".repeat(5);
        let filler = "Lakes and oceans have been studied by many scientists over the years in different ways. ".repeat(12);
        let article = format!("{}\n\n{}\n\n{}", intro, filler, answer);
        let got = best_chunk("Water", &article, "how do I purify river water by boiling");
        assert!(got.contains("boil it for one minute"), "{}", got);
        assert!(got.len() <= 1500);
    }

    #[test]
    fn encodes_urls() {
        assert_eq!(url_encode("boil water?"), "boil%20water%3F");
    }

    #[test]
    fn arguments_bind_only_to_this_computer() {
        let a = args(&[PathBuf::from("a.zim")], 8123, false);
        assert!(a.windows(2).any(|w| w[0] == "--address" && w[1] == "127.0.0.1"));
        assert_eq!(a.last().unwrap(), "a.zim");
    }

    #[test]
    fn sharing_is_only_on_when_asked_for() {
        let on = args(&[PathBuf::from("a.zim")], 8787, true);
        assert!(on.windows(2).any(|w| w[0] == "--address" && w[1] == "0.0.0.0"));
        let off = args(&[PathBuf::from("a.zim")], 8787, false);
        assert!(!off.iter().any(|x| x == "0.0.0.0"));
    }

    #[test]
    fn searches_a_fake_server() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.starts_with("GET /search") {
                    "<item><title>Water purification</title><link>/content/wiki_en/A/Water_purification</link></item>".to_string()
                } else {
                    format!("<html><body><p>{}</p></body></html>", "Boil water for one minute to make it safe to drink. ".repeat(4))
                };
                let _ = write!(s, "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
            }
        });
        let p = search(port, "how do I purify water", 3);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].heading, "Water purification");
        assert_eq!(p[0].source, "wiki en");
        assert!(p[0].text.contains("Boil water"));
    }

    #[test]
    fn head_lines_are_read() {
        let (st, ct, loc) = parse_head("HTTP/1.1 302 Found\r\nContent-Type: text/html; charset=utf-8\r\nLocation: /content/wiki/A/Main\r\nX: y").unwrap();
        assert_eq!((st, ct.as_str(), loc.as_deref()), (302, "text/html; charset=utf-8", Some("/content/wiki/A/Main")));
        assert!(parse_head("garbage").is_none());
    }

    #[test]
    fn the_reader_can_only_reach_the_library_parts_of_the_server() {
        for ok in ["/content/wiki/A/Water", "/skin/kiwix.css", "/suggest?content=wiki&term=water", "/search?content=wiki&pattern=x", "/catalog/v2/entries"] {
            assert!(proxy_path_ok(ok), "{}", ok);
        }
        for bad in ["", "/", "/admin", "/random", "/content/../etc", "/content/a%2e%2e/b", "/content/a b", "/content/a\r\nHost: x", "/content/a\nb", "http://evil/content/x", "//evil/content/", &format!("/content/{}", "a".repeat(3000))] {
            assert!(!proxy_path_ok(bad), "{:?}", bad);
        }
    }

    #[test]
    fn follows_redirects_on_this_server_only() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in l.incoming().flatten() {
                let mut s = stream;
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                let out = if line.starts_with("GET /content/book ") {
                    "HTTP/1.0 302 Found\r\nLocation: /content/book/A/Main\r\nContent-Length: 0\r\n\r\n".to_string()
                } else if line.starts_with("GET /content/away ") {
                    "HTTP/1.0 302 Found\r\nLocation: http://example.com/x\r\nContent-Length: 0\r\n\r\n".to_string()
                } else if line.starts_with("GET /content/loop ") {
                    "HTTP/1.0 302 Found\r\nLocation: /content/loop\r\nContent-Length: 0\r\n\r\n".to_string()
                } else {
                    let body = "<html><head><title>Main</title></head><body>hi</body></html>";
                    format!("HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                };
                let _ = s.write_all(out.as_bytes());
            }
        });
        let (r, final_path) = follow(port, "/content/book").unwrap();
        assert_eq!((r.status, final_path.as_str(), r.content_type.as_str()), (200, "/content/book/A/Main", "text/html"));
        assert!(String::from_utf8_lossy(&r.body).contains("hi"));
        assert!(follow(port, "/content/away").unwrap_err().contains("another site"));
        assert!(follow(port, "/content/loop").unwrap_err().contains("redirects"));
        assert!(follow(port, "/admin").is_err());
    }

    #[test]
    fn the_catalog_gives_book_names_and_titles() {
        let xml = r#"<feed><entry><title>Wikipedia &amp; more</title><name>wikipedia_en_x</name><link rel="alternate" type="text/html" href="/content/wikipedia_en_x_2025-01"/><link type="application/x-zim" href="/x.zim"/></entry><entry><title></title><name>fallback_book</name></entry></feed>"#;
        let b = parse_catalog(xml);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].name.as_str(), b[0].title.as_str()), ("wikipedia_en_x_2025-01", "Wikipedia & more"));
        assert_eq!((b[1].name.as_str(), b[1].title.as_str()), ("fallback_book", "fallback book"));
        assert!(parse_catalog("<feed></feed>").is_empty());
    }

    #[test]
    fn the_reader_script_goes_into_the_head() {
        let h = inject_reader_script("<!DOCTYPE html><HTML><HEAD lang=x><title>T</title></HEAD><body>b</body></HTML>");
        let at = h.find("<script>").unwrap();
        assert!(h[..at].ends_with("<HEAD lang=x>"));
        assert!(h.contains("<title>T</title>"));
        assert!(inject_reader_script("plain").starts_with("<script>"));
        assert_eq!(base_tag("http://h/a\"><script>"), "<base href=\"http://h/a%22%3E%3Cscript%3E\">");
        let both = inject_head(&inject_head("<head></head>", READER_SCRIPT), &base_tag("http://h/x/"));
        assert!(both.starts_with("<head><base"));
        // multi-byte text before the head cannot split a character
        assert!(inject_reader_script("<html>é<head></head>").contains("<script>"));
    }
}
