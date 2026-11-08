//! Communications records for Security (docs/46, plan COPS-COM-001 sections 3, 12, 13 and 15):
//! the communications log, radio coverage survey results, the channel plan per site and operator training.
//!
//! Plain tab-separated files under `user/security/`, each starting `# format: <kind> 1`. The three logs are
//! append-only (a mistake or an update is a later line; nothing is changed or erased) and a log that cannot be read
//! is never added to. The channel plan is a small list that is rewritten with a backup first.
//! The software is an organizer: every radio procedure works without it.

use crate::fsio;
use crate::ops::{esc, unesc};
use crate::security::{check_format, parse_instant, valid_id, Parsed};
use crate::security_store as store;
use std::fs;
use std::path::{Path, PathBuf};

// ---------- the communications log (plan section 13) ----------

pub const CATEGORIES: &[(&str, &str)] = &[
    ("watch-turnover", "Watch turnover"),
    ("departure-return", "Personnel departure and return"),
    ("missed-checkin", "Missed check-in"),
    ("equipment-failure", "Communications equipment failure"),
    ("emergency-traffic", "Emergency traffic"),
    ("channel-change", "Channel change"),
    ("weather-warning", "Significant weather warning"),
    ("coverage-problem", "Radio coverage problem"),
];
/// Categories that stay on "needs a look" while they are open.
const WATCHED: &[&str] = &["missed-checkin", "equipment-failure", "emergency-traffic", "coverage-problem"];

#[derive(Debug, Clone, PartialEq)]
pub struct CommsEntry {
    pub id: String,
    pub site: String,
    pub at: String,
    pub recorded: String,
    pub author: String,
    pub station: String,
    pub category: String,
    pub event: String,
    pub action: String,
    /// open, complete (done at once) or closed (this line closes the entry named in `closes`)
    pub status: String,
    pub closes: String,
}

fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect::<String>().trim().to_string()
}

fn rows(text: &str, kind: &str, cols: usize) -> Result<(Vec<Vec<String>>, Vec<String>), String> {
    check_format(text, kind, 1)?;
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<String> = l.split('\t').map(|c| c.to_string()).collect();
        if f.len() == cols && valid_id(&f[0]) { ok.push(f) } else { bad.push(l.to_string()) }
    }
    Ok((ok, bad))
}

fn read_text(p: &Path) -> Result<String, String> {
    match fs::read_to_string(p) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(_) => Err("A communications file could not be read.".to_string()),
    }
}

pub fn comms_path(r: &Path) -> PathBuf {
    store::dir(r).join("comms-log.txt")
}
pub fn parse_comms(text: &str) -> Result<Parsed<CommsEntry>, String> {
    let (ok, mut rejected) = rows(text, "comms-log", 11)?;
    let mut items: Vec<CommsEntry> = Vec::new();
    for f in ok {
        let e = CommsEntry { id: f[0].clone(), site: f[1].clone(), at: f[2].clone(), recorded: f[3].clone(), author: unesc(&f[4]), station: unesc(&f[5]), category: f[6].clone(), event: unesc(&f[7]), action: unesc(&f[8]), status: f[9].clone(), closes: f[10].clone() };
        if items.iter().any(|x| x.id == e.id) || !CATEGORIES.iter().any(|c| c.0 == e.category) || !matches!(e.status.as_str(), "open" | "complete" | "closed") {
            rejected.push(format!("{}", e.id));
        } else {
            items.push(e);
        }
    }
    Ok(Parsed { items, rejected })
}
pub fn load_comms(r: &Path) -> Result<Parsed<CommsEntry>, String> {
    parse_comms(&read_text(&comms_path(r))?)
}
fn comms_line(e: &CommsEntry) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", e.id, e.site, e.at, e.recorded, esc(&e.author), esc(&e.station), e.category, esc(&e.event), esc(&e.action), e.status, e.closes)
}
const COMMS_HEADER: &str = "# format: comms-log 1\n# id, site, time, recorded, author, station, category, event, action, status, closes (never changed)\n";

/// Entries that are still open: not closed by a later line.
pub fn open_items(all: &[CommsEntry]) -> Vec<&CommsEntry> {
    all.iter().filter(|e| e.status == "open" && !all.iter().any(|c| c.status == "closed" && c.closes == e.id)).collect()
}

