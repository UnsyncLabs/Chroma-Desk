//! Animation library: reusable, portable animations stored as JSON files in
//! `~/.config/chroma-desk/animations`.
//!
//! An animation refers to LEDs by *name* (`name:Key: W`) when the LED has a unique name and by
//! index (`idx:41`) otherwise, so the same file works on another keyboard. When it is loaded on a
//! device the references are resolved to indices (unknown ones are ignored) and rendered by the
//! index-based effects (`frames`, `path`, `palette`) of `engine::effects`.
//!
//! Built-in presets ship in the binary (`builtins`), are read-only and can be duplicated.

use crate::config::config_dir;
use crate::engine::compositor::{Geometry, Role};
use crate::engine::effects::{self, Params, Px};
use crate::error::{ApiError, Res};
use crate::fsutil::write_atomic;
use crate::themes::{Color, Palette};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

pub const SCHEMA_VERSION: u32 = 1;
/// Largest animation file we are willing to read.
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_FRAMES: usize = 256;
pub const MAX_LEDS: usize = 4096;
pub const MAX_STOPS: usize = 32;
pub const MAX_STEPS: usize = 64;
const MAX_TAGS: usize = 16;

pub type Rgb3 = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Keyboard,
    Linear,
    #[default]
    Any,
}

/// What kind of device an animation was made for (a hint, not a restriction).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub kind: TargetKind,
    /// Device the animation was drawn on (e.g. "Razer Huntsman V3 Pro TKL").
    #[serde(default)]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LoopMode {
    #[default]
    #[serde(rename = "loop")]
    Loop,
    #[serde(rename = "pingpong")]
    PingPong,
    #[serde(rename = "once")]
    Once,
}

impl LoopMode {
    fn as_str(self) -> &'static str {
        match self {
            LoopMode::Loop => "loop",
            LoopMode::PingPong => "pingpong",
            LoopMode::Once => "once",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// `LedRef` -> colour; LEDs not listed are transparent.
    #[serde(default)]
    pub leds: BTreeMap<String, Color>,
    /// Overrides the animation's default frame time.
    #[serde(default)]
    pub duration_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub pos: f32,
    pub color: Color,
}

fn d_frame_ms() -> u32 {
    200
}
fn d_speed_path() -> f32 {
    0.5
}
fn d_speed_palette() -> f32 {
    0.3
}
fn d_length() -> f32 {
    0.35
}
fn d_gap() -> f32 {
    0.3
}
fn d_tail() -> f32 {
    0.6
}

/// One step of a [`AnimKind::Sequence`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// `meter`, `blink`, `orbit`, `hold` (nothing), `animation` (a library animation) or any
    /// other effect id (`solid`, `strobe`...).
    pub effect: String,
    #[serde(default)]
    pub params: Params,
    /// Ordered `LedRef`s: the path of `meter` / `blink` / `orbit`, the LEDs the other effects are
    /// limited to. Empty = the whole device (a meter needs keys).
    #[serde(default)]
    pub keys: Vec<String>,
    pub duration_ms: u32,
    /// `animation` steps: the animation to play (a copy, so it keeps working if the library changes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anim: Option<Box<Animation>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnimKind {
    /// A preset of one of the engine's effects.
    Effect {
        effect: String,
        #[serde(default)]
        params: Params,
    },
    /// Flipbook.
    Frames {
        frames: Vec<Frame>,
        #[serde(default = "d_frame_ms")]
        frame_ms: u32,
        #[serde(default, rename = "loop")]
        loop_mode: LoopMode,
        #[serde(default)]
        crossfade: f32,
    },
    /// A gradient travelling along a drawn path.
    Path {
        path: Vec<String>,
        palette: Vec<Stop>,
        #[serde(default = "d_speed_path")]
        speed: f32,
        #[serde(default = "d_length")]
        length: f32,
        #[serde(default = "d_gap")]
        gap: f32,
        #[serde(default)]
        reverse: bool,
        #[serde(default = "d_tail")]
        tail_fade: f32,
    },
    /// Steps played one after another (system meters, chat notifications...).
    Sequence {
        steps: Vec<Step>,
        #[serde(default, rename = "loop")]
        looped: bool,
    },
    /// A shape whose cells cycle through a palette.
    Palette {
        /// `LedRef` -> position 0..1 in the palette.
        cells: BTreeMap<String, f32>,
        palette: Vec<Stop>,
        #[serde(default = "d_speed_palette")]
        speed: f32,
        #[serde(default)]
        reverse: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Animation {
    #[serde(default = "d_schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unix milliseconds.
    #[serde(default)]
    pub created: u64,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub target: Target,
    /// Built-in (read-only) preset; never stored in files.
    #[serde(default)]
    pub builtin: bool,
    /// Quick-tab role: a base look ("theme", one at a time) or an overlay ("effect", stackable).
    /// Absent in older files; `normalize` infers it from the kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    pub kind: AnimKind,
}

/// Role of an animation that does not state one: sequences and overlay-like engine effects (or
/// anything tagged `notification`) are effects, the rest is a base look.
pub fn infer_role(kind: &AnimKind, tags: &[String]) -> Role {
    match kind {
        AnimKind::Sequence { .. } => Role::Effect,
        _ if tags.iter().any(|t| t == "notification") => Role::Effect,
        AnimKind::Effect { effect, .. } => Role::of_effect(effect),
        _ if tags.iter().any(|t| t == "notification") => Role::Effect,
        _ => Role::Theme,
    }
}

impl Animation {
    /// Fills in the role when missing so every animation handed out has one.
    pub fn normalize(&mut self) {
        if self.role.is_none() {
            self.role = Some(infer_role(&self.kind, &self.tags));
        }
    }
}

fn d_schema() -> u32 {
    SCHEMA_VERSION
}

// ---------- LED references ----------

/// Portable reference to LED `i`: by name when it has a unique non-empty one, else by index.
pub fn led_ref(i: usize, names: &[String]) -> String {
    match names.get(i) {
        Some(n) if !n.is_empty() && names.iter().filter(|x| *x == n).count() == 1 => format!("name:{n}"),
        _ => format!("idx:{i}"),
    }
}

/// First LED of each non-empty name.
pub fn name_index(names: &[String]) -> HashMap<&str, usize> {
    let mut m = HashMap::new();
    for (i, n) in names.iter().enumerate() {
        if !n.is_empty() {
            m.entry(n.as_str()).or_insert(i);
        }
    }
    m
}

/// Index of the LED a reference points to on a device with these names/`n` LEDs, if it exists.
pub fn resolve_ref(r: &str, index: &HashMap<&str, usize>, n: usize) -> Option<usize> {
    if let Some(name) = r.strip_prefix("name:") {
        index.get(name).copied()
    } else if let Some(i) = r.strip_prefix("idx:") {
        i.trim().parse::<usize>().ok().filter(|&i| i < n)
    } else {
        None
    }
}

/// Every `LedRef` an animation uses (empty for effect presets).
pub fn refs_of(a: &Animation) -> Vec<&str> {
    match &a.kind {
        AnimKind::Effect { .. } => vec![],
        AnimKind::Frames { frames, .. } => {
            let mut v: Vec<&str> = frames.iter().flat_map(|f| f.leds.keys().map(String::as_str)).collect();
            v.sort_unstable();
            v.dedup();
            v
        }
        AnimKind::Path { path, .. } => path.iter().map(String::as_str).collect(),
        AnimKind::Palette { cells, .. } => cells.keys().map(String::as_str).collect(),
        AnimKind::Sequence { steps, .. } => {
            let mut v: Vec<&str> = steps.iter().flat_map(|s| s.keys.iter().map(String::as_str)).collect();
            v.sort_unstable();
            v.dedup();
            v
        }
    }
}

/// `(resolved, total)` distinct references on a device (effect presets are always fully compatible).
pub fn compatibility(a: &Animation, names: &[String]) -> (usize, usize) {
    let index = name_index(names);
    let refs = refs_of(a);
    let ok = refs.iter().filter(|r| resolve_ref(r, &index, names.len()).is_some()).count();
    (ok, refs.len())
}

// ---------- resolution to index-based effects ----------

/// An animation resolved for one device: an index-based effect plus its parameters.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub effect: String,
    pub params: Params,
    pub matched: usize,
    pub total: usize,
}

fn stops_json(stops: &[Stop]) -> Value {
    Value::Array(stops.iter().map(|s| json!({ "pos": s.pos, "color": s.color })).collect())
}

/// Resolves `anim`'s LED references against a device's LED names and its slot colours against `pal`.
pub fn resolve(anim: &Animation, names: &[String], pal: &Palette) -> Resolved {
    let index = name_index(names);
    let n = names.len();
    let mut m = Map::new();
    let (effect, params) = match &anim.kind {
        AnimKind::Effect { effect, params } => (effect.clone(), params.clone()),
        AnimKind::Frames { frames, frame_ms, loop_mode, crossfade } => {
            let list: Vec<Value> = frames
                .iter()
                .map(|f| {
                    let leds: Map<String, Value> =
                        f.leds.iter().filter_map(|(r, c)| Some((resolve_ref(r, &index, n)?.to_string(), json!(c)))).collect();
                    let mut o = Map::new();
                    o.insert("leds".into(), Value::Object(leds));
                    if let Some(d) = f.duration_ms {
                        o.insert("duration_ms".into(), d.into());
                    }
                    Value::Object(o)
                })
                .collect();
            m.insert("frames".into(), Value::Array(list));
            m.insert("frame_ms".into(), (*frame_ms).into());
            m.insert("loop".into(), loop_mode.as_str().into());
            m.insert("crossfade".into(), json!(crossfade));
            ("frames".to_string(), Params(m))
        }
        AnimKind::Path { path, palette, speed, length, gap, reverse, tail_fade } => {
            let idx: Vec<Value> = path.iter().filter_map(|r| resolve_ref(r, &index, n)).map(|i| json!(i)).collect();
            m.insert("path".into(), Value::Array(idx));
            m.insert("palette".into(), stops_json(palette));
            m.insert("speed".into(), json!(speed));
            m.insert("length".into(), json!(length));
            m.insert("gap".into(), json!(gap));
            m.insert("reverse".into(), json!(reverse));
            m.insert("tail_fade".into(), json!(tail_fade));
            ("path".to_string(), Params(m))
        }
        AnimKind::Sequence { steps, looped } => {
            let list: Vec<Value> = steps.iter().map(|st| resolve_step(st, &index, n, pal)).collect();
            m.insert("steps".into(), Value::Array(list));
            m.insert("loop".into(), json!(looped));
            ("sequence".to_string(), Params(m))
        }
        AnimKind::Palette { cells, palette, speed, reverse } => {
            let c: Map<String, Value> = cells.iter().filter_map(|(r, p)| Some((resolve_ref(r, &index, n)?.to_string(), json!(p)))).collect();
            m.insert("cells".into(), Value::Object(c));
            m.insert("palette".into(), stops_json(palette));
            m.insert("speed".into(), json!(speed));
            m.insert("reverse".into(), json!(reverse));
            ("palette".to_string(), Params(m))
        }
    };
    let (matched, total) = compatibility(anim, names);
    let mut params = params;
    pal.resolve_map(&mut params.0);
    Resolved { effect, params, matched, total }
}

/// Path-based effects: their `keys` are the route, in order.
fn is_path_effect(e: &str) -> bool {
    matches!(e, "meter" | "blink" | "orbit")
}

/// A step as the `sequence` effect wants it: LED references resolved to indices.
fn resolve_step(st: &Step, index: &HashMap<&str, usize>, n: usize, pal: &Palette) -> Value {
    let leds: Vec<usize> = st.keys.iter().filter_map(|r| resolve_ref(r, index, n)).collect();
    let hold = json!({ "effect": "hold", "params": {}, "duration_ms": st.duration_ms });
    // Keys were chosen but none exists on this device: show nothing rather than everything.
    if !st.keys.is_empty() && leds.is_empty() {
        return hold;
    }
    let (effect, mut params) = match (st.effect.as_str(), &st.anim) {
        ("hold", _) => return hold,
        ("animation", Some(a)) if !matches!(a.kind, AnimKind::Sequence { .. }) => {
            let r = resolve(a, &index_names(index, n), pal);
            (r.effect, r.params.0)
        }
        ("animation", _) => return hold,
        (e, _) => (e.to_string(), st.params.0.clone()),
    };
    let mut step = Map::new();
    if is_path_effect(&effect) {
        params.insert("path".into(), json!(leds));
    } else if !leds.is_empty() {
        step.insert("mask".into(), json!(leds));
    }
    step.insert("effect".into(), effect.into());
    step.insert("params".into(), Value::Object(params));
    step.insert("duration_ms".into(), st.duration_ms.into());
    let mut v = Value::Object(step);
    pal.resolve_value(&mut v);
    v
}

/// Names by index rebuilt from a name index (library animation steps resolve against the same device).
fn index_names(index: &HashMap<&str, usize>, n: usize) -> Vec<String> {
    let mut names = vec![String::new(); n];
    for (name, &i) in index {
        if let Some(slot) = names.get_mut(i) {
            *slot = (*name).to_string();
        }
    }
    names
}

type CacheKey = (String, u64, String, usize, u64);

fn cache() -> &'static Mutex<HashMap<CacheKey, Arc<Resolved>>> {
    static C: OnceLock<Mutex<HashMap<CacheKey, Arc<Resolved>>>> = OnceLock::new();
    C.get_or_init(Mutex::default)
}

