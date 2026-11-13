//! Pure, deterministic LED effects.
//!
//! `render(effect, params, t, positions, seed, out)` fills `out` (one pixel per LED) for the
//! time `t` (seconds). Nothing is stored between frames: "random" effects derive their
//! randomness from a hash of `(seed, led, time slot)`, so the same inputs always give the same
//! output and tests can check exact values.
//!
//! The parameter schema (`effect_defs`) is the single source of truth for defaults and for the
//! UI, which generates its editor from it.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

/// Normalised LED position (0..1 on both axes) inside its device.
pub type Pos = [f32; 2];

/// Colour channels 0..255 plus coverage `a` (0 = transparent, 1 = opaque).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Px {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Px {
    pub const BLACK: Px = Px { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const CLEAR: Px = Px { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    fn rgb(c: [f32; 3], a: f32) -> Px {
        Px { r: c[0], g: c[1], b: c[2], a }
    }
}

/// Effect parameters as JSON; missing keys fall back to the schema defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Params(pub Map<String, Value>);

type Rgbf = [f32; 3];

impl Params {
    fn num(&self, k: &str) -> f32 {
        self.0.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32
    }
    fn flag(&self, k: &str) -> bool {
        self.0.get(k).and_then(Value::as_bool).unwrap_or(false)
    }
    fn color(&self, k: &str) -> Rgbf {
        self.0.get(k).and_then(parse_color).unwrap_or([0.0; 3])
    }
    fn text(&self, k: &str) -> &str {
        self.0.get(k).and_then(Value::as_str).unwrap_or("")
    }
    /// Colour stops `[{ "pos": 0..1, "color": [r,g,b] }]`, sorted by position.
    fn stops(&self, k: &str) -> Vec<Stop> {
        let mut v: Vec<Stop> = self
            .0
            .get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|s| Some(Stop { pos: s.get("pos")?.as_f64()? as f32, color: parse_color(s.get("color")?)? })).collect())
            .unwrap_or_default();
        v.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        v
    }
    fn colors(&self, k: &str) -> Vec<Rgbf> {
        self.0
            .get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(parse_color).collect())
            .unwrap_or_default()
    }
}

/// A colour stop of a palette.
#[derive(Debug, Clone, Copy)]
struct Stop {
    pos: f32,
    color: Rgbf,
}

fn parse_color(v: &Value) -> Option<Rgbf> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

// ---------- schema ----------

#[derive(Debug, Clone, Serialize)]
pub struct ParamDef {
    pub key: &'static str,
    /// `slider`, `angle`, `color`, `colors`, `bool`, `paint` or `data` (no generic editor).
    pub kind: &'static str,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub default: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct EffectDef {
    pub id: &'static str,
    pub params: Vec<ParamDef>,
}

fn slider(key: &'static str, min: f32, max: f32, step: f32, default: f32) -> ParamDef {
    ParamDef { key, kind: "slider", min, max, step, default: json!(default) }
}
fn angle(key: &'static str, default: f32) -> ParamDef {
    ParamDef { key, kind: "angle", min: 0.0, max: 360.0, step: 1.0, default: json!(default) }
}
fn color(key: &'static str, c: Rgbf) -> ParamDef {
    ParamDef { key, kind: "color", min: 0.0, max: 0.0, step: 0.0, default: json!(c) }
}
fn colors(key: &'static str, cs: &[Rgbf]) -> ParamDef {
    ParamDef { key, kind: "colors", min: 1.0, max: 8.0, step: 0.0, default: json!(cs) }
}
/// A parameter edited by dedicated UI (or not at all): the generic editor skips it.
fn data(key: &'static str, default: Value) -> ParamDef {
    ParamDef { key, kind: "data", min: 0.0, max: 0.0, step: 0.0, default }
}
fn flag(key: &'static str, default: bool) -> ParamDef {
    ParamDef { key, kind: "bool", min: 0.0, max: 1.0, step: 1.0, default: json!(default) }
}

const RED: Rgbf = [255.0, 0.0, 0.0];
const BLUE: Rgbf = [0.0, 80.0, 255.0];
const WHITE: Rgbf = [255.0, 255.0, 255.0];

