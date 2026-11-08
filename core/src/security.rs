//! Security Operations, part 1: sites, posture and the instants everything is timed with.
//! Design: `docs/44-security-operations-architecture.md` (sections 4, 5, 10, 14). Plain logic, no screen.
//!
//! Rules this file keeps:
//!  * a posture is **Normal, Elevated, Emergency or Unknown**; Unknown is the default and never means safe;
//!  * silence never lowers a posture: an overdue check only marks the entry "check due";
//!  * the posture file is a log that is only ever added to; the current posture is the newest line;
//!  * a summary of several sites never says "all Normal" unless every site is Normal and none is overdue;
//!  * files are tab-separated plain text with a `# format:` line, like `tasks.txt`, so they outlive the program.

use crate::calendar;
use crate::ops::{esc, unesc};

// ---------- instants ----------

/// A moment as the files write it: `2026-11-08T06:00-05:00` (or `...Z`). The offset is required, so a schedule
/// stays right across a clock change. Returned as minutes since 1970-01-01 00:00 UTC.
pub fn parse_instant(s: &str) -> Option<i64> {
    let s = s.trim();
    if !s.is_ascii() || s.len() < 17 || s.as_bytes()[10] != b'T' || s.as_bytes()[13] != b':' {
        return None;
    }
    let days = calendar::parse_date(&s[..10])?;
    let (h, m) = (s[11..13].parse::<i64>().ok()?, s[14..16].parse::<i64>().ok()?);
    if h > 23 || m > 59 {
        return None;
    }
    let off = parse_offset(&s[16..])?;
    Some(days * 1440 + h * 60 + m - off)
}

/// `Z` or `+HH:MM` / `-HH:MM` as minutes east of UTC.
fn parse_offset(s: &str) -> Option<i64> {
    if s == "Z" {
        return Some(0);
    }
    if s.len() != 6 || s.as_bytes()[3] != b':' {
        return None;
    }
    let sign = match s.as_bytes()[0] { b'+' => 1, b'-' => -1, _ => return None };
    let (h, m) = (s[1..3].parse::<i64>().ok()?, s[4..6].parse::<i64>().ok()?);
    if h > 14 || m > 59 {
        return None;
    }
    Some(sign * (h * 60 + m))
}

/// The other way: UTC minutes written in a given offset (minutes east of UTC).
pub fn format_instant(utc_minutes: i64, offset_minutes: i64) -> String {
    let local = utc_minutes + offset_minutes;
    let (y, mo, d) = calendar::civil_from_days(local.div_euclid(1440));
    let r = local.rem_euclid(1440);
    let off = if offset_minutes == 0 {
        "Z".to_string()
    } else {
        let a = offset_minutes.abs();
        format!("{}{:02}:{:02}", if offset_minutes < 0 { '-' } else { '+' }, a / 60, a % 60)
    };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}{}", y, mo, d, r / 60, r % 60, off)
}

/// The offset a written instant carries, in minutes east of UTC.
pub fn instant_offset(s: &str) -> Option<i64> {
    let s = s.trim();
    if !s.is_ascii() || s.len() < 17 {
        return None;
    }
    parse_offset(&s[16..])
}

// ---------- ids and text ----------

/// A site, watch or record id: lower-case letters, digits and dashes, up to 40 characters. Ids are used in file
/// names and links, so nothing else is accepted.
pub fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 40 && !s.starts_with('-') && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Make an id out of a name ("Evacuation destination" -> "evacuation-destination"); empty if nothing usable is left.
pub fn slug(name: &str) -> String {
    let mut o = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            o.push(c);
        } else if !o.is_empty() && !o.ends_with('-') {
            o.push('-');
        }
    }
    while o.ends_with('-') {
        o.pop();
    }
    o.truncate(40);
    while o.ends_with('-') {
        o.pop();
    }
    o
}

/// Longest free text a record keeps. Imported or pasted text is untrusted, so it is cut, never trusted.
const MAX_TEXT: usize = 2000;

fn clean_text(s: &str) -> String {
    let t: String = s.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').take(MAX_TEXT).collect();
    t.trim().to_string()
}

