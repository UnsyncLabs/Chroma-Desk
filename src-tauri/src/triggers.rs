//! Triggers: start/stop overlay animations from the UI, the CLI and the local socket.

use crate::animations::{self, Animation};
use crate::engine::overlay::{self, Overlay};
use crate::engine::{Engine, SceneMode};
use crate::error::{ApiError, Res};
use crate::profiles::{AiMatch, DesktopMatch, NamedTrigger};
use crate::themes::{self, Color, ThemeSource};
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

const MAX_KEYS: usize = 64;
const MAX_TEXT: usize = 128;
const MAX_SECS: f64 = 3600.0;
const DEFAULT_SECS: f64 = 3.0;

/// How long: seconds as a number, or text such as `2s`, `500ms`, `1.5`, `2m`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DurationSpec {
    Secs(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerSpec {
    /// Animation name or id.
    pub animation: String,
    /// Device key or name fragment; omitted = every device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Key names; omitted = the whole device.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<DurationSpec>,
    /// `#rrggbb`, `#rgb` or `@slot` (a palette slot, e.g. `@alert`): recolours the animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Number of passes of the animation (limited by `duration` when both are given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    /// Higher is drawn on top of lower.
    #[serde(default)]
    pub priority: i32,
    /// Playback speed multiplier (1 = as designed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    /// Replace the previous overlay of the same notification instead of stacking on it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriggerResult {
    pub overlay: u64,
    /// Devices that will show it.
    pub devices: usize,
    pub duration: f64,
}

/// `3s`, `500ms`, `2m`, `1.5` (seconds). Rejects negatives and non-numbers.
pub fn parse_duration(s: &str) -> Option<f64> {
    let s = s.trim().to_lowercase();
    let (num, mul) = if let Some(n) = s.strip_suffix("ms") {
        (n, 0.001)
    } else if let Some(n) = s.strip_suffix('s') {
        (n, 1.0)
    } else if let Some(n) = s.strip_suffix('m') {
        (n, 60.0)
    } else {
        (s.as_str(), 1.0)
    };
    let v: f64 = num.trim().parse().ok()?;
    (v.is_finite() && v >= 0.0).then_some(v * mul)
}

/// `#ff0000`, `ff0000`, `#f00` or a palette slot (`@alert`).
pub fn parse_color(s: &str) -> Option<Color> {
    Color::parse(s)
}

fn invalid(why: impl ToString) -> ApiError {
    ApiError::new("trigger_invalid", why)
}

impl TriggerSpec {
    /// Field checks shared by every input path.
    pub fn validate(&self) -> Res<()> {
        let short = |s: &str| !s.trim().is_empty() && s.chars().count() <= MAX_TEXT;
        if !short(&self.animation) {
            return Err(invalid("animation"));
        }
        if self.device.as_deref().is_some_and(|d| d.chars().count() > MAX_TEXT) {
            return Err(invalid("device"));
        }
        if self.keys.len() > MAX_KEYS || self.keys.iter().any(|k| k.trim().is_empty() || k.chars().count() > MAX_TEXT) {
            return Err(invalid("keys"));
        }
        if self.color.as_deref().is_some_and(|c| parse_color(c).is_none()) {
            return Err(invalid("color"));
        }
        if self.repeat.is_some_and(|r| !(1..=1000).contains(&r)) {
            return Err(invalid("repeat"));
        }
        if !(-1000..=1000).contains(&self.priority) {
            return Err(invalid("priority"));
        }
        if self.speed.is_some_and(|v| !v.is_finite() || !(0.1..=10.0).contains(&v)) {
            return Err(invalid("speed"));
        }
        self.secs()?;
        Ok(())
    }

    fn secs(&self) -> Res<Option<f64>> {
        let s = match &self.duration {
            None => return Ok(None),
            Some(DurationSpec::Secs(v)) => Some(*v).filter(|v| v.is_finite() && *v >= 0.0),
            Some(DurationSpec::Text(t)) => parse_duration(t),
        };
        match s {
            Some(v) if (0.05..=MAX_SECS).contains(&v) => Ok(Some(v)),
            _ => Err(invalid("duration")),
        }
    }
}

/// How long the overlay lasts: `duration`, else `repeat` passes of the animation, else 3 s;
/// when both are given the shorter wins.
pub fn effective_secs(spec: &TriggerSpec, anim: &Animation) -> Res<f64> {
    let explicit = spec.secs()?;
    let by_repeat = spec.repeat.map(|r| (animations::cycle_secs(anim) as f64 * r as f64).clamp(0.05, MAX_SECS));
    Ok(match (explicit, by_repeat) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        // A sequence has a length of its own: play it once.
        (None, None) if matches!(anim.kind, animations::AnimKind::Sequence { .. }) => (animations::cycle_secs(anim) as f64).clamp(0.05, MAX_SECS),
        (None, None) => DEFAULT_SECS,
    })
}

/// The first copy of `spec` with the notification's own settings underneath: anything given
/// explicitly (command line, socket) wins over what the notification says.
pub fn merged(n: &NamedTrigger, spec: &TriggerSpec) -> TriggerSpec {
    TriggerSpec {
        animation: n.anim.clone(),
        keys: if spec.keys.is_empty() { n.keys.clone() } else { spec.keys.clone() },
        color: spec.color.clone().or_else(|| n.color.clone()),
        duration: spec.duration.clone().or(n.duration.map(DurationSpec::Secs)),
        device: spec.device.clone(),
        repeat: spec.repeat.or(n.repeat),
        priority: if spec.priority != 0 { spec.priority } else { n.priority },
        speed: spec.speed.or(n.speed),
        replace: spec.replace || n.replace,
    }
}

