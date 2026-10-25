//! "My details": where home is, where to bug out to, who is in the household, who to reach, needs and
//! skills. Every part is optional. It is one plain text file on the drive (`user/profile.txt`), one
//! `key: value` line per fact, so it can be read and fixed in any editor. It is host-only: guests
//! never see it. Standard library only.

use crate::fsio;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Place {
    pub name: String,
    pub address: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

impl Place {
    pub fn is_empty(&self) -> bool {
        self.name.trim().is_empty() && self.address.trim().is_empty() && self.lat.is_none()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Contact {
    pub name: String,
    pub how: String,
    pub notes: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub home: Place,
    pub bugout: Vec<Place>,
    pub adults: u32,
    pub children: u32,
    pub babies: u32,
    pub small_pets: u32,
    pub large_pets: u32,
    pub contacts: Vec<Contact>,
    /// Mobility limits, medical equipment to carry, vehicle and fuel range (no medicine doses).
    pub needs: String,
    pub skills: String,
    pub notes: String,
    /// The usual last spring frost and first fall frost, as `MM-DD`. They place planting dates on the calendar.
    pub last_frost: String,
    pub first_frost: String,
}

/// A month and day the program accepts (`MM-DD`), or empty.
fn clean_month_day(s: &str) -> String {
    match crate::seasons::month_day(s) {
        Some((m, d)) => format!("{:02}-{:02}", m, d),
        None => String::new(),
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").replace('|', "/")
}

fn num(v: Option<f64>) -> String {
    v.map(|x| format!("{:.6}", x)).unwrap_or_default()
}

fn place_line(p: &Place) -> String {
    format!("{} | {} | {} | {}", one_line(&p.name), one_line(&p.address), num(p.lat), num(p.lon))
}

fn parse_place(v: &str) -> Place {
    let f: Vec<&str> = v.split('|').map(|x| x.trim()).collect();
    let g = |i: usize| f.get(i).copied().unwrap_or("");
    let ok = |s: &str| s.parse::<f64>().ok().filter(|x| x.is_finite());
    let (lat, lon) = (ok(g(2)), ok(g(3)));
    let valid = matches!((lat, lon), (Some(a), Some(b)) if (-90.0..=90.0).contains(&a) && (-180.0..=180.0).contains(&b));
    Place { name: g(0).to_string(), address: g(1).to_string(), lat: if valid { lat } else { None }, lon: if valid { lon } else { None } }
}

pub fn serialize(p: &Profile) -> String {
    let mut o = String::from("# My details. One fact per line; edit here or in Settings. Kept on this drive only.\n");
    if !p.home.is_empty() {
        o.push_str(&format!("home: {}\n", place_line(&p.home)));
    }
    for b in p.bugout.iter().filter(|b| !b.is_empty()) {
        o.push_str(&format!("bugout: {}\n", place_line(b)));
    }
    for (k, v) in [("adults", p.adults), ("children", p.children), ("babies", p.babies), ("small_pets", p.small_pets), ("large_pets", p.large_pets)] {
        if v > 0 {
            o.push_str(&format!("{}: {}\n", k, v));
        }
    }
    for (k, v) in [("last_frost", &p.last_frost), ("first_frost", &p.first_frost)] {
        let v = clean_month_day(v);
        if !v.is_empty() {
            o.push_str(&format!("{}: {}\n", k, v));
        }
    }
    for c in p.contacts.iter().filter(|c| !c.name.trim().is_empty() || !c.how.trim().is_empty()) {
        o.push_str(&format!("contact: {} | {} | {}\n", one_line(&c.name), one_line(&c.how), one_line(&c.notes)));
    }
    for (k, v) in [("needs", &p.needs), ("skills", &p.skills), ("notes", &p.notes)] {
        for line in v.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            o.push_str(&format!("{}: {}\n", k, line));
        }
    }
    o
}

pub fn parse(text: &str) -> Profile {
    let mut p = Profile::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim().to_lowercase(), v.trim());
        let n = || v.parse::<u32>().unwrap_or(0).min(99);
        match k.as_str() {
            "home" => p.home = parse_place(v),
            "bugout" => p.bugout.push(parse_place(v)),
            "adults" => p.adults = n(),
            "children" => p.children = n(),
            "babies" => p.babies = n(),
            "small_pets" => p.small_pets = n(),
            "large_pets" => p.large_pets = n(),
            "last_frost" => p.last_frost = clean_month_day(v),
            "first_frost" => p.first_frost = clean_month_day(v),
            "contact" => {
                let f: Vec<&str> = v.split('|').map(|x| x.trim()).collect();
                p.contacts.push(Contact { name: f.first().unwrap_or(&"").to_string(), how: f.get(1).unwrap_or(&"").to_string(), notes: f.get(2).unwrap_or(&"").to_string() });
            }
            "needs" | "skills" | "notes" => {
                let t = match k.as_str() {
                    "needs" => &mut p.needs,
                    "skills" => &mut p.skills,
                    _ => &mut p.notes,
                };
                if !t.is_empty() {
                    t.push('\n');
                }
                t.push_str(v);
            }
            _ => {}
        }
    }
    p
}

pub fn path(user_dir: &Path) -> PathBuf {
    user_dir.join("profile.txt")
}

pub fn load(user_dir: &Path) -> Result<Profile, String> {
    match fs::read_to_string(path(user_dir)) {
        Ok(t) => Ok(parse(&t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Profile::default()),
        Err(e) => Err(format!("Could not read your details: {}", e)),
    }
}

/// Save, keeping the earlier file in the drive's backups folder first.
pub fn save(user_dir: &Path, p: &Profile) -> Result<(), String> {
    fs::create_dir_all(user_dir).map_err(|e| format!("Could not make the folder for your details: {}", e))?;
    fsio::backup_then_write(&path(user_dir), &user_dir.join("backups"), serialize(p).as_bytes()).map_err(|e| format!("Could not save your details: {}", e))
}

/// Whether a question is about the household's own situation (home, bug-out places, who lives
/// here), so "My details" is worth giving to the assistant. General questions get nothing.
pub fn question_is_personal(question: &str) -> bool {
    let q = format!(" {} ", question.to_lowercase().replace(|c: char| !c.is_alphanumeric() && c != '-', " "));
    const WORDS: &[&str] = &[
        " home ", " house ", " my place ", " bug out ", " bug-out ", " bugout ", " evacuate ", " evacuation ", " retreat ",
        " my family ", " our family ", " my household ", " our household ", " my kids ", " our kids ", " my children ", " our children ",
        " my baby ", " our baby ", " my pets ", " our pets ", " my dog ", " our dog ", " my cat ", " our cat ", " how much do we ", " how many do we ",
        " do we need ", " do i need ", " should we ", " for us ", " for my family ", " cabin ", " meet up ", " meeting place ", " rally point ",
    ];
    WORDS.iter().any(|w| q.contains(w))
}

/// The assistant's passage from "My details" for a personal question, or nothing. It ranks first
/// when it applies. Phone numbers are left out (see `for_assistant`).
pub fn passage_for(question: &str, p: &Profile) -> Option<crate::retrieval::Passage> {
    if !question_is_personal(question) {
        return None;
    }
    let text = for_assistant(p);
    if text.is_empty() {
        return None;
    }
    Some(crate::retrieval::Passage { source: "My details".into(), heading: "My details (the household's own facts)".into(), text, score: 100.0, link: String::new() })
}

/// A few plain sentences the assistant may use ("where is home", "how far to the bug-out place").
/// Nothing is said about parts left empty. Contacts' phone numbers are left out.
pub fn for_assistant(p: &Profile) -> String {
    let mut s = Vec::new();
    let pl = |x: &Place| {
        let mut t = x.name.trim().to_string();
        if !x.address.trim().is_empty() {
            t = if t.is_empty() { x.address.trim().to_string() } else { format!("{} ({})", t, x.address.trim()) };
        }
        if let (Some(a), Some(b)) = (x.lat, x.lon) {
            t.push_str(&format!(" at {:.4}, {:.4}", a, b));
        }
        t
    };
    if !p.home.is_empty() {
        s.push(format!("Home: {}.", pl(&p.home)));
    }
    let bo: Vec<String> = p.bugout.iter().filter(|b| !b.is_empty()).map(pl).collect();
    if !bo.is_empty() {
        s.push(format!("Places to bug out to: {}.", bo.join("; ")));
    }
    let h = [(p.adults, "adult"), (p.children, "child"), (p.babies, "baby"), (p.small_pets, "small pet"), (p.large_pets, "large pet")];
    let parts: Vec<String> = h.iter().filter(|(n, _)| *n > 0).map(|(n, w)| format!("{} {}", n, if *n > 1 { if *w == "child" { "children".to_string() } else { format!("{}s", w) } } else { w.to_string() })).collect();
    if !parts.is_empty() {
        s.push(format!("Household: {}.", parts.join(", ")));
    }
    if !p.needs.trim().is_empty() {
        s.push(format!("Needs: {}.", one_line(&p.needs)));
    }
    if !p.skills.trim().is_empty() {
        s.push(format!("Skills: {}.", one_line(&p.skills)));
    }
    s.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Profile {
        Profile {
            home: Place { name: "Home".into(), address: "12 Oak St, Morgantown WV".into(), lat: Some(39.6295), lon: Some(-79.9559) },
            bugout: vec![Place { name: "Cabin".into(), address: String::new(), lat: Some(38.9), lon: Some(-80.1) }, Place { name: "Aunt's".into(), address: "Elkins".into(), lat: None, lon: None }],
            adults: 2,
            children: 1,
            small_pets: 1,
            contacts: vec![Contact { name: "Sam".into(), how: "555-0100".into(), notes: "out of state | calls first".into() }],
            needs: "walking stick\nspare glasses".into(),
            skills: "first aid".into(),
            notes: String::new(),
            last_frost: "05-10".into(),
            first_frost: "10-12".into(),
            ..Default::default()
        }
    }

    #[test]
    fn it_round_trips_through_the_plain_file() {
        let mut p = sample();
        let back = parse(&serialize(&p));
        p.contacts[0].notes = "out of state / calls first".into(); // a bar would break the line, so it is swapped
        assert_eq!(back, p);
    }

    #[test]
    fn an_empty_profile_is_an_empty_file_and_junk_lines_are_ignored() {
        assert_eq!(parse(&serialize(&Profile::default())), Profile::default());
        let p = parse("nonsense\nadults: many\nhome: X | | 999 | 5\nchildren: 3\n");
        assert_eq!(p.adults, 0);
        assert_eq!(p.children, 3);
        assert_eq!((p.home.name.as_str(), p.home.lat), ("X", None), "a position out of range is dropped");
    }

    #[test]
    fn it_saves_keeping_the_earlier_file_and_loads_back() {
        let d = std::env::temp_dir().join(format!("cindrix-profile-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        assert_eq!(load(&d).unwrap(), Profile::default(), "no file yet is not an error");
        save(&d, &sample()).unwrap();
        let mut second = sample();
        second.adults = 3;
        save(&d, &second).unwrap();
        assert_eq!(load(&d).unwrap().adults, 3);
        assert!(d.join("backups").is_dir());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_assistant_gets_plain_sentences_without_phone_numbers() {
        let t = for_assistant(&sample());
        assert!(t.contains("Home: Home (12 Oak St, Morgantown WV) at 39.6295, -79.9559."), "{}", t);
        assert!(t.contains("Cabin at 38.9000, -80.1000"), "{}", t);
        assert!(t.contains("2 adults") && t.contains("1 child") && t.contains("1 small pet"), "{}", t);
        assert!(!t.contains("555"), "{}", t);
        assert_eq!(for_assistant(&Profile::default()), "");
    }

    #[test]
    fn personal_questions_get_my_details_and_general_ones_do_not() {
        assert!(question_is_personal("How far is the cabin from home?"));
        assert!(question_is_personal("How many days of food do we need?"));
        assert!(question_is_personal("What should we take if we evacuate?"));
        assert!(question_is_personal("Where is our meeting place?"));
        assert!(!question_is_personal("How do I purify river water?"));
        assert!(!question_is_personal("What is the boiling point of water at altitude?"));
        let p = passage_for("Which bug out place is closest to home?", &sample()).unwrap();
        assert_eq!(p.source, "My details");
        assert!(p.text.contains("Cabin") && !p.text.contains("555"));
        assert!(passage_for("How do I purify river water?", &sample()).is_none());
        assert!(passage_for("Where is home?", &Profile::default()).is_none());
    }
}
