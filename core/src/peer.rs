//! The link from this device to the other one it keeps in step with (a Vault laptop and a Hub phone).
//!
//! The other device runs the share server (doc 34). This device joins it once with an invite, like a trusted phone
//! does, and keeps the sign-in it was given. From then on it asks the other device for its gear sheet, inventory
//! and security logs and sends its own (`peersync.rs`).
//!
//! Only a device on the same local network is ever called: the address must be a private one (192.168.x.x, 10.x.x.x,
//! 172.16 to 31.x.x, 169.254.x.x) or this very computer. Nothing here can reach the internet. Plain HTTP, standard
//! library only. The sign-in is kept in `user/security/peer.txt`, so the passcode lock covers it.

use crate::fsio;
use crate::peersync::Remote;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MAX_REPLY: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// `http://192.168.43.1:8080`
    pub base: String,
    pub cookie: String,
    /// The name this device gave itself when it joined.
    pub name: String,
}

pub fn path(user_root: &Path) -> PathBuf {
    crate::security_store::dir(user_root).join("peer.txt")
}

pub fn load(user_root: &Path) -> Result<Option<Link>, String> {
    match fsio::read_to_string(&path(user_root)) {
        Ok(t) => {
            let l = t.lines().find(|l| !l.starts_with('#') && !l.trim().is_empty()).ok_or("The link to the other device could not be read.")?;
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != 3 || split_base(f[0]).is_err() {
                return Err("The link to the other device could not be read.".to_string());
            }
            Ok(Some(Link { base: f[0].into(), cookie: f[1].into(), name: f[2].into() }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("The link to the other device could not be read.".to_string()),
    }
}

pub fn save(user_root: &Path, l: &Link) -> Result<(), String> {
    let text = format!("# format: peer-link 1\n{}\t{}\t{}\n", l.base, l.cookie, l.name.replace(['\t', '\n'], " "));
    fsio::write_atomic(&path(user_root), text.as_bytes()).map_err(|e| format!("The link could not be saved: {}", e))
}

pub fn unlink(user_root: &Path) -> Result<(), String> {
    match std::fs::remove_file(path(user_root)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("The link could not be removed.".to_string()),
    }
}

/// True for an address on this network or this computer. Hostnames other than `localhost` are refused.
pub fn local_only(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let p: Vec<u8> = match host.split('.').map(|x| x.parse::<u8>()).collect::<Result<Vec<_>, _>>() {
        Ok(v) if v.len() == 4 => v,
        _ => return false,
    };
    p[0] == 10 || p[0] == 127 || (p[0] == 192 && p[1] == 168) || (p[0] == 172 && (16..=31).contains(&p[1])) || (p[0] == 169 && p[1] == 254)
}

/// `(host, port, host:port)` of an `http://` address that is on this network.
pub fn split_base(base: &str) -> Result<(String, u16, String), String> {
    let rest = base.strip_prefix("http://").ok_or("The address must start with http:// (the other device is reached on this network only).")?;
    let authority = rest.split('/').next().unwrap_or("");
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().map_err(|_| "The address has no usable port number.".to_string())?),
        None => (authority, 80),
    };
    if host.is_empty() {
        return Err("The address is empty.".to_string());
    }
    if !local_only(host) {
        return Err("That address is not on this network. The two devices must be on the same Wi-Fi or hotspot.".to_string());
    }
    Ok((host.to_string(), port, format!("{}:{}", host, port)))
}

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Reply {
    fn header(&self, k: &str) -> Option<&str> {
        self.headers.iter().find(|(a, _)| a.eq_ignore_ascii_case(k)).map(|(_, v)| v.as_str())
    }
}

fn http(base: &str, method: &str, target: &str, headers: &[(&str, String)], body: &[u8]) -> Result<Reply, String> {
    let (_, _, hostport) = split_base(base)?;
    let addr = hostport.to_socket_addrs().map_err(|_| "The other device could not be found on this network.".to_string())?.next().ok_or("The other device could not be found on this network.")?;
    let mut s = TcpStream::connect_timeout(&addr, Duration::from_secs(6)).map_err(|_| "The other device did not answer. Check that it is on, that sharing is on there, and that both are on the same network.".to_string())?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(60)));
    let mut req = format!("{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Length: {}\r\n", method, target, hostport, body.len());
    for (k, v) in headers {
        req.push_str(&format!("{}: {}\r\n", k, v.replace(['\r', '\n'], " ")));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).and_then(|_| s.write_all(body)).map_err(|_| "The other device stopped answering.".to_string())?;
    let mut all = Vec::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                all.extend_from_slice(&buf[..n]);
                if all.len() > MAX_REPLY {
                    return Err("The other device sent far more than expected.".to_string());
                }
            }
            Err(_) => break,
        }
    }
    let at = all.windows(4).position(|w| w == b"\r\n\r\n").ok_or("The other device sent something that was not a web reply.")?;
    let head = String::from_utf8_lossy(&all[..at]).to_string();
    let mut lines = head.lines();
    let status: u16 = lines.next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse().ok()).ok_or("The other device sent something that was not a web reply.")?;
    let headers = lines.filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
    let mut body = all[at + 4..].to_vec();
    let r = Reply { status, headers, body: Vec::new() };
    if let Some(n) = r.header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        body.truncate(n);
    }
    Ok(Reply { body, ..r })
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}

