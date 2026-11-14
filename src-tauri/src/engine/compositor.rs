//! Layer compositor: stacks effect layers (bottom to top) with masks, opacity and blend modes.

use super::effects::{self, Params, Pos, Px};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Blend {
    #[default]
    Normal,
    Add,
    Multiply,
    Screen,
}

/// What a layer is for in the Quick tab: the base look of a device (one at a time) or an overlay
/// that stacks on top of any theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    #[default]
    Theme,
    Effect,
}

impl Role {
    /// Role of a plain engine effect: transparent overlays are effects, everything else a base look.
    pub fn of_effect(effect: &str) -> Role {
        match effect {
            "glitch" | "starlight" | "strobe" | "ripple" | "blink" | "orbit" | "sequence" | "meter" => Role::Effect,
            _ => Role::Theme,
        }
    }
}

/// Which LEDs a layer affects.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Mask {
    #[default]
    All,
    Zone { id: usize },
    Leds { leds: Vec<usize> },
}

fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub blend: Blend,
    #[serde(default)]
    pub mask: Mask,
    /// Effect id (see `effects::effect_defs`).
    pub effect: String,
    #[serde(default)]
    pub params: Params,
    /// Theme or effect; `None` (older scenes) is inferred from the effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

impl Layer {
    pub fn role(&self) -> Role {
        self.role.unwrap_or_else(|| Role::of_effect(&self.effect))
    }
}

/// What the compositor needs to know about a device.
pub struct Geometry<'a> {
    pub led_count: usize,
    /// Identity key of the device (animation resolution cache).
    pub device_key: &'a str,
    /// OpenRGB LED names (resolve portable `LedRef`s).
    pub led_names: &'a [String],
    /// `(zone id, first LED, LED count)`.
    pub zones: &'a [(usize, usize, usize)],
    pub positions: &'a [Pos],
    /// Active palette (animations resolve their slot colours with it).
    pub palette: &'a crate::themes::Palette,
}

impl Mask {
    pub fn coverage(&self, geo: &Geometry) -> Option<Vec<bool>> {
        match self {
            Mask::All => None,
            Mask::Zone { id } => {
                let mut v = vec![false; geo.led_count];
                for &(zid, off, len) in geo.zones {
                    if zid == *id {
                        v.iter_mut().skip(off).take(len).for_each(|b| *b = true);
                    }
                }
                Some(v)
            }
            Mask::Leds { leds } => {
                let mut v = vec![false; geo.led_count];
                for &l in leds {
                    if let Some(b) = v.get_mut(l) {
                        *b = true;
                    }
                }
                Some(v)
            }
        }
    }
}

fn blend_channel(mode: Blend, base: f32, src: f32) -> f32 {
    match mode {
        Blend::Normal => src,
        Blend::Add => (base + src).min(255.0),
        Blend::Multiply => base * src / 255.0,
        Blend::Screen => 255.0 - (255.0 - base) * (255.0 - src) / 255.0,
    }
}

/// Blends `src` (one scratch layer) onto `buf` through an optional LED mask.
pub fn blend_onto(buf: &mut [Px], src: &[Px], mask: Option<&[bool]>, opacity: f32, blend: Blend) {
    let opacity = opacity.clamp(0.0, 1.0);
    for (i, (dst, src)) in buf.iter_mut().zip(src).enumerate() {
        if mask.is_some_and(|m| !m.get(i).copied().unwrap_or(false)) {
            continue;
        }
        let a = src.a * opacity;
        if a <= 0.0 {
            continue;
        }
        dst.r += (blend_channel(blend, dst.r, src.r) - dst.r) * a;
        dst.g += (blend_channel(blend, dst.g, src.g) - dst.g) * a;
        dst.b += (blend_channel(blend, dst.b, src.b) - dst.b) * a;
    }
}

/// Draws every enabled layer (index 0 = bottom) on top of `buf`.
pub fn composite_onto(buf: &mut [Px], layers: &[Layer], geo: &Geometry, t: f32, seed: u32) {
    let mut scratch = vec![Px::CLEAR; buf.len()];
    for (li, layer) in layers.iter().enumerate() {
        if !layer.enabled || layer.opacity <= 0.0 {
            continue;
        }
        let seed = seed.wrapping_add(li as u32);
        if layer.effect == "animation" {
            crate::animations::render_layer(&layer.params, t, geo, seed, &mut scratch);
        } else {
            effects::render(&layer.effect, &layer.params, t, geo.positions, seed, &mut scratch);
        }
        blend_onto(buf, &scratch, layer.mask.coverage(geo).as_deref(), layer.opacity, layer.blend);
    }
}

/// Opaque black canvas of `n` LEDs.
pub fn black(n: usize) -> Vec<Px> {
    vec![Px::BLACK; n]
}

