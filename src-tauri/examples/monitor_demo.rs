//! Verifies the system monitors, chat notifications and thresholds on the real keyboard by reading
//! colours back from the OpenRGB server, next to the real sensor readings.
//!
//! Run it with its own XDG dirs so it never touches the installed app's config:
//! `XDG_CONFIG_HOME=/tmp/x XDG_RUNTIME_DIR=/tmp/y cargo run --example monitor_demo monitors|chat|threshold|cost`
use chroma_desk_lib::engine::compositor::{Blend, Layer, Mask};
use chroma_desk_lib::engine::effects::Params;
use chroma_desk_lib::engine::{DeviceScene, SceneMode, runner};
use chroma_desk_lib::profiles::{NamedTrigger, Threshold};
use chroma_desk_lib::{animations, config::Config, state::AppState, sysmon, templates, triggers};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SCENE: [u8; 3] = [20, 0, 80];

/// `(full, blend, scene, foreign, fraction)` of a row.
type Counts = (usize, usize, usize, usize, f32);

fn scene_layer() -> Layer {
    Layer { id: "bg".into(), name: String::new(), enabled: true, opacity: 1.0, blend: Blend::Normal, mask: Mask::All, effect: "solid".into(), params: Params(json!({"color": SCENE}).as_object().unwrap().clone()), role: None }
}

async fn keys(state: &AppState, names: &[&str]) -> Vec<[u8; 3]> {
    state.refresh(false).await.expect("refresh");
    let devs = state.devices().await.unwrap();
    let kb = devs.iter().find(|d| d.device_type == "Keyboard" && d.led_names.iter().any(|n| n == "Key: W")).expect("keyboard");
    names.iter().map(|k| kb.led_names.iter().position(|n| n == &format!("Key: {k}")).map(|i| [kb.colors[i].0, kb.colors[i].1, kb.colors[i].2]).unwrap_or([0, 0, 0])).collect()
}

