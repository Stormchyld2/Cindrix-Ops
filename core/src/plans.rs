//! Plans that come after the first walkthrough: checks to repeat, things to learn, and plans for one
//! kind of emergency. Several can be followed at once.
//!
//! A plan is one plain text file in a `plans` folder (the program's own, each library drive's, and
//! the user's `user/plans`). Anyone can add one with a text editor:
//! ```text
//! ---
//! title: Keep it fresh
//! summary: Short checks to repeat so your supplies and plans stay true.
//! kind: checks            (checks, learning, scenario, prevent = keep a supply from failing, or planning)
//! author: Cindrix
//! version: 1
//! status: draft           (draft until a qualified person has read it)
//! ---
//! ## Turn over your water
//! every: 180              (days until it is due again; leave out for a one-time step)
//! check: water            (ticks by itself from the Inventory: water, food, family, inventory, drill, maps, model, library, or lesson:<id>)
//! go: Inventory           (a screen to jump to) 
//! button: Open Inventory
//! Plain lines are the explanation.
//! - Lines starting "- " are shown as a list.
//! ```
//! Progress (which plans are followed, when each step was last done) is `plans.csv` in the user's
//! folder. A damaged file is reported and never overwritten. A plan that contains a medicine dose is
//! left out, like Academy lessons.

