//! Ready-made automations shipped in the binary: the system monitors on the F-row and the number
//! row, the Discord and Telegram message notifications and the "AI needs you" alert. They are
//! read-only; the UI creates an editable copy of one when the user adds it.
//!
//! LED references use the key names OpenRGB reports for keyboards (`Key: Escape`, `Key: F1`...),
//! so they work on any keyboard with those keys and are ignored elsewhere.

use crate::animations::{AnimKind, Animation, Step, Target};
use crate::engine::effects::Params;
use crate::profiles::{AiMatch, DesktopMatch, NamedTrigger};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Template {
    /// Stable id; also the suggested name of the copy (a valid trigger name).
    pub id: &'static str,
    /// `monitor` or `notification`.
    pub category: &'static str,
    pub notification: NamedTrigger,
}

fn key(k: &str) -> String {
    format!("name:Key: {k}")
}

fn keys(list: &[&str]) -> Vec<String> {
    list.iter().map(|k| key(k)).collect()
}

fn params(v: Value) -> Params {
    Params(v.as_object().cloned().unwrap_or_default())
}

fn step(effect: &str, p: Value, ks: Vec<String>, ms: u32) -> Step {
    Step { effect: effect.into(), params: params(p), keys: ks, duration_ms: ms, anim: None }
}

fn sequence(name: &str, steps: Vec<Step>, looped: bool) -> Animation {
    Animation {
        schema_version: crate::animations::SCHEMA_VERSION,
        id: String::new(),
        name: name.into(),
        description: None,
        tags: vec![],
        created: 0,
        updated: 1,
        target: Target { kind: crate::animations::TargetKind::Keyboard, hint: None },
        builtin: false,
        kind: AnimKind::Sequence { steps, looped },
    }
}

/// The notification keys: every key any step uses, so devices without them are skipped.
fn union_keys(a: &Animation) -> Vec<String> {
    let mut v: Vec<String> = crate::animations::refs_of(a).into_iter().map(str::to_string).collect();
    v.sort();
    v
}

fn with_sequence(a: Animation) -> NamedTrigger {
    NamedTrigger { keys: union_keys(&a), sequence: Some(Box::new(a)), ..NamedTrigger::new("") }
}

pub const F_ROW: [&str; 13] = ["Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"];
pub const NUMBER_ROW: [&str; 14] = ["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Backspace"];
const WASD_RING: [&str; 4] = ["W", "D", "S", "A"];
const ARROW_RING: [&str; 4] = ["Up Arrow", "Right Arrow", "Down Arrow", "Left Arrow"];

fn meter(metric: &str, color: Value, min: f64, max: f64, ks: &[&str]) -> Step {
    step("meter", json!({"metric": metric, "color": color, "min": min, "max": max, "style": "sweep", "sweep_ms": 1200, "background": "none"}), keys(ks), 3200)
}

fn chat(id: &'static str, color: [u8; 3], app: &str) -> Template {
    let all: Vec<&str> = WASD_RING.iter().chain(ARROW_RING.iter()).copied().collect();
    let seq = sequence(
        id,
        vec![
            step("blink", json!({"color": color, "count": 5, "on_ms": 150, "off_ms": 150}), keys(&all), 1500),
            step("orbit", json!({"color": color, "laps": 1, "lap_ms": 800, "trail": 0.5}), keys(&WASD_RING), 800),
            step("orbit", json!({"color": color, "laps": 1, "lap_ms": 800, "trail": 0.5}), keys(&ARROW_RING), 800),
            step("hold", json!({}), vec![], 200),
        ],
        false,
    );
    let mut n = with_sequence(seq);
    n.sources.desktop = Some(DesktopMatch { app: app.into(), urgency: None, contains: String::new() });
    Template { id, category: "notification", notification: n }
}

