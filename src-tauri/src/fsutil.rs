//! Filesystem helpers.

use std::io::Write;
use std::path::Path;

/// Writes `data` to `path` atomically: temp file in the same directory, fsync, rename.
/// A crash mid-write leaves the previous file intact.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.tmp"));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The user's home directory: `$HOME`, else `%USERPROFILE%` (Windows).
pub fn home_dir() -> std::path::PathBuf {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).or_else(|| std::env::var_os("USERPROFILE")).map(Into::into).unwrap_or_default()
}

/// `$XDG_CONFIG_HOME`, else the platform's per-user config root (`~/.config`, `%APPDATA%`,
/// `~/Library/Application Support`).
pub fn config_root() -> std::path::PathBuf {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        return x.into();
    }
    if cfg!(windows)
        && let Some(a) = std::env::var_os("APPDATA")
    {
        return a.into();
    }
    if cfg!(target_os = "macos") {
        return home_dir().join("Library/Application Support");
    }
    home_dir().join(".config")
}

/// A fresh empty directory under the system temp dir (tests only).
#[cfg(test)]
pub fn test_dir(tag: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!("chroma-desk-test-{}-{tag}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_replaces_without_leftovers() {
        let d = test_dir("atomic");
        let p = d.join("a.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "two");
        let names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1, "temp file must be gone: {names:?}");
    }

    #[test]
    fn creates_missing_directories() {
        let d = test_dir("atomic-dirs");
        let p = d.join("x/y/z.json");
        write_atomic(&p, b"{}").unwrap();
        assert!(p.exists());
    }
}
