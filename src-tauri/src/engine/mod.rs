//! Animation engine: scene model, layer compositor, effects and the 60 Hz runner.

pub mod compositor;
pub mod effects;
pub mod overlay;
pub mod runner;

use crate::sdk::{DeviceDto, Rgb};
use crate::themes;
use crate::themes::Palette;
use compositor::{Geometry, Layer};
use effects::Pos;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneMode {
    /// chroma-desk draws frames with its layers.
    Effects,
    /// The device runs its own native mode; the engine does not touch it.
    #[default]
    Hardware,
}

/// A native mode to re-apply (mode by name: indices are not stable across servers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareMode {
    pub mode: String,
    pub speed: Option<u32>,
    pub brightness: Option<u32>,
    pub direction: Option<u32>,
    pub colors: Option<Vec<Rgb>>,
}

impl HardwareMode {
    /// Request to re-apply this mode once its index on the device is known.
    pub fn to_request(&self, index: usize) -> crate::sdk::ModeRequest {
        crate::sdk::ModeRequest {
            mode: index,
            speed: self.speed,
            brightness: self.brightness,
            direction: self.direction,
            colors: self.colors.clone(),
            save: false,
        }
    }
}

fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceScene {
    #[serde(default)]
    pub mode: SceneMode,
    /// Bottom to top.
    #[serde(default)]
    pub layers: Vec<Layer>,
    #[serde(default = "one")]
    pub brightness: f32,
    /// fps cap override for this device.
    #[serde(default)]
    pub fps: Option<u32>,
    /// Take part in the global (synchronised) scene.
    #[serde(default = "yes")]
    pub follow_global: bool,
    /// LEDs per fan by zone name (drawn as rings in the UI).
    #[serde(default)]
    pub fan_leds: BTreeMap<String, u32>,
    #[serde(default)]
    pub hardware: Option<HardwareMode>,
}

