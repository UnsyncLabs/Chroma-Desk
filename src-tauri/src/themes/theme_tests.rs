//! Tests of the theme sources and the live palette, with fixture files only (no network, no
//! real desktop): Omarchy colors.toml copies, a sample kdeglobals and a sample pywal colors.json.

use super::ctl::ThemeCtl;
use super::sources::{Env, Source};
use super::*;
use crate::fsutil::test_dir;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const TOKYO: &str = include_str!("fixtures/omarchy-tokyo-night/colors.toml");
const CATPPUCCIN: &str = include_str!("fixtures/omarchy-catppuccin/colors.toml");
const KDEGLOBALS: &str = include_str!("fixtures/kdeglobals");
const PYWAL: &str = include_str!("fixtures/pywal-colors.json");

fn put(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn omarchy_env(name: &str, colors: &str) -> (Env, std::path::PathBuf) {
    let root = test_dir("omarchy");
    let cur = ".local/state/omarchy/current";
    put(&root, &format!("{cur}/theme/colors.toml"), colors);
    put(&root, &format!("{cur}/theme/keyboard.rgb"), "ff00ff\n");
    put(&root, &format!("{cur}/theme.name"), &format!("{name}\n"));
    put(&root, &format!("{cur}/theme.generation"), "1-1\n");
    (Env::under(&root), root)
}

fn hex(p: &Palette, slot: &str) -> String {
    to_hex(p.get(slot))
}

#[test]
fn omarchy_tokyo_night_maps_slots() {
    let (env, _) = omarchy_env("tokyo-night", TOKYO);
    assert!(omarchy::Omarchy.detect(&env));
    let p = omarchy::Omarchy.read(&env).unwrap();
    assert_eq!(p.name, "tokyo-night");
    assert_eq!(p.source, "omarchy");
    let want = [
        ("accent", "#7aa2f7"),
        ("background", "#1a1b26"),
        ("foreground", "#a9b1d6"),
        ("alert", "#f7768e"),
        ("warn", "#e0af68"),
        ("ok", "#9ece6a"),
        ("info", "#449dab"),
        ("primary", "#7aa2f7"),
        ("secondary", "#ad8ee6"),
        ("tertiary", "#449dab"),
        ("c1", "#f7768e"),
        ("c2", "#eb927b"),
        ("c3", "#e0af68"),
        ("c4", "#9ece6a"),
        ("c5", "#449dab"),
        ("c6", "#7aa2f7"),
        ("c7", "#ad8ee6"),
        ("c8", "#7aa2f7"),
    ];
    for (slot, h) in want {
        assert_eq!(hex(&p, slot), h, "{slot}");
    }
    assert_eq!(omarchy::theme_name(&env).as_deref(), Some("tokyo-night"));
}

#[test]
fn omarchy_keyboard_rgb_is_the_accent_fallback() {
    let without_accent: String = TOKYO.lines().filter(|l| !l.starts_with("accent")).collect::<Vec<_>>().join("\n");
    let (env, _) = omarchy_env("tokyo-night", &without_accent);
    assert_eq!(hex(&omarchy::Omarchy.read(&env).unwrap(), "accent"), "#ff00ff");
}

#[test]
fn omarchy_errors_are_explained() {
    let root = test_dir("omarchy-missing");
    let env = Env::under(&root);
    assert!(omarchy::Omarchy.read(&env).is_err());
    let (env, _) = omarchy_env("x", "mode = \"dark\"\naccent = \"#fff\"\n");
    assert!(omarchy::Omarchy.read(&env).unwrap_err().contains("background"));
}

#[test]
fn kde_reads_kdeglobals() {
    let root = test_dir("kde");
    let env = Env::under(&root);
    assert!(!kde::Kde.detect(&env), "no kdeglobals: not available, no error");
    put(&root, ".config/kdeglobals", KDEGLOBALS);
    assert!(kde::Kde.detect(&env));
    let p = kde::Kde.read(&env).unwrap();
    assert_eq!(p.name, "BreezeLight");
    assert_eq!(hex(&p, "background"), "#fcfcfc");
    assert_eq!(hex(&p, "foreground"), "#232629");
    assert_eq!(hex(&p, "accent"), "#926ee9", "General.AccentColor wins");
    assert_eq!(hex(&p, "alert"), "#da4453");
    assert_eq!(hex(&p, "ok"), "#27ae60");
    assert_eq!(hex(&p, "warn"), "#f67400");
    assert_eq!(hex(&p, "primary"), "#2980b9");
    // Without AccentColor the view's focus decoration is the accent.
    put(&root, ".config/kdeglobals", &KDEGLOBALS.replace("AccentColor=146,110,233", ""));
    assert_eq!(hex(&kde::Kde.read(&env).unwrap(), "accent"), "#3daee9");
    put(&root, ".config/kdeglobals", "[General]\nColorScheme=Broken\n");
    assert!(kde::Kde.read(&env).is_err());
}

#[test]
fn pywal_reads_colors_json() {
    let root = test_dir("wal");
    let env = Env::under(&root);
    assert!(!pywal::Pywal.detect(&env));
    put(&root, ".cache/wal/colors.json", PYWAL);
    assert!(pywal::Pywal.detect(&env));
    let p = pywal::Pywal.read(&env).unwrap();
    assert_eq!(hex(&p, "background"), "#0f1419");
    assert_eq!(hex(&p, "foreground"), "#e6e1cf");
    assert_eq!(hex(&p, "accent"), "#f07178");
    assert_eq!(hex(&p, "primary"), "#36a3d9");
    assert_eq!(hex(&p, "ok"), "#b8cc52");
    assert_eq!(hex(&p, "c3"), "#e7c547");
    put(&root, ".cache/wal/colors.json", "{ not json");
    assert!(pywal::Pywal.read(&env).is_err());
    put(&root, ".cache/wal/colors.json", "{}");
    assert!(pywal::Pywal.read(&env).is_err());
}

#[test]
fn gtk_derives_from_the_accent_and_scheme() {
    let dark = gtk::palette_from("'purple'", "'prefer-dark'").unwrap();
    assert_eq!(hex(&dark, "accent"), "#9141ac");
    assert_eq!(hex(&dark, "background"), "#242424");
    let light = gtk::palette_from("'blue'", "'default'").unwrap();
    assert_eq!(hex(&light, "background"), "#fafafa");
    assert!(gtk::palette_from("'chartreuse'", "").is_err());
    for s in SLOTS {
        assert!(dark.slots.contains_key(s));
    }
}

#[test]
fn live_palette_layers_source_overrides_and_game_colours() {
    let (env, _) = omarchy_env("tokyo-night", TOKYO);
    let ctl = ThemeCtl::new(env);
    // Manual by default.
    assert_eq!(ctl.palette().source, "manual");
    let mut theme = ProfileTheme { source: ThemeSource::Omarchy, ..Default::default() };
    ctl.set_theme(theme.clone());
    assert_eq!(hex(&ctl.palette(), "alert"), "#f7768e");
    // An override pins one slot over the source.
    theme.overrides.insert("alert".into(), [255, 0, 0]);
    ctl.set_theme(theme);
    assert_eq!(hex(&ctl.palette(), "alert"), "#ff0000");
    assert_eq!(hex(&ctl.palette(), "accent"), "#7aa2f7");
    // A game's colours sit on top, and clearing them restores the theme.
    let before = ctl.epoch();
    assert!(ctl.set_external([("accent".to_string(), [1, 2, 3])].into()).unwrap());
    assert_eq!(ctl.palette().get("accent"), [1, 2, 3]);
    assert!(ctl.epoch() > before);
    assert!(ctl.event().external);
    assert!(ctl.set_external([("nope".to_string(), [1, 2, 3])].into()).is_err());
    assert!(ctl.clear_external());
    assert_eq!(hex(&ctl.palette(), "accent"), "#7aa2f7");
    // Setting the same thing again is not a change.
    assert!(!ctl.reload());
}

#[test]
fn a_broken_file_keeps_the_last_good_palette_and_reports_it() {
    let (env, root) = omarchy_env("tokyo-night", TOKYO);
    let ctl = ThemeCtl::new(env);
    ctl.set_theme(ProfileTheme { source: ThemeSource::Omarchy, ..Default::default() });
    let good = hex(&ctl.palette(), "background");
    put(&root, ".local/state/omarchy/current/theme/colors.toml", "garbage = {");
    ctl.reload();
    assert_eq!(hex(&ctl.palette(), "background"), good, "last good palette kept");
    assert!(ctl.event().error.is_some());
    put(&root, ".local/state/omarchy/current/theme/colors.toml", CATPPUCCIN);
    assert!(ctl.reload());
    assert_eq!(hex(&ctl.palette(), "background"), "#1e1e2e");
    assert!(ctl.event().error.is_none());
}

#[test]
fn runtime_source_override_is_dropped_when_a_profile_is_applied() {
    let root = test_dir("rt");
    put(&root, ".cache/wal/colors.json", PYWAL);
    let ctl = ThemeCtl::new(Env::under(&root));
    assert!(ctl.set_runtime_source(ThemeSource::Pywal));
    assert_eq!(hex(&ctl.palette(), "accent"), "#f07178");
    assert!(ctl.status().runtime_override);
    ctl.set_theme(ProfileTheme::default());
    assert_eq!(ctl.palette().source, "manual");
    assert!(!ctl.status().runtime_override);
    // Status lists every source; the ones this machine lacks are "not detected", not errors.
    let st = ctl.status();
    assert_eq!(st.sources.len(), 6);
    assert!(st.sources.iter().find(|s| s.id == "kde").is_some_and(|s| !s.detected && s.error.is_none()));
    assert!(st.sources.iter().find(|s| s.id == "pywal").is_some_and(|s| s.detected && s.name.as_deref() == Some("pywal")));
}

#[test]
fn watcher_picks_up_a_theme_switch_quickly() {
    let (env, root) = omarchy_env("tokyo-night", TOKYO);
    let ctl = Arc::new(ThemeCtl::new(env));
    ctl.set_theme(ProfileTheme { source: ThemeSource::Omarchy, ..Default::default() });
    let stop = Arc::new(AtomicBool::new(false));
    let handle = watch::spawn(ctl.clone(), stop.clone());
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(hex(&ctl.palette(), "background"), "#1a1b26");
    // Omarchy replaces the whole theme directory on every switch.
    let cur = root.join(".local/state/omarchy/current");
    std::fs::remove_dir_all(cur.join("theme")).unwrap();
    put(&root, ".local/state/omarchy/current/theme/colors.toml", CATPPUCCIN);
    put(&root, ".local/state/omarchy/current/theme.name", "catppuccin\n");
    put(&root, ".local/state/omarchy/current/theme.generation", "2-2\n");
    let t0 = Instant::now();
    while hex(&ctl.palette(), "background") != "#1e1e2e" && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let took = t0.elapsed();
    assert_eq!(hex(&ctl.palette(), "background"), "#1e1e2e", "palette did not follow the theme switch");
    assert_eq!(ctl.palette().name, "catppuccin");
    assert!(took < Duration::from_millis(1800), "took {took:?}");
    // And back again.
    std::fs::remove_dir_all(cur.join("theme")).unwrap();
    put(&root, ".local/state/omarchy/current/theme/colors.toml", TOKYO);
    let t0 = Instant::now();
    while hex(&ctl.palette(), "background") != "#1a1b26" && t0.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(hex(&ctl.palette(), "background"), "#1a1b26");
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    drop(handle); // detached: it exits on its next wake-up
}

#[test]
fn socket_commands_change_the_palette() {
    use crate::engine::Engine;
    use serde_json::json;
    let e = Engine::default();
    let d = test_dir("sockpal");
    let ask = |v: serde_json::Value| crate::triggers::handle_line(&e, &d, &v.to_string(), Instant::now());
    let r = ask(json!({"cmd": "set_palette", "slots": {"accent": "#ff0000", "alert": [0, 255, 0]}}));
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(e.themes.palette().get("accent"), [255, 0, 0]);
    assert_eq!(e.themes.palette().get("alert"), [0, 255, 0]);
    let r = ask(json!({"cmd": "palette"}));
    assert_eq!(r["palette"]["palette"]["slots"]["accent"], json!([255, 0, 0]));
    assert_eq!(r["palette"]["external"], true);
    for bad in [json!({"cmd": "set_palette"}), json!({"cmd": "set_palette", "slots": {}}), json!({"cmd": "set_palette", "slots": {"accent": "red"}}), json!({"cmd": "set_palette", "slots": {"nope": "#fff"}}), json!({"cmd": "set_palette", "slots": {"accent": [1, 2, 300]}}), json!({"cmd": "palette_source", "source": "x"})] {
        assert_eq!(ask(bad.clone())["ok"], false, "{bad}");
    }
    assert_eq!(ask(json!({"cmd": "clear_palette"}))["ok"], true);
    assert_ne!(e.themes.palette().get("accent"), [255, 0, 0]);
    assert_eq!(ask(json!({"cmd": "palette_source", "source": "external"}))["ok"], true);
}

#[test]
fn theme_name_changes_are_reported_once_for_rules() {
    let (env, root) = omarchy_env("tokyo-night", TOKYO);
    let ctl = ThemeCtl::new(env);
    // Not watched: nothing to report.
    ctl.reload();
    assert_eq!(ctl.take_theme_change(), None);
    ctl.set_name_watch(true);
    assert_eq!(ctl.take_theme_change(), None, "the first reading is a baseline");
    put(&root, ".local/state/omarchy/current/theme.name", "catppuccin\n");
    ctl.reload();
    assert_eq!(ctl.take_theme_change().as_deref(), Some("catppuccin"));
    assert_eq!(ctl.take_theme_change(), None, "reported once");
    ctl.reload();
    assert_eq!(ctl.take_theme_change(), None);
    // The Omarchy directories are watched for rules even if the active source is Manual.
    assert!(ctl.watch_paths().iter().any(|p| p.ends_with("omarchy/current")));
}

#[test]
fn builtin_cards_respect_availability_and_the_enabled_switch() {
    use crate::config::Settings;
    let (env, _) = omarchy_env("tokyo-night", TOKYO);
    let ctl = ThemeCtl::new(env);
    let mut st = Settings::default();
    let cards = crate::builtin::infos(&st, &ctl);
    let cyber = cards.iter().find(|c| c.name == "Cyberpunk").unwrap();
    assert!(cyber.builtin && cyber.enabled && cyber.strip.len() == 7);
    let omarchy = cards.iter().find(|c| c.name == crate::builtin::OMARCHY).expect("Omarchy is detected here");
    assert!(omarchy.builtin && !omarchy.enabled, "ships disabled");
    st.builtin_enabled.insert(crate::builtin::OMARCHY.into(), true);
    assert!(crate::builtin::infos(&st, &ctl).iter().any(|c| c.name == crate::builtin::OMARCHY && c.enabled));
}
