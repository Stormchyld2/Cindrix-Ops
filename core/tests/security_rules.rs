//! Rules from docs/44 section 14 that must hold for everything Security Operations adds.

use cindrix_core::security_api::SECURITY_COMMANDS;
use cindrix_core::share::{guest_allowed, GUEST_COMMANDS};
use std::fs;
use std::path::PathBuf;

fn repo(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
}

#[test]
fn guests_are_refused_every_security_command() {
    for c in SECURITY_COMMANDS {
        assert!(!guest_allowed(c), "{} must be host-only", c);
    }
    assert!(!GUEST_COMMANDS.iter().any(|c| c.starts_with("security")), "nothing named security_* may be on the guest list");
    // and the guest list has not quietly grown a way in under another name
    for c in GUEST_COMMANDS {
        assert!(!c.contains("incident") && !c.contains("posture") && !c.contains("watch"), "{} looks like a security command", c);
    }
}

#[test]
fn the_command_list_matches_the_tauri_file_and_the_registration() {
    let tauri = fs::read_to_string(repo("src-tauri/src/security.rs")).unwrap();
    let mut found: Vec<String> = tauri.lines().filter_map(|l| l.strip_prefix("pub fn ")).filter_map(|l| l.split('(').next()).filter(|n| n.starts_with("security_")).map(|n| n.to_string()).collect();
    found.sort();
    let mut listed: Vec<String> = SECURITY_COMMANDS.iter().map(|s| s.to_string()).collect();
    listed.sort();
    assert_eq!(found, listed, "src-tauri/src/security.rs and SECURITY_COMMANDS must list the same commands");
    let lib = fs::read_to_string(repo("src-tauri/src/lib.rs")).unwrap();
    for c in SECURITY_COMMANDS {
        assert!(lib.contains(&format!("security::{},", c)), "{} is not registered in lib.rs", c);
    }
}

#[test]
fn security_code_makes_no_network_calls() {
    for f in ["security.rs", "security_schedule.rs", "security_incident.rs", "security_store.rs", "security_api.rs"] {
        let src = fs::read_to_string(repo("core/src").join(f)).unwrap();
        let code: String = src.split("#[cfg(test)]").next().unwrap().to_string();
        for banned in ["std::net", "TcpStream", "UdpSocket", "TcpListener", "http://", "https://", "std::process", "Command::new"] {
            assert!(!code.contains(banned), "{} must not use {}", f, banned);
        }
    }
    let t = fs::read_to_string(repo("src-tauri/src/security.rs")).unwrap();
    for banned in ["reqwest", "std::net", "http://", "https://", "fetch"] {
        assert!(!t.contains(banned), "src-tauri/src/security.rs must not use {}", banned);
    }
}

#[test]
fn security_files_stay_inside_the_security_folder() {
    // every path the store builds is under user/security, and ids that could climb out are refused
    let r = std::env::temp_dir().join(format!("cindrix-secrules-{}", std::process::id()));
    let _ = fs::remove_dir_all(&r);
    fs::create_dir_all(&r).unwrap();
    for p in [
        cindrix_core::security_store::sites_path(&r),
        cindrix_core::security_store::posture_path(&r),
        cindrix_core::security_store::watch_path(&r),
        cindrix_core::security_store::incident_path(&r),
        cindrix_core::security_store::plan_copy_path(&r, "i1", "e1"),
    ] {
        assert!(p.starts_with(r.join("security")), "{:?}", p);
    }
    for bad in ["../x", "a/b", "..", "A", ""] {
        assert!(!cindrix_core::security::valid_id(bad), "{}", bad);
    }
    let _ = fs::remove_dir_all(&r);
}
