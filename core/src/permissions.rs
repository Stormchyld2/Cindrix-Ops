//! What each named person on a trusted phone may do (docs/61, doc 44 section 11, doc 45).
//!
//! Two things a phone can be allowed, and nothing more:
//! - `log`: send reports to the host (check-ins, counts, GPS tracks, notes);
//! - `approve`: accept or set aside the reports that are waiting on the host.
//!
//! A role is a name with a set of those. Two roles exist at the start, Reporter and Approver, and the host may add
//! or remove roles in Settings. A person (the name on a trusted phone) has one role; a person with none written
//! down gets the first role in the list, and when no roles are left a person can read and nothing else.
//! The file is plain text, `user/roles.txt`, host-only. A damaged file is never replaced silently, and while it
//! cannot be read nobody is allowed anything beyond reading.

use crate::fsio;
use crate::share::Role as Who;
use std::path::{Path, PathBuf};

pub const CAPS: &[(&str, &str)] = &[
    ("log", "Send reports to the host: check-ins, counts, tracks and notes"),
    ("approve", "Accept or set aside the reports waiting on the host"),
    ("sync", "Keep gear, inventory and security records in step with this device (for your other computer or phone)"),
];

/// Commands a phone may ask for, and the permission each one needs.
pub const NEEDS: &[(&str, &str)] = &[
    ("pocket_upload", "log"),
    ("approvals_list", "approve"),
    ("approvals_accept", "approve"),
    ("approvals_set_aside", "approve"),
    ("peer_get", "sync"),
    ("peer_put", "sync"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    pub name: String,
    pub caps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub roles: Vec<Role>,
    /// (person, role name)
    pub people: Vec<(String, String)>,
}

pub fn defaults() -> Table {
    Table {
        roles: vec![
            Role { name: "Reporter".into(), caps: vec!["log".into()] },
            Role { name: "Approver".into(), caps: vec!["log".into(), "approve".into()] },
        ],
        people: vec![],
    }
}

pub fn path(user_root: &Path) -> PathBuf { user_root.join("roles.txt") }

const HEADER: &str = "# format: roles 1";

pub fn serialize(t: &Table) -> String {
    let mut s = format!("{}\n# role<TAB>name<TAB>permissions   person<TAB>name<TAB>role\n", HEADER);
    for r in &t.roles { s.push_str(&format!("role\t{}\t{}\n", r.name, r.caps.join(","))); }
    for (p, r) in &t.people { s.push_str(&format!("person\t{}\t{}\n", p, r)); }
    s
}

pub fn parse(text: &str) -> Result<Table, String> {
    if !text.starts_with(HEADER) { return Err("The roles file is from a different version, so it was left alone.".into()); }
    let mut t = Table { roles: vec![], people: vec![] };
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let f: Vec<&str> = l.split('\t').collect();
        match f.as_slice() {
            ["role", name, caps] if !name.trim().is_empty() => {
                let caps: Vec<String> = caps.split(',').map(|c| c.trim().to_string()).filter(|c| CAPS.iter().any(|(k, _)| k == c)).collect();
                t.roles.push(Role { name: name.trim().to_string(), caps });
            }
            ["person", name, role] if !name.trim().is_empty() => t.people.push((name.trim().to_string(), role.trim().to_string())),
            _ => return Err("A line of the roles file could not be read, so it was left alone.".into()),
        }
    }
    Ok(t)
}

