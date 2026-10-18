//! Talks to a local llama-server (from llama.cpp) over plain HTTP on 127.0.0.1.
//! Standard library only: a small HTTP/1.1 client that understands chunked replies and
//! the "data: {...}" lines of a streamed chat completion.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Escape text for use inside a JSON string.
pub fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Read a JSON string value that starts right after an opening quote. Returns (text, bytes used).
fn read_json_string(s: &str) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut it = s.char_indices();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Some((out, i + 1)),
            '\\' => {
                let (_, e) = it.next()?;
                match e {
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'b' => out.push('\u{8}'),
                    'f' => out.push('\u{c}'),
                    '/' => out.push('/'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'u' => {
                        let mut hex = String::new();
                        for _ in 0..4 {
                            hex.push(it.next()?.1);
                        }
                        let hi = u32::from_str_radix(&hex, 16).ok()?;
                        if (0xD800..0xDC00).contains(&hi) {
                            // a surrogate pair: 😀
                            let (_, b1) = it.next()?;
                            let (_, b2) = it.next()?;
                            if b1 != '\\' || b2 != 'u' {
                                return None;
                            }
                            let mut hex2 = String::new();
                            for _ in 0..4 {
                                hex2.push(it.next()?.1);
                            }
                            let lo = u32::from_str_radix(&hex2, 16).ok()?;
                            let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo.checked_sub(0xDC00)?);
                            out.push(char::from_u32(cp)?);
                        } else {
                            out.push(char::from_u32(hi)?);
                        }
                    }
                    other => out.push(other),
                }
            }
            c => out.push(c),
        }
    }
    None
}

/// The text of `"content":"..."` inside the `delta` of one streamed event. None if there is none.
pub fn delta_content(event_json: &str) -> Option<String> {
    let d = event_json.find("\"delta\"")?;
    let rest = &event_json[d..];
    let k = rest.find("\"content\"")?;
    let after = rest[k + "\"content\"".len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    let after = after.strip_prefix('"')?;
    read_json_string(after).map(|x| x.0)
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl ChatRequest {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"model\":\"cindrix\",\"stream\":true,\"temperature\":{},\"max_tokens\":{},\
\"chat_template_kwargs\":{{\"enable_thinking\":false}},\
\"messages\":[{{\"role\":\"system\",\"content\":\"{}\"}},{{\"role\":\"user\",\"content\":\"{}\"}}]}}",
            self.temperature, self.max_tokens, json_escape(&self.system), json_escape(&self.user)
        )
    }
}

fn connect(port: u16, timeout: Duration) -> Result<TcpStream, String> {
    let s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("cannot reach the model: {}", e))?;
    s.set_read_timeout(Some(timeout)).ok();
    s.set_write_timeout(Some(Duration::from_secs(30))).ok();
    Ok(s)
}

/// Read the status line and headers. Returns (status code, chunked?, content length).
fn read_head<R: BufRead>(r: &mut R) -> Result<(u16, bool, Option<usize>), String> {
    let mut line = String::new();
    r.read_line(&mut line).map_err(|e| e.to_string())?;
    let status: u16 = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("bad reply from the model")?;
    let mut chunked = false;
    let mut len = None;
    loop {
        let mut h = String::new();
        let n = r.read_line(&mut h).map_err(|e| e.to_string())?;
        if n == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        let low = h.to_lowercase();
        if low.starts_with("transfer-encoding:") && low.contains("chunked") {
            chunked = true;
        }
        if let Some(v) = low.strip_prefix("content-length:") {
            len = v.trim().parse().ok();
        }
    }
    Ok((status, chunked, len))
}

/// Feeds decoded body bytes to `sink` as they arrive.
fn read_body<R: BufRead>(r: &mut R, chunked: bool, len: Option<usize>, mut sink: impl FnMut(&[u8]) -> bool) -> Result<(), String> {
    if chunked {
        loop {
            let mut size_line = String::new();
            if r.read_line(&mut size_line).map_err(|e| e.to_string())? == 0 {
                return Ok(());
            }
            let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16).map_err(|_| "bad chunk size".to_string())?;
            if size == 0 {
                return Ok(());
            }
            let mut buf = vec![0u8; size];
            r.read_exact(&mut buf).map_err(|e| e.to_string())?;
            let mut crlf = [0u8; 2];
            let _ = r.read_exact(&mut crlf);
            if !sink(&buf) {
                return Ok(());
            }
        }
    } else {
        let mut remaining = len;
        let mut buf = [0u8; 4096];
        loop {
            let want = remaining.map(|n| n.min(buf.len())).unwrap_or(buf.len());
            if want == 0 {
                return Ok(());
            }
            let n = r.read(&mut buf[..want]).map_err(|e| e.to_string())?;
            if n == 0 {
                return Ok(());
            }
            if let Some(rem) = remaining.as_mut() {
                *rem -= n;
            }
            if !sink(&buf[..n]) {
                return Ok(());
            }
        }
    }
}

