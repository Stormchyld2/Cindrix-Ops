//! The Academy: a text course in modules and lessons, with a short quiz after each lesson.
//!
//! Layout on the drive (the course is plain Markdown-like text, so it is tiny and anyone can edit it):
//! ```text
//! content/academy/01-first-72-hours/_module.md     title, summary
//! content/academy/01-first-72-hours/01-water.md    one lesson
//! ```
//! A lesson starts with a header, then the text, then an optional quiz after a `## Check yourself`
//! line:
//! ```text
//! ---
//! title: Water first
//! minutes: 8
//! status: draft
//! source: FM 21-76, chapter 6
//! ---
//! Plain paragraphs. A line starting "## " is a heading, "- " a bullet, "1. " a numbered step,
//! "> " a note box.
//!
//! ## Check yourself
//! ? How long can a person last without water?
//! - Three weeks
//! + About three days
//! > The rule of threes: 3 minutes air, 3 hours shelter, 3 days water, 3 weeks food.
//! ```
//! Results go in `academy.csv` in the user's folder. Lessons are looked up by their id from the
//! loaded list, never by building a file path from what the interface sends.

use crate::fsio;
use crate::inventory::{quote, split_record};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading(String),
    Para(String),
    Bullets(Vec<String>),
    Steps(Vec<String>),
    Note(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub text: String,
    pub options: Vec<String>,
    pub correct: usize,
    pub explain: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lesson {
    /// "module-folder/lesson-file" without the extension
    pub id: String,
    pub title: String,
    pub minutes: u32,
    /// "draft" until someone qualified has reviewed the lesson
    pub status: String,
    pub source: String,
    pub body: Vec<Block>,
    pub quiz: Vec<Question>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub lessons: Vec<Lesson>,
}

fn header(text: &str) -> Result<(Vec<(String, String)>, String), String> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or("must start with ---")?;
    let (head, body) = rest.split_once("\n---\n").ok_or("header is not closed with ---")?;
    let pairs = head
        .lines()
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect();
    Ok((pairs, body.to_string()))
}

fn get(pairs: &[(String, String)], key: &str) -> String {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default()
}

fn numbered(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let rest = &line[digits..];
    rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))
}

pub fn parse_blocks(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut para = String::new();
    fn flush(para: &mut String, out: &mut Vec<Block>) {
        let t = para.trim();
        if !t.is_empty() {
            out.push(Block::Para(t.to_string()));
        }
        para.clear();
    }
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(h) = line.strip_prefix("## ") {
            flush(&mut para, &mut out);
            out.push(Block::Heading(h.trim().to_string()));
        } else if let Some(b) = line.strip_prefix("- ") {
            flush(&mut para, &mut out);
            match out.last_mut() {
                Some(Block::Bullets(v)) => v.push(b.trim().to_string()),
                _ => out.push(Block::Bullets(vec![b.trim().to_string()])),
            }
        } else if let Some(s) = numbered(line) {
            flush(&mut para, &mut out);
            match out.last_mut() {
                Some(Block::Steps(v)) => v.push(s.trim().to_string()),
                _ => out.push(Block::Steps(vec![s.trim().to_string()])),
            }
        } else if let Some(n) = line.strip_prefix("> ") {
            flush(&mut para, &mut out);
            match out.last_mut() {
                Some(Block::Note(t)) => {
                    t.push(' ');
                    t.push_str(n.trim());
                }
                _ => out.push(Block::Note(n.trim().to_string())),
            }
        } else {
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(line);
        }
    }
    flush(&mut para, &mut out);
    out
}