use crate::calendar;
use crate::fsio;
use crate::inventory::{quote, split_record};
use crate::safety;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 100_000;
const MAX_STEPS: usize = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// Made from the title, so it stays the same if steps are reordered.
    pub id: String,
    pub title: String,
    /// Days until the step is due again; None for a one-time step.
    pub every_days: Option<u32>,
    pub check: String,
    pub go: String,
    pub button: String,
    pub body: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub kind: String,
    pub author: String,
    pub version: u32,
    pub status: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub place: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct Loaded {
    pub plans: Vec<Plan>,
    pub problems: Vec<Problem>,
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').chars().take(60).collect()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 60 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Read one plan file's text. `id` comes from the file name.
pub fn parse_plan(id: &str, text: &str) -> Result<Plan, String> {
    if !valid_id(id) {
        return Err("the file name may only use letters, numbers, dashes and underscores".into());
    }
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    if lines.next().map(|l| l.trim()) != Some("---") {
        return Err("the file must start with a line of three dashes (---) and the plan's details".into());
    }
    let mut head: Vec<(String, String)> = Vec::new();
    let mut closed = false;
    for l in lines.by_ref() {
        if l.trim() == "---" {
            closed = true;
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            head.push((k.trim().to_lowercase(), v.split('(').next().unwrap_or("").trim().to_string()));
        }
    }
    if !closed {
        return Err("the details at the top never end (a second --- line is missing)".into());
    }
    let get = |k: &str| head.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
    let title = get("title");
    if title.is_empty() {
        return Err("the plan has no title".into());
    }
    let kind = match get("kind").to_lowercase().as_str() {
        "checks" | "learning" | "scenario" | "prevent" | "planning" => get("kind").to_lowercase(),
        "" => "scenario".to_string(),
        other => return Err(format!("kind must be checks, learning, scenario, prevent or planning (found \"{}\")", other)),
    };
    let version = get("version").parse::<u32>().unwrap_or(1).max(1);
    let status = if get("status").eq_ignore_ascii_case("reviewed") { "reviewed".to_string() } else { "draft".to_string() };

    let mut steps: Vec<Step> = Vec::new();
    for l in lines {
        if let Some(t) = l.strip_prefix("## ") {
            let title = t.trim().to_string();
            if title.is_empty() {
                return Err("a step has no title".into());
            }
            let mut sid = slug(&title);
            if sid.is_empty() {
                sid = format!("step-{}", steps.len() + 1);
            }
            let mut n = 2;
            let base = sid.clone();
            while steps.iter().any(|s| s.id == sid) {
                sid = format!("{}-{}", base, n);
                n += 1;
            }
            steps.push(Step { id: sid, title, every_days: None, check: String::new(), go: String::new(), button: String::new(), body: vec![] });
            continue;
        }
        let Some(cur) = steps.last_mut() else { continue };
        let t = l.trim();
        if t.is_empty() {
            continue;
        }
        // key lines come before the explanation
        if cur.body.is_empty() {
            if let Some((k, v)) = t.split_once(':') {
                let v = v.split('(').next().unwrap_or("").trim().to_string();
                match k.trim().to_lowercase().as_str() {
                    "every" => {
                        cur.every_days = Some(v.parse::<u32>().ok().filter(|d| (1..=3650).contains(d)).ok_or_else(|| format!("step \"{}\": every must be a number of days from 1 to 3650", cur.title))?);
                        continue;
                    }
                    "check" => {
                        cur.check = v;
                        continue;
                    }
                    "go" => {
                        cur.go = v;
                        continue;
                    }
                    "button" => {
                        cur.button = v;
                        continue;
                    }
                    _ => {}
                }
            }
        }
        cur.body.push(t.to_string());
    }
    if steps.is_empty() {
        return Err("the plan has no steps (each step starts with a line like \"## Step title\")".into());
    }
    if steps.len() > MAX_STEPS {
        return Err(format!("the plan has more than {} steps", MAX_STEPS));
    }
    let mut all = format!("{} {}", title, get("summary"));
    for s in &steps {
        all.push(' ');
        all.push_str(&s.title);
        for b in &s.body {
            all.push(' ');
            all.push_str(b);
        }
    }
    if safety::output_contains_dose(&all) {
        return Err("it contains what looks like a medicine dose; plans never give doses, so it was left out".into());
    }
    Ok(Plan { id: id.to_string(), title, summary: get("summary"), kind, author: get("author"), version, status, steps })
}

// ---------- plan packs ----------

/// First line of a plan pack: one text file that holds several plans, to hand to another household.
pub const PACK_HEADER: &str = "CINDRIX-PLAN-PACK 1";
const PACK_MARK: &str = "@@@ plan ";

/// Put plans (id, the plan file's text) into one pack file.
pub fn pack(plans: &[(String, String)]) -> String {
    let mut out = String::from(PACK_HEADER);
    out.push('\n');
    for (id, text) in plans {
        out.push_str(PACK_MARK);
        out.push_str(id);
        out.push('\n');
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// Split a pack into (id, text) pairs. Each plan must parse (and carry no medicine dose); a pack
/// with a bad plan is refused as a whole, with the plan's name in the message.
pub fn unpack(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = text.lines();
    if lines.next().map(|l| l.trim_start_matches('\u{feff}').trim()) != Some(PACK_HEADER) {
        return Err("This is not a plan pack file.".into());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur: Option<(String, Vec<&str>)> = None;
    let mut finish = |c: Option<(String, Vec<&str>)>, out: &mut Vec<(String, String)>| -> Result<(), String> {
        if let Some((id, body)) = c {
            let t = body.join("\n");
            parse_plan(&id, &t).map_err(|m| format!("The plan \"{}\" in the pack cannot be used: {}", id, m))?;
            if out.iter().any(|(i, _)| *i == id) {
                return Err(format!("The plan \"{}\" is in the pack twice.", id));
            }
            out.push((id, t));
        }
        Ok(())
    };
    for l in lines {
        if let Some(id) = l.strip_prefix(PACK_MARK) {
            finish(cur.take(), &mut out)?;
            let id = id.trim().to_string();
            if !valid_id(&id) {
                return Err(format!("\"{}\" is not a usable plan name.", id));
            }
            cur = Some((id, Vec::new()));
        } else if let Some((_, body)) = cur.as_mut() {
            body.push(l);
        }
    }
    finish(cur.take(), &mut out)?;
    if out.is_empty() {
        return Err("The pack has no plans in it.".into());
    }
    Ok(out)
}

/// Every `*.md` plan in one folder. Names that start with `_` or `.` are skipped (templates, notes).
pub fn load_dir(dir: &Path) -> Loaded {
    let mut out = Loaded::default();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with('_') || name.starts_with('.') {
            continue;
        }
        if name.to_lowercase().ends_with(".cplan") {
            // a plan pack dropped into the folder: each plan in it counts
            let place = format!("plans/{}", name);
            match fs::metadata(&p) {
                Ok(m) if m.len() <= MAX_FILE * 40 => {}
                _ => {
                    out.problems.push(Problem { place, message: "the pack is too big or cannot be read".into() });
                    continue;
                }
            }
            match fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| unpack(&t)) {
                Ok(list) => {
                    for (id, t) in list {
                        match parse_plan(&id, &t) {
                            Ok(plan) => out.plans.push(plan),
                            Err(m) => out.problems.push(Problem { place: format!("{} ({})", place, id), message: m }),
                        }
                    }
                }
                Err(m) => out.problems.push(Problem { place, message: m }),
            }
            continue;
        }
        if !name.to_lowercase().ends_with(".md") {
            continue;
        }
        let id = name[..name.len() - 3].to_string();
        let place = format!("plans/{}", name);
        match fs::metadata(&p) {
            Ok(m) if m.len() > MAX_FILE => {
                out.problems.push(Problem { place, message: "the file is too big to be a plan".into() });
                continue;
            }
            Err(e) => {
                out.problems.push(Problem { place, message: e.to_string() });
                continue;
            }
            _ => {}
        }
        match fs::read_to_string(&p) {
            Ok(t) => match parse_plan(&id, &t) {
                Ok(plan) => out.plans.push(plan),
                Err(m) => out.problems.push(Problem { place, message: m }),
            },
            Err(e) => out.problems.push(Problem { place, message: e.to_string() }),
        }
    }
    out
}

/// Plans from several folders. If the same id is in more than one, the higher version is kept
/// (the earlier folder wins a tie).
pub fn load_all(dirs: &[PathBuf]) -> Loaded {
    let mut out = Loaded::default();
    for d in dirs {
        let l = load_dir(d);
        out.problems.extend(l.problems);
        for p in l.plans {
            match out.plans.iter().position(|x| x.id == p.id) {
                Some(i) if out.plans[i].version >= p.version => {}
                Some(i) => out.plans[i] = p,
                None => out.plans.push(p),
            }
        }
    }
    out
}

// ---------- progress ----------

const HEADER: &str = "kind,plan,step,date,person";
/// The first version of the file had no person column; it still loads.
const OLD_HEADER: &str = "kind,plan,step,date";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct State {
    pub followed: Vec<String>,
    /// (plan id, step id, date done)
    pub done: Vec<(String, String, String)>,
    /// (plan id, person) for plans followed for one named person of the household
    pub person_followed: Vec<(String, String)>,
    /// (plan id, step id, person, date done)
    pub person_done: Vec<(String, String, String, String)>,
}

pub struct Store {
    pub path: PathBuf,
}

impl Store {
    pub fn new(user_dir: &Path) -> Store {
        Store { path: user_dir.join("plans.csv") }
    }

    /// A missing file is an empty state. A damaged one is an error and is never overwritten.
    pub fn load(&self) -> Result<State, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(h) if h.trim() == HEADER || h.trim() == OLD_HEADER => {}
            None => return Ok(State::default()),
            _ => return Err("plans.csv has an unexpected header".into()),
        }
        let mut st = State::default();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 4 && f.len() != 5 {
                return Err(format!("plans.csv line {} is damaged", i + 2));
            }
            let who = f.get(4).cloned().unwrap_or_default();
            match f[0].as_str() {
                "follow" if who.is_empty() => st.followed.push(f[1].clone()),
                "follow" => st.person_followed.push((f[1].clone(), who)),
                "done" => {
                    if calendar::parse_date(&f[3]).is_none() {
                        return Err(format!("plans.csv line {}: not a date: {}", i + 2, f[3]));
                    }
                    if who.is_empty() {
                        st.done.push((f[1].clone(), f[2].clone(), f[3].clone()));
                    } else {
                        st.person_done.push((f[1].clone(), f[2].clone(), who, f[3].clone()));
                    }
                }
                other => return Err(format!("plans.csv line {}: unknown kind {}", i + 2, other)),
            }
        }
        Ok(st)
    }

    fn save(&self, st: &State) -> Result<(), String> {
        let mut csv = String::from(HEADER);
        csv.push('\n');
        for p in &st.followed {
            csv.push_str(&format!("follow,{},,,\n", quote(p)));
        }
        for (p, s, d) in &st.done {
            csv.push_str(&format!("done,{},{},{},\n", quote(p), quote(s), quote(d)));
        }
        for (p, w) in &st.person_followed {
            csv.push_str(&format!("follow,{},,,{}\n", quote(p), quote(w)));
        }
        for (p, s, w, d) in &st.person_done {
            csv.push_str(&format!("done,{},{},{},{}\n", quote(p), quote(s), quote(d), quote(w)));
        }
        fsio::write_atomic(&self.path, csv.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn follow(&self, plan: &str, on: bool) -> Result<(), String> {
        if !valid_id(plan) {
            return Err("There is no such plan.".into());
        }
        let mut st = self.load()?;
        st.followed.retain(|p| p != plan);
        if on {
            st.followed.push(plan.to_string());
        }
        self.save(&st)
    }

    /// Follow a plan for one named person of the household (or stop). The household's own
    /// following is separate and is not touched.
    pub fn follow_for(&self, plan: &str, person: &str, on: bool) -> Result<(), String> {
        if !valid_id(plan) {
            return Err("There is no such plan.".into());
        }
        let person = clean_person(person)?;
        let mut st = self.load()?;
        st.person_followed.retain(|(p, w)| !(p == plan && *w == person));
        if on {
            st.person_followed.push((plan.to_string(), person));
        }
        self.save(&st)
    }

    /// Mark a step done for one person on `date`, or clear it.
    pub fn tick_for(&self, plan: &str, step: &str, person: &str, done: bool, date: &str) -> Result<(), String> {
        if !valid_id(plan) || step.is_empty() || step.len() > 80 {
            return Err("There is no such step.".into());
        }
        if calendar::parse_date(date).is_none() {
            return Err("That is not a date.".into());
        }
        let person = clean_person(person)?;
        let mut st = self.load()?;
        st.person_done.retain(|(p, s, w, _)| !(p == plan && s == step && *w == person));
        if done {
            st.person_done.push((plan.to_string(), step.to_string(), person, date.to_string()));
        }
        self.save(&st)
    }

    /// Mark a step done on `date`, or clear it. One entry per step: doing it again replaces the date.
    pub fn tick(&self, plan: &str, step: &str, done: bool, date: &str) -> Result<(), String> {
        if !valid_id(plan) || step.is_empty() || step.len() > 80 {
            return Err("There is no such step.".into());
        }
        if calendar::parse_date(date).is_none() {
            return Err("That is not a date.".into());
        }
        let mut st = self.load()?;
        st.done.retain(|(p, s, _)| !(p == plan && s == step));
        if done {
            st.done.push((plan.to_string(), step.to_string(), date.to_string()));
        }
        self.save(&st)
    }
}

fn clean_person(person: &str) -> Result<String, String> {
    let p = person.trim();
    if p.is_empty() || p.chars().count() > 60 || p.chars().any(|c| c.is_control()) {
        return Err("Choose a person.".into());
    }
    Ok(p.to_string())
}

impl State {
    /// Whether the plan is followed: by the household when `person` is empty, otherwise for that person.
    pub fn is_followed(&self, plan: &str, person: &str) -> bool {
        if person.is_empty() {
            self.followed.iter().any(|p| p == plan)
        } else {
            self.person_followed.iter().any(|(p, w)| p == plan && w == person)
        }
    }

    /// The date a step was last done by the household (`person` empty) or by one person.
    pub fn last_done(&self, plan: &str, step: &str, person: &str) -> Option<String> {
        if person.is_empty() {
            self.done.iter().find(|(p, s, _)| p == plan && s == step).map(|(_, _, d)| d.clone())
        } else {
            self.person_done.iter().find(|(p, s, w, _)| p == plan && s == step && w == person).map(|(_, _, _, d)| d.clone())
        }
    }
}

// ---------- what is due ----------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepState {
    /// Never done.
    Todo,
    /// Done, and nothing needs doing yet.
    Done,
    /// Done before, but its repeat time has passed.
    Due,
}

