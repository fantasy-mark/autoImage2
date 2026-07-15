use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;

pub const DOCKERFILE_NAME: &str = "Dockerfile";

pub fn backup_filename(now: chrono::DateTime<chrono::Local>) -> String {
    format!("Dockerfile.bak.{}", now.format("%Y%m%d-%H%M%S"))
}

/// Reserve a unique backup path for `now` in `dir`. If a collision occurs, append
/// `.2`, `.3`, ... until the path does not exist.
pub fn next_backup_path(dir: &Path, now: chrono::DateTime<chrono::Local>) -> Result<PathBuf> {
    let base = backup_filename(now);
    let mut candidate = dir.join(&base);
    if !candidate.exists() {
        return Ok(candidate);
    }
    for n in 2..=9999 {
        let name = format!("{base}.{n}");
        candidate = dir.join(&name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    anyhow::bail!("could not find a free backup filename after 9999 attempts");
}

/// Write `contents` to `path`, creating parent directories if needed.
pub fn write_bytes(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }
    std::fs::write(path, contents).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Backup the current `Dockerfile` (if it exists) using a timestamped name in the
/// same directory. Returns the backup path that was written, or `None` if there
/// was no current file.
pub fn backup_dockerfile(dir: &Path) -> Result<Option<PathBuf>> {
    let current = dir.join(DOCKERFILE_NAME);
    if !current.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&current).with_context(|| format!("read {}", current.display()))?;
    let target = next_backup_path(dir, Local::now())?;
    write_bytes(&target, &bytes)?;
    Ok(Some(target))
}

pub fn list_backups(dir: &Path) -> Result<Vec<BackupEntry>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_backup_name(&name) {
            continue;
        }
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
        let created_at = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| chrono::DateTime::<chrono::Local>::from(t))
            .map(|t| t.to_rfc3339())
            .unwrap_or_default();
        out.push(BackupEntry { name, size, created_at });
    }
    // newest first
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

pub fn is_backup_name(name: &str) -> bool {
    let re = regex::Regex::new(r"^Dockerfile\.bak\.\d{8}-\d{6}(\.\d+)?$").unwrap();
    re.is_match(name)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupEntry {
    pub name: String,
    pub size: u64,
    pub created_at: String,
}
