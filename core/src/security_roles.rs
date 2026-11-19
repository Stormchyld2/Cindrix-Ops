//! Role qualifications and equipment assignments: doc 44 section 13 (phase 4).
//!
//! A **role** is something a person has to be qualified for (watch stander, radio operator, patrol leader). The person
//! sets how long a qualification stays good (0 = it does not run out) and how many qualified people there should be.
//! A **qualification** says who earned it and when; it can be taken away with a reason. An **assignment** hands a piece
//! from the Gear list (`gear.rs`) to a person until it is handed back. Nothing is made up: no roles, qualifications or
//! assignments exist until the person writes them.
//!
//! One append-only log, `user/security/roles-log.txt`. A line is never changed; a change is a new line. A log that
//! cannot be read in full is never added to. Gear itself is never changed from here.

use crate::gear::Piece;
use crate::ops::{esc, next_id, unesc};
use crate::security::{format_instant, parse_instant, valid_id, Parsed};
use crate::security_comms::{clean, read_text, rows};
use crate::security_review::{date_days, today_days};
use crate::security_store as store;
use std::path::{Path, PathBuf};

/// A qualification that runs out within this many days is shown as running out.
pub const WARN_DAYS: i64 = 30;
const EVENTS: &[&str] = &["role", "qualify", "revoke", "issue", "return"];

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub id: String,
    pub at: String,
    pub by: String,
    pub ev: String,
    /// role: name. qualify and revoke: person. issue and return: gear id.
    pub a: String,
    /// role: site id ("" = every site). qualify and revoke: role name. issue and return: person.
    pub b: String,
    /// role: days a qualification stays good. qualify: date earned.
    pub c: String,
    /// role: how many qualified people there should be.
    pub d: String,
    pub note: String,
}

pub fn path(r: &Path) -> PathBuf {
    store::dir(r).join("roles-log.txt")
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}
fn gear_id_ok(s: &str) -> bool {
    s.len() > 1 && s.starts_with('g') && s[1..].parse::<u64>().is_ok()
}

