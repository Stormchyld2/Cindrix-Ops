//! Finding out whether this computer or phone is running a Wi-Fi hotspot, and what address other devices
//! would use to reach it. Reads the output of the system's own network listing (`ipconfig` on Windows,
//! `ip` elsewhere); nothing goes out to the internet.

#[derive(Debug, Clone, PartialEq)]
pub struct Adapter {
    pub name: String,
    pub ip: String,
}

/// Adapters and their IPv4 addresses from Windows `ipconfig` output.
pub fn parse_ipconfig(text: &str) -> Vec<Adapter> {
    let mut out = Vec::new();
    let mut name = String::new();
    for line in text.lines() {
        let l = line.trim_end();
        if l.is_empty() {
            continue;
        }
        if !l.starts_with(' ') && !l.starts_with('\t') {
            if l.ends_with(':') {
                let n = l.trim_end_matches(':');
                // "Wireless LAN adapter Local Area Connection* 10" -> "Local Area Connection* 10"
                name = match n.to_lowercase().find(" adapter ") {
                    Some(i) => n[i + " adapter ".len()..].trim().to_string(),
                    None => n.trim().to_string(),
                };
            }
            continue;
        }
        if l.contains("IPv4") {
            if let Some(i) = l.rfind(':') {
                let ip = l[i + 1..].trim().trim_end_matches("(Preferred)").trim();
                if ip.split('.').count() == 4 && ip.split('.').all(|p| p.parse::<u8>().is_ok()) {
                    out.push(Adapter { name: name.clone(), ip: ip.to_string() });
                }
            }
        }
    }
    out
}

/// Adapters and addresses from `ip -o -4 addr show` output (Linux, Android).
pub fn parse_ip_addr(text: &str) -> Vec<Adapter> {
    let mut out = Vec::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(k) = words.iter().position(|w| *w == "inet") else { continue };
        let Some(addr) = words.get(k + 1) else { continue };
        let ip = addr.split('/').next().unwrap_or("");
        // "3: wlan0 inet ..." -> the name is the second word, without a colon
        let name = words.get(1).map(|w| w.trim_end_matches(':')).unwrap_or("");
        if !ip.is_empty() && !name.is_empty() {
            out.push(Adapter { name: name.to_string(), ip: ip.to_string() });
        }
    }
    out
}

fn usable(ip: &str) -> bool {
    !(ip.starts_with("127.") || ip.starts_with("169.254.") || ip == "0.0.0.0" || ip.is_empty())
}

/// Is this adapter the one a hotspot hands out addresses on?
/// - Windows Mobile Hotspot uses 192.168.137.x, on a "Local Area Connection*" adapter.
/// - Android uses 192.168.43.x (older phones) or an access-point interface (`ap0`, `swlan0`, `wlan1`).
pub fn is_hotspot(a: &Adapter) -> bool {
    if !usable(&a.ip) {
        return false;
    }
    let n = a.name.to_lowercase();
    a.ip.starts_with("192.168.137.")
        || a.ip.starts_with("192.168.43.")
        || n.starts_with("local area connection*")
        || n.contains("hotspot")
        || n == "ap0"
        || n == "swlan0"
        || n == "wlan1"
        || n == "softap0"
}

/// The hotspot's adapter, if one is up.
pub fn find_hotspot(list: &[Adapter]) -> Option<Adapter> {
    list.iter().find(|a| is_hotspot(a)).cloned()
}

/// Every address other devices on the same network could try (not this device's own loopback).
pub fn reachable_addresses(list: &[Adapter]) -> Vec<Adapter> {
    list.iter().filter(|a| usable(&a.ip)).cloned().collect()
}

/// Ask the system for its adapters. Returns an empty list when the listing cannot be run.
pub fn detect() -> Vec<Adapter> {
    use std::process::Command;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        return match Command::new("ipconfig").creation_flags(NO_WINDOW).output() {
            Ok(o) => parse_ipconfig(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => vec![],
        };
    }
    #[cfg(not(windows))]
    {
        match Command::new("ip").args(["-o", "-4", "addr", "show"]).output() {
            Ok(o) => parse_ip_addr(&String::from_utf8_lossy(&o.stdout)),
            Err(_) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIN: &str = "Windows IP Configuration\r\n\r\nEthernet adapter Ethernet:\r\n\r\n   Media State . . . . . . . . . . . : Media disconnected\r\n\r\nWireless LAN adapter Local Area Connection* 10:\r\n\r\n   Connection-specific DNS Suffix  . :\r\n   Link-local IPv6 Address . . . . . : fe80::1%12\r\n   IPv4 Address. . . . . . . . . . . : 192.168.137.1\r\n   Subnet Mask . . . . . . . . . . . : 255.255.255.0\r\n\r\nWireless LAN adapter Wi-Fi:\r\n\r\n   IPv4 Address. . . . . . . . . . . : 192.168.1.20\r\n   Default Gateway . . . . . . . . . : 192.168.1.1\r\n";

    #[test]
    fn windows_listing_finds_the_hotspot_and_the_normal_wifi() {
        let l = parse_ipconfig(WIN);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], Adapter { name: "Local Area Connection* 10".into(), ip: "192.168.137.1".into() });
        assert_eq!(find_hotspot(&l).unwrap().ip, "192.168.137.1");
        assert_eq!(reachable_addresses(&l).len(), 2);
    }

    #[test]
    fn no_hotspot_means_none() {
        let l = parse_ipconfig("Wireless LAN adapter Wi-Fi:\r\n\r\n   IPv4 Address. . . . . . . . . . . : 192.168.1.20\r\n");
        assert!(find_hotspot(&l).is_none());
        assert!(find_hotspot(&[]).is_none());
    }

    #[test]
    fn android_style_listing_finds_the_access_point() {
        let t = "1: lo    inet 127.0.0.1/8 scope host lo\n21: wlan0    inet 192.168.1.50/24 brd 192.168.1.255 scope global wlan0\n30: ap0    inet 192.168.49.1/24 brd 192.168.49.255 scope global ap0\n";
        let l = parse_ip_addr(t);
        assert_eq!(l.len(), 3);
        assert_eq!(find_hotspot(&l).unwrap().name, "ap0");
        assert_eq!(reachable_addresses(&l).len(), 2);
    }

    #[test]
    fn link_local_and_empty_addresses_are_never_a_hotspot() {
        assert!(!is_hotspot(&Adapter { name: "Local Area Connection* 3".into(), ip: "169.254.9.9".into() }));
        assert!(!is_hotspot(&Adapter { name: "ap0".into(), ip: "".into() }));
    }
}