impl Default for DeviceScene {
    fn default() -> Self {
        Self {
            mode: SceneMode::Hardware,
            layers: vec![],
            brightness: 1.0,
            fps: None,
            follow_global: true,
            fan_leds: BTreeMap::new(),
            hardware: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    #[serde(default = "one")]
    pub master_brightness: f32,
    /// Global synchronisation on/off.
    #[serde(default)]
    pub sync: bool,
    /// Device identity keys, left to right on the global canvas.
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub global_layers: Vec<Layer>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceScene>,
}

impl Default for Scene {
    fn default() -> Self {
        Self { master_brightness: 1.0, sync: false, order: vec![], global_layers: vec![], devices: BTreeMap::new() }
    }
}

/// A copy of `scene` with every palette slot replaced by its colour (the engine renders this).
pub fn resolve_scene(scene: &Scene, pal: &Palette) -> Scene {
    let mut s = scene.clone();
    let fix = |layers: &mut Vec<Layer>| {
        for l in layers.iter_mut() {
            if l.params.0.values().any(themes::contains_slot) {
                pal.resolve_map(&mut l.params.0);
            }
        }
    };
    fix(&mut s.global_layers);
    s.devices.values_mut().for_each(|d| fix(&mut d.layers));
    s
}

/// What the engine knows about a connected device.
#[derive(Debug, Clone)]
pub struct DevInfo {
    pub index: usize,
    pub key: String,
    pub name: String,
    pub device_type: String,
    pub led_count: usize,
    /// OpenRGB LED names (empty strings for unnamed LEDs).
    pub led_names: Vec<String>,
    /// `(zone id, first LED, LED count)`.
    pub zones: Vec<(usize, usize, usize)>,
    pub default_positions: Vec<Pos>,
}

impl DevInfo {
    pub fn from_dto(d: &DeviceDto) -> Self {
        Self {
            index: d.id,
            key: d.key.clone(),
            name: d.name.clone(),
            device_type: d.device_type.clone(),
            led_count: d.colors.len(),
            led_names: (0..d.colors.len()).map(|i| d.led_names.get(i).cloned().unwrap_or_default()).collect(),
            zones: d.zones.iter().map(|z| (z.id, z.offset, z.num_leds)).collect(),
            default_positions: default_positions(d),
        }
    }
}

/// Positions when the frontend has not supplied a physical layout: matrix cells for matrix
/// zones, a line along the zone otherwise, zones stacked vertically.
pub fn default_positions(d: &DeviceDto) -> Vec<Pos> {
    let n = d.colors.len();
    let mut out: Vec<Pos> = vec![[0.5, 0.5]; n];
    let zc = d.zones.len().max(1) as f32;
    for (zi, z) in d.zones.iter().enumerate() {
        let y_row = (zi as f32 + 0.5) / zc;
        for i in 0..z.num_leds {
            let Some(slot) = out.get_mut(z.offset + i) else { continue };
            *slot = match &z.matrix {
                Some(m) => match m.map.iter().position(|v| *v == Some(i as u32)) {
                    Some(idx) => [
                        ((idx as u32 % m.width) as f32 + 0.5) / m.width as f32,
                        ((idx as u32 / m.width) as f32 + 0.5) / m.height as f32,
                    ],
                    None => [0.5, 0.5],
                },
                None => [(i as f32 + 0.5) / z.num_leds as f32, y_row],
            };
        }
    }
    out
}

/// Default frame-rate cap by device type (the SDK/bus is the bottleneck, not the CPU).
pub fn default_fps(device_type: &str) -> u32 {
    match device_type {
        "Keyboard" | "Mouse" | "MouseMat" | "Headset" | "HeadsetStand" | "Gamepad" => 60,
        "DRam" => 10,
        "Gpu" | "Motherboard" => 15,
        _ => 30,
    }
}

/// Effective fps: per-device override, then per-type setting, then the default.
pub fn fps_for(device_type: &str, device_override: Option<u32>, by_type: &HashMap<String, u32>) -> u32 {
    device_override.or_else(|| by_type.get(device_type).copied()).unwrap_or_else(|| default_fps(device_type)).clamp(1, 60)
}

/// Whether a frame may be sent now given the last send time and the fps cap.
pub fn is_due(last: Option<Instant>, now: Instant, fps: u32) -> bool {
    let interval = Duration::from_secs_f32(1.0 / fps.max(1) as f32);
    // Half a 60 Hz tick of tolerance so a 30 fps cap really yields 30 fps.
    last.is_none_or(|l| now.saturating_duration_since(l) + Duration::from_micros(8_000) >= interval)
}

/// Slot `(index, count)` of each synced device on the global canvas, in scene order.
pub fn sync_slots(scene: &Scene, devs: &[DevInfo]) -> HashMap<String, (usize, usize)> {
    let follows = |k: &str| {
        scene.devices.get(k).is_none_or(|d| d.follow_global && d.mode == SceneMode::Effects)
            && devs.iter().any(|d| d.key == k)
    };
    let mut keys: Vec<&str> = scene.order.iter().map(String::as_str).filter(|k| follows(k)).collect();
    for d in devs {
        if !keys.contains(&d.key.as_str()) && follows(&d.key) {
            keys.push(&d.key);
        }
    }
    let n = keys.len();
    keys.into_iter().enumerate().map(|(i, k)| (k.to_string(), (i, n))).collect()
}

fn key_seed(key: &str) -> u32 {
    key.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// Time-dependent inputs of one rendered frame.
#[derive(Clone, Copy)]
pub struct Tick<'a> {
    /// Seconds since the engine started.
    pub t: f32,
    pub blackout: bool,
    pub overlays: &'a [std::sync::Arc<overlay::Overlay>],
    pub now: Instant,
    /// Palette for the colours the scene did not resolve itself (overlays).
    pub palette: &'a Palette,
}

impl Tick<'_> {
    /// No overlays, `now` = the real clock (tests and simple callers).
    pub fn plain(t: f32, blackout: bool) -> Self {
        Tick { t, blackout, overlays: &[], now: Instant::now(), palette: crate::themes::default_palette() }
    }
}

/// Computes the colours of one device in Effects mode, or `None` if the engine does not own it.
pub fn render_device(
    scene: &Scene,
    dev: &DevInfo,
    custom: Option<&Vec<Pos>>,
    slot: Option<(usize, usize)>,
    tick: &Tick,
) -> Option<Vec<[u8; 3]>> {
    let Tick { t, blackout, overlays, now, palette } = *tick;
    let ds = scene.devices.get(&dev.key)?;
    if ds.mode != SceneMode::Effects {
        return None;
    }
    if blackout {
        return Some(vec![[0, 0, 0]; dev.led_count]);
    }
    let local: &[Pos] = match custom {
        Some(p) if p.len() == dev.led_count => p,
        _ => &dev.default_positions,
    };
    let mut buf = compositor::black(dev.led_count);
    if scene.sync
        && ds.follow_global
        && !scene.global_layers.is_empty()
        && let Some((i, n)) = slot
    {
        let global: Vec<Pos> = local.iter().map(|p| [(i as f32 + p[0]) / n as f32, p[1]]).collect();
        let geo = Geometry { led_count: dev.led_count, device_key: &dev.key, led_names: &dev.led_names, zones: &dev.zones, positions: &global, palette };
        compositor::composite_onto(&mut buf, &scene.global_layers, &geo, t, 7);
    }
    let geo = Geometry { led_count: dev.led_count, device_key: &dev.key, led_names: &dev.led_names, zones: &dev.zones, positions: local, palette };
    compositor::composite_onto(&mut buf, &ds.layers, &geo, t, key_seed(&dev.key));
    overlay::compose(&mut buf, overlays, dev, local, now, palette);
    Some(compositor::finalize(&buf, scene.master_brightness * ds.brightness))
}

/// State shared between the Tauri commands and the runner task.
pub struct Engine {
    pub scene: Mutex<Scene>,
    pub devs: Mutex<Vec<DevInfo>>,
    /// Physical layouts sent by the frontend (normalised), by device key.
    pub positions: Mutex<HashMap<String, Vec<Pos>>>,
    /// Devices whose live preview the UI is showing.
    pub visible: Mutex<HashSet<String>>,
    /// fps caps by device type (Settings).
    pub fps_by_type: Mutex<HashMap<String, u32>>,
    /// Temporary animations drawn over the scene (triggers).
    pub overlays: Mutex<overlay::Overlays>,
    /// The active palette and where it comes from (slot colours resolve against it).
    pub themes: std::sync::Arc<themes::ctl::ThemeCtl>,
    /// Named triggers of the active profile (`chroma-desk trigger low_health`).
    pub profile_triggers: Mutex<BTreeMap<String, crate::profiles::NamedTrigger>>,
    /// Notifications available in every profile (Settings).
    pub global_triggers: Mutex<BTreeMap<String, crate::profiles::NamedTrigger>>,
    /// The desktop-notification source is switched on (Settings).
    pub desktop_enabled: std::sync::Arc<AtomicBool>,
    /// `off`, `listening` or `error: ...` (desktop notification source).
    pub desktop_status: Mutex<String>,
    /// "Stop all" silences always-on automations until they are edited, disabled or resumed.
    pub paused_always: Mutex<BTreeMap<String, crate::profiles::NamedTrigger>>,
    /// Everything off until cleared (tray "Lights off").
    pub blackout: AtomicBool,
    pub start: Instant,
}

impl Engine {
    /// The notification called `name`: the active profile's own, else a global one.
    pub fn notification(&self, name: &str) -> Option<crate::profiles::NamedTrigger> {
        if let Some(n) = self.profile_triggers.lock().unwrap().get(name) {
            return Some(n.clone());
        }
        self.global_triggers.lock().unwrap().get(name).cloned()
    }

