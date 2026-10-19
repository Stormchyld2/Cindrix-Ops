//! Checking a finished drive folder before it is sold or trusted. Run it with
//! `cargo run --example check_drive -- <drive folder>`. It looks for the things doc 7 and doc 14
//! say must never ship: a missing marker, content over the product's size budget, archives under
//! a non-commercial license, archives that are damaged, and manuals stamped as restricted.
//! Standard library only. It reads files; it never changes or deletes anything.

use crate::drive;
use crate::kiwix;
use crate::product::{check_budget, Product};
use crate::zim;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Report {
    pub product: Product,
    /// Everything on the drive except the owner's own `user/` folder, in GB.
    pub content_gb: f64,
    pub zims: usize,
    pub pdfs: usize,
    /// Things that must be fixed before the drive ships.
    pub problems: Vec<String>,
    /// Things a person must look at by hand.
    pub warnings: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Does a license string (or a file name) point at a non-commercial license? Such archives are
/// fine for the owner's private use but must never be sold on a drive.
pub fn noncommercial(text: &str) -> bool {
    let t = text.to_lowercase();
    ["by-nc", "by nc", "non-commercial", "noncommercial", "non commercial"].iter().any(|p| t.contains(p))
}

/// Archives whose publishers do not allow commercial redistribution (doc 14). Matched by name
/// when the archive carries no license of its own.
const NONCOMMERCIAL_SOURCES: &[&str] = &["wikihow", "ifixit", "hesperian"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// A restricted distribution statement was found. Must not ship.
    Restricted,
    /// "Approved for public release" was found.
    PublicRelease,
    /// Neither was found, so a person has to read the cover.
    Unknown,
}

/// Read a government document's distribution statement (doc 14). Restricted wins when both appear.
pub fn distribution_verdict(text: &str) -> Verdict {
    let t: String = text.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    const RESTRICTED: &[&str] = &[
        "distribution authorized to u.s. government agencies",
        "distribution authorized to us government agencies",
        "distribution authorized to dod",
        "distribution restricted",
        "distribution is limited to",
        "for official use only",
    ];
    if RESTRICTED.iter().any(|p| t.contains(p)) {
        Verdict::Restricted
    } else if t.contains("approved for public release") {
        Verdict::PublicRelease
    } else {
        Verdict::Unknown
    }
}

/// Every file under `dir` with its size, skipping `skip` folders (by name) at the top level.
/// Symbolic links are not followed.
fn walk(dir: &Path, skip_top: &[&str]) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), true)];
    while let Some((d, top)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if top && skip_top.iter().any(|s| *s == name) {
                    continue;
                }
                stack.push((p, false));
            } else if ft.is_file() {
                out.push((p, e.metadata().map(|m| m.len()).unwrap_or(0)));
            }
        }
    }
    out
}

fn is_ext(p: &Path, ext: &str) -> bool {
    p.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case(ext)).unwrap_or(false)
}