/// Final 8-bit colours, scaled by `brightness` (0..1).
pub fn finalize(buf: &[Px], brightness: f32) -> Vec<[u8; 3]> {
    let k = brightness.clamp(0.0, 1.0);
    let q = |v: f32| (v * k).round().clamp(0.0, 255.0) as u8;
    buf.iter().map(|p| [q(p.r), q(p.g), q(p.b)]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn solid(id: &str, c: [u8; 3]) -> Layer {
        Layer {
            id: id.into(),
            name: id.into(),
            enabled: true,
            opacity: 1.0,
            blend: Blend::Normal,
            mask: Mask::All,
            effect: "solid".into(),
            params: effects::params(json!({ "color": c })),
            role: None,
        }
    }

    fn run(layers: &[Layer], n: usize, zones: &[(usize, usize, usize)]) -> Vec<[u8; 3]> {
        let pos = vec![[0.5, 0.5]; n];
        let geo = Geometry { led_count: n, device_key: "t", led_names: &[], zones, positions: &pos, palette: crate::themes::default_palette() };
        let mut buf = black(n);
        composite_onto(&mut buf, layers, &geo, 0.0, 1);
        finalize(&buf, 1.0)
    }

    #[test]
    fn empty_stack_is_black() {
        assert_eq!(run(&[], 2, &[]), vec![[0, 0, 0]; 2]);
    }

    #[test]
    fn top_layer_wins_with_normal_blend() {
        let out = run(&[solid("a", [255, 0, 0]), solid("b", [0, 0, 255])], 1, &[]);
        assert_eq!(out[0], [0, 0, 255]);
    }

    #[test]
    fn opacity_mixes() {
        let mut top = solid("b", [200, 0, 0]);
        top.opacity = 0.5;
        let out = run(&[solid("a", [0, 100, 0]), top], 1, &[]);
        assert_eq!(out[0], [100, 50, 0]);
    }

    #[test]
    fn disabled_layers_are_skipped() {
        let mut top = solid("b", [9, 9, 9]);
        top.enabled = false;
        assert_eq!(run(&[solid("a", [1, 2, 3]), top], 1, &[])[0], [1, 2, 3]);
    }

    #[test]
    fn blend_modes() {
        let base = solid("a", [100, 100, 100]);
        let mut top = solid("b", [200, 100, 50]);
        top.blend = Blend::Add;
        assert_eq!(run(&[base.clone(), top.clone()], 1, &[])[0], [255, 200, 150]);
        top.blend = Blend::Multiply;
        assert_eq!(run(&[base.clone(), top.clone()], 1, &[])[0], [78, 39, 20]);
        top.blend = Blend::Screen;
        // 255 - (155 * 55)/255 = 221.56
        assert_eq!(run(&[base, top], 1, &[])[0][0], 222);
    }

    #[test]
    fn led_mask_limits_layer() {
        let mut top = solid("b", [255, 0, 0]);
        top.mask = Mask::Leds { leds: vec![1, 3, 99] };
        let out = run(&[top], 4, &[]);
        assert_eq!(out, vec![[0, 0, 0], [255, 0, 0], [0, 0, 0], [255, 0, 0]]);
    }

    #[test]
    fn zone_mask_uses_zone_ranges() {
        let mut top = solid("b", [0, 255, 0]);
        top.mask = Mask::Zone { id: 1 };
        let out = run(&[top], 5, &[(0, 0, 2), (1, 2, 2)]);
        assert_eq!(out.iter().filter(|c| c[1] == 255).count(), 2);
        assert_eq!(out[2], [0, 255, 0]);
        assert_eq!(out[4], [0, 0, 0]);
    }

    #[test]
    fn paint_layer_over_background_keeps_unpainted() {
        let paint = Layer {
            effect: "paint".into(),
            params: effects::params(json!({"leds": {"0": [255, 0, 0]}})),
            ..solid("p", [0, 0, 0])
        };
        let out = run(&[solid("bg", [0, 0, 80]), paint], 2, &[]);
        assert_eq!(out, vec![[255, 0, 0], [0, 0, 80]]);
    }

    #[test]
    fn brightness_scales_final() {
        let buf = vec![Px { r: 200.0, g: 100.0, b: 50.0, a: 1.0 }];
        assert_eq!(finalize(&buf, 0.5), vec![[100, 50, 25]]);
        assert_eq!(finalize(&buf, 2.0), vec![[200, 100, 50]]);
    }

    #[test]
    fn layer_serde_defaults() {
        let l: Layer = serde_json::from_value(json!({"id": "x", "name": "n", "effect": "solid"})).unwrap();
        assert!(l.enabled && l.opacity == 1.0 && l.blend == Blend::Normal && l.mask == Mask::All);
    }
}
