//! Themes and palettes.
//!
//! Any colour of the system is either fixed (`[r, g, b]`) or a *palette slot* (`{"slot":
//! "accent"}`). Slots are resolved against the active [`Palette`] right before composing, so a
//! theme change recolours the lights without touching the scene. Fixed colours keep working
//! unchanged (serde accepts both forms).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
pub mod ctl;
pub mod gtk;
pub mod hook;
pub mod kde;
pub mod omarchy;
pub mod pywal;
pub mod sources;
pub mod watch;

#[cfg(test)]
mod theme_tests;

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub type Rgb3 = [u8; 3];

/// The standard slots every theme provides, in display order.
pub const SLOTS: [&str; 18] = [
    "background", "foreground", "accent", "primary", "secondary", "tertiary", "alert", "warn", "ok", "info", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8",
];

pub fn is_slot(s: &str) -> bool {
    SLOTS.contains(&s)
}

/// A fixed colour or a reference to a palette slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Color {
    Rgb(Rgb3),
    Slot(SlotRef),
}

/// `{"slot": "accent"}`, optionally lightened (`lighten` > 0 mixes towards white, < 0 towards black).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotRef {
    pub slot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighten: Option<f32>,
}

impl From<Rgb3> for Color {
    fn from(c: Rgb3) -> Self {
        Color::Rgb(c)
    }
}

impl Color {
    pub fn slot(name: &str) -> Color {
        Color::Slot(SlotRef { slot: name.into(), lighten: None })
    }

    /// `#ff0000`, `#f00` or `@alert`.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(slot) = s.strip_prefix('@') {
            return is_slot(slot).then(|| Color::slot(slot));
        }
        parse_hex(s).map(Color::Rgb)
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Color::Rgb(_) => true,
            Color::Slot(s) => is_slot(&s.slot) && s.lighten.is_none_or(|l| l.is_finite() && (-1.0..=1.0).contains(&l)),
        }
    }
}

/// `#ff0000`, `ff0000`, `#f00`.
pub fn parse_hex(s: &str) -> Option<Rgb3> {
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

pub fn to_hex(c: Rgb3) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn next_generation() -> u64 {
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

/// `c` mixed towards white (`k` > 0) or black (`k` < 0).
pub fn lighten(c: Rgb3, k: f32) -> Rgb3 {
    let target = if k >= 0.0 { 255.0 } else { 0.0 };
    let k = k.abs().clamp(0.0, 1.0);
    c.map(|v| (v as f32 + (target - v as f32) * k).round().clamp(0.0, 255.0) as u8)
}

/// The colours of a theme by slot name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Palette {
    #[serde(default)]
    pub name: String,
    /// `manual`, `omarchy`, `kde`, `pywal`, `gtk` or `external`.
    #[serde(default)]
    pub source: String,
    pub slots: BTreeMap<String, Rgb3>,
    /// Changes whenever the colours change; resolution caches are keyed by it.
    #[serde(default)]
    pub generation: u64,
}

impl PartialEq for Palette {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name && self.source == o.source && self.slots == o.slots
    }
}

impl Palette {
    /// A palette from `slots`: missing standard slots are derived from the ones present.
    pub fn new(name: &str, source: &str, slots: BTreeMap<String, Rgb3>) -> Palette {
        let mut p = Palette { name: name.into(), source: source.into(), slots, generation: 0 };
        p.fill_missing();
        p.generation = next_generation();
        p
    }

    /// Neutral default (used by the Manual source and as the engine's initial palette).
    pub fn default_manual() -> Palette {
        let s = |k: &str, h: &str| (k.to_string(), parse_hex(h).unwrap_or([0; 3]));
        let slots = [
            s("background", "#0b0b12"),
            s("foreground", "#e6e6f0"),
            s("accent", "#7c5cff"),
            s("primary", "#3ea6ff"),
            s("secondary", "#ff5caa"),
            s("tertiary", "#2fe0c0"),
            s("alert", "#ff3b3b"),
            s("warn", "#ffb020"),
            s("ok", "#2ee66b"),
            s("info", "#3ec9ff"),
            s("c1", "#ff3b3b"),
            s("c2", "#ff8a2b"),
            s("c3", "#ffd23f"),
            s("c4", "#2ee66b"),
            s("c5", "#2fe0c0"),
            s("c6", "#3ea6ff"),
            s("c7", "#a66cff"),
            s("c8", "#ff5caa"),
        ]
        .into_iter()
        .collect();
        Palette::new("Default", "manual", slots)
    }