fn describe(names: &[&str], c: &[[u8; 3]]) -> String {
    names.iter().zip(c).map(|(k, v)| format!("{k}={:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect::<Vec<_>>().join(" ")
}

async fn setup() -> (AppState, tokio::task::JoinHandle<()>, String) {
    let state = AppState::new(Config::default());
    state.refresh(false).await.expect("connect");
    let devs = state.devices().await.unwrap();
    let kb = devs.iter().find(|d| d.device_type == "Keyboard" && d.led_names.iter().any(|n| n == "Key: W")).expect("keyboard").clone();
    println!("keyboard: {} ({} LEDs)", kb.name, kb.colors.len());
    state.engine.scene.lock().unwrap().devices.insert(kb.key.clone(), DeviceScene { mode: SceneMode::Effects, layers: vec![scene_layer()], fps: Some(60), ..Default::default() });
    let emit: runner::Emit = Arc::new(|_, _| {});
    let h = tokio::spawn(runner::run(emit, state.clone()));
    (state, h, kb.key)
}

fn template(id: &str) -> NamedTrigger {
    templates::templates().into_iter().find(|t| t.id == id).unwrap().notification
}

/// A key is `scene`, the bar colour `full`, a `blend` (on the scene-bar line: the partly filled key) or
/// `foreign` (written by something else: the installed app also drives this keyboard).
fn classify(c: &[[u8; 3]], bar: [u8; 3]) -> Counts {
    let (mut full, mut blend, mut scene, mut foreign) = (0, 0, 0, 0);
    let mut frac = 0.0;
    for v in c {
        if *v == SCENE {
            scene += 1;
        } else if *v == bar {
            full += 1;
        } else {
            // Position on the scene -> bar line, if the colour is on it.
            let k = (0..3).map(|i| (v[i] as f32 - SCENE[i] as f32) / (bar[i] as f32 - SCENE[i] as f32 + 1e-6)).sum::<f32>() / 3.0;
            let on_line = (0..3).all(|i| (SCENE[i] as f32 + (bar[i] as f32 - SCENE[i] as f32) * k - v[i] as f32).abs() < 4.0);
            if on_line && k > 0.0 && k < 1.0 {
                blend += 1;
                frac = k;
            } else {
                foreign += 1;
            }
        }
    }
    (full, blend, scene, foreign, frac)
}

/// Several quick reads; the one with the fewest foreign keys (the installed app writes in between).
async fn row(state: &AppState, names: &[&str], bar: [u8; 3]) -> (Vec<[u8; 3]>, Counts) {
    let mut best: Option<(Vec<[u8; 3]>, Counts)> = None;
    for _ in 0..14 {
        let c = keys(state, names).await;
        let k = classify(&c, bar);
        if best.as_ref().is_none_or(|b| k.3 < (b.1).3) {
            best = Some((c, k));
        }
        if k.3 == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
    best.unwrap()
}

async fn monitors() {
    let (state, h, _) = setup().await;
    let dir = animations::animations_dir();
    let mut gpu_n = template("gpu_monitor");
    if std::env::var_os("SCALE_GPU").is_some() {
        // The GPU is idle: stretch the scales so the bar has something to show (vram 0..0.03 %, temperature 30..50 C).
        if let Some(seq) = gpu_n.sequence.as_mut() && let animations::AnimKind::Sequence { steps, .. } = &mut seq.kind {
            steps[0].params.0.insert("metric".into(), json!("gpu.temp"));
            steps[0].params.0.insert("min".into(), json!(30));
            steps[0].params.0.insert("max".into(), json!(50));
            steps[1].params.0.insert("metric".into(), json!("gpu.vram"));
            steps[1].params.0.insert("max".into(), json!(0.03));
        }
    }
    if std::env::var_os("LOAD").is_some() {
        let n = std::thread::available_parallelism().map_or(4, |n| n.get()) * 2 / 5;
        println!("burning {n} threads");
        for _ in 0..n {
            std::thread::spawn(|| loop { std::hint::black_box((0..1000u64).sum::<u64>()); });
        }
    }
    state.engine.global_triggers.lock().unwrap().insert("gpu_monitor".into(), gpu_n);
    state.engine.global_triggers.lock().unwrap().insert("cpu_monitor".into(), template("cpu_monitor"));
    let f: Vec<&str> = templates::F_ROW.to_vec();
    let n: Vec<&str> = templates::NUMBER_ROW.to_vec();
    let _ = dir;
    let pal = chroma_desk_lib::themes::default_palette();
    let (warn, alert, ok) = (pal.get("warn"), pal.get("alert"), pal.get("ok"));
    println!("scene = {SCENE:?}; palette ok={ok:?} warn={warn:?} alert={alert:?}");
    let t0 = Instant::now();
    // The always-on overlays start at the first scheduler tick (~200 ms) and loop every 6.4 s.
    for cycle in 0..2 {
        for (label, offset, fbar, nbar) in [("step1 vram/cpu.usage", 2.6, ok, warn), ("step2 gpu.usage/cpu.temp", 5.8, [255, 255, 255], alert)] {
            let target = Duration::from_secs_f64(0.25 + cycle as f64 * 6.4 + offset);
            tokio::time::sleep(target.saturating_sub(t0.elapsed())).await;
            let (fr, fk) = row(&state, &f, fbar).await;
            let (nr, nk) = row(&state, &n, nbar).await;
            let g = |m| sysmon::reading(m).map(|r| r.value);
            let (cpu, temp, vram, gpu) = (g(sysmon::Metric::CpuUsage), g(sysmon::Metric::CpuTemp), g(sysmon::Metric::GpuVram), g(sysmon::Metric::GpuUsage));
            println!("[{label} @ {:.1}s] sensors: cpu.usage={cpu:?} cpu.temp={temp:?} gpu.vram={vram:?} gpu.usage={gpu:?}", t0.elapsed().as_secs_f32());
            let scaled = std::env::var_os("SCALE_GPU").is_some();
            let gt = g(sysmon::Metric::GpuTemp);
            let (want_f, want_n) = match (label.starts_with("step1"), scaled) {
                (true, true) => (gt.map(|t| (t - 30.0) / 20.0 * 100.0), cpu),
                (true, false) => (vram, cpu),
                (false, true) => (vram.map(|v| v / 0.03 * 100.0), temp.map(|t| (t - 30.0) / 65.0 * 100.0)),
                (false, false) => (gpu, temp.map(|t| (t - 30.0) / 65.0 * 100.0)),
            };
            let keys_for = |pct: Option<f32>, n: usize| pct.map(|p| p.clamp(0.0, 100.0) / 100.0 * n as f32);
            println!("  F-row   full={} blend={} (frac {:.2}) scene={} foreign={}   expected {:?} keys   {}", fk.0, fk.1, fk.4, fk.2, fk.3, keys_for(want_f, 13), describe(&["Escape", "F1", "F6", "F7", "F12"], &fr.iter().enumerate().filter(|(i, _)| [0, 1, 6, 7, 12].contains(i)).map(|(_, v)| *v).collect::<Vec<_>>()));
            println!("  numrow  full={} blend={} (frac {:.2}) scene={} foreign={}   expected {:?} keys   {}", nk.0, nk.1, nk.4, nk.2, nk.3, keys_for(want_n, 14), describe(&["`", "1", "2", "3", "Backspace"], &[nr[0], nr[1], nr[2], nr[3], nr[13]]));
        }
    }
    h.abort();
}

async fn chat() {
    let (state, h, _) = setup().await;
    let dir = animations::animations_dir();
    let all = ["W", "A", "S", "D", "Up Arrow", "Left Arrow", "Down Arrow", "Right Arrow"];
    for (id, app) in [("discord", "discord"), ("telegram", "Telegram Desktop"), ("slack", "Slack"), ("matrix", "Element Desktop")] {
        state.engine.global_triggers.lock().unwrap().insert(id.into(), template(id));
        let note = triggers::DesktopNote { app: app.into(), urgency: 1, summary: "test".into() };
        println!("== {id}: simulated desktop notification from {app:?}");
        let t0 = Instant::now();
        let fired = triggers::fire_desktop(&state.engine, &dir, &note, Instant::now());
        println!("  fired {fired}");
        let mut seen = vec![];
        while t0.elapsed() < Duration::from_millis(3600) {
            tokio::time::sleep(Duration::from_millis(110)).await;
            let c = keys(&state, &all).await;
            let lit: Vec<&str> = all.iter().zip(&c).filter(|(_, v)| **v != SCENE).map(|(k, _)| *k).collect();
            seen.push((t0.elapsed().as_millis(), lit.join("+"), c[0]));
        }
        for (ms, lit, w) in seen.iter().step_by(2) {
            println!("  {ms:>5} ms lit=[{lit}] W={:02x}{:02x}{:02x}", w[0], w[1], w[2]);
        }
        println!("  after: {}", describe(&all, &keys(&state, &all).await));
    }
    h.abort();
}

async fn threshold() {
    let (state, h, _) = setup().await;
    let temp = sysmon::reading(sysmon::Metric::CpuTemp).map(|r| r.value).unwrap_or(0.0);
    let limit = (temp - 20.0).max(10.0);
    println!("real cpu.temp ~ {temp:.1} C; rule: cpu.temp > {limit:.0} C for 1 s (forced low limit)");
    let mut n = template("cpu_monitor");
    n.sources.always = false;
    n.sources.threshold = Some(Threshold { metric: "cpu.temp".into(), op: "gt".into(), value: limit, for_secs: 1.0, hysteresis: 2.0, cooldown_secs: 60.0 });
    state.engine.global_triggers.lock().unwrap().insert("cpu_hot".into(), n);
    let row = templates::NUMBER_ROW.to_vec();
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(9) {
        tokio::time::sleep(Duration::from_millis(700)).await;
        let c = keys(&state, &row).await;
        println!("  {:>5} ms  numrow classify(full,blend,scene,foreign,frac)={:?}  overlays={}", t0.elapsed().as_millis(), classify(&c, chroma_desk_lib::themes::default_palette().get("warn")), state.engine.overlays.lock().unwrap().len());
    }
    h.abort();
}

async fn cost() {
    let (state, h, _) = setup().await;
    state.engine.global_triggers.lock().unwrap().insert("gpu_monitor".into(), template("gpu_monitor"));
    state.engine.global_triggers.lock().unwrap().insert("cpu_monitor".into(), template("cpu_monitor"));
    let cpu_time = || -> f64 {
        let s = std::fs::read_to_string("/proc/self/stat").unwrap();
        let f: Vec<&str> = s.rsplit(')').next().unwrap().split_whitespace().collect();
        (f[11].parse::<f64>().unwrap() + f[12].parse::<f64>().unwrap()) / 100.0
    };
    tokio::time::sleep(Duration::from_secs(3)).await;
    let (c0, t0) = (cpu_time(), Instant::now());
    tokio::time::sleep(Duration::from_secs(20)).await;
    let busy = (cpu_time() - c0) / t0.elapsed().as_secs_f64() * 100.0;
    println!("both monitors on: {busy:.2} % of one core (this demo process)");
    // Disable them and make sure sampling stops.
    state.engine.global_triggers.lock().unwrap().clear();
    tokio::time::sleep(Duration::from_secs(6)).await;
    let s0 = sysmon::samples_taken();
    let (c1, t1) = (cpu_time(), Instant::now());
    tokio::time::sleep(Duration::from_secs(10)).await;
    println!("after removing them: sampler awake={} samples during 10 s = {}, cpu {:.2} %", sysmon::awake(), sysmon::samples_taken() - s0, (cpu_time() - c1) / t1.elapsed().as_secs_f64() * 100.0);
    h.abort();
}

#[tokio::main]
async fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("monitors") => monitors().await,
        Some("chat") => chat().await,
        Some("threshold") => threshold().await,
        Some("cost") => cost().await,
        _ => eprintln!("usage: monitor_demo monitors|chat|threshold|cost"),
    }
}
