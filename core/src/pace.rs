//! The comms PACE plan: four ways to reach each other, in order, so a group can always communicate.
//! Primary (cell phones, internet and power all work), Alternate (cell phones are down, the internet
//! works), Contingency (all down: radio) and Emergency (the last resort).
//!
//! The person answers a short list of plain questions (Settings > PACE plan). The answers are one
//! plain text file on the drive (`user/pace.txt`), one `key: answer` line each, so they can be read
//! and fixed in any editor. From the answers this module builds the structured plan that the Comms
//! plan page shows and prints. With no answers there is nothing to build, and the page says where
//! to answer. Host-only: guests never see it. Standard library only.

use crate::fsio;
use std::fs;
use std::path::{Path, PathBuf};

/// The longest answer kept, in characters.
pub const MAX_ANSWER: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Question {
    pub key: &'static str,
    /// "G" the group, "P", "A", "C" or "E"
    pub tier: &'static str,
    pub prompt: &'static str,
    pub hint: &'static str,
    pub example: &'static str,
}

pub const QUESTIONS: &[Question] = &[
    Question { key: "specialist", tier: "G", prompt: "Who is your Comms Specialist?", hint: "The person who knows radios best. Every fallback plan goes through them, and they keep the group's contact chains going.", example: "Dana Reyes, callsign Hawk" },
    Question { key: "people", tier: "G", prompt: "Who is in this plan, and how can each person be reached?", hint: "Names, phone numbers and radio callsigns or nicknames, one person per line.", example: "Sam: 555 0101, callsign Wren\nPat: 555 0102, callsign Fox" },
    Question { key: "ooa", tier: "G", prompt: "Who is your out-of-area contact?", hint: "Someone far away, outside the area a disaster would hit, who everyone can call or text and who can pass news along.", example: "Aunt Lee in Boise, 555 0199" },

    Question { key: "p_ways", tier: "P", prompt: "What will you use every day?", hint: "Phone calls, texts, a messaging app. If a call will not go through, a text often will.", example: "Call first, then text, then the Signal group" },
    Question { key: "p_group", tier: "P", prompt: "What is your group chat or emergency channel called?", hint: "The name of the chat everyone already has, or the one you will set up.", example: "Signal group: EMERGENCY TRAFFIC" },
    Question { key: "p_codes", tier: "P", prompt: "What code words will you use for private information?", hint: "Normal phone and text messages are not secret. Agree words for places and plans.", example: "\"Cabin\" means our meeting place" },

    Question { key: "a_ways", tier: "A", prompt: "What will you try when cell phones fail but the internet still works?", hint: "Messaging over Wi-Fi, email, a satellite link.", example: "Signal chat over Wi-Fi, then email" },
    Question { key: "a_chain", tier: "A", prompt: "Who starts the email or chat chain, and where do messages go?", hint: "A group address, or who passes the word to whom.", example: "Dana starts it. Messages go to group@example.org" },

    Question { key: "c_radios", tier: "C", prompt: "Which radios and licenses does each person have?", hint: "The make of radio, and whether the person holds a GMRS or amateur license.", example: "Sam: handheld, GMRS license\nPat: handheld, no license yet" },
    Question { key: "c_channels", tier: "C", prompt: "Which channels or frequencies will you use first, and which are the backups?", hint: "The first place to call, and where to move once you are in contact.", example: "Call on 146.520 MHz, then move to GMRS channel 20" },
    Question { key: "c_repeaters", tier: "C", prompt: "Which repeaters will you use, and what tone does each need?", hint: "The name or channel of each repeater and the tone (CTCSS) it sends you.", example: "Hill repeater, GMRS channel 23, tone 141.3" },
    Question { key: "c_times", tier: "C", prompt: "When will you listen, and for how long?", hint: "Fixed times so everyone is listening together.", example: "Every 3 hours for 5 minutes, starting at 7 am" },

    Question { key: "e_last", tier: "E", prompt: "What is the last resort if the radios fail too?", hint: "Going to the meeting place, a written note, a signal.", example: "Go to the meeting place and leave a note on the door" },
    Question { key: "e_rally", tier: "E", prompt: "Where are the meeting places?", hint: "A first choice and a backup, written so anyone can find them.", example: "First: the library car park. Backup: the school field" },
    Question { key: "e_wait", tier: "E", prompt: "When do you go there, and how long do you wait?", hint: "The time you start going and how long you stay before moving on.", example: "Go if no contact for 24 hours. Wait 2 hours" },
];

/// What each level is for, in plain words.
pub fn tier_name(code: &str) -> &'static str {
    match code {
        "G" => "The group",
        "P" => "Primary",
        "A" => "Alternate",
        "C" => "Contingency",
        "E" => "Emergency",
        _ => "",
    }
}