pub fn parse_quiz(text: &str) -> Result<Vec<Question>, String> {
    let mut qs: Vec<Question> = Vec::new();
    let mut cur: Option<(Question, usize)> = None; // question and how many "+" lines it has
    fn close(cur: &mut Option<(Question, usize)>, qs: &mut Vec<Question>) -> Result<(), String> {
        if let Some((q, plus)) = cur.take() {
            if q.options.len() < 2 {
                return Err(format!("question \"{}\" needs at least two answers", q.text));
            }
            if plus != 1 {
                return Err(format!("question \"{}\" must have exactly one + answer", q.text));
            }
            qs.push(q);
        }
        Ok(())
    }
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(q) = line.strip_prefix("? ") {
            close(&mut cur, &mut qs)?;
            cur = Some((Question { text: q.trim().to_string(), options: vec![], correct: 0, explain: String::new() }, 0));
        } else if let Some((q, plus)) = cur.as_mut() {
            if let Some(o) = line.strip_prefix("+ ") {
                q.correct = q.options.len();
                q.options.push(o.trim().to_string());
                *plus += 1;
            } else if let Some(o) = line.strip_prefix("- ") {
                q.options.push(o.trim().to_string());
            } else if let Some(e) = line.strip_prefix("> ") {
                if !q.explain.is_empty() {
                    q.explain.push(' ');
                }
                q.explain.push_str(e.trim());
            }
        }
    }
    close(&mut cur, &mut qs)?;
    Ok(qs)
}

pub fn parse_lesson(id: &str, text: &str) -> Result<Lesson, String> {
    let (pairs, body) = header(text)?;
    let title = get(&pairs, "title");
    if title.is_empty() {
        return Err("lesson has no title".into());
    }
    // the quiz starts at a "## Check yourself" line
    let mut main = String::new();
    let mut quiz = String::new();
    let mut in_quiz = false;
    for l in body.lines() {
        if l.trim().eq_ignore_ascii_case("## check yourself") {
            in_quiz = true;
            continue;
        }
        let dst = if in_quiz { &mut quiz } else { &mut main };
        dst.push_str(l);
        dst.push('\n');
    }
    let blocks = parse_blocks(&main);
    if blocks.is_empty() {
        return Err("lesson has no text".into());
    }
    let status = get(&pairs, "status");
    Ok(Lesson {
        id: id.to_string(),
        title,
        minutes: get(&pairs, "minutes").parse().unwrap_or(0),
        status: if status.is_empty() { "draft".into() } else { status },
        source: get(&pairs, "source"),
        body: blocks,
        quiz: parse_quiz(&quiz)?,
    })
}

fn md_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"))
                .map(|e| (e.path().file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), e.path()))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// All the words of a lesson (text and quiz), for the dosing check.
fn lesson_text(l: &Lesson) -> String {
    let mut t = String::new();
    for b in &l.body {
        match b {
            Block::Heading(x) | Block::Para(x) | Block::Note(x) => t.push_str(x),
            Block::Bullets(v) | Block::Steps(v) => t.push_str(&v.join(". ")),
        }
        t.push_str(". ");
    }
    for q in &l.quiz {
        t.push_str(&q.text);
        t.push_str(". ");
        t.push_str(&q.options.join(". "));
        t.push_str(". ");
        t.push_str(&q.explain);
        t.push_str(". ");
    }
    t
}

/// A thing wrong with a course file, in plain words. The file is left out; the rest of the course loads.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    /// Where, as "course/module/file.md" (relative to the Academy folder).
    pub place: String,
    pub message: String,
}

fn skipped(name: &str) -> bool {
    name.starts_with('_') || name.starts_with('.')
}

fn load_module(mpath: &Path, mid: &str, prefix: &str, problems: &mut Vec<Problem>) -> Option<Module> {
    let (title, summary) = fs::read_to_string(mpath.join("_module.md"))
        .ok()
        .and_then(|t| header(&t).ok())
        .map(|(p, _)| (get(&p, "title"), get(&p, "summary")))
        .unwrap_or_default();
    let mut lessons = Vec::new();
    for (stem, path) in md_files(mpath) {
        if stem.starts_with('_') || stem.eq_ignore_ascii_case("readme") {
            continue;
        }
        let place = format!("{}{}/{}.md", prefix, mid, stem);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => {
                problems.push(Problem { place, message: "This file could not be read, so it was left out.".into() });
                continue;
            }
        };
        match parse_lesson(&format!("{}{}/{}", prefix, mid, stem), &text) {
            Ok(l) => {
                if crate::safety::output_contains_dose(&lesson_text(&l)) {
                    problems.push(Problem { place, message: "This lesson gives a medicine amount (a dose). Courses here never do that, so it was left out. Take the amount out and say to follow the label or ask a doctor or pharmacist.".into() });
                } else {
                    lessons.push(l);
                }
            }
            Err(e) => problems.push(Problem { place, message: format!("This lesson was left out: {}.", e) }),
        }
    }
    if lessons.is_empty() {
        return None;
    }
    Some(Module { id: mid.to_string(), title: if title.is_empty() { mid.to_string() } else { title }, summary, lessons })
}