pub fn check_comms(existing: &[CommsEntry], sites: &[String], e: &CommsEntry) -> Option<String> {
    if existing.iter().any(|x| x.id == e.id) || !valid_id(&e.id) {
        return Some("That entry number is already used.".to_string());
    }
    if !sites.iter().any(|s| s == &e.site) {
        return Some("That site is not on the list.".to_string());
    }
    if parse_instant(&e.at).is_none() {
        return Some("The time could not be read.".to_string());
    }
    if e.author.trim().is_empty() {
        return Some("Say who is writing this.".to_string());
    }
    if e.event.trim().is_empty() {
        return Some("Say what happened.".to_string());
    }
    if e.event.len() > 400 || e.action.len() > 400 || e.station.len() > 60 {
        return Some("That entry is too long.".to_string());
    }
    if !CATEGORIES.iter().any(|c| c.0 == e.category) {
        return Some("Pick one of the log categories.".to_string());
    }
    match e.status.as_str() {
        "open" | "complete" if e.closes.is_empty() => None,
        "closed" => match existing.iter().find(|x| x.id == e.closes) {
            Some(t) if t.status == "open" && open_items(existing).iter().any(|o| o.id == t.id) => None,
            Some(_) => Some("That entry is not open.".to_string()),
            None => Some("There is no such entry to close.".to_string()),
        },
        _ => Some("The status has to be open, complete or closed.".to_string()),
    }
}

pub fn append_comms(r: &Path, e: &CommsEntry) -> Result<(), String> {
    let sites: Vec<String> = store::load_sites(r)?.items.into_iter().map(|s| s.id).collect();
    let log = load_comms(r)?;
    if !log.rejected.is_empty() {
        return Err("Some lines in the communications log could not be read, so nothing was added. Look at those first.".to_string());
    }
    if let Some(p) = check_comms(&log.items, &sites, e) {
        return Err(p);
    }
    store::append_line(r, &comms_path(r), COMMS_HEADER, &comms_line(e), |t| parse_comms(t).map(|_| ()))
}

pub fn new_entry(existing: &[CommsEntry], site: &str, at: &str, recorded: &str, author: &str, station: &str, category: &str, event: &str, action: &str, status: &str, closes: &str) -> CommsEntry {
    CommsEntry { id: crate::ops::next_id('c', existing.iter().map(|e| e.id.clone())), site: site.into(), at: at.into(), recorded: recorded.into(), author: clean(author, 80), station: clean(station, 60), category: category.into(), event: clean(event, 400), action: clean(action, 400), status: status.into(), closes: closes.into() }
}

// ---------- radio coverage survey (plan section 12) ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    pub id: String,
    pub site: String,
    pub place: String,
    /// Ratings 0 to 5 on the primary and alternate channel, heard / sent. None = not tested.
    pub p_in: Option<u8>,
    pub p_out: Option<u8>,
    pub a_in: Option<u8>,
    pub a_out: Option<u8>,
    pub at: String,
    pub recorded: String,
    pub by: String,
    pub note: String,
}
pub fn coverage_path(r: &Path) -> PathBuf {
    store::dir(r).join("coverage-log.txt")
}
fn rating(s: &str) -> Result<Option<u8>, ()> {
    if s.is_empty() { return Ok(None); }
    match s.parse::<u8>() { Ok(n) if n <= 5 => Ok(Some(n)), _ => Err(()) }
}
fn rate_word(n: Option<u8>) -> String {
    n.map(|n| n.to_string()).unwrap_or_default()
}
pub fn parse_coverage(text: &str) -> Result<Parsed<Coverage>, String> {
    let (ok, mut rejected) = rows(text, "coverage-log", 11)?;
    let mut items: Vec<Coverage> = Vec::new();
    for f in ok {
        let r: Result<Coverage, ()> = (|| Ok(Coverage { id: f[0].clone(), site: f[1].clone(), place: unesc(&f[2]), p_in: rating(&f[3])?, p_out: rating(&f[4])?, a_in: rating(&f[5])?, a_out: rating(&f[6])?, at: f[7].clone(), recorded: String::new(), by: unesc(&f[8]), note: unesc(&f[9]) }))();
        match r {
            Ok(mut c) if !c.place.is_empty() && !items.iter().any(|x| x.id == c.id) => { c.recorded = f[10].clone(); items.push(c) }
            _ => rejected.push(f[0].clone()),
        }
    }
    Ok(Parsed { items, rejected })
}
pub fn load_coverage(r: &Path) -> Result<Parsed<Coverage>, String> {
    parse_coverage(&read_text(&coverage_path(r))?)
}
fn coverage_line(c: &Coverage) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", c.id, c.site, esc(&c.place), rate_word(c.p_in), rate_word(c.p_out), rate_word(c.a_in), rate_word(c.a_out), c.at, esc(&c.by), esc(&c.note), c.recorded)
}
const COVERAGE_HEADER: &str = "# format: coverage-log 1\n# id, site, place, primary heard, primary sent, alternate heard, alternate sent, time, by, note, recorded (0 to 5; never changed)\n";

