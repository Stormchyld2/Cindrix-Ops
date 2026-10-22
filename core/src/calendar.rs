//! Today's year and month from the system clock, with no date library.

use std::time::{SystemTime, UNIX_EPOCH};

/// Convert days since 1970-01-01 to (year, month, day).
pub fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

/// Days since 1970-01-01 for a calendar date (the opposite of `civil_from_days`).
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = y as i64 - if m <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "YYYY-MM-DD" as days since 1970-01-01. None for anything that is not a real date.
pub fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.trim().split('-');
    let (y, m, d) = (it.next()?.parse::<i32>().ok()?, it.next()?.parse::<u32>().ok()?, it.next()?.parse::<u32>().ok()?);
    if it.next().is_some() || !(1..=12).contains(&m) || d == 0 || d > 31 {
        return None;
    }
    let days = days_from_civil(y, m, d);
    // 31 February and such come back as a different date: refuse them
    if civil_from_days(days) != (y, m, d) {
        return None;
    }
    Some(days)
}

pub fn today_days() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0).div_euclid(86_400)
}

pub fn today_ym() -> (i32, u32) {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (y, m, _) = civil_from_days(secs.div_euclid(86_400));
    (y, m)
}

/// Today as "YYYY-MM-DD".
pub fn today_string() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{:04}-{:02}-{:02}", y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
        assert_eq!(civil_from_days(20_014), (2024, 10, 18));
    }

    #[test]
    fn dates_go_both_ways_and_bad_dates_are_refused() {
        for d in [0, 19_723, 19_782, 20_014, -1, 25_000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        assert_eq!(parse_date("2024-02-29"), Some(19_782));
        assert_eq!(parse_date("2023-02-29"), None);
        assert_eq!(parse_date("2024-13-01"), None);
        assert_eq!(parse_date("yesterday"), None);
        assert_eq!(parse_date("2024-01-01-01"), None);
    }
}