/// The newest `# format: <kind> <n>` line, checked so a newer file is refused instead of guessed at.
/// No line at all counts as version 1.
pub fn check_format(text: &str, kind: &str, supported: u32) -> Result<(), String> {
    for l in text.lines() {
        if let Some(rest) = l.strip_prefix("# format:") {
            let mut it = rest.split_whitespace();
            if it.next() != Some(kind) {
                return Err(format!("This file is not a {} file.", kind));
            }
            let v: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(1);
            return if v > supported { Err("This file was written by a newer version of the program. It was left alone.".to_string()) } else { Ok(()) };
        }
    }
    Ok(())
}

/// What a reader found: the good records, and the lines it could not use (kept so a screen can say so and
/// nothing is silently thrown away).
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed<T> {
    pub items: Vec<T>,
    pub rejected: Vec<String>,
}

impl<T> Default for Parsed<T> {
    fn default() -> Self {
        Parsed { items: Vec::new(), rejected: Vec::new() }
    }
}

fn data_lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
}

// ---------- posture ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posture {
    Normal,
    Elevated,
    Emergency,
    Unknown,
}

impl Posture {
    pub fn word(self) -> &'static str {
        match self {
            Posture::Normal => "normal",
            Posture::Elevated => "elevated",
            Posture::Emergency => "emergency",
            Posture::Unknown => "unknown",
        }
    }
    /// Anything that is not one of the three set words is Unknown.
    pub fn from_word(s: &str) -> Posture {
        match s.trim().to_lowercase().as_str() {
            "normal" => Posture::Normal,
            "elevated" => Posture::Elevated,
            "emergency" => Posture::Emergency,
            _ => Posture::Unknown,
        }
    }
}

// ---------- sites ----------

pub const SITES_FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Site {
    pub id: String,
    pub name: String,
    /// IANA zone name such as `America/New_York`; only a label here, times carry their own offset.
    pub tz: String,
    /// Id of a saved map from the Maps screen, or empty.
    pub map_id: String,
    pub notes: String,
}

pub fn parse_sites(text: &str) -> Result<Parsed<Site>, String> {
    check_format(text, "security-sites", SITES_FORMAT)?;
    let mut p = Parsed::default();
    for l in data_lines(text) {
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let name = clean_text(&unesc(get(1)));
        if !valid_id(get(0)) || name.is_empty() || p.items.iter().any(|s: &Site| s.id == get(0)) {
            p.rejected.push(l.to_string());
            continue;
        }
        p.items.push(Site { id: get(0).to_string(), name, tz: clean_text(get(2)), map_id: get(3).trim().to_string(), notes: clean_text(&unesc(get(4))) });
    }
    Ok(p)
}

pub fn serialize_sites(v: &[Site]) -> String {
    let mut o = format!("# format: security-sites {}\n# id\tname\ttimezone\tmap_id\tnotes\n", SITES_FORMAT);
    for s in v {
        o.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", s.id, esc(&s.name), s.tz, s.map_id, esc(&s.notes)));
    }
    o
}

// ---------- the posture log ----------

pub const POSTURE_FORMAT: u32 = 1;

/// One line of the posture log. Lines are added, never edited.
#[derive(Debug, Clone, PartialEq)]
pub struct PostureEntry {
    pub site: String,
    pub posture: Posture,
    /// Who set it (a name from the Family page, or the host).
    pub set_by: String,
    pub reason: String,
    /// "operator" for a person's decision; anything else (an imported alert, say) is only a source note.
    pub source: String,
    /// When it took effect.
    pub at: String,
    /// When someone last checked it still holds, or empty.
    pub verified_at: String,
    /// When it must be looked at again, or empty.
    pub review_by: String,
}

pub fn parse_posture_log(text: &str) -> Result<Parsed<PostureEntry>, String> {
    check_format(text, "security-posture", POSTURE_FORMAT)?;
    let mut p = Parsed::default();
    for l in data_lines(text) {
        let f: Vec<&str> = l.split('\t').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let when_ok = |s: &str| s.is_empty() || parse_instant(s).is_some();
        if !valid_id(get(0)) || parse_instant(get(5)).is_none() || !when_ok(get(6)) || !when_ok(get(7)) {
            p.rejected.push(l.to_string());
            continue;
        }
        p.items.push(PostureEntry {
            site: get(0).to_string(),
            posture: Posture::from_word(get(1)),
            set_by: clean_text(&unesc(get(2))),
            reason: clean_text(&unesc(get(3))),
            source: clean_text(get(4)),
            at: get(5).trim().to_string(),
            verified_at: get(6).trim().to_string(),
            review_by: get(7).trim().to_string(),
        });
    }
    Ok(p)
}

