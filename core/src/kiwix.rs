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
}