fn subfolders(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(rd) = fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<(String, PathBuf)> = rd.flatten().filter(|e| e.path().is_dir()).map(|e| (e.file_name().to_string_lossy().to_string(), e.path())).filter(|(n, _)| !skipped(n)).collect();
    v.sort();
    v
}

/// Load the modules in `dir`, in folder-name order. Lesson ids are `prefix` + "module/lesson".
/// A file that cannot be read or parsed is left out rather than breaking the course.
pub fn load(dir: &Path) -> Vec<Module> {
    let mut ignored = Vec::new();
    subfolders(dir).into_iter().filter_map(|(mid, mpath)| load_module(&mpath, &mid, "", &mut ignored)).collect()
}

/// One course: a folder holding `course.md`, then modules, then lessons.
#[derive(Debug, Clone, PartialEq)]
pub struct Course {
    /// The folder name.
    pub id: String,
    pub title: String,
    pub summary: String,
    pub author: String,
    /// A whole number; when two copies of a course are found, the higher one is used.
    pub version: u32,
    pub license: String,
    pub modules: Vec<Module>,
}

/// The course made of modules placed straight in the Academy folder (how the first course was laid out).
pub const BASICS_ID: &str = "survival-basics";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loaded {
    pub courses: Vec<Course>,
    pub problems: Vec<Problem>,
}

/// Everything in one Academy folder: each folder with a `course.md` is a course; any other folder
/// holding lessons is a module of the "Survival basics" course. Folders starting with `_` or `.`
/// (the template, for one) are skipped.
pub fn load_dir(dir: &Path) -> Loaded {
    let mut out = Loaded::default();
    let mut basics: Vec<Module> = Vec::new();
    for (name, path) in subfolders(dir) {
        let cfile = path.join("course.md");
        if !cfile.is_file() {
            if let Some(m) = load_module(&path, &name, "", &mut out.problems) {
                basics.push(m);
            }
            continue;
        }
        let place = format!("{}/course.md", name);
        let meta = match fs::read_to_string(&cfile).map_err(|e| e.to_string()).and_then(|t| header(&t)) {
            Ok((p, _)) => p,
            Err(e) => {
                out.problems.push(Problem { place: place.clone(), message: format!("The course description could not be read ({}). The folder name is used instead.", e) });
                vec![]
            }
        };
        let mut title = get(&meta, "title");
        if title.is_empty() {
            if !meta.is_empty() {
                out.problems.push(Problem { place: place.clone(), message: "The course has no title: line. The folder name is used instead.".into() });
            }
            title = name.clone();
        }
        if get(&meta, "author").is_empty() && !meta.is_empty() {
            out.problems.push(Problem { place: place.clone(), message: "The course has no author: line. Add one so readers know who wrote it.".into() });
        }
        let prefix = format!("{}/", name);
        let modules: Vec<Module> = subfolders(&path).into_iter().filter_map(|(mid, mpath)| load_module(&mpath, &mid, &prefix, &mut out.problems)).collect();
        if modules.is_empty() {
            out.problems.push(Problem { place: format!("{}/", name), message: "This course has no lessons that could be read, so it is not shown.".into() });
            continue;
        }
        out.courses.push(Course {
            id: name.clone(),
            title,
            summary: get(&meta, "summary"),
            author: get(&meta, "author"),
            version: get(&meta, "version").parse().unwrap_or(0),
            license: get(&meta, "license"),
            modules,
        });
    }
    if !basics.is_empty() {
        out.courses.insert(0, Course { id: BASICS_ID.into(), title: "Survival basics".into(), summary: "The core course: the first days of an emergency, mindset, shelter, water, first aid and longer-term planning.".into(), author: String::new(), version: 0, license: String::new(), modules: basics });
    }
    out
}