/// The table on disk, or the two starting roles when nothing has been saved yet.
pub fn load(user_root: &Path) -> Result<Table, String> {
    match fs_read(&path(user_root)) {
        Ok(text) => parse(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(defaults()),
        Err(_) => Err("The roles file could not be read.".into()),
    }
}
fn fs_read(p: &Path) -> std::io::Result<String> { fsio::read_to_string(p) }

pub fn save(user_root: &Path, t: &Table) -> Result<(), String> {
    fsio::backup_then_write(&path(user_root), &user_root.join("backups"), serialize(t).as_bytes()).map_err(|e| format!("The roles could not be saved: {}", e))
}

fn clean_name(name: &str) -> Result<String, String> {
    let s: String = name.chars().filter(|c| !c.is_control()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() { return Err("A role needs a name.".into()); }
    if s.chars().count() > 30 { return Err("Use a shorter name for the role (30 letters at most).".into()); }
    Ok(s)
}

pub fn add_role(t: &mut Table, name: &str, caps: &[String]) -> Result<(), String> {
    let name = clean_name(name)?;
    if t.roles.iter().any(|r| r.name.eq_ignore_ascii_case(&name)) { return Err("There is already a role with that name.".into()); }
    if t.roles.len() >= 12 { return Err("That is as many roles as the list holds. Remove one first.".into()); }
    let mut keep: Vec<String> = vec![];
    for c in caps {
        if !CAPS.iter().any(|(k, _)| k == c) { return Err("That permission does not exist.".into()); }
        if !keep.contains(c) { keep.push(c.clone()); }
    }
    t.roles.push(Role { name, caps: keep });
    Ok(())
}

/// Take a role away. People who had it go back to having none written down (the first role in the list).
pub fn remove_role(t: &mut Table, name: &str) -> Result<(), String> {
    let at = t.roles.iter().position(|r| r.name == name).ok_or("There is no such role.")?;
    t.roles.remove(at);
    t.people.retain(|(_, r)| r != name);
    Ok(())
}

pub fn assign(t: &mut Table, person: &str, role: &str) -> Result<(), String> {
    if !t.roles.iter().any(|r| r.name == role) { return Err("There is no such role.".into()); }
    let person = person.trim();
    if person.is_empty() { return Err("A person needs a name.".into()); }
    t.people.retain(|(p, _)| p != person);
    t.people.push((person.to_string(), role.to_string()));
    Ok(())
}

/// The role a person has: the one written down, else the first in the list, else none.
pub fn role_of<'a>(t: &'a Table, person: &str) -> Option<&'a Role> {
    t.people.iter().find(|(p, _)| p == person).and_then(|(_, r)| t.roles.iter().find(|x| &x.name == r)).or_else(|| t.roles.first())
}

pub fn may(t: &Table, person: &str, cap: &str) -> bool {
    role_of(t, person).map(|r| r.caps.iter().any(|c| c == cap)).unwrap_or(false)
}

/// The one question the share server asks before it answers a request: may this person (or guest) ask for this?
/// The fixed lists in `share.rs` come first; a command in `NEEDS` also needs the permission. A table that could not
/// be read (`None`) allows nothing that needs a permission.
pub fn allowed(who: &Who, cmd: &str, table: Option<&Table>) -> bool {
    if !crate::share::command_allowed(who, cmd) { return false; }
    let Some((_, cap)) = NEEDS.iter().find(|(c, _)| *c == cmd) else { return true };
    match who {
        Who::Person(p) => table.map(|t| may(t, p, cap)).unwrap_or(false),
        _ => false,
    }
}

