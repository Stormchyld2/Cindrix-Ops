//! Dates the program works out by itself, so the calendar fills in without typing:
//!  * planting times, from each seed's "when to plant" words and the person's usual frost dates (Settings > My details);
//!  * the two frost days themselves, as markers.
//!
//! These are rules of thumb, not a forecast. Every entry says what it came from, and the person's own frost dates
//! and seed notes are the only inputs: nothing is looked up and nothing phones out.

use crate::calendar;
use crate::ops::Dated;
use crate::seeds::Seed;

const MONTH_DAYS: [u32; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

/// "MM-DD" (or "M-D") as (month, day). None when it is not a real day of the year (29 February is allowed).
pub fn month_day(s: &str) -> Option<(u32, u32)> {
    let (m, d) = s.trim().split_once('-')?;
    let (m, d) = (m.trim().parse::<u32>().ok()?, d.trim().parse::<u32>().ok()?);
    if (1..=12).contains(&m) && d >= 1 && d <= MONTH_DAYS[m as usize - 1] { Some((m, d)) } else { None }
}

/// That day of the year as a day number (29 February becomes 28 February in a year without one).
pub fn day_in(year: i32, (m, d): (u32, u32)) -> i64 {
    let d = if m == 2 && d == 29 && calendar::parse_date(&format!("{:04}-02-29", year)).is_none() { 28 } else { d };
    calendar::days_from_civil(year, m, d)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum When {
    /// A named month: (month, day of that month).
    Fixed(u32, u32),
    /// This many days from the usual last spring frost (negative is before it).
    LastFrost(i64),
    /// This many days from the usual first fall frost.
    FirstFrost(i64),
}

/// What a seed's "when to plant" words mean, and whether the seed is started indoors. None when the words say nothing
/// this can work with (the seed is then left off the calendar rather than guessed at).
fn parse_when(phrase: &str) -> Option<(When, bool)> {
    let p = phrase.to_lowercase();
    let indoors = p.contains("indoor") || p.contains("inside") || p.contains("start seeds") || p.contains("transplant");
    // a month named outright needs no frost date
    if let Some((_, m)) = MONTHS.iter().enumerate().filter_map(|(i, n)| p.find(n).map(|pos| (pos, i as u32 + 1))).min() {
        let day = if p.contains("early") { 5 } else if p.contains("late") { 25 } else { 15 };
        return Some((When::Fixed(m, day), indoors));
    }
    let frost = p.contains("frost");
    let w = if frost && p.contains("first") {
        if p.contains("before") { When::FirstFrost(-60) } else { When::FirstFrost(14) }
    } else if frost && p.contains("before") {
        When::LastFrost(-42)
    } else if frost && (p.contains("after") || p.contains("last")) {
        When::LastFrost(7)
    } else if p.contains("early spring") {
        When::LastFrost(-21)
    } else if p.contains("late spring") {
        When::LastFrost(14)
    } else if p.contains("spring") {
        When::LastFrost(0)
    } else if p.contains("early summer") {
        When::LastFrost(35)
    } else if p.contains("mid summer") || p.contains("midsummer") || p.contains("mid-summer") {
        When::LastFrost(70)
    } else if p.contains("late summer") {
        When::FirstFrost(-70)
    } else if p.contains("summer") {
        When::LastFrost(42)
    } else if p.contains("early fall") || p.contains("early autumn") {
        When::FirstFrost(-56)
    } else if p.contains("late fall") || p.contains("late autumn") {
        When::FirstFrost(14)
    } else if p.contains("fall") || p.contains("autumn") {
        When::FirstFrost(-42)
    } else {
        return None;
    };
    let indoors = indoors || matches!(w, When::LastFrost(o) if o <= -35);
    Some((w, indoors))
}

fn seed_name(s: &Seed) -> String {
    let (c, v) = (s.crop.trim(), s.variety.trim());
    if v.is_empty() { c.to_string() } else { format!("{} ({})", c, v) }
}

/// The usual frost days as markers on the calendar, inside `from..=to`.
pub fn frost_markers(last_frost: &str, first_frost: &str, from: i64, to: i64) -> Vec<Dated> {
    let mut out = Vec::new();
    let (fy, _, _) = calendar::civil_from_days(from);
    let (ty, _, _) = calendar::civil_from_days(to);
    for (text, id, title) in [(last_frost, "last-frost", "Usual last spring frost here"), (first_frost, "first-frost", "Usual first fall frost here")] {
        let Some(md) = month_day(text) else { continue };
        for y in fy..=ty {
            let day = day_in(y, md);
            if day >= from && day <= to {
                out.push(Dated { kind: "season".into(), id: id.into(), title: title.into(), day });
            }
        }
    }
    out
}

/// Planting dates for the seeds in the vault, inside `from..=to`. A seed whose words need a frost date that the
/// person has not entered is skipped.
pub fn planting_dates(seeds: &[Seed], last_frost: &str, first_frost: &str, from: i64, to: i64) -> Vec<Dated> {
    let (lf, ff) = (month_day(last_frost), month_day(first_frost));
    let (fy, _, _) = calendar::civil_from_days(from);
    let (ty, _, _) = calendar::civil_from_days(to);
    let mut out = Vec::new();
    for s in seeds.iter().filter(|s| !s.plant_when.trim().is_empty() && !s.crop.trim().is_empty()) {
        let Some((when, indoors)) = parse_when(&s.plant_when) else { continue };
        for y in fy - 1..=ty + 1 {
            let day = match when {
                When::Fixed(m, d) => Some(day_in(y, (m, d.min(MONTH_DAYS[m as usize - 1])))),
                When::LastFrost(o) => lf.map(|md| day_in(y, md) + o),
                When::FirstFrost(o) => ff.map(|md| day_in(y, md) + o),
            };
            let Some(day) = day else { continue };
            if day < from || day > to { continue; }
            let what = if indoors { format!("Start {} indoors", seed_name(s)) } else { format!("Plant {}", seed_name(s)) };
            out.push(Dated { kind: "seed".into(), id: s.id.clone(), title: format!("{} (about now: {})", what, s.plant_when.trim()), day });
        }
    }
    out
}

/// Everything above in one list.
pub fn seasonal_dates(seeds: &[Seed], last_frost: &str, first_frost: &str, from: i64, to: i64) -> Vec<Dated> {
    let mut v = frost_markers(last_frost, first_frost, from, to);
    v.extend(planting_dates(seeds, last_frost, first_frost, from, to));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> i64 { calendar::parse_date(s).unwrap() }
    fn seed(id: &str, crop: &str, when: &str) -> Seed { Seed { id: id.into(), crop: crop.into(), plant_when: when.into(), ..Default::default() } }

    #[test]
    fn month_and_day_must_be_a_real_day() {
        assert_eq!(month_day("05-10"), Some((5, 10)));
        assert_eq!(month_day("5-1"), Some((5, 1)));
        assert_eq!(month_day("02-29"), Some((2, 29)));
        assert_eq!(month_day("02-30"), None);
        assert_eq!(month_day("13-01"), None);
        assert_eq!(month_day("soon"), None);
        assert_eq!(month_day(""), None);
        assert_eq!(day_in(2027, (2, 29)), day("2027-02-28"));
        assert_eq!(day_in(2028, (2, 29)), day("2028-02-29"));
    }

    #[test]
    fn seed_words_become_rules_and_unknown_words_are_left_alone() {
        assert_eq!(parse_when("Spring after last frost"), Some((When::LastFrost(7), false)));
        assert_eq!(parse_when("Start indoors 6 weeks before last frost"), Some((When::LastFrost(-42), true)));
        assert_eq!(parse_when("Late summer"), Some((When::FirstFrost(-70), false)));
        assert_eq!(parse_when("Fall, 6 weeks before first frost"), Some((When::FirstFrost(-60), false)));
        assert_eq!(parse_when("early March"), Some((When::Fixed(3, 5), false)));
        assert_eq!(parse_when("Sow in October"), Some((When::Fixed(10, 15), false)));
        assert_eq!(parse_when("when the soil is warm"), None);
        assert_eq!(parse_when(""), None);
    }

    #[test]
    fn planting_dates_follow_the_frost_dates_and_stay_inside_the_range() {
        let seeds = vec![seed("s1", "Tomato", "Spring after last frost"), seed("s2", "Spinach", "Late summer"), seed("s3", "Peas", "March"), seed("s4", "Mystery", "whenever")];
        let all = planting_dates(&seeds, "05-10", "10-12", day("2027-01-01"), day("2027-12-31"));
        let find = |id: &str| all.iter().find(|d| d.id == id).map(|d| d.day);
        assert_eq!(find("s1"), Some(day("2027-05-17")));
        assert_eq!(find("s2"), Some(day("2027-08-03")));
        assert_eq!(find("s3"), Some(day("2027-03-15")));
        assert_eq!(find("s4"), None);
        assert!(all.iter().all(|d| d.kind == "seed"));
        assert!(all[0].title.contains("about now"));
        // a narrow range only sees what falls in it
        let may = planting_dates(&seeds, "05-10", "10-12", day("2027-05-01"), day("2027-05-31"));
        assert_eq!(may.len(), 1);
        assert_eq!(may[0].id, "s1");
    }

    #[test]
    fn without_frost_dates_only_named_months_are_planned() {
        let seeds = vec![seed("s1", "Tomato", "Spring after last frost"), seed("s3", "Peas", "March")];
        let all = planting_dates(&seeds, "", "", day("2027-01-01"), day("2027-12-31"));
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "s3");
    }

    #[test]
    fn indoor_starts_say_so_and_a_range_that_crosses_new_year_works() {
        let seeds = vec![seed("s1", "Pepper", "Start indoors before last frost")];
        let all = planting_dates(&seeds, "05-10", "", day("2027-03-01"), day("2027-04-30"));
        assert_eq!(all.len(), 1);
        assert!(all[0].title.starts_with("Start Pepper indoors"));
        assert_eq!(all[0].day, day("2027-03-29"));
        let mark = frost_markers("05-10", "10-12", day("2026-12-01"), day("2027-05-31"));
        assert_eq!(mark.len(), 1);
        assert_eq!(mark[0].day, day("2027-05-10"));
        let two = frost_markers("05-10", "10-12", day("2026-10-01"), day("2027-05-31"));
        assert_eq!(two.len(), 2);
    }

    #[test]
    fn every_suggested_seed_has_planting_words_the_calendar_understands() {
        for x in crate::seedlist::SUGGESTED {
            assert!(parse_when(x.plant_when).is_some(), "{}: {}", x.crop, x.plant_when);
        }
    }
}