/// Is the model loaded and ready? (GET /health is 200 when ready, 503 while loading.)
pub fn health(port: u16) -> bool {
    let mut s = match connect(port, Duration::from_secs(3)) {
        Ok(s) => s,
        Err(_) => return false,
    };
    if s.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").is_err() {
        return false;
    }
    let mut r = BufReader::new(s);
    matches!(read_head(&mut r), Ok((200, _, _)))
}

/// Ask the model and call `on_token` for every piece of text as it arrives.
/// `keep_going` is checked between pieces; return false to stop early.
/// Returns the whole text.
pub fn chat_stream(
    port: u16,
    req: &ChatRequest,
    mut on_token: impl FnMut(&str),
    mut keep_going: impl FnMut() -> bool,
) -> Result<String, String> {
    let body = req.to_json();
    let mut s = connect(port, Duration::from_secs(600))?;
    let head = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nAccept: text/event-stream\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    s.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    let mut r = BufReader::new(s);
    let (status, chunked, len) = read_head(&mut r)?;
    if status != 200 {
        let mut msg = Vec::new();
        let _ = read_body(&mut r, chunked, len, |b| {
            msg.extend_from_slice(b);
            msg.len() < 2000
        });
        return Err(format!("the model answered with an error ({}): {}", status, String::from_utf8_lossy(&msg)));
    }

    let mut pending = String::new(); // text not yet split into lines
    let mut raw: Vec<u8> = Vec::new(); // bytes of a partly received UTF-8 character
    let mut all = String::new();
    let mut done = false;
    read_body(&mut r, chunked, len, |bytes| {
        raw.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&raw) {
            Ok(_) => raw.len(),
            Err(e) => e.valid_up_to(),
        };
        pending.push_str(std::str::from_utf8(&raw[..valid]).unwrap_or(""));
        raw.drain(..valid);
        while let Some(nl) = pending.find('\n') {
            let line: String = pending.drain(..=nl).collect();
            let line = line.trim();
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                if data == "[DONE]" {
                    done = true;
                    return false;
                }
                if let Some(t) = delta_content(data) {
                    if !t.is_empty() {
                        all.push_str(&t);
                        on_token(&t);
                    }
                }
            }
            if !keep_going() {
                done = true;
                return false;
            }
        }
        true
    })?;
    let _ = done;
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Read;
    use std::net::TcpListener;
    use std::thread;

    /// A fake llama-server: sends `parts` as separate chunks of a chunked reply.
    fn fake_server(status: u16, parts: Vec<&'static str>) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                // read the request head and body
                let mut buf = Vec::new();
                let mut tmp = [0u8; 1024];
                loop {
                    let n = s.read(&mut tmp).unwrap_or(0);
                    buf.extend_from_slice(&tmp[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(h) = text.find("\r\n\r\n") {
                        let cl: usize = text.to_lowercase().split("content-length:").nth(1).and_then(|x| x.split("\r\n").next()).and_then(|x| x.trim().parse().ok()).unwrap_or(0);
                        if buf.len() >= h + 4 + cl {
                            break;
                        }
                    }
                    if n == 0 {
                        break;
                    }
                }
                let _ = s.write_all(format!("HTTP/1.1 {} X\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n", status).as_bytes());
                for p in parts {
                    let _ = s.write_all(format!("{:x}\r\n", p.len()).as_bytes());
                    let _ = s.write_all(p.as_bytes());
                    let _ = s.write_all(b"\r\n");
                    let _ = s.flush();
                    thread::sleep(Duration::from_millis(5));
                }
                let _ = s.write_all(b"0\r\n\r\n");
            }
        });
        port
    }

    fn req() -> ChatRequest {
        ChatRequest { system: "s".into(), user: "How long do I boil water?".into(), max_tokens: 64, temperature: 0.2 }
    }

    #[test]
    fn escapes_json() {
        assert_eq!(json_escape("a\"b\\c\nd"), "a\\\"b\\\\c\\nd");
    }

    #[test]
    fn request_has_thinking_off_and_both_messages() {
        let j = req().to_json();
        assert!(j.contains("\"enable_thinking\":false"));
        assert!(j.contains("\"role\":\"system\""));
        assert!(j.contains("How long do I boil water?"));
    }

    #[test]
    fn reads_delta_text_with_escapes() {
        assert_eq!(delta_content(r#"{"choices":[{"delta":{"content":"say \"hi\"\nnow"}}]}"#).as_deref(), Some("say \"hi\"\nnow"));
        assert_eq!(delta_content(r#"{"choices":[{"delta":{"content":"😀 ok"}}]}"#).as_deref(), Some("😀 ok"));
        assert_eq!(delta_content(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#), None);
        assert_eq!(delta_content(r#"{"choices":[{"delta":{"content":null}}]}"#), None);
    }

    #[test]
    fn streams_tokens_even_when_lines_are_split_across_chunks() {
        let port = fake_server(200, vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"Boil\"}}]}\n\ndata: {\"choices\":[{\"del",
            "ta\":{\"content\":\" for\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\" 1 minute \u{e9}\"}}]}\n\ndata: [DONE]\n\n",
        ]);
        let mut tokens = Vec::new();
        let all = chat_stream(port, &req(), |t| tokens.push(t.to_string()), || true).unwrap();
        assert_eq!(tokens, vec!["Boil", " for", " 1 minute \u{e9}"]);
        assert_eq!(all, "Boil for 1 minute \u{e9}");
    }

    #[test]
    fn error_status_is_an_error() {
        let port = fake_server(503, vec!["{\"error\":\"loading\"}"]);
        let e = chat_stream(port, &req(), |_| {}, || true).unwrap_err();
        assert!(e.contains("503"));
    }

    #[test]
    fn unreachable_model_is_an_error() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        assert!(chat_stream(port, &req(), |_| {}, || true).is_err());
        assert!(!health(port));
    }

    #[test]
    fn health_is_true_on_200() {
        let port = fake_server(200, vec!["{}"]);
        assert!(health(port));
    }

    #[test]
    fn can_stop_early() {
        let port = fake_server(200, vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\ndata: [DONE]\n\n",
        ]);
        let n = Cell::new(0);
        let _ = chat_stream(port, &req(), |_| n.set(n.get() + 1), || n.get() < 1);
        assert_eq!(n.get(), 1);
    }
}

/// llama-server behind the pipeline's `Model` trait.
pub struct LlamaModel {
    pub port: u16,
    pub max_tokens: u32,
}

impl crate::pipeline::Model for LlamaModel {
    fn complete(&self, system: &str, user: &str, on_token: &mut dyn FnMut(&str) -> bool) -> Result<String, String> {
        let req = ChatRequest { system: system.to_string(), user: user.to_string(), max_tokens: self.max_tokens, temperature: 0.2 };
        let stop = std::cell::Cell::new(false);
        chat_stream(
            self.port,
            &req,
            |t| {
                if !on_token(t) {
                    stop.set(true);
                }
            },
            || !stop.get(),
        )
    }
}

#[cfg(test)]
mod end_to_end {
    use super::*;
    use crate::pipeline::{answer_stream, Event, Kind};
    use std::io::Read;
    use std::net::TcpListener;
    use std::path::PathBuf;

    fn serve(tokens: &'static [&'static str]) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut buf = vec![0u8; 8192];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n");
                for t in tokens {
                    let ev = format!("data: {{\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n", t);
                    let _ = s.write_all(format!("{:x}\r\n{}\r\n", ev.len(), ev).as_bytes());
                }
                let done = "data: [DONE]\n\n";
                let _ = s.write_all(format!("{:x}\r\n{}\r\n0\r\n\r\n", done.len(), done).as_bytes());
            }
        });
        port
    }

    fn lib() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../content/library-sample")
    }

    #[test]
    fn a_real_http_stream_with_a_dose_never_reaches_the_screen() {
        let port = serve(&["Take", " 12", " units", " of", " insulin", " now", "."]);
        let model = LlamaModel { port, max_tokens: 64 };
        let mut events = Vec::new();
        let r = answer_stream("How long do I boil water?", &lib(), &model, &mut |e| events.push(e));
        assert_eq!(r.kind, Kind::Replaced);
        for e in &events {
            if let Event::Text(t) = e {
                assert!(!t.contains("12"), "{:?}", events);
            }
        }
    }

    #[test]
    fn a_real_http_stream_with_a_good_answer_arrives_whole() {
        let port = serve(&["Boil", " it", " for", " 1", " minute", "."]);
        let model = LlamaModel { port, max_tokens: 64 };
        let r = answer_stream("How long do I boil water?", &lib(), &model, &mut |_| {});
        assert_eq!(r.kind, Kind::Answered);
        assert_eq!(r.text, "Boil it for 1 minute.");
    }
}
