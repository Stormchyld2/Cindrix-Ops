//! Small, testable helpers for running the assistant engine on a phone.
//!
//! On Android the engine (llama-server) is not a loose file on the drive. It is built into the app and
//! installed next to the app's own libraries, under the name `libllama_server.so`. The app finds that folder
//! by looking at where its own library was loaded from. It also reads memory figures for the
//! "Check the assistant" page.

use std::path::PathBuf;

/// File name the engine has inside the Android app (a program dressed as a library so the app installer keeps it).
pub const ANDROID_ENGINE_FILE: &str = "libllama_server.so";

/// The folder an app library was loaded from, read from the text of `/proc/self/maps`.
/// `lib_stem` is the library's name without `lib` and `.so`, for example `cindrix_ops_lib`.
pub fn native_dir_from_maps(maps: &str, lib_stem: &str) -> Option<PathBuf> {
    let wanted = format!("/lib{}.so", lib_stem);
    for line in maps.lines() {
        let path = line.split_whitespace().last().unwrap_or("");
        if path.starts_with('/') && path.ends_with(&wanted) {
            return std::path::Path::new(path).parent().map(|p| p.to_path_buf());
        }
    }
    None
}

/// A figure in kilobytes from `/proc/meminfo` or `/proc/<pid>/status` (for example `MemAvailable` or `VmRSS`).
pub fn kb_field(text: &str, key: &str) -> Option<u64> {
    let prefix = format!("{}:", key);
    let line = text.lines().find(|l| l.starts_with(&prefix))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

pub fn kb_to_gb(kb: u64) -> f64 {
    kb as f64 / 1_048_576.0
}

/// Words per second, or 0 when no time passed.
pub fn per_second(count: usize, seconds: f64) -> f64 {
    if seconds <= 0.0 { 0.0 } else { count as f64 / seconds }
}

/// A rough count of the pieces of an answer (words), enough to say how fast the assistant writes.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPS: &str = "\
7a2c000000-7a2c001000 r--p 00000000 00:00 0                              [anon:linker_alloc]
7a30000000-7a30200000 r-xp 00000000 fd:0b 4242 /data/app/~~Xy12==/org.cindrix.ops-AbC==/lib/arm64/libcindrix_ops_lib.so
7a30200000-7a30210000 r--p 00200000 fd:0b 4242 /data/app/~~Xy12==/org.cindrix.ops-AbC==/lib/arm64/libcindrix_ops_lib.so
7a40000000-7a40010000 r-xp 00000000 fd:0b 17 /system/lib64/libc.so";

    #[test]
    fn finds_the_folder_the_app_library_was_loaded_from() {
        let d = native_dir_from_maps(MAPS, "cindrix_ops_lib").unwrap();
        assert_eq!(d, PathBuf::from("/data/app/~~Xy12==/org.cindrix.ops-AbC==/lib/arm64"));
        assert_eq!(d.join(ANDROID_ENGINE_FILE).file_name().unwrap(), "libllama_server.so");
    }

    #[test]
    fn no_match_gives_none_and_a_longer_name_does_not_match() {
        assert_eq!(native_dir_from_maps(MAPS, "other_lib"), None);
        assert_eq!(native_dir_from_maps("", "cindrix_ops_lib"), None);
        // "libxcindrix_ops_lib.so" is a different library
        assert_eq!(native_dir_from_maps("1-2 r-xp 0 fd:0 1 /x/libxcindrix_ops_lib.so", "cindrix_ops_lib"), None);
    }

    #[test]
    fn reads_memory_figures() {
        let info = "MemTotal:        7812344 kB\nMemFree:          210000 kB\nMemAvailable:    3145728 kB\n";
        assert_eq!(kb_field(info, "MemTotal"), Some(7_812_344));
        assert_eq!(kb_field(info, "MemAvailable"), Some(3_145_728));
        assert_eq!(kb_field(info, "VmRSS"), None);
        assert!((kb_to_gb(3_145_728) - 3.0).abs() < 1e-9);
        let status = "Name:\tllama-server\nVmRSS:\t 1843200 kB\n";
        assert_eq!(kb_field(status, "VmRSS"), Some(1_843_200));
    }

    #[test]
    fn speed_helpers_do_not_divide_by_zero() {
        assert_eq!(per_second(10, 0.0), 0.0);
        assert!((per_second(30, 6.0) - 5.0).abs() < 1e-9);
        assert_eq!(word_count("  one two\nthree "), 3);
        assert_eq!(word_count(""), 0);
    }
}
