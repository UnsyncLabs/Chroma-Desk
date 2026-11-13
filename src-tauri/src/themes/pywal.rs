//! pywal (`~/.cache/wal/colors.json`), the generic route for XFCE, i3, sway and friends.
//! pywal's colours come from the wallpaper, so the slot meaning is by convention:
//! `color1` red-ish (accent/alert), `color2` green, `color3` yellow, `color4` blue,
//! `color5` magenta, `color6` cyan.

use super::sources::{Env, Source, hex_or_none, read_small};
use super::{Palette, ThemeSource};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct Pywal;

fn path(env: &Env) -> PathBuf {
    env.cache.join("wal/colors.json")
}

pub fn palette_from(text: &str) -> Result<Palette, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("colors.json: {e}"))?;
    let special = |k: &str| v["special"][k].as_str().map(str::to_string);
    let color = |i: usize| hex_or_none(v["colors"][format!("color{i}")].as_str().map(str::to_string).as_ref());
    let background = hex_or_none(special("background").as_ref()).or_else(|| color(0)).ok_or("colors.json has no background")?;
    let foreground = hex_or_none(special("foreground").as_ref()).or_else(|| color(7)).ok_or("colors.json has no foreground")?;
    let mut s = BTreeMap::new();
    let mut put = |k: &str, v: Option<[u8; 3]>| {
        if let Some(v) = v {
            s.insert(k.to_string(), v);
        }
    };
    put("background", Some(background));
    put("foreground", Some(foreground));
    put("accent", color(1).or_else(|| color(4)));
    put("primary", color(4));
    put("secondary", color(5));
    put("tertiary", color(6));
    put("alert", color(1));
    put("warn", color(3));
    put("ok", color(2));
    put("info", color(6));
    for (i, n) in [1, 2, 3, 4, 5, 6, 13, 14].iter().enumerate() {
        put(&format!("c{}", i + 1), color(*n));
    }
    if s.len() <= 2 {
        return Err("colors.json has no colour entries".into());
    }
    Ok(Palette::new("pywal", "pywal", s))
}

impl Source for Pywal {
    fn id(&self) -> ThemeSource {
        ThemeSource::Pywal
    }
    fn detect(&self, env: &Env) -> bool {
        path(env).is_file()
    }
    fn read(&self, env: &Env) -> Result<Palette, String> {
        palette_from(&read_small(&path(env))?)
    }
    fn watch_paths(&self, env: &Env) -> Vec<PathBuf> {
        vec![env.cache.join("wal")]
    }
    fn poll_files(&self, env: &Env) -> Vec<PathBuf> {
        vec![path(env)]
    }
}
