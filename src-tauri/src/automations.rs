//! The scheduler behind the time- and measurement-based triggers of an automation: *always on*,
//! *every N seconds for D seconds* and *threshold* (a metric stays above or below a value).
//!
//! It is polled from the engine loop a few times a second and does nothing at all (no sampling,
//! no allocation beyond an empty map) while no enabled automation uses one of those triggers.
//! Desktop, Omarchy, AI and command triggers are event driven and live in `triggers`.

use crate::engine::Engine;
use crate::profiles::{NamedTrigger, Threshold};
use crate::sysmon::{self, Metric};
use crate::triggers::{self, TriggerSpec};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

/// How often the scheduler looks at the automations.
pub const POLL: Duration = Duration::from_millis(200);
/// An "always on" overlay lasts this long and is then started again.
const ALWAYS_SECS: f64 = 3600.0;
/// After a failed start (no device in Effects mode yet) wait before trying again.
const RETRY: Duration = Duration::from_secs(2);

/// A duration from user data: NaN, infinite or negative values must not panic the engine loop.
fn secs(v: f32) -> Duration {
    Duration::from_secs_f32(if v.is_finite() { v.clamp(0.0, 86_400.0) } else { 0.0 })
}

/// State of one threshold rule.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdState {
    since: Option<Instant>,
    armed: bool,
    last_fire: Option<Instant>,
}

impl Default for ThresholdState {
    fn default() -> Self {
        ThresholdState { since: None, armed: true, last_fire: None }
    }
}

impl ThresholdState {
    /// Feeds a reading; true when the rule fires now. It needs the condition to hold for `for_secs`,
    /// then stays quiet until the reading has moved back past `hysteresis` and `cooldown_secs` passed.
    pub fn update(&mut self, cfg: &Threshold, value: Option<f32>, now: Instant) -> bool {
        let Some(v) = value else {
            self.since = None;
            return false;
        };
        let gt = cfg.op != "lt";
        let holds = if gt { v > cfg.value } else { v < cfg.value };
        let released = if gt { v <= cfg.value - cfg.hysteresis } else { v >= cfg.value + cfg.hysteresis };
        if !self.armed {
            if released {
                self.armed = true;
            }
            self.since = None;
            return false;
        }
        if !holds {
            self.since = None;
            return false;
        }
        let since = *self.since.get_or_insert(now);
        let cooled = self.last_fire.is_none_or(|l| now.saturating_duration_since(l) >= secs(cfg.cooldown_secs));
        if now.saturating_duration_since(since) >= secs(cfg.for_secs) && cooled {
            self.armed = false;
            self.since = None;
            self.last_fire = Some(now);
            return true;
        }
        false
    }
}

/// Whether the automation has a trigger this scheduler runs.
pub fn is_scheduled(n: &NamedTrigger) -> bool {
    n.enabled && (n.sources.always || n.sources.every.is_some() || n.sources.threshold.is_some())
}

struct Entry {
    /// The automation as it was when this state was created: a change restarts it.
    n: NamedTrigger,
    next_every: Option<Instant>,
    threshold: ThresholdState,
    retry_at: Option<Instant>,
}

#[derive(Default)]
pub struct Scheduler {
    entries: HashMap<String, Entry>,
    next_poll: Option<Instant>,
}

fn every_dur(v: f64) -> Duration {
    Duration::from_secs_f64(if v.is_finite() { v.clamp(1.0, 86_400.0) } else { 60.0 })
}

fn prefix(name: &str) -> String {
    format!("notif:{name}#")
}

