//! GNOME/GTK (basic): the accent colour of `org.gnome.desktop.interface accent-color`
//! (read with `gsettings`); the rest of the palette is derived from it and from the colour scheme.

use super::sources::{Env, Source, which};
use super::{Palette, ThemeSource, parse_hex};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

pub struct Gtk;

/// GNOME's named accent colours.
pub fn accent_hex(name: &str) -> Option<&'static str> {
    Some(match name {
        "blue" => "#3584e4",
        "teal" => "#2190a4",
        "green" => "#3a944a",
        "yellow" => "#c88800",
        "orange" => "#ed5b00",
        "red" => "#e62d42",
        "pink" => "#d56199",
        "purple" => "#9141ac",
        "slate" => "#6f8396",
        _ => return None,
    })
}

/// `'blue'` (gsettings output) to `blue`.
fn unquote(s: &str) -> &str {
    s.trim().trim_matches('\'')
}

pub fn palette_from(accent: &str, scheme: &str) -> Result<Palette, String> {
    let accent_c = parse_hex(accent_hex(unquote(accent)).ok_or_else(|| format!("unknown accent colour {accent:?}"))?).ok_or("bad accent")?;
    let dark = unquote(scheme).contains("dark");
    let mut s = BTreeMap::new();
    s.insert("accent".to_string(), accent_c);
    s.insert("primary".to_string(), accent_c);
    s.insert("background".to_string(), if dark { [36, 36, 36] } else { [250, 250, 250] });
    s.insert("foreground".to_string(), if dark { [255, 255, 255] } else { [46, 52, 54] });
    for (k, h) in [("alert", "red"), ("warn", "orange"), ("ok", "green"), ("secondary", "purple"), ("tertiary", "teal"), ("info", "blue")] {
        if let Some(c) = accent_hex(h).and_then(parse_hex) {
            s.insert(k.to_string(), c);
        }
    }
    Ok(Palette::new(&format!("GNOME {}", unquote(accent)), "gtk", s))
}

fn gsettings(key: &str) -> Result<String, String> {
    let out = Command::new("gsettings").args(["get", "org.gnome.desktop.interface", key]).output().map_err(|e| format!("gsettings: {e}"))?;
    if !out.status.success() {
        return Err(format!("gsettings get {key} failed"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl Source for Gtk {
    fn id(&self) -> ThemeSource {
        ThemeSource::Gtk
    }
    fn detect(&self, _env: &Env) -> bool {
        which("gsettings") && gsettings("accent-color").is_ok_and(|v| accent_hex(unquote(&v)).is_some())
    }
    fn read(&self, _env: &Env) -> Result<Palette, String> {
        palette_from(&gsettings("accent-color")?, &gsettings("color-scheme").unwrap_or_default())
    }
    /// dconf's database directory.
    fn watch_paths(&self, env: &Env) -> Vec<PathBuf> {
        vec![env.config.join("dconf")]
    }
    /// The user's dconf database changes when the accent or scheme does; polling its fingerprint
    /// avoids running `gsettings` every second.
    fn poll_files(&self, env: &Env) -> Vec<PathBuf> {
        vec![env.config.join("dconf/user")]
    }
}
