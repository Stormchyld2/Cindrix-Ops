//! The list of phones and other devices the Hub trusts, and what happens when one is lost (docs/45, docs/34).
//!
//! A device is trusted by its own code (the short id made when its invite was used), never by the person who
//! owns it: the person is only a label. The tokens that prove a device stay in `trusted-devices.txt`; this file,
//! `user/devices.txt`, keeps what the host needs to see and decide:
//!
//! ```text
//! # format: devices 1
//! <id> TAB <kind> TAB <state> TAB <since> TAB <last seen> TAB <state changed> TAB <person> TAB <label>
//! ```
//! Times are seconds since 1970 (0 = never). `kind` is `browser` or `pocket`; `state` is `trusted`,
//! `erase-ordered`, `erased` or `revoked`.
//!
//! **Erase order.** The host cannot reach a phone that is switched off or out of range, so an order *waits*: the
//! device stays known, can do nothing at all except ask "anything for me?" and say "done", and the next time it
//! reaches the Hub it is told to erase its Cindrix data. When it says it has, or the host locks it out by hand,
//! its sign-in is removed for good. What an erase can reach is only the app's own data and saved sign-in: an app
//! cannot wipe the rest of a phone, and a plain browser can only be shut out and asked to clear what the page
//! itself stored. Every step is added to `devices-log.txt`, which is only ever added to.

use crate::fsio;
use crate::ops::{esc, unesc};
use crate::security::{check_format, Parsed};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Browser,
    Pocket,
}
impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Browser => "browser",
            Kind::Pocket => "pocket",
        }
    }
    fn from_word(w: &str) -> Option<Kind> {
        match w {
            "browser" => Some(Kind::Browser),
            "pocket" => Some(Kind::Pocket),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Trusted,
    EraseOrdered,
    Erased,
    Revoked,
}
impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Trusted => "trusted",
            State::EraseOrdered => "erase-ordered",
            State::Erased => "erased",
            State::Revoked => "revoked",
        }
    }
    fn from_word(w: &str) -> Option<State> {
        match w {
            "trusted" => Some(State::Trusted),
            "erase-ordered" => Some(State::EraseOrdered),
            "erased" => Some(State::Erased),
            "revoked" => Some(State::Revoked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: String,
    pub kind: Kind,
    pub state: State,
    pub since: u64,
    pub last_seen: u64,
    pub state_at: u64,
    pub person: String,
    pub label: String,
}

pub fn path(user_root: &Path) -> PathBuf {
    user_root.join("devices.txt")
}
pub fn log_path(user_root: &Path) -> PathBuf {
    user_root.join("devices-log.txt")
}

pub fn header() -> String {
    "# format: devices 1\n# one device per line: id, kind, state, since, last seen, state changed, person, label\n".to_string()
}

pub fn line(d: &Device) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", d.id, d.kind.word(), d.state.word(), d.since, d.last_seen, d.state_at, esc(&d.person), esc(&d.label))
}

pub fn parse(text: &str) -> Result<Parsed<Device>, String> {
    check_format(text, "devices", 1)?;
    let mut out = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.splitn(8, '\t').collect();
        let d = (|| {
            if f.len() != 8 {
                return None;
            }
            let id = f[0].to_string();
            if !crate::security::valid_id(&id) || id.len() > 12 {
                return None;
            }
            Some(Device {
                id,
                kind: Kind::from_word(f[1])?,
                state: State::from_word(f[2])?,
                since: f[3].parse().ok()?,
                last_seen: f[4].parse().ok()?,
                state_at: f[5].parse().ok()?,
                person: unesc(f[6]),
                label: unesc(f[7]),
            })
        })();
        match d {
            Some(d) if !out.items.iter().any(|o: &Device| o.id == d.id) => out.items.push(d),
            _ => out.rejected.push(l.to_string()),
        }
    }
    Ok(out)
}

/// Read the list. A missing file is an empty list; a damaged or newer file is an error and is never overwritten.
pub fn load(user_root: &Path) -> Result<Parsed<Device>, String> {
    match fsio::read_to_string(&path(user_root)) {
        Ok(t) => parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Parsed::default()),
        Err(_) => Err("The device list could not be read.".to_string()),
    }
}

/// Save the list. Refuses if the file on disk cannot be read as it stands, so a damaged file is never replaced.
pub fn save(user_root: &Path, list: &[Device]) -> Result<(), String> {
    load(user_root)?;
    let mut t = header();
    for d in list {
        t.push_str(&line(d));
    }
    let backups = user_root.join("backups");
    fsio::backup_then_write(&path(user_root), &backups, t.as_bytes()).map_err(|_| "The device list could not be saved.".to_string())
}

