//! Memory and CPU detection, and which model file to use.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Size {
    B2,
    B4,
    B9,
}

impl Size {
    pub fn label(&self) -> &'static str {
        match self {
            Size::B2 => "2B",
            Size::B4 => "4B",
            Size::B9 => "9B",
        }
    }
}

/// Which size a .gguf file name says it is ("qwen3.5-2b-q4_k_m.gguf" -> 2B).
pub fn size_of(file_name: &str) -> Option<Size> {
    let n = file_name.to_lowercase();
    if !n.ends_with(".gguf") {
        return None;
    }
    let find = |tag: &str| {
        n.match_indices(tag).any(|(i, _)| {
            let before = n[..i].chars().last();
            let after = n[i + tag.len()..].chars().next();
            !before.map(|c| c.is_ascii_digit() || c == '.').unwrap_or(false) && !after.map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        })
    };
    if find("9b") {
        Some(Size::B9)
    } else if find("4b") {
        Some(Size::B4)
    } else if find("2b") {
        Some(Size::B2)
    } else {
        None
    }
}

/// Default model for this much memory: 2B under 12 GB, 4B from 12 GB. 9B is only used when chosen by hand.
pub fn default_size(ram_gb: f64) -> Size {
    if ram_gb >= 12.0 { Size::B4 } else { Size::B2 }
}

/// Pick a file from the models folder: the wanted size, else the largest that is not bigger than
/// what the memory allows, else the smallest there is.
pub fn choose_model(ram_gb: f64, files: &[String], wanted: Option<Size>) -> Option<(String, Size)> {
    let mut found: Vec<(String, Size)> = files.iter().filter_map(|f| size_of(f).map(|s| (f.clone(), s))).collect();
    if found.is_empty() {
        return None;
    }
    found.sort_by_key(|x| x.1);
    let want = wanted.unwrap_or_else(|| default_size(ram_gb));
    if let Some(x) = found.iter().find(|x| x.1 == want) {
        return Some(x.clone());
    }
    let allowed = if ram_gb >= 16.0 { Size::B9 } else if ram_gb >= 12.0 { Size::B4 } else { Size::B2 };
    if let Some(x) = found.iter().rev().find(|x| x.1 <= allowed) {
        return Some(x.clone());
    }
    found.into_iter().next()
}

/// Context length that fits in this much memory.
pub fn context_for(ram_gb: f64) -> u32 {
    if ram_gb < 6.0 { 2048 } else { 4096 }
}

/// A sensible thread count: half the logical processors (about the physical cores), 2 to 8.
pub fn threads() -> usize {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    (n / 2).clamp(2, 8)
}

/// Total physical memory in GB, if it can be read.
pub fn total_ram_gb() -> Option<f64> {
    total_ram_bytes().map(|b| b as f64 / 1_073_741_824.0)
}

#[cfg(windows)]
fn total_ram_bytes() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatusEx {
        length: u32,
        memory_load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(buffer: *mut MemoryStatusEx) -> i32;
    }
    let mut m = MemoryStatusEx {
        length: std::mem::size_of::<MemoryStatusEx>() as u32,
        memory_load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0,
        total_virtual: 0, avail_virtual: 0, avail_extended: 0,
    };
    // SAFETY: m is a correctly sized, initialised MEMORYSTATUSEX with `length` set, as the call requires.
    let ok = unsafe { GlobalMemoryStatusEx(&mut m) };
    if ok != 0 { Some(m.total_phys) } else { None }
}

#[cfg(target_os = "linux")]
fn total_ram_bytes() -> Option<u64> {
    let t = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = t.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn total_ram_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn reads_size_from_file_names() {
        assert_eq!(size_of("Qwen3.5-2B-Q4_K_M.gguf"), Some(Size::B2));
        assert_eq!(size_of("qwen3.5-4b-instruct-q4.gguf"), Some(Size::B4));
        assert_eq!(size_of("qwen3.5-9B.gguf"), Some(Size::B9));
        assert_eq!(size_of("qwen3.5-32b.gguf"), None);
        assert_eq!(size_of("notes.txt"), None);
    }

    #[test]
    fn eight_gb_gets_the_2b() {
        let files = f(&["qwen-2b.gguf", "qwen-4b.gguf", "qwen-9b.gguf"]);
        assert_eq!(choose_model(7.8, &files, None).unwrap().1, Size::B2);
    }

    #[test]
    fn sixteen_gb_gets_the_4b_by_default_and_9b_when_asked() {
        let files = f(&["qwen-2b.gguf", "qwen-4b.gguf", "qwen-9b.gguf"]);
        assert_eq!(choose_model(15.8, &files, None).unwrap().1, Size::B4);
        assert_eq!(choose_model(15.8, &files, Some(Size::B9)).unwrap().1, Size::B9);
    }

    #[test]
    fn falls_back_to_what_is_there() {
        assert_eq!(choose_model(15.8, &f(&["qwen-2b.gguf"]), None).unwrap().1, Size::B2);
        assert_eq!(choose_model(7.8, &f(&["qwen-9b.gguf"]), None).unwrap().1, Size::B9);
        assert_eq!(choose_model(7.8, &f(&["readme.txt"]), None), None);
    }

    #[test]
    fn small_memory_gets_small_context() {
        assert_eq!(context_for(4.0), 2048);
        assert_eq!(context_for(8.0), 4096);
    }

    #[test]
    fn threads_in_range() {
        let t = threads();
        assert!((2..=8).contains(&t));
    }
}