pub fn tier_when(code: &str) -> &'static str {
    match code {
        "G" => "Who is in the plan and who leads it.",
        "P" => "Cell phones, internet and power all work.",
        "A" => "Cell phones are down, but the internet works.",
        "C" => "Cell phones, internet and power are all down. Radio is your lifeline.",
        "E" => "Everything has failed, radios included.",
        _ => "",
    }
}

pub fn question(key: &str) -> Option<&'static Question> {
    QUESTIONS.iter().find(|q| q.key == key)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub prompt: String,
    pub answer: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub code: String,
    pub name: String,
    pub when: String,
    /// The questions that have an answer.
    pub items: Vec<Item>,
    /// The questions still without one.
    pub missing: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// True when no question has an answer: there is nothing to build a plan from.
    pub empty: bool,
    pub answered: usize,
    pub total: usize,
    /// The group first, then Primary, Alternate, Contingency and Emergency.
    pub levels: Vec<Level>,
}

fn escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => {}
            _ => o.push(c),
        }
    }
    o
}

fn unescape(s: &str) -> String {
    let mut o = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => o.push('\n'),
                Some('\\') => o.push('\\'),
                Some(x) => {
                    o.push('\\');
                    o.push(x);
                }
                None => o.push('\\'),
            }
        } else {
            o.push(c);
        }
    }
    o
}

/// Answers as `(key, answer)` pairs for the known questions, in the order of the questions. A
/// question without an answer gives an empty string.
pub fn parse(text: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else { continue };
        let k = k.trim().to_lowercase();
        if question(&k).is_some() {
            found.retain(|(x, _)| *x != k);
            found.push((k, unescape(v.trim())));
        }
    }
    QUESTIONS
        .iter()
        .map(|q| (q.key.to_string(), found.iter().find(|(k, _)| k == q.key).map(|(_, v)| v.clone()).unwrap_or_default()))
        .collect()
}

pub fn serialize(answers: &[(String, String)]) -> String {
    let mut o = String::from("# The comms PACE plan: answers to the questions in Settings > PACE plan. One line per answer; a line break inside an answer is written \\n.\n");
    for q in QUESTIONS {
        let v = answers.iter().find(|(k, _)| k == q.key).map(|(_, v)| v.trim()).unwrap_or("");
        if !v.is_empty() {
            o.push_str(&format!("{}: {}\n", q.key, escape(v)));
        }
    }
    o
}

pub fn path(user_dir: &Path) -> PathBuf {
    user_dir.join("pace.txt")
}

