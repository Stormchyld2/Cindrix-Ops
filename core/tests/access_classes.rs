//! Backend permissions (roadmap, docs/61): every command the app registers has exactly one reach.
//! Host only is the default; a command reaches a guest or a trusted phone only when it is on the
//! lists in `core/src/share.rs`. These tests fail when a new command breaks that.

use cindrix_core::share::{command_allowed, guest_allowed, Role, GUEST_COMMANDS, PERSON_COMMANDS};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn repo(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(p)
}

/// Names inside `generate_handler![ ... ]` in src-tauri/src/lib.rs (last part of each path).
fn registered() -> BTreeSet<String> {
    let lib = fs::read_to_string(repo("src-tauri/src/lib.rs")).unwrap();
    let start = lib.find("generate_handler![").expect("generate_handler not found");
    let body = &lib[start + "generate_handler![".len()..];
    let end = body.find("])").expect("end of generate_handler not found");
    body[..end]
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("//"))
        .map(|s| s.rsplit("::").next().unwrap().to_string())
        .collect()
}

/// Names of the `"name" =>` arms in the share server's `dispatch`.
fn served() -> BTreeSet<String> {
    let src = fs::read_to_string(repo("src-tauri/src/share.rs")).unwrap();
    let start = src.find("fn dispatch(").expect("dispatch not found");
    let rest = &src[start..];
    let body = &rest[..rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len())];
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix('"') {
            if let Some(end) = rest.find("\" =>") {
                let name = &rest[..end];
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                    out.insert(name.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn nobody_reaches_anything_before_the_pin() {
    for c in registered() {
        assert!(!command_allowed(&Role::Nobody, &c), "{} must be refused before sign-in", c);
    }
}

#[test]
fn everything_the_share_server_answers_is_on_a_list_and_nothing_on_a_list_is_unserved() {
    let served = served();
    assert!(served.len() > 20, "could not read the dispatch list ({} found)", served.len());
    let person = Role::Person("x".into());
    for c in &served {
        assert!(command_allowed(&Role::Guest, c) || command_allowed(&person, c), "{} is answered by the server but no role may ask for it", c);
    }
    // the commands handled before dispatch (devices, mailbox, photos, maps) are the only ones allowed to be missing from it
    let before_dispatch = ["photo_read", "photo_add", "device_check_in", "device_erased", "pocket_upload", "phone_map_read", "ask_stream", "music_prepare", "shelf_read", "shelf_download", "whoami", "peer_get", "peer_put"];
    for c in GUEST_COMMANDS.iter().chain(PERSON_COMMANDS.iter()) {
        assert!(served.contains(*c) || before_dispatch.contains(c), "{} is on a role list but the server never answers it", c);
    }
}

#[test]
fn every_command_a_guest_or_phone_may_ask_is_a_real_command_or_a_known_server_route() {
    let reg = registered();
    let routes = ["photo_add", "photo_read", "device_check_in", "device_erased", "pocket_upload", "phone_map_read", "ask_stream", "music_prepare", "shelf_read", "shelf_download", "whoami", "photos_list", "photo_delete", "phone_maps_list", "dashboard_suggestions", "guides_list", "bible_notes", "approvals_list", "approvals_accept", "approvals_set_aside", "peer_get", "peer_put"];
    for c in GUEST_COMMANDS.iter().chain(PERSON_COMMANDS.iter()) {
        assert!(reg.contains(*c) || routes.contains(c), "{} is on a role list but is neither registered nor a known server route", c);
    }
}

#[test]
fn guests_never_get_a_command_that_writes_or_touches_private_files() {
    let writes = ["_save", "_add", "_delete", "_set", "_write", "_erase", "_import", "_accept", "_remove", "_start", "_stop", "_create", "_update", "_restore", "_bring_back", "_now", "_clear", "_upload"];
    let private = ["family", "inventory", "gear", "journal", "profile", "pace", "backup", "seclock", "security", "device", "calendar", "task", "project", "pocket", "scan", "safe", "druglabel", "drug_label", "tracks", "unlock", "lock", "passcode", "hotspot", "share_"];
    for c in GUEST_COMMANDS {
        assert!(!writes.iter().any(|w| c.ends_with(w)), "{} looks like a write and is on the guest list", c);
        assert!(!private.iter().any(|p| c.contains(p)), "{} looks private and is on the guest list", c);
    }
    // a phone may only add to its own album, say hello, say it erased itself, and send logs
    let person: BTreeSet<&str> = PERSON_COMMANDS.iter().copied().collect();
    let expected: BTreeSet<&str> = ["whoami", "photo_add", "photo_delete", "device_check_in", "device_erased", "pocket_upload", "phone_maps_list", "phone_map_read", "approvals_list", "approvals_accept", "approvals_set_aside", "peer_get", "peer_put"].into_iter().collect();
    assert_eq!(person, expected, "the trusted-phone list changed: check docs/34 and docs/45, then update this test on purpose");
}

#[test]
fn the_unlock_and_sharing_switches_are_host_only() {
    let reg = registered();
    for c in reg.iter().filter(|c| c.starts_with("security_") || c.starts_with("seclock") || c.starts_with("backup_") || c.starts_with("devices_") || c.starts_with("gear_") || c.starts_with("share_") || c.starts_with("roles_") || c.starts_with("hotspot")) {
        assert!(!command_allowed(&Role::Guest, c), "{} must be host only", c);
        assert!(!command_allowed(&Role::Person("x".into()), c), "{} must not be open to a phone", c);
    }
}