/// People as JSON for the Settings screen: roles with their permissions, and who has which.
pub fn json(t: &Table, people: &[String]) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    let roles: Vec<String> = t.roles.iter().map(|r| format!("{{\"name\":{},\"caps\":[{}]}}", q(&r.name), r.caps.iter().map(|c| q(c)).collect::<Vec<_>>().join(","))).collect();
    let caps: Vec<String> = CAPS.iter().map(|(k, d)| format!("{{\"key\":{},\"text\":{}}}", q(k), q(d))).collect();
    let who: Vec<String> = people.iter().map(|p| format!("{{\"person\":{},\"role\":{}}}", q(p), role_of(t, p).map(|r| q(&r.name)).unwrap_or_else(|| "null".into()))).collect();
    format!("{{\"roles\":[{}],\"caps\":[{}],\"people\":[{}]}}", roles.join(","), caps.join(","), who.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::Role as W;

    fn dir(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-perm-{}-{}", n, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn two_roles_exist_at_the_start_and_a_new_person_is_a_reporter() {
        let t = defaults();
        assert_eq!(t.roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), vec!["Reporter", "Approver"]);
        assert!(may(&t, "Sam", "log"));
        assert!(!may(&t, "Sam", "approve"));
    }

    #[test]
    fn roles_can_be_added_assigned_and_removed() {
        let mut t = defaults();
        add_role(&mut t, "  Watch   lead ", &["approve".into()]).unwrap();
        assert!(add_role(&mut t, "watch lead", &[]).is_err(), "names are unique, whatever the capitals");
        assert!(add_role(&mut t, "X", &["delete-everything".into()]).is_err());
        assign(&mut t, "Sam", "Watch lead").unwrap();
        assert!(may(&t, "Sam", "approve") && !may(&t, "Sam", "log"));
        assign(&mut t, "Sam", "Approver").unwrap();
        assert!(may(&t, "Sam", "approve") && may(&t, "Sam", "log"));
        remove_role(&mut t, "Approver").unwrap();
        // Sam falls back to the first role in the list
        assert_eq!(role_of(&t, "Sam").unwrap().name, "Reporter");
        remove_role(&mut t, "Reporter").unwrap();
        remove_role(&mut t, "Watch lead").unwrap();
        assert!(!may(&t, "Sam", "log"), "with no roles left a phone can read and nothing else");
        assert!(remove_role(&mut t, "Nope").is_err());
    }

    #[test]
    fn the_file_round_trips_and_a_damaged_one_is_refused_and_kept() {
        let d = dir("file");
        assert_eq!(load(&d).unwrap(), defaults(), "nothing saved yet means the starting roles");
        let mut t = defaults();
        add_role(&mut t, "Lookout", &["log".into()]).unwrap();
        assign(&mut t, "Ana", "Lookout").unwrap();
        save(&d, &t).unwrap();
        assert_eq!(load(&d).unwrap(), t);
        std::fs::write(path(&d), "garbage").unwrap();
        assert!(load(&d).is_err());
        assert!(save_over_damaged_keeps_a_copy(&d, &t));
    }
    fn save_over_damaged_keeps_a_copy(d: &Path, t: &Table) -> bool {
        save(d, t).is_ok() && std::fs::read_dir(d.join("backups")).map(|r| r.count() > 0).unwrap_or(false)
    }

    #[test]
    fn the_server_check_needs_the_permission_and_never_widens_the_fixed_lists() {
        let t = defaults();
        let sam = W::Person("Sam".into());
        // a reporter may send logs, not approve
        assert!(allowed(&sam, "pocket_upload", Some(&t)));
        assert!(!allowed(&sam, "approvals_accept", Some(&t)));
        let mut t2 = t.clone();
        assign(&mut t2, "Sam", "Approver").unwrap();
        assert!(allowed(&sam, "approvals_accept", Some(&t2)));
        // guests and nobody never get a command that needs a permission
        for c in ["pocket_upload", "approvals_list", "approvals_accept", "approvals_set_aside", "peer_get", "peer_put"] {
            assert!(!allowed(&W::Guest, c, Some(&t2)), "{}", c);
            assert!(!allowed(&W::Nobody, c, Some(&t2)), "{}", c);
            assert!(!allowed(&sam, c, None), "{} must be refused when the roles file cannot be read", c);
        }
        // a command nobody listed stays refused even for an approver
        assert!(!allowed(&sam, "security_posture_set", Some(&t2)));
        // ordinary reading still works for a person
        assert!(allowed(&sam, "list_cards", Some(&t2)));
    }

    #[test]
    fn json_lists_roles_permissions_and_people() {
        let t = defaults();
        let j = json(&t, &["Sam".to_string()]);
        assert!(j.contains("\"Reporter\"") && j.contains("\"approve\"") && j.contains("\"person\":\"Sam\",\"role\":\"Reporter\""));
    }
}