pub fn check_coverage(existing: &[Coverage], sites: &[String], c: &Coverage) -> Option<String> {
    if existing.iter().any(|x| x.id == c.id) { return Some("That entry number is already used.".to_string()); }
    if !sites.iter().any(|s| s == &c.site) { return Some("That site is not on the list.".to_string()); }
    if c.place.trim().is_empty() || c.place.len() > 80 { return Some("Name the place that was tested.".to_string()); }
    if c.by.trim().is_empty() { return Some("Say who tested it.".to_string()); }
    if parse_instant(&c.at).is_none() { return Some("The time could not be read.".to_string()); }
    if [c.p_in, c.p_out, c.a_in, c.a_out].iter().all(|x| x.is_none()) { return Some("Give at least one rating from 0 to 5.".to_string()); }
    None
}
pub fn append_coverage(r: &Path, c: &Coverage) -> Result<(), String> {
    let sites: Vec<String> = store::load_sites(r)?.items.into_iter().map(|s| s.id).collect();
    let log = load_coverage(r)?;
    if !log.rejected.is_empty() { return Err("Some lines in the coverage log could not be read, so nothing was added. Look at those first.".to_string()); }
    if let Some(p) = check_coverage(&log.items, &sites, c) { return Err(p); }
    store::append_line(r, &coverage_path(r), COVERAGE_HEADER, &coverage_line(c), |t| parse_coverage(t).map(|_| ()))
}
/// The newest test of each place at a site.
pub fn latest_coverage<'a>(all: &'a [Coverage], site: &str) -> Vec<&'a Coverage> {
    let mut out: Vec<&Coverage> = Vec::new();
    for c in all.iter().filter(|c| c.site == site) {
        match out.iter().position(|o| o.place.eq_ignore_ascii_case(&c.place)) {
            Some(i) => { if parse_instant(&c.at) >= parse_instant(&out[i].at) { out[i] = c } }
            None => out.push(c),
        }
    }
    out
}
fn worst(c: &Coverage) -> Option<u8> {
    [c.p_in, c.p_out, c.a_in, c.a_out].iter().flatten().copied().min()
}

// ---------- operator training (plan section 15) ----------

pub const SKILLS: &[&str] = &[
    "Turn the radio on and off",
    "Adjust volume and channel",
    "Turn privacy codes off",
    "Make and answer a routine call",
    "Report an emergency in plain words",
    "Change to the back-up channel",
    "Spot a low battery",
    "Charge the radio by USB",
    "Use the hand-crank back-up",
    "Receive a weather broadcast",
    "Follow the lost-contact steps",
    "Write a log entry",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Training {
    pub id: String,
    pub person: String,
    /// 1 to 12, the number of the skill in the plan
    pub skill: u8,
    /// pass or practice
    pub result: String,
    pub at: String,
    pub by: String,
    pub note: String,
}
pub fn training_path(r: &Path) -> PathBuf {
    store::dir(r).join("radio-training.txt")
}
pub fn parse_training(text: &str) -> Result<Parsed<Training>, String> {
    let (ok, mut rejected) = rows(text, "radio-training", 7)?;
    let mut items: Vec<Training> = Vec::new();
    for f in ok {
        let t = Training { id: f[0].clone(), person: unesc(&f[1]), skill: f[2].parse().unwrap_or(0), result: f[3].clone(), at: f[4].clone(), by: unesc(&f[5]), note: unesc(&f[6]) };
        if (1..=12).contains(&t.skill) && matches!(t.result.as_str(), "pass" | "practice") && !t.person.is_empty() && !items.iter().any(|x| x.id == t.id) { items.push(t) } else { rejected.push(f[0].clone()) }
    }
    Ok(Parsed { items, rejected })
}
pub fn load_training(r: &Path) -> Result<Parsed<Training>, String> {
    parse_training(&read_text(&training_path(r))?)
}
fn training_line(t: &Training) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\n", t.id, esc(&t.person), t.skill, t.result, t.at, esc(&t.by), esc(&t.note))
}
const TRAINING_HEADER: &str = "# format: radio-training 1\n# id, person, skill (1 to 12), result (pass or practice), time, signed by, note (never changed; the newest line for a person and skill counts)\n";

