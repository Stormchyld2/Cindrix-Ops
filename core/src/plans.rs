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

/// Every `*.md` plan in one folder. Names that start with `_` or `.` are skipped (templates, notes).
pub fn load_dir(dir: &Path) -> Loaded {
    let mut out = Loaded::default();
    let Ok(rd) = fs::read_dir(dir) else { return out };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if name.starts_with('_') || name.starts_with('.') || !name.to_lowercase().ends_with(".md") {
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

const HEADER: &str = "kind,plan,step,date";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct State {
    pub followed: Vec<String>,
    /// (plan id, step id, date done)
    pub done: Vec<(String, String, String)>,
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
            Some(h) if h.trim() == HEADER => {}
            None => return Ok(State::default()),
            _ => return Err("plans.csv has an unexpected header".into()),
        }
        let mut st = State::default();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 4 {
                return Err(format!("plans.csv line {} is damaged", i + 2));
            }
            match f[0].as_str() {
                "follow" => st.followed.push(f[1].clone()),
                "done" => {
                    if calendar::parse_date(&f[3]).is_none() {
                        return Err(format!("plans.csv line {}: not a date: {}", i + 2, f[3]));
                    }
                    st.done.push((f[1].clone(), f[2].clone(), f[3].clone()));
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
            csv.push_str(&format!("follow,{},,\n", quote(p)));
        }
        for (p, s, d) in &st.done {
            csv.push_str(&format!("done,{},{},{}\n", quote(p), quote(s), quote(d)));
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