/// Resolution cached by `(animation id, updated, device, LED count, palette generation)`: names are resolved once
/// per device, not once per frame.
fn cached(id: &str, updated: u64, geo: &Geometry, make: impl FnOnce() -> Option<Resolved>) -> Option<Arc<Resolved>> {
    let key = (id.to_string(), updated, geo.device_key.to_string(), geo.led_count, geo.palette.generation);
    if let Some(r) = cache().lock().unwrap().get(&key) {
        return Some(r.clone());
    }
    let r = Arc::new(make()?);
    let mut c = cache().lock().unwrap();
    if c.len() >= 256 {
        c.clear();
    }
    c.insert(key, r.clone());
    Some(r)
}

fn draw(r: &Resolved, t: f32, geo: &Geometry, seed: u32, out: &mut [Px]) {
    effects::render(&r.effect, &r.params, t, geo.positions, seed, out);
}

/// Renders an animation onto `out` (transparent where it draws nothing).
pub fn render_animation(anim: &Animation, t: f32, geo: &Geometry, seed: u32, out: &mut [Px]) {
    match cached(&anim.id, anim.updated, geo, || Some(resolve(anim, geo.led_names, geo.palette))) {
        Some(r) => draw(&r, t, geo, seed, out),
        None => out.fill(Px::CLEAR),
    }
}

/// The `animation` layer: params `{ anim_id, snapshot }`. The snapshot travels with the profile,
/// so the layer keeps working when the library entry changes or disappears.
pub fn render_layer(params: &Params, t: f32, geo: &Geometry, seed: u32, out: &mut [Px]) {
    let Some(snap) = params.0.get("snapshot").filter(|v| v.is_object()) else {
        out.fill(Px::CLEAR);
        return;
    };
    let id = params.0.get("anim_id").and_then(Value::as_str).or_else(|| snap.get("id").and_then(Value::as_str)).unwrap_or("");
    let updated = snap.get("updated").and_then(Value::as_u64).unwrap_or(0);
    let r = cached(id, updated, geo, || serde_json::from_value::<Animation>(snap.clone()).ok().map(|a| resolve(&a, geo.led_names, geo.palette)));
    match r {
        Some(r) => draw(&r, t, geo, seed, out),
        None => out.fill(Px::CLEAR),
    }
}

// ---------- validation ----------

fn bad(why: impl ToString) -> ApiError {
    ApiError::new("anim_invalid", why)
}

pub fn validate_name(name: &str) -> Res<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 64 {
        return Err(ApiError::new("anim_name_invalid", n));
    }
    Ok(n.to_string())
}

fn check_stops(stops: &[Stop]) -> Res<()> {
    if stops.is_empty() || stops.len() > MAX_STOPS {
        return Err(bad(format!("palette needs 1..={MAX_STOPS} stops")));
    }
    if stops.iter().any(|s| !s.color.is_valid()) {
        return Err(bad("unknown palette slot"));
    }
    if stops.iter().any(|s| !s.pos.is_finite()) {
        return Err(bad("palette position is not a number"));
    }
    Ok(())
}