impl Scheduler {
    /// Looks at the automations if [`POLL`] has passed. Cheap to call every engine tick.
    pub fn tick(&mut self, engine: &Engine, dir: &Path, now: Instant) {
        if self.next_poll.is_some_and(|at| now < at) {
            return;
        }
        self.next_poll = Some(now + POLL);
        let all = engine.notifications();
        if self.entries.is_empty() && !all.values().any(is_scheduled) {
            return;
        }
        // Gone, disabled, edited: forget the state and take an always-on overlay down.
        let stale: Vec<String> = self.entries.iter().filter(|(k, e)| all.get(*k).is_none_or(|n| !is_scheduled(n) || *n != e.n)).map(|(k, _)| k.clone()).collect();
        // A pause only lasts while the automation is unchanged and enabled.
        engine.paused_always.lock().unwrap().retain(|k, p| all.get(k) == Some(p));
        for k in stale {
            if let Some(e) = self.entries.remove(&k)
                && e.n.sources.always
            {
                engine.overlays.lock().unwrap().remove_prefix(&prefix(&k));
            }
        }
        for (name, n) in all.iter().filter(|(_, n)| is_scheduled(n)) {
            let e = self.entries.entry(name.clone()).or_insert_with(|| Entry {
                n: n.clone(),
                next_every: n.sources.every.as_ref().map(|ev| now + every_dur(ev.every_secs)),
                threshold: ThresholdState::default(),
                retry_at: None,
            });
            let spec = TriggerSpec { animation: name.clone(), ..Default::default() };
            if e.retry_at.is_some_and(|at| now < at) {
                continue;
            }
            e.retry_at = None;
            let start = |secs: Option<f64>, e: &mut Entry| {
                if triggers::start_with(engine, dir, &spec, now, secs).is_err() {
                    e.retry_at = Some(now + RETRY);
                }
            };
            let paused = engine.paused_always.lock().unwrap().contains_key(name);
            if n.sources.always && !paused && !engine.overlays.lock().unwrap().any_prefix(&prefix(name), now) {
                start(Some(ALWAYS_SECS), e);
            }
            if let (Some(ev), Some(at)) = (&n.sources.every, e.next_every)
                && now >= at
            {
                let every = every_dur(ev.every_secs);
                e.next_every = Some(if now.saturating_duration_since(at) > every { now + every } else { at + every });
                start(Some(ev.for_secs), e);
            }
            if let Some(th) = &n.sources.threshold
                && let Some(m) = Metric::parse(&th.metric)
            {
                // Asking keeps the sampler awake for as long as this rule exists.
                let value = sysmon::reading(m).map(|r| r.value);
                if e.threshold.update(th, value, now) {
                    start(None, e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::{AnimKind, Animation, Step, Target};
    use crate::engine::{DevInfo, DeviceScene, SceneMode};
    use crate::profiles::{Every, Sources};
    use serde_json::json;

    fn th(value: f32, for_secs: f32, hyst: f32, cool: f32, op: &str) -> Threshold {
        Threshold { metric: "cpu.temp".into(), op: op.into(), value, for_secs, hysteresis: hyst, cooldown_secs: cool }
    }

    #[test]
    fn threshold_needs_the_condition_to_hold_then_rearms_with_hysteresis_and_cooldown() {
        let cfg = th(80.0, 5.0, 3.0, 20.0, "gt");
        let mut s = ThresholdState::default();
        let t0 = Instant::now();
        let at = |secs: u64| t0 + Duration::from_secs(secs);
        assert!(!s.update(&cfg, Some(85.0), at(0)), "just crossed");
        assert!(!s.update(&cfg, Some(85.0), at(4)));
        assert!(!s.update(&cfg, Some(70.0), at(4)), "dipped: the timer restarts");
        assert!(!s.update(&cfg, Some(85.0), at(5)));
        assert!(!s.update(&cfg, Some(85.0), at(9)));
        assert!(s.update(&cfg, Some(85.0), at(10)), "held for 5 s");
        assert!(!s.update(&cfg, Some(90.0), at(60)), "disarmed while still above");
        assert!(!s.update(&cfg, Some(79.0), at(61)), "below the limit but inside the hysteresis band");
        assert!(!s.update(&cfg, Some(85.0), at(62)), "still disarmed");
        assert!(!s.update(&cfg, Some(76.0), at(63)), "released");
        assert!(!s.update(&cfg, Some(85.0), at(64)));
        assert!(s.update(&cfg, Some(85.0), at(70)), "re-armed, held again, cooldown over");
        // Cooldown blocks a quick second fire even when re-armed and held.
        let cfg = th(80.0, 0.0, 1.0, 100.0, "gt");
        let mut s = ThresholdState::default();
        assert!(s.update(&cfg, Some(90.0), at(0)));
        assert!(!s.update(&cfg, Some(70.0), at(1)));
        assert!(!s.update(&cfg, Some(90.0), at(2)), "cooldown");
        assert!(s.update(&cfg, Some(90.0), at(101)));
        // No reading, no fire; `lt` works the other way round.
        assert!(!s.update(&cfg, None, at(300)));
        let low = th(20.0, 0.0, 5.0, 0.0, "lt");
        let mut s = ThresholdState::default();
        assert!(!s.update(&low, Some(30.0), at(0)));
        assert!(s.update(&low, Some(10.0), at(1)));
        assert!(!s.update(&low, Some(22.0), at(2)) && !s.update(&low, Some(10.0), at(3)), "needs 25 to re-arm");
        assert!(!s.update(&low, Some(26.0), at(4)) && s.update(&low, Some(10.0), at(5)));
    }

    fn engine() -> Engine {
        let e = Engine::default();
        let names: Vec<String> = ["Key: W", "Key: A"].iter().map(|s| s.to_string()).collect();
        e.devs.lock().unwrap().push(DevInfo {
            index: 0,
            key: "kb#1".into(),
            name: "Test Keyboard".into(),
            device_type: "Keyboard".into(),
            led_count: 2,
            led_names: names,
            zones: vec![(0, 0, 2)],
            default_positions: vec![[0.5, 0.5]; 2],
        });
        e.scene.lock().unwrap().devices.insert("kb#1".into(), DeviceScene { mode: SceneMode::Effects, ..Default::default() });
        e
    }

    fn auto(sources: Sources) -> NamedTrigger {
        let seq = Animation {
            schema_version: 1,
            id: String::new(),
            name: "inline".into(),
            description: None,
            tags: vec![],
            created: 0,
            updated: 0,
            target: Target::default(),
            builtin: false,
            role: None,
            kind: AnimKind::Sequence { steps: vec![Step { effect: "blink".into(), params: crate::engine::effects::params(json!({"color": [255, 0, 0]})), keys: vec!["name:Key: W".into()], duration_ms: 1000, anim: None }], looped: false },
        };
        NamedTrigger { sequence: Some(Box::new(seq)), sources, ..NamedTrigger::new("") }
    }

    fn active(e: &Engine, name: &str, now: Instant) -> bool {
        e.overlays.lock().unwrap().any_prefix(&prefix(name), now)
    }

    #[test]
    fn always_keeps_one_overlay_running_and_follows_edits() {
        let e = engine();
        let dir = crate::fsutil::test_dir("auto-always");
        let mut sch = Scheduler::default();
        let t0 = Instant::now();
        e.global_triggers.lock().unwrap().insert("mon".into(), auto(Sources { always: true, ..Default::default() }));
        sch.tick(&e, &dir, t0);
        assert!(active(&e, "mon", t0));
        assert_eq!(e.overlays.lock().unwrap().len(), 1);
        // Polled again: still exactly one overlay.
        sch.tick(&e, &dir, t0 + Duration::from_secs(1));
        assert_eq!(e.overlays.lock().unwrap().len(), 1);
        // Even a one-second sequence keeps looping: the overlay lasts an hour.
        assert!(active(&e, "mon", t0 + Duration::from_secs(3000)));
        // Disabling takes it down at the next poll.
        let mut off = auto(Sources { always: true, ..Default::default() });
        off.enabled = false;
        e.global_triggers.lock().unwrap().insert("mon".into(), off);
        sch.tick(&e, &dir, t0 + Duration::from_secs(2));
        assert!(!active(&e, "mon", t0 + Duration::from_secs(2)));
        assert!(sch.entries.is_empty());
        // Back on, then removed altogether.
        e.global_triggers.lock().unwrap().insert("mon".into(), auto(Sources { always: true, ..Default::default() }));
        sch.tick(&e, &dir, t0 + Duration::from_secs(3));
        assert!(active(&e, "mon", t0 + Duration::from_secs(3)));
        e.global_triggers.lock().unwrap().clear();
        sch.tick(&e, &dir, t0 + Duration::from_secs(4));
        assert_eq!(e.overlays.lock().unwrap().len(), 0);
    }

    #[test]
    fn stop_all_pauses_always_on_until_resumed_or_edited() {
        let e = engine();
        let dir = crate::fsutil::test_dir("auto-pause");
        let mut sch = Scheduler::default();
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        e.global_triggers.lock().unwrap().insert("mon".into(), auto(Sources { always: true, ..Default::default() }));
        sch.tick(&e, &dir, at(0));
        assert!(active(&e, "mon", at(0)));
        assert_eq!(triggers::stop_all(&e), 1);
        assert_eq!(triggers::paused_always(&e), vec!["mon".to_string()]);
        sch.tick(&e, &dir, at(1));
        sch.tick(&e, &dir, at(2));
        assert!(!active(&e, "mon", at(2)), "stays stopped");
        assert_eq!(triggers::resume_always(&e), 1);
        sch.tick(&e, &dir, at(3));
        assert!(active(&e, "mon", at(3)), "resumed");
        // Editing a paused automation (here: a different colour) un-pauses it.
        triggers::stop_all(&e);
        let mut edited = auto(Sources { always: true, ..Default::default() });
        edited.color = Some("#ff0000".into());
        e.global_triggers.lock().unwrap().insert("mon".into(), edited);
        sch.tick(&e, &dir, at(4));
        assert!(triggers::paused_always(&e).is_empty());
        assert!(active(&e, "mon", at(4)));
    }

    #[test]
    fn nonsense_numbers_from_a_hand_edited_profile_do_not_panic() {
        let cfg = Threshold { metric: "cpu.temp".into(), op: "gt".into(), value: 1.0, for_secs: f32::NAN, hysteresis: 0.0, cooldown_secs: -5.0 };
        let mut s = ThresholdState::default();
        let t0 = Instant::now();
        assert!(s.update(&cfg, Some(9.0), t0));
        assert!(!s.update(&cfg, Some(9.0), t0 + Duration::from_secs(1)));
        assert_eq!(every_dur(f64::NAN), Duration::from_secs(60));
        assert_eq!(every_dur(-3.0), Duration::from_secs(1));
    }

    #[test]
    fn every_fires_on_schedule_for_the_given_time() {
        let e = engine();
        let dir = crate::fsutil::test_dir("auto-every");
        let mut sch = Scheduler::default();
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        e.global_triggers.lock().unwrap().insert("tick".into(), auto(Sources { every: Some(Every { every_secs: 60.0, for_secs: 4.0 }), ..Default::default() }));
        sch.tick(&e, &dir, at(0));
        assert!(!active(&e, "tick", at(0)), "first run is one interval away");
        sch.tick(&e, &dir, at(59));
        assert!(!active(&e, "tick", at(59)));
        sch.tick(&e, &dir, at(60));
        assert!(active(&e, "tick", at(61)) && active(&e, "tick", at(63)));
        assert!(!active(&e, "tick", at(65)), "ends after for_secs");
        sch.tick(&e, &dir, at(120));
        assert!(active(&e, "tick", at(121)), "again a minute later");
        // A long stall does not fire a burst of missed runs.
        sch.tick(&e, &dir, at(1000));
        sch.tick(&e, &dir, at(1001));
        assert_eq!(e.overlays.lock().unwrap().len(), 1, "one run after the stall, no catching up");
        sch.tick(&e, &dir, at(1002));
        assert_eq!(e.overlays.lock().unwrap().len(), 1);
    }

    /// "Every 10 s, show it for 7 s": visible 10..17, hidden 17..20, visible again 20..27.
    #[test]
    fn every_10_for_7_shows_then_hides_until_the_next_period() {
        let e = engine();
        let dir = crate::fsutil::test_dir("auto-every-7");
        let mut sch = Scheduler::default();
        let t0 = Instant::now();
        let at = |s: u64| t0 + Duration::from_secs(s);
        let mut n = auto(Sources { every: Some(Every { every_secs: 10.0, for_secs: 7.0 }), ..Default::default() });
        n.sequence.as_mut().unwrap().id = "inline".into();
        e.global_triggers.lock().unwrap().insert("mon".into(), n);
        let mut shown = vec![];
        for s in 0..=30 {
            sch.tick(&e, &dir, at(s));
            shown.push(active(&e, "mon", at(s)));
        }
        let expect: Vec<bool> = (0..=30).map(|s| (10..17).contains(&s) || (20..27).contains(&s) || s == 30).collect();
        assert_eq!(shown, expect);
    }

    #[test]
    fn nothing_is_scheduled_without_scheduled_triggers() {
        let e = engine();
        let dir = crate::fsutil::test_dir("auto-none");
        let mut sch = Scheduler::default();
        e.global_triggers.lock().unwrap().insert("chat".into(), auto(Sources { desktop: Some(Default::default()), ..Default::default() }));
        sch.tick(&e, &dir, Instant::now());
        assert!(sch.entries.is_empty() && e.overlays.lock().unwrap().is_empty());
        assert!(!is_scheduled(e.global_triggers.lock().unwrap().get("chat").unwrap()));
    }
}
