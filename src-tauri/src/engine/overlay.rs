//! Temporary overlays: an animation that plays on top of the scene for a while (notifications,
//! "something happened" flashes) and then disappears. Overlays stack, carry a priority (higher
//! is drawn on top), may be limited to some keys/LEDs and expire by themselves.
//!
//! Only devices in Effects mode are affected: devices running a native hardware mode are not
//! owned by the engine and are left alone.

use super::DevInfo;
use super::compositor::{self, Blend, Geometry};
use super::effects::{Pos, Px};
use crate::animations::{self, Animation};
use crate::error::{ApiError, Res};
use std::sync::Arc;
use std::time::Instant;

/// Active overlays are capped so a runaway script cannot pile them up forever.
pub const MAX_OVERLAYS: usize = 32;

#[derive(Debug, Clone)]
pub struct Overlay {
    pub id: u64,
    pub anim: Animation,
    /// Device key or part of its name (case-insensitive); `None` = every device.
    pub device: Option<String>,
    /// Key names (`W`, `Left Shift`, `idx:41`); empty = the whole device.
    pub keys: Vec<String>,
    pub start: Instant,
    pub until: Instant,
    pub priority: i32,
}

#[derive(Default)]
pub struct Overlays {
    list: Vec<Arc<Overlay>>,
    next_id: u64,
}

impl Overlays {
    /// Drops expired overlays.
    pub fn prune(&mut self, now: Instant) {
        self.list.retain(|o| o.until > now);
    }

    /// Adds an overlay built by `make(id)`; refuses when `MAX_OVERLAYS` are already active.
    pub fn add(&mut self, now: Instant, make: impl FnOnce(u64) -> Overlay) -> Res<u64> {
        self.prune(now);
        if self.list.len() >= MAX_OVERLAYS {
            return Err(ApiError::new("overlay_limit", MAX_OVERLAYS));
        }
        self.next_id += 1;
        let ov = make(self.next_id);
        let id = ov.id;
        self.list.push(Arc::new(ov));
        Ok(id)
    }

    /// Active overlays in drawing order (lowest priority first, older first).
    pub fn active(&mut self, now: Instant) -> Vec<Arc<Overlay>> {
        self.prune(now);
        let mut v = self.list.clone();
        v.sort_by_key(|o| (o.priority, o.id));
        v
    }

    /// Removes the overlays whose animation id starts with `prefix` (replace-on-refire); returns how many.
    pub fn remove_prefix(&mut self, prefix: &str) -> usize {
        let before = self.list.len();
        self.list.retain(|o| !o.anim.id.starts_with(prefix));
        before - self.list.len()
    }

    /// Whether an overlay of `prefix` is still showing at `now`.
    pub fn any_prefix(&self, prefix: &str, now: Instant) -> bool {
        self.list.iter().any(|o| o.until > now && o.anim.id.starts_with(prefix))
    }