pub fn append_training(r: &Path, t: &Training) -> Result<(), String> {
    let log = load_training(r)?;
    if !log.rejected.is_empty() { return Err("Some lines in the training record could not be read, so nothing was added. Look at those first.".to_string()); }
    if log.items.iter().any(|x| x.id == t.id) { return Err("That entry number is already used.".to_string()); }
    if t.person.trim().is_empty() || t.person.len() > 80 { return Err("Say whose training this is.".to_string()); }
    if !(1..=12).contains(&t.skill) { return Err("That is not one of the twelve skills.".to_string()); }
    if !matches!(t.result.as_str(), "pass" | "practice") { return Err("The result has to be pass or practice.".to_string()); }
    if t.by.trim().is_empty() { return Err("Say who watched and signed it off.".to_string()); }
    if t.by.trim().eq_ignore_ascii_case(t.person.trim()) { return Err("Someone else has to watch and sign off a skill.".to_string()); }
    if parse_instant(&t.at).is_none() { return Err("The time could not be read.".to_string()); }
    store::append_line(r, &training_path(r), TRAINING_HEADER, &training_line(t), |x| parse_training(x).map(|_| ()))
}
pub fn new_training(existing: &[Training], person: &str, skill: u8, result: &str, at: &str, by: &str, note: &str) -> Training {
    Training { id: crate::ops::next_id('t', existing.iter().map(|e| e.id.clone())), person: clean(person, 80), skill, result: result.into(), at: at.into(), by: clean(by, 80), note: clean(note, 300) }
}

/// For each person with any training record: how many of the twelve skills currently stand as passed, and which need practice.
pub fn standing(all: &[Training]) -> Vec<(String, Vec<Option<bool>>)> {
    let mut people: Vec<String> = Vec::new();
    for t in all { if !people.iter().any(|p| p.eq_ignore_ascii_case(&t.person)) { people.push(t.person.clone()) } }
    people.into_iter().map(|p| {
        let mut sk: Vec<Option<bool>> = vec![None; 12];
        for t in all.iter().filter(|t| t.person.eq_ignore_ascii_case(&p)) {
            sk[(t.skill - 1) as usize] = Some(t.result == "pass"); // later lines come later in the file
        }
        (p, sk)
    }).collect()
}
/// True when this person has passed all twelve. A person with no record at all is simply not tracked (None).
pub fn qualified(all: &[Training], person: &str) -> Option<bool> {
    standing(all).into_iter().find(|(p, _)| p.eq_ignore_ascii_case(person)).map(|(_, sk)| sk.iter().all(|s| *s == Some(true)))
}