/// Starts a trigger. When `spec.animation` names a notification (of the active profile or a
/// global one) the notification decides the animation, devices, keys, colour and duration.
pub fn start(engine: &Engine, dir: &Path, spec: &TriggerSpec, now: Instant) -> Res<TriggerResult> {
    start_with(engine, dir, spec, now, None)
}

/// Like [`start`], playing for exactly `secs` seconds when given (the scheduler's `Every` and `Always`).
pub fn start_with(engine: &Engine, dir: &Path, spec: &TriggerSpec, now: Instant, secs: Option<f64>) -> Res<TriggerResult> {
    spec.validate()?;
    let name = spec.animation.trim();
    let Some(n) = engine.notification(name) else { return start_one(engine, dir, spec, now, None, None, secs) };
    if !n.enabled {
        return Err(ApiError::new("notification_disabled", name));
    }
    start_notification(engine, dir, name, &n, spec, now, secs)
}

/// Fires `n` as it is (an automation being edited, not yet saved): what the editor's Test button does.
pub fn test_notification(engine: &Engine, dir: &Path, name: &str, n: &NamedTrigger, now: Instant) -> Res<TriggerResult> {
    let mut map = BTreeMap::new();
    map.insert(name.to_string(), n.clone());
    crate::profiles::validate_triggers(&map)?;
    start_notification(engine, dir, name, n, &TriggerSpec { animation: name.into(), ..Default::default() }, now, None)
}

fn start_notification(engine: &Engine, dir: &Path, name: &str, n: &NamedTrigger, spec: &TriggerSpec, now: Instant, secs: Option<f64>) -> Res<TriggerResult> {
    let base = merged(n, spec);
    let targets: Vec<Option<String>> = if spec.device.is_some() {
        vec![spec.device.clone()]
    } else if !n.devices.is_empty() {
        n.devices.iter().cloned().map(Some).collect()
    } else {
        vec![n.device.clone()]
    };
    let many = targets.len() > 1;
    let mut total = TriggerResult { overlay: 0, devices: 0, duration: 0.0 };
    let mut last_err = None;
    for t in targets {
        match start_one(engine, dir, &TriggerSpec { device: t, ..base.clone() }, now, Some(name), n.sequence.as_deref(), secs) {
            Ok(r) => {
                total.overlay = r.overlay;
                total.devices += r.devices;
                total.duration = total.duration.max(r.duration);
            }
            // A chosen device that is off or absent is skipped as long as another one shows it.
            Err(e) if many && e.code == "trigger_no_device" => last_err = Some(e),
            Err(e) => return Err(e),
        }
    }
    if total.devices == 0 {
        return Err(last_err.unwrap_or_else(|| ApiError::new("trigger_no_device", "")));
    }
    Ok(total)
}

/// An application notification as seen on the session bus (only these three fields are read).
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopNote {
    pub app: String,
    /// 0 low, 1 normal, 2 critical.
    pub urgency: u8,
    pub summary: String,
}

pub fn desktop_matches(m: &DesktopMatch, note: &DesktopNote) -> bool {
    // Several alternatives may be given separated by commas ("element, fractal, nheko").
    let has = |hay: &str, needle: &str| {
        let alts: Vec<String> = needle.split(',').map(|a| a.trim().to_lowercase()).filter(|a| !a.is_empty()).collect();
        alts.is_empty() || alts.iter().any(|a| hay.to_lowercase().contains(a))
    };
    let urgency_ok = match m.urgency.as_deref() {
        None => true,
        Some("low") => note.urgency == 0,
        Some("normal") => note.urgency == 1,
        Some("critical") => note.urgency == 2,
        Some(_) => false,
    };
    urgency_ok && has(&note.app, &m.app) && has(&note.summary, &m.contains)
}

/// Fires every enabled notification whose desktop rule matches; returns how many fired.
pub fn fire_desktop(engine: &Engine, dir: &Path, note: &DesktopNote, now: Instant) -> usize {
    let hits: Vec<String> = engine.notifications().into_iter().filter(|(_, n)| n.enabled && n.sources.desktop.as_ref().is_some_and(|m| desktop_matches(m, note))).map(|(k, _)| k).collect();
    fire_all(engine, dir, hits, now)
}

/// Fires every enabled notification listening to `event` (`omarchy:battery-low`...).
pub fn fire_event(engine: &Engine, dir: &Path, event: &str, now: Instant) -> usize {
    let hits: Vec<String> = engine.notifications().into_iter().filter(|(_, n)| n.enabled && n.sources.omarchy.iter().any(|e| e == event)).map(|(k, _)| k).collect();
    fire_all(engine, dir, hits, now)
}

/// Fires every enabled notification whose AI rule matches `from` / `kind`; returns how many fired.
pub fn fire_ai(engine: &Engine, dir: &Path, from: &str, kind: &str, now: Instant) -> usize {
    let hits: Vec<String> = engine.notifications().into_iter().filter(|(_, n)| n.enabled && n.sources.ai.as_ref().is_some_and(|m| ai_matches(m, from, kind))).map(|(k, _)| k).collect();
    fire_all(engine, dir, hits, now)
}

/// Whether an AI rule selects an event from `from` of `kind` (empty fields match anything).
pub fn ai_matches(m: &AiMatch, from: &str, kind: &str) -> bool {
    let same = |rule: &str, got: &str| rule.trim().is_empty() || rule.trim().eq_ignore_ascii_case(got.trim());
    same(&m.from, from) && same(&m.kind, kind)
}