/// Check the drive at `dir` (the drive's top folder, or a folder that contains `Cindrix/`).
pub fn check_drive(dir: &Path) -> Report {
    let mut r = Report { product: Product::Go, content_gb: 0.0, zims: 0, pdfs: 0, problems: vec![], warnings: vec![] };

    let Some(info) = drive::check(dir) else {
        r.problems.push(format!("No {} found in {} (or in its Cindrix folder). The program finds the drive by this file.", drive::MARKER, dir.display()));
        return r;
    };
    let root = info.root.clone();
    r.product = info.product();
    if info.edition.is_empty() {
        r.warnings.push("The marker has no \"edition\", so this drive counts as Go.".into());
    }

    // Folders a working drive has.
    if !root.join("library").is_dir() {
        r.problems.push("There is no library/ folder.".into());
    }
    if r.product != Product::Hub {
        for f in ["engines", "models"] {
            if !root.join(f).is_dir() {
                r.warnings.push(format!("There is no {}/ folder.", f));
            }
        }
    }

    // Size budget, over everything except the owner's own files.
    let files = walk(&root, &["user"]);
    let bytes: u64 = files.iter().map(|(_, n)| *n).sum();
    r.content_gb = bytes as f64 / 1_073_741_824.0;
    if let Err(e) = check_budget(r.product, r.content_gb) {
        r.problems.push(e);
    }

    // Archives: readable, and not under a non-commercial license.
    for z in kiwix::zim_files(&root.join("library")) {
        r.zims += 1;
        let name = z.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if NONCOMMERCIAL_SOURCES.iter().any(|s| name.to_lowercase().contains(s)) {
            r.problems.push(format!("{}: from a source that does not allow commercial redistribution.", name));
        }
        match zim::Reader::open(&z) {
            Err(e) => r.problems.push(format!("{}: cannot be read as a ZIM file ({}).", name, e)),
            Ok(mut rd) => match rd.get(b'M', "License") {
                Ok(Some(item)) => {
                    let lic = String::from_utf8_lossy(&item.data).to_string();
                    if noncommercial(&lic) {
                        r.problems.push(format!("{}: license is \"{}\", which is non-commercial.", name, lic.trim()));
                    }
                }
                _ => r.warnings.push(format!("{}: no license in its metadata. Check SOURCES.md.", name)),
            },
        }
    }

    // Manuals: no restricted distribution statements. Only text that is stored uncompressed can
    // be read here, so a PDF with no readable statement is a warning, not a pass.
    let mut unreadable: Vec<String> = Vec::new();
    for (p, _) in files.iter().filter(|(p, _)| is_ext(p, "pdf")) {
        r.pdfs += 1;
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let mut buf = Vec::new();
        if let Ok(f) = fs::File::open(p) {
            let _ = f.take(512 * 1024).read_to_end(&mut buf);
        }
        match distribution_verdict(&String::from_utf8_lossy(&buf)) {
            Verdict::Restricted => r.problems.push(format!("{}: carries a restricted distribution statement. Do not ship it.", name)),
            Verdict::PublicRelease => {}
            Verdict::Unknown => unreadable.push(name),
        }
    }
    if !unreadable.is_empty() {
        let shown: Vec<_> = unreadable.iter().take(5).cloned().collect();
        r.warnings.push(format!(
            "{} PDF(s) have no readable distribution statement; read each cover by hand ({}{}).",
            unreadable.len(),
            shown.join(", "),
            if unreadable.len() > 5 { ", ..." } else { "" }
        ));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drive::MARKER;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("cindrix-dc-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn drive_of(name: &str, edition: &str) -> PathBuf {
        let d = tmp(name);
        fs::write(d.join(MARKER), format!(r#"{{"name":"t","edition":"{}"}}"#, edition)).unwrap();
        for f in ["library", "engines", "models"] {
            fs::create_dir_all(d.join(f)).unwrap();
        }
        d
    }

    fn zim_with_license(path: &Path, license: Option<&str>) {
        let mut w = zim::ZimWriter::create(path).unwrap();
        w.add(b'C', "a/index.html", "Home", "text/html", b"<h1>x</h1>".to_vec()).unwrap();
        w.metadata("Title", "T").unwrap();
        if let Some(l) = license {
            w.metadata("License", l).unwrap();
        }
        w.set_main_page("a/index.html");
        w.finish().unwrap();
    }

    #[test]
    fn noncommercial_licenses_are_recognised() {
        assert!(noncommercial("CC BY-NC-SA 4.0"));
        assert!(noncommercial("Creative Commons Attribution Non-Commercial"));
        assert!(noncommercial("noncommercial use only"));
        assert!(!noncommercial("CC BY-SA 4.0"));
        assert!(!noncommercial("Public domain"));
    }

    #[test]
    fn distribution_statements_are_read() {
        assert_eq!(distribution_verdict("DISTRIBUTION AUTHORIZED TO U.S.\n  GOVERNMENT AGENCIES and their contractors only"), Verdict::Restricted);
        assert_eq!(distribution_verdict("Distribution restricted. Approved for public release"), Verdict::Restricted);
        assert_eq!(distribution_verdict("DISTRIBUTION STATEMENT A: Approved for public release; distribution is unlimited"), Verdict::PublicRelease);
        assert_eq!(distribution_verdict("Chapter 1"), Verdict::Unknown);
    }

    #[test]
    fn missing_marker_is_a_problem() {
        let r = check_drive(&tmp("nomarker"));
        assert!(!r.ok());
        assert!(r.problems[0].contains(MARKER));
    }

    #[test]
    fn a_clean_drive_passes_and_the_edition_is_read() {
        let d = drive_of("clean", "vault");
        zim_with_license(&d.join("library/ok.zim"), Some("CC BY-SA 4.0"));
        let r = check_drive(&d);
        assert!(r.ok(), "{:?}", r.problems);
        assert_eq!(r.product, Product::Vault);
        assert_eq!(r.zims, 1);
    }

    #[test]
    fn noncommercial_archive_and_damaged_archive_are_problems() {
        let d = drive_of("zims", "go");
        zim_with_license(&d.join("library/nc.zim"), Some("CC BY-NC-SA 4.0"));
        fs::write(d.join("library/broken.zim"), b"not a zim file at all").unwrap();
        zim_with_license(&d.join("library/ifixit_en_all.zim"), Some("CC BY-SA 4.0"));
        let r = check_drive(&d);
        assert!(r.problems.iter().any(|p| p.contains("nc.zim") && p.contains("non-commercial")), "{:?}", r.problems);
        assert!(r.problems.iter().any(|p| p.contains("broken.zim")), "{:?}", r.problems);
        assert!(r.problems.iter().any(|p| p.contains("ifixit_en_all.zim")), "{:?}", r.problems);
    }

    #[test]
    fn archive_without_a_license_is_only_a_warning() {
        let d = drive_of("nolic", "go");
        zim_with_license(&d.join("library/x.zim"), None);
        let r = check_drive(&d);
        assert!(r.ok(), "{:?}", r.problems);
        assert!(r.warnings.iter().any(|w| w.contains("x.zim") && w.contains("no license")));
    }

    #[test]
    fn restricted_pdf_is_a_problem_and_unreadable_pdf_a_warning() {
        let d = drive_of("pdfs", "go");
        fs::write(d.join("library/bad.pdf"), b"%PDF-1.4 Distribution authorized to U.S. Government agencies only").unwrap();
        fs::write(d.join("library/good.pdf"), b"%PDF-1.4 Approved for public release; distribution is unlimited").unwrap();
        fs::write(d.join("library/scan.pdf"), b"%PDF-1.4 \x00\x01compressed").unwrap();
        let r = check_drive(&d);
        assert_eq!(r.pdfs, 3);
        assert!(r.problems.iter().any(|p| p.contains("bad.pdf")));
        assert!(!r.problems.iter().any(|p| p.contains("good.pdf")));
        assert!(r.warnings.iter().any(|w| w.contains("1 PDF") && w.contains("scan.pdf")));
    }

    #[test]
    fn go_over_budget_fails_but_user_files_and_hub_do_not_count() {
        // Sparse files: they report a size without using the disk.
        let big = |p: PathBuf, gb: u64| {
            let f = fs::File::create(p).unwrap();
            f.set_len(gb * 1_073_741_824).unwrap();
        };
        let d = drive_of("budget", "go");
        big(d.join("library/huge.bin"), 101);
        let r = check_drive(&d);
        assert!(r.problems.iter().any(|p| p.contains("limit")), "{:?} {}", r.problems, r.content_gb);

        // 101 GB in user/ is the owner's own data and is not counted.
        let d = drive_of("budget-user", "go");
        fs::create_dir_all(d.join("user")).unwrap();
        big(d.join("user/backup.bin"), 101);
        assert!(check_drive(&d).ok());

        // Hub has no budget.
        let d = drive_of("budget-hub", "hub");
        big(d.join("library/huge.bin"), 101);
        assert!(check_drive(&d).ok());
    }

    #[test]
    fn missing_library_folder_is_a_problem() {
        let d = tmp("nolib");
        fs::write(d.join(MARKER), r#"{"name":"t","edition":"go"}"#).unwrap();
        let r = check_drive(&d);
        assert!(r.problems.iter().any(|p| p.contains("library/")));
    }
}
