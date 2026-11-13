//! Built-in profiles shipped in the binary: read-only, themed, duplicable.
//!
//! A profile file stores scenes by device identity, which a built-in cannot know in advance, so
//! the scene of a built-in is *generated* for the devices that are connected when it is applied
//! (`scene_for`). Duplicating one freezes that scene into an ordinary, editable profile.

use crate::engine::compositor::{Blend, Layer, Mask};
use crate::engine::{DevInfo, DeviceScene, Scene, SceneMode, effects, overlay};
use crate::profiles::{AnimRef, NamedTrigger, Profile};
use crate::themes::{ProfileTheme, SlotRef, ThemeSource};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const OMARCHY: &str = "Omarchy (follow theme)";
pub const CYBERPUNK: &str = "Cyberpunk";

/// Whether `name` is the name of a built-in profile.
pub fn is_builtin(name: &str) -> bool {
    all().iter().any(|p| p.name.eq_ignore_ascii_case(name.trim()))
}

fn tr(anim: &str, color: Option<&str>, secs: f64) -> NamedTrigger {
    NamedTrigger { anim: anim.into(), keys: vec![], color: color.map(str::to_string), duration: Some(secs), device: None, ..NamedTrigger::new("") }
}

fn presets(ids: &[(&str, &str)]) -> Vec<AnimRef> {
    ids.iter().map(|(id, name)| AnimRef { id: format!("builtin:{id}"), name: (*name).into() }).collect()
}