/// Every effect with its parameters and defaults.
pub fn effect_defs() -> &'static [EffectDef] {
    static DEFS: OnceLock<Vec<EffectDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        vec![
            EffectDef { id: "solid", params: vec![color("color", RED)] },
            EffectDef { id: "paint", params: vec![ParamDef { key: "leds", kind: "paint", min: 0.0, max: 0.0, step: 0.0, default: json!({}) }] },
            EffectDef {
                id: "gradient",
                params: vec![colors("colors", &[RED, BLUE]), angle("angle", 0.0), slider("speed", -1.0, 1.0, 0.01, 0.0)],
            },
            EffectDef { id: "breathing", params: vec![colors("colors", &[RED]), slider("period", 0.5, 12.0, 0.1, 4.0)] },
            EffectDef { id: "spectrum", params: vec![slider("speed", 0.02, 2.0, 0.01, 0.15), slider("saturation", 0.0, 1.0, 0.01, 1.0)] },
            EffectDef {
                id: "rainbow",
                params: vec![slider("speed", -2.0, 2.0, 0.01, 0.25), angle("angle", 0.0), slider("scale", 0.2, 4.0, 0.05, 1.0)],
            },
            EffectDef {
                id: "color_wave",
                params: vec![colors("colors", &[RED, BLUE]), slider("speed", -2.0, 2.0, 0.01, 0.25), angle("angle", 0.0), slider("scale", 0.2, 4.0, 0.05, 1.0)],
            },
            EffectDef {
                id: "starlight",
                params: vec![
                    slider("density", 0.05, 1.0, 0.01, 0.35),
                    slider("duration", 0.2, 6.0, 0.05, 1.5),
                    flag("random", true),
                    colors("colors", &[WHITE]),
                ],
            },
            EffectDef {
                id: "strobe",
                params: vec![colors("colors", &[WHITE]), slider("rate", 0.5, 20.0, 0.1, 4.0), slider("duty", 0.05, 0.95, 0.01, 0.5)],
            },
            EffectDef {
                id: "chase",
                params: vec![
                    colors("colors", &[RED]),
                    slider("speed", 0.05, 3.0, 0.01, 0.6),
                    slider("length", 0.05, 1.0, 0.01, 0.3),
                    slider("gap", 0.0, 1.0, 0.01, 0.4),
                    slider("segment", 0.0, 64.0, 1.0, 0.0),
                    flag("reverse", false),
                ],
            },
            EffectDef {
                id: "frames",
                params: vec![
                    data("frames", json!([])),
                    slider("frame_ms", 20.0, 2000.0, 10.0, 200.0),
                    data("loop", json!("loop")),
                    slider("crossfade", 0.0, 1.0, 0.01, 0.0),
                ],
            },
            EffectDef {
                id: "path",
                params: vec![
                    data("path", json!([])),
                    data("palette", json!([{ "pos": 0.0, "color": RED }, { "pos": 1.0, "color": BLUE }])),
                    slider("speed", 0.05, 3.0, 0.01, 0.5),
                    slider("length", 0.05, 1.0, 0.01, 0.35),
                    slider("gap", 0.0, 1.0, 0.01, 0.3),
                    slider("tail_fade", 0.0, 1.0, 0.01, 0.6),
                    flag("reverse", false),
                ],
            },
            EffectDef {
                id: "palette",
                params: vec![
                    data("cells", json!({})),
                    data("palette", json!([{ "pos": 0.0, "color": RED }, { "pos": 1.0, "color": BLUE }])),
                    slider("speed", -2.0, 2.0, 0.01, 0.3),
                    flag("reverse", false),
                ],
            },
            // Live bar over an ordered LED path (system monitors).
            EffectDef {
                id: "meter",
                params: vec![
                    data("path", json!([])),
                    data("metric", json!("")),
                    slider("min", -50.0, 200.0, 1.0, 0.0),
                    slider("max", -50.0, 200.0, 1.0, 100.0),
                    color("color", [0.0, 220.0, 90.0]),
                    data("style", json!("sweep")),
                    slider("sweep_ms", 0.0, 10000.0, 50.0, 1200.0),
                    data("background", json!("dim")),
                    flag("gradient", false),
                    // Explicit reading in the metric's unit (tests, previews); absent = the live sensor.
                    data("value", Value::Null),
                ],
            },
            // Blinks the path LEDs an exact number of times.
            EffectDef {
                id: "blink",
                params: vec![data("path", json!([])), color("color", RED), slider("count", 1.0, 50.0, 1.0, 5.0), slider("on_ms", 20.0, 2000.0, 10.0, 150.0), slider("off_ms", 20.0, 2000.0, 10.0, 150.0)],
            },
            // A light travelling round a closed path with a fading tail.
            EffectDef {
                id: "orbit",
                params: vec![data("path", json!([])), color("color", WHITE), slider("laps", 1.0, 20.0, 1.0, 1.0), slider("lap_ms", 100.0, 10000.0, 50.0, 800.0), slider("trail", 0.0, 1.0, 0.01, 0.5)],
            },
            // Steps played one after another: `[{ effect, params, duration_ms, mask? }]`.
            EffectDef { id: "sequence", params: vec![data("steps", json!([])), flag("loop", false)] },
            // Plays a library animation (snapshot in the params); rendered by `animations::render_layer`.
            EffectDef { id: "animation", params: vec![data("anim_id", json!("")), data("snapshot", Value::Null)] },
            EffectDef {
                id: "glitch",
                params: vec![
                    colors("colors", &[[0.0, 240.0, 255.0], [255.0, 42.0, 109.0]]),
                    slider("interval", 1.0, 30.0, 0.5, 6.0),
                    slider("duration", 0.05, 1.0, 0.01, 0.25),
                    slider("density", 0.05, 0.9, 0.01, 0.3),
                    slider("size", 0.04, 0.5, 0.01, 0.16),
                ],
            },
            EffectDef {
                id: "ripple",
                params: vec![
                    slider("rate", 0.1, 5.0, 0.05, 0.8),
                    slider("speed", 0.2, 3.0, 0.01, 0.9),
                    slider("width", 0.02, 0.5, 0.01, 0.12),
                    flag("random", true),
                    colors("colors", &[BLUE]),
                ],
            },
        ]
    })
}

pub fn effect_def(id: &str) -> Option<&'static EffectDef> {
    effect_defs().iter().find(|d| d.id == id)
}

/// Params with every missing key replaced by its schema default.
fn resolve(def: &EffectDef, params: &Params) -> Params {
    let mut m = params.0.clone();
    for p in &def.params {
        m.entry(p.key).or_insert_with(|| p.default.clone());
    }
    Params(m)
}

// ---------- deterministic randomness ----------

/// 32-bit mixing hash of `(seed, a, b)`.
pub fn hash(seed: u32, a: u32, b: u32) -> u32 {
    let mut h = seed.wrapping_mul(0x9E37_79B1) ^ a.wrapping_mul(0x85EB_CA6B) ^ b.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^ (h >> 16)
}

/// Uniform value in `[0, 1)` from a hash.
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

// ---------- colour helpers ----------

/// HSV (h in turns, s and v in 0..1) to RGB 0..255.
pub fn hsv(h: f32, s: f32, v: f32) -> Rgbf {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as u32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r * 255.0, g * 255.0, b * 255.0]
}

fn mix(a: Rgbf, b: Rgbf, f: f32) -> Rgbf {
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f]
}

