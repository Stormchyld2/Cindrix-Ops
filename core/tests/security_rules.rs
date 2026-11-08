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

#[test]
fn guests_cannot_reach_the_device_list_erase_orders_or_the_phone_mailbox() {
    use cindrix_core::share::{command_allowed, Role, PERSON_COMMANDS};
    for c in ["device_check_in", "device_erased", "pocket_upload", "devices_list", "devices_action", "security_pocket_accept"] {
        assert!(!guest_allowed(c), "{} must not be open to guests", c);
        assert!(!command_allowed(&Role::Guest, c), "{} must not be open to a guest's browser", c);
        assert!(!command_allowed(&Role::Nobody, c));
    }
    // a trusted phone may say hello, say it has erased itself and hand in a report; it may not run the host's screens
    let phone = Role::Person("Dana".into());
    for c in ["device_check_in", "device_erased", "pocket_upload"] {
        assert!(command_allowed(&phone, c) && PERSON_COMMANDS.contains(&c));
    }
    for c in ["devices_list", "devices_action", "security_pocket_accept", "security_pocket_list", "security_overview", "security_posture_set"] {
        assert!(!command_allowed(&phone, c), "{} is for the host only", c);
    }
}

#[test]
fn the_server_checks_a_devices_state_before_it_answers_anything() {
    let s = fs::read_to_string(repo("src-tauri/src/share.rs")).unwrap();
    let gate = s.find("device_gate(app, req)").expect("the server must call device_gate");
    let api = s.find("send(stream, &api_call(app, cmd").expect("api_call is where commands run");
    assert!(gate < api, "the device check must come before any command runs");
    assert!(s.contains("pub fn devices_list") && s.contains("pub fn devices_action"));
    let lib = fs::read_to_string(repo("src-tauri/src/lib.rs")).unwrap();
    assert!(lib.contains("share::devices_list,") && lib.contains("share::devices_action,"));
}

#[test]
fn device_and_mailbox_code_make_no_network_calls() {
    for f in ["devices.rs", "security_pocket.rs"] {
        let src = fs::read_to_string(repo("core/src").join(f)).unwrap();
        let code: String = src.split("#[cfg(test)]").next().unwrap().to_string();
        for banned in ["std::net", "TcpStream", "UdpSocket", "TcpListener", "http://", "https://", "std::process", "Command::new"] {
            assert!(!code.contains(banned), "{} must not use {}", f, banned);
        }
    }
}
