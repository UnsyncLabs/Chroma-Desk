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
use crate::engine::compositor::Geometry;
use crate::engine::effects::{self, Params, Px};
use crate::error::{ApiError, Res};
use crate::fsutil::write_atomic;
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
    pub leds: BTreeMap<String, Rgb3>,
    /// Overrides the animation's default frame time.
    #[serde(default)]
    pub duration_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub pos: f32,
    pub color: Rgb3,
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
    pub kind: AnimKind,
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

/// Resolves `anim`'s LED references against a device's LED names.
pub fn resolve(anim: &Animation, names: &[String]) -> Resolved {
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
    Resolved { effect, params, matched, total }
}

type CacheKey = (String, u64, String, usize);

fn cache() -> &'static Mutex<HashMap<CacheKey, Arc<Resolved>>> {
    static C: OnceLock<Mutex<HashMap<CacheKey, Arc<Resolved>>>> = OnceLock::new();
    C.get_or_init(Mutex::default)
}

/// Resolution cached by `(animation id, updated, device, LED count)`: names are resolved once
/// per device, not once per frame.
fn cached(id: &str, updated: u64, geo: &Geometry, make: impl FnOnce() -> Option<Resolved>) -> Option<Arc<Resolved>> {
    let key = (id.to_string(), updated, geo.device_key.to_string(), geo.led_count);
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
    match cached(&anim.id, anim.updated, geo, || Some(resolve(anim, geo.led_names))) {
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
    let r = cached(id, updated, geo, || serde_json::from_value::<Animation>(snap.clone()).ok().map(|a| resolve(&a, geo.led_names)));
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
    let a: Animation = serde_json::from_value(v).map_err(|e| ApiError::new("anim_corrupt", e))?;
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
    Animation {
        schema_version: SCHEMA_VERSION,
        id: format!("builtin:{slug}"),
        name: name.into(),
        description: None,
        tags: tags.iter().map(|s| s.to_string()).collect(),
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
    Stop { pos, color: c }
}

/// Read-only presets shipped with the app: five effect presets and one example of each new kind.
pub fn builtins() -> Vec<Animation> {
    let all_keys: Vec<String> = KEY_ROWS.iter().flat_map(|r| r.iter()).map(|k| key_ref(k)).collect();
    let heart = |c: Rgb3, ms: u32| Frame { leds: all_keys.iter().map(|k| (k.clone(), c)).collect(), duration_ms: Some(ms) };
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
    }
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
        AnimKind::Effect { .. } => 1.0,
    }
}

/// A copy of `a` recoloured with `c` (keeps each colour's brightness as shading).
pub fn recolor(a: &Animation, c: Rgb3) -> Animation {
    let shade = |old: Rgb3| {
        let k = old.iter().copied().max().unwrap_or(255) as f32 / 255.0;
        [0, 1, 2].map(|i| (c[i] as f32 * k).round() as u8)
    };
    let mut out = a.clone();
    match &mut out.kind {
        AnimKind::Frames { frames, .. } => frames.iter_mut().flat_map(|f| f.leds.values_mut()).for_each(|v| *v = shade(*v)),
        AnimKind::Path { palette, .. } | AnimKind::Palette { palette, .. } => palette.iter_mut().for_each(|s| s.color = shade(s.color)),
        AnimKind::Effect { params, .. } => {
            if params.0.contains_key("color") {
                params.0.insert("color".into(), json!(c));
            }
            if let Some(Value::Array(list)) = params.0.get_mut("colors") {
                list.iter_mut().for_each(|v| *v = json!(c));
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
            kind: AnimKind::Frames {
                frames: vec![
                    Frame { leds: [("name:Key: W".to_string(), [255, 0, 0])].into(), duration_ms: Some(100) },
                    Frame { leds: [("idx:2".to_string(), [0, 255, 0]), ("name:Key: Missing".to_string(), [1, 1, 1])].into(), duration_ms: None },
                ],
                frame_ms: 100,
                loop_mode: LoopMode::Loop,
                crossfade: 0.0,
            },
        }
    }

    fn geo<'a>(key: &'a str, names: &'a [String], pos: &'a [[f32; 2]]) -> Geometry<'a> {
        Geometry { led_count: names.len(), device_key: key, led_names: names, zones: &[], positions: pos }
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
        let r = resolve(&a, &other);
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
        assert_eq!(all.len(), 9);
        let mut seen = std::collections::HashSet::new();
        // A keyboard that has every key the presets use.
        let kb: Vec<String> = KEY_ROWS.iter().flat_map(|r| r.iter()).map(|k| format!("Key: {k}")).collect();
        let pos: Vec<[f32; 2]> = (0..kb.len()).map(|i| [i as f32 / kb.len() as f32, 0.5]).collect();
        for a in &all {
            assert!(a.builtin && a.id.starts_with("builtin:") && seen.insert(a.id.clone()), "{}", a.id);
            validate(a).unwrap_or_else(|e| panic!("{}: {e}", a.name));
            let g = geo("kb", &kb, &pos);
            let mut out = vec![Px::CLEAR; kb.len()];
            render_animation(a, 0.3, &g, 1, &mut out);
            assert!(out.iter().any(|p| p.a > 0.0), "{} draws nothing at t=0.3", a.name);
            let (ok, total) = compatibility(a, &kb);
            assert_eq!(ok, total, "{} should fully match a standard keyboard", a.name);
        }
        assert_eq!(list_all(&test_dir("anim-none")).animations.len(), 9);
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
        let r = recolor(&a, [0, 0, 200]);
        let AnimKind::Frames { frames, .. } = r.kind else { panic!() };
        assert_eq!(frames[0].leds["name:Key: W"], [0, 0, 200]);
        let alert = builtins().into_iter().find(|b| b.id == "builtin:alert").unwrap();
        let AnimKind::Effect { params, .. } = recolor(&alert, [1, 2, 3]).kind else { panic!() };
        assert_eq!(params.0["colors"], json!([[1, 2, 3], [1, 2, 3]]));
        assert!((cycle_secs(&a) - 0.2).abs() < 1e-3, "{}", cycle_secs(&a));
    }
}
