//! KDE Plasma: `~/.config/kdeglobals` (`[Colors:View]`, `[Colors:Selection]`, `[General]`, `[WM]`;
//! colours are `r,g,b`).

use super::sources::{Env, Source, parse_csv_rgb, parse_ini, read_small};
use super::{Palette, ThemeSource};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct Kde;

fn path(env: &Env) -> PathBuf {
    env.config.join("kdeglobals")
}

pub fn palette_from(text: &str) -> Result<Palette, String> {
    let ini = parse_ini(text);
    let get = |sec: &str, key: &str| ini.get(sec).and_then(|s| s.get(key)).and_then(|v| parse_csv_rgb(v));
    let view = |k: &str| get("Colors:View", k);
    let background = view("BackgroundNormal").ok_or("kdeglobals has no [Colors:View] BackgroundNormal")?;
    let foreground = view("ForegroundNormal").ok_or("kdeglobals has no [Colors:View] ForegroundNormal")?;
    let accent = get("General", "AccentColor")
        .or_else(|| view("DecorationFocus"))
        .or_else(|| get("WM", "activeBackground"))
        .or_else(|| get("Colors:Selection", "BackgroundNormal"))
        .ok_or("kdeglobals has no accent colour")?;
    let mut s = BTreeMap::new();
    let mut put = |k: &str, v: Option<[u8; 3]>| {
        if let Some(v) = v {
            s.insert(k.to_string(), v);
        }
    };
    put("background", Some(background));
    put("foreground", Some(foreground));
    put("accent", Some(accent));
    put("primary", view("ForegroundLink").or(Some(accent)));
    put("secondary", view("ForegroundVisited"));
    put("tertiary", view("DecorationHover").or_else(|| view("ForegroundActive")));
    put("alert", view("ForegroundNegative"));
    put("warn", view("ForegroundNeutral"));
    put("ok", view("ForegroundPositive"));
    put("info", view("ForegroundLink"));
    for (i, k) in ["ForegroundNegative", "ForegroundNeutral", "ForegroundActive", "ForegroundPositive", "DecorationFocus", "ForegroundLink", "ForegroundVisited"].iter().enumerate() {
        put(&format!("c{}", i + 1), view(k));
    }
    put("c8", Some(accent));
    let name = ini.get("General").and_then(|g| g.get("ColorScheme")).cloned().unwrap_or_else(|| "KDE".into());
    Ok(Palette::new(&name, "kde", s))
}

impl Source for Kde {
    fn id(&self) -> ThemeSource {
        ThemeSource::Kde
    }
    fn detect(&self, env: &Env) -> bool {
        path(env).is_file()
    }
    fn read(&self, env: &Env) -> Result<Palette, String> {
        palette_from(&read_small(&path(env))?)
    }
    fn watch_paths(&self, env: &Env) -> Vec<PathBuf> {
        vec![env.config.clone()]
    }
    fn poll_files(&self, env: &Env) -> Vec<PathBuf> {
        vec![path(env)]
    }
}