/// State of a step given the date it was last done (if ever). Also the days left until it is due
/// again (negative when overdue); None for a one-time step.
pub fn step_state(step: &Step, last_done: Option<&str>, today: i64) -> (StepState, Option<i64>) {
    let Some(d) = last_done.and_then(calendar::parse_date) else { return (StepState::Todo, None) };
    match step.every_days {
        None => (StepState::Done, None),
        Some(n) => {
            let left = d + n as i64 - today;
            (if left < 0 { StepState::Due } else { StepState::Done }, Some(left))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\ntitle: Keep it fresh\nsummary: Repeat these.\nkind: checks\nauthor: Me\nversion: 2\nstatus: draft\n---\n## Turn over your water\nevery: 180 (days)\ncheck: water\ngo: Inventory\nbutton: Open Inventory\nUse the oldest first.\n- Label the date\n- Refill\n\n## Test the radio\nevery: 90\nCheck the batteries.\n\n## Write the plan down\nOnce is enough.\n";

    fn tmp(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let d = std::env::temp_dir().join(format!("cindrix-plans-{}-{}-{}", name, std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_plan_is_read_with_its_steps_and_keys() {
        let p = parse_plan("keep-it-fresh", SAMPLE).unwrap();
        assert_eq!((p.title.as_str(), p.kind.as_str(), p.version, p.status.as_str()), ("Keep it fresh", "checks", 2, "draft"));
        assert_eq!(p.steps.len(), 3);
        let s = &p.steps[0];
        assert_eq!((s.id.as_str(), s.every_days, s.check.as_str(), s.go.as_str(), s.button.as_str()), ("turn-over-your-water", Some(180), "water", "Inventory", "Open Inventory"));
        assert_eq!(s.body, vec!["Use the oldest first.", "- Label the date", "- Refill"]);
        assert_eq!(p.steps[2].every_days, None);
    }

    #[test]
    fn bad_plans_say_what_is_wrong() {
        assert!(parse_plan("x", "no header").unwrap_err().contains("three dashes"));
        assert!(parse_plan("x", "---\ntitle: T\n").unwrap_err().contains("never end"));
        assert!(parse_plan("x", "---\nsummary: s\n---\n## A\nb").unwrap_err().contains("no title"));
        assert!(parse_plan("x", "---\ntitle: T\n---\nnothing").unwrap_err().contains("no steps"));
        assert!(parse_plan("x", "---\ntitle: T\nkind: weird\n---\n## A\nb").unwrap_err().contains("kind must be"));
        assert!(parse_plan("x", "---\ntitle: T\n---\n## A\nevery: soon\nb").unwrap_err().contains("every must be"));
        assert!(parse_plan("../x", SAMPLE).is_err());
        // two steps with the same title still get different ids
        let p = parse_plan("x", "---\ntitle: T\n---\n## Same\na\n## Same\nb").unwrap();
        assert_ne!(p.steps[0].id, p.steps[1].id);
    }

    #[test]
    fn a_plan_with_a_dose_is_refused() {
        let t = "---\ntitle: T\n---\n## Take medicine\nGive 500 mg of ibuprofen every 4 hours.\n";
        assert!(parse_plan("x", t).unwrap_err().contains("dose"));
    }

    #[test]
    fn folders_skip_templates_and_the_higher_version_wins() {
        let a = tmp("a");
        let b = tmp("b");
        fs::write(a.join("keep.md"), SAMPLE).unwrap(); // version 2
        fs::write(a.join("_template.md"), "not a plan").unwrap();
        fs::write(a.join("notes.txt"), "x").unwrap();
        fs::write(a.join("broken.md"), "oops").unwrap();
        fs::write(b.join("keep.md"), SAMPLE.replace("version: 2", "version: 3").replace("Keep it fresh", "Keep it fresher")).unwrap();
        let l = load_all(&[a, b]);
        assert_eq!(l.plans.len(), 1);
        assert_eq!(l.plans[0].title, "Keep it fresher");
        assert_eq!(l.problems.len(), 1);
        assert!(l.problems[0].place.contains("broken.md"));
    }

    #[test]
    fn progress_is_saved_replaced_and_a_damaged_file_is_never_overwritten() {
        let d = tmp("store");
        let s = Store::new(&d);
        assert_eq!(s.load().unwrap(), State::default());
        s.follow("keep-it-fresh", true).unwrap();
        s.follow("keep-it-fresh", true).unwrap();
        s.tick("keep-it-fresh", "test-the-radio", true, "2026-01-05").unwrap();
        s.tick("keep-it-fresh", "test-the-radio", true, "2026-02-05").unwrap();
        let st = s.load().unwrap();
        assert_eq!(st.followed, vec!["keep-it-fresh"]);
        assert_eq!(st.done, vec![("keep-it-fresh".into(), "test-the-radio".into(), "2026-02-05".into())]);
        s.tick("keep-it-fresh", "test-the-radio", false, "2026-02-06").unwrap();
        assert!(s.load().unwrap().done.is_empty());
        s.follow("keep-it-fresh", false).unwrap();
        assert!(s.load().unwrap().followed.is_empty());
        assert!(s.tick("keep-it-fresh", "x", true, "tomorrow").is_err());
        assert!(s.follow("../evil", true).is_err());
        fs::write(&s.path, "garbage\n").unwrap();
        assert!(s.follow("a", true).is_err());
        assert_eq!(fs::read_to_string(&s.path).unwrap(), "garbage\n");
    }

    #[test]
    fn plans_can_be_followed_and_ticked_for_one_person_without_touching_the_household() {
        let d = tmp("people");
        let s = Store::new(&d);
        // a file from before the person column still loads
        fs::write(&s.path, "kind,plan,step,date\nfollow,keep-it-fresh,,\ndone,keep-it-fresh,a,2026-01-05\n").unwrap();
        let st = s.load().unwrap();
        assert!(st.is_followed("keep-it-fresh", "") && st.last_done("keep-it-fresh", "a", "").as_deref() == Some("2026-01-05"));
        s.follow_for("keep-it-fresh", "Sam", true).unwrap();
        s.follow_for("keep-it-fresh", "Sam", true).unwrap();
        s.tick_for("keep-it-fresh", "a", "Sam", true, "2026-03-01").unwrap();
        s.tick_for("keep-it-fresh", "a", "Alex, Jr.", true, "2026-03-02").unwrap();
        let st = s.load().unwrap();
        assert!(st.is_followed("keep-it-fresh", "Sam") && !st.is_followed("keep-it-fresh", "Alex, Jr."));
        assert!(st.is_followed("keep-it-fresh", "")); // the household's own is untouched
        assert_eq!(st.last_done("keep-it-fresh", "a", "Sam").as_deref(), Some("2026-03-01"));
        assert_eq!(st.last_done("keep-it-fresh", "a", "Alex, Jr.").as_deref(), Some("2026-03-02"));
        assert_eq!(st.last_done("keep-it-fresh", "a", "").as_deref(), Some("2026-01-05"));
        s.tick_for("keep-it-fresh", "a", "Sam", false, "2026-03-03").unwrap();
        assert_eq!(s.load().unwrap().last_done("keep-it-fresh", "a", "Sam"), None);
        s.follow_for("keep-it-fresh", "Sam", false).unwrap();
        assert!(!s.load().unwrap().is_followed("keep-it-fresh", "Sam"));
        assert!(s.follow_for("keep-it-fresh", "  ", true).is_err());
        assert!(s.tick_for("keep-it-fresh", "a", "Sam", true, "soon").is_err());
        // the household's own ticks still work and keep the people's
        s.tick("keep-it-fresh", "a", true, "2026-04-01").unwrap();
        assert_eq!(s.load().unwrap().last_done("keep-it-fresh", "a", "Alex, Jr.").as_deref(), Some("2026-03-02"));
    }

    #[test]
    fn a_plan_pack_round_trips_and_loads_from_a_folder() {
        let a = parse_plan("keep-it-fresh", SAMPLE).unwrap();
        let text = pack(&[("keep-it-fresh".to_string(), SAMPLE.to_string()), ("second".to_string(), SAMPLE.replace("Keep it fresh", "Second plan"))]);
        let list = unpack(&text).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(parse_plan(&list[0].0, &list[0].1).unwrap().steps.len(), a.steps.len());
        assert!(parse_plan(&list[1].0, &list[1].1).unwrap().title.contains("Second plan"));
        // dropped into a plans folder it loads like separate plan files
        let d = tmp("pack");
        fs::write(d.join("friends.cplan"), &text).unwrap();
        fs::write(d.join("bad.cplan"), "not a pack").unwrap();
        let l = load_dir(&d);
        assert_eq!(l.plans.len(), 2);
        assert_eq!(l.problems.len(), 1);
        // refused: wrong header, a bad plan, a repeated plan, a bad name, an empty pack
        assert!(unpack("hello").is_err());
        assert!(unpack(&format!("{}\n@@@ plan x\noops", PACK_HEADER)).unwrap_err().contains("\"x\""));
        assert!(unpack(&format!("{}\n@@@ plan ../evil\n{}", PACK_HEADER, SAMPLE)).is_err());
        let twice = format!("{}\n@@@ plan k\n{}\n@@@ plan k\n{}", PACK_HEADER, SAMPLE, SAMPLE);
        assert!(unpack(&twice).unwrap_err().contains("twice"));
        assert!(unpack(PACK_HEADER).is_err());
    }

    #[test]
    fn repeating_steps_become_due_again() {
        let p = parse_plan("x", SAMPLE).unwrap();
        let radio = &p.steps[1]; // every 90 days
        let day = |s: &str| calendar::parse_date(s).unwrap();
        assert_eq!(step_state(radio, None, day("2026-03-01")).0, StepState::Todo);
        assert_eq!(step_state(radio, Some("2026-01-01"), day("2026-02-01")), (StepState::Done, Some(59)));
        assert_eq!(step_state(radio, Some("2026-01-01"), day("2026-04-01")), (StepState::Done, Some(0)));
        assert_eq!(step_state(radio, Some("2026-01-01"), day("2026-04-02")), (StepState::Due, Some(-1)));
        let once = &p.steps[2];
        assert_eq!(step_state(once, Some("2020-01-01"), day("2026-04-02")), (StepState::Done, None));
    }
}
