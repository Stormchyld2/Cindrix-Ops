//! Unpacking the content pack that the Android app carries.
//!
//! The phone app is one installable file. The shipped cards, plans, courses and guides travel inside it as a
//! single `.tar.gz` (installed beside the app's libraries as `libcontent_pack.so`) and are unpacked once into
//! the app's own folder, again whenever a new app version is installed. Standard library only.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Largest pack we will unpack, a guard against a damaged file filling the phone.
pub const MAX_PACK_BYTES: usize = 400 * 1024 * 1024;

fn field_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).to_string()
}

fn octal(b: &[u8]) -> Option<usize> {
    let s = field_str(b);
    let s = s.trim();
    if s.is_empty() {
        return Some(0);
    }
    usize::from_str_radix(s, 8).ok()
}

/// A path from the pack is accepted only when it stays inside the folder it is unpacked to.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    let p = Path::new(name.trim_start_matches("./"));
    if name.is_empty() || p.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() { None } else { Some(out) }
}

/// Unpack a tar (ustar or GNU long names) into `dest`. Returns how many files were written.
pub fn extract_tar(tar: &[u8], dest: &Path) -> Result<usize, String> {
    let mut pos = 0usize;
    let mut files = 0usize;
    let mut long_name: Option<String> = None;
    while pos + 512 <= tar.len() {
        let h = &tar[pos..pos + 512];
        if h.iter().all(|&c| c == 0) {
            break;
        }
        let size = octal(&h[124..136]).ok_or("the content pack is damaged (bad size)")?;
        let kind = h[156];
        let mut name = field_str(&h[0..100]);
        if &h[257..262] == b"ustar" {
            let prefix = field_str(&h[345..500]);
            if !prefix.is_empty() {
                name = format!("{}/{}", prefix, name);
            }
        }
        let data_start = pos + 512;
        let data_end = data_start.checked_add(size).ok_or("the content pack is damaged")?;
        if data_end > tar.len() {
            return Err("the content pack is cut short".into());
        }
        pos = data_start + size.div_ceil(512) * 512;
        match kind {
            b'L' => {
                long_name = Some(field_str(&tar[data_start..data_end]));
            }
            b'0' | 0 | b'5' => {
                if let Some(l) = long_name.take() {
                    name = l;
                }
                // `tar -C dir .` starts with an entry for the folder itself
                if kind == b'5' && matches!(name.trim_end_matches('/'), "" | ".") {
                    continue;
                }
                let rel = safe_relative(&name).ok_or_else(|| format!("the content pack has an unsafe path ({})", name))?;
                let target = dest.join(&rel);
                if kind == b'5' {
                    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
                } else {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    fs::write(&target, &tar[data_start..data_end]).map_err(|e| format!("could not write {}: {}", rel.display(), e))?;
                    files += 1;
                }
            }
            // links, extended headers and the like are not used by our pack: skip them
            _ => {
                long_name = None;
            }
        }
    }
    Ok(files)
}

/// Unpack a gzip-compressed tar file's bytes into `dest`.
pub fn extract_gz_tar(gz: &[u8], dest: &Path) -> Result<usize, String> {
    let tar = crate::inflate::gunzip(gz, MAX_PACK_BYTES).map_err(|e| format!("the content pack could not be opened: {:?}", e))?;
    extract_tar(&tar, dest)
}

/// Name of the file that records which pack is already unpacked.
pub const STAMP_FILE: &str = ".content-pack-id";

/// Is the folder already unpacked from this pack? `id` identifies the pack (its size and install time).
pub fn is_current(dest: &Path, id: &str) -> bool {
    fs::read_to_string(dest.join(STAMP_FILE)).map(|s| s.trim() == id).unwrap_or(false)
}