fn check_ref(r: &str) -> Res<()> {
    if (r.starts_with("name:") || r.starts_with("idx:")) && r.len() <= 200 { Ok(()) } else { Err(bad(format!("bad LED reference {r:?}"))) }
}

/// Checks sizes and fields; run on everything that comes from a file, the UI or a script.
pub fn validate(a: &Animation) -> Res<()> {
    validate_name(&a.name)?;
    if a.description.as_ref().is_some_and(|d| d.chars().count() > 500) {
        return Err(bad("description too long"));
    }
    if a.tags.len() > MAX_TAGS || a.tags.iter().any(|t| t.chars().count() > 32) {
        return Err(bad("too many or too long tags"));
    }
    match &a.kind {
        AnimKind::Effect { effect, params } => {
            if effect == "animation" || effects::effect_def(effect).is_none() {
                return Err(bad(format!("unknown effect {effect:?}")));
            }
            if serde_json::to_vec(params).map_err(bad)?.len() > 256 * 1024 {
                return Err(bad("effect parameters too large"));
            }
        }
        AnimKind::Frames { frames, frame_ms, crossfade, .. } => {
            if frames.is_empty() || frames.len() > MAX_FRAMES {
                return Err(bad(format!("1..={MAX_FRAMES} frames required")));
            }
            if !(10..=60_000).contains(frame_ms) || !crossfade.is_finite() || !(0.0..=1.0).contains(crossfade) {
                return Err(bad("frame time or crossfade out of range"));
            }
            for f in frames {
                if f.leds.len() > MAX_LEDS || f.duration_ms.is_some_and(|d| !(10..=600_000).contains(&d)) {
                    return Err(bad("frame too large or duration out of range"));
                }
                f.leds.keys().try_for_each(|r| check_ref(r))?;
                if f.leds.values().any(|c| !c.is_valid()) {
                    return Err(bad("unknown palette slot"));
                }
            }
        }
        AnimKind::Path { path, palette, speed, length, gap, tail_fade, .. } => {
            if path.is_empty() || path.len() > MAX_LEDS {
                return Err(bad("path needs 1..=4096 LEDs"));
            }
            path.iter().try_for_each(|r| check_ref(r))?;
            check_stops(palette)?;
            if ![speed, length, gap, tail_fade].iter().all(|v| v.is_finite()) {
                return Err(bad("number out of range"));
            }
        }
        AnimKind::Sequence { steps, .. } => {
            if steps.is_empty() || steps.len() > MAX_STEPS {
                return Err(bad(format!("1..={MAX_STEPS} steps required")));
            }
            for st in steps {
                if !(10..=3_600_000).contains(&st.duration_ms) || st.keys.len() > MAX_LEDS {
                    return Err(bad("step duration or key count out of range"));
                }
                st.keys.iter().try_for_each(|r| check_ref(r))?;
                if serde_json::to_vec(&st.params).map_err(bad)?.len() > 64 * 1024 {
                    return Err(bad("step parameters too large"));
                }
                match (st.effect.as_str(), &st.anim) {
                    ("hold", _) => {}
                    ("animation", Some(a)) => {
                        if matches!(a.kind, AnimKind::Sequence { .. }) {
                            return Err(bad("a step cannot play another sequence"));
                        }
                        validate(a)?;
                    }
                    ("animation", None) => return Err(bad("animation step without an animation")),
                    ("sequence", _) => return Err(bad("sequences cannot nest")),
                    (e, _) if effects::effect_def(e).is_some() => {}
                    (e, _) => return Err(bad(format!("unknown step effect {e:?}"))),
                }
            }
        }
        AnimKind::Palette { cells, palette, speed, .. } => {
            if cells.is_empty() || cells.len() > MAX_LEDS {
                return Err(bad("palette needs 1..=4096 cells"));
            }
            cells.iter().try_for_each(|(r, p)| if p.is_finite() { check_ref(r) } else { Err(bad("cell position is not a number")) })?;
            check_stops(palette)?;
            if !speed.is_finite() {
                return Err(bad("number out of range"));
            }
        }
    }
    Ok(())
}

// ---------- parsing and migration ----------

pub fn parse(text: &str) -> Res<Animation> {
    let v: Value = serde_json::from_str(text).map_err(|e| ApiError::new("anim_corrupt", e))?;
    let version = v.get("schema_version").and_then(Value::as_u64).unwrap_or(1);
    if version > SCHEMA_VERSION as u64 {
        return Err(ApiError::new("anim_too_new", version));
    }
    let mut a: Animation = serde_json::from_value(v).map_err(|e| ApiError::new("anim_corrupt", e))?;
    a.normalize();
    validate(&a)?;
    Ok(a)
}

fn read_file(p: &Path) -> Res<Animation> {
    let len = std::fs::metadata(p).map_err(ApiError::io)?.len();
    if len > MAX_FILE_BYTES {
        return Err(ApiError::new("anim_corrupt", "file too large"));
    }
    let mut a = parse(&std::fs::read_to_string(p).map_err(ApiError::io)?)?;
    if a.id.is_empty() {
        // Hand-written files without an id: the file name identifies them.
        a.id = format!("f-{}", p.file_stem().unwrap_or_default().to_string_lossy());
    }
    Ok(a)
}

// ---------- the library on disk ----------

pub fn animations_dir() -> PathBuf {
    config_dir().join("animations")
}

#[derive(Debug, Default, Serialize)]
pub struct AnimList {
    pub animations: Vec<Animation>,
    /// Files that could not be read.
    pub corrupt: Vec<String>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let h = effects::hash((nanos >> 32) as u32 ^ std::process::id(), nanos as u32, N.fetch_add(1, Ordering::Relaxed));
    format!("u-{nanos:x}{h:08x}")
}

fn json_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut files: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    files
}