/// Add one line to the log. `what` is a short word (`erase-ordered`, `erased`, `locked-out`, `seen`, ...).
pub fn log(user_root: &Path, now: u64, id: &str, what: &str, detail: &str) {
    let existing = fsio::read_to_string(&log_path(user_root)).unwrap_or_default();
    let mut out = String::new();
    if existing.is_empty() {
        out.push_str("# format: devices-log 1\n# time (seconds since 1970), device, what happened, detail\n");
    } else if !existing.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{}\t{}\t{}\t{}\n", now, id, what, esc(detail)));
    let _ = fsio::append(&log_path(user_root), out.as_bytes());
}

/// Make sure every device that can sign in has a line, and that a device taken off the trusted list elsewhere
/// (Settings > Sharing > remove) shows as revoked. `trusted` is (id, person, since) from the sign-in list.
/// Returns true if anything changed.
pub fn sync(list: &mut Vec<Device>, trusted: &[(String, String, u64)], now: u64) -> bool {
    let mut changed = false;
    for (id, person, since) in trusted {
        if !list.iter().any(|d| &d.id == id) {
            list.push(Device { id: id.clone(), kind: Kind::Browser, state: State::Trusted, since: *since, last_seen: 0, state_at: *since, person: person.clone(), label: String::new() });
            changed = true;
        }
    }
    for d in list.iter_mut() {
        if matches!(d.state, State::Trusted | State::EraseOrdered) && !trusted.iter().any(|(id, _, _)| id == &d.id) {
            d.state = State::Revoked;
            d.state_at = now;
            changed = true;
        }
    }
    changed
}

/// What a device that has just reached the Hub is told.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Instruction {
    /// Carry on.
    Ok,
    /// Erase your Cindrix data and sign-in, then say so.
    Erase,
    /// You are not trusted any more.
    Refused,
}

fn find<'a>(list: &'a mut [Device], id: &str) -> Option<&'a mut Device> {
    list.iter_mut().find(|d| d.id == id)
}

/// A trusted device reached the Hub: note the time, and take its kind and a short label if it gave them.
pub fn check_in(list: &mut Vec<Device>, id: &str, kind: Option<Kind>, label: &str, now: u64) -> Instruction {
    let Some(d) = find(list, id) else { return Instruction::Refused };
    match d.state {
        State::Erased | State::Revoked => return Instruction::Refused,
        _ => {}
    }
    d.last_seen = now;
    if let Some(k) = kind {
        d.kind = k;
    }
    let label: String = label.chars().filter(|c| !c.is_control()).take(40).collect();
    if !label.trim().is_empty() {
        d.label = label.trim().to_string();
    }
    if d.state == State::EraseOrdered {
        Instruction::Erase
    } else {
        Instruction::Ok
    }
}

/// The host orders an erase. Only a trusted device can be ordered; the order waits until the device connects.
pub fn order_erase(list: &mut [Device], id: &str, now: u64) -> Result<(), String> {
    let d = find(list, id).ok_or("That device is not on the list.")?;
    match d.state {
        State::Trusted => {
            d.state = State::EraseOrdered;
            d.state_at = now;
            Ok(())
        }
        State::EraseOrdered => Err("An erase is already waiting for this device.".to_string()),
        _ => Err("This device is already locked out.".to_string()),
    }
}

/// The phone was found before it connected: take the order back.
pub fn cancel_erase(list: &mut [Device], id: &str, now: u64) -> Result<(), String> {
    let d = find(list, id).ok_or("That device is not on the list.")?;
    if d.state != State::EraseOrdered {
        return Err("There is no waiting erase for this device.".to_string());
    }
    d.state = State::Trusted;
    d.state_at = now;
    Ok(())
}

/// The device says it has erased itself. Only a device with a waiting order can say so.
pub fn confirm_erased(list: &mut [Device], id: &str, now: u64) -> Result<(), String> {
    let d = find(list, id).ok_or("That device is not on the list.")?;
    if d.state != State::EraseOrdered {
        return Err("No erase was ordered for this device.".to_string());
    }
    d.state = State::Erased;
    d.state_at = now;
    Ok(())
}

/// The host shuts a device out now, without waiting for it to connect.
pub fn lock_out(list: &mut [Device], id: &str, now: u64) -> Result<(), String> {
    let d = find(list, id).ok_or("That device is not on the list.")?;
    if matches!(d.state, State::Erased | State::Revoked) {
        return Err("This device is already locked out.".to_string());
    }
    d.state = State::Revoked;
    d.state_at = now;
    Ok(())
}