    /// Removes everything; returns how many were active.
    pub fn clear(&mut self) -> usize {
        std::mem::take(&mut self.list).len()
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// Whether `filter` (key or name fragment) selects `dev`.
pub fn device_matches(filter: Option<&str>, dev: &DevInfo) -> bool {
    match filter {
        None => true,
        Some(f) => {
            let f = f.trim().to_lowercase();
            f.is_empty() || dev.key.to_lowercase() == f || dev.name.to_lowercase().contains(&f)
        }
    }
}

/// LEDs the key names select on a device: `W` matches `Key: W`, `Left Shift` matches
/// `Key: Left Shift` (case-insensitive), `name:<exact>` and `idx:<n>` are exact.
pub fn resolve_keys(keys: &[String], names: &[String]) -> Vec<usize> {
    let mut out = Vec::new();
    for k in keys {
        let k = k.trim();
        if let Some(i) = k.strip_prefix("idx:") {
            if let Ok(i) = i.trim().parse::<usize>()
                && i < names.len()
            {
                out.push(i);
            }
            continue;
        }
        let exact = k.strip_prefix("name:");
        let want = exact.unwrap_or(k).to_lowercase();
        for (i, n) in names.iter().enumerate() {
            if n.is_empty() {
                continue;
            }
            let hit = if exact.is_some() { n.to_lowercase() == want } else { n.to_lowercase() == want || n.strip_prefix("Key: ").is_some_and(|s| s.to_lowercase() == want) };
            if hit {
                out.push(i);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Draws the active overlays onto `buf` (a device already composed from its layers).
pub fn compose(buf: &mut [Px], overlays: &[Arc<Overlay>], dev: &DevInfo, positions: &[Pos], now: Instant, palette: &crate::themes::Palette) {
    if overlays.is_empty() {
        return;
    }
    let geo = Geometry { led_count: dev.led_count, device_key: &dev.key, led_names: &dev.led_names, zones: &dev.zones, positions, palette };
    let mut scratch = vec![Px::CLEAR; buf.len()];
    for ov in overlays {
        if !device_matches(ov.device.as_deref(), dev) || now < ov.start || now >= ov.until {
            continue;
        }
        let mask = if ov.keys.is_empty() {
            None
        } else {
            let leds = resolve_keys(&ov.keys, &dev.led_names);
            if leds.is_empty() {
                continue;
            }
            let mut m = vec![false; buf.len()];
            for l in leds {
                if let Some(b) = m.get_mut(l) {
                    *b = true;
                }
            }
            Some(m)
        };
        let t = now.saturating_duration_since(ov.start).as_secs_f32();
        animations::render_animation(&ov.anim, t, &geo, ov.id as u32, &mut scratch);
        compositor::blend_onto(buf, &scratch, mask.as_deref(), 1.0, Blend::Normal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::{AnimKind, Frame, LoopMode, Target};
    use std::time::Duration;

    fn dev() -> DevInfo {
        let names: Vec<String> = ["Key: W", "Key: A", "Key: Left Shift", ""].iter().map(|s| s.to_string()).collect();
        DevInfo {
            index: 0,
            key: "kb#1".into(),
            name: "Razer Test Keyboard".into(),
            device_type: "Keyboard".into(),
            led_count: names.len(),
            led_names: names,
            zones: vec![(0, 0, 4)],
            default_positions: vec![[0.5, 0.5]; 4],
        }
    }

    fn solid_anim(id: &str, c: [u8; 3]) -> Animation {
        Animation {
            schema_version: 1,
            id: id.into(),
            name: id.into(),
            description: None,
            tags: vec![],
            created: 0,
            updated: 1,
            target: Target::default(),
            builtin: false,
            kind: AnimKind::Frames {
                frames: vec![Frame { leds: (0..4).map(|i| (format!("idx:{i}"), crate::themes::Color::Rgb(c))).collect(), duration_ms: Some(1000) }],
                frame_ms: 1000,
                loop_mode: LoopMode::Loop,
                crossfade: 0.0,
            },
        }
    }

    fn ov(id: u64, anim: Animation, now: Instant, secs: f32, priority: i32) -> Overlay {
        Overlay { id, anim, device: None, keys: vec![], start: now, until: now + Duration::from_secs_f32(secs), priority }
    }

    #[test]
    fn overlays_expire() {
        let t0 = Instant::now();
        let mut o = Overlays::default();
        o.add(t0, |id| ov(id, solid_anim("a", [1, 1, 1]), t0, 1.0, 0)).unwrap();
        o.add(t0, |id| ov(id, solid_anim("b", [1, 1, 1]), t0, 3.0, 0)).unwrap();
        assert_eq!(o.active(t0 + Duration::from_millis(500)).len(), 2);
        assert_eq!(o.active(t0 + Duration::from_millis(1500)).len(), 1);
        assert_eq!(o.active(t0 + Duration::from_secs(4)).len(), 0);
        assert!(o.is_empty());
    }

    #[test]
    fn limit_is_enforced_and_expired_ones_make_room() {
        let t0 = Instant::now();
        let mut o = Overlays::default();
        for _ in 0..MAX_OVERLAYS {
            o.add(t0, |id| ov(id, solid_anim("a", [1, 1, 1]), t0, 1.0, 0)).unwrap();
        }
        let err = o.add(t0, |id| ov(id, solid_anim("a", [1, 1, 1]), t0, 1.0, 0)).unwrap_err();
        assert_eq!(err.code, "overlay_limit");
        assert_eq!(o.len(), MAX_OVERLAYS);
        o.add(t0 + Duration::from_secs(2), |id| ov(id, solid_anim("a", [1, 1, 1]), t0, 5.0, 0)).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o.clear(), 1);
    }

    #[test]
    fn priority_decides_who_is_on_top() {
        let t0 = Instant::now();
        let mut o = Overlays::default();
        // High priority added first, low priority later: high still wins.
        o.add(t0, |id| ov(id, solid_anim("hi", [0, 255, 0]), t0, 5.0, 10)).unwrap();
        o.add(t0, |id| ov(id, solid_anim("lo", [255, 0, 0]), t0, 5.0, 1)).unwrap();
        let active = o.active(t0);
        let mut buf = compositor::black(4);
        compose(&mut buf, &active, &dev(), &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        assert_eq!((buf[0].r, buf[0].g), (0.0, 255.0));
        // Same priority: the newer one is on top.
        let mut o = Overlays::default();
        o.add(t0, |id| ov(id, solid_anim("first", [0, 255, 0]), t0, 5.0, 0)).unwrap();
        o.add(t0, |id| ov(id, solid_anim("second", [255, 0, 0]), t0, 5.0, 0)).unwrap();
        let mut buf = compositor::black(4);
        compose(&mut buf, &o.active(t0), &dev(), &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        assert_eq!(buf[1].r, 255.0);
    }

    #[test]
    fn keys_limit_the_overlay_and_missing_keys_skip_the_device() {
        let t0 = Instant::now();
        let mut limited = ov(1, solid_anim("k", [255, 0, 0]), t0, 5.0, 0);
        limited.keys = vec!["w".into(), "Left Shift".into(), "Nope".into()];
        let mut buf = compositor::black(4);
        compose(&mut buf, &[Arc::new(limited)], &dev(), &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        let lit: Vec<bool> = buf.iter().map(|p| p.r > 0.0).collect();
        assert_eq!(lit, vec![true, false, true, false]);
        let mut none = ov(2, solid_anim("k2", [255, 0, 0]), t0, 5.0, 0);
        none.keys = vec!["F13".into()];
        let mut buf = compositor::black(4);
        compose(&mut buf, &[Arc::new(none)], &dev(), &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        assert!(buf.iter().all(|p| p.r == 0.0), "no matching key: nothing is drawn");
    }

    #[test]
    fn device_filter_and_expiry_in_compose() {
        let t0 = Instant::now();
        let mut o = ov(1, solid_anim("d", [255, 0, 0]), t0, 1.0, 0);
        o.device = Some("mouse".into());
        let mut buf = compositor::black(4);
        compose(&mut buf, &[Arc::new(o.clone())], &dev(), &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        assert!(buf.iter().all(|p| p.r == 0.0), "other device");
        o.device = Some("razer".into());
        compose(&mut buf, &[Arc::new(o.clone())], &dev(), &[[0.5, 0.5]; 4], t0, crate::themes::default_palette());
        assert!(buf.iter().all(|p| p.r == 255.0));
        let mut buf = compositor::black(4);
        compose(&mut buf, &[Arc::new(o)], &dev(), &[[0.5, 0.5]; 4], t0 + Duration::from_secs(2), crate::themes::default_palette());
        assert!(buf.iter().all(|p| p.r == 0.0), "expired");
    }

    #[test]
    fn key_resolution() {
        let names = dev().led_names;
        assert_eq!(resolve_keys(&["W".into(), "a".into()], &names), vec![0, 1]);
        assert_eq!(resolve_keys(&["Key: W".into(), "idx:3".into(), "idx:9".into()], &names), vec![0, 3]);
        assert_eq!(resolve_keys(&["name:Key: Left Shift".into(), "name:left shift".into()], &names), vec![2]);
        assert!(resolve_keys(&["".into()], &names).is_empty(), "unnamed LEDs never match by name");
    }
}