/// Unpack `pack` into `dest` unless `dest` already holds it. The old copy is replaced as a whole, so a
/// half-finished unpack never leaves a mixture: the new files go into a side folder, then it is renamed.
/// Returns Ok(true) when it unpacked, Ok(false) when it was already current.
pub fn ensure_unpacked(pack: &Path, dest: &Path, id: &str) -> Result<bool, String> {
    if is_current(dest, id) {
        return Ok(false);
    }
    let bytes = fs::read(pack).map_err(|e| format!("could not read the content pack: {}", e))?;
    let side = dest.with_extension("new");
    let _ = fs::remove_dir_all(&side);
    fs::create_dir_all(&side).map_err(|e| e.to_string())?;
    extract_gz_tar(&bytes, &side)?;
    fs::write(side.join(STAMP_FILE), id).map_err(|e| e.to_string())?;
    let old = dest.with_extension("old");
    let _ = fs::remove_dir_all(&old);
    if dest.exists() {
        fs::rename(dest, &old).map_err(|e| e.to_string())?;
    }
    fs::rename(&side, dest).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&old);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(name: &str, size: usize, kind: u8) -> Vec<u8> {
        let mut h = vec![0u8; 512];
        h[..name.len().min(100)].copy_from_slice(&name.as_bytes()[..name.len().min(100)]);
        h[100..107].copy_from_slice(b"0000644");
        h[124..135].copy_from_slice(format!("{:011o}", size).as_bytes());
        h[156] = kind;
        h[257..262].copy_from_slice(b"ustar");
        // checksum
        for b in h[148..156].iter_mut() {
            *b = b' ';
        }
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        h[148..155].copy_from_slice(format!("{:06o}\0", sum).as_bytes());
        h
    }

    fn entry(tar: &mut Vec<u8>, name: &str, data: &[u8], kind: u8) {
        tar.extend(header(name, data.len(), kind));
        tar.extend_from_slice(data);
        tar.extend(vec![0u8; (512 - data.len() % 512) % 512]);
    }

    fn finish(mut tar: Vec<u8>) -> Vec<u8> {
        tar.extend(vec![0u8; 1024]);
        tar
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cindrix-tar-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn unpacks_folders_and_files() {
        let mut t = Vec::new();
        entry(&mut t, "cards/", b"", b'5');
        entry(&mut t, "cards/boil-water.md", b"---\ntitle: Boil\n---\nBody", b'0');
        entry(&mut t, "skills.csv", b"skill,area\n", b'0');
        let d = tmp("ok");
        assert_eq!(extract_tar(&finish(t), &d).unwrap(), 2);
        assert_eq!(fs::read_to_string(d.join("cards/boil-water.md")).unwrap(), "---\ntitle: Boil\n---\nBody");
        assert_eq!(fs::read_to_string(d.join("skills.csv")).unwrap(), "skill,area\n");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_path_that_climbs_out_is_refused() {
        for bad in ["../evil.txt", "/etc/passwd", "a/../../evil.txt"] {
            let mut t = Vec::new();
            entry(&mut t, bad, b"x", b'0');
            let d = tmp("bad");
            assert!(extract_tar(&finish(t), &d).is_err(), "{}", bad);
            let _ = fs::remove_dir_all(&d);
        }
        assert!(safe_relative("academy/x.md").is_some());
        assert!(safe_relative("./academy/x.md").is_some());
        assert!(safe_relative("").is_none());
    }

    #[test]
    fn gnu_long_names_and_ustar_prefixes_work() {
        let long = format!("academy/{}/lesson.md", "a-very-long-folder-name-".repeat(6));
        let mut t = Vec::new();
        entry(&mut t, "././@LongLink", format!("{}\0", long).as_bytes(), b'L');
        entry(&mut t, "truncated-name", b"hello", b'0');
        // a ustar prefix: name field "file.md", prefix "deep/folder"
        let mut h = header("file.md", 3, b'0');
        h[345..356].copy_from_slice(b"deep/folder");
        for b in h[148..156].iter_mut() {
            *b = b' ';
        }
        let sum: u32 = h.iter().map(|&b| b as u32).sum();
        h[148..155].copy_from_slice(format!("{:06o}\0", sum).as_bytes());
        t.extend(h);
        t.extend_from_slice(b"abc");
        t.extend(vec![0u8; 509]);
        let d = tmp("long");
        assert_eq!(extract_tar(&finish(t), &d).unwrap(), 2);
        assert_eq!(fs::read_to_string(d.join(&long)).unwrap(), "hello");
        assert_eq!(fs::read_to_string(d.join("deep/folder/file.md")).unwrap(), "abc");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_cut_short_pack_is_an_error() {
        let mut t = Vec::new();
        entry(&mut t, "a.txt", &vec![7u8; 2000], b'0');
        let cut = &t[..700];
        let d = tmp("cut");
        assert!(extract_tar(cut, &d).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_real_tar_gz_made_by_the_tar_program_unpacks_and_is_only_redone_for_a_new_pack() {
        // skip quietly where tar or gzip is missing
        let ok = |c: &str| std::process::Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
        if !ok("tar") || !ok("gzip") {
            return;
        }
        let src = tmp("src");
        fs::create_dir_all(src.join("cards")).unwrap();
        fs::write(src.join("cards/a.md"), "A").unwrap();
        fs::write(src.join("b.csv"), "B").unwrap();
        let pack = std::env::temp_dir().join(format!("cindrix-pack-{}.tar.gz", std::process::id()));
        let st = std::process::Command::new("tar").args(["--format=ustar", "-czf"]).arg(&pack).arg("-C").arg(&src).arg(".").status().unwrap();
        assert!(st.success());
        let dest = std::env::temp_dir().join(format!("cindrix-dest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dest);
        assert_eq!(ensure_unpacked(&pack, &dest, "100-1").unwrap(), true);
        assert_eq!(fs::read_to_string(dest.join("cards/a.md")).unwrap(), "A");
        assert!(is_current(&dest, "100-1"));
        // same pack: nothing to do, and a file the person added is not touched
        fs::write(dest.join("mine.txt"), "x").unwrap();
        assert_eq!(ensure_unpacked(&pack, &dest, "100-1").unwrap(), false);
        assert!(dest.join("mine.txt").exists());
        // a new pack replaces the folder as a whole
        assert_eq!(ensure_unpacked(&pack, &dest, "200-2").unwrap(), true);
        assert!(!dest.join("mine.txt").exists());
        assert_eq!(fs::read_to_string(dest.join("b.csv")).unwrap(), "B");
        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&dest);
        let _ = fs::remove_file(&pack);
    }
}