fn scale(c: Rgbf, k: f32) -> Rgbf {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Cyclic palette lookup, `x` in turns.
fn sample_cyclic(pal: &[Rgbf], x: f32) -> Rgbf {
    match pal.len() {
        0 => [0.0; 3],
        1 => pal[0],
        n => {
            let f = x.rem_euclid(1.0) * n as f32;
            let k = (f.floor() as usize).min(n - 1);
            mix(pal[k], pal[(k + 1) % n], f - k as f32)
        }
    }
}

/// Clamped (non-cyclic) palette lookup, `x` in 0..1.
fn sample_clamped(pal: &[Rgbf], x: f32) -> Rgbf {
    match pal.len() {
        0 => [0.0; 3],
        1 => pal[0],
        n => {
            let f = x.clamp(0.0, 1.0) * (n - 1) as f32;
            let k = (f.floor() as usize).min(n - 2);
            mix(pal[k], pal[k + 1], f - k as f32)
        }
    }
}

/// Position projected on a direction (degrees), normalised to 0..1 across the unit square.
fn project(p: Pos, deg: f32) -> f32 {
    let a = deg.to_radians();
    let (s, c) = a.sin_cos();
    let span = c.abs() + s.abs();
    0.5 + ((p[0] - 0.5) * c + (p[1] - 0.5) * s) / span.max(1e-6)
}

fn pos_of(positions: &[Pos], i: usize, n: usize) -> Pos {
    positions.get(i).copied().unwrap_or([i as f32 / n.max(1) as f32, 0.5])
}

// ---------- rendering ----------

/// Renders `effect` for time `t` into `out`. Unknown effects yield transparent pixels.
pub fn render(effect: &str, params: &Params, t: f32, positions: &[Pos], seed: u32, out: &mut [Px]) {
    let Some(def) = effect_def(effect) else {
        out.fill(Px::CLEAR);
        return;
    };
    let p = resolve(def, params);
    let n = out.len();
    match effect {
        "solid" => out.fill(Px::rgb(p.color("color"), 1.0)),
        "paint" => paint(&p, out),
        "gradient" => {
            let pal = p.colors("colors");
            let (ang, speed) = (p.num("angle"), p.num("speed"));
            for (i, o) in out.iter_mut().enumerate() {
                let u = project(pos_of(positions, i, n), ang);
                let c = if speed == 0.0 { sample_clamped(&pal, u) } else { sample_cyclic(&pal, u - t * speed) };
                *o = Px::rgb(c, 1.0);
            }
        }
        "breathing" => {
            let pal = p.colors("colors");
            let period = p.num("period").max(0.05);
            let cycle = t / period;
            let k = cycle.floor();
            let level = 0.5 - 0.5 * (TAU * (cycle - k)).cos();
            let c = if pal.is_empty() { [0.0; 3] } else { pal[(k.rem_euclid(pal.len() as f32)) as usize] };
            out.fill(Px::rgb(scale(c, level), 1.0));
        }
        "spectrum" => out.fill(Px::rgb(hsv(t * p.num("speed"), p.num("saturation"), 1.0), 1.0)),
        "rainbow" => {
            let (speed, ang, sc) = (p.num("speed"), p.num("angle"), p.num("scale"));
            for (i, o) in out.iter_mut().enumerate() {
                let u = project(pos_of(positions, i, n), ang);
                *o = Px::rgb(hsv(u * sc - t * speed, 1.0, 1.0), 1.0);
            }
        }
        "color_wave" => {
            let pal = p.colors("colors");
            let (speed, ang, sc) = (p.num("speed"), p.num("angle"), p.num("scale"));
            for (i, o) in out.iter_mut().enumerate() {
                let u = project(pos_of(positions, i, n), ang);
                *o = Px::rgb(sample_cyclic(&pal, u * sc - t * speed), 1.0);
            }
        }
        "starlight" => starlight(&p, t, seed, out),
        "strobe" => {
            let pal = p.colors("colors");
            let cycle = t * p.num("rate");
            let k = cycle.floor();
            let on = cycle - k < p.num("duty");
            let c = if on && !pal.is_empty() { pal[(k.rem_euclid(pal.len() as f32)) as usize] } else { [0.0; 3] };
            out.fill(Px::rgb(c, 1.0));
        }
        "chase" => chase(&p, t, out),
        "ripple" => ripple(&p, t, positions, seed, out),
        "glitch" => glitch(&p, t, positions, seed, out),
        "frames" => frames(&p, t, out),
        "path" => path(&p, t, out),
        "palette" => palette(&p, t, out),
        "meter" => meter(&p, t, out),
        "blink" => blink(&p, t, out),
        "orbit" => orbit(&p, t, out),
        "sequence" => sequence(&p, t, positions, seed, out),
        _ => out.fill(Px::CLEAR),
    }
}

/// `leds`: `{ "<index>": [r,g,b] }`; anything not listed stays transparent.
fn paint(p: &Params, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    if let Some(map) = p.0.get("leds").and_then(Value::as_object) {
        for (k, v) in map {
            if let (Ok(i), Some(c)) = (k.parse::<usize>(), parse_color(v))
                && let Some(o) = out.get_mut(i)
            {
                *o = Px::rgb(c, 1.0);
            }
        }
    }
}

/// Digital glitch: every `interval` seconds, at a random moment, the keys flicker for `duration`
/// seconds. The area is cut into blocks of about `size`; every 40 ms a different random `density`
/// of them lights up in a palette colour, displaced sideways by a random amount (the
/// "tearing"). Everything else stays transparent, so it sits on top of the scene.
fn glitch(p: &Params, t: f32, positions: &[Pos], seed: u32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let pal = p.colors("colors");
    let interval = p.num("interval").max(0.2);
    let dur = p.num("duration").clamp(0.02, interval);
    let k = (t / interval).floor();
    let local = t - k * interval;
    let start = unit(hash(seed, k as i64 as u32, 0xA11C)) * (interval - dur);
    if pal.is_empty() || local < start || local >= start + dur {
        return;
    }
    let step = ((local - start) / 0.04).floor() as u32;
    let size = p.num("size").clamp(0.03, 1.0);
    let density = p.num("density").clamp(0.0, 1.0);
    let n = out.len();
    for (i, o) in out.iter_mut().enumerate() {
        let pos = pos_of(positions, i, n);
        // Rows tear sideways by a different amount each step.
        let row = (pos[1] / (size * 0.6)).floor() as i32 as u32;
        let shift = (unit(hash(seed ^ 0x51F7, step, row)) - 0.5) * size * 2.0;
        let col = ((pos[0] + shift) / size).floor() as i32 as u32;
        let h = hash(seed ^ 0x6C17, step, col.wrapping_mul(31).wrapping_add(row));
        if unit(h) < density {
            *o = Px::rgb(pal[(h >> 9) as usize % pal.len()], 1.0);
        }
    }
}

/// Every LED flashes once per cycle (`duration / density`) at a random moment of it.
fn starlight(p: &Params, t: f32, seed: u32, out: &mut [Px]) {
    let pal = p.colors("colors");
    let random = p.flag("random") || pal.is_empty();
    let dur = p.num("duration").max(0.05);
    let cycle = dur / p.num("density").clamp(0.01, 1.0);
    for (i, o) in out.iter_mut().enumerate() {
        let i = i as u32;
        let off = unit(hash(seed, i, 0xFFFF)) * cycle;
        let shifted = t + off;
        let k = (shifted / cycle).floor();
        let local = shifted - k * cycle;
        let h = hash(seed, i, k as i64 as u32);
        let start = unit(h) * (cycle - dur);
        *o = Px::CLEAR;
        if local >= start && local < start + dur {
            let level = (PI * (local - start) / dur).sin();
            let c = if random { hsv(unit(h.rotate_left(13)), 1.0, 1.0) } else { pal[(h as usize >> 4) % pal.len()] };
            *o = Px::rgb(c, level);
        }
    }
}

/// Comets running along the LED order (or inside segments of `segment` LEDs, e.g. one per fan).
fn chase(p: &Params, t: f32, out: &mut [Px]) {
    let pal = p.colors("colors");
    let (len, gap, speed) = (p.num("length"), p.num("gap"), p.num("speed"));
    let period = (len + gap).max(0.01);
    let seg = p.num("segment") as usize;
    let dir = if p.flag("reverse") { -1.0 } else { 1.0 };
    let n = out.len();
    for (i, o) in out.iter_mut().enumerate() {
        let (idx, count) = if seg > 0 { (i % seg, seg) } else { (i, n) };
        let s = idx as f32 / count.max(1) as f32;
        let u = s - dir * t * speed;
        let q = u.rem_euclid(period);
        *o = Px::CLEAR;
        if q < len && !pal.is_empty() {
            let k = (u / period).floor().rem_euclid(pal.len() as f32) as usize;
            *o = Px::rgb(pal[k], 1.0 - q / len);
        }
    }
}

/// Colour of a palette at `x`. Cyclic palettes wrap from the last stop back to the first.
fn sample_stops(stops: &[Stop], x: f32, cyclic: bool) -> Rgbf {
    let n = stops.len();
    match n {
        0 => return [0.0; 3],
        1 => return stops[0].color,
        _ => {}
    }
    let (first, last) = (stops[0], stops[n - 1]);
    let x = if cyclic { x.rem_euclid(1.0) } else { x.clamp(0.0, 1.0) };
    let wrap = |from: Stop, to: Stop, at: f32| {
        let span = (to.pos + 1.0 - from.pos).max(1e-6);
        mix(from.color, to.color, ((at - from.pos) / span).clamp(0.0, 1.0))
    };
    if x <= first.pos {
        if !cyclic || x == first.pos {
            return first.color;
        }
        return wrap(last, first, x + 1.0);
    }
    if x >= last.pos {
        return if cyclic { wrap(last, first, x) } else { last.color };
    }
    let k = stops.windows(2).position(|w| x >= w[0].pos && x < w[1].pos).unwrap_or(n - 2);
    let (a, b) = (stops[k], stops[k + 1]);
    mix(a.color, b.color, (x - a.pos) / (b.pos - a.pos).max(1e-6))
}

/// Draws one frame (`{ "leds": { "<index>": [r,g,b] } }`) into `out`; unlisted LEDs are transparent.
fn read_frame(frame: &Value, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    if let Some(map) = frame.get("leds").and_then(Value::as_object) {
        for (k, v) in map {
            if let (Ok(i), Some(c)) = (k.parse::<usize>(), parse_color(v))
                && let Some(o) = out.get_mut(i)
            {
                *o = Px::rgb(c, 1.0);
            }
        }
    }
}

/// Flipbook: frames play in order with optional crossfade. `loop` is `loop`, `pingpong` or `once`
/// (once holds the last frame). LEDs missing from a frame are transparent.
fn frames(p: &Params, t: f32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let Some(list) = p.0.get("frames").and_then(Value::as_array) else { return };
    let n = list.len();
    if n == 0 {
        return;
    }
    let base = p.num("frame_ms").clamp(10.0, 60_000.0);
    let durs: Vec<f32> =
        list.iter().map(|f| f.get("duration_ms").and_then(Value::as_f64).map_or(base, |d| d as f32).clamp(10.0, 600_000.0)).collect();
    let mode = p.text("loop");
    let seq: Vec<usize> = if mode == "pingpong" && n > 1 { (0..n).chain((1..n - 1).rev()).collect() } else { (0..n).collect() };
    let total: f32 = seq.iter().map(|&i| durs[i]).sum();
    let tm = (t * 1000.0).max(0.0);
    let once = mode == "once";
    let (k, local) = if once && tm >= total {
        (seq.len() - 1, durs[seq[seq.len() - 1]])
    } else {
        let mut rest = tm.rem_euclid(total.max(1.0));
        let mut k = 0;
        while k + 1 < seq.len() && rest >= durs[seq[k]] {
            rest -= durs[seq[k]];
            k += 1;
        }
        (k, rest)
    };
    let cur = seq[k];
    read_frame(&list[cur], out);
    let cf = p.num("crossfade").clamp(0.0, 1.0);
    let next = if k + 1 < seq.len() {
        Some(seq[k + 1])
    } else if once {
        None
    } else {
        Some(seq[0])
    };
    let Some(next) = next else { return };
    if cf <= 0.0 || next == cur {
        return;
    }
    let f = (((local / durs[cur]) - (1.0 - cf)) / cf).clamp(0.0, 1.0);
    if f <= 0.0 {
        return;
    }
    let mut b = vec![Px::CLEAR; out.len()];
    read_frame(&list[next], &mut b);
    for (a, b) in out.iter_mut().zip(&b) {
        *a = match (a.a > 0.0, b.a > 0.0) {
            (true, true) => Px::rgb(mix([a.r, a.g, a.b], [b.r, b.g, b.b], f), 1.0),
            (true, false) => Px { a: 1.0 - f, ..*a },
            (false, true) => Px { a: f, ..*b },
            (false, false) => Px::CLEAR,
        };
    }
}

/// A gradient segment travelling along a drawn path (`path`: LED indices in stroke order).
/// `length`/`gap` are fractions of the path, `tail_fade` fades the segment towards its tail.
fn path(p: &Params, t: f32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let Some(list) = p.0.get("path").and_then(Value::as_array) else { return };
    let m = list.len();
    let stops = p.stops("palette");
    if m == 0 || stops.is_empty() {
        return;
    }
    let (len, gap, speed, tail) = (p.num("length").clamp(0.02, 1.0), p.num("gap").max(0.0), p.num("speed"), p.num("tail_fade").clamp(0.0, 1.0));
    let period = (len + gap).max(0.02);
    let dir = if p.flag("reverse") { -1.0 } else { 1.0 };
    for (k, v) in list.iter().enumerate() {
        let Some(o) = v.as_u64().and_then(|i| out.get_mut(i as usize)) else { continue };
        let s = (k as f32 + 0.5) / m as f32;
        let q = (s - dir * t * speed).rem_euclid(period);
        if q < len {
            let x = q / len;
            *o = Px::rgb(sample_stops(&stops, x, false), 1.0 - tail * x);
        }
    }
}

/// Palette cycling: every cell (`cells`: `{ "<index>": position 0..1 }`) shows the palette at
/// `position + t * speed`, wrapping around.
fn palette(p: &Params, t: f32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let Some(cells) = p.0.get("cells").and_then(Value::as_object) else { return };
    let stops = p.stops("palette");
    let dir = if p.flag("reverse") { -1.0 } else { 1.0 };
    let shift = dir * t * p.num("speed");
    for (k, v) in cells {
        if let (Ok(i), Some(pos)) = (k.parse::<usize>(), v.as_f64())
            && let Some(o) = out.get_mut(i)
        {
            *o = Px::rgb(sample_stops(&stops, pos as f32 + shift, true), 1.0);
        }
    }
}

/// LED indices of the `path` parameter, in order; an empty path means every LED of `n`.
fn path_leds(p: &Params, n: usize) -> Vec<usize> {
    if p.0.get("path").and_then(Value::as_array).is_none_or(Vec::is_empty) { (0..n).collect() } else { valid_path(p, n) }
}

/// The `path` indices that exist on a device of `n` LEDs.
fn valid_path(p: &Params, n: usize) -> Vec<usize> {
    p.0.get("path").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|i| i as usize).filter(|&i| i < n).collect()).unwrap_or_default()
}