    /// Fills every standard slot that is absent: from sensible relatives, else from `accent`.
    pub fn fill_missing(&mut self) {
        let accent = self.slots.get("accent").copied().or_else(|| self.slots.get("primary").copied()).unwrap_or([124, 92, 255]);
        let fb = |s: &mut Self, k: &str, v: Rgb3| {
            s.slots.entry(k.to_string()).or_insert(v);
        };
        fb(self, "accent", accent);
        let bg = self.slots.get("background").copied().unwrap_or([11, 11, 18]);
        fb(self, "background", bg);
        fb(self, "foreground", [230, 230, 240]);
        fb(self, "primary", accent);
        let primary = self.slots["primary"];
        fb(self, "secondary", lighten(primary, 0.35));
        let secondary = self.slots["secondary"];
        fb(self, "tertiary", secondary);
        fb(self, "alert", [255, 59, 59]);
        fb(self, "warn", [255, 176, 32]);
        fb(self, "ok", [46, 230, 107]);
        fb(self, "info", primary);
        let ramp = ["alert", "warn", "warn", "ok", "info", "primary", "secondary", "accent"];
        for (i, from) in ramp.iter().enumerate() {
            let v = self.slots[*from];
            fb(self, &format!("c{}", i + 1), v);
        }
    }

    pub fn get(&self, slot: &str) -> Rgb3 {
        self.slots.get(slot).or_else(|| self.slots.get("foreground")).copied().unwrap_or([255, 255, 255])
    }

    pub fn resolve(&self, c: &Color) -> Rgb3 {
        match c {
            Color::Rgb(v) => *v,
            Color::Slot(s) => {
                let base = self.get(&s.slot);
                s.lighten.map_or(base, |l| lighten(base, l))
            }
        }
    }

    /// Replaces every slot reference inside `v` (any depth) by its `[r, g, b]`.
    pub fn resolve_value(&self, v: &mut Value) {
        match v {
            Value::Object(m) => {
                if let Some(c) = slot_object(m) {
                    *v = serde_json::json!(self.resolve(&c));
                } else {
                    m.values_mut().for_each(|x| self.resolve_value(x));
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| self.resolve_value(x)),
            _ => {}
        }
    }

    pub fn resolve_map(&self, m: &mut Map<String, Value>) {
        m.values_mut().for_each(|x| self.resolve_value(x));
    }
}

/// The initial/default palette, shared (tests and callers without a live one).
pub fn default_palette() -> &'static Palette {
    static P: OnceLock<Palette> = OnceLock::new();
    P.get_or_init(Palette::default_manual)
}

/// `{"slot": "accent"[, "lighten": 0.2]}` as a [`Color`].
fn slot_object(m: &Map<String, Value>) -> Option<Color> {
    let slot = m.get("slot")?.as_str()?;
    if m.len() > 2 || (m.len() == 2 && !m.contains_key("lighten")) {
        return None;
    }
    Some(Color::Slot(SlotRef { slot: slot.to_string(), lighten: m.get("lighten").and_then(Value::as_f64).map(|f| f as f32) }))
}

/// Whether `v` contains any slot reference.
pub fn contains_slot(v: &Value) -> bool {
    match v {
        Value::Object(m) => slot_object(m).is_some() || m.values().any(contains_slot),
        Value::Array(a) => a.iter().any(contains_slot),
        _ => false,
    }
}

// ---------- the theme of a profile ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSource {
    /// The profile's own palette.
    #[default]
    Manual,
    Omarchy,
    Kde,
    Pywal,
    Gtk,
    /// Colours sent by a game or script.
    External,
}

impl ThemeSource {
    pub const ALL: [ThemeSource; 6] = [ThemeSource::Manual, ThemeSource::Omarchy, ThemeSource::Kde, ThemeSource::Pywal, ThemeSource::Gtk, ThemeSource::External];

    pub fn id(self) -> &'static str {
        match self {
            ThemeSource::Manual => "manual",
            ThemeSource::Omarchy => "omarchy",
            ThemeSource::Kde => "kde",
            ThemeSource::Pywal => "pywal",
            ThemeSource::Gtk => "gtk",
            ThemeSource::External => "external",
        }
    }

    pub fn parse(s: &str) -> Option<ThemeSource> {
        Self::ALL.into_iter().find(|x| x.id() == s.trim().to_lowercase())
    }
}