/// Join the other device with the address its invite shows (`http://192.168.43.1:8080/join?c=...`). The name is how
/// this device is called on the other one.
pub fn join(invite_url: &str, name: &str) -> Result<Link, String> {
    let url = invite_url.trim();
    let (base, rest) = match url.find("/join") {
        Some(i) => (&url[..i], &url[i..]),
        None => return Err("That is not an invite address. It should look like http://192.168.43.1:8080/join?c=...".to_string()),
    };
    split_base(base)?;
    let code = rest.split(['?', '&']).find_map(|kv| kv.strip_prefix("c=")).filter(|c| !c.is_empty()).ok_or("The invite address has no code in it.")?;
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > 40 {
        return Err("Give this device a short name, like Laptop.".to_string());
    }
    let r = http(base, "GET", &format!("/join?c={}&name={}", enc(code), enc(&name)), &[], &[])?;
    let cookie = r.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie")).find_map(|(_, v)| v.strip_prefix("cx=").map(|t| t.split(';').next().unwrap_or("").to_string())).filter(|t| !t.is_empty());
    match (r.status, cookie) {
        (302, Some(c)) => Ok(Link { base: base.to_string(), cookie: c, name }),
        _ => Err("The other device did not accept the invite. It may have run out (they last ten minutes) or been used. Make a new one there.".to_string()),
    }
}

/// The other device, reached over the network with the sign-in this device was given.
pub struct Http(pub Link);

impl Http {
    fn call(&self, what: &str, kind: &str, body: &str) -> Result<String, String> {
        let r = http(&self.0.base, "POST", &format!("/api/{}", what), &[("Cookie", format!("cx={}", self.0.cookie)), ("X-Kind", kind.to_string())], body.as_bytes())?;
        let text = String::from_utf8_lossy(&r.body).to_string();
        match r.status {
            200 => Ok(text),
            401 | 403 => Err(serde_error(&text).unwrap_or_else(|| "The other device does not allow this device to copy. Its role needs the copy permission.".to_string())),
            503 => Err(serde_error(&text).unwrap_or_else(|| "The other device is locked.".to_string())),
            _ => Err(serde_error(&text).unwrap_or_else(|| format!("The other device answered with an error ({}).", r.status))),
        }
    }
}

/// The `"error"` text in a reply like `{"error":"..."}`, without a JSON library.
fn serde_error(body: &str) -> Option<String> {
    let i = body.find("\"error\"")?;
    let rest = &body[i + 7..];
    let start = rest.find('"')? + 1;
    let mut out = String::new();
    let mut chars = rest[start..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(match n { 'n' => ' ', other => other });
                }
            }
            _ => out.push(c),
        }
    }
    None
}