const GRADIENT: [Rgbf; 3] = [[0.0, 220.0, 90.0], [255.0, 200.0, 0.0], [255.0, 40.0, 40.0]];

/// A bar over the `path` LEDs filled to `(value - min) / (max - min)`, where the value is the
/// explicit `value` parameter or the live sensor named by `metric`. `style` `sweep` grows the bar
/// from empty over `sweep_ms`, `live` shows it at once. The last, partly filled LED is dimmed in
/// proportion, so the bar reads exactly. `background`: `dim` (default), `off` (black) or `none`.
fn meter(p: &Params, t: f32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let list = valid_path(p, out.len());
    let m = list.len();
    if m == 0 {
        return;
    }
    let raw = p.0.get("value").and_then(Value::as_f64).map(|v| v as f32).or_else(|| crate::sysmon::live(p.text("metric")));
    let (lo, hi) = (p.num("min"), p.num("max"));
    let mut frac = raw.map_or(0.0, |v| ((v - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0));
    if p.text("style") != "live" {
        let sweep = p.num("sweep_ms");
        if sweep > 0.0 {
            let x = (t * 1000.0 / sweep).clamp(0.0, 1.0);
            frac *= 1.0 - (1.0 - x).powi(3);
        }
    }
    let fill = frac * m as f32;
    let base = p.color("color");
    let bg = match p.text("background") {
        "off" => Some(0.0),
        "none" => None,
        _ => Some(0.12),
    };
    for (k, &led) in list.iter().enumerate() {
        let cover = (fill - k as f32).clamp(0.0, 1.0);
        let color = if p.flag("gradient") { sample_clamped(&GRADIENT, (k as f32 + 0.5) / m as f32) } else { base };
        out[led] = match (cover > 0.0, bg) {
            (true, _) => Px::rgb(scale(color, cover.max(bg.unwrap_or(0.0))), 1.0),
            (false, Some(b)) => Px::rgb(scale(color, b), 1.0),
            (false, None) => Px::CLEAR,
        };
    }
}

/// The path LEDs light up for `on_ms`, go dark for `off_ms`, `count` times; then nothing.
fn blink(p: &Params, t: f32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let (on, off) = (p.num("on_ms").max(1.0), p.num("off_ms").max(0.0));
    let tm = t * 1000.0;
    let cycle = on + off;
    let k = (tm / cycle).floor();
    if tm < 0.0 || k >= p.num("count").max(1.0) || tm - k * cycle >= on {
        return;
    }
    let c = p.color("color");
    for led in path_leds(p, out.len()) {
        out[led] = Px::rgb(c, 1.0);
    }
}

/// A bright head runs round the path (`laps` times, `lap_ms` each) with a tail of `trail` of the loop.
fn orbit(p: &Params, t: f32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let list = path_leds(p, out.len());
    let m = list.len();
    let lap = p.num("lap_ms").max(10.0);
    let tm = t * 1000.0;
    if m == 0 || tm < 0.0 || tm >= p.num("laps").max(1.0) * lap {
        return;
    }
    let head = tm / lap * m as f32;
    let tail = (p.num("trail").clamp(0.0, 1.0) * m as f32).max(1.0);
    let c = p.color("color");
    for (k, &led) in list.iter().enumerate() {
        let behind = (head - k as f32).rem_euclid(m as f32);
        if behind < tail {
            out[led] = Px::rgb(scale(c, 1.0 - behind / tail), 1.0);
        }
    }
}

/// Plays `steps` in order (looping when `loop`); each step is an effect with its own duration
/// and time (starting at 0), optionally limited to the LEDs of `mask`.
fn sequence(p: &Params, t: f32, positions: &[Pos], seed: u32, out: &mut [Px]) {
    out.fill(Px::CLEAR);
    let Some(steps) = p.0.get("steps").and_then(Value::as_array) else { return };
    let dur = |s: &Value| s.get("duration_ms").and_then(Value::as_f64).unwrap_or(1000.0).clamp(10.0, 3_600_000.0) as f32;
    let total: f32 = steps.iter().map(dur).sum();
    if steps.is_empty() || t < 0.0 {
        return;
    }
    let mut tm = t * 1000.0;
    if p.flag("loop") {
        tm = tm.rem_euclid(total);
    } else if tm >= total {
        return;
    }
    for s in steps {
        let d = dur(s);
        if tm >= d {
            tm -= d;
            continue;
        }
        let effect = s.get("effect").and_then(Value::as_str).unwrap_or("");
        if effect != "sequence" && effect != "animation" {
            let sp = Params(s.get("params").and_then(Value::as_object).cloned().unwrap_or_default());
            render(effect, &sp, tm / 1000.0, positions, seed, out);
            if let Some(mask) = s.get("mask").and_then(Value::as_array) {
                let keep: Vec<usize> = mask.iter().filter_map(Value::as_u64).map(|i| i as usize).collect();
                for (i, o) in out.iter_mut().enumerate() {
                    if !keep.contains(&i) {
                        *o = Px::CLEAR;
                    }
                }
            }
        }
        return;
    }
}

/// Rings expanding from random LEDs.
fn ripple(p: &Params, t: f32, positions: &[Pos], seed: u32, out: &mut [Px]) {
    let pal = p.colors("colors");
    let random = p.flag("random") || pal.is_empty();
    // Clamped: profiles come from files and could otherwise ask for millions of ripples.
    let (rate, speed, width) = (p.num("rate").clamp(0.01, 20.0), p.num("speed").clamp(0.05, 20.0), p.num("width").max(0.01));
    let life = 1.6 / speed;
    let n = out.len();
    out.fill(Px::CLEAR);
    if n == 0 {
        return;
    }
    let newest = (t * rate).floor() as i64;
    let oldest = ((t - life) * rate).floor() as i64;
    for j in oldest.max(0)..=newest {
        let h = hash(seed, j as u32, 0xABCD);
        let birth = (j as f32 + unit(h) * 0.5) / rate;
        let age = t - birth;
        if !(0.0..life).contains(&age) {
            continue;
        }
        let origin = pos_of(positions, h as usize % n, n);
        let radius = age * speed;
        let fade = 1.0 - age / life;
        let col = if random { hsv(unit(h.rotate_left(7)), 1.0, 1.0) } else { pal[(h as usize >> 3) % pal.len()] };
        for (i, o) in out.iter_mut().enumerate() {
            let pp = pos_of(positions, i, n);
            let d = ((pp[0] - origin[0]).powi(2) + (pp[1] - origin[1]).powi(2)).sqrt();
            let k = (-((d - radius) / width).powi(2)).exp() * fade;
            if k > o.a {
                *o = Px::rgb(col, k);
            }
        }
    }
}

/// Test helper: effect params from a JSON literal.
#[cfg(test)]
pub fn params(v: Value) -> Params {
    Params(v.as_object().cloned().unwrap_or_default())
}

/// Default params of an effect (every key at its schema value).
pub fn default_params(effect: &str) -> Params {
    effect_def(effect).map(|d| resolve(d, &Params::default())).unwrap_or_default()
}

/// All schema defaults, by effect id (handy for the frontend).
pub fn all_defaults() -> HashMap<&'static str, Params> {
    effect_defs().iter().map(|d| (d.id, default_params(d.id))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize) -> Vec<Pos> {
        (0..n).map(|i| [i as f32 / (n - 1).max(1) as f32, 0.5]).collect()
    }
    fn run(effect: &str, p: Value, t: f32, n: usize) -> Vec<Px> {
        let mut out = vec![Px::default(); n];
        render(effect, &params(p), t, &line(n), 42, &mut out);
        out
    }

    #[test]
    fn every_effect_is_deterministic_and_in_bounds() {
        for def in effect_defs() {
            for &t in &[0.0, 0.37, 5.0, 123.456] {
                let a = run(def.id, json!({}), t, 30);
                let b = run(def.id, json!({}), t, 30);
                assert_eq!(a, b, "{} not deterministic", def.id);
                for px in &a {
                    for ch in [px.r, px.g, px.b] {
                        assert!((-0.01..=255.01).contains(&ch), "{} channel {ch} out of range at t={t}", def.id);
                    }
                    assert!((0.0..=1.0001).contains(&px.a), "{} alpha {}", def.id, px.a);
                }
            }
        }
    }

    #[test]
    fn seed_changes_random_effects() {
        let mut a = vec![Px::default(); 40];
        let mut b = vec![Px::default(); 40];
        let pos = line(40);
        render("starlight", &Params::default(), 3.3, &pos, 1, &mut a);
        render("starlight", &Params::default(), 3.3, &pos, 2, &mut b);
        assert_ne!(a, b);
    }

    fn lit(out: &[Px]) -> Vec<usize> {
        out.iter().enumerate().filter(|(_, p)| p.a > 0.0 && (p.r + p.g + p.b) > 40.0).map(|(i, _)| i).collect()
    }

    #[test]
    fn meter_fills_in_path_order_with_a_partial_last_led() {
        // 10 LEDs, path in reverse order; 35 % -> 3.5 LEDs: path[0..3] full, path[3] half.
        let path: Vec<usize> = (0..10).rev().collect();
        let out = run("meter", json!({"path": path, "value": 35.0, "color": [200, 100, 0], "style": "live", "background": "off"}), 0.0, 10);
        assert_eq!((out[9].r, out[9].g), (200.0, 100.0));
        assert_eq!(out[7].r, 200.0);
        assert!((out[6].r - 100.0).abs() < 0.01, "partial LED {}", out[6].r);
        assert_eq!(out[5], Px { r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
        assert_eq!(lit(&out), vec![6, 7, 8, 9]);
    }

    #[test]
    fn meter_sweep_grows_then_holds_and_scales_by_min_max() {
        let p = |t: f32| run("meter", json!({"path": [0, 1, 2, 3], "value": 80.0, "min": 30, "max": 90, "sweep_ms": 1000, "background": "none", "color": [255, 0, 0]}), t, 4);
        // (80-30)/60 = 0.8333 -> 3.33 LEDs at the end of the sweep.
        assert!(lit(&p(0.0)).is_empty());
        let mid = lit(&p(0.2)).len();
        let end = p(1.0);
        assert!(mid < 4, "still growing at 0.2 s");
        assert_eq!(lit(&end), vec![0, 1, 2, 3]);
        assert!((end[3].r - 255.0 / 3.0).abs() < 1.0, "a third of the last LED");
        assert_eq!(p(5.0), end);
        // Unlit LEDs of a `none` background stay transparent.
        assert_eq!(end[3].a, 1.0);
        let full = run("meter", json!({"path": [0, 1], "value": 10.0, "min": 0, "max": 100, "style": "live", "background": "none"}), 0.0, 3);
        assert_eq!(full[1].a, 0.0);
        assert_eq!(full[2].a, 0.0);
    }

    #[test]
    fn meter_without_a_reading_shows_an_empty_bar_and_gradient_follows_position() {
        let out = run("meter", json!({"path": [0, 1, 2], "metric": "", "background": "dim", "color": [100, 100, 100]}), 9.0, 3);
        assert!(out.iter().all(|p| p.r == 12.0 && p.a == 1.0));
        let g = run("meter", json!({"path": [0, 1, 2, 3, 4], "value": 100.0, "style": "live", "gradient": true}), 0.0, 5);
        assert!(g[0].g > g[0].r && g[4].r > g[4].g, "green at the start, red at the end");
        // Paths pointing past the device are ignored.
        assert!(lit(&run("meter", json!({"path": [7, 8], "value": 100.0, "style": "live"}), 0.0, 3)).is_empty());
    }

    #[test]
    fn blink_flashes_exactly_count_times() {
        let p = json!({"path": [1, 2], "color": [0, 0, 255], "count": 3, "on_ms": 100, "off_ms": 100});
        let on = |t: f32| !lit(&run("blink", p.clone(), t, 4)).is_empty();
        let pattern: Vec<bool> = (0..8).map(|k| on(k as f32 * 0.1 + 0.05)).collect();
        assert_eq!(pattern, vec![true, false, true, false, true, false, false, false]);
        assert_eq!(lit(&run("blink", p, 0.05, 4)), vec![1, 2]);
        // No path = the whole device.
        assert_eq!(lit(&run("blink", json!({"color": [0, 0, 255]}), 0.01, 3)), vec![0, 1, 2]);
    }

    #[test]
    fn orbit_runs_round_the_path_laps_times_and_ends() {
        let p = json!({"path": [3, 4, 5, 6], "color": [255, 255, 255], "laps": 2, "lap_ms": 1000, "trail": 0.25});
        // trail 0.25 of 4 LEDs = 1 LED: only the head is lit.
        let head = |t: f32| lit(&run("orbit", p.clone(), t, 8));
        assert_eq!(head(0.1), vec![3]);
        assert_eq!(head(0.35), vec![4]);
        assert_eq!(head(0.6), vec![5]);
        assert_eq!(head(0.85), vec![6]);
        assert_eq!(head(1.1), vec![3], "second lap");
        assert!(head(2.0).is_empty(), "off after the last lap");
        let tail = run("orbit", json!({"path": [0, 1, 2, 3], "laps": 1, "lap_ms": 1000, "trail": 1.0}), 0.6, 4);
        assert!(tail[2].r > tail[1].r && tail[1].r > tail[0].r, "fades behind the head: {tail:?}");
    }

    #[test]
    fn sequence_plays_steps_in_order_loops_and_masks() {
        let steps = json!([
            {"effect": "solid", "params": {"color": [255, 0, 0]}, "duration_ms": 1000, "mask": [0]},
            {"effect": "solid", "params": {"color": [0, 255, 0]}, "duration_ms": 500}
        ]);
        let at = |t: f32, looped: bool| run("sequence", json!({"steps": steps, "loop": looped}), t, 2);
        let a = at(0.5, false);
        assert_eq!((a[0].r, a[1].a), (255.0, 0.0), "step 1 only on LED 0");
        let b = at(1.2, false);
        assert_eq!((b[0].g, b[1].g), (255.0, 255.0));
        assert!(at(1.6, false).iter().all(|p| p.a == 0.0), "ends");
        assert_eq!(at(1.6, true)[0].r, 255.0, "loops back to step 1");
        assert_eq!(at(3.2, true), at(0.2, true));
    }

    #[test]
    fn solid_is_exact() {
        let out = run("solid", json!({"color": [10, 20, 30]}), 9.0, 3);
        assert!(out.iter().all(|p| *p == Px { r: 10.0, g: 20.0, b: 30.0, a: 1.0 }));
    }

    #[test]
    fn paint_is_sparse() {
        let out = run("paint", json!({"leds": {"1": [255, 0, 0], "9": [1, 2, 3]}}), 0.0, 4);
        assert_eq!(out[0].a, 0.0);
        assert_eq!(out[1], Px { r: 255.0, g: 0.0, b: 0.0, a: 1.0 });
        assert_eq!(out[2].a, 0.0);
        assert_eq!(out.len(), 4); // index 9 ignored
    }

    #[test]
    fn gradient_static_endpoints() {
        let out = run("gradient", json!({"colors": [[0, 0, 0], [200, 100, 0]], "angle": 0, "speed": 0}), 0.0, 5);
        assert!(out[0].r < 1.0);
        assert!((out[4].r - 200.0).abs() < 1.0);
        assert!((out[2].g - 50.0).abs() < 1.0);
    }

    #[test]
    fn breathing_peaks_mid_period() {
        let off = run("breathing", json!({"colors": [[100, 0, 0]], "period": 4.0}), 0.0, 1)[0];
        let peak = run("breathing", json!({"colors": [[100, 0, 0]], "period": 4.0}), 2.0, 1)[0];
        assert!(off.r < 0.01);
        assert!((peak.r - 100.0).abs() < 0.01);
    }

    #[test]
    fn spectrum_hues() {
        // speed 1 turn/s: t = 0 red, t = 1/3 green.
        let red = run("spectrum", json!({"speed": 1.0}), 0.0, 1)[0];
        let green = run("spectrum", json!({"speed": 1.0}), 1.0 / 3.0, 1)[0];
        assert!(red.r > 254.0 && red.g < 1.0);
        assert!(green.g > 254.0 && green.r < 1.0);
    }

    #[test]
    fn rainbow_wave_moves() {
        let a = run("rainbow", json!({"speed": 0.5}), 0.0, 10);
        let b = run("rainbow", json!({"speed": 0.5}), 0.5, 10);
        assert_ne!(a, b);
        // full period brings it back.
        let c = run("rainbow", json!({"speed": 0.5}), 2.0, 10);
        for (x, y) in a.iter().zip(&c) {
            assert!((x.r - y.r).abs() < 0.5 && (x.g - y.g).abs() < 0.5 && (x.b - y.b).abs() < 0.5);
        }
    }

    #[test]
    fn strobe_duty() {
        let p = json!({"colors": [[255, 255, 255]], "rate": 1.0, "duty": 0.25});
        assert_eq!(run("strobe", p.clone(), 0.1, 1)[0].r, 255.0);
        assert_eq!(run("strobe", p, 0.5, 1)[0].r, 0.0);
    }

    #[test]
    fn chase_head_moves_and_reverses() {
        let p = json!({"colors": [[255, 0, 0]], "speed": 0.5, "length": 0.2, "gap": 1.0, "segment": 0});
        let lit = |v: &[Px]| v.iter().position(|p| p.a > 0.9).unwrap();
        let a = lit(&run("chase", p.clone(), 0.0, 20));
        let b = lit(&run("chase", p, 0.4, 20));
        assert_eq!(a, 0);
        assert!(b > a);
        let r = json!({"colors": [[255, 0, 0]], "speed": 0.5, "length": 0.2, "gap": 0.5, "reverse": true});
        let c = run("chase", r, 0.4, 20);
        // reversed: head moved backwards, wraps near the end.
        assert!(c.iter().rposition(|p| p.a > 0.0).unwrap() >= 10);
    }

    #[test]
    fn chase_segments_repeat_per_fan() {
        let p = json!({"colors": [[255, 0, 0]], "speed": 0.1, "length": 0.25, "gap": 0.75, "segment": 8});
        let out = run("chase", p, 0.0, 24);
        for fan in 0..3 {
            assert!(out[fan * 8].a > 0.9, "fan {fan} head");
        }
    }

    #[test]
    fn ripple_expands() {
        let pos = line(50);
        let p = params(json!({"rate": 4.0, "speed": 1.0, "width": 0.1, "random": false, "colors": [[0, 0, 255]]}));
        let mut early = vec![Px::default(); 50];
        let mut late = vec![Px::default(); 50];
        render("ripple", &p, 0.5, &pos, 5, &mut early);
        render("ripple", &p, 1.2, &pos, 5, &mut late);
        assert!(early.iter().any(|p| p.a > 0.2));
        assert!(early.iter().all(|p| p.a == 0.0 || (p.b - 255.0).abs() < 1.0));
        assert_ne!(early, late);
    }

    #[test]
    fn frames_play_in_order_and_loop() {
        let f = json!({"frames": [
            {"leds": {"0": [255, 0, 0]}}, {"leds": {"1": [0, 255, 0]}}, {"leds": {"2": [0, 0, 255]}}
        ], "frame_ms": 100, "loop": "loop"});
        let at = |t: f32| run("frames", f.clone(), t, 3);
        assert_eq!(at(0.05)[0].r, 255.0);
        assert_eq!(at(0.05)[1].a, 0.0, "LEDs missing from a frame are transparent");
        assert_eq!(at(0.15)[1].g, 255.0);
        assert_eq!(at(0.25)[2].b, 255.0);
        assert_eq!(at(0.35)[0].r, 255.0, "loops back to frame 0");
    }

    #[test]
    fn frames_pingpong_and_once() {
        let mk = |mode: &str| json!({"frames": [{"leds": {"0": [1, 1, 1]}}, {"leds": {"1": [1, 1, 1]}}, {"leds": {"2": [1, 1, 1]}}], "frame_ms": 100, "loop": mode});
        let lit = |mode: &str, t: f32| run("frames", mk(mode), t, 3).iter().position(|p| p.a > 0.0).unwrap();
        // 0 1 2 1 0 1 2 ...
        let seq: Vec<usize> = [0.05, 0.15, 0.25, 0.35, 0.45, 0.55].iter().map(|&t| lit("pingpong", t)).collect();
        assert_eq!(seq, vec![0, 1, 2, 1, 0, 1]);
        assert_eq!(lit("once", 0.05), 0);
        assert_eq!(lit("once", 9.0), 2, "once holds the last frame");
    }

    #[test]
    fn frames_per_frame_duration_and_crossfade() {
        let f = json!({"frames": [
            {"leds": {"0": [200, 0, 0]}, "duration_ms": 400}, {"leds": {"0": [0, 0, 200], "1": [0, 200, 0]}, "duration_ms": 100}
        ], "loop": "loop", "crossfade": 1.0});
        // Frame 0 lasts 400 ms: at 200 ms we are half-way to frame 1 (crossfade over the whole frame).
        let mid = run("frames", f.clone(), 0.2, 2);
        assert!((mid[0].r - 100.0).abs() < 1.0 && (mid[0].b - 100.0).abs() < 1.0, "{:?}", mid[0]);
        assert!((mid[1].a - 0.5).abs() < 0.01, "LED only in the next frame fades in");
        let no_fade = json!({"frames": f["frames"].clone(), "loop": "loop", "crossfade": 0.0});
        assert_eq!(run("frames", no_fade, 0.2, 2)[0].r, 200.0);
    }

    #[test]
    fn frames_empty_is_transparent() {
        assert!(run("frames", json!({"frames": []}), 1.0, 4).iter().all(|p| p.a == 0.0));
    }

    #[test]
    fn path_segment_travels_in_stroke_order() {
        let pal = json!([{"pos": 0.0, "color": [255, 255, 255]}, {"pos": 1.0, "color": [255, 255, 255]}]);
        let p = json!({"path": [3, 1, 0, 2], "palette": pal, "speed": 1.0, "length": 0.25, "gap": 0.75, "tail_fade": 0.0});
        let lit = |t: f32| run("path", p.clone(), t, 5).iter().position(|px| px.a > 0.0);
        // One LED of the 4-LED path is lit at a time; the head moves through the stroke order 3, 1, 0, 2.
        let order: Vec<_> = [0.03f32, 0.28, 0.53, 0.78].iter().map(|&t| lit(t)).collect();
        assert_eq!(order, vec![Some(3), Some(1), Some(0), Some(2)]);
        assert_eq!(run("path", p, 0.1, 5)[4].a, 0.0, "LEDs outside the path stay transparent");
    }

    #[test]
    fn path_reverse_and_tail_fade() {
        let pal = json!([{"pos": 0.0, "color": [10, 20, 30]}]);
        let base = json!({"path": [0, 1, 2, 3, 4, 5, 6, 7], "palette": pal, "speed": 0.0, "length": 0.5, "gap": 0.5, "tail_fade": 1.0});
        let out = run("path", base.clone(), 0.0, 8);
        // Head (q = 0) opaque, tail fading to transparent.
        let alphas: Vec<f32> = out.iter().map(|p| p.a).collect();
        assert!(alphas[0] > alphas[1] && alphas[1] > alphas[2] && alphas[3] > 0.0 && alphas[4] == 0.0, "{alphas:?}");
        let mut fwd = base.clone();
        fwd["speed"] = json!(1.0);
        let mut rev = fwd.clone();
        rev["reverse"] = json!(true);
        assert_ne!(run("path", fwd, 0.3, 8), run("path", rev, 0.3, 8));
    }

    #[test]
    fn palette_cycles_without_moving() {
        let pal = json!([{"pos": 0.0, "color": [255, 0, 0]}, {"pos": 0.5, "color": [0, 255, 0]}, {"pos": 1.0, "color": [255, 0, 0]}]);
        let p = json!({"cells": {"0": 0.0, "1": 0.5}, "palette": pal, "speed": 1.0});
        let a = run("palette", p.clone(), 0.0, 3);
        assert_eq!((a[0].r, a[0].g), (255.0, 0.0));
        assert_eq!((a[1].r, a[1].g), (0.0, 255.0));
        assert_eq!(a[2].a, 0.0, "LEDs without a cell stay transparent");
        // Half a cycle later the colours swapped places; a full cycle brings them back.
        let b = run("palette", p.clone(), 0.5, 3);
        assert_eq!((b[0].r, b[0].g), (0.0, 255.0));
        let c = run("palette", p.clone(), 1.0, 3);
        assert!((c[0].r - 255.0).abs() < 0.01 && c[0].g.abs() < 0.01);
        let mut rev = p.clone();
        rev["reverse"] = json!(true);
        assert_ne!(run("palette", rev, 0.1, 3)[0], run("palette", p, 0.1, 3)[0]);
    }

    #[test]
    fn palette_sampling_wraps_between_last_and_first() {
        let stops = [Stop { pos: 0.2, color: [0.0, 0.0, 0.0] }, Stop { pos: 0.8, color: [60.0, 0.0, 0.0] }];
        assert_eq!(sample_stops(&stops, 0.0, false), [0.0; 3]);
        assert_eq!(sample_stops(&stops, 1.0, false)[0], 60.0);
        // Cyclic: from 0.8 (60) through 1.0/0.0 back to 0.2 (0): 1.0 is half-way.
        assert!((sample_stops(&stops, 1.0, true)[0] - 30.0).abs() < 0.01);
        assert!((sample_stops(&stops, 0.5, true)[0] - 30.0).abs() < 0.01);
        assert_eq!(sample_stops(&[], 0.5, true), [0.0; 3]);
    }

    #[test]
    fn unknown_effect_is_transparent() {
        assert!(run("nope", json!({}), 1.0, 3).iter().all(|p| p.a == 0.0));
    }

    #[test]
    fn schema_defaults_are_complete() {
        for def in effect_defs() {
            let d = default_params(def.id);
            for p in &def.params {
                assert!(d.0.contains_key(p.key));
            }
        }
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash(1, 2, 3), hash(1, 2, 3));
        assert_ne!(hash(1, 2, 3), hash(1, 3, 2));
        assert!((0.0..1.0).contains(&unit(hash(9, 9, 9))));
    }

    #[test]
    fn glitch_is_deterministic_sparse_in_time_and_transparent_elsewhere() {
        let n = 40;
        let params = json!({"interval": 4.0, "duration": 0.3, "density": 0.5, "size": 0.1, "colors": [[0, 240, 255], [255, 42, 109]]});
        let mut active = 0;
        let mut total = 0;
        let mut t = 0.0;
        while t < 40.0 {
            let a = run("glitch", params.clone(), t, n);
            assert_eq!(a, run("glitch", params.clone(), t, n), "deterministic at t={t}");
            total += 1;
            if a.iter().any(|p| p.a > 0.0) {
                active += 1;
                // Only palette colours are ever drawn.
                for px in a.iter().filter(|p| p.a > 0.0) {
                    assert!([[0.0, 240.0, 255.0], [255.0, 42.0, 109.0]].contains(&[px.r, px.g, px.b]));
                }
            }
            t += 0.02;
        }
        // 0.3 s of every 4 s: about 7.5 % of the time, never all of it and never none.
        let share = active as f32 / total as f32;
        assert!((0.03..0.15).contains(&share), "glitch visible {share:.3} of the time");
        // A glitch happens once in every interval.
        for k in 0..10 {
            let found = (0..200).any(|i| run("glitch", params.clone(), k as f32 * 4.0 + i as f32 * 0.02, n).iter().any(|p| p.a > 0.0));
            assert!(found, "interval {k} had no glitch");
        }
        assert!(run("glitch", json!({"colors": []}), 3.0, n).iter().all(|p| p.a == 0.0));
    }
}
