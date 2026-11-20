//! Acceptance matrix rows (docs/44 section 12) that need more than one part of Security working together.
//! Rows that a single module's own tests already prove are listed in doc 44 section 12 with the test names.

use cindrix_core::security_api as api;
use std::fs;

fn tmp(n: &str) -> std::path::PathBuf {
    let r = std::env::temp_dir().join(format!("cindrix-accept-{}-{}", n, std::process::id()));
    let _ = fs::remove_dir_all(&r);
    fs::create_dir_all(&r).unwrap();
    r
}

/// Row 4: several sites keep their own posture, incidents and plans; nothing one site does shows on another.
#[test]
fn row_4_sites_are_isolated_from_each_other() {
    let r = tmp("row4");
    api::site_save(&r, "", "Home", "", "", "").unwrap();
    api::site_save(&r, "", "Camp", "", "", "").unwrap();
    // a new site starts Unknown (row 3), and raising one leaves the other as it was
    api::posture_set(&r, "home", "elevated", "Dad", "Strange car", 4, 0).unwrap();
    let j = api::overview(&r, 0).unwrap();
    assert!(j.contains("\"id\":\"home\"") && j.contains("\"id\":\"camp\""));
    let camp = &j[j.find("\"id\":\"camp\"").unwrap()..];
    assert!(camp.contains("\"posture\":\"unknown\""), "camp must still read unknown: {}", camp);
    // incidents on two sites get different ids, and each shows against its own site
    let a = api::incident_open(&r, "home", "Noise at the gate", 3, false, "Dad", 0).unwrap();
    let b = api::incident_open(&r, "camp", "Noise at the gate", 3, false, "Mom", 0).unwrap();
    assert_ne!(a, b);
    let list = api::incident_list(&r).unwrap();
    assert!(list.contains("\"site\":\"home\"") && list.contains("\"site\":\"camp\""), "{}", list);
    // the camp's "needs a look" does not carry anything about the home's posture
    let looks_for = |site: &str| {
        let from = j.find(&format!("\"id\":\"{}\"", site)).unwrap();
        j[from..].split("\"id\":").nth(1).unwrap_or("").to_string()
    };
    assert!(!looks_for("camp").contains("Strange car"));
    let _ = fs::remove_dir_all(&r);
}

/// Rows 5 and 13: nothing in Security needs a server, the internet or another program to open or write its records.
#[test]
fn rows_5_and_13_security_works_with_nothing_else_running() {
    let r = tmp("row13");
    // an empty drive: every list opens and says "nothing yet", no error
    assert!(api::overview(&r, 0).is_ok());
    assert!(api::incident_list(&r).is_ok());
    api::site_save(&r, "", "Home", "", "", "").unwrap();
    api::posture_set(&r, "home", "normal", "Dad", "", 4, 0).ok();
    assert!(api::overview(&r, 0).is_ok());
    let _ = fs::remove_dir_all(&r);
}