// ---------- the channel plan (plan section 3) ----------

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub site: String,
    pub channel: String,
    pub code: String,
    /// primary, alternate, work or other
    pub role: String,
    pub purpose: String,
}
pub fn channels_path(r: &Path) -> PathBuf {
    store::dir(r).join("channels.txt")
}
/// What the plan says before a site has its own: CH 01, CH 04 and CH 05, privacy codes off.
pub fn default_channels(site: &str) -> Vec<Channel> {
    vec![
        Channel { site: site.into(), channel: "01".into(), code: "off".into(), role: "primary".into(), purpose: "Routine communications, watch reports, emergencies".into() },
        Channel { site: site.into(), channel: "04".into(), code: "off".into(), role: "alternate".into(), purpose: "Back-up when the primary channel cannot be used".into() },
        Channel { site: site.into(), channel: "05".into(), code: "off".into(), role: "work".into(), purpose: "Non-urgent logistics while someone still monitors the primary".into() },
    ]
}
pub fn parse_channels(text: &str) -> Result<Parsed<Channel>, String> {
    check_format(text, "security-channels", 1)?;
    let mut p = Parsed::default();
    for l in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let f: Vec<&str> = l.split('\t').collect();
        let c = (f.len() == 5 && valid_id(f[0]) && !f[1].is_empty() && f[1].len() <= 8 && f[2].len() <= 8 && matches!(f[3], "primary" | "alternate" | "work" | "other")).then(|| Channel { site: f[0].into(), channel: f[1].into(), code: f[2].into(), role: f[3].into(), purpose: unesc(f[4]) });
        match c { Some(c) => p.items.push(c), None => p.rejected.push(l.to_string()) }
    }
    Ok(p)
}
pub fn load_channels(r: &Path) -> Result<Parsed<Channel>, String> {
    parse_channels(&read_text(&channels_path(r))?)
}
/// The channels for one site (the plan's defaults if it has none of its own), in the order saved.
pub fn channels_for(all: &[Channel], site: &str) -> (Vec<Channel>, bool) {
    let mine: Vec<Channel> = all.iter().filter(|c| c.site == site).cloned().collect();
    if mine.is_empty() { (default_channels(site), true) } else { (mine, false) }
}
/// Save one site's channels, keeping every other site's. At least one primary, at most one primary and one alternate.
pub fn save_channels(r: &Path, site: &str, list: &[Channel]) -> Result<(), String> {
    let sites: Vec<String> = store::load_sites(r)?.items.into_iter().map(|s| s.id).collect();
    if !sites.iter().any(|s| s == site) { return Err("That site is not on the list.".to_string()); }
    if list.iter().filter(|c| c.role == "primary").count() != 1 { return Err("Every site needs exactly one primary channel.".to_string()); }
    if list.iter().filter(|c| c.role == "alternate").count() > 1 { return Err("A site can have only one alternate channel.".to_string()); }
    let cur = load_channels(r)?;
    if !cur.rejected.is_empty() { return Err("Some lines in the channel plan could not be read, so nothing was changed.".to_string()); }
    let mut t = String::from("# format: security-channels 1\n# site, channel, privacy code, role, purpose\n");
    for c in cur.items.iter().filter(|c| c.site != site).chain(list.iter()) {
        if !valid_id(&c.site) || c.channel.trim().is_empty() || c.channel.len() > 8 || c.code.len() > 8 || !matches!(c.role.as_str(), "primary" | "alternate" | "work" | "other") { return Err("A channel line could not be used.".to_string()); }
        t.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", c.site, c.channel.trim(), c.code.trim(), c.role, esc(&clean(&c.purpose, 200))));
    }
    fsio::backup_then_write(&channels_path(r), &store::dir(r).join("backups"), t.as_bytes()).map_err(|_| "The channel plan could not be saved.".to_string())
}

// ---------- what needs a look ----------

fn clock(at: &str) -> String {
    at.get(11..16).unwrap_or("").to_string()
}

/// Plain-words lines for one site from the communications records.
pub fn looks(site: &str, comms: &[CommsEntry], coverage: &[Coverage]) -> Vec<String> {
    let mut out = Vec::new();
    for e in open_items(comms).into_iter().filter(|e| e.site == site && WATCHED.contains(&e.category.as_str())) {
        let cat = CATEGORIES.iter().find(|c| c.0 == e.category).map(|c| c.1).unwrap_or("");
        out.push(format!("Open in the communications log: {} ({}, {}). {}", cat, if e.station.is_empty() { "no station" } else { e.station.as_str() }, clock(&e.at), e.event));
    }
    for c in latest_coverage(coverage, site) {
        if let Some(w) = worst(c) {
            if w <= 2 {
                out.push(format!("Radio is weak at {} (rated {}). People there need another way to check in.", c.place, w));
            }
        }
    }
    out
}