/// What a device in a given state may ask for. A trusted device may use what its role allows; a device with a
/// waiting erase may only ask what is waiting for it and say it is done; a locked-out device may do nothing.
pub fn may_ask(state: State, command: &str) -> bool {
    match state {
        State::Trusted => true,
        State::EraseOrdered => matches!(command, "device_check_in" | "device_erased"),
        State::Erased | State::Revoked => false,
    }
}

/// The state of a device by id. A device that is not on the list yet counts as trusted: `sync` adds it.
pub fn state_of(list: &[Device], id: &str) -> State {
    list.iter().find(|d| d.id == id).map(|d| d.state).unwrap_or(State::Trusted)
}

fn q(s: &str) -> String {
    format!("\"{}\"", crate::llama::json_escape(s))
}

/// The list for the Settings screen, host only. Never includes a sign-in token.
pub fn list_json(list: &[Device]) -> String {
    let rows: Vec<String> = list
        .iter()
        .map(|d| {
            format!(
                "{{\"id\":{},\"kind\":{},\"state\":{},\"since\":{},\"last_seen\":{},\"state_at\":{},\"person\":{},\"label\":{}}}",
                q(&d.id), q(d.kind.word()), q(d.state.word()), d.since, d.last_seen, d.state_at, q(&d.person), q(&d.label)
            )
        })
        .collect();
    format!("{{\"devices\":[{}]}}", rows.join(","))
}

// ---------- what the app calls ----------

/// The sign-in list as (id, person, since), which is all this file needs to know of it.
pub type TrustedIds = [(String, String, u64)];

fn load_synced(user_root: &Path, trusted: &TrustedIds, now: u64) -> Result<Vec<Device>, String> {
    let mut list = load(user_root)?.items;
    if sync(&mut list, trusted, now) {
        save(user_root, &list)?;
    }
    Ok(list)
}

/// The list for the host's Settings screen.
pub fn host_list(user_root: &Path, trusted: &TrustedIds, now: u64) -> Result<String, String> {
    Ok(list_json(&load_synced(user_root, trusted, now)?))
}

/// What the host can do to one device: `erase` (order it), `cancel` (take the order back), `lock` (shut it out now).
/// Returns true when the device's sign-in must be removed from the sign-in list as well.
pub fn host_action(user_root: &Path, trusted: &TrustedIds, id: &str, action: &str, now: u64) -> Result<bool, String> {
    let mut list = load_synced(user_root, trusted, now)?;
    let (word, revoke) = match action {
        "erase" => {
            order_erase(&mut list, id, now)?;
            ("erase-ordered", false)
        }
        "cancel" => {
            cancel_erase(&mut list, id, now)?;
            ("erase-cancelled", false)
        }
        "lock" => {
            lock_out(&mut list, id, now)?;
            ("locked-out", true)
        }
        _ => return Err("That is not something the host can do to a device.".to_string()),
    };
    save(user_root, &list)?;
    log(user_root, now, id, word, "by the host");
    Ok(revoke)
}

/// A trusted device says hello. Returns the answer for the device (JSON) and what it was told.
/// `accepted` lists this device's reports the host has taken in, so it can clear its outbox.
pub fn device_check_in(user_root: &Path, trusted: &TrustedIds, id: &str, kind: Option<Kind>, label: &str, now: u64) -> Result<(String, Instruction), String> {
    let mut list = load_synced(user_root, trusted, now)?;
    let before = list.iter().find(|d| d.id == id).map(|d| d.state);
    let told = check_in(&mut list, id, kind, label, now);
    save(user_root, &list)?;
    if told == Instruction::Erase && before == Some(State::EraseOrdered) {
        log(user_root, now, id, "told-to-erase", "");
    }
    let word = match told {
        Instruction::Ok => "ok",
        Instruction::Erase => "erase",
        Instruction::Refused => "refused",
    };
    let accepted: Vec<String> = if told == Instruction::Ok { crate::security_pocket::accepted_for(user_root, id) } else { Vec::new() };
    let names: Vec<String> = accepted.iter().map(|n| q(n)).collect();
    Ok((format!("{{\"instruction\":\"{}\",\"accepted\":[{}]}}", word, names.join(",")), told))
}

