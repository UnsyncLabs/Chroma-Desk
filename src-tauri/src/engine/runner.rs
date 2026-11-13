//! The 60 Hz engine task: composes frames, applies per-device fps caps, skips unchanged
//! frames, feeds the UI preview and survives SDK failures by reconnecting.

use super::{DevInfo, Engine, SceneMode, fps_for, is_due, render_device, sync_slots, Tick};
use crate::error::ApiError;
use crate::sdk::{self, Rgb};
use crate::state::AppState;
use futures_util::future::join_all;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use std::sync::Arc;

/// Event sink (`frame`, `engine-status`, `devices-changed`); the app forwards them to the webview.
pub type Emit = Arc<dyn Fn(&'static str, serde_json::Value) + Send + Sync>;

const TICK: Duration = Duration::from_micros(16_667);
const PREVIEW_INTERVAL: Duration = Duration::from_millis(33);
/// Unchanged frames are re-sent this often: some controllers fall back to their own mode when idle.
const KEEPALIVE: Duration = Duration::from_secs(10);
const MAX_BACKOFF: Duration = Duration::from_secs(10);
/// Consecutive ticks in which *every* send failed before the connection is considered dead.
const FAILURES_BEFORE_RECONNECT: u32 = 3;
/// A device whose send failed is left alone this long (no 60 Hz retry storm, no log spam).
const DEVICE_RETRY: Duration = Duration::from_secs(1);
/// How often the controller list is compared with the server's (hot-plug).
const DEVICE_POLL: Duration = Duration::from_secs(3);

#[derive(Serialize, Clone)]
struct FramePayload {
    key: String,
    /// Flat RGB bytes.
    colors: Vec<u8>,
}

#[derive(Serialize, Clone)]
struct StatusPayload {
    /// `ok`, `reconnecting` or `server_down`.
    state: &'static str,
}

struct Sent {
    at: Instant,
    frame: Vec<[u8; 3]>,
}

struct Job {
    index: usize,
    key: String,
    frame: Vec<[u8; 3]>,
}

#[derive(Default)]
struct Runner {
    sent: HashMap<String, Sent>,
    previewed: HashMap<String, (Instant, Vec<[u8; 3]>)>,
    /// Devices already switched to Direct.
    direct: HashSet<String>,
    failures: u32,
    backoff: Duration,
    retry_at: Option<Instant>,
    status: Option<&'static str>,
    /// Devices that failed recently, and when they may be tried again.
    blocked: HashMap<String, Instant>,
    next_poll: Option<Instant>,
}

/// Frames to send and to preview for this tick.
struct Plan {
    jobs: Vec<Job>,
    previews: Vec<FramePayload>,
}

impl Runner {
    fn plan(&mut self, engine: &Engine, now: Instant) -> Plan {
        let t = now.saturating_duration_since(engine.start).as_secs_f32();
        let scene = engine.scene.lock().unwrap().clone();
        let devs: Vec<DevInfo> = engine.devs.lock().unwrap().clone();
        let positions = engine.positions.lock().unwrap().clone();
        let visible = engine.visible.lock().unwrap().clone();
        let by_type = engine.fps_by_type.lock().unwrap().clone();
        let blackout = engine.blackout.load(Ordering::Relaxed);
        let slots = sync_slots(&scene, &devs);
        let overlays = engine.overlays.lock().unwrap().active(now);

        let mut plan = Plan { jobs: vec![], previews: vec![] };
        let mut active = HashSet::new();
        for dev in &devs {
            let Some(ds) = scene.devices.get(&dev.key) else { continue };
            if ds.mode != SceneMode::Effects {
                continue;
            }
            active.insert(dev.key.clone());
            if self.blocked.get(&dev.key).is_some_and(|until| now < *until) {
                continue;
            }
            let fps = fps_for(&dev.device_type, ds.fps, &by_type);
            let last = self.sent.get(&dev.key).map(|s| s.at);
            let due = is_due(last, now, fps) || last.is_some_and(|l| now.saturating_duration_since(l) >= KEEPALIVE);
            let want_preview = visible.contains(&dev.key)
                && self.previewed.get(&dev.key).is_none_or(|(at, _)| now.saturating_duration_since(*at) >= PREVIEW_INTERVAL);
            if !due && !want_preview {
                continue;
            }
            let Some(frame) = render_device(&scene, dev, positions.get(&dev.key), slots.get(&dev.key).copied(), &Tick { t, blackout, overlays: &overlays, now }) else {
                continue;
            };
            if want_preview && self.previewed.get(&dev.key).is_none_or(|(_, f)| *f != frame) {
                plan.previews.push(FramePayload { key: dev.key.clone(), colors: frame.iter().flatten().copied().collect() });
                self.previewed.insert(dev.key.clone(), (now, frame.clone()));
            }
            let changed = self.sent.get(&dev.key).is_none_or(|s| s.frame != frame);
            let keepalive = last.is_some_and(|l| now.saturating_duration_since(l) >= KEEPALIVE);
            if due && (changed || keepalive) {
                plan.jobs.push(Job { index: dev.index, key: dev.key.clone(), frame });
            }
        }
        // Devices that left Effects mode must be put in Direct again when they come back.
        self.direct.retain(|k| active.contains(k));
        self.sent.retain(|k, _| active.contains(k));
        self.previewed.retain(|k, _| active.contains(k));
        self.blocked.retain(|k, until| active.contains(k) && now < *until);
        plan
    }

    /// Sends the jobs; returns `(succeeded, failed)` counts.
    async fn send(&mut self, state: &AppState, jobs: Vec<Job>, now: Instant) -> (usize, usize) {
        let guard = state.conn.lock().await;
        let Some(conn) = guard.as_ref() else {
            for job in &jobs {
                self.blocked.insert(job.key.clone(), now + DEVICE_RETRY);
            }
            return (0, jobs.len());
        };
        let direct = &self.direct;
        let results = join_all(jobs.into_iter().map(|job| async move {
            let res = async {
                let c = conn.group.get_controller(job.index).map_err(ApiError::sdk)?;
                if !direct.contains(&job.key) {
                    sdk::controllable(c).await?;
                }
                let colors: Vec<Rgb> = job.frame.iter().map(|c| Rgb(c[0], c[1], c[2])).collect();
                sdk::set_leds(c, &colors, false).await
            }
            .await;
            (job, res)
        }))
        .await;
        drop(guard);
        let (mut ok, mut failed) = (0, 0);
        for (job, res) in results {
            match res {
                Ok(()) => {
                    ok += 1;
                    self.direct.insert(job.key.clone());
                    self.blocked.remove(&job.key);
                    self.sent.insert(job.key, Sent { at: now, frame: job.frame });
                }
                Err(e) => {
                    failed += 1;
                    eprintln!("engine: sending to {} failed: {e}", job.key);
                    // Mode may have been reset by whatever broke the send: re-enter Direct.
                    self.direct.remove(&job.key);
                    self.blocked.insert(job.key, now + DEVICE_RETRY);
                }
            }
        }
        (ok, failed)
    }
}

fn set_status(emit: &Emit, r: &mut Runner, s: &'static str) {
    if r.status != Some(s) {
        r.status = Some(s);
        emit("engine-status", serde_json::json!(StatusPayload { state: s }));
    }
}

/// Runs forever; spawn it once at startup.
pub async fn run(emit: Emit, state: AppState) {
    let mut tick = tokio::time::interval(TICK);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut r = Runner::default();
    loop {
        tick.tick().await;
        let now = Instant::now();
        if let Some(at) = r.retry_at {
            if now < at {
                continue;
            }
            // Time to try again: reconnect without spawning the server.
            match state.refresh(false).await {
                Ok(()) => {
                    let _ = state.devices().await;
                    r.retry_at = None;
                    r.backoff = Duration::ZERO;
                    r.failures = 0;
                    r.direct.clear();
                    r.sent.clear();
                    r.blocked.clear();
                    set_status(&emit, &mut r, "ok");
                    emit("devices-changed", serde_json::Value::Null);
                }
                Err(e) => {
                    r.backoff = (r.backoff * 2).clamp(Duration::from_secs(1), MAX_BACKOFF);
                    r.retry_at = Some(now + r.backoff);
                    set_status(&emit, &mut r, if e.code == "server_down" { "server_down" } else { "reconnecting" });
                }
            }
            continue;
        }

        // Hot-plug: OpenRGB renumbers its controllers when a device appears or goes away, which
        // would make every cached index point at the wrong controller.
        if r.next_poll.is_none_or(|at| now >= at) {
            let first = r.next_poll.is_none();
            r.next_poll = Some(now + DEVICE_POLL);
            if !first && matches!(state.poll_device_list().await, Ok(true)) {
                if state.apply_saved_zone_sizes().await.unwrap_or(false) {
                    let _ = state.refresh(false).await;
                }
                let _ = state.devices().await;
                r.direct.clear();
                r.sent.clear();
                r.blocked.clear();
                r.previewed.clear();
                emit("devices-changed", serde_json::Value::Null);
            }
        }

        let plan = r.plan(&state.engine, now);
        for p in plan.previews {
            emit("frame", serde_json::json!(p));
        }
        if plan.jobs.is_empty() {
            continue;
        }
        let (ok, failed) = r.send(&state, plan.jobs, now).await;
        if ok > 0 || failed == 0 {
            // At least one device took its frame: the connection is alive (a single broken
            // device must not tear it down for everyone).
            r.failures = 0;
            set_status(&emit, &mut r, "ok");
        } else {
            r.failures += 1;
            if r.failures >= FAILURES_BEFORE_RECONNECT {
                // Drop the connection; the next ticks retry with exponential backoff.
                *state.conn.lock().await = None;
                r.backoff = Duration::from_secs(1);
                r.retry_at = Some(now + r.backoff);
                set_status(&emit, &mut r, "reconnecting");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::compositor::{Blend, Layer, Mask};
    use crate::engine::{DeviceScene, effects};
    use serde_json::json;

    fn engine_with(device_type: &str, effect: &str) -> Engine {
        let e = Engine::default();
        e.devs.lock().unwrap().push(DevInfo {
            index: 0,
            key: "k".into(),
            name: "k".into(),
            device_type: device_type.into(),
            led_count: 4,
            led_names: vec![String::new(); 4],
            zones: vec![(0, 0, 4)],
            default_positions: vec![[0.5, 0.5]; 4],
        });
        let layer = Layer {
            id: "l".into(),
            name: "l".into(),
            enabled: true,
            opacity: 1.0,
            blend: Blend::Normal,
            mask: Mask::All,
            effect: effect.into(),
            params: effects::params(json!({})),
        };
        e.scene.lock().unwrap().devices.insert("k".into(), DeviceScene { mode: SceneMode::Effects, layers: vec![layer], ..Default::default() });
        e
    }

    /// Simulates a second of 60 Hz ticks and counts frames sent.
    fn simulate(e: &Engine, ticks: u32) -> usize {
        let mut r = Runner::default();
        let t0 = e.start;
        let mut sent = 0;
        for i in 0..ticks {
            let now = t0 + TICK * i;
            let plan = r.plan(e, now);
            for job in plan.jobs {
                r.sent.insert(job.key, Sent { at: now, frame: job.frame });
                sent += 1;
            }
        }
        sent
    }

    #[test]
    fn animated_device_is_capped_by_type() {
        // Rainbow changes every frame, so only the cap limits the rate.
        let ram = simulate(&engine_with("DRam", "rainbow"), 60);
        assert!((9..=11).contains(&ram), "RAM sent {ram} frames/s");
        let strip = simulate(&engine_with("LEDStrip", "rainbow"), 60);
        assert!((28..=31).contains(&strip), "strip sent {strip} frames/s");
        let kb = simulate(&engine_with("Keyboard", "rainbow"), 60);
        assert!(kb >= 55, "keyboard sent {kb}");
    }

    #[test]
    fn failed_device_is_not_retried_every_tick() {
        let e = engine_with("Keyboard", "rainbow");
        let mut r = Runner::default();
        let t0 = e.start;
        assert_eq!(r.plan(&e, t0).jobs.len(), 1);
        // Pretend the send failed at t0.
        r.blocked.insert("k".into(), t0 + DEVICE_RETRY);
        for i in 1..50 {
            assert!(r.plan(&e, t0 + TICK * i).jobs.is_empty(), "tick {i} must be skipped");
        }
        assert_eq!(r.plan(&e, t0 + DEVICE_RETRY + TICK).jobs.len(), 1, "retried after the backoff");
    }

    #[test]
    fn static_scene_is_sent_once() {
        let n = simulate(&engine_with("Keyboard", "solid"), 120);
        assert_eq!(n, 1, "unchanged frames must not be re-sent");
    }

    #[test]
    fn keepalive_resends_static_frames() {
        let e = engine_with("Keyboard", "solid");
        let n = simulate(&e, 60 * 25); // 25 s
        assert!((2..=3).contains(&n), "{n}");
    }

    #[test]
    fn preview_only_for_visible_devices_at_30fps() {
        let e = engine_with("Keyboard", "rainbow");
        let mut r = Runner::default();
        let mut count = 0;
        for i in 0..60 {
            count += r.plan(&e, e.start + TICK * i).previews.len();
        }
        assert_eq!(count, 0);
        e.visible.lock().unwrap().insert("k".into());
        let mut r = Runner::default();
        for i in 0..60 {
            count += r.plan(&e, e.start + TICK * i).previews.len();
        }
        assert!((29..=31).contains(&count), "{count} preview frames/s");
    }
}
