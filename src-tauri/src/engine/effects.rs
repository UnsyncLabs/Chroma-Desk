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
    fn colors(&self, k: &str) -> Vec<Rgbf> {
        self.0
            .get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(parse_color).collect())
            .unwrap_or_default()
    }
}

fn parse_color(v: &Value) -> Option<Rgbf> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32])
}

// ---------- schema ----------

#[derive(Debug, Clone, Serialize)]
pub struct ParamDef {
    pub key: &'static str,
    /// `slider`, `angle`, `color`, `colors` or `bool`.
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

/// Rings expanding from random LEDs.
fn ripple(p: &Params, t: f32, positions: &[Pos], seed: u32, out: &mut [Px]) {
    let pal = p.colors("colors");
    let random = p.flag("random") || pal.is_empty();
    let (rate, speed, width) = (p.num("rate").max(0.01), p.num("speed").max(0.01), p.num("width").max(0.01));
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
}
