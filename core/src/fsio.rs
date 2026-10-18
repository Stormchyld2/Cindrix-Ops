//! Safe file writes: write to a temporary file, then rename. Keeps a dated backup first.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Write `data` to `path` so a power loss never leaves a half-written file.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp: PathBuf = path.to_path_buf();
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    tmp.set_file_name(format!("{}.tmp", name));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Copy the current file into `backups_dir` with a timestamp before it is replaced.
pub fn backup_then_write(path: &Path, backups_dir: &Path, data: &[u8]) -> std::io::Result<()> {
    if path.exists() {
        fs::create_dir_all(backups_dir)?;
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        fs::copy(path, backups_dir.join(format!("{}-{}", secs, name)))?;
    }
    write_atomic(path, data)
}