/// People on a watch that is scheduled or under way who are tracked in the training record but have not passed all
/// twelve skills. People with no record at all are not mentioned: the program cannot tell "untrained" from "not written down".
pub fn training_looks(site: &str, watches: &[crate::security_schedule::Watch], training: &[Training]) -> Vec<String> {
    use crate::security_schedule::WatchState::*;
    let mut seen: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for w in watches.iter().filter(|w| w.site == site && matches!(w.state, Scheduled | Acknowledged | OnDuty)) {
        if seen.iter().any(|p| p.eq_ignore_ascii_case(&w.person)) { continue }
        seen.push(w.person.clone());
        if qualified(training, &w.person) == Some(false) {
            out.push(format!("{} is on the watchbill but has not passed all twelve radio skills yet.", w.person));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-comms-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        store::save_sites(&p, &[crate::security::Site { id: "camp".into(), name: "Camp".into(), ..Default::default() }]).unwrap();
        p
    }
    const T: &str = "2026-11-08T10:00-05:00";
    fn log(r: &Path, cat: &str, event: &str, status: &str, closes: &str) -> Result<(), String> {
        let ex = load_comms(r)?.items;
        append_comms(r, &new_entry(&ex, "camp", T, T, "Bravo", "Base", cat, event, "Radio checked", status, closes))
    }

    #[test]
    fn the_log_takes_the_eight_categories_and_open_items_close_by_a_later_line() {
        let r = tmp("log");
        for (c, _) in CATEGORIES { log(&r, c, "Something", "complete", "").unwrap(); }
        assert_eq!(load_comms(&r).unwrap().items.len(), 8);
        log(&r, "equipment-failure", "Radio 3 will not charge", "open", "").unwrap();
        let all = load_comms(&r).unwrap().items;
        assert_eq!(open_items(&all).len(), 1);
        assert!(looks("camp", &all, &[])[0].contains("Radio 3 will not charge"));
        // closing: only an open entry, only once
        assert!(log(&r, "equipment-failure", "Fixed", "closed", "c1").unwrap_err().contains("not open"));
        assert!(log(&r, "equipment-failure", "Fixed", "closed", "c99").unwrap_err().contains("no such"));
        log(&r, "equipment-failure", "Replaced the cable", "closed", "c9").unwrap();
        let all = load_comms(&r).unwrap().items;
        assert!(open_items(&all).is_empty() && looks("camp", &all, &[]).is_empty());
        assert!(log(&r, "equipment-failure", "Again", "closed", "c9").unwrap_err().contains("not open"));
        // lines are never changed
        assert!(fs::read_to_string(comms_path(&r)).unwrap().lines().filter(|l| !l.starts_with('#')).count() == 10);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn bad_log_entries_are_refused_and_a_damaged_log_is_never_added_to() {
        let r = tmp("badlog");
        assert!(log(&r, "gossip", "x", "open", "").unwrap_err().contains("categories"));
        assert!(log(&r, "channel-change", "  ", "open", "").unwrap_err().contains("what happened"));
        assert!(log(&r, "channel-change", "x", "maybe", "").is_err());
        let ex = vec![];
        let mut e = new_entry(&ex, "nowhere", T, T, "Bravo", "", "channel-change", "x", "", "complete", "");
        assert!(append_comms(&r, &e).unwrap_err().contains("site"));
        e.site = "camp".into();
        e.at = "yesterday".into();
        assert!(append_comms(&r, &e).unwrap_err().contains("time"));
        fs::create_dir_all(store::dir(&r)).unwrap();
        fs::write(comms_path(&r), "# format: comms-log 1\ngarbage\n").unwrap();
        assert!(log(&r, "channel-change", "x", "complete", "").unwrap_err().contains("could not be read"));
        fs::write(comms_path(&r), "# format: comms-log 7\n").unwrap();
        assert!(log(&r, "channel-change", "x", "complete", "").is_err());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn coverage_uses_the_zero_to_five_scale_and_a_weak_place_says_so_until_it_is_retested() {
        let r = tmp("cov");
        let add = |place: &str, pi: &str, when: &str| {
            let ex = load_coverage(&r).unwrap().items;
            let c = Coverage { id: crate::ops::next_id('v', ex.iter().map(|e| e.id.clone())), site: "camp".into(), place: place.into(), p_in: rating(pi).unwrap_or(None), p_out: rating(pi).unwrap_or(None), a_in: None, a_out: None, at: when.into(), recorded: T.into(), by: "Sam".into(), note: String::new() };
            append_coverage(&r, &c)
        };
        add("Base", "5", T).unwrap();
        add("North boundary", "1", T).unwrap();
        assert!(add("Creek", "9", T).is_err());
        let all = load_coverage(&r).unwrap().items;
        let l = looks("camp", &[], &all);
        assert_eq!(l.len(), 1);
        assert!(l[0].contains("North boundary") && l[0].contains("another way to check in"));
        // retested better on a later day: the old result stays in the file, the warning goes
        add("north boundary", "4", "2026-11-09T10:00-05:00").unwrap();
        let all = load_coverage(&r).unwrap().items;
        assert_eq!(all.len(), 3);
        assert!(looks("camp", &[], &all).is_empty());
        assert_eq!(latest_coverage(&all, "camp").len(), 2);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn training_needs_someone_else_to_sign_off_and_the_newest_line_counts() {
        let r = tmp("train");
        let add = |person: &str, skill: u8, res: &str, by: &str| {
            let ex = load_training(&r).unwrap().items;
            append_training(&r, &new_training(&ex, person, skill, res, T, by, ""))
        };
        assert!(add("Sam", 1, "pass", "Sam").unwrap_err().contains("Someone else"));
        assert!(add("Sam", 13, "pass", "Dana").is_err());
        assert!(add("Sam", 1, "good", "Dana").is_err());
        for s in 1..=12 { add("Dana", s, "pass", "Sam").unwrap(); }
        assert_eq!(qualified(&load_training(&r).unwrap().items, "dana"), Some(true));
        add("Dana", 9, "practice", "Sam").unwrap(); // the hand-crank went badly this time
        let all = load_training(&r).unwrap().items;
        assert_eq!(qualified(&all, "Dana"), Some(false));
        add("Dana", 9, "pass", "Sam").unwrap();
        assert_eq!(qualified(&load_training(&r).unwrap().items, "Dana"), Some(true));
        assert_eq!(qualified(&all, "Nobody"), None);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn only_tracked_people_who_are_not_yet_qualified_are_flagged_on_the_watchbill() {
        use crate::security_schedule::{Watch, WatchState};
        let w = |p: &str, st: WatchState| Watch { id: format!("w-{}", p), site: "camp".into(), person: p.into(), role: "watch".into(), start: T.into(), end: T.into(), state: st, ack_at: String::new(), entered_at: T.into(), notes: String::new() };
        let tr = vec![Training { id: "t1".into(), person: "Sam".into(), skill: 1, result: "pass".into(), at: T.into(), by: "Dana".into(), note: String::new() }];
        let l = training_looks("camp", &[w("Sam", WatchState::Scheduled), w("Kim", WatchState::Scheduled), w("Sam", WatchState::Acknowledged), w("Lee", WatchState::Proposed)], &tr);
        assert_eq!(l.len(), 1, "{:?}", l);
        assert!(l[0].contains("Sam"));
        assert!(training_looks("camp", &[w("Sam", WatchState::Completed)], &tr).is_empty());
    }

    #[test]
    fn channels_default_to_the_plan_and_save_per_site_with_rules() {
        let r = tmp("chan");
        let (d, is_default) = channels_for(&load_channels(&r).unwrap().items, "camp");
        assert!(is_default && d[0].channel == "01" && d[0].role == "primary" && d[1].channel == "04");
        let mut mine = d.clone();
        mine[0].channel = "03".into();
        save_channels(&r, "camp", &mine).unwrap();
        let (got, is_default) = channels_for(&load_channels(&r).unwrap().items, "camp");
        assert!(!is_default && got[0].channel == "03");
        assert!(save_channels(&r, "camp", &[]).is_err());
        let mut two = mine.clone();
        two[2].role = "primary".into();
        assert!(save_channels(&r, "camp", &two).is_err());
        let mut alt = mine.clone();
        alt[2].role = "alternate".into();
        assert!(save_channels(&r, "camp", &alt).is_err());
        assert!(save_channels(&r, "nowhere", &mine).is_err());
        // a newer file is left alone
        fs::write(channels_path(&r), "# format: security-channels 4\n").unwrap();
        assert!(save_channels(&r, "camp", &mine).is_err());
        let _ = fs::remove_dir_all(&r);
    }
}