/// Animations stored in `dir` (corrupt files are reported, never fatal).
pub fn list_user(dir: &Path) -> AnimList {
    let mut out = AnimList::default();
    for p in json_files(dir) {
        match read_file(&p) {
            Ok(mut a) => {
                a.builtin = false;
                out.animations.push(a);
            }
            Err(_) => out.corrupt.push(p.file_name().unwrap_or_default().to_string_lossy().into_owned()),
        }
    }
    out.animations.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// Built-in presets followed by the user's animations.
pub fn list_all(dir: &Path) -> AnimList {
    let mut l = list_user(dir);
    let mut all = builtins();
    all.append(&mut l.animations);
    l.animations = all;
    l
}

fn find_path(dir: &Path, id: &str) -> Option<PathBuf> {
    json_files(dir).into_iter().find(|p| read_file(p).is_ok_and(|a| a.id == id))
}

pub fn get(dir: &Path, id: &str) -> Res<Animation> {
    if let Some(b) = builtins().into_iter().find(|b| b.id == id) {
        return Ok(b);
    }
    let mut a = read_file(&find_path(dir, id).ok_or_else(|| ApiError::new("anim_not_found", id))?)?;
    a.builtin = false;
    Ok(a)
}

/// By id, then by exact name, then by case-insensitive name (what `trigger` accepts).
pub fn lookup(dir: &Path, key: &str) -> Res<Animation> {
    let all = list_all(dir).animations;
    let k = key.trim();
    all.iter()
        .find(|a| a.id == k)
        .or_else(|| all.iter().find(|a| a.name == k))
        .or_else(|| all.iter().find(|a| a.name.eq_ignore_ascii_case(k)))
        .or_else(|| all.iter().find(|a| a.id.strip_prefix("builtin:").is_some_and(|s| s.eq_ignore_ascii_case(k))))
        .cloned()
        .ok_or_else(|| ApiError::new("anim_not_found", k))
}

fn name_taken(dir: &Path, name: &str, except_id: &str) -> bool {
    list_all(dir).animations.iter().any(|a| a.id != except_id && a.name.eq_ignore_ascii_case(name))
}

fn free_path(dir: &Path, name: &str) -> PathBuf {
    let base = crate::profiles::slug(name);
    let mut p = dir.join(format!("{base}.json"));
    let mut i = 2;
    while p.exists() {
        p = dir.join(format!("{base}-{i}.json"));
        i += 1;
    }
    p
}

/// Saves (creates or overwrites by id) a user animation and returns it as stored.
pub fn save(dir: &Path, anim: &Animation) -> Res<Animation> {
    let mut a = anim.clone();
    a.name = validate_name(&a.name)?;
    a.schema_version = SCHEMA_VERSION;
    a.builtin = false;
    a.normalize();
    if a.id.is_empty() || a.id.starts_with("builtin:") {
        a.id = new_id();
    }
    validate(&a)?;
    if name_taken(dir, &a.name, &a.id) {
        return Err(ApiError::new("anim_exists", &a.name));
    }
    let existing = find_path(dir, &a.id);
    let now = now_ms();
    a.created = if a.created == 0 { now } else { a.created };
    // Strictly increasing so caches keyed by `updated` notice every save.
    a.updated = now.max(a.updated + 1);
    let path = existing.unwrap_or_else(|| free_path(dir, &a.name));
    let json = serde_json::to_vec_pretty(&a).map_err(ApiError::io)?;
    write_atomic(&path, &json).map_err(ApiError::io)?;
    Ok(a)
}

pub fn delete(dir: &Path, id: &str) -> Res<()> {
    if id.starts_with("builtin:") {
        return Err(ApiError::new("anim_builtin", id));
    }
    let p = find_path(dir, id).ok_or_else(|| ApiError::new("anim_not_found", id))?;
    std::fs::remove_file(p).map_err(ApiError::io)
}

/// A copy of any animation (built-ins included) under a new name.
pub fn duplicate(dir: &Path, id: &str, new_name: &str) -> Res<Animation> {
    let mut a = get(dir, id)?;
    a.id = String::new();
    a.created = 0;
    a.updated = 0;
    a.name = validate_name(new_name)?;
    save(dir, &a)
}

pub fn export(dir: &Path, id: &str, dest: &Path) -> Res<()> {
    let mut a = get(dir, id)?;
    a.builtin = false;
    let json = serde_json::to_vec_pretty(&a).map_err(ApiError::io)?;
    write_atomic(dest, &json).map_err(ApiError::io)
}

/// Imports an animation file as a new library entry; a name clash gets a numeric suffix.
pub fn import(dir: &Path, src: &Path) -> Res<Animation> {
    let mut a = read_file(src)?;
    a.id = String::new();
    a.created = 0;
    a.updated = 0;
    let base = validate_name(&a.name)?;
    a.name = base.clone();
    let mut i = 2;
    while name_taken(dir, &a.name, "") {
        a.name = format!("{base} ({i})");
        i += 1;
    }
    save(dir, &a)
}

// ---------- built-in presets ----------

const KEY_ROWS: [&[&str]; 5] = [
    &["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="],
    &["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]"],
    &["A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'"],
    &["Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"],
    &["Space"],
];

fn key_ref(k: &str) -> String {
    format!("name:Key: {k}")
}

fn builtin(slug: &str, name: &str, kind: AnimKind, target: TargetKind, tags: &[&str]) -> Animation {
    let tags: Vec<String> = tags.iter().map(|s| s.to_string()).collect();
    // Built-ins whose role is not what their kind suggests.
    let role = match slug {
        "breach-alert" => Role::Effect,
        "fill" | "neon-scan" => Role::Theme,
        _ => infer_role(&kind, &tags),
    };
    Animation {
        role: Some(role),
        schema_version: SCHEMA_VERSION,
        id: format!("builtin:{slug}"),
        name: name.into(),
        description: None,
        tags,
        created: 0,
        updated: 1,
        target: Target { kind: target, hint: None },
        builtin: true,
        kind,
    }
}

fn effect(slug: &str, name: &str, effect: &str, params: Value, tags: &[&str]) -> Animation {
    builtin(slug, name, AnimKind::Effect { effect: effect.into(), params: Params(params.as_object().cloned().unwrap_or_default()) }, TargetKind::Any, tags)
}

fn stop(pos: f32, c: Rgb3) -> Stop {
    Stop { pos, color: Color::Rgb(c) }
}

/// Read-only presets shipped with the app: five effect presets and one example of each new kind.
pub fn builtins() -> Vec<Animation> {
    let all_keys: Vec<String> = KEY_ROWS.iter().flat_map(|r| r.iter()).map(|k| key_ref(k)).collect();
    let heart = |c: Rgb3, ms: u32| Frame { leds: all_keys.iter().map(|k| (k.clone(), Color::Rgb(c))).collect(), duration_ms: Some(ms) };
    let snake: Vec<String> = ["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", ";", "L", "K", "J", "H", "G", "F", "D", "S", "A", "Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"]
        .iter()
        .map(|k| key_ref(k))
        .collect();
    let mut fire = BTreeMap::new();
    for (r, row) in KEY_ROWS[..4].iter().enumerate() {
        for (c, k) in row.iter().enumerate() {
            fire.insert(key_ref(k), r as f32 / 4.0 * 0.7 + c as f32 / row.len() as f32 * 0.3);
        }
    }
    vec![
        effect("rainbow", "Rainbow", "rainbow", json!({"speed": 0.25, "angle": 0, "scale": 1.0}), &["classic"]),
        effect("breathing", "Breathing", "breathing", json!({"colors": [[80, 140, 255]], "period": 4.0}), &["calm"]),
        effect("starlight", "Starlight", "starlight", json!({"density": 0.35, "duration": 1.5, "random": false, "colors": [[255, 255, 255], [160, 200, 255]]}), &["calm"]),
        effect("wave", "Wave", "color_wave", json!({"colors": [[0, 60, 200], [0, 220, 220], [0, 60, 200]], "speed": 0.3, "angle": 0, "scale": 1.0}), &["classic"]),
        effect("chase", "Chase", "chase", json!({"colors": [[255, 60, 0]], "speed": 0.6, "length": 0.25, "gap": 0.35}), &["classic"]),
        effect("alert", "Alert", "strobe", json!({"colors": [[255, 0, 0], [255, 255, 255]], "rate": 4.0, "duty": 0.6}), &["notification"]),
        builtin(
            "heartbeat",
            "Heartbeat",
            AnimKind::Frames {
                frames: vec![heart([70, 0, 6], 140), heart([255, 0, 30], 110), heart([100, 0, 10], 140), heart([255, 0, 30], 110), heart([40, 0, 4], 700)],
                frame_ms: 200,
                loop_mode: LoopMode::Loop,
                crossfade: 0.6,
            },
            TargetKind::Keyboard,
            &["frames"],
        ),
        builtin(
            "snake",
            "Snake",
            AnimKind::Path {
                path: snake,
                palette: vec![stop(0.0, [200, 255, 220]), stop(0.4, [0, 230, 120]), stop(1.0, [0, 60, 90])],
                speed: 0.45,
                length: 0.3,
                gap: 0.2,
                reverse: false,
                tail_fade: 0.7,
            },
            TargetKind::Keyboard,
            &["path"],
        ),
        // ---- themed presets: their colours are palette slots, so they follow the active theme ----
        effect("accent-breathing", "Accent breathing", "breathing", json!({"colors": [{"slot": "accent"}], "period": 4.0}), &["themed", "calm"]),
        effect("theme-wave", "Theme wave", "color_wave", json!({"colors": [{"slot": "accent"}, {"slot": "secondary"}, {"slot": "accent"}], "speed": 0.2, "angle": 0, "scale": 1.0}), &["themed", "classic"]),
        effect("theme-chase", "Theme chase", "chase", json!({"colors": [{"slot": "primary"}], "speed": 0.6, "length": 0.25, "gap": 0.35}), &["themed", "classic"]),
        effect("flash", "Flash", "strobe", json!({"colors": [{"slot": "info"}], "rate": 3.0, "duty": 0.35}), &["themed", "notification"]),
        effect("pulse", "Pulse", "breathing", json!({"colors": [{"slot": "ok"}], "period": 1.0}), &["themed", "notification"]),
        effect("blink", "Blink", "strobe", json!({"colors": [{"slot": "alert"}], "rate": 2.0, "duty": 0.5}), &["themed", "notification"]),
        effect("fill", "Fill", "solid", json!({"color": {"slot": "accent"}}), &["themed", "notification"]),
        effect("glitch", "Glitch", "glitch", json!({"colors": [{"slot": "primary"}, {"slot": "secondary"}, {"slot": "accent"}], "interval": 5.0, "duration": 0.3, "density": 0.3, "size": 0.16}), &["themed", "cyberpunk"]),
        // The Cyberpunk base look as one effect; on a device Quick lays down its per-device layers
        // instead (`builtin::theme_layers`), hence the `layers` tag.
        effect("cyberpunk", "Cyberpunk", "color_wave", json!({"colors": [{"slot": "primary"}, {"slot": "secondary"}, {"slot": "tertiary"}], "speed": 0.3, "angle": 0, "scale": 1.0}), &["themed", "cyberpunk", "layers"]),
        builtin(
            "neon-scan",
            "Neon Scan",
            AnimKind::Path {
                path: KEY_ROWS[..4].iter().flat_map(|r| r.iter()).map(|k| key_ref(k)).collect(),
                palette: vec![Stop { pos: 0.0, color: Color::slot("primary") }, Stop { pos: 0.5, color: Color::slot("secondary") }, Stop { pos: 1.0, color: Color::slot("tertiary") }],
                speed: 0.55,
                length: 0.22,
                gap: 0.3,
                reverse: false,
                tail_fade: 0.8,
            },
            TargetKind::Keyboard,
            &["themed", "path", "cyberpunk"],
        ),
        builtin(
            "breach-alert",
            "Breach Alert",
            {
                let flash = |c: Color, ms: u32| Frame { leds: all_keys.iter().map(|k| (k.clone(), c.clone())).collect(), duration_ms: Some(ms) };
                let dim = Color::Slot(crate::themes::SlotRef { slot: "alert".into(), lighten: Some(-0.8) });
                AnimKind::Frames {
                    frames: vec![flash(Color::slot("alert"), 90), flash(dim.clone(), 70), flash(Color::slot("accent"), 60), flash(Color::slot("alert"), 110), flash(dim, 80), flash(Color::slot("secondary"), 50), flash(Color::slot("alert"), 400)],
                    frame_ms: 100,
                    loop_mode: LoopMode::Loop,
                    crossfade: 0.0,
                }
            },
            TargetKind::Keyboard,
            &["themed", "frames", "cyberpunk"],
        ),
        builtin(
            "soft-fire",
            "Soft fire",
            AnimKind::Palette {
                cells: fire,
                palette: vec![stop(0.0, [40, 0, 0]), stop(0.3, [200, 30, 0]), stop(0.6, [255, 140, 0]), stop(0.8, [255, 200, 40]), stop(1.0, [40, 0, 0])],
                speed: 0.25,
                reverse: false,
            },
            TargetKind::Keyboard,
            &["palette"],
        ),
    ]
}

// ---------- layers ----------

/// An `animation` layer playing `anim` (the snapshot keeps it self-contained).
pub fn layer_for(anim: &Animation) -> crate::engine::compositor::Layer {
    use crate::engine::compositor::{Blend, Layer, Mask};
    let mut m = Map::new();
    m.insert("anim_id".into(), anim.id.clone().into());
    m.insert("snapshot".into(), serde_json::to_value(anim).unwrap_or(Value::Null));
    Layer {
        id: format!("anim-{}", new_id()),
        name: String::new(),
        enabled: true,
        opacity: 1.0,
        blend: Blend::Normal,
        mask: Mask::All,
        effect: "animation".into(),
        params: Params(m),
        role: anim.role,
    }
}

/// A copy of `a` playing `k` times faster (`k` > 1) or slower.
pub fn with_speed(a: &Animation, k: f32) -> Animation {
    let k = k.clamp(0.1, 10.0);
    let mut out = a.clone();
    match &mut out.kind {
        AnimKind::Frames { frames, frame_ms, .. } => {
            *frame_ms = ((*frame_ms as f32 / k).round() as u32).clamp(10, 60_000);
            frames.iter_mut().for_each(|f| f.duration_ms = f.duration_ms.map(|d| ((d as f32 / k).round() as u32).clamp(10, 600_000)));
        }
        AnimKind::Path { speed, .. } | AnimKind::Palette { speed, .. } => *speed *= k,
        AnimKind::Sequence { steps, .. } => {
            for st in steps {
                st.duration_ms = ((st.duration_ms as f32 / k).round() as u32).clamp(10, 3_600_000);
            }
        }
        AnimKind::Effect { params, .. } => {
            for key in ["speed", "rate"] {
                if let Some(v) = params.0.get(key).and_then(Value::as_f64) {
                    params.0.insert(key.into(), json!(v * k as f64));
                }
            }
            if let Some(v) = params.0.get("period").and_then(Value::as_f64) {
                params.0.insert("period".into(), json!(v / k as f64));
            }
        }
    }
    out
}

/// Seconds one pass of the animation takes (used to turn `repeat` into a duration).
pub fn cycle_secs(a: &Animation) -> f32 {
    match &a.kind {
        AnimKind::Frames { frames, frame_ms, loop_mode, .. } => {
            let ms = |f: &Frame| f.duration_ms.unwrap_or(*frame_ms) as f32;
            let sum: f32 = frames.iter().map(ms).sum();
            let back: f32 = if *loop_mode == LoopMode::PingPong && frames.len() > 2 { frames[1..frames.len() - 1].iter().map(ms).sum() } else { 0.0 };
            ((sum + back) / 1000.0).max(0.05)
        }
        AnimKind::Path { speed, length, gap, .. } => ((length + gap).max(0.02) / speed.abs().max(0.01)).clamp(0.2, 60.0),
        AnimKind::Palette { speed, .. } => (1.0 / speed.abs().max(0.01)).clamp(0.2, 60.0),
        AnimKind::Sequence { steps, .. } => (steps.iter().map(|s| s.duration_ms as f32).sum::<f32>() / 1000.0).max(0.05),
        // One strobe pulse / one breath, else a nominal second.
        AnimKind::Effect { effect, params } => {
            let n = |k: &str| params.0.get(k).and_then(Value::as_f64).map(|v| v as f32);
            match effect.as_str() {
                "strobe" => (1.0 / n("rate").unwrap_or(4.0).max(0.1)).max(0.05),
                "breathing" => n("period").unwrap_or(4.0).max(0.05),
                _ => 1.0,
            }
        }
    }
}

/// A copy of `a` recoloured with `c` (keeps each colour's brightness as shading).
pub fn recolor(a: &Animation, c: Rgb3, pal: &Palette) -> Animation {
    let shade = |old: &Color| {
        let old = pal.resolve(old);
        let k = old.iter().copied().max().unwrap_or(255) as f32 / 255.0;
        Color::Rgb([0, 1, 2].map(|i| (c[i] as f32 * k).round() as u8))
    };
    let mut out = a.clone();
    match &mut out.kind {
        AnimKind::Frames { frames, .. } => frames.iter_mut().flat_map(|f| f.leds.values_mut()).for_each(|v| *v = shade(v)),
        AnimKind::Path { palette, .. } | AnimKind::Palette { palette, .. } => palette.iter_mut().for_each(|s| s.color = shade(&s.color)),
        AnimKind::Sequence { steps, .. } => {
            for st in steps {
                if st.params.0.contains_key("color") {
                    st.params.0.insert("color".into(), json!(c));
                }
            }
        }
        AnimKind::Effect { params, .. } => {
            if params.0.contains_key("color") {
                params.0.insert("color".into(), json!(c));
            }
            if let Some(Value::Array(list)) = params.0.get_mut("colors") {
                // Several colours (a wave) keep some variation: lighter and darker shades of `c`.
                let multi = list.len() > 1;
                for (i, v) in list.iter_mut().enumerate() {
                    let shade = if !multi || i % 3 == 0 { 0.0 } else if i % 3 == 1 { 0.45 } else { -0.35 };
                    *v = json!(crate::themes::lighten(c, shade));
                }
            }
            // Effects with random colours use their palette once told to.
            if params.0.contains_key("random") {
                params.0.insert("random".into(), json!(false));
            }
            if !params.0.contains_key("color") && !params.0.contains_key("colors") {
                params.0.insert("colors".into(), json!([c]));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir;

    fn names() -> Vec<String> {
        ["Key: Q", "Key: W", "", "Key: E", "Dup", "Dup"].iter().map(|s| s.to_string()).collect()
    }

    fn frames_anim(name: &str) -> Animation {
        Animation {
            schema_version: 1,
            id: String::new(),
            name: name.into(),
            description: None,
            tags: vec!["t".into()],
            created: 0,
            updated: 0,
            target: Target::default(),
            builtin: false,
            role: None,
            kind: AnimKind::Frames {
                frames: vec![
                    Frame { leds: [("name:Key: W".to_string(), Color::Rgb([255, 0, 0]))].into(), duration_ms: Some(100) },
                    Frame { leds: [("idx:2".to_string(), Color::Rgb([0, 255, 0])), ("name:Key: Missing".to_string(), Color::Rgb([1, 1, 1]))].into(), duration_ms: None },
                ],
                frame_ms: 100,
                loop_mode: LoopMode::Loop,
                crossfade: 0.0,
            },
        }
    }

    fn geo<'a>(key: &'a str, names: &'a [String], pos: &'a [[f32; 2]]) -> Geometry<'a> {
        Geometry { led_count: names.len(), device_key: key, led_names: names, zones: &[], positions: pos, palette: crate::themes::default_palette() }
    }

    #[test]
    fn led_refs_use_names_when_unique() {
        let n = names();
        assert_eq!(led_ref(1, &n), "name:Key: W");
        assert_eq!(led_ref(2, &n), "idx:2", "unnamed LEDs fall back to the index");
        assert_eq!(led_ref(4, &n), "idx:4", "duplicated names are not portable");
        let idx = name_index(&n);
        assert_eq!(resolve_ref("name:Key: E", &idx, n.len()), Some(3));
        assert_eq!(resolve_ref("idx:5", &idx, n.len()), Some(5));
        assert_eq!(resolve_ref("idx:6", &idx, n.len()), None, "out of range");
        assert_eq!(resolve_ref("name:Nope", &idx, n.len()), None);
        assert_eq!(resolve_ref("junk", &idx, n.len()), None);
    }

    #[test]
    fn resolution_is_portable_across_layouts() {
        let a = frames_anim("x");
        // Same keys in a different order on another keyboard.
        let other: Vec<String> = ["", "Key: W", "Key: Q"].iter().map(|s| s.to_string()).collect();
        let r = resolve(&a, &other, crate::themes::default_palette());
        let f0 = r.params.0["frames"][0]["leds"].as_object().unwrap();
        assert!(f0.contains_key("1"), "W moved to index 1: {f0:?}");
        // idx:2 exists on both, the missing key is ignored.
        assert_eq!(r.params.0["frames"][1]["leds"].as_object().unwrap().len(), 1);
        assert_eq!(compatibility(&a, &other), (2, 3));
        assert_eq!(compatibility(&a, &names()), (2, 3));
        let (ok, total) = compatibility(&builtins()[0], &other);
        assert_eq!((ok, total), (0, 0), "effect presets are always compatible");
    }

    #[test]
    fn layer_renders_snapshot_and_caches() {
        let a = Animation { id: "t-render".into(), updated: 5, ..frames_anim("render") };
        let layer = layer_for(&a);
        let n = names();
        let pos = vec![[0.5, 0.5]; n.len()];
        let g = geo("dev-a", &n, &pos);
        let mut out = vec![Px::CLEAR; n.len()];
        render_layer(&layer.params, 0.05, &g, 1, &mut out);
        assert_eq!((out[1].r, out[1].a), (255.0, 1.0), "W lit in frame 0");
        assert_eq!(out[2].a, 0.0);
        render_layer(&layer.params, 0.15, &g, 1, &mut out);
        assert_eq!((out[2].g, out[1].a), (255.0, 0.0), "frame 1 lights the unnamed LED by index");
        // Missing snapshot draws nothing.
        render_layer(&Params::default(), 0.0, &g, 1, &mut out);
        assert!(out.iter().all(|p| p.a == 0.0));
    }

    #[test]
    fn roles_are_inferred_stored_and_backward_compatible() {
        let b = |id: &str| builtins().into_iter().find(|a| a.id == format!("builtin:{id}")).unwrap().role;
        for id in ["glitch", "starlight", "breach-alert", "alert", "flash", "pulse", "blink"] {
            assert_eq!(b(id), Some(Role::Effect), "{id}");
        }
        for id in ["cyberpunk", "neon-scan", "theme-wave", "theme-chase", "accent-breathing", "rainbow", "breathing", "wave", "chase", "fill", "heartbeat", "snake"] {
            assert_eq!(b(id), Some(Role::Theme), "{id}");
        }
        assert!(builtins().iter().all(|a| a.role.is_some()));
        // Old files have no role: it is inferred on load; a stored one wins.
        let old = parse(r#"{"name": "o", "kind": {"type": "effect", "effect": "glitch"}}"#).unwrap();
        assert_eq!(old.role, Some(Role::Effect));
        let plain = parse(r#"{"name": "o2", "kind": {"type": "effect", "effect": "rainbow"}}"#).unwrap();
        assert_eq!(plain.role, Some(Role::Theme));
        let marked = parse(r#"{"name": "o3", "role": "effect", "kind": {"type": "effect", "effect": "rainbow"}}"#).unwrap();
        assert_eq!(marked.role, Some(Role::Effect));
        let d = crate::fsutil::test_dir("anim-role");
        let mut a = frames_anim("Marked");
        assert_eq!(save(&d, &a).unwrap().role, Some(Role::Theme));
        a.role = Some(Role::Effect);
        a.name = "Marked fx".into();
        assert_eq!(get(&d, &save(&d, &a).unwrap().id).unwrap().role, Some(Role::Effect));
        // Layers of animations carry the role; scene layers without one are inferred from the effect.
        assert_eq!(layer_for(&builtins().into_iter().find(|a| a.id == "builtin:glitch").unwrap()).role(), Role::Effect);
        let l: crate::engine::compositor::Layer = serde_json::from_value(json!({"id": "x", "name": "", "effect": "starlight"})).unwrap();
        assert!(l.role.is_none() && l.role() == Role::Effect);
        let l: crate::engine::compositor::Layer = serde_json::from_value(json!({"id": "x", "name": "", "effect": "chase"})).unwrap();
        assert_eq!(l.role(), Role::Theme);
        assert!(!serde_json::to_string(&l).unwrap().contains("role"));
    }

    #[test]
    fn serde_roundtrip_and_defaults() {
        let a = frames_anim("rt");
        let back: Animation = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(a, back);
        let v: Value = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(v["kind"]["type"], "frames");
        assert_eq!(v["kind"]["loop"], "loop");
        let p: Animation = serde_json::from_value(json!({"name": "p", "kind": {"type": "path", "path": ["idx:0"], "palette": [{"pos": 0, "color": [1, 2, 3]}]}})).unwrap();
        validate(&p).unwrap();
        assert!(matches!(p.kind, AnimKind::Path { speed, .. } if (speed - 0.5).abs() < 1e-6));
    }

    #[test]
    fn library_save_load_list_delete() {
        let d = test_dir("anim-lib");
        let saved = save(&d, &frames_anim("My blink")).unwrap();
        assert!(saved.id.starts_with("u-") && saved.created > 0 && saved.updated > 0);
        let list = list_user(&d);
        assert_eq!(list.animations.len(), 1);
        assert_eq!(list.animations[0], saved);
        assert_eq!(get(&d, &saved.id).unwrap().name, "My blink");
        // Saving again overwrites the same file and bumps `updated`.
        let again = save(&d, &Animation { name: "My blink v2".into(), ..saved.clone() }).unwrap();
        assert_eq!(json_files(&d).len(), 1);
        assert!(again.updated > saved.updated);
        assert_eq!(list_user(&d).animations[0].name, "My blink v2");
        delete(&d, &saved.id).unwrap();
        assert!(list_user(&d).animations.is_empty());
        assert_eq!(delete(&d, &saved.id).unwrap_err().code, "anim_not_found");
    }

    #[test]
    fn names_are_unique_and_builtins_protected() {
        let d = test_dir("anim-names");
        save(&d, &frames_anim("Alpha")).unwrap();
        assert_eq!(save(&d, &frames_anim("alpha")).unwrap_err().code, "anim_exists");
        assert_eq!(save(&d, &frames_anim("Rainbow")).unwrap_err().code, "anim_exists", "clashes with a preset");
        assert_eq!(save(&d, &frames_anim("  ")).unwrap_err().code, "anim_name_invalid");
        assert_eq!(delete(&d, "builtin:rainbow").unwrap_err().code, "anim_builtin");
        // Duplicating a preset gives an editable user copy.
        let copy = duplicate(&d, "builtin:heartbeat", "My heartbeat").unwrap();
        assert!(!copy.builtin && copy.id.starts_with("u-"));
        assert!(matches!(copy.kind, AnimKind::Frames { .. }));
        assert_eq!(duplicate(&d, "builtin:heartbeat", "My heartbeat").unwrap_err().code, "anim_exists");
        // Saving with a preset id cannot overwrite it.
        let forged = save(&d, &Animation { id: "builtin:snake".into(), ..frames_anim("Forged") }).unwrap();
        assert_ne!(forged.id, "builtin:snake");
    }

    #[test]
    fn corrupt_files_are_reported_not_fatal() {
        let d = test_dir("anim-corrupt");
        save(&d, &frames_anim("Good")).unwrap();
        std::fs::write(d.join("broken.json"), "{ not json").unwrap();
        std::fs::write(d.join("future.json"), r#"{"schema_version": 99, "name": "f", "kind": {"type": "effect", "effect": "solid"}}"#).unwrap();
        std::fs::write(d.join("evil.json"), r#"{"name": "evil", "kind": {"type": "effect", "effect": "animation"}}"#).unwrap();
        std::fs::write(d.join("notes.txt"), "ignored").unwrap();
        let l = list_user(&d);
        assert_eq!(l.animations.len(), 1);
        assert_eq!(l.corrupt, vec!["broken.json", "evil.json", "future.json"]);
        assert_eq!(parse(r#"{"schema_version": 99, "name": "f", "kind": {"type": "effect", "effect": "solid"}}"#).unwrap_err().code, "anim_too_new");
    }

    #[test]
    fn writes_are_atomic_no_temp_files() {
        let d = test_dir("anim-atomic");
        save(&d, &frames_anim("One")).unwrap();
        save(&d, &frames_anim("Two")).unwrap();
        let names: Vec<_> = std::fs::read_dir(&d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| n.ends_with(".json") && !n.starts_with('.')), "{names:?}");
    }

    #[test]
    fn export_import_roundtrip_renames_on_clash() {
        let d = test_dir("anim-io");
        let a = save(&d, &frames_anim("Shared")).unwrap();
        let file = d.join("out.json");
        export(&d, &a.id, &file).unwrap();
        let imp = import(&d, &file).unwrap();
        assert_eq!(imp.name, "Shared (2)");
        assert_ne!(imp.id, a.id);
        assert_eq!(imp.kind, a.kind);
        // Presets export as ordinary (non built-in) files.
        let pf = d.join("preset.json");
        export(&d, "builtin:snake", &pf).unwrap();
        let p = import(&d, &pf).unwrap();
        assert_eq!(p.name, "Snake (2)");
        assert!(!p.builtin);
        assert_eq!(import(&d, &d.join("missing.json")).unwrap_err().code, "io");
    }

    #[test]
    fn validation_limits() {
        let mut a = frames_anim("v");
        if let AnimKind::Frames { frames, .. } = &mut a.kind {
            frames.clear();
        }
        assert_eq!(validate(&a).unwrap_err().code, "anim_invalid");
        let big = Animation {
            kind: AnimKind::Path { path: (0..5000).map(|i| format!("idx:{i}")).collect(), palette: vec![stop(0.0, [1, 1, 1])], speed: 1.0, length: 0.3, gap: 0.3, reverse: false, tail_fade: 0.5 },
            ..frames_anim("big")
        };
        assert!(validate(&big).is_err());
        let bad_ref = Animation { kind: AnimKind::Palette { cells: [("junk".to_string(), 0.5)].into(), palette: vec![stop(0.0, [1, 1, 1])], speed: 1.0, reverse: false }, ..frames_anim("r") };
        assert!(validate(&bad_ref).is_err());
        let no_pal = Animation { kind: AnimKind::Palette { cells: [("idx:0".to_string(), 0.5)].into(), palette: vec![], speed: 1.0, reverse: false }, ..frames_anim("p") };
        assert!(validate(&no_pal).is_err());
        assert!(validate(&Animation { name: "x".repeat(65), ..frames_anim("n") }).is_err());
        // Oversized files are refused before parsing.
        let d = test_dir("anim-big");
        let f = d.join("huge.json");
        std::fs::write(&f, vec![b' '; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        assert_eq!(import(&d, &f).unwrap_err().code, "anim_corrupt");
    }

    #[test]
    fn builtins_are_valid_and_render() {
        let all = builtins();
        assert_eq!(all.len(), 20);
        let mut seen = std::collections::HashSet::new();
        // A keyboard that has every key the presets use.
        let kb: Vec<String> = KEY_ROWS.iter().flat_map(|r| r.iter()).map(|k| format!("Key: {k}")).collect();
        let pos: Vec<[f32; 2]> = (0..kb.len()).map(|i| [i as f32 / kb.len() as f32, 0.5]).collect();
        for a in &all {
            assert!(a.builtin && a.id.starts_with("builtin:") && seen.insert(a.id.clone()), "{}", a.id);
            validate(a).unwrap_or_else(|e| panic!("{}: {e}", a.name));
            let g = geo("kb", &kb, &pos);
            let mut out = vec![Px::CLEAR; kb.len()];
            // The glitch flickers only now and then: look for a moment where it shows.
            let t = if a.id == "builtin:glitch" { (0..500).map(|i| i as f32 * 0.01).find(|t| { render_animation(a, *t, &g, 1, &mut out); out.iter().any(|p| p.a > 0.0) }).unwrap_or(0.3) } else { 0.3 };
            render_animation(a, t, &g, 1, &mut out);
            assert!(out.iter().any(|p| p.a > 0.0), "{} draws nothing at t={t}", a.name);
            let (ok, total) = compatibility(a, &kb);
            assert_eq!(ok, total, "{} should fully match a standard keyboard", a.name);
        }
        assert_eq!(list_all(&test_dir("anim-none")).animations.len(), 20);
    }

    /// The mock backend and the TS tests read the presets from this file; regenerate it with
    /// `UPDATE_FIXTURES=1 cargo test builtins_fixture`.
    #[test]
    fn builtins_fixture_is_current() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/mock/builtins.json");
        let json = serde_json::to_string_pretty(&builtins()).unwrap() + "\n";
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(path, &json).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap_or_default(), json, "run `UPDATE_FIXTURES=1 cargo test builtins_fixture` to refresh {path}");
    }

    #[test]
    fn lookup_by_id_or_name() {
        let d = test_dir("anim-lookup");
        let a = save(&d, &frames_anim("Notify me")).unwrap();
        assert_eq!(lookup(&d, &a.id).unwrap().id, a.id);
        assert_eq!(lookup(&d, "notify ME").unwrap().id, a.id);
        assert_eq!(lookup(&d, "Rainbow").unwrap().id, "builtin:rainbow");
        assert_eq!(lookup(&d, "soft-fire").unwrap().name, "Soft fire");
        assert_eq!(lookup(&d, "nope").unwrap_err().code, "anim_not_found");
    }

    #[test]
    fn recolor_and_cycle() {
        let a = frames_anim("rc");
        let r = recolor(&a, [0, 0, 200], crate::themes::default_palette());
        let AnimKind::Frames { frames, .. } = r.kind else { panic!() };
        assert_eq!(frames[0].leds["name:Key: W"], Color::Rgb([0, 0, 200]));
        let alert = builtins().into_iter().find(|b| b.id == "builtin:alert").unwrap();
        let AnimKind::Effect { params, .. } = recolor(&alert, [1, 2, 3], crate::themes::default_palette()).kind else { panic!() };
        assert_eq!(params.0["colors"][0], json!([1, 2, 3]));
        assert_ne!(params.0["colors"][1], json!([1, 2, 3]), "a multi-colour effect keeps some variation");
        assert!((cycle_secs(&a) - 0.2).abs() < 1e-3, "{}", cycle_secs(&a));
    }

    #[test]
    fn old_rgb_animations_load_unchanged_and_slots_roundtrip() {
        let old = r#"{ "name": "Old", "kind": { "type": "path", "path": ["idx:0"], "palette": [ { "pos": 0.0, "color": [1, 2, 3] }, { "pos": 1.0, "color": [4, 5, 6] } ] } }"#;
        let a = parse(old).unwrap();
        let AnimKind::Path { palette, .. } = &a.kind else { panic!() };
        assert_eq!(palette[0].color, Color::Rgb([1, 2, 3]));
        let back: Animation = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(back, a);
        let with_slots = old.replace("[1, 2, 3]", r#"{"slot": "accent"}"#);
        let b = parse(&with_slots).unwrap();
        let AnimKind::Path { palette, .. } = &b.kind else { panic!() };
        assert_eq!(palette[0].color, Color::slot("accent"));
        assert!(parse(&old.replace("[1, 2, 3]", r#"{"slot": "nope"}"#)).is_err());
        // Resolution turns the slot into the palette colour.
        let mut pal = Palette::default_manual();
        pal.slots.insert("accent".into(), [255, 128, 0]);
        let r = resolve(&b, &["Key: A".to_string()], &pal);
        assert_eq!(r.params.0["palette"][0]["color"], json!([255, 128, 0]));
        // Recolouring a slot-coloured animation shades by the slot's brightness.
        let AnimKind::Path { palette, .. } = recolor(&b, [200, 100, 0], &pal).kind else { panic!() };
        assert_eq!(palette[0].color, Color::Rgb([200, 100, 0]));
    }

    fn seq(steps: Vec<Step>, looped: bool) -> Animation {
        Animation { kind: AnimKind::Sequence { steps, looped }, ..frames_anim("seq") }
    }

    fn step(effect: &str, params: Value, keys: &[&str], ms: u32) -> Step {
        Step { effect: effect.into(), params: Params(params.as_object().cloned().unwrap_or_default()), keys: keys.iter().map(|k| format!("name:Key: {k}")).collect(), duration_ms: ms, anim: None }
    }

    fn keyboard() -> Vec<String> {
        ["Key: W", "Key: A", "Key: S", "Key: D", "Key: Q"].iter().map(|s| s.to_string()).collect()
    }

    fn at(a: &Animation, names: &[String], t: f32) -> Vec<Px> {
        let pos = vec![[0.5, 0.5]; names.len()];
        let mut out = vec![Px::CLEAR; names.len()];
        render_animation(a, t, &geo("kb", names, &pos), 1, &mut out);
        out
    }

    #[test]
    fn sequence_blinks_then_orbits_then_ends_on_the_named_keys() {
        let mut a = seq(
            vec![
                step("blink", json!({"color": [0, 0, 255], "count": 2, "on_ms": 100, "off_ms": 100}), &["W", "A", "S", "D"], 400),
                step("orbit", json!({"color": [255, 255, 255], "laps": 1, "lap_ms": 400, "trail": 0.25}), &["W", "D", "S", "A"], 400),
            ],
            false,
        );
        a.id = "seq1".into();
        let n = keyboard();
        let lit = |t: f32| at(&a, &n, t).iter().enumerate().filter(|(_, p)| p.a > 0.0 && p.r + p.g + p.b > 0.0).map(|(i, _)| i).collect::<Vec<_>>();
        assert_eq!(lit(0.05), vec![0, 1, 2, 3], "blink on");
        assert!(lit(0.15).is_empty(), "blink off");
        assert_eq!(lit(0.25), vec![0, 1, 2, 3], "second blink");
        // Orbit visits W (0), D (3), S (2), A (1) in that order.
        assert_eq!(lit(0.45), vec![0]);
        assert_eq!(lit(0.55), vec![3]);
        assert_eq!(lit(0.65), vec![2]);
        assert_eq!(lit(0.75), vec![1]);
        assert!(lit(0.9).is_empty(), "off after the sequence");
        assert!(at(&a, &n, 0.3)[4].a == 0.0, "Q is never touched");
        assert_eq!(cycle_secs(&a), 0.8);
        assert_eq!(compatibility(&a, &n), (4, 4));
    }

    #[test]
    fn sequence_with_missing_keys_shows_nothing_and_meter_follows_key_order() {
        let n = keyboard();
        let mut a = seq(vec![step("blink", json!({"color": [255, 0, 0]}), &["F5", "F6"], 500)], true);
        a.id = "seq2".into();
        assert!(at(&a, &n, 0.01).iter().all(|p| p.a == 0.0), "unknown keys must not light the whole keyboard");
        let mut m = seq(vec![step("meter", json!({"value": 50.0, "color": [255, 128, 0], "style": "live", "background": "off"}), &["D", "S", "A", "W"], 1000)], true);
        m.id = "seq3".into();
        let out = at(&m, &n, 0.5);
        // 50 % of 4 keys: D and S lit (indices 3 and 2), A and W dark.
        assert_eq!((out[3].r, out[2].r, out[1].r, out[0].r), (255.0, 255.0, 0.0, 0.0));
        assert_eq!(out[4].a, 0.0);
    }

    #[test]
    fn sequence_validation_and_json_round_trip() {
        let ok = seq(vec![step("meter", json!({}), &["W"], 1000), step("hold", json!({}), &[], 500)], true);
        validate(&ok).unwrap();
        let back: Animation = serde_json::from_str(&serde_json::to_string(&ok).unwrap()).unwrap();
        assert_eq!(back, ok);
        assert!(serde_json::to_string(&ok).unwrap().contains("\"type\":\"sequence\""));
        assert!(validate(&seq(vec![], false)).is_err());
        assert!(validate(&seq(vec![step("meter", json!({}), &["W"], 5)], false)).is_err());
        assert!(validate(&seq(vec![step("nope", json!({}), &[], 100)], false)).is_err());
        assert!(validate(&seq(vec![step("sequence", json!({}), &[], 100)], false)).is_err());
        let mut bad_ref = step("blink", json!({}), &[], 100);
        bad_ref.keys = vec!["W".into()];
        assert!(validate(&seq(vec![bad_ref], false)).is_err());
        assert!(validate(&seq(vec![step("animation", json!({}), &[], 100)], false)).is_err());
        let inner = seq(vec![step("hold", json!({}), &[], 100)], false);
        let mut nested = step("animation", json!({}), &[], 100);
        nested.anim = Some(Box::new(inner));
        assert!(validate(&seq(vec![nested], false)).is_err());
    }

    #[test]
    fn sequence_can_play_a_library_animation_step_and_slots_follow_the_palette() {
        let n = keyboard();
        let lib = builtins().into_iter().find(|b| b.id == "builtin:fill").unwrap();
        let mut play = step("animation", json!({}), &["Q"], 1000);
        play.anim = Some(Box::new(lib));
        let mut a = seq(vec![play, step("solid", json!({"color": {"slot": "alert"}}), &[], 1000)], false);
        a.id = "seq4".into();
        validate(&a).unwrap();
        let first = at(&a, &n, 0.5);
        assert!(first[4].a > 0.0 && first[0].a == 0.0, "only Q plays the library animation");
        let second = at(&a, &n, 1.5);
        let alert = crate::themes::default_palette().get("alert");
        assert_eq!([second[0].r as u8, second[0].g as u8, second[0].b as u8], alert, "slot colours resolve");
        // Speed and recolour touch the steps.
        assert_eq!(cycle_secs(&with_speed(&a, 2.0)), 1.0);
        let r = recolor(&seq(vec![step("blink", json!({"color": [1, 2, 3]}), &["W"], 100)], false), [9, 9, 9], crate::themes::default_palette());
        let AnimKind::Sequence { steps, .. } = &r.kind else { panic!() };
        assert_eq!(steps[0].params.0["color"], json!([9, 9, 9]));
    }
}