    /// Every notification in effect (global ones, overridden by the profile's).
    pub fn notifications(&self) -> BTreeMap<String, crate::profiles::NamedTrigger> {
        let mut all = self.global_triggers.lock().unwrap().clone();
        all.extend(self.profile_triggers.lock().unwrap().clone());
        all
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            scene: Mutex::default(),
            devs: Mutex::default(),
            positions: Mutex::default(),
            visible: Mutex::default(),
            fps_by_type: Mutex::default(),
            overlays: Mutex::default(),
            themes: std::sync::Arc::default(),
            profile_triggers: Mutex::default(),
            global_triggers: Mutex::default(),
            desktop_status: Mutex::new("off".into()),
            desktop_enabled: std::sync::Arc::default(),
            paused_always: Mutex::default(),
            blackout: AtomicBool::new(false),
            start: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compositor::{Blend, Mask};
    use serde_json::json;

    fn dev(key: &str, n: usize) -> DevInfo {
        DevInfo {
            index: 0,
            key: key.into(),
            name: key.into(),
            device_type: "Keyboard".into(),
            led_count: n,
            led_names: vec![String::new(); n],
            zones: vec![(0, 0, n)],
            default_positions: (0..n).map(|i| [i as f32 / (n as f32 - 1.0).max(1.0), 0.5]).collect(),
        }
    }

    fn layer(effect: &str, params: serde_json::Value) -> Layer {
        Layer {
            id: effect.into(),
            name: effect.into(),
            enabled: true,
            opacity: 1.0,
            blend: Blend::Normal,
            mask: Mask::All,
            effect: effect.into(),
            params: effects::params(params),
        }
    }

    fn scene_with(key: &str, layers: Vec<Layer>) -> Scene {
        let mut s = Scene::default();
        s.devices.insert(key.into(), DeviceScene { mode: SceneMode::Effects, layers, ..Default::default() });
        s
    }

    #[test]
    fn fps_defaults_and_overrides() {
        assert_eq!(default_fps("Keyboard"), 60);
        assert_eq!(default_fps("DRam"), 10);
        assert_eq!(default_fps("Gpu"), 15);
        assert_eq!(default_fps("LEDStrip"), 30);
        let mut by_type = HashMap::new();
        by_type.insert("Keyboard".to_string(), 24);
        assert_eq!(fps_for("Keyboard", None, &by_type), 24);
        assert_eq!(fps_for("Keyboard", Some(90), &by_type), 60); // clamped
        assert_eq!(fps_for("Mouse", None, &by_type), 60);
    }

    #[test]
    fn throttling_respects_cap() {
        let t0 = Instant::now();
        assert!(is_due(None, t0, 30));
        // 60 Hz ticks, 10 fps cap: one send per 100 ms.
        let mut last = Some(t0);
        let mut sends = 0;
        for tick in 1..=60 {
            let now = t0 + Duration::from_micros(16_667 * tick);
            if is_due(last, now, 10) {
                sends += 1;
                last = Some(now);
            }
        }
        assert!((9..=10).contains(&sends), "sent {sends} frames in ~1 s at 10 fps");
        // 30 fps cap at 60 Hz ticks: every other tick.
        let mut last = Some(t0);
        let mut sends = 0;
        for tick in 1..=60 {
            let now = t0 + Duration::from_micros(16_667 * tick);
            if is_due(last, now, 30) {
                sends += 1;
                last = Some(now);
            }
        }
        assert!((28..=31).contains(&sends), "sent {sends}");
    }

    #[test]
    fn hardware_devices_are_not_rendered() {
        let mut s = Scene::default();
        s.devices.insert("k".into(), DeviceScene::default());
        assert!(render_device(&s, &dev("k", 3), None, None, &Tick::plain(0.0, false)).is_none());
        assert!(render_device(&s, &dev("unknown", 3), None, None, &Tick::plain(0.0, false)).is_none());
    }

    #[test]
    fn effects_render_with_brightness() {
        let mut s = scene_with("k", vec![layer("solid", json!({"color": [200, 100, 50]}))]);
        s.master_brightness = 0.5;
        s.devices.get_mut("k").unwrap().brightness = 0.5;
        let out = render_device(&s, &dev("k", 2), None, None, &Tick::plain(0.0, false)).unwrap();
        assert_eq!(out, vec![[50, 25, 13]; 2]);
    }

    #[test]
    fn blackout_overrides() {
        let s = scene_with("k", vec![layer("solid", json!({"color": [255, 255, 255]}))]);
        assert_eq!(render_device(&s, &dev("k", 2), None, None, &Tick::plain(0.0, true)).unwrap(), vec![[0, 0, 0]; 2]);
    }

    #[test]
    fn sync_slots_follow_order_and_skip_hardware() {
        let mut s = Scene { sync: true, order: vec!["b".into(), "a".into()], ..Default::default() };
        for k in ["a", "b", "c", "hw"] {
            s.devices.insert(k.into(), DeviceScene { mode: SceneMode::Effects, ..Default::default() });
        }
        s.devices.insert("hw".into(), DeviceScene::default());
        let devs = [dev("a", 1), dev("b", 1), dev("c", 1), dev("hw", 1)];
        let slots = sync_slots(&s, &devs);
        assert_eq!(slots["b"], (0, 3));
        assert_eq!(slots["a"], (1, 3));
        assert_eq!(slots["c"], (2, 3));
        assert!(!slots.contains_key("hw"));
    }

    #[test]
    fn global_wave_runs_across_devices() {
        // Global rainbow: the left device must differ from the right one at the same time.
        let mut s = scene_with("a", vec![]);
        s.devices.insert("b".into(), DeviceScene { mode: SceneMode::Effects, ..Default::default() });
        s.sync = true;
        s.global_layers = vec![layer("gradient", json!({"colors": [[0, 0, 0], [255, 255, 255]], "angle": 0, "speed": 0}))];
        let devs = [dev("a", 4), dev("b", 4)];
        let slots = sync_slots(&s, &devs);
        let a = render_device(&s, &devs[0], None, slots.get("a").copied(), &Tick::plain(0.0, false)).unwrap();
        let b = render_device(&s, &devs[1], None, slots.get("b").copied(), &Tick::plain(0.0, false)).unwrap();
        assert!(a.last().unwrap()[0] < b[0][0] + 80, "a ends near where b starts");
        assert!(b.last().unwrap()[0] > a[0][0]);
        assert!(a[0][0] < 60 && b[3][0] > 200);
    }

    #[test]
    fn custom_positions_are_used_when_length_matches() {
        let s = scene_with("k", vec![layer("gradient", json!({"colors": [[0, 0, 0], [255, 0, 0]], "angle": 0, "speed": 0}))]);
        let d = dev("k", 2);
        let custom = vec![[1.0, 0.5], [0.0, 0.5]];
        let out = render_device(&s, &d, Some(&custom), None, &Tick::plain(0.0, false)).unwrap();
        assert!(out[0][0] > out[1][0]);
        // Wrong length: ignored.
        let bad = vec![[1.0, 0.5]];
        let out2 = render_device(&s, &d, Some(&bad), None, &Tick::plain(0.0, false)).unwrap();
        assert!(out2[0][0] < out2[1][0]);
    }

    #[test]
    fn overlay_covers_the_scene_then_the_scene_returns() {
        use crate::animations::{AnimKind, Animation, Frame, LoopMode, Target};
        let s = scene_with("k", vec![layer("solid", json!({"color": [0, 0, 255]}))]);
        let d = dev("k", 2);
        let t0 = Instant::now();
        let anim = Animation {
            schema_version: 1,
            id: "ov-test".into(),
            name: "ov".into(),
            description: None,
            tags: vec![],
            created: 0,
            updated: 1,
            target: Target::default(),
            builtin: false,
            kind: AnimKind::Frames { frames: vec![Frame { leds: [("idx:0".to_string(), crate::themes::Color::Rgb([255, 0, 0]))].into(), duration_ms: None }], frame_ms: 100, loop_mode: LoopMode::Loop, crossfade: 0.0 },
        };
        let mut ov = overlay::Overlays::default();
        ov.add(t0, |id| overlay::Overlay { id, anim, device: None, keys: vec![], start: t0, until: t0 + Duration::from_secs(1), priority: 0 }).unwrap();
        let during = ov.active(t0 + Duration::from_millis(500));
        fn tick(now: Instant, overlays: &[std::sync::Arc<overlay::Overlay>]) -> Tick<'_> {
            Tick { t: 0.0, blackout: false, overlays, now, palette: crate::themes::default_palette() }
        }
        let a = render_device(&s, &d, None, None, &tick(t0 + Duration::from_millis(500), &during)).unwrap();
        assert_eq!(a, vec![[255, 0, 0], [0, 0, 255]]);
        let after = ov.active(t0 + Duration::from_secs(2));
        let b = render_device(&s, &d, None, None, &tick(t0 + Duration::from_secs(2), &after)).unwrap();
        assert_eq!(b, vec![[0, 0, 255]; 2], "scene restored after expiry");
    }