/// Stops what AI automations are showing (the assistant went on working); returns how many overlays ended.
pub fn clear_ai(engine: &Engine) -> usize {
    let names: Vec<String> = engine.notifications().into_iter().filter(|(_, n)| n.sources.ai.is_some()).map(|(k, _)| k).collect();
    let mut overlays = engine.overlays.lock().unwrap();
    names.iter().map(|n| overlays.remove_prefix(&format!("notif:{n}#"))).sum()
}

fn fire_all(engine: &Engine, dir: &Path, names: Vec<String>, now: Instant) -> usize {
    names.iter().filter(|n| start(engine, dir, &TriggerSpec { animation: (*n).clone(), ..Default::default() }, now).is_ok()).count()
}

/// Starts one overlay on the devices that match `spec`. `tag` is the notification name (for `replace`).
fn start_one(engine: &Engine, dir: &Path, spec: &TriggerSpec, now: Instant, tag: Option<&str>, inline: Option<&Animation>, secs: Option<f64>) -> Res<TriggerResult> {
    if inline.is_none() {
        spec.validate()?;
    }
    let mut anim = match inline {
        Some(a) => a.clone(),
        None => animations::lookup(dir, &spec.animation)?,
    };
    if secs.is_some()
        && let animations::AnimKind::Sequence { looped, .. } = &mut anim.kind
    {
        // Timed runs (always on, every N seconds) repeat the sequence until their time is up.
        *looped = true;
    }
    if let Some(k) = spec.speed {
        anim = animations::with_speed(&anim, k);
    }
    let secs = match secs {
        Some(s) => s.clamp(0.05, MAX_SECS),
        None => effective_secs(spec, &anim)?,
    };
    let keys: Vec<String> = spec.keys.iter().map(|k| k.trim().to_string()).collect();
    let device = spec.device.as_deref().map(str::trim).filter(|d| !d.is_empty()).map(str::to_string);

    let devices = {
        let scene = engine.scene.lock().unwrap();
        let devs = engine.devs.lock().unwrap();
        devs.iter()
            .filter(|d| scene.devices.get(&d.key).is_some_and(|s| s.mode == SceneMode::Effects))
            .filter(|d| overlay::device_matches(device.as_deref(), d))
            .filter(|d| keys.is_empty() || !overlay::resolve_keys(&keys, &d.led_names).is_empty())
            .count()
    };
    if devices == 0 {
        return Err(ApiError::new("trigger_no_device", ""));
    }
    if let Some(c) = spec.color.as_deref().and_then(Color::parse) {
        // `@slot` colours are taken from the palette at the moment the trigger fires.
        let pal = engine.themes.palette();
        anim = animations::recolor(&anim, pal.resolve(&c), &pal);
    }
    let priority = spec.priority;
    let mut overlays = engine.overlays.lock().unwrap();
    if let (true, Some(tag)) = (spec.replace, tag) {
        overlays.remove_prefix(&format!("notif:{tag}#"));
    }
    let id = overlays.add(now, |id| {
        // A recoloured copy must not share the resolution cache entry of the original.
        anim.id = match tag {
            Some(tag) => format!("notif:{tag}#{id}"),
            None => format!("{}#ov{id}", anim.id),
        };
        Overlay { id, anim, device, keys, start: now, until: now + Duration::from_secs_f64(secs), priority }
    })?;
    Ok(TriggerResult { overlay: id, devices, duration: secs })
}

/// Stops every overlay; returns how many were active.
pub fn stop_all(engine: &Engine) -> usize {
    engine.overlays.lock().unwrap().clear()
}

fn err_json(e: &ApiError) -> Value {
    json!({ "ok": false, "error": e.code, "detail": e.detail })
}

/// `{"accent": "#ff0000", "alert": [255, 0, 0]}` -> slots.
pub fn parse_slots(v: Option<&Value>) -> Result<BTreeMap<String, [u8; 3]>, String> {
    let m = v.and_then(Value::as_object).ok_or("slots must be an object like {\"accent\": \"#ff0000\"}")?;
    if m.is_empty() || m.len() > themes::SLOTS.len() {
        return Err("1 to 18 slots expected".into());
    }
    m.iter()
        .map(|(k, v)| {
            let c = match v {
                Value::String(s) => themes::parse_hex(s),
                Value::Array(a) if a.len() == 3 => {
                    let n = |i: usize| a[i].as_u64().filter(|n| *n <= 255).map(|n| n as u8);
                    n(0).zip(n(1)).zip(n(2)).map(|((r, g), b)| [r, g, b])
                }
                _ => None,
            };
            c.map(|c| (k.clone(), c)).ok_or_else(|| format!("bad colour for slot {k:?}"))
        })
        .collect()
}