/// Every built-in automation template.
pub fn templates() -> Vec<Template> {
    let gpu = sequence(
        "gpu_monitor",
        vec![meter("gpu.vram", json!({"slot": "ok"}), 0.0, 100.0, &F_ROW), meter("gpu.usage", json!([255, 255, 255]), 0.0, 100.0, &F_ROW)],
        true,
    );
    let cpu = sequence(
        "cpu_monitor",
        vec![meter("cpu.usage", json!({"slot": "warn"}), 0.0, 100.0, &NUMBER_ROW), meter("cpu.temp", json!({"slot": "alert"}), 30.0, 95.0, &NUMBER_ROW)],
        true,
    );
    let mut gpu_n = with_sequence(gpu);
    gpu_n.sources.always = true;
    let mut cpu_n = with_sequence(cpu);
    cpu_n.sources.always = true;
    let mut ai = NamedTrigger::new("builtin:alert");
    ai.color = Some("@alert".into());
    ai.duration = Some(10.0);
    ai.priority = 100;
    ai.replace = true;
    ai.sources.ai = Some(AiMatch::default());
    vec![
        Template { id: "gpu_monitor", category: "monitor", notification: gpu_n },
        Template { id: "cpu_monitor", category: "monitor", notification: cpu_n },
        chat("discord", [0x7c, 0x5c, 0xff], "discord"),
        chat("telegram", [0x2a, 0xab, 0xee], "telegram"),
        chat("slack", [0xe0, 0x1e, 0x5a], "slack"),
        chat("matrix", [0x0d, 0xbd, 0x8b], "element,fractal,nheko,cinny"),
        Template { id: "ai_needs_you", category: "notification", notification: ai },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::render_animation;
    use crate::engine::compositor::Geometry;
    use crate::engine::effects::Px;

    /// The LED names of a Razer Huntsman V3 Pro TKL / Ornata (the subset the templates use) plus noise.
    fn keyboard() -> Vec<String> {
        let mut n: Vec<String> = ["Escape", "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"].iter().map(|k| format!("Key: {k}")).collect();
        n.extend(["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "=", "Backspace", "W", "A", "S", "D", "Up Arrow", "Left Arrow", "Down Arrow", "Right Arrow", "Q"].iter().map(|k| format!("Key: {k}")));
        n
    }

    fn named(n: &[String], k: &str) -> usize {
        n.iter().position(|x| x == &format!("Key: {k}")).unwrap()
    }

    fn get(id: &str) -> NamedTrigger {
        templates().into_iter().find(|t| t.id == id).unwrap().notification
    }

    /// The template's sequence with every meter pinned to `value` (so no sensor is involved).
    fn pinned(id: &str, values: &[f64]) -> Animation {
        let mut a = *get(id).sequence.unwrap();
        // The resolution cache is keyed by id: every pinned copy needs its own.
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        a.id = format!("tpl-{id}-{}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        let AnimKind::Sequence { steps, .. } = &mut a.kind else { panic!() };
        for (s, v) in steps.iter_mut().zip(values) {
            s.params.0.insert("value".into(), json!(v));
        }
        a
    }

    fn at(a: &Animation, n: &[String], t: f32) -> Vec<Px> {
        let pos = vec![[0.5, 0.5]; n.len()];
        let mut out = vec![Px::CLEAR; n.len()];
        let geo = Geometry { led_count: n.len(), device_key: "kb", led_names: n, zones: &[], positions: &pos, palette: crate::themes::default_palette() };
        render_animation(a, t, &geo, 1, &mut out);
        out
    }

    #[test]
    fn all_templates_are_valid_automations_with_valid_names() {
        let list = templates();
        assert_eq!(list.iter().map(|t| t.id).collect::<Vec<_>>(), ["gpu_monitor", "cpu_monitor", "discord", "telegram", "slack", "matrix", "ai_needs_you"]);
        let map = list.iter().map(|t| (t.id.to_string(), t.notification.clone())).collect();
        crate::profiles::validate_triggers(&map).unwrap();
        for t in &list {
            assert!(crate::profiles::valid_trigger_name(t.id));
        }
    }

    #[test]
    fn gpu_monitor_fills_the_f_row_green_then_white() {
        let n = keyboard();
        let a = pinned("gpu_monitor", &[50.0, 100.0]);
        // End of the first step's sweep: VRAM 50 % of 13 keys = 6.5 keys, in the @ok colour.
        let ok = crate::themes::default_palette().get("ok");
        let out = at(&a, &n, 2.0);
        let esc = named(&n, "Escape");
        assert_eq!([out[esc].r as u8, out[esc].g as u8, out[esc].b as u8], ok);
        assert_eq!(out[named(&n, "F5")].g, ok[1] as f32, "key 6 is full");
        let half = &out[named(&n, "F6")];
        assert!((half.a - 0.5).abs() < 0.01 && half.g == ok[1] as f32, "key 7 blends with the scene by half: {half:?}");
        assert_eq!(out[named(&n, "F7")].a, 0.0, "keys past the value are transparent");
        // The sweep grows: early on only a part of the bar is lit.
        let early = at(&a, &n, 0.3);
        assert_eq!(early[named(&n, "F4")].a, 0.0);
        // Second step: white, full bar.
        let white = at(&a, &n, 3.2 + 2.0);
        assert!(F_ROW.iter().all(|k| (white[named(&n, k)].r - 255.0).abs() < 1.0));
        // Nothing outside the F-row is touched; the sequence loops.
        assert_eq!(out[named(&n, "W")].a, 0.0);
        assert_eq!(at(&a, &n, 6.4 + 2.0)[esc], out[esc]);
    }

    #[test]
    fn cpu_monitor_uses_the_number_row_orange_then_red_with_temperature_scale() {
        let n = keyboard();
        // 62.5 °C on 30..95 = 50 % of 14 keys = 7 keys.
        let a = pinned("cpu_monitor", &[100.0, 62.5]);
        let warn = crate::themes::default_palette().get("warn");
        let alert = crate::themes::default_palette().get("alert");
        let first = at(&a, &n, 2.0);
        assert_eq!(first[named(&n, "Backspace")].r, warn[0] as f32, "100 % fills all 14 keys");
        assert_eq!(first[named(&n, "`")].g, warn[1] as f32);
        let second = at(&a, &n, 3.2 + 2.0);
        assert_eq!(second[named(&n, "6")].r, alert[0] as f32);
        assert_eq!(second[named(&n, "-")].a, 0.0, "past 7 keys: transparent");
        assert_eq!(first[named(&n, "F1")].a, 0.0, "F-row untouched");
        let t = get("cpu_monitor");
        assert!(t.sources.always && t.keys.len() == 14);
    }

    /// A monitor sits ON TOP of the running scene: the filled part shows the bar, the rest keeps the scene.
    #[test]
    fn cpu_monitor_at_40_percent_leaves_the_scene_visible_on_the_unfilled_keys() {
        use crate::engine::compositor::{Blend, blend_onto};
        let n = keyboard();
        let a = pinned("cpu_monitor", &[40.0, 0.0]);
        let bar = at(&a, &n, 2.0);
        let scene = [10.0_f32, 20.0, 30.0];
        let mut buf: Vec<Px> = vec![Px { r: scene[0], g: scene[1], b: scene[2], a: 1.0 }; n.len()];
        blend_onto(&mut buf, &bar, None, 1.0, Blend::Normal);
        let warn = crate::themes::default_palette().get("warn");
        // 40 % of 14 keys = 5.6: keys 1-5 full orange, key 6 is 60 % orange over the scene, the rest is the scene.
        for k in NUMBER_ROW.iter().take(5) {
            let p = buf[named(&n, k)];
            assert_eq!([p.r as u8, p.g as u8, p.b as u8], warn, "{k}");
        }
        let part = buf[named(&n, NUMBER_ROW[5])];
        assert!((part.r - (scene[0] + (warn[0] as f32 - scene[0]) * 0.6)).abs() < 0.5, "{part:?}");
        for k in &NUMBER_ROW[6..] {
            let p = buf[named(&n, k)];
            assert_eq!((p.r, p.g, p.b), (scene[0], scene[1], scene[2]), "{k} shows the scene, not black");
        }
        assert_eq!((buf[named(&n, "F1")].r, buf[named(&n, "W")].g), (scene[0], scene[1]), "other keys untouched");
    }

    #[test]
    fn chat_templates_blink_five_times_orbit_both_clusters_and_switch_off() {
        let n = keyboard();
        for (id, color, app) in [("discord", [0x7c, 0x5c, 0xff], "discord"), ("telegram", [0x2a, 0xab, 0xee], "telegram"), ("slack", [0xe0, 0x1e, 0x5a], "slack"), ("matrix", [0x0d, 0xbd, 0x8b], "element,fractal,nheko,cinny")] {
            let t = get(id);
            assert_eq!(t.sources.desktop.as_ref().unwrap().app, app);
            assert_eq!(t.keys.len(), 8);
            let mut a = *t.sequence.clone().unwrap();
            a.id = format!("chat-{id}");
            let lit = |t: f32| at(&a, &n, t).iter().enumerate().filter(|(_, p)| p.a > 0.0 && p.r + p.g + p.b > 0.0).map(|(i, _)| i).collect::<Vec<_>>();
            let wasd = [named(&n, "W"), named(&n, "A"), named(&n, "S"), named(&n, "D")];
            let arrows = [named(&n, "Up Arrow"), named(&n, "Left Arrow"), named(&n, "Down Arrow"), named(&n, "Right Arrow")];
            let mut all: Vec<usize> = wasd.iter().chain(arrows.iter()).copied().collect();
            all.sort();
            // Five blinks: on at 0, 300, 600, 900, 1200 ms, off at 150, 450, ... and quiet after 1500 ms.
            let on: Vec<bool> = (0..5).map(|k| lit(k as f32 * 0.3 + 0.05) == all).collect();
            assert_eq!(on, vec![true; 5], "{id}");
            assert!((0..5).all(|k| lit(k as f32 * 0.3 + 0.2).is_empty()), "{id}: dark between blinks");
            let px = at(&a, &n, 0.05)[wasd[0]];
            assert_eq!([px.r as u8, px.g as u8, px.b as u8], color);
            // WASD orbit: W, D, S, A (800 ms per lap starting at 1500 ms).
            // The head is the brightest LED (a fading tail follows it).
            let brightest = |t: f32| {
                let out = at(&a, &n, t);
                let (i, _) = out.iter().enumerate().max_by(|x, y| x.1.a.total_cmp(&y.1.a)).unwrap();
                vec![i]
            };
            let head = |ms: f32| brightest((1500.0 + ms) / 1000.0);
            assert_eq!(head(50.0), vec![named(&n, "W")]);
            assert_eq!(head(250.0), vec![named(&n, "D")]);
            assert_eq!(head(450.0), vec![named(&n, "S")]);
            assert_eq!(head(650.0), vec![named(&n, "A")]);
            // Arrows: Up, Right, Down, Left.
            let arrow = |ms: f32| brightest((2300.0 + ms) / 1000.0);
            assert_eq!(arrow(50.0), vec![named(&n, "Up Arrow")]);
            assert_eq!(arrow(250.0), vec![named(&n, "Right Arrow")]);
            assert_eq!(arrow(450.0), vec![named(&n, "Down Arrow")]);
            assert_eq!(arrow(650.0), vec![named(&n, "Left Arrow")]);
            assert!(lit(3.2).is_empty() && lit(2.0 + 1.5).is_empty(), "off at the end");
            assert_eq!(crate::animations::cycle_secs(&a), 3.3);
        }
    }

    #[test]
    fn ai_template_is_the_themed_alert_on_the_whole_keyboard() {
        let t = get("ai_needs_you");
        assert_eq!((t.anim.as_str(), t.color.as_deref(), t.duration, t.priority), ("builtin:alert", Some("@alert"), Some(10.0), 100));
        assert!(t.keys.is_empty() && t.sources.ai.is_some() && t.replace);
    }

    /// The mock backend reads the templates from this file; regenerate it with
    /// `UPDATE_FIXTURES=1 cargo test templates_fixture`.
    #[test]
    fn templates_fixture_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/mock/templates.json");
        let json = serde_json::to_string_pretty(&templates()).unwrap() + "\n";
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(path, &json).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap_or_default(), json, "run `UPDATE_FIXTURES=1 cargo test templates_fixture` to refresh {path}");
    }
}