    #[test]
    fn animation_layer_plays_in_the_scene() {
        use crate::animations;
        let anim = animations::builtins().into_iter().find(|a| a.id == "builtin:rainbow").unwrap();
        let s = scene_with("k", vec![animations::layer_for(&anim)]);
        let out = render_device(&s, &dev("k", 4), None, None, &Tick::plain(0.0, false)).unwrap();
        assert!(out.iter().any(|c| c != &[0, 0, 0]));
        // The profile round-trips with the snapshot inside.
        let j = serde_json::to_string(&s).unwrap();
        let back: Scene = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn scene_serde_roundtrip() {
        let mut s = scene_with("k", vec![layer("rainbow", json!({"speed": 1.0}))]);
        s.sync = true;
        s.order = vec!["k".into()];
        let j = serde_json::to_string(&s).unwrap();
        let back: Scene = serde_json::from_str(&j).unwrap();
        assert_eq!(s, back);
        let minimal: Scene = serde_json::from_str("{}").unwrap();
        assert_eq!(minimal.master_brightness, 1.0);
    }

    #[test]
    fn slot_colours_follow_the_palette_and_fixed_ones_do_not() {
        let s = scene_with("k", vec![layer("solid", json!({"color": {"slot": "accent"}})), {
            let mut l = layer("solid", json!({"color": [1, 2, 3]}));
            l.mask = Mask::Leds { leds: vec![1] };
            l
        }]);
        let d = dev("k", 2);
        let mut pal = Palette::default_manual();
        pal.slots.insert("accent".into(), [200, 10, 20]);
        let r = resolve_scene(&s, &pal);
        let out = render_device(&r, &d, None, None, &Tick::plain(0.0, false)).unwrap();
        assert_eq!(out, vec![[200, 10, 20], [1, 2, 3]]);
        // The source scene is untouched; a new palette gives new colours.
        assert!(themes::contains_slot(&serde_json::to_value(&s).unwrap()));
        pal.slots.insert("accent".into(), [0, 99, 0]);
        let out = render_device(&resolve_scene(&s, &pal), &d, None, None, &Tick::plain(0.0, false)).unwrap();
        assert_eq!(out[0], [0, 99, 0]);
    }

    #[test]
    fn overlay_and_animation_colours_resolve_with_the_palette() {
        use crate::animations::{AnimKind, Animation, Frame, LoopMode, Target};
        use crate::themes::Color;
        let s = scene_with("k", vec![]);
        let d = dev("k", 2);
        let t0 = Instant::now();
        let anim = Animation {
            schema_version: 1,
            id: "ov-slot".into(),
            name: "ov".into(),
            description: None,
            tags: vec![],
            created: 0,
            updated: 1,
            target: Target::default(),
            builtin: false,
            kind: AnimKind::Frames { frames: vec![Frame { leds: [("idx:0".to_string(), Color::slot("alert"))].into(), duration_ms: None }], frame_ms: 100, loop_mode: LoopMode::Loop, crossfade: 0.0 },
        };
        let mut ov = overlay::Overlays::default();
        ov.add(t0, |id| overlay::Overlay { id, anim, device: None, keys: vec![], start: t0, until: t0 + Duration::from_secs(5), priority: 0 }).unwrap();
        let active = ov.active(t0 + Duration::from_millis(10));
        let mut pal = Palette::default_manual();
        pal.slots.insert("alert".into(), [10, 200, 30]);
        pal.generation += 1000;
        let tick = Tick { t: 0.0, blackout: false, overlays: &active, now: t0 + Duration::from_millis(10), palette: &pal };
        assert_eq!(render_device(&s, &d, None, None, &tick).unwrap()[0], [10, 200, 30]);
        // A new palette generation re-resolves the cached animation.
        let mut pal2 = pal.clone();
        pal2.slots.insert("alert".into(), [7, 7, 7]);
        pal2.generation += 1;
        let tick = Tick { palette: &pal2, ..tick };
        assert_eq!(render_device(&s, &d, None, None, &tick).unwrap()[0], [7, 7, 7]);
    }

    #[test]
    fn plain_rgb_scenes_are_byte_identical_after_resolution() {
        let s = scene_with("k", vec![layer("gradient", json!({"colors": [[255, 0, 0], [0, 0, 255]], "angle": 0, "speed": 0}))]);
        assert_eq!(resolve_scene(&s, &Palette::default_manual()), s);
    }
}