/// One log line, ready to add to the end of the file. The header is written once, when the file is new.
pub fn posture_line(e: &PostureEntry) -> String {
    format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", e.site, e.posture.word(), esc(&e.set_by), esc(&e.reason), e.source, e.at, e.verified_at, e.review_by)
}

pub fn posture_log_header() -> String {
    format!("# format: security-posture {}\n# site\tposture\tset_by\treason\tsource\tat\tverified_at\treview_by\n", POSTURE_FORMAT)
}

/// A site's current entry: the one with the latest `at`; when two share a moment, the one written later.
/// None when the site has never been assessed.
pub fn current_entry<'a>(log: &'a [PostureEntry], site: &str) -> Option<&'a PostureEntry> {
    let mut best: Option<&PostureEntry> = None;
    for e in log.iter().filter(|e| e.site == site) {
        if best.map(|b| parse_instant(&e.at) >= parse_instant(&b.at)).unwrap_or(true) {
            best = Some(e);
        }
    }
    best
}

/// A site's posture as the screen shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SiteStatus {
    pub site: String,
    pub posture: Posture,
    /// True when its review date has passed. The posture shown is still the last one set; it is not lowered.
    pub check_due: bool,
    /// True when no one has ever set a posture for it.
    pub never_assessed: bool,
}

pub fn site_status(log: &[PostureEntry], site: &str, now: i64) -> SiteStatus {
    match current_entry(log, site) {
        None => SiteStatus { site: site.to_string(), posture: Posture::Unknown, check_due: false, never_assessed: true },
        Some(e) => {
            let due = parse_instant(&e.review_by).map(|r| now >= r).unwrap_or(false);
            SiteStatus { site: site.to_string(), posture: e.posture, check_due: due, never_assessed: false }
        }
    }
}

/// What the Dashboard and Command show when there are several sites.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    pub sites: Vec<SiteStatus>,
    /// The one word for the whole picture: Emergency if any site is, else Elevated if any is, else Unknown if any
    /// site is Unknown or overdue, else Normal. No sites at all is Unknown.
    pub headline: Posture,
    pub unknown: usize,
    pub check_due: usize,
    /// True only when there is at least one site, every one is Normal, and none is overdue.
    pub all_normal_confirmed: bool,
}

pub fn overview(sites: &[Site], log: &[PostureEntry], now: i64) -> Overview {
    let st: Vec<SiteStatus> = sites.iter().map(|s| site_status(log, &s.id, now)).collect();
    let unknown = st.iter().filter(|s| s.posture == Posture::Unknown).count();
    let check_due = st.iter().filter(|s| s.check_due).count();
    let has = |p: Posture| st.iter().any(|s| s.posture == p);
    let headline = if st.is_empty() {
        Posture::Unknown
    } else if has(Posture::Emergency) {
        Posture::Emergency
    } else if has(Posture::Elevated) {
        Posture::Elevated
    } else if unknown > 0 || check_due > 0 {
        Posture::Unknown
    } else {
        Posture::Normal
    };
    Overview { all_normal_confirmed: !st.is_empty() && headline == Posture::Normal, sites: st, headline, unknown, check_due }
}