impl Remote for Http {
    fn get(&self, kind: &str) -> Result<String, String> {
        self.call("peer_get", kind, "")
    }
    fn put(&self, kind: &str, body: &str) -> Result<String, String> {
        self.call("peer_put", kind, body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A one-shot server that reads one request and sends back this reply; returns the port and the request it saw.
    fn serve(reply: &'static str) -> (u16, std::thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = s.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(i) = text.find("\r\n\r\n") {
                    let want: usize = text.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
                    if got.len() >= i + 4 + want {
                        break;
                    }
                }
            }
            s.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&got).to_string()
        });
        (port, h)
    }

    #[test]
    fn only_addresses_on_this_network_are_ever_called() {
        for ok in ["192.168.43.1", "10.0.0.5", "172.16.0.1", "172.31.255.1", "169.254.1.1", "127.0.0.1", "localhost"] {
            assert!(local_only(ok), "{}", ok);
        }
        for bad in ["8.8.8.8", "172.32.0.1", "172.15.0.1", "example.com", "192.169.0.1", "1.2.3", "", "192.168.1.1.evil.com"] {
            assert!(!local_only(bad), "{}", bad);
        }
        assert!(split_base("https://192.168.1.2:8080").is_err());
        assert!(split_base("http://example.com").is_err());
        assert!(join("http://example.com/join?c=abc", "Laptop").is_err());
        assert_eq!(split_base("http://192.168.1.2:8080").unwrap().1, 8080);
    }

    #[test]
    fn joining_keeps_the_sign_in_and_the_link_round_trips() {
        let (port, h) = serve("HTTP/1.1 302 Found\r\nLocation: /\r\nSet-Cookie: cx=tok123; Path=/; HttpOnly\r\nContent-Length: 0\r\n\r\n");
        let l = join(&format!("http://127.0.0.1:{}/join?c=ABC", port), "My Laptop").unwrap();
        assert_eq!((l.cookie.as_str(), l.name.as_str()), ("tok123", "My Laptop"));
        let seen = h.join().unwrap();
        assert!(seen.starts_with("GET /join?c=ABC&name=My%20Laptop HTTP/1.1"), "{}", seen);
        let r = std::env::temp_dir().join(format!("cindrix-peerlink-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&r);
        assert_eq!(load(&r).unwrap(), None);
        save(&r, &l).unwrap();
        assert_eq!(load(&r).unwrap(), Some(l));
        unlink(&r).unwrap();
        assert_eq!(load(&r).unwrap(), None);
        std::fs::create_dir_all(crate::security_store::dir(&r)).unwrap();
        std::fs::write(path(&r), "# format: peer-link 1\nhttp://8.8.8.8:1\tx\ty\n").unwrap();
        assert!(load(&r).is_err(), "a link to an address off this network is refused when read");
    }

    #[test]
    fn a_refused_invite_and_a_dead_address_say_so_in_plain_words() {
        let (port, _h) = serve("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nno");
        let e = join(&format!("http://127.0.0.1:{}/join?c=ABC", port), "Laptop").unwrap_err();
        assert!(e.contains("did not accept"), "{}", e);
        assert!(join("http://127.0.0.1:1/join?c=ABC", "Laptop").unwrap_err().contains("did not answer"));
        assert!(join("http://127.0.0.1:9/nothing", "Laptop").is_err());
        assert!(join("http://127.0.0.1:9/join?c=ABC", "  ").is_err());
    }

    #[test]
    fn asking_and_sending_carry_the_sign_in_and_the_kind() {
        let (port, h) = serve("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
        let remote = Http(Link { base: format!("http://127.0.0.1:{}", port), cookie: "tok".into(), name: "L".into() });
        assert_eq!(remote.put("log:comms-log.txt", "BODY").unwrap(), "hello");
        let seen = h.join().unwrap();
        assert!(seen.starts_with("POST /api/peer_put"), "{}", seen);
        assert!(seen.contains("Cookie: cx=tok") && seen.contains("X-Kind: log:comms-log.txt") && seen.ends_with("BODY"), "{}", seen);
        let (port, _h) = serve("HTTP/1.1 403 Forbidden\r\nContent-Length: 40\r\n\r\n{\"error\":\"The role does not allow this.\"}");
        let remote = Http(Link { base: format!("http://127.0.0.1:{}", port), cookie: "tok".into(), name: "L".into() });
        assert_eq!(remote.get("gear").unwrap_err(), "The role does not allow this.");
    }
}