/// The device says it has erased itself. Returns true: its sign-in must now be removed.
pub fn device_erased(user_root: &Path, trusted: &TrustedIds, id: &str, now: u64) -> Result<bool, String> {
    let mut list = load_synced(user_root, trusted, now)?;
    confirm_erased(&mut list, id, now)?;
    save(user_root, &list)?;
    log(user_root, now, id, "erased", "the device said it had erased its data");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-devices-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn tr(id: &str, person: &str) -> (String, String, u64) {
        (id.to_string(), person.to_string(), 100)
    }

    #[test]
    fn devices_appear_from_the_trusted_list_and_lines_survive_writing() {
        let mut l = vec![];
        assert!(sync(&mut l, &[tr("ab12cd34", "Dana"), tr("ef56ab78", "Sam")], 200));
        assert_eq!(l.len(), 2);
        assert!(!sync(&mut l, &[tr("ab12cd34", "Dana"), tr("ef56ab78", "Sam")], 300));
        l[0].label = "Dana\tphone".into();
        let text = format!("{}{}{}", header(), line(&l[0]), line(&l[1]));
        let back = parse(&text).unwrap();
        assert_eq!(back.items, l);
        assert!(back.rejected.is_empty());
    }

    #[test]
    fn trust_is_by_the_device_not_the_person() {
        // two phones for one person are two devices; losing one does not touch the other
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana"), tr("bbbb2222", "Dana")], 0);
        order_erase(&mut l, "aaaa1111", 5).unwrap();
        assert_eq!(state_of(&l, "aaaa1111"), State::EraseOrdered);
        assert_eq!(state_of(&l, "bbbb2222"), State::Trusted);
    }

    #[test]
    fn an_erase_order_waits_then_the_device_is_told_and_is_shut_out() {
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana")], 0);
        assert_eq!(check_in(&mut l, "aaaa1111", Some(Kind::Pocket), "Dana's phone", 10), Instruction::Ok);
        order_erase(&mut l, "aaaa1111", 20).unwrap();
        assert!(order_erase(&mut l, "aaaa1111", 21).is_err());
        // it can do nothing but ask and answer
        assert!(!may_ask(State::EraseOrdered, "security_overview"));
        assert!(!may_ask(State::EraseOrdered, "pocket_upload"));
        assert!(may_ask(State::EraseOrdered, "device_check_in") && may_ask(State::EraseOrdered, "device_erased"));
        assert_eq!(check_in(&mut l, "aaaa1111", None, "", 30), Instruction::Erase);
        confirm_erased(&mut l, "aaaa1111", 31).unwrap();
        assert_eq!(l[0].state, State::Erased);
        assert_eq!(check_in(&mut l, "aaaa1111", None, "", 40), Instruction::Refused);
        assert!(!may_ask(State::Erased, "device_check_in"));
        assert!(confirm_erased(&mut l, "aaaa1111", 50).is_err());
    }

    #[test]
    fn a_found_phone_can_have_its_order_taken_back_but_only_while_it_waits() {
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana")], 0);
        assert!(cancel_erase(&mut l, "aaaa1111", 1).is_err());
        order_erase(&mut l, "aaaa1111", 2).unwrap();
        cancel_erase(&mut l, "aaaa1111", 3).unwrap();
        assert_eq!(l[0].state, State::Trusted);
        order_erase(&mut l, "aaaa1111", 4).unwrap();
        confirm_erased(&mut l, "aaaa1111", 5).unwrap();
        assert!(cancel_erase(&mut l, "aaaa1111", 6).is_err());
    }

    #[test]
    fn only_a_device_with_a_waiting_order_can_say_it_erased() {
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana")], 0);
        assert!(confirm_erased(&mut l, "aaaa1111", 1).is_err());
        assert!(confirm_erased(&mut l, "nobody", 1).is_err());
        assert_eq!(l[0].state, State::Trusted);
    }

    #[test]
    fn locking_out_by_hand_and_removal_elsewhere_both_end_in_revoked() {
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana"), tr("bbbb2222", "Sam")], 0);
        lock_out(&mut l, "aaaa1111", 5).unwrap();
        assert!(lock_out(&mut l, "aaaa1111", 6).is_err());
        assert_eq!(check_in(&mut l, "aaaa1111", None, "", 7), Instruction::Refused);
        // Sam's sign-in is removed from the Sharing screen
        assert!(sync(&mut l, &[], 9));
        assert_eq!(state_of(&l, "bbbb2222"), State::Revoked);
        // a waiting erase is also closed if its sign-in vanishes: nothing is left to deliver it to
        let mut m = vec![];
        sync(&mut m, &[tr("cccc3333", "Kim")], 0);
        order_erase(&mut m, "cccc3333", 1).unwrap();
        sync(&mut m, &[], 2);
        assert_eq!(m[0].state, State::Revoked);
    }

    #[test]
    fn labels_are_cleaned_and_short() {
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana")], 0);
        check_in(&mut l, "aaaa1111", Some(Kind::Pocket), &format!("Ph\u{7}one{}", "x".repeat(100)), 1);
        assert!(l[0].label.chars().count() <= 40 && !l[0].label.contains('\u{7}'));
        check_in(&mut l, "aaaa1111", None, "   ", 2);
        assert!(!l[0].label.is_empty());
        assert_eq!(l[0].last_seen, 2);
        assert_eq!(l[0].kind, Kind::Pocket);
    }

    #[test]
    fn the_file_is_saved_backed_up_and_never_replaces_a_damaged_one() {
        let r = tmp("save");
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana")], 0);
        save(&r, &l).unwrap();
        assert_eq!(load(&r).unwrap().items, l);
        order_erase(&mut l, "aaaa1111", 3).unwrap();
        save(&r, &l).unwrap();
        assert_eq!(load(&r).unwrap().items[0].state, State::EraseOrdered);
        // a file from a newer version is left alone
        fs::write(path(&r), "# format: devices 9\nx\n").unwrap();
        assert!(load(&r).is_err());
        assert!(save(&r, &l).is_err());
        assert_eq!(fs::read_to_string(path(&r)).unwrap(), "# format: devices 9\nx\n");
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn bad_lines_are_kept_aside_not_lost() {
        let t = format!("{}aaaa1111\tpocket\ttrusted\t1\t2\t3\tDana\tphone\nbroken line\naaaa1111\tpocket\ttrusted\t1\t2\t3\tDana\tdup\n", header());
        let p = parse(&t).unwrap();
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.rejected.len(), 2);
    }

    #[test]
    fn the_log_only_grows_and_the_list_never_shows_a_token() {
        let r = tmp("log");
        log(&r, 1, "aaaa1111", "erase-ordered", "from the host");
        log(&r, 2, "aaaa1111", "erased", "");
        let t = fs::read_to_string(log_path(&r)).unwrap();
        assert_eq!(t.lines().filter(|l| !l.starts_with('#')).count(), 2);
        let mut l = vec![];
        sync(&mut l, &[tr("aaaa1111", "Dana")], 0);
        let j = list_json(&l);
        assert!(j.contains("\"state\":\"trusted\"") && !j.contains("token"));
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_whole_trip_from_order_to_lock_out() {
        let r = tmp("trip");
        let t = vec![tr("aaaa1111", "Dana")];
        // hello from a phone: nothing waiting yet
        let (j, told) = device_check_in(&r, &t, "aaaa1111", Some(Kind::Pocket), "Dana's phone", 10).unwrap();
        assert_eq!(told, Instruction::Ok);
        assert!(j.contains("\"instruction\":\"ok\""));
        // the host orders the erase; the sign-in stays so the order can be delivered
        assert!(!host_action(&r, &t, "aaaa1111", "erase", 20).unwrap());
        assert!(host_list(&r, &t, 21).unwrap().contains("erase-ordered"));
        let (j, told) = device_check_in(&r, &t, "aaaa1111", None, "", 30).unwrap();
        assert_eq!(told, Instruction::Erase);
        assert!(j.contains("\"instruction\":\"erase\"") && j.contains("\"accepted\":[]"));
        // the device says done: its sign-in must go
        assert!(device_erased(&r, &t, "aaaa1111", 31).unwrap());
        // once the sign-in is gone it shows as locked out and is refused
        let (_, told) = device_check_in(&r, &[], "aaaa1111", None, "", 40).unwrap();
        assert_eq!(told, Instruction::Refused);
        let log = fs::read_to_string(log_path(&r)).unwrap();
        for w in ["erase-ordered", "told-to-erase", "erased"] {
            assert!(log.contains(w), "{}", w);
        }
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn the_host_can_lock_out_at_once_and_unknown_actions_do_nothing() {
        let r = tmp("lock");
        let t = vec![tr("aaaa1111", "Dana")];
        assert!(host_action(&r, &t, "aaaa1111", "wipe-everything", 1).is_err());
        assert!(host_action(&r, &t, "nobody", "erase", 1).is_err());
        assert!(host_action(&r, &t, "aaaa1111", "lock", 2).unwrap());
        assert!(host_action(&r, &t, "aaaa1111", "erase", 3).is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