/// Handles one request line of the socket protocol and returns the reply.
/// `{"cmd":"trigger", "animation": "...", ...}`, `{"cmd":"stop"}`, `{"cmd":"list"}`, `{"cmd":"ping"}`,
/// `{"cmd":"set_palette","slots":{...}}`, `{"cmd":"clear_palette"}`, `{"cmd":"palette"}`,
/// `{"cmd":"palette_reload"}`, `{"cmd":"palette_source","source":"omarchy"}` (`set_profile` is
/// handled by the socket server, which can reach the application).
pub fn handle_line(engine: &Engine, dir: &Path, line: &str, now: Instant) -> Value {
    let v: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": "bad_json", "detail": e.to_string() }),
    };
    let Value::Object(mut obj) = v else {
        return json!({ "ok": false, "error": "bad_request", "detail": "expected a JSON object" });
    };
    let cmd = match obj.remove("cmd") {
        None => "trigger".to_string(),
        Some(Value::String(s)) => s,
        Some(_) => return json!({ "ok": false, "error": "bad_request", "detail": "cmd must be a string" }),
    };
    match cmd.as_str() {
        "ping" => json!({ "ok": true }),
        "stop" => json!({ "ok": true, "stopped": stop_all(engine) }),
        "event" => {
            let text = |k: &str| match obj.get(k) {
                None => Some(String::new()),
                Some(Value::String(s)) if s.chars().count() <= 32 => Some(s.trim().to_string()),
                _ => None,
            };
            match obj.get("event").and_then(Value::as_str) {
                Some(e) if crate::profiles::OMARCHY_EVENTS.contains(&e) => json!({ "ok": true, "fired": fire_event(engine, dir, e, now) }),
                Some("ai:attention") => match (text("from"), text("kind")) {
                    (Some(from), Some(kind)) => json!({ "ok": true, "fired": fire_ai(engine, dir, &from, &kind, now) }),
                    _ => json!({ "ok": false, "error": "bad_request", "detail": "from and kind must be short strings" }),
                },
                Some("ai:clear") => json!({ "ok": true, "stopped": clear_ai(engine) }),
                _ => json!({ "ok": false, "error": "bad_request", "detail": "event must be omarchy:battery-low, omarchy:theme-set, omarchy:post-update, ai:attention or ai:clear" }),
            }
        }
        "palette" => json!({ "ok": true, "palette": engine.themes.event() }),
        "palette_reload" => json!({ "ok": true, "changed": engine.themes.reload(), "palette": engine.themes.event() }),
        "clear_palette" => json!({ "ok": true, "changed": engine.themes.clear_external() }),
        "set_palette" => match parse_slots(obj.get("slots")) {
            Err(why) => json!({ "ok": false, "error": "bad_request", "detail": why }),
            Ok(slots) => match engine.themes.set_external(slots) {
                Ok(changed) => json!({ "ok": true, "changed": changed }),
                Err(why) => json!({ "ok": false, "error": "bad_request", "detail": why }),
            },
        },
        "palette_source" => match obj.get("source").and_then(Value::as_str).and_then(ThemeSource::parse) {
            None => json!({ "ok": false, "error": "bad_request", "detail": "source must be manual, omarchy, kde, pywal, gtk or external" }),
            Some(src) => json!({ "ok": true, "changed": engine.themes.set_runtime_source(src), "palette": engine.themes.event() }),
        },
        "list" => {
            let names: Vec<String> = animations::list_all(dir).animations.into_iter().map(|a| a.name).collect();
            let triggers: Vec<String> = engine.profile_triggers.lock().unwrap().keys().cloned().collect();
            json!({ "ok": true, "animations": names, "triggers": triggers })
        }
        "trigger" => match serde_json::from_value::<TriggerSpec>(Value::Object(obj)) {
            Err(e) => json!({ "ok": false, "error": "bad_request", "detail": e.to_string() }),
            Ok(spec) => match start(engine, dir, &spec, now) {
                Ok(r) => json!({ "ok": true, "overlay": r.overlay, "devices": r.devices, "duration": r.duration }),
                Err(e) => err_json(&e),
            },
        },
        other => json!({ "ok": false, "error": "unknown_cmd", "detail": other }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::{AnimKind, Frame, LoopMode, Target};
    use crate::engine::{DevInfo, DeviceScene};
    use crate::fsutil::test_dir;

    fn engine(mode: SceneMode) -> Engine {
        let e = Engine::default();
        let names: Vec<String> = ["Key: W", "Key: A", "Key: S", "Key: D"].iter().map(|s| s.to_string()).collect();
        e.devs.lock().unwrap().push(DevInfo {
            index: 0,
            key: "kb#1".into(),
            name: "Test Keyboard".into(),
            device_type: "Keyboard".into(),
            led_count: 4,
            led_names: names,
            zones: vec![(0, 0, 4)],
            default_positions: vec![[0.5, 0.5]; 4],
        });
        e.scene.lock().unwrap().devices.insert("kb#1".into(), DeviceScene { mode, ..Default::default() });
        e
    }

    fn lib() -> std::path::PathBuf {
        let d = test_dir("trig");
        let a = Animation {
            schema_version: 1,
            id: String::new(),
            name: "Alerta".into(),
            description: None,
            tags: vec![],
            created: 0,
            updated: 0,
            target: Target::default(),
            builtin: false,
            kind: AnimKind::Frames { frames: vec![Frame { leds: [("name:Key: W".to_string(), Color::Rgb([255, 0, 0]))].into(), duration_ms: Some(500) }], frame_ms: 500, loop_mode: LoopMode::Loop, crossfade: 0.0 },
        };
        animations::save(&d, &a).unwrap();
        d
    }

    #[test]
    fn duration_and_color_parsing() {
        assert_eq!(parse_duration("3s"), Some(3.0));
        assert_eq!(parse_duration("500ms"), Some(0.5));
        assert_eq!(parse_duration("1.5"), Some(1.5));
        assert_eq!(parse_duration("2m"), Some(120.0));
        assert_eq!(parse_duration(" 2S "), Some(2.0));
        for bad in ["", "abc", "-1s", "NaN", "inf", "1x"] {
            assert_eq!(parse_duration(bad), None, "{bad}");
        }
        assert_eq!(parse_color("#ff8000"), Some(Color::Rgb([255, 128, 0])));
        assert_eq!(parse_color("@alert"), Some(Color::slot("alert")));
        assert_eq!(parse_color("@bogus"), None);
        assert_eq!(parse_color("f00"), Some(Color::Rgb([255, 0, 0])));
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("#gg0000"), None);
        assert_eq!(parse_color("é12345"), None);
    }

    #[test]
    fn trigger_starts_overlay_for_matching_devices() {
        let e = engine(SceneMode::Effects);
        let d = lib();
        let spec = TriggerSpec { animation: "alerta".into(), keys: vec!["W".into(), "A".into()], duration: Some(DurationSpec::Text("2s".into())), ..Default::default() };
        let t0 = Instant::now();
        let r = start(&e, &d, &spec, t0).unwrap();
        assert_eq!((r.devices, r.duration), (1, 2.0));
        assert_eq!(e.overlays.lock().unwrap().active(t0).len(), 1);
        assert_eq!(e.overlays.lock().unwrap().active(t0 + Duration::from_secs(3)).len(), 0, "expires");
    }

    #[test]
    fn trigger_errors() {
        let d = lib();
        let t0 = Instant::now();
        let code = |e: &Engine, s: &TriggerSpec| start(e, &d, s, t0).unwrap_err().code;
        let base = TriggerSpec { animation: "Alerta".into(), ..Default::default() };
        let eff = engine(SceneMode::Effects);
        assert_eq!(code(&eff, &TriggerSpec { animation: "Nope".into(), ..base.clone() }), "anim_not_found");
        assert_eq!(code(&eff, &TriggerSpec { device: Some("mouse".into()), ..base.clone() }), "trigger_no_device");
        assert_eq!(code(&eff, &TriggerSpec { keys: vec!["F13".into()], ..base.clone() }), "trigger_no_device");
        assert_eq!(code(&engine(SceneMode::Hardware), &base), "trigger_no_device", "hardware-mode devices are left alone");
        assert_eq!(code(&eff, &TriggerSpec { duration: Some(DurationSpec::Text("soon".into())), ..base.clone() }), "trigger_invalid");
        assert_eq!(code(&eff, &TriggerSpec { duration: Some(DurationSpec::Secs(-1.0)), ..base.clone() }), "trigger_invalid");
        assert_eq!(code(&eff, &TriggerSpec { color: Some("red".into()), ..base.clone() }), "trigger_invalid");
        assert_eq!(code(&eff, &TriggerSpec { repeat: Some(0), ..base.clone() }), "trigger_invalid");
        assert_eq!(code(&eff, &TriggerSpec { keys: vec!["x".into(); 65], ..base.clone() }), "trigger_invalid");
        assert_eq!(code(&eff, &TriggerSpec { animation: "x".repeat(200), ..base.clone() }), "trigger_invalid");
        assert_eq!(code(&eff, &TriggerSpec { priority: 99999, ..base }), "trigger_invalid");
    }

    #[test]
    fn repeat_and_duration_combine() {
        let d = lib();
        let a = animations::lookup(&d, "Alerta").unwrap(); // one pass = 0.5 s
        let spec = |dur: Option<f64>, rep: Option<u32>| TriggerSpec { animation: "Alerta".into(), duration: dur.map(DurationSpec::Secs), repeat: rep, ..Default::default() };
        assert_eq!(effective_secs(&spec(None, None), &a).unwrap(), 3.0);
        assert_eq!(effective_secs(&spec(None, Some(4)), &a).unwrap(), 2.0);
        assert_eq!(effective_secs(&spec(Some(1.0), Some(4)), &a).unwrap(), 1.0);
        assert_eq!(effective_secs(&spec(Some(9.0), Some(4)), &a).unwrap(), 2.0);
    }

    #[test]
    fn color_override_applies() {
        let e = engine(SceneMode::Effects);
        let d = lib();
        let t0 = Instant::now();
        let spec = TriggerSpec { animation: "Alerta".into(), color: Some("#0000ff".into()), ..Default::default() };
        start(&e, &d, &spec, t0).unwrap();
        let active = e.overlays.lock().unwrap().active(t0);
        let mut buf = crate::engine::compositor::black(4);
        let devs = e.devs.lock().unwrap().clone();
        overlay::compose(&mut buf, &active, &devs[0], &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        assert_eq!((buf[0].r, buf[0].b), (0.0, 255.0));
    }

    #[test]
    fn socket_protocol_requests() {
        let e = engine(SceneMode::Effects);
        let d = lib();
        let t0 = Instant::now();
        let h = |line: &str| handle_line(&e, &d, line, t0);
        assert_eq!(h(r#"{"cmd":"ping"}"#), json!({"ok": true}));
        let ok = h(r#"{"cmd":"trigger","animation":"Alerta","keys":["W"],"duration":"2s","priority":3}"#);
        assert_eq!(ok["ok"], true);
        assert_eq!(ok["devices"], 1);
        assert_eq!(h(r#"{"animation":"Alerta"}"#)["ok"], true, "cmd defaults to trigger");
        assert_eq!(h(r#"{"cmd":"list"}"#)["animations"].as_array().unwrap().iter().filter(|n| *n == "Alerta").count(), 1);
        assert_eq!(h(r#"{"cmd":"stop"}"#)["stopped"], 2);
        assert_eq!(e.overlays.lock().unwrap().len(), 0);
        // Bad input never panics and says why.
        for (line, code) in [
            ("not json", "bad_json"),
            ("[1,2]", "bad_request"),
            ("42", "bad_request"),
            (r#"{"cmd": 5}"#, "bad_request"),
            (r#"{"cmd":"explode"}"#, "unknown_cmd"),
            (r#"{"cmd":"trigger"}"#, "bad_request"),
            (r#"{"cmd":"trigger","animation":"Alerta","bogus":1}"#, "bad_request"),
            (r#"{"cmd":"trigger","animation":"Alerta","keys":"W"}"#, "bad_request"),
            (r#"{"cmd":"trigger","animation":"Alerta","duration":{"a":1}}"#, "bad_request"),
            (r#"{"cmd":"trigger","animation":"Alerta","duration":"-3"}"#, "trigger_invalid"),
            (r##"{"cmd":"trigger","animation":"Alerta","color":"#zzz"}"##, "trigger_invalid"),
            (r#"{"cmd":"trigger","animation":"ghost"}"#, "anim_not_found"),
        ] {
            let r = h(line);
            assert_eq!((r["ok"].clone(), r["error"].as_str().unwrap_or("")), (json!(false), code), "{line}: {r}");
        }
    }

    #[test]
    fn limit_via_protocol() {
        let e = engine(SceneMode::Effects);
        let d = lib();
        let t0 = Instant::now();
        for _ in 0..crate::engine::overlay::MAX_OVERLAYS {
            assert_eq!(handle_line(&e, &d, r#"{"animation":"Alerta"}"#, t0)["ok"], true);
        }
        assert_eq!(handle_line(&e, &d, r#"{"animation":"Alerta"}"#, t0)["error"], "overlay_limit");
    }

    fn add_device(e: &Engine, key: &str, name: &str) {
        let names: Vec<String> = ["Key: W", "Key: Q"].iter().map(|s| s.to_string()).collect();
        e.devs.lock().unwrap().push(DevInfo { index: 1, key: key.into(), name: name.into(), device_type: "Keyboard".into(), led_count: 2, led_names: names, zones: vec![(0, 0, 2)], default_positions: vec![[0.5, 0.5]; 2] });
        e.scene.lock().unwrap().devices.insert(key.into(), DeviceScene { mode: SceneMode::Effects, ..Default::default() });
    }

    fn spec(a: &str) -> TriggerSpec {
        TriggerSpec { animation: a.into(), ..Default::default() }
    }

    #[test]
    fn notification_names_decide_the_look_and_flags_win() {
        let e = engine(SceneMode::Effects);
        let d = lib();
        // No notification of that name: the text is an animation name.
        assert!(e.notification("Alerta").is_none());
        assert_eq!(start(&e, &d, &spec("Alerta"), Instant::now()).unwrap().devices, 1);
        // Two profiles give the same name a different meaning.
        let mut neon = NamedTrigger::new("Breach Alert");
        neon.keys = vec!["W".into()];
        neon.duration = Some(2.0);
        e.profile_triggers.lock().unwrap().insert("alert".into(), neon.clone());
        let r = start(&e, &d, &spec("alert"), Instant::now()).unwrap();
        assert!((r.duration - 2.0).abs() < 1e-6);
        assert_eq!(e.overlays.lock().unwrap().active(Instant::now()).last().unwrap().keys, vec!["W".to_string()]);
        let mut calm = NamedTrigger::new("Pulse");
        calm.color = Some("@alert".into());
        e.profile_triggers.lock().unwrap().insert("alert".into(), calm);
        start(&e, &d, &spec("alert"), Instant::now()).unwrap();
        let ov = e.overlays.lock().unwrap().active(Instant::now());
        let last = ov.last().unwrap();
        assert!(last.anim.id.starts_with("notif:alert#"), "{}", last.anim.id);
        // Explicit command-line values win over the notification's.
        let m = merged(&neon, &TriggerSpec { animation: "alert".into(), keys: vec!["Q".into()], color: Some("#00ff00".into()), duration: Some(DurationSpec::Secs(9.0)), ..Default::default() });
        assert_eq!((m.keys, m.color.as_deref(), m.duration), (vec!["Q".to_string()], Some("#00ff00"), Some(DurationSpec::Secs(9.0))));
    }

    #[test]
    fn global_notifications_apply_everywhere_and_the_profile_overrides_them() {
        let e = engine(SceneMode::Effects);
        let d = lib();
        e.global_triggers.lock().unwrap().insert("ping".into(), NamedTrigger::new("Alerta"));
        e.global_triggers.lock().unwrap().insert("both".into(), NamedTrigger::new("Alerta"));
        assert_eq!(start(&e, &d, &spec("ping"), Instant::now()).unwrap().devices, 1);
        let mut own = NamedTrigger::new("Alerta");
        own.enabled = false;
        e.profile_triggers.lock().unwrap().insert("both".into(), own);
        assert_eq!(start(&e, &d, &spec("both"), Instant::now()).unwrap_err().code, "notification_disabled");
        assert_eq!(e.notifications().len(), 2);
        assert_eq!(start(&e, &d, &spec("ping"), Instant::now()).unwrap().devices, 1, "the other global one still works");
    }

    #[test]
    fn devices_replace_speed_and_repeat() {
        let e = engine(SceneMode::Effects);
        add_device(&e, "kb#2", "Second Keyboard");
        let d = lib();
        let mut n = NamedTrigger::new("Alerta");
        n.devices = vec!["kb#2".into(), "gone#9".into()];
        e.profile_triggers.lock().unwrap().insert("one".into(), n);
        let r = start(&e, &d, &spec("one"), Instant::now()).unwrap();
        assert_eq!(r.devices, 1, "only the chosen (and present) device");
        assert!(e.overlays.lock().unwrap().active(Instant::now()).iter().all(|o| o.device.as_deref() == Some("kb#2") || o.device.as_deref() == Some("gone#9")));
        // Several devices at once.
        let mut n = NamedTrigger::new("Alerta");
        n.devices = vec!["kb#1".into(), "kb#2".into()];
        e.profile_triggers.lock().unwrap().insert("both".into(), n);
        assert_eq!(start(&e, &d, &spec("both"), Instant::now()).unwrap().devices, 2);
        // Replace: firing again does not stack.
        e.overlays.lock().unwrap().clear();
        let mut n = NamedTrigger::new("Alerta");
        n.replace = true;
        e.profile_triggers.lock().unwrap().insert("r".into(), n);
        for _ in 0..3 {
            start(&e, &d, &spec("r"), Instant::now()).unwrap();
        }
        assert_eq!(e.overlays.lock().unwrap().len(), 1);
        let mut n = NamedTrigger::new("Alerta");
        e.profile_triggers.lock().unwrap().insert("stack".into(), n.clone());
        for _ in 0..3 {
            start(&e, &d, &spec("stack"), Instant::now()).unwrap();
        }
        assert_eq!(e.overlays.lock().unwrap().len(), 4, "stacking is the default");
        // Speed halves the time of a frames animation; repeat multiplies passes.
        n.repeat = Some(2);
        n.speed = Some(2.0);
        e.profile_triggers.lock().unwrap().insert("fast".into(), n);
        let r = start(&e, &d, &spec("fast"), Instant::now()).unwrap();
        assert!((r.duration - 2.0 * 0.5 / 2.0).abs() < 0.01, "2 passes of a 0.5 s animation at double speed: {}", r.duration);
    }

    #[test]
    fn desktop_and_omarchy_sources_fire_matching_notifications_only() {
        use crate::profiles::{DesktopMatch, Sources};
        let e = engine(SceneMode::Effects);
        let d = lib();
        let mut disc = NamedTrigger::new("Alerta");
        disc.sources = Sources { ai: None, always: false, every: None, threshold: None, desktop: Some(DesktopMatch { app: "disc".into(), urgency: None, contains: "message".into() }), omarchy: vec![] };
        let mut crit = NamedTrigger::new("Alerta");
        crit.sources.desktop = Some(DesktopMatch { app: String::new(), urgency: Some("critical".into()), contains: String::new() });
        let mut bat = NamedTrigger::new("Alerta");
        bat.sources.omarchy = vec!["omarchy:battery-low".into()];
        let mut off = bat.clone();
        off.enabled = false;
        for (k, v) in [("disc", disc), ("crit", crit), ("bat", bat), ("off", off)] {
            e.profile_triggers.lock().unwrap().insert(k.into(), v);
        }
        let note = |app: &str, urgency: u8, summary: &str| DesktopNote { app: app.into(), urgency, summary: summary.into() };
        let now = Instant::now();
        assert_eq!(fire_desktop(&e, &d, &note("Discord", 1, "New MESSAGE from Ana"), now), 1);
        assert_eq!(fire_desktop(&e, &d, &note("Discord", 1, "Server update"), now), 0, "text must match too");
        assert_eq!(fire_desktop(&e, &d, &note("Telegram", 1, "New message"), now), 0, "wrong app");
        assert_eq!(fire_desktop(&e, &d, &note("anything", 2, "boom"), now), 1, "critical urgency rule");
        assert_eq!(fire_desktop(&e, &d, &note("Discord", 2, "New message"), now), 2, "both rules match");
        assert_eq!(fire_event(&e, &d, "omarchy:battery-low", now), 1, "the disabled one stays quiet");
        assert_eq!(fire_event(&e, &d, "omarchy:post-update", now), 0);
        // Over the socket protocol.
        let r = handle_line(&e, &d, r#"{"cmd":"event","event":"omarchy:battery-low"}"#, now);
        assert_eq!((r["ok"].as_bool(), r["fired"].as_u64()), (Some(true), Some(1)));
        assert_eq!(handle_line(&e, &d, r#"{"cmd":"event","event":"bogus"}"#, now)["ok"], false);
    }

    #[test]
    fn desktop_app_filter_accepts_a_comma_separated_list() {
        let m = DesktopMatch { app: "Element, fractal ,nheko".into(), urgency: None, contains: String::new() };
        let note = |app: &str| DesktopNote { app: app.into(), urgency: 1, summary: "x".into() };
        assert!(desktop_matches(&m, &note("Element Desktop")));
        assert!(desktop_matches(&m, &note("org.gnome.Fractal")));
        assert!(!desktop_matches(&m, &note("Slack")));
        assert!(desktop_matches(&DesktopMatch { app: " , ".into(), ..m }, &note("anything")));
    }

    fn inline_seq(secs: u32) -> Animation {
        use crate::animations::{AnimKind, Step};
        Animation {
            schema_version: 1,
            id: String::new(),
            name: "inline".into(),
            description: None,
            tags: vec![],
            created: 0,
            updated: 0,
            target: crate::animations::Target::default(),
            builtin: false,
            kind: AnimKind::Sequence { steps: vec![Step { effect: "blink".into(), params: crate::engine::effects::params(json!({"color": [0, 0, 255]})), keys: vec!["name:Key: W".into()], duration_ms: secs * 1000, anim: None }], looped: false },
        }
    }

    #[test]
    fn inline_sequence_notifications_play_for_their_own_length() {
        let e = engine(SceneMode::Effects);
        let dir = lib();
        let mut n = NamedTrigger::new("");
        n.sequence = Some(Box::new(inline_seq(2)));
        e.global_triggers.lock().unwrap().insert("chat".into(), n);
        let t0 = Instant::now();
        let r = start(&e, &dir, &TriggerSpec { animation: "chat".into(), ..Default::default() }, t0).unwrap();
        assert_eq!((r.devices, r.duration), (1, 2.0));
        // An explicit duration still wins.
        let r = start(&e, &dir, &TriggerSpec { animation: "chat".into(), duration: Some(DurationSpec::Secs(1.0)), ..Default::default() }, t0).unwrap();
        assert_eq!(r.duration, 1.0);
        assert_eq!(e.overlays.lock().unwrap().active(t0).len(), 2);
    }

    #[test]
    fn ai_events_fire_matching_automations_and_clear_ends_them() {
        let e = engine(SceneMode::Effects);
        let dir = lib();
        let mut any = NamedTrigger::new("Alerta");
        any.sources.ai = Some(AiMatch::default());
        let mut claude = NamedTrigger::new("Alerta");
        claude.sources.ai = Some(AiMatch { from: "Claude".into(), kind: "permission".into() });
        let mut off = NamedTrigger::new("Alerta");
        off.enabled = false;
        off.sources.ai = Some(AiMatch::default());
        for (k, n) in [("any", any), ("claude", claude), ("off", off)] {
            e.global_triggers.lock().unwrap().insert(k.into(), n);
        }
        let now = Instant::now();
        assert_eq!(fire_ai(&e, &dir, "claude", "permission", now), 2);
        assert_eq!(fire_ai(&e, &dir, "codex", "idle", now), 1, "only the open rule");
        assert_eq!(fire_ai(&e, &dir, "claude", "", now), 1, "kind missing does not match a rule that wants permission");
        let r = handle_line(&e, &dir, r#"{"cmd":"event","event":"ai:attention","from":"claude","kind":"permission"}"#, now);
        assert_eq!((r["ok"].as_bool(), r["fired"].as_u64()), (Some(true), Some(2)));
        for bad in [r#"{"cmd":"event","event":"ai:attention","from":5}"#, r#"{"cmd":"event","event":"ai:nope"}"#] {
            assert_eq!(handle_line(&e, &dir, bad, now)["ok"], false, "{bad}");
        }
        let long = format!(r#"{{"cmd":"event","event":"ai:attention","from":"{}"}}"#, "x".repeat(40));
        assert_eq!(handle_line(&e, &dir, &long, now)["ok"], false);
        assert!(!e.overlays.lock().unwrap().is_empty());
        let r = handle_line(&e, &dir, r#"{"cmd":"event","event":"ai:clear"}"#, now);
        assert!(r["stopped"].as_u64().unwrap() >= 4);
        assert_eq!(e.overlays.lock().unwrap().len(), 0);
    }

    #[test]
    fn old_notification_json_loads_unchanged_and_new_triggers_round_trip() {
        let old = r#"{"anim":"builtin:flash","color":"@info","duration":1.5,"sources":{"desktop":{"app":"discord"},"omarchy":["omarchy:battery-low"]}}"#;
        let n: NamedTrigger = serde_json::from_str(old).unwrap();
        assert!(n.sequence.is_none() && !n.sources.always && n.sources.ai.is_none());
        assert_eq!(serde_json::to_string(&n).unwrap(), serde_json::to_string(&serde_json::from_str::<NamedTrigger>(old).unwrap()).unwrap());
        assert!(!serde_json::to_string(&n).unwrap().contains("always"));
        let new = r#"{"sequence":null,"anim":"x","sources":{"always":true,"every":{"every_secs":60},"threshold":{"metric":"cpu.temp","value":80},"ai":{}}}"#;
        let n: NamedTrigger = serde_json::from_str(new).unwrap();
        let th = n.sources.threshold.clone().unwrap();
        assert_eq!((th.op.as_str(), th.hysteresis, th.cooldown_secs), ("gt", 2.0, 60.0));
        assert_eq!(n.sources.every.clone().unwrap().for_secs, 5.0);
        let back: NamedTrigger = serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        assert_eq!(back, n);
        let mut map = std::collections::BTreeMap::new();
        map.insert("ok".to_string(), n.clone());
        crate::profiles::validate_triggers(&map).unwrap();
        for tweak in [
            |n: &mut NamedTrigger| n.sources.threshold.as_mut().unwrap().metric = "nope".into(),
            |n: &mut NamedTrigger| n.sources.threshold.as_mut().unwrap().op = "eq".into(),
            |n: &mut NamedTrigger| n.sources.every.as_mut().unwrap().every_secs = 0.1,
            |n: &mut NamedTrigger| n.sources.ai.as_mut().unwrap().from = "x".repeat(40),
            |n: &mut NamedTrigger| n.anim = String::new(),
        ] {
            let mut bad = n.clone();
            tweak(&mut bad);
            let mut m = std::collections::BTreeMap::new();
            m.insert("bad".to_string(), bad);
            assert!(crate::profiles::validate_triggers(&m).is_err());
        }
    }

    #[test]
    fn test_notification_fires_an_unsaved_automation_and_rejects_invalid_ones() {
        let e = engine(SceneMode::Effects);
        let dir = lib();
        let mut n = NamedTrigger::new("");
        n.sequence = Some(Box::new(inline_seq(2)));
        n.enabled = false;
        let r = test_notification(&e, &dir, "draft", &n, Instant::now()).unwrap();
        assert_eq!((r.devices, r.duration), (1, 2.0), "works for a disabled, never saved automation");
        n.sequence = None;
        assert_eq!(test_notification(&e, &dir, "draft", &n, Instant::now()).unwrap_err().code, "profile_invalid");
        assert_eq!(test_notification(&e, &dir, "bad name!", &NamedTrigger::new("Alerta"), Instant::now()).unwrap_err().code, "profile_invalid");
    }
}