pub fn parse(text: &str) -> Result<Parsed<Line>, String> {
    let (ok, mut rejected) = rows(text, "roles-log", 9)?;
    let mut items: Vec<Line> = Vec::new();
    for f in ok {
        let l = Line { id: f[0].clone(), at: f[2].clone(), by: unesc(&f[1]), ev: f[3].clone(), a: unesc(&f[4]), b: unesc(&f[5]), c: f[6].clone(), d: f[7].clone(), note: unesc(&f[8]) };
        let has_role = |name: &str| items.iter().any(|x| x.ev == "role" && same(&x.a, name));
        let shape = match l.ev.as_str() {
            "role" => !l.a.is_empty() && l.c.parse::<u32>().map(|n| n <= 3650).unwrap_or(false) && l.d.parse::<u32>().map(|n| n <= 99).unwrap_or(false) && (l.b.is_empty() || valid_id(&l.b)),
            "qualify" => !l.a.is_empty() && has_role(&l.b) && date_days(&l.c).is_some(),
            "revoke" => !l.a.is_empty() && has_role(&l.b) && !l.note.is_empty(),
            "issue" | "return" => gear_id_ok(&l.a) && !l.b.is_empty(),
            _ => false,
        };
        let good = shape && parse_instant(&l.at).is_some() && !l.by.is_empty() && !items.iter().any(|x| x.id == l.id);
        if good { items.push(l) } else { rejected.push(f[0].clone()) }
    }
    Ok(Parsed { items, rejected })
}
pub fn load(r: &Path) -> Result<Parsed<Line>, String> {
    parse(&read_text(&path(r))?)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Role {
    pub name: String,
    pub site: String,
    pub valid_days: u32,
    pub min_people: u32,
    pub note: String,
}

/// The latest definition of each role (a role written again replaces the old wording), oldest first.
pub fn roles(lines: &[Line]) -> Vec<Role> {
    let mut out: Vec<Role> = Vec::new();
    for l in lines.iter().filter(|l| l.ev == "role") {
        let r = Role { name: l.a.clone(), site: l.b.clone(), valid_days: l.c.parse().unwrap_or(0), min_people: l.d.parse().unwrap_or(0), note: l.note.clone() };
        match out.iter_mut().find(|x| same(&x.name, &l.a)) {
            Some(slot) => *slot = r,
            None => out.push(r),
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Current,
    RunningOut,
    Lapsed,
}
impl Standing {
    pub fn word(self) -> &'static str {
        match self {
            Standing::Current => "current",
            Standing::RunningOut => "running out",
            Standing::Lapsed => "lapsed",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Qual {
    pub person: String,
    pub role: String,
    pub earned: String,
    /// The last good day, or empty when it does not run out.
    pub until: String,
    pub days_left: Option<i64>,
    pub standing: Standing,
    pub by: String,
}

fn day_text(days: i64) -> String {
    // civil from days (Hinnant), the inverse of `date_days`
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

/// Who holds what: the newest of "qualify" or "revoke" for each person and role decides. How long it stays good is
/// taken from the role as it is written now.
pub fn quals(lines: &[Line], today: i64) -> Vec<Qual> {
    let defs = roles(lines);
    let mut out: Vec<Qual> = Vec::new();
    for l in lines.iter().filter(|l| l.ev == "qualify" || l.ev == "revoke") {
        out.retain(|q| !(same(&q.person, &l.a) && same(&q.role, &l.b)));
        if l.ev == "revoke" {
            continue;
        }
        let def = defs.iter().find(|r| same(&r.name, &l.b));
        let earned = date_days(&l.c).unwrap_or(today);
        let days = def.map(|r| r.valid_days).unwrap_or(0) as i64;
        let (until, left, standing) = if days == 0 {
            (String::new(), None, Standing::Current)
        } else {
            let last = earned + days;
            let left = last - today;
            (day_text(last), Some(left), if left < 0 { Standing::Lapsed } else if left <= WARN_DAYS { Standing::RunningOut } else { Standing::Current })
        };
        out.push(Qual { person: l.a.clone(), role: def.map(|r| r.name.clone()).unwrap_or_else(|| l.b.clone()), earned: l.c.clone(), until, days_left: left, standing, by: l.by.clone() });
    }
    out.sort_by(|a, b| a.role.to_lowercase().cmp(&b.role.to_lowercase()).then(a.person.to_lowercase().cmp(&b.person.to_lowercase())));
    out
}

/// How many people have a qualification that has not lapsed.
pub fn qualified_count(qs: &[Qual], role: &str) -> usize {
    qs.iter().filter(|q| same(&q.role, role) && q.standing != Standing::Lapsed).count()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Holding {
    pub gear: String,
    pub person: String,
    pub since: String,
    pub by: String,
    pub note: String,
}

/// Pieces that are handed out right now. A "return" removes the matching person's hold on that piece.
pub fn holdings(lines: &[Line]) -> Vec<Holding> {
    let mut out: Vec<Holding> = Vec::new();
    for l in lines {
        match l.ev.as_str() {
            "issue" => out.push(Holding { gear: l.a.clone(), person: l.b.clone(), since: l.at.clone(), by: l.by.clone(), note: l.note.clone() }),
            "return" => {
                if let Some(i) = out.iter().position(|h| h.gear == l.a && same(&h.person, &l.b)) {
                    out.remove(i);
                }
            }
            _ => {}
        }
    }
    out
}

/// "Needs a look" for one site: roles with too few qualified people, and qualifications that have run out or are about to.
pub fn looks(site: &str, lines: &[Line], today: i64) -> Vec<String> {
    let qs = quals(lines, today);
    let mut out = Vec::new();
    for r in roles(lines).iter().filter(|r| r.site.is_empty() || r.site == site) {
        let n = qualified_count(&qs, &r.name);
        if r.min_people > 0 && (n as u32) < r.min_people {
            out.push(format!("Only {} {} for the role \"{}\" (the aim is {}).", n, if n == 1 { "person is qualified" } else { "people are qualified" }, r.name, r.min_people));
        }
        for q in qs.iter().filter(|q| same(&q.role, &r.name)) {
            match q.standing {
                Standing::Lapsed => out.push(format!("{}'s \"{}\" qualification ran out on {}.", q.person, q.role, q.until)),
                Standing::RunningOut => out.push(format!("{}'s \"{}\" qualification runs out in {} day{} ({}).", q.person, q.role, q.days_left.unwrap_or(0), if q.days_left == Some(1) { "" } else { "s" }, q.until)),
                Standing::Current => {}
            }
        }
    }
    out
}

/// Lines that are about the handed-out gear, not about one site: a piece that is damaged, out of use or gone from the list.
pub fn assignment_looks(lines: &[Line], gear: &[Piece]) -> Vec<String> {
    let mut out = Vec::new();
    for h in holdings(lines) {
        match gear.iter().find(|g| g.id == h.gear) {
            None => out.push(format!("{} has a piece of gear ({}) that is no longer in the Gear list.", h.person, h.gear)),
            Some(g) if g.condition == "out" => out.push(format!("{} has {}, which is marked out of use.", h.person, g.name)),
            Some(g) if g.condition == "repair" => out.push(format!("{} has {}, which is marked as needing repair.", h.person, g.name)),
            _ => {}
        }
    }
    out
}

/// Add a line. `a`, `b`, `c`, `d` mean what the columns of `Line` say for each event. Returns the new line's id.
#[allow(clippy::too_many_arguments)]
pub fn add(r: &Path, gear: &[Piece], ev: &str, a: &str, b: &str, c: &str, d: &str, note: &str, by: &str, off: i64, now: i64) -> Result<String, String> {
    if !EVENTS.contains(&ev) {
        return Err("That is not something this record can do.".to_string());
    }
    let log = load(r)?;
    if !log.rejected.is_empty() {
        return Err("Some lines in the qualifications and gear-handout log could not be read, so nothing was added. Look at those first.".to_string());
    }
    let by = clean(by, 80);
    if by.is_empty() {
        return Err("Say who is writing this.".to_string());
    }
    let (a, b, c, d, note) = (clean(a, 120), clean(b, 120), c.trim().to_string(), d.trim().to_string(), clean(note, 600));
    let today = today_days(now, off);
    let defs = roles(&log.items);
    let mut b = b;
    let mut c = c;
    let mut d = d;
    match ev {
        "role" => {
            if a.is_empty() {
                return Err("Give the role a short name, like Radio operator.".to_string());
            }
            let days: u32 = if c.is_empty() { 0 } else { c.parse().map_err(|_| "Say how many days a qualification stays good, or 0 if it does not run out.".to_string())? };
            let min: u32 = if d.is_empty() { 0 } else { d.parse().map_err(|_| "Say how many qualified people there should be, or 0.".to_string())? };
            if days > 3650 || min > 99 {
                return Err("Those numbers are too large.".to_string());
            }
            if !b.is_empty() && (!valid_id(&b) || !store::load_sites(r)?.items.iter().any(|s| s.id == b)) {
                return Err("That site is not on the list.".to_string());
            }
            c = days.to_string();
            d = min.to_string();
        }
        "qualify" | "revoke" => {
            if a.is_empty() {
                return Err("Say who this is about.".to_string());
            }
            let def = defs.iter().find(|x| same(&x.name, &b)).ok_or("That role is not in the record. Add the role first.")?;
            b = def.name.clone();
            let held = quals(&log.items, today).iter().any(|q| same(&q.person, &a) && same(&q.role, &b));
            if ev == "qualify" {
                let e = date_days(&c).ok_or("Enter the date it was earned as year-month-day, like 2026-11-20.")?;
                if e > today + 1 {
                    return Err("That date is in the future.".to_string());
                }
            } else {
                if !held {
                    return Err("That person does not hold this qualification.".to_string());
                }
                if note.is_empty() {
                    return Err("Say why it is being taken away.".to_string());
                }
                c.clear();
            }
        }
        _ => {
            if !gear_id_ok(&a) {
                return Err("Pick a piece from the Gear list.".to_string());
            }
            if b.is_empty() {
                return Err("Say who it is for.".to_string());
            }
            let piece = gear.iter().find(|g| g.id == a);
            let now_held = holdings(&log.items);
            if ev == "issue" {
                let g = piece.ok_or("That piece is not in the Gear list.")?;
                if g.condition == "out" {
                    return Err("That piece is marked out of use.".to_string());
                }
                if now_held.iter().any(|h| h.gear == a && same(&h.person, &b)) {
                    return Err("That person already has this piece.".to_string());
                }
                if now_held.iter().filter(|h| h.gear == a).count() as u32 >= g.qty {
                    return Err(if g.qty == 1 { "That piece is already handed out." } else { "Every one of those is already handed out." }.to_string());
                }
            } else if !now_held.iter().any(|h| h.gear == a && same(&h.person, &b)) {
                return Err("That person does not have this piece.".to_string());
            }
            c.clear();
            d.clear();
        }
    }
    let id = next_id('q', log.items.iter().map(|x| x.id.clone()));
    let at = format_instant(now, off);
    let line = format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", id, esc(&by), at, ev, esc(&a), esc(&b), c, d, esc(&note));
    store::append_line(r, &path(r), "# format: roles-log 1\n# id, by, time, event (role, qualify, revoke, issue, return), role or person or gear, site or role or person, days good or date earned, people wanted, words (never changed)\n", &line, |t| parse(t).map(|_| ()))?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const NOW: i64 = 29_000_000;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-roles-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn gear(id: &str, name: &str, qty: u32, cond: &str) -> Piece {
        Piece { id: id.into(), name: name.into(), kind: "radio".into(), make: String::new(), model: String::new(), serial: String::new(), location: String::new(), qty, condition: cond.into(), bought: String::new(), notes: String::new() }
    }
    fn days_ago(n: i64) -> String {
        day_text(today_days(NOW, 0) - n)
    }
    fn go(d: &Path, g: &[Piece], ev: &str, a: &str, b: &str, c: &str, dd: &str, note: &str) -> Result<String, String> {
        add(d, g, ev, a, b, c, dd, note, "Dana", 0, NOW)
    }

    #[test]
    fn day_text_matches_date_days() {
        for s in ["2000-01-01", "2024-02-29", "2026-11-20", "2100-12-31"] {
            assert_eq!(day_text(date_days(s).unwrap()), s);
        }
    }

    #[test]
    fn nothing_exists_until_the_person_writes_it() {
        let d = tmp("empty");
        let l = load(&d).unwrap();
        assert!(l.items.is_empty() && roles(&l.items).is_empty() && holdings(&l.items).is_empty());
        assert!(looks("camp", &l.items, 100).is_empty() && assignment_looks(&l.items, &[]).is_empty());
        assert!(!path(&d).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_qualification_needs_a_role_a_real_past_date_and_can_be_renewed_or_taken_away() {
        let d = tmp("qual");
        assert!(go(&d, &[], "qualify", "Sam", "Radio operator", &days_ago(1), "", "").unwrap_err().contains("Add the role first"));
        go(&d, &[], "role", "Radio operator", "", "365", "2", "Can run the radio net").unwrap();
        assert!(go(&d, &[], "qualify", "Sam", "radio operator", "tomorrow", "", "").is_err());
        assert!(go(&d, &[], "qualify", "Sam", "radio operator", &day_text(today_days(NOW, 0) + 5), "", "").unwrap_err().contains("future"));
        go(&d, &[], "qualify", "Sam", "radio operator", &days_ago(400), "", "").unwrap();
        let t = today_days(NOW, 0);
        let q = quals(&load(&d).unwrap().items, t);
        assert_eq!((q[0].role.as_str(), q[0].standing), ("Radio operator", Standing::Lapsed), "the role's own spelling is kept");
        // renewing replaces the old date
        go(&d, &[], "qualify", "Sam", "Radio operator", &days_ago(10), "", "Passed the net test").unwrap();
        let q = quals(&load(&d).unwrap().items, t);
        assert_eq!(q.len(), 1);
        assert_eq!((q[0].standing, q[0].days_left), (Standing::Current, Some(355)));
        // taking it away needs a reason and a holder
        assert!(go(&d, &[], "revoke", "Alex", "Radio operator", "", "", "x").is_err());
        assert!(go(&d, &[], "revoke", "Sam", "Radio operator", "", "", "").is_err());
        go(&d, &[], "revoke", "sam", "Radio operator", "", "", "Stopped practising").unwrap();
        assert!(quals(&load(&d).unwrap().items, t).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_qualification_that_does_not_run_out_never_lapses_and_running_out_is_flagged() {
        let d = tmp("expiry");
        go(&d, &[], "role", "Watch stander", "", "0", "0", "").unwrap();
        go(&d, &[], "role", "First aid", "", "730", "0", "").unwrap();
        go(&d, &[], "qualify", "Sam", "Watch stander", &days_ago(3000 % 3600), "", "").unwrap();
        go(&d, &[], "qualify", "Dana", "First aid", &days_ago(715), "", "").unwrap();
        go(&d, &[], "qualify", "Alex", "First aid", &days_ago(800), "", "").unwrap();
        let t = today_days(NOW, 0);
        let lines = load(&d).unwrap().items;
        let q = quals(&lines, t);
        assert_eq!(q.iter().find(|x| x.person == "Sam").unwrap().standing, Standing::Current);
        assert_eq!(q.iter().find(|x| x.person == "Dana").unwrap().standing, Standing::RunningOut);
        assert_eq!(q.iter().find(|x| x.person == "Alex").unwrap().standing, Standing::Lapsed);
        let l = looks("camp", &lines, t);
        assert!(l.iter().any(|x| x.contains("Dana") && x.contains("runs out in 15 days")), "{:?}", l);
        assert!(l.iter().any(|x| x.contains("Alex") && x.contains("ran out on")), "{:?}", l);
        assert!(!l.iter().any(|x| x.contains("Sam")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn too_few_qualified_people_is_flagged_and_a_lapsed_one_does_not_count() {
        let d = tmp("min");
        go(&d, &[], "role", "Radio operator", "", "365", "2", "").unwrap();
        let t = today_days(NOW, 0);
        assert!(looks("camp", &load(&d).unwrap().items, t).iter().any(|x| x.contains("Only 0 people are qualified") && x.contains("aim is 2")));
        go(&d, &[], "qualify", "Sam", "Radio operator", &days_ago(5), "", "").unwrap();
        go(&d, &[], "qualify", "Alex", "Radio operator", &days_ago(500), "", "").unwrap();
        let l = looks("camp", &load(&d).unwrap().items, t);
        assert!(l.iter().any(|x| x.contains("Only 1 person is qualified")), "{:?}", l);
        go(&d, &[], "qualify", "Alex", "Radio operator", &days_ago(5), "", "").unwrap();
        assert!(!looks("camp", &load(&d).unwrap().items, t).iter().any(|x| x.contains("Only")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_role_for_one_site_is_only_a_look_for_that_site() {
        let d = tmp("site");
        let site = |id: &str, name: &str| crate::security::Site { id: id.into(), name: name.into(), tz: "America/New_York".into(), map_id: String::new(), notes: String::new() };
        store::save_sites(&d, &[site("camp", "Camp"), site("home", "Home")]).unwrap();
        go(&d, &[], "role", "Gate guard", "camp", "0", "1", "").unwrap();
        assert!(go(&d, &[], "role", "Gate guard", "nowhere", "0", "1", "").is_err());
        let lines = load(&d).unwrap().items;
        assert_eq!(looks("camp", &lines, 100).len(), 1);
        assert!(looks("home", &lines, 100).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn gear_is_handed_out_to_one_person_per_piece_and_taken_back() {
        let d = tmp("issue");
        let g = [gear("g1", "Radio 3", 1, "good"), gear("g2", "Walking poles", 3, "good"), gear("g3", "Broken radio", 1, "out")];
        go(&d, &g, "issue", "g1", "Sam", "", "", "For the night watch").unwrap();
        assert!(go(&d, &g, "issue", "g1", "Alex", "", "", "").unwrap_err().contains("already handed out"));
        assert!(go(&d, &g, "issue", "g1", "Sam", "", "", "").is_err());
        assert!(go(&d, &g, "issue", "g3", "Sam", "", "", "").unwrap_err().contains("out of use"));
        assert!(go(&d, &g, "issue", "g9", "Sam", "", "", "").unwrap_err().contains("not in the Gear list"));
        // a piece with a count can go to several people, up to the count
        for p in ["Sam", "Alex", "Dana"] {
            go(&d, &g, "issue", "g2", p, "", "", "").unwrap();
        }
        assert!(go(&d, &g, "issue", "g2", "Pat", "", "", "").unwrap_err().contains("Every one"));
        assert_eq!(holdings(&load(&d).unwrap().items).len(), 4);
        assert!(go(&d, &g, "return", "g1", "Alex", "", "", "").unwrap_err().contains("does not have"));
        go(&d, &g, "return", "g1", "sam", "", "", "Battery low").unwrap();
        go(&d, &g, "issue", "g1", "Alex", "", "", "").unwrap();
        let h = holdings(&load(&d).unwrap().items);
        assert!(h.iter().any(|x| x.gear == "g1" && x.person == "Alex"));
        assert!(!h.iter().any(|x| x.gear == "g1" && x.person == "Sam"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn handed_out_gear_that_is_damaged_or_gone_needs_a_look() {
        let d = tmp("assign-looks");
        let good = [gear("g1", "Radio 3", 1, "good"), gear("g2", "Tent", 1, "good")];
        go(&d, &good, "issue", "g1", "Sam", "", "", "").unwrap();
        go(&d, &good, "issue", "g2", "Alex", "", "", "").unwrap();
        let lines = load(&d).unwrap().items;
        assert!(assignment_looks(&lines, &good).is_empty());
        let later = [gear("g1", "Radio 3", 1, "repair")];
        let l = assignment_looks(&lines, &later);
        assert!(l.iter().any(|x| x.contains("Sam has Radio 3") && x.contains("needing repair")), "{:?}", l);
        assert!(l.iter().any(|x| x.contains("Alex") && x.contains("no longer in the Gear list")), "{:?}", l);
        let out = [gear("g1", "Radio 3", 1, "out"), gear("g2", "Tent", 1, "good")];
        assert!(assignment_looks(&lines, &out).iter().any(|x| x.contains("out of use")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_damaged_log_is_never_added_to_and_nothing_touches_gear_or_incidents() {
        let d = tmp("damaged");
        let p = path(&d);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "# format: roles-log 1\nq1\tDana\t2025-01-01T10:00+00:00\tqualify\tSam\tNo such role\t2025-01-01\t\t\nq2\tDana\t2025-01-01T10:00+00:00\tmystery\t\t\t\t\t\n").unwrap();
        assert_eq!(load(&d).unwrap().rejected.len(), 2);
        let before = fs::read_to_string(&p).unwrap();
        assert!(go(&d, &[], "role", "Radio operator", "", "0", "0", "").is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), before);
        let code = include_str!("security_roles.rs").split("#[cfg(test)]").next().unwrap().to_string();
        assert!(!code.contains("gear::save") && !code.contains("gear::delete") && !code.contains("append_incident") && !code.contains("incident-log"));
        let _ = fs::remove_dir_all(&d);
    }
}