/// Check a new posture entry before it is added: a known site, an `at` that is a real moment, and a reason for
/// anything above Normal. Returns the plain-words problem, or None when it is fine.
pub fn check_new_entry(sites: &[Site], e: &PostureEntry) -> Option<String> {
    if !sites.iter().any(|s| s.id == e.site) {
        return Some("That site is not on the list.".to_string());
    }
    if parse_instant(&e.at).is_none() {
        return Some("The time needs a date, a time and the time zone offset, like 2026-11-08T06:00-05:00.".to_string());
    }
    for w in [&e.verified_at, &e.review_by] {
        if !w.is_empty() && parse_instant(w).is_none() {
            return Some("One of the times could not be read.".to_string());
        }
    }
    if matches!(e.posture, Posture::Elevated | Posture::Emergency) && e.reason.trim().is_empty() {
        return Some("Say why, so the next person on watch knows.".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(id: &str) -> Site {
        Site { id: id.into(), name: id.into(), ..Default::default() }
    }
    fn entry(site: &str, p: Posture, at: &str, review: &str) -> PostureEntry {
        PostureEntry { site: site.into(), posture: p, set_by: "Dad".into(), reason: "r".into(), source: "operator".into(), at: at.into(), verified_at: String::new(), review_by: review.into() }
    }
    const NOW: &str = "2026-11-08T12:00-05:00";

    #[test]
    fn instants_round_trip_and_respect_offsets() {
        let a = parse_instant("2026-11-08T06:00-05:00").unwrap();
        let b = parse_instant("2026-11-08T11:00Z").unwrap();
        assert_eq!(a, b);
        assert_eq!(format_instant(a, -300), "2026-11-08T06:00-05:00");
        assert_eq!(format_instant(a, 0), "2026-11-08T11:00Z");
        assert_eq!(format_instant(a, 330), "2026-11-08T16:30+05:30");
        assert_eq!(instant_offset("2026-11-08T06:00-05:00"), Some(-300));
    }

    #[test]
    fn instants_refuse_junk() {
        for bad in ["", "2026-11-08", "2026-11-08T06:00", "2026-13-08T06:00Z", "2026-02-30T06:00Z", "2026-11-08T24:00Z", "2026-11-08T06:60Z", "2026-11-08 06:00Z", "2026-11-08T06:00+25:00", "2026-11-08T06:00é05:00"] {
            assert!(parse_instant(bad).is_none(), "{}", bad);
        }
    }

    #[test]
    fn clock_change_keeps_the_real_length() {
        // US clocks went back on 2026-11-01: midnight EDT to midnight EST the next day is 25 hours apart.
        let a = parse_instant("2026-11-01T00:00-04:00").unwrap();
        let b = parse_instant("2026-11-02T00:00-05:00").unwrap();
        assert_eq!(b - a, 25 * 60);
    }

    #[test]
    fn ids_and_slugs() {
        assert!(valid_id("home") && valid_id("camp-2") && !valid_id("") && !valid_id("-x") && !valid_id("Home") && !valid_id("a/b") && !valid_id("../x"));
        assert!(!valid_id(&"a".repeat(41)));
        assert_eq!(slug("Evacuation destination"), "evacuation-destination");
        assert_eq!(slug("  Camp #2!! "), "camp-2");
        assert_eq!(slug("???"), "");
        assert!(valid_id(&slug(&"x ".repeat(60))));
    }

    #[test]
    fn unknown_is_the_default_and_the_word_for_anything_odd() {
        assert_eq!(Posture::from_word(""), Posture::Unknown);
        assert_eq!(Posture::from_word("safe"), Posture::Unknown);
        assert_eq!(Posture::from_word(" Elevated "), Posture::Elevated);
        let st = site_status(&[], "home", parse_instant(NOW).unwrap());
        assert_eq!(st.posture, Posture::Unknown);
        assert!(st.never_assessed);
    }

    #[test]
    fn newest_entry_wins_and_equal_times_take_the_later_line() {
        let log = vec![
            entry("home", Posture::Normal, "2026-11-08T08:00-05:00", ""),
            entry("home", Posture::Elevated, "2026-11-08T09:00-05:00", ""),
            entry("home", Posture::Emergency, "2026-11-08T09:00-05:00", ""),
            entry("camp", Posture::Normal, "2026-11-08T10:00-05:00", ""),
        ];
        assert_eq!(current_entry(&log, "home").unwrap().posture, Posture::Emergency);
        // an older line written last does not beat a newer moment
        let mut log2 = log.clone();
        log2.push(entry("home", Posture::Normal, "2026-11-08T07:00-05:00", ""));
        assert_eq!(current_entry(&log2, "home").unwrap().posture, Posture::Emergency);
    }

    #[test]
    fn silence_never_lowers_a_posture() {
        let log = vec![entry("home", Posture::Elevated, "2026-11-08T08:00-05:00", "2026-11-08T10:00-05:00")];
        let st = site_status(&log, "home", parse_instant(NOW).unwrap());
        assert_eq!(st.posture, Posture::Elevated);
        assert!(st.check_due);
    }

    #[test]
    fn overview_never_claims_all_normal_without_proof() {
        let now = parse_instant(NOW).unwrap();
        let sites = vec![site("home"), site("camp")];
        // only one site assessed
        let log = vec![entry("home", Posture::Normal, "2026-11-08T08:00-05:00", "")];
        let o = overview(&sites, &log, now);
        assert_eq!(o.headline, Posture::Unknown);
        assert!(!o.all_normal_confirmed);
        assert_eq!(o.unknown, 1);
        // both assessed, one overdue
        let log = vec![entry("home", Posture::Normal, "2026-11-08T08:00-05:00", "2026-11-08T09:00-05:00"), entry("camp", Posture::Normal, "2026-11-08T08:00-05:00", "")];
        let o = overview(&sites, &log, now);
        assert_eq!(o.headline, Posture::Unknown);
        assert_eq!(o.check_due, 1);
        // both fine
        let log = vec![entry("home", Posture::Normal, "2026-11-08T08:00-05:00", ""), entry("camp", Posture::Normal, "2026-11-08T08:00-05:00", "")];
        let o = overview(&sites, &log, now);
        assert!(o.all_normal_confirmed);
        // one elevated beats unknown and normal; emergency beats elevated
        let log = vec![entry("home", Posture::Elevated, "2026-11-08T08:00-05:00", "")];
        assert_eq!(overview(&sites, &log, now).headline, Posture::Elevated);
        let log = vec![entry("home", Posture::Elevated, "2026-11-08T08:00-05:00", ""), entry("camp", Posture::Emergency, "2026-11-08T08:00-05:00", "")];
        assert_eq!(overview(&sites, &log, now).headline, Posture::Emergency);
        // no sites is not "normal"
        let o = overview(&[], &[], now);
        assert_eq!(o.headline, Posture::Unknown);
        assert!(!o.all_normal_confirmed);
    }

    #[test]
    fn sites_round_trip_and_keep_bad_lines() {
        let sites = vec![Site { id: "home".into(), name: "Home".into(), tz: "America/New_York".into(), map_id: "m1".into(), notes: "Back gate\nsticks".into() }];
        let text = serialize_sites(&sites);
        assert_eq!(parse_sites(&text).unwrap().items, sites);
        let damaged = format!("{}Bad Id\tX\t\t\t\nhome\tDuplicate\t\t\t\n\tno id\n", text);
        let p = parse_sites(&damaged).unwrap();
        assert_eq!(p.items, sites);
        assert_eq!(p.rejected.len(), 3);
    }

    #[test]
    fn newer_or_foreign_files_are_refused_not_guessed() {
        assert!(parse_sites("# format: security-sites 2\nhome\tHome\n").is_err());
        assert!(parse_sites("# format: security-posture 1\nhome\tHome\n").is_err());
        assert!(parse_sites("home\tHome\n").is_ok());
    }

    #[test]
    fn posture_log_round_trips_and_rejects_bad_times() {
        let e = PostureEntry { reason: "Strange\tcar\nseen".into(), ..entry("home", Posture::Elevated, "2026-11-08T09:00-05:00", "2026-11-08T15:00-05:00") };
        let text = format!("{}{}", posture_log_header(), posture_line(&e));
        let p = parse_posture_log(&text).unwrap();
        assert_eq!(p.items, vec![e]);
        let bad = format!("{}home\televated\tDad\tr\toperator\tyesterday\t\t\nhome/../x\tnormal\tDad\tr\toperator\t2026-11-08T09:00Z\t\t\n", text);
        let p = parse_posture_log(&bad).unwrap();
        assert_eq!(p.items.len(), 1);
        assert_eq!(p.rejected.len(), 2);
    }

    #[test]
    fn long_and_control_text_is_cut() {
        let long = "x".repeat(5000);
        let text = format!("home\tHome\t\t\t{}\x07\n", long);
        let p = parse_sites(&text).unwrap();
        assert_eq!(p.items[0].notes.len(), 2000);
        assert!(!p.items[0].notes.contains('\x07'));
    }

    #[test]
    fn new_entries_are_checked() {
        let sites = vec![site("home")];
        assert!(check_new_entry(&sites, &entry("home", Posture::Normal, NOW, "")).is_none());
        assert!(check_new_entry(&sites, &entry("nowhere", Posture::Normal, NOW, "")).is_some());
        assert!(check_new_entry(&sites, &entry("home", Posture::Normal, "soon", "")).is_some());
        let mut e = entry("home", Posture::Emergency, NOW, "");
        e.reason = " ".into();
        assert!(check_new_entry(&sites, &e).is_some());
        e.posture = Posture::Normal;
        assert!(check_new_entry(&sites, &e).is_none());
    }
}