pub fn load(user_dir: &Path) -> Result<Vec<(String, String)>, String> {
    match fs::read_to_string(path(user_dir)) {
        Ok(t) => Ok(parse(&t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(parse("")),
        Err(e) => Err(format!("Could not read your PACE answers: {}", e)),
    }
}

/// Save the answers (unknown keys are ignored), keeping the earlier file in the drive's backups folder first.
pub fn save(user_dir: &Path, answers: &[(String, String)]) -> Result<(), String> {
    for (k, v) in answers {
        if let Some(q) = question(k) {
            if v.trim().chars().count() > MAX_ANSWER {
                return Err(format!("The answer to \"{}\" is too long. Please keep it under {} characters.", q.prompt, MAX_ANSWER));
            }
        }
    }
    fs::create_dir_all(user_dir).map_err(|e| format!("Could not make the folder for your PACE answers: {}", e))?;
    fsio::backup_then_write(&path(user_dir), &user_dir.join("backups"), serialize(answers).as_bytes()).map_err(|e| format!("Could not save your PACE answers: {}", e))
}

/// The structured plan made from the answers.
pub fn build(answers: &[(String, String)]) -> Plan {
    let get = |key: &str| answers.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim().to_string()).unwrap_or_default();
    let mut levels = Vec::new();
    let mut answered = 0;
    for code in ["G", "P", "A", "C", "E"] {
        let mut items = Vec::new();
        let mut missing = Vec::new();
        for q in QUESTIONS.iter().filter(|q| q.tier == code) {
            let a = get(q.key);
            if a.is_empty() {
                missing.push(q.prompt.to_string());
            } else {
                answered += 1;
                items.push(Item { prompt: q.prompt.to_string(), answer: a });
            }
        }
        levels.push(Level { code: code.into(), name: tier_name(code).into(), when: tier_when(code).into(), items, missing });
    }
    Plan { empty: answered == 0, answered, total: QUESTIONS.len(), levels }
}

/// The plan as plain text, for printing or copying. Empty when there is nothing to build from.
pub fn to_text(plan: &Plan) -> String {
    if plan.empty {
        return String::new();
    }
    let mut o = String::from("COMMS PLAN: PACE\n");
    for l in &plan.levels {
        o.push_str(&format!("\n{}\n({})\n", l.name.to_uppercase(), l.when));
        for i in &l.items {
            o.push_str(&format!("- {}\n  {}\n", i.prompt, i.answer.replace('\n', "\n  ")));
        }
        for m in &l.missing {
            o.push_str(&format!("- {}\n  (not filled in yet)\n", m));
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-pace-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn set(key: &str, v: &str) -> (String, String) {
        (key.to_string(), v.to_string())
    }

    #[test]
    fn the_questions_cover_every_level_and_have_their_own_keys() {
        let mut keys: Vec<&str> = QUESTIONS.iter().map(|q| q.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), QUESTIONS.len(), "keys must be different");
        for code in ["G", "P", "A", "C", "E"] {
            assert!(QUESTIONS.iter().any(|q| q.tier == code), "no question for {}", code);
            assert!(!tier_name(code).is_empty() && !tier_when(code).is_empty());
        }
        for q in QUESTIONS {
            assert!(!q.prompt.is_empty() && !q.hint.is_empty() && !q.example.is_empty(), "{}", q.key);
            assert!(!crate::safety::output_contains_dose(&format!("{} {} {}", q.prompt, q.hint, q.example)), "{}", q.key);
        }
    }

    #[test]
    fn with_no_answers_there_is_nothing_to_build() {
        let d = tmp("none");
        let a = load(&d).unwrap();
        assert_eq!(a.len(), QUESTIONS.len());
        assert!(a.iter().all(|(_, v)| v.is_empty()));
        let p = build(&a);
        assert!(p.empty);
        assert_eq!((p.answered, p.total), (0, QUESTIONS.len()));
        assert_eq!(to_text(&p), "");
    }

    #[test]
    fn answers_round_trip_with_line_breaks_and_backslashes() {
        let d = tmp("round");
        let answers = vec![set("people", "Sam: 555 0101\nPat: 555 0102"), set("c_channels", "Call on 146.520 MHz, C:\\radio"), set("specialist", "  Dana  ")];
        save(&d, &answers).unwrap();
        let back = load(&d).unwrap();
        let get = |k: &str| back.iter().find(|(x, _)| x == k).unwrap().1.clone();
        assert_eq!(get("people"), "Sam: 555 0101\nPat: 555 0102");
        assert_eq!(get("c_channels"), "Call on 146.520 MHz, C:\\radio");
        assert_eq!(get("specialist"), "Dana");
        assert_eq!(get("ooa"), "");
        // one line per answer in the file
        let text = fs::read_to_string(path(&d)).unwrap();
        assert_eq!(text.lines().filter(|l| !l.starts_with('#')).count(), 3);
    }

    #[test]
    fn saving_keeps_the_earlier_file_and_ignores_unknown_keys() {
        let d = tmp("backup");
        save(&d, &[set("specialist", "Dana")]).unwrap();
        save(&d, &[set("specialist", "Sam"), set("nonsense", "x")]).unwrap();
        assert!(!fs::read_to_string(path(&d)).unwrap().contains("nonsense"));
        let kept = fs::read_dir(d.join("backups")).unwrap().count();
        assert_eq!(kept, 1, "the first file is kept before it is replaced");
        assert_eq!(load(&d).unwrap().iter().find(|(k, _)| k == "specialist").unwrap().1, "Sam");
    }

    #[test]
    fn a_very_long_answer_is_refused_and_nothing_is_written() {
        let d = tmp("long");
        let e = save(&d, &[set("people", &"x".repeat(MAX_ANSWER + 1))]).unwrap_err();
        assert!(e.contains("too long") && e.contains("Who is in this plan"), "{}", e);
        assert!(!path(&d).exists());
    }

    #[test]
    fn a_partly_filled_plan_shows_what_is_answered_and_what_is_missing() {
        let answers = vec![set("p_ways", "Call, then text"), set("c_times", "Every 3 hours"), set("c_channels", "146.520 MHz")];
        let p = build(&answers);
        assert!(!p.empty);
        assert_eq!(p.answered, 3);
        let c = p.levels.iter().find(|l| l.code == "C").unwrap();
        assert_eq!(c.items.len(), 2);
        assert_eq!(c.missing.len(), 2);
        let a = p.levels.iter().find(|l| l.code == "A").unwrap();
        assert!(a.items.is_empty() && a.missing.len() == 2);
        let t = to_text(&p);
        assert!(t.contains("PRIMARY") && t.contains("CONTINGENCY") && t.contains("Call, then text") && t.contains("(not filled in yet)"));
        // the levels come in the order of the plan
        let order: Vec<&str> = p.levels.iter().map(|l| l.code.as_str()).collect();
        assert_eq!(order, ["G", "P", "A", "C", "E"]);
    }

    #[test]
    fn a_hand_edited_file_still_loads() {
        let a = parse("# note\nSPECIALIST: Dana\nbroken line\nspecialist: Sam\nunknown: x\ne_rally: Library\\nSchool\n");
        let get = |k: &str| a.iter().find(|(x, _)| x == k).unwrap().1.clone();
        assert_eq!(get("specialist"), "Sam", "the last line for a question wins");
        assert_eq!(get("e_rally"), "Library\nSchool");
    }
}
