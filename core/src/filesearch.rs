//! Search the loose files and folders of the drive (text, Markdown and saved web pages) so the
//! assistant can answer from anything the owner has copied onto it, not only from the shelf and
//! the archives. Standard library only.
//!
//! It is a plain scan, not a stored index, so it is held to a time and file budget: on a very
//! large drive the scan stops early and answers from what it has seen. PDFs are not read here (a
//! PDF's text is read by the Shelf when the book is opened); put PDFs in `library`, `books` or
//! `manuals` and the Shelf takes them in by itself.

use crate::kiwix::{best_chunk, html_to_text};
use crate::retrieval::{self, Passage};
use crate::shelf::title_from_filename;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Folders that are never searched: program parts, media, and places the assistant already
/// searches in its own way (the shelf's text, saved websites, the sample content, backups).
pub const SKIP_DIRS: &[&str] = &[
    "engines", "models", "maps", "music", "backups", "node_modules", "web-text", "my-library", "content", "scripts", "manifest", "licenses",
    "$recycle.bin", "system volume information",
];

/// File types that are read.
const EXTS: &[&str] = &["md", "markdown", "txt", "text", "html", "htm"];

#[derive(Debug, Clone)]
pub struct Limits {
    pub max_depth: usize,
    /// Folders looked into plus files looked at.
    pub max_entries: usize,
    pub max_file_bytes: u64,
    pub budget: Duration,
    /// Leave out the `user` folders (the person's own notes, chats and family page), for questions from guests.
    pub hide_user: bool,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits { max_depth: 8, max_entries: 60_000, max_file_bytes: 4 * 1024 * 1024, budget: Duration::from_millis(2500), hide_user: false }
    }
}

/// What a scan did, so the screen can say when it stopped early.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scan {
    pub files_read: usize,
    pub stopped_early: bool,
}

fn skipped(name: &str) -> bool {
    let n = name.to_lowercase();
    n.starts_with('.') || SKIP_DIRS.contains(&n.as_str())
}

/// The readable files under `roots`, within the limits. Links are not followed, so a link back up
/// the tree cannot loop.
pub fn files_under(roots: &[PathBuf], lim: &Limits, started: Instant) -> (Vec<PathBuf>, bool) {
    fn walk(dir: &Path, depth: usize, lim: &Limits, started: Instant, seen: &mut usize, out: &mut Vec<PathBuf>, stopped: &mut bool) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            if *stopped {
                return;
            }
            *seen += 1;
            if *seen > lim.max_entries || started.elapsed() > lim.budget {
                *stopped = true;
                return;
            }
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().to_string();
            let p = e.path();
            if ft.is_dir() {
                // each person's journal and comms plan (`user/people`) are never searched, not even for the host
                let private = name.eq_ignore_ascii_case("people") && dir.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("user"));
                if depth > 0 && !skipped(&name) && !private && !(lim.hide_user && name.eq_ignore_ascii_case("user")) {
                    walk(&p, depth - 1, lim, started, seen, out, stopped);
                }
            } else if ft.is_file() && !name.starts_with('.') {
                let ext = name.rsplit_once('.').map(|x| x.1.to_lowercase()).unwrap_or_default();
                if EXTS.contains(&ext.as_str()) && e.metadata().map(|m| m.len() <= lim.max_file_bytes && m.len() > 0).unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = 0usize;
    let mut stopped = false;
    for r in roots {
        walk(r, lim.max_depth, lim, started, &mut seen, &mut out, &mut stopped);
        if stopped {
            break;
        }
    }
    (out, stopped)
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let a = lower.find("<title")?;
    let s = a + lower[a..].find('>')? + 1;
    let e = s + lower[s..].find("</title")?;
    let t = html.get(s..e)?.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { None } else { Some(t) }
}