/// Several Academy folders at once (the program's own, and one on each library drive). A course found
/// twice is shown once: the higher `version` wins, and on a tie the one listed first.
pub fn load_all(dirs: &[PathBuf]) -> Loaded {
    let mut all = Loaded::default();
    for d in dirs {
        let l = load_dir(d);
        all.problems.extend(l.problems);
        for c in l.courses {
            match all.courses.iter().position(|x| x.id == c.id) {
                Some(i) if c.version > all.courses[i].version => all.courses[i] = c,
                Some(_) => {}
                None => all.courses.push(c),
            }
        }
    }
    all
}

// ---------- results ----------

pub const HEADER: &str = "date,lesson,score,total";

#[derive(Debug, Clone, PartialEq)]
pub struct Result_ {
    pub date: String,
    pub lesson: String,
    pub score: u32,
    pub total: u32,
}

pub struct Log {
    pub path: PathBuf,
}

impl Log {
    pub fn new(user_dir: &Path) -> Log {
        Log { path: user_dir.join("academy.csv") }
    }

    /// A missing file is an empty log. A damaged one is an error and is never overwritten.
    pub fn list(&self) -> Result<Vec<Result_>, String> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.to_string()),
        };
        let mut lines = text.lines();
        match lines.next() {
            Some(h) if h.trim() == HEADER => {}
            None => return Ok(vec![]),
            _ => return Err("academy.csv has an unexpected header".into()),
        }
        let mut out = Vec::new();
        for (i, l) in lines.enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let f = split_record(l);
            if f.len() != 4 {
                return Err(format!("academy.csv line {} is damaged", i + 2));
            }
            let n = |s: &str| s.trim().parse::<u32>().map_err(|_| format!("academy.csv line {}: not a number: {}", i + 2, s));
            out.push(Result_ { date: f[0].clone(), lesson: f[1].clone(), score: n(&f[2])?, total: n(&f[3])? });
        }
        Ok(out)
    }

    pub fn add(&self, r: Result_) -> Result<(), String> {
        if r.score > r.total {
            return Err("score is higher than the number of questions".into());
        }
        let mut all = self.list()?;
        all.push(r);
        let mut csv = String::from(HEADER);
        csv.push('\n');
        for r in &all {
            csv.push_str(&format!("{},{},{},{}\n", quote(&r.date), quote(&r.lesson), r.score, r.total));
        }
        fsio::write_atomic(&self.path, csv.as_bytes()).map_err(|e| e.to_string())
    }
}

/// How far along someone is in one lesson: tries and best score.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Progress {
    pub tries: usize,
    pub best_score: u32,
    pub total: u32,
    pub last_date: String,
}

