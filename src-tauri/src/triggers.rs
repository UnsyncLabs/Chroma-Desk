//! Triggers: start/stop overlay animations from the UI, the CLI and the local socket.

use crate::animations::{self, Animation};
use crate::engine::overlay::{self, Overlay};
use crate::engine::{Engine, SceneMode};
use crate::error::{ApiError, Res};
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
    /// `#rrggbb` (or `#rgb`): recolours the animation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Number of passes of the animation (limited by `duration` when both are given).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<u32>,
    /// Higher is drawn on top of lower.
    #[serde(default)]
    pub priority: i32,
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

/// `#ff0000`, `ff0000`, `#f00`.
pub fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if !h.is_ascii() {
        return None;
    }
    let byte = |a: &str| u8::from_str_radix(a, 16).ok();
    match h.len() {
        6 => Some([byte(&h[0..2])?, byte(&h[2..4])?, byte(&h[4..6])?]),
        3 => Some([byte(&h[0..1])? * 17, byte(&h[1..2])? * 17, byte(&h[2..3])? * 17]),
        _ => None,
    }
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
        (None, None) => DEFAULT_SECS,
    })
}

/// Starts an overlay. Errors: unknown animation, bad fields, no matching device, limit reached.
pub fn start(engine: &Engine, dir: &Path, spec: &TriggerSpec, now: Instant) -> Res<TriggerResult> {
    spec.validate()?;
    let mut anim = animations::lookup(dir, &spec.animation)?;
    let secs = effective_secs(spec, &anim)?;
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
    if let Some(c) = spec.color.as_deref().and_then(parse_color) {
        anim = animations::recolor(&anim, c);
    }
    let priority = spec.priority;
    let id = engine.overlays.lock().unwrap().add(now, |id| {
        // A recoloured copy must not share the resolution cache entry of the original.
        anim.id = format!("{}#ov{id}", anim.id);
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

/// Handles one request line of the socket protocol and returns the reply.
/// `{"cmd":"trigger", "animation": "...", ...}`, `{"cmd":"stop"}`, `{"cmd":"list"}`, `{"cmd":"ping"}`.
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
        "list" => {
            let names: Vec<String> = animations::list_all(dir).animations.into_iter().map(|a| a.name).collect();
            json!({ "ok": true, "animations": names })
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
            kind: AnimKind::Frames { frames: vec![Frame { leds: [("name:Key: W".to_string(), [255, 0, 0])].into(), duration_ms: Some(500) }], frame_ms: 500, loop_mode: LoopMode::Loop, crossfade: 0.0 },
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
        assert_eq!(parse_color("#ff8000"), Some([255, 128, 0]));
        assert_eq!(parse_color("f00"), Some([255, 0, 0]));
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
        overlay::compose(&mut buf, &active, &devs[0], &[[0.5, 0.5]; 4], t0);
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
}