/// Where a file is, shown as the passage's source: the path below the folder it was found in.
fn shown(root_for: &Path, p: &Path) -> String {
    p.strip_prefix(root_for).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// Passages from the loose files under `roots` that answer `question`, best first (at most `limit`).
/// `base` is the folder paths are shown relative to (the drive's root).
pub fn search(base: &Path, roots: &[PathBuf], question: &str, limit: usize, lim: &Limits) -> (Vec<Passage>, Scan) {
    let q = retrieval::query_tokens(question);
    let mut scan = Scan::default();
    if q.is_empty() || limit == 0 {
        return (vec![], scan);
    }
    let started = Instant::now();
    let (files, stopped) = files_under(roots, lim, started);
    scan.stopped_early = stopped;
    let mut found: Vec<Passage> = Vec::new();
    for f in files {
        if started.elapsed() > lim.budget {
            scan.stopped_early = true;
            break;
        }
        let Ok(bytes) = fs::read(&f) else { continue };
        let text = String::from_utf8_lossy(&bytes).to_string();
        scan.files_read += 1;
        let ext = f.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        let source = shown(base, &f);
        let stem = f.file_name().map(|n| title_from_filename(&n.to_string_lossy())).unwrap_or_default();
        match ext.as_str() {
            "md" | "markdown" => {
                for (head, body) in retrieval::sections(&text) {
                    let head = if head.is_empty() { stem.clone() } else { head };
                    let sc = retrieval::score(&q, &head, &body);
                    if sc >= retrieval::MIN_SCORE {
                        found.push(Passage { source: source.clone(), heading: head, text: body, score: sc, link: crate::sourcelink::for_file(&source) });
                    }
                }
            }
            "html" | "htm" => {
                let title = html_title(&text).unwrap_or_else(|| stem.clone());
                let plain = html_to_text(&text);
                let sc = retrieval::score(&q, &title, &plain);
                if sc >= retrieval::MIN_SCORE {
                    found.push(Passage { source: source.clone(), heading: title.clone(), text: best_chunk(&title, &plain, question), score: sc, link: crate::sourcelink::for_file(&source) });
                }
            }
            _ => {
                let sc = retrieval::score(&q, &stem, &text);
                if sc >= retrieval::MIN_SCORE {
                    found.push(Passage { source: source.clone(), heading: stem.clone(), text: best_chunk(&stem, &text, question), score: sc, link: crate::sourcelink::for_file(&source) });
                }
            }
        }
    }
    found.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    found.truncate(limit);
    (found, scan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(tag: &str) -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("cindrix-fs-{}-{}", tag, n));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn finds_answers_in_markdown_text_and_web_pages_in_any_folder() {
        let d = tmp("a");
        put(&d, "notes/water.md", "# Boiling water\nBoil water for one minute at a rolling boil to make it safe to drink.\n");
        put(&d, "deep/er/still/plan.txt", "Family plan\n\nBoil water first. Boil water again if the power is out. Boil water for one minute.\n");
        put(&d, "saved/page.html", "<html><head><title>Water safety</title></head><body><p>Boil water for one minute before drinking.</p><script>boil water</script></body></html>");
        let (p, scan) = search(&d, std::slice::from_ref(&d), "how long should I boil water", 5, &Limits::default());
        assert!(!scan.stopped_early);
        assert_eq!(scan.files_read, 3);
        let heads: Vec<&str> = p.iter().map(|x| x.heading.as_str()).collect();
        assert!(heads.contains(&"Boiling water") && heads.contains(&"Water safety"), "{:?}", heads);
        let md = p.iter().find(|x| x.heading == "Boiling water").unwrap();
        assert_eq!(md.source, "notes/water.md");
        assert!(md.text.contains("rolling boil"));
        // the script in the web page is not read as text
        let html = p.iter().find(|x| x.heading == "Water safety").unwrap();
        assert!(!html.text.contains("script"));
    }

    #[test]
    fn nothing_relevant_gives_nothing() {
        let d = tmp("b");
        put(&d, "a.txt", "A recipe for bread with flour and yeast and salt and warm water.");
        assert!(search(&d, std::slice::from_ref(&d), "quantum chromodynamics lattice", 3, &Limits::default()).0.is_empty());
        assert!(search(&d, std::slice::from_ref(&d), "the and of", 3, &Limits::default()).0.is_empty());
    }

    #[test]
    fn skips_program_folders_hidden_folders_other_types_and_huge_files() {
        let d = tmp("c");
        let words = "Boil water for one minute. Boil water. Boil water.";
        put(&d, "models/readme.txt", words);
        put(&d, "engines/readme.md", &format!("# Boil water\n{}", words));
        put(&d, ".hidden/x.txt", words);
        put(&d, "my-library/text/b1.md", &format!("# Boil water\n{}", words));
        put(&d, "pic.png", words);
        put(&d, "empty.txt", "");
        put(&d, "big.txt", &format!("{}{}", words, "x".repeat(200)));
        put(&d, "ok/readme.txt", words);
        let lim = Limits { max_file_bytes: 150, ..Limits::default() };
        let (files, _) = files_under(std::slice::from_ref(&d), &lim, Instant::now());
        let rel: Vec<String> = files.iter().map(|f| shown(&d, f)).collect();
        assert_eq!(rel, vec!["ok/readme.txt".to_string()]);
    }

    #[test]
    fn a_guests_search_never_reads_the_users_own_folder() {
        let d = tmp("g");
        let words = "Boil water for one minute. Boil water. Boil water.";
        put(&d, "user/chats/2026-q.txt", words);
        put(&d, "user/notes.md", &format!("# Boil water\n{}", words));
        put(&d, "ok/readme.txt", words);
        let host = files_under(std::slice::from_ref(&d), &Limits::default(), Instant::now()).0.len();
        let guest = files_under(std::slice::from_ref(&d), &Limits { hide_user: true, ..Limits::default() }, Instant::now()).0;
        assert_eq!(host, 3);
        assert_eq!(guest.iter().map(|f| shown(&d, f)).collect::<Vec<_>>(), vec!["ok/readme.txt".to_string()]);
    }

    #[test]
    fn journals_and_comms_plans_are_never_searched() {
        let d = tmp("people");
        let words = "Boil water for one minute. Boil water. Boil water.";
        put(&d, "user/people/Anna/journal/2026-10-23.md", words);
        put(&d, "user/people/Anna/comms-plan.md", words);
        put(&d, "user/notes.md", words);
        let host = files_under(std::slice::from_ref(&d), &Limits::default(), Instant::now()).0;
        assert_eq!(host.iter().map(|f| shown(&d, f)).collect::<Vec<_>>(), vec!["user/notes.md".to_string()]);
    }

    #[test]
    fn stops_at_its_limits_and_says_so() {
        let d = tmp("d");
        for i in 0..20 {
            put(&d, &format!("f{:02}.txt", i), "Boil water. Boil water. Boil water for one minute.");
        }
        let lim = Limits { max_entries: 5, ..Limits::default() };
        let (_, scan) = search(&d, std::slice::from_ref(&d), "boil water", 3, &lim);
        assert!(scan.stopped_early);
        // depth limit: a file too deep down is not seen
        put(&d, "a/b/c/deep.txt", "Boil water. Boil water. Boil water.");
        let lim = Limits { max_depth: 1, ..Limits::default() };
        let (files, _) = files_under(std::slice::from_ref(&d), &lim, Instant::now());
        assert!(!files.iter().any(|f| f.ends_with("deep.txt")));
    }

    #[test]
    fn a_missing_folder_is_not_an_error() {
        let (p, scan) = search(Path::new("/nope"), &[PathBuf::from("/nope/at/all")], "boil water", 3, &Limits::default());
        assert!(p.is_empty() && scan.files_read == 0 && !scan.stopped_early);
    }
}