pub fn progress_for(results: &[Result_], lesson: &str) -> Progress {
    let mut p = Progress::default();
    for r in results.iter().filter(|r| r.lesson == lesson) {
        p.tries += 1;
        if r.score >= p.best_score {
            p.best_score = r.score;
            p.total = r.total;
        }
        p.last_date = r.date.clone();
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const LESSON: &str = "---\ntitle: Water first\nminutes: 8\nsource: FM 21-76\n---\nPeople last about three days without water.\nThis line continues the paragraph.\n\n## What to do\n- Find a source\n- Make it safe\n\n1. Collect\n2. Boil\n\n> A note\n> that goes on.\n\n## Check yourself\n? How long without water?\n- Three weeks\n+ About three days\n> Rule of threes.\n? Second question\n+ Yes\n- No\n";

    #[test]
    fn a_lesson_splits_into_text_blocks_and_a_quiz() {
        let l = parse_lesson("m/l", LESSON).unwrap();
        assert_eq!((l.title.as_str(), l.minutes, l.status.as_str(), l.source.as_str()), ("Water first", 8, "draft", "FM 21-76"));
        assert_eq!(l.body[0], Block::Para("People last about three days without water. This line continues the paragraph.".into()));
        assert_eq!(l.body[1], Block::Heading("What to do".into()));
        assert_eq!(l.body[2], Block::Bullets(vec!["Find a source".into(), "Make it safe".into()]));
        assert_eq!(l.body[3], Block::Steps(vec!["Collect".into(), "Boil".into()]));
        assert_eq!(l.body[4], Block::Note("A note that goes on.".into()));
        assert_eq!(l.quiz.len(), 2);
        assert_eq!(l.quiz[0].options, vec!["Three weeks", "About three days"]);
        assert_eq!(l.quiz[0].correct, 1);
        assert_eq!(l.quiz[0].explain, "Rule of threes.");
        assert_eq!(l.quiz[1].correct, 0);
    }

    #[test]
    fn bad_lessons_and_quizzes_are_refused() {
        assert!(parse_lesson("x", "no header").is_err());
        assert!(parse_lesson("x", "---\nminutes: 3\n---\ntext\n").is_err());
        assert!(parse_lesson("x", "---\ntitle: A\n---\n").is_err());
        let two_right = "---\ntitle: A\n---\ntext\n## Check yourself\n? q\n+ a\n+ b\n";
        assert!(parse_lesson("x", two_right).is_err());
        let none_right = "---\ntitle: A\n---\ntext\n## Check yourself\n? q\n- a\n- b\n";
        assert!(parse_lesson("x", none_right).is_err());
        let one_answer = "---\ntitle: A\n---\ntext\n## Check yourself\n? q\n+ a\n";
        assert!(parse_lesson("x", one_answer).is_err());
    }

    fn tmp(n: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-acad-{}-{}", n, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_course_loads_in_order_and_skips_broken_files() {
        let d = tmp("load");
        fs::create_dir_all(d.join("02-b")).unwrap();
        fs::create_dir_all(d.join("01-a")).unwrap();
        fs::create_dir_all(d.join("03-empty")).unwrap();
        fs::write(d.join("01-a/_module.md"), "---\ntitle: Module A\nsummary: First.\n---\n").unwrap();
        fs::write(d.join("01-a/02-second.md"), LESSON.replace("Water first", "Second")).unwrap();
        fs::write(d.join("01-a/01-first.md"), LESSON).unwrap();
        fs::write(d.join("01-a/03-broken.md"), "garbage").unwrap();
        fs::write(d.join("02-b/01-x.md"), LESSON).unwrap();
        fs::write(d.join("03-empty/_module.md"), "---\ntitle: Nothing\n---\n").unwrap();
        let m = load(&d);
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].title.as_str(), m[0].summary.as_str()), ("Module A", "First."));
        assert_eq!(m[0].lessons.iter().map(|l| l.id.as_str()).collect::<Vec<_>>(), vec!["01-a/01-first", "01-a/02-second"]);
        assert_eq!(m[1].title, "02-b"); // no _module.md: the folder name stands in
        assert!(load(&d.join("missing")).is_empty());
    }

    #[test]
    fn results_round_trip_and_best_score_wins() {
        let d = tmp("log");
        let log = Log::new(&d);
        assert!(log.list().unwrap().is_empty());
        log.add(Result_ { date: "2026-10-20".into(), lesson: "m/l".into(), score: 1, total: 3 }).unwrap();
        log.add(Result_ { date: "2026-10-21".into(), lesson: "m/l".into(), score: 3, total: 3 }).unwrap();
        log.add(Result_ { date: "2026-10-22".into(), lesson: "m/l".into(), score: 2, total: 3 }).unwrap();
        log.add(Result_ { date: "2026-10-22".into(), lesson: "m/other".into(), score: 0, total: 2 }).unwrap();
        assert!(log.add(Result_ { date: "d".into(), lesson: "m/l".into(), score: 4, total: 3 }).is_err());
        let all = log.list().unwrap();
        assert_eq!(all.len(), 4);
        let p = progress_for(&all, "m/l");
        assert_eq!((p.tries, p.best_score, p.total, p.last_date.as_str()), (3, 3, 3, "2026-10-22"));
        assert_eq!(progress_for(&all, "none").tries, 0);
        // a damaged file is reported and left alone
        fs::write(&log.path, "date,lesson,score,total\nx,y\n").unwrap();
        assert!(log.list().is_err());
        assert!(log.add(Result_ { date: "d".into(), lesson: "l".into(), score: 0, total: 1 }).is_err());
        assert_eq!(fs::read_to_string(&log.path).unwrap(), "date,lesson,score,total\nx,y\n");
    }

    #[test]
    fn the_shipped_course_is_complete_and_never_gives_doses() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/academy");
        let mods = load(&dir);
        assert!(mods.len() >= 6, "found {} modules", mods.len());
        let mut ids = std::collections::HashSet::new();
        for m in &mods {
            assert!(!m.summary.is_empty(), "module {} has no summary", m.id);
            assert!(m.lessons.len() >= 3, "module {} has {} lessons", m.id, m.lessons.len());
            for l in &m.lessons {
                assert!(ids.insert(l.id.clone()), "duplicate lesson {}", l.id);
                assert!(l.minutes > 0, "{} has no minutes", l.id);
                assert!(!l.source.is_empty(), "{} names no source", l.id);
                assert!(l.status == "draft" || l.status == "reviewed", "{}", l.id);
                assert!(l.quiz.len() >= 3, "{} has {} quiz questions", l.id, l.quiz.len());
                let mut all = String::new();
                for b in &l.body {
                    match b {
                        Block::Heading(t) | Block::Para(t) | Block::Note(t) => all.push_str(&format!("{}\n", t)),
                        Block::Bullets(v) | Block::Steps(v) => v.iter().for_each(|t| all.push_str(&format!("{}\n", t))),
                    }
                }
                for q in &l.quiz {
                    all.push_str(&q.text);
                    all.push('\n');
                    q.options.iter().for_each(|o| all.push_str(&format!("{}\n", o)));
                    all.push_str(&q.explain);
                    all.push('\n');
                }
                assert!(!crate::safety::output_contains_dose(&all), "{} looks like it gives a dose", l.id);
            }
        }
    }

    fn lesson_with(title: &str, extra: &str) -> String {
        format!("---\ntitle: {}\nminutes: 3\nstatus: draft\n---\nPeople last about three days without water. {}\n\n## Check yourself\n? How long?\n+ About three days\n- Three weeks\n", title, extra)
    }

    #[test]
    fn courses_modules_and_the_old_layout_load_together() {
        let d = tmp("courses");
        // the old layout: a module straight in the Academy folder
        fs::create_dir_all(d.join("01-first")).unwrap();
        fs::write(d.join("01-first/01-a.md"), lesson_with("Old A", "")).unwrap();
        // a course
        fs::create_dir_all(d.join("gardening/01-soil")).unwrap();
        fs::write(d.join("gardening/course.md"), "---\ntitle: Gardening\nsummary: Grow food.\nauthor: Someone\nversion: 2\n---\n").unwrap();
        fs::write(d.join("gardening/01-soil/_module.md"), "---\ntitle: Soil\n---\n").unwrap();
        fs::write(d.join("gardening/01-soil/01-start.md"), lesson_with("Start", "")).unwrap();
        // the template and hidden folders are skipped
        fs::create_dir_all(d.join("_template-course/01-m")).unwrap();
        fs::write(d.join("_template-course/01-m/01-x.md"), lesson_with("Template", "")).unwrap();
        let l = load_dir(&d);
        assert!(l.problems.is_empty(), "{:?}", l.problems);
        assert_eq!(l.courses.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec![BASICS_ID, "gardening"]);
        // the old layout keeps its old lesson ids, so saved results still match
        assert_eq!(l.courses[0].modules[0].lessons[0].id, "01-first/01-a");
        let g = &l.courses[1];
        assert_eq!((g.title.as_str(), g.author.as_str(), g.version), ("Gardening", "Someone", 2));
        assert_eq!(g.modules[0].title, "Soil");
        assert_eq!(g.modules[0].lessons[0].id, "gardening/01-soil/01-start");
    }

    #[test]
    fn a_lesson_that_gives_a_dose_is_left_out_and_reported() {
        let d = tmp("dose");
        fs::create_dir_all(d.join("c/01-m")).unwrap();
        fs::write(d.join("c/course.md"), "---\ntitle: C\nauthor: A\n---\n").unwrap();
        fs::write(d.join("c/01-m/01-ok.md"), lesson_with("Fine", "Blood sugar of 70 mg/dL is low.")).unwrap();
        fs::write(d.join("c/01-m/02-dose.md"), lesson_with("Dose", "Give the child 500 mg of ibuprofen every six hours.")).unwrap();
        fs::write(d.join("c/01-m/03-bad.md"), "garbage").unwrap();
        let l = load_dir(&d);
        let lessons: Vec<&str> = l.courses[0].modules[0].lessons.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(lessons, vec!["c/01-m/01-ok"]);
        let dose = l.problems.iter().find(|p| p.place == "c/01-m/02-dose.md").expect("dose reported");
        assert!(dose.message.contains("dose"), "{}", dose.message);
        assert!(l.problems.iter().any(|p| p.place == "c/01-m/03-bad.md"));
    }

    #[test]
    fn a_course_with_nothing_readable_is_not_shown_and_missing_details_are_reported() {
        let d = tmp("empty");
        fs::create_dir_all(d.join("c/01-m")).unwrap();
        fs::write(d.join("c/course.md"), "---\ntitle: C\n---\n").unwrap();
        fs::write(d.join("c/01-m/01-bad.md"), "garbage").unwrap();
        let l = load_dir(&d);
        assert!(l.courses.is_empty());
        assert!(l.problems.iter().any(|p| p.message.contains("no author")));
        assert!(l.problems.iter().any(|p| p.message.contains("no lessons")));
    }

    #[test]
    fn the_newer_copy_of_a_course_wins_across_folders() {
        let a = tmp("verA");
        let b = tmp("verB");
        for (d, ver, title) in [(&a, 1, "Old title"), (&b, 3, "New title")] {
            fs::create_dir_all(d.join("c/01-m")).unwrap();
            fs::write(d.join("c/course.md"), format!("---\ntitle: {}\nauthor: A\nversion: {}\n---\n", title, ver)).unwrap();
            fs::write(d.join("c/01-m/01-x.md"), lesson_with("X", "")).unwrap();
        }
        let l = load_all(&[a.clone(), b.clone()]);
        assert_eq!(l.courses.len(), 1);
        assert_eq!(l.courses[0].title, "New title");
        // on a tie the first folder keeps it
        fs::write(b.join("c/course.md"), "---\ntitle: Tie\nauthor: A\nversion: 1\n---\n").unwrap();
        assert_eq!(load_all(&[a, b]).courses[0].title, "Old title");
    }

    #[test]
    fn everything_that_ships_loads_cleanly() {
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/academy");
        let l = load_dir(&shipped);
        assert!(l.problems.is_empty(), "{:?}", l.problems);
        assert!(!l.courses.is_empty());
        assert!(l.courses.iter().map(|c| c.modules.iter().map(|m| m.lessons.len()).sum::<usize>()).sum::<usize>() >= 7);
    }

    #[test]
    fn the_template_course_is_a_valid_course_once_renamed() {
        let tpl = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/academy/_template-course");
        let d = tmp("tpl");
        fn copy(from: &Path, to: &Path) {
            fs::create_dir_all(to).unwrap();
            for e in fs::read_dir(from).unwrap().flatten() {
                let (f, t) = (e.path(), to.join(e.file_name()));
                if f.is_dir() { copy(&f, &t) } else { fs::copy(&f, &t).unwrap(); }
            }
        }
        copy(&tpl, &d.join("my-course"));
        let l = load_dir(&d);
        assert!(l.problems.is_empty(), "{:?}", l.problems);
        assert_eq!(l.courses.len(), 1);
        assert_eq!(l.courses[0].modules[0].lessons[0].id, "my-course/01-first-module/01-first-lesson");
        // as shipped (with the underscore) it is ignored
        assert!(load_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/academy").as_path()).courses.iter().all(|c| c.id != "_template-course"));
    }
}
