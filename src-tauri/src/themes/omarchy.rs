//! Omarchy: `~/.local/state/omarchy/current/theme/colors.toml` (the current theme's palette),
//! `theme.name`, `theme.generation` (changes on every apply) and `theme/keyboard.rgb` (a single
//! hex colour the theme offers for RGB keyboards, used when `accent` is missing).

use super::sources::{Env, Source, fingerprint, hex_or_none, parse_flat_toml, read_small, which};
use super::{Palette, ThemeSource, parse_hex};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct Omarchy;

pub fn current_dir(env: &Env) -> PathBuf {
    env.state.join("omarchy/current")
}

/// Palette from the colours of a theme (`colors.toml` keys).
pub fn palette_from(colors: &BTreeMap<String, String>, keyboard_rgb: Option<&str>, name: &str) -> Result<Palette, String> {
    let c = |k: &str| hex_or_none(colors.get(k));
    let background = c("background").ok_or("colors.toml has no background")?;
    let foreground = c("foreground").ok_or("colors.toml has no foreground")?;
    let accent = c("accent").or_else(|| keyboard_rgb.and_then(parse_hex)).or_else(|| c("blue")).ok_or("colors.toml has no accent")?;
    let mut s = BTreeMap::new();
    let mut put = |k: &str, v: Option<[u8; 3]>| {
        if let Some(v) = v {
            s.insert(k.to_string(), v);
        }
    };
    put("accent", Some(accent));
    put("background", Some(background));
    put("foreground", Some(foreground));
    put("alert", c("red"));
    put("warn", c("yellow").or_else(|| c("orange")));
    put("ok", c("green"));
    put("info", c("cyan").or_else(|| c("blue")));
    put("primary", c("blue"));
    put("secondary", c("magenta"));
    put("tertiary", c("cyan"));
    for (i, k) in ["red", "orange", "yellow", "green", "cyan", "blue", "magenta"].iter().enumerate() {
        put(&format!("c{}", i + 1), c(k));
    }
    put("c8", Some(accent));
    Ok(Palette::new(name, "omarchy", s))
}

impl Source for Omarchy {
    fn id(&self) -> ThemeSource {
        ThemeSource::Omarchy
    }

    fn detect(&self, env: &Env) -> bool {
        which("omarchy") || current_dir(env).is_dir()
    }

    fn read(&self, env: &Env) -> Result<Palette, String> {
        let dir = current_dir(env);
        let colors = parse_flat_toml(&read_small(&dir.join("theme/colors.toml"))?);
        let keyboard = read_small(&dir.join("theme/keyboard.rgb")).ok();
        let name = read_small(&dir.join("theme.name")).map(|n| n.trim().to_string()).unwrap_or_default();
        palette_from(&colors, keyboard.as_deref(), &name)
    }

    fn watch_paths(&self, env: &Env) -> Vec<PathBuf> {
        let dir = current_dir(env);
        vec![dir.clone(), dir.join("theme")]
    }

    fn poll_files(&self, env: &Env) -> Vec<PathBuf> {
        let dir = current_dir(env);
        vec![dir.join("theme.generation"), dir.join("theme.name"), dir.join("theme/colors.toml")]
    }
}

/// Name of the current Omarchy theme, if any.
pub fn theme_name(env: &Env) -> Option<String> {
    read_small(&current_dir(env).join("theme.name")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Content fingerprint used by tests.
pub fn files_fingerprint(env: &Env) -> String {
    fingerprint(&Omarchy.poll_files(env))
}