/// How a profile gets its colours.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProfileTheme {
    #[serde(default)]
    pub source: ThemeSource,
    /// Used by the Manual source (and as the fallback of the others).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_palette: Option<BTreeMap<String, Rgb3>>,
    /// Slots pinned on top of any source (e.g. `alert` always pure red).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, Rgb3>,
}

impl ProfileTheme {
    /// Checks slot names.
    pub fn validate(&self) -> Result<(), String> {
        let bad = |m: &BTreeMap<String, Rgb3>| m.keys().find(|k| !is_slot(k)).cloned();
        if let Some(k) = self.manual_palette.as_ref().and_then(bad).or_else(|| bad(&self.overrides)) {
            return Err(format!("unknown palette slot {k:?}"));
        }
        Ok(())
    }

    /// Palette of the Manual source.
    pub fn manual(&self) -> Palette {
        match &self.manual_palette {
            Some(m) => Palette::new("Manual", "manual", m.clone()),
            None => Palette::default_manual(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn color_accepts_both_forms() {
        let c: Color = serde_json::from_value(json!([1, 2, 3])).unwrap();
        assert_eq!(c, Color::Rgb([1, 2, 3]));
        let s: Color = serde_json::from_value(json!({"slot": "accent"})).unwrap();
        assert_eq!(s, Color::slot("accent"));
        assert_eq!(serde_json::to_value(Color::Rgb([9, 8, 7])).unwrap(), json!([9, 8, 7]));
        assert_eq!(serde_json::to_value(Color::slot("ok")).unwrap(), json!({"slot": "ok"}));
        assert!(serde_json::from_value::<Color>(json!({"slot": "x", "bogus": 1})).is_err());
    }

    #[test]
    fn parse_specs() {
        assert_eq!(Color::parse("#ff0000"), Some(Color::Rgb([255, 0, 0])));
        assert_eq!(Color::parse("@alert"), Some(Color::slot("alert")));
        assert_eq!(Color::parse("@nope"), None);
        assert_eq!(Color::parse("red"), None);
        assert_eq!(to_hex([1, 171, 255]), "#01abff");
    }

    #[test]
    fn resolves_slots_in_any_depth_and_lightens() {
        let mut p = Palette::default_manual();
        p.slots.insert("accent".into(), [100, 0, 0]);
        let mut v = json!({"colors": [{"slot": "accent"}, [1, 2, 3]], "palette": [{"pos": 0.0, "color": {"slot": "accent", "lighten": 1.0}}], "leds": {"3": {"slot": "ok"}}});
        assert!(contains_slot(&v));
        p.resolve_value(&mut v);
        assert!(!contains_slot(&v));
        assert_eq!(v["colors"][0], json!([100, 0, 0]));
        assert_eq!(v["colors"][1], json!([1, 2, 3]));
        assert_eq!(v["palette"][0]["color"], json!([255, 255, 255]));
        assert_eq!(v["leds"]["3"], json!(p.get("ok")));
        // A map that merely has a "slot" key next to other data is not a reference.
        let other = json!({"slot": "accent", "x": 1});
        assert!(!contains_slot(&other));
    }

    #[test]
    fn missing_slots_are_derived() {
        let mut m = BTreeMap::new();
        m.insert("accent".to_string(), [10, 20, 30]);
        let p = Palette::new("t", "manual", m);
        for s in SLOTS {
            assert!(p.slots.contains_key(s), "{s}");
        }
        assert_eq!(p.get("primary"), [10, 20, 30]);
        assert_eq!(p.get("c8"), [10, 20, 30]);
        assert_ne!(p.generation, Palette::default_manual().generation);
    }

    #[test]
    fn theme_validation_and_source_ids() {
        let mut t = ProfileTheme::default();
        assert!(t.validate().is_ok());
        t.overrides.insert("nope".into(), [0, 0, 0]);
        assert!(t.validate().is_err());
        for s in ThemeSource::ALL {
            assert_eq!(ThemeSource::parse(s.id()), Some(s));
        }
        assert_eq!(ThemeSource::parse("Omarchy"), Some(ThemeSource::Omarchy));
    }
}