/// The built-in profiles (their scene is empty: see [`scene_for`]).
pub fn all() -> Vec<Profile> {
    let hex = |s: &str| crate::themes::parse_hex(s).unwrap_or([0; 3]);
    let neon: BTreeMap<String, [u8; 3]> = [
        ("background", "#0a0612"),
        ("foreground", "#e8e0ff"),
        ("accent", "#fcee0a"),
        ("primary", "#00f0ff"),
        ("secondary", "#ff2a6d"),
        ("tertiary", "#7b2cff"),
        ("alert", "#ff003c"),
        ("warn", "#ff8a00"),
        ("ok", "#00ff9f"),
        ("info", "#00f0ff"),
        ("c1", "#ff003c"),
        ("c2", "#ff8a00"),
        ("c3", "#fcee0a"),
        ("c4", "#00ff9f"),
        ("c5", "#00f0ff"),
        ("c6", "#3a7bff"),
        ("c7", "#7b2cff"),
        ("c8", "#ff2a6d"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), hex(v)))
    .collect();

    let mut omarchy = Profile::new(OMARCHY, Scene::default());
    omarchy.builtin = true;
    omarchy.enabled = false;
    omarchy.requires = Some("omarchy".into());
    omarchy.theme = ProfileTheme { source: ThemeSource::Omarchy, manual_palette: None, overrides: BTreeMap::new() };
    omarchy.presets = presets(&[("accent-breathing", "Accent breathing"), ("theme-wave", "Theme wave"), ("theme-chase", "Theme chase"), ("starlight", "Starlight")]);
    omarchy.triggers = [("notify", tr("Flash", Some("@info"), 1.5)), ("alert", tr("Alert", Some("@alert"), 3.0)), ("success", tr("Pulse", Some("@ok"), 1.5))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();

    let mut cyber = Profile::new(CYBERPUNK, Scene::default());
    cyber.builtin = true;
    cyber.theme = ProfileTheme { source: ThemeSource::Manual, manual_palette: Some(neon), overrides: BTreeMap::new() };
    cyber.presets = presets(&[("neon-scan", "Neon Scan"), ("breach-alert", "Breach Alert"), ("glitch", "Glitch"), ("theme-wave", "Theme wave"), ("theme-chase", "Theme chase")]);
    cyber.triggers = [("notify", tr("Neon Scan", None, 2.5)), ("alert", tr("Breach Alert", None, 3.0)), ("success", tr("Pulse", Some("@ok"), 1.5))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();

    vec![omarchy, cyber]
}

/// Cards for the built-ins this machine can offer (`requires` met), with the user's on/off choice.
pub fn infos(settings: &crate::config::Settings, themes: &crate::themes::ctl::ThemeCtl) -> Vec<crate::profiles::ProfileInfo> {
    all()
        .into_iter()
        .filter(|p| available(p, themes))
        .map(|mut p| {
            p.enabled = settings.builtin_on(&p);
            crate::profiles::ProfileInfo::of(&p, String::new())
        })
        .collect()
}

/// Whether the desktop a built-in needs is present.
pub fn available(p: &Profile, themes: &crate::themes::ctl::ThemeCtl) -> bool {
    match p.requires.as_deref() {
        Some("omarchy") => themes.detected(ThemeSource::Omarchy),
        Some("kde") => themes.detected(ThemeSource::Kde),
        _ => true,
    }
}

pub fn find(name: &str) -> Option<Profile> {
    all().into_iter().find(|p| p.name.eq_ignore_ascii_case(name.trim()))
}

fn slot(name: &str) -> Value {
    json!({ "slot": name })
}

fn lit(name: &str, lighten: f32) -> Value {
    serde_json::to_value(SlotRef { slot: name.into(), lighten: Some(lighten) }).unwrap_or(Value::Null)
}

fn layer(id: &str, effect: &str, params: Value, opacity: f32, mask: Mask) -> Layer {
    Layer { id: id.into(), name: String::new(), enabled: true, opacity, blend: Blend::Normal, mask, effect: effect.into(), params: effects::Params(params.as_object().cloned().unwrap_or_default()) }
}

/// Keys that get the accent colour (gaming cluster and arrows).
const KEYS: [&str; 8] = ["W", "A", "S", "D", "Up Arrow", "Down Arrow", "Left Arrow", "Right Arrow"];

fn key_mask(dev: &DevInfo) -> Mask {
    let keys: Vec<String> = KEYS.iter().map(|k| k.to_string()).collect();
    let leds = overlay::resolve_keys(&keys, &dev.led_names);
    if leds.is_empty() { Mask::Leds { leds: vec![] } } else { Mask::Leds { leds } }
}

fn layers_for(profile: &str, dev: &DevInfo) -> Vec<Layer> {
    let cyber = profile == CYBERPUNK;
    match dev.device_type.as_str() {
        "Keyboard" if cyber => vec![
            layer("bg", "solid", json!({ "color": lit("tertiary", -0.78) }), 1.0, Mask::All),
            layer("scan", "color_wave", json!({ "colors": [slot("primary"), slot("secondary"), slot("primary")], "speed": 0.35, "angle": 90, "scale": 1.5 }), 0.8, Mask::All),
            layer("keys", "solid", json!({ "color": slot("accent") }), 1.0, key_mask(dev)),
            layer("glitch", "glitch", json!({ "colors": [slot("primary"), slot("secondary"), slot("accent")], "interval": 6.0, "duration": 0.25, "density": 0.3, "size": 0.14 }), 1.0, Mask::All),
        ],
        "Keyboard" => vec![
            layer("bg", "solid", json!({ "color": lit("background", 0.14) }), 1.0, Mask::All),
            layer("wave", "color_wave", json!({ "colors": [slot("accent"), slot("secondary"), slot("accent")], "speed": 0.12, "angle": 0, "scale": 1.0 }), 0.7, Mask::All),
            layer("keys", "solid", json!({ "color": slot("accent") }), 1.0, key_mask(dev)),
        ],
        "LEDStrip" | "Cooler" | "Light" | "Unknown" if cyber => vec![layer("chase", "chase", json!({ "colors": [slot("primary"), slot("secondary")], "speed": 0.5, "length": 0.3, "gap": 0.35 }), 1.0, Mask::All)],
        "LEDStrip" | "Cooler" | "Light" | "Unknown" => vec![layer("chase", "chase", json!({ "colors": [slot("primary")], "speed": 0.5, "length": 0.3, "gap": 0.4 }), 1.0, Mask::All)],
        "DRam" | "Motherboard" if cyber => vec![layer("wave", "color_wave", json!({ "colors": [slot("primary"), slot("secondary"), slot("tertiary")], "speed": 0.3, "angle": 0, "scale": 1.0 }), 1.0, Mask::All)],
        "Gpu" if cyber => vec![layer("wave", "color_wave", json!({ "colors": [slot("secondary"), slot("tertiary"), slot("primary")], "speed": 0.25, "angle": 0, "scale": 1.0 }), 1.0, Mask::All)],
        _ if cyber => vec![layer("breath", "breathing", json!({ "colors": [slot("accent"), slot("secondary")], "period": 5.0 }), 1.0, Mask::All)],
        _ => vec![layer("breath", "breathing", json!({ "colors": [slot("accent")], "period": 5.0 }), 1.0, Mask::All)],
    }
}

/// The scene of built-in profile `name` for the connected devices.
pub fn scene_for(name: &str, devs: &[DevInfo]) -> Scene {
    let canonical = find(name).map(|p| p.name).unwrap_or_default();
    let mut scene = Scene::default();
    for d in devs {
        scene.devices.insert(d.key.clone(), DeviceScene { mode: SceneMode::Effects, layers: layers_for(&canonical, d), ..Default::default() });
    }
    scene
}

/// A user profile frozen from built-in `name` on the connected devices.
pub fn materialize(name: &str, devs: &[DevInfo], new_name: &str) -> Option<Profile> {
    let mut p = find(name)?;
    p.scene = scene_for(&p.name, devs);
    p.name = new_name.into();
    p.builtin = false;
    p.enabled = true;
    p.requires = None;
    Some(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Tick, render_device, resolve_scene};
    use crate::themes::Palette;

    fn dev(key: &str, ty: &str, names: &[&str]) -> DevInfo {
        let n = names.len().max(8);
        DevInfo {
            index: 0,
            key: key.into(),
            name: key.into(),
            device_type: ty.into(),
            led_count: n,
            led_names: (0..n).map(|i| names.get(i).unwrap_or(&"").to_string()).collect(),
            zones: vec![(0, 0, n)],
            default_positions: (0..n).map(|i| [i as f32 / n as f32, 0.5]).collect(),
        }
    }

    fn devices() -> Vec<DevInfo> {
        vec![dev("kb", "Keyboard", &["Key: Q", "Key: W", "Key: A", "Key: S", "Key: D", "Key: Up Arrow"]), dev("fans", "LEDStrip", &[]), dev("ram", "DRam", &[]), dev("gpu", "Gpu", &[]), dev("mouse", "Mouse", &[])]
    }

    #[test]
    fn listing_and_flags() {
        let all = all();
        assert_eq!(all.len(), 2);
        let o = find("omarchy (FOLLOW theme)").unwrap();
        assert!(o.builtin && !o.enabled && o.requires.as_deref() == Some("omarchy") && o.theme.source == ThemeSource::Omarchy);
        let c = find(CYBERPUNK).unwrap();
        assert!(c.builtin && c.enabled && c.theme.source == ThemeSource::Manual);
        assert_eq!(c.theme.manual_palette.as_ref().unwrap()["accent"], [0xfc, 0xee, 0x0a]);
        assert_eq!(c.theme.manual_palette.as_ref().unwrap()["background"], [0x0a, 0x06, 0x12]);
        assert!(is_builtin("cyberpunk") && !is_builtin("Gaming"));
        for p in &all {
            p.validate_extras().unwrap();
            assert!(p.triggers.contains_key("alert") && p.triggers.contains_key("notify") && p.triggers.contains_key("success"));
            for t in p.triggers.values() {
                assert!(crate::animations::lookup(&crate::fsutil::test_dir("bp-anim"), &t.anim).is_ok(), "trigger animation {} exists", t.anim);
            }
            for a in &p.presets {
                assert!(crate::animations::get(&crate::fsutil::test_dir("bp-anim2"), &a.id).is_ok(), "preset {} exists", a.id);
            }
        }
    }

    #[test]
    fn scenes_use_slots_and_render_with_the_profile_palette() {
        let devs = devices();
        for name in [OMARCHY, CYBERPUNK] {
            let scene = scene_for(name, &devs);
            assert_eq!(scene.devices.len(), devs.len());
            assert!(crate::themes::contains_slot(&serde_json::to_value(&scene).unwrap()), "{name} is made of slots");
            let theme = find(name).unwrap().theme;
            let pal = if name == CYBERPUNK { theme.manual() } else { Palette::default_manual() };
            let resolved = resolve_scene(&scene, &pal);
            for d in &devs {
                let out = render_device(&resolved, d, None, None, &Tick::plain(1.0, false)).unwrap();
                assert!(out.iter().any(|c| *c != [0, 0, 0]), "{name}/{} is lit", d.key);
            }
        }
        // Keyboard WASD/arrows get a mask on the real LED names; Cyberpunk has the glitch layer.
        let kb = &scene_for(CYBERPUNK, &devs).devices["kb"];
        assert_eq!(kb.layers.iter().map(|l| l.effect.as_str()).collect::<Vec<_>>(), ["solid", "color_wave", "solid", "glitch"]);
        assert_eq!(kb.layers[2].mask, Mask::Leds { leds: vec![1, 2, 3, 4, 5] });
        let o = &scene_for(OMARCHY, &devs).devices["fans"];
        assert_eq!(o.layers[0].effect, "chase");
        // The two profiles really differ on the same device.
        assert_ne!(scene_for(OMARCHY, &devs).devices["kb"], scene_for(CYBERPUNK, &devs).devices["kb"]);
    }

    #[test]
    fn omarchy_scene_follows_the_palette() {
        let devs = devices();
        let scene = scene_for(OMARCHY, &devs);
        let mut a = Palette::default_manual();
        a.slots.insert("primary".into(), [10, 20, 30]);
        let fans_a = render_device(&resolve_scene(&scene, &a), &devs[1], None, None, &Tick::plain(0.0, false)).unwrap();
        let mut b = a.clone();
        b.slots.insert("primary".into(), [200, 100, 50]);
        b.generation += 1;
        let fans_b = render_device(&resolve_scene(&scene, &b), &devs[1], None, None, &Tick::plain(0.0, false)).unwrap();
        assert_ne!(fans_a, fans_b);
    }

    #[test]
    fn materialize_makes_an_editable_copy() {
        let p = materialize(CYBERPUNK, &devices(), "My neon").unwrap();
        assert!(!p.builtin && p.enabled && p.name == "My neon" && p.requires.is_none());
        assert_eq!(p.scene.devices.len(), 5);
        let d = crate::fsutil::test_dir("bp-mat");
        crate::profiles::save(&d, &p).unwrap();
        assert_eq!(crate::profiles::load(&d, "My neon").unwrap().theme, p.theme);
        assert!(materialize("nope", &devices(), "x").is_none());
    }

    /// The mock backend shows the built-in profiles from this file; refresh it with
    /// `UPDATE_FIXTURES=1 cargo test builtin_profiles_fixture`.
    #[test]
    fn builtin_profiles_fixture_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/mock/builtin_profiles.json");
        let json = serde_json::to_string_pretty(&all()).unwrap() + "\n";
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(path, &json).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap_or_default(), json, "run `UPDATE_FIXTURES=1 cargo test builtin_profiles_fixture` to refresh {path}");
    }
}
