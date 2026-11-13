//! Theme sources: where a palette can be read from. Sources only *read* the desktop's own
//! files; nothing here writes to them.

use super::{Palette, Rgb3, ThemeSource, parse_hex};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the sources look (injectable so tests use fixture directories).
#[derive(Debug, Clone)]
pub struct Env {
    pub home: PathBuf,
    /// `$XDG_STATE_HOME` (Omarchy keeps its current theme there).
    pub state: PathBuf,
    /// `$XDG_CACHE_HOME` (pywal).
    pub cache: PathBuf,
    /// `$XDG_CONFIG_HOME` (kdeglobals).
    pub config: PathBuf,
}

impl Env {
    pub fn from_env() -> Env {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        let dir = |var: &str, rel: &str| std::env::var_os(var).filter(|v| !v.is_empty()).map_or_else(|| home.join(rel), PathBuf::from);
        Env { state: dir("XDG_STATE_HOME", ".local/state"), cache: dir("XDG_CACHE_HOME", ".cache"), config: dir("XDG_CONFIG_HOME", ".config"), home }
    }

    /// Everything below `root` (tests).
    pub fn under(root: &Path) -> Env {
        Env { home: root.to_path_buf(), state: root.join(".local/state"), cache: root.join(".cache"), config: root.join(".config") }
    }

    /// `~/.config/omarchy/hooks/theme-set.d` (Omarchy's own tools use `$HOME/.config`).
    pub fn omarchy_hook_dir(&self) -> PathBuf {
        self.home.join(".config/omarchy/hooks/theme-set.d")
    }
}

/// A place a palette can come from.
pub trait Source {
    fn id(&self) -> ThemeSource;
    /// Whether the desktop/tool this source reads is present.
    fn detect(&self, env: &Env) -> bool;
    /// The current palette; an error explains why it could not be read.
    fn read(&self, env: &Env) -> Result<Palette, String>;
    /// Directories to watch for changes (watching the directory survives atomic file replaces).
    fn watch_paths(&self, env: &Env) -> Vec<PathBuf>;
    /// Files whose `(mtime, size)` signal a change when inotify does not (polled).
    fn poll_files(&self, env: &Env) -> Vec<PathBuf> {
        let _ = env;
        vec![]
    }
}

/// The source implementation for `id` (`Manual` and `External` have none).
pub fn source(id: ThemeSource) -> Option<&'static dyn Source> {
    match id {
        ThemeSource::Omarchy => Some(&super::omarchy::Omarchy),
        ThemeSource::Kde => Some(&super::kde::Kde),
        ThemeSource::Pywal => Some(&super::pywal::Pywal),
        ThemeSource::Gtk => Some(&super::gtk::Gtk),
        ThemeSource::Manual | ThemeSource::External => None,
    }
}

/// Reads a text file of reasonable size.
pub fn read_small(p: &Path) -> Result<String, String> {
    let len = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?.len();
    if len > 1024 * 1024 {
        return Err(format!("{}: file too large", p.display()));
    }
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

/// `key = "value"` lines of a flat TOML file (sections and comments are skipped).
pub fn parse_flat_toml(text: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim();
            let v = match v.chars().next() {
                // A quoted value ends at the closing quote (a `#` inside is part of it).
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or("").to_string(),
                _ => v.split(" #").next().unwrap_or("").trim().to_string(),
            };
            m.insert(k.trim().to_string(), v);
        }
    }
    m
}

/// `[Section]` / `key=value` (KDE's `kdeglobals`): `section -> key -> value`.
pub fn parse_ini(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut cur = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            cur = s.to_string();
        } else if let Some((k, v)) = line.split_once('=') {
            out.entry(cur.clone()).or_default().insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// `r,g,b` (KDE) to a colour.
pub fn parse_csv_rgb(s: &str) -> Option<Rgb3> {
    let mut it = s.split(',').map(|p| p.trim().parse::<u8>());
    let c = [it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?];
    Some(c)
}

pub fn hex_or_none(v: Option<&String>) -> Option<Rgb3> {
    v.and_then(|s| parse_hex(s))
}

/// `(mtime, size)` of each file, as text; files that do not exist contribute `-`.
pub fn fingerprint(files: &[PathBuf]) -> String {
    files
        .iter()
        .map(|p| match std::fs::metadata(p) {
            Ok(m) => format!("{}:{:?}", m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos())),
            Err(_) => "-".into(),
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// Whether `name` is an executable found on `PATH`.
pub fn which(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}
