//! Desktop-notification chat templates through a REAL (private) D-Bus session: `notify-send -a ...`
//! calls on `$DESKTOP_DEMO_BUS`, seen by the monitor connection, firing the templates on the real
//! keyboard (colours read back from the OpenRGB server). Never touches the user's session bus.
//!
//! `dbus-daemon --session --address=unix:path=/tmp/x/bus --nofork &` then
//! `DESKTOP_DEMO_BUS=unix:path=/tmp/x/bus XDG_CONFIG_HOME=... cargo run --release --example desktop_demo`
use chroma_desk_lib::engine::compositor::{Blend, Layer, Mask};
use chroma_desk_lib::engine::effects::Params;
use chroma_desk_lib::engine::{DeviceScene, SceneMode, runner};
use chroma_desk_lib::{animations, config::Config, desktop, state::AppState, templates, triggers};
use serde_json::json;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

const SCENE: [u8; 3] = [20, 0, 80];

#[tokio::main]
async fn main() {
    let bus = std::env::var("DESKTOP_DEMO_BUS").expect("set DESKTOP_DEMO_BUS to the private bus address");
    let state = AppState::new(Config::default());
    state.refresh(false).await.expect("connect");
    let devs = state.devices().await.unwrap();
    let kb = devs.iter().find(|d| d.device_type == "Keyboard" && d.led_names.iter().any(|n| n == "Key: W")).expect("keyboard").clone();
    println!("keyboard: {} ({} LEDs)", kb.name, kb.colors.len());
    let layer = Layer { id: "bg".into(), name: String::new(), enabled: true, opacity: 1.0, blend: Blend::Normal, mask: Mask::All, effect: "solid".into(), params: Params(json!({"color": SCENE}).as_object().unwrap().clone()) };
    state.engine.scene.lock().unwrap().devices.insert(kb.key.clone(), DeviceScene { mode: SceneMode::Effects, layers: vec![layer], fps: Some(60), ..Default::default() });
    for id in ["discord", "telegram", "slack", "matrix"] {
        state.engine.global_triggers.lock().unwrap().insert(id.into(), templates::templates().into_iter().find(|t| t.id == id).unwrap().notification);
    }
    let emit: runner::Emit = Arc::new(|_, _| {});
    let runner_task = tokio::spawn(runner::run(emit, state.clone()));
    let enabled = Arc::new(AtomicBool::new(true));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (b2, e2) = (bus.clone(), enabled.clone());
    let mon = tokio::spawn(async move { desktop::monitor(Some(&b2), &e2, &tx).await });
    tokio::time::sleep(Duration::from_millis(800)).await;
    let dir = animations::animations_dir();
    let all = ["W", "A", "S", "D", "Up Arrow", "Left Arrow", "Down Arrow", "Right Arrow"];
    for (app, expect, tint) in [("discord", "discord", [0x7c, 0x5c, 0xff]), ("Telegram Desktop", "telegram", [0x2a, 0xab, 0xee]), ("Slack", "slack", [0xe0, 0x1e, 0x5a]), ("Element", "matrix", [0x0d, 0xbd, 0x8b]), ("org.gnome.Fractal", "matrix", [0x0d, 0xbd, 0x8b]), ("firefox", "", [0x7c, 0x5c, 0xff])] {
        let st = Command::new("notify-send").env("DBUS_SESSION_BUS_ADDRESS", &bus).args(["-a", app, "chat", "hello"]).stdout(Stdio::null()).status().unwrap();
        let note = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await.ok().flatten();
        let fired = note.as_ref().map(|n| triggers::fire_desktop(&state.engine, &dir, n, Instant::now())).unwrap_or(0);
        println!("== notify-send -a {app:?} (exit {st}) -> monitor saw {:?}; fired {fired} (expect {})", note.as_ref().map(|n| (&n.app, n.urgency)), if expect.is_empty() { "0" } else { expect });
        let t0 = Instant::now();
        let mut peak: Option<(Vec<&str>, [u8; 3])> = None;
        while t0.elapsed() < Duration::from_millis(2600) {
            tokio::time::sleep(Duration::from_millis(40)).await;
            state.refresh(false).await.unwrap();
            let d = state.devices().await.unwrap();
            let k = d.iter().find(|d| d.key == kb.key).unwrap();
            let col = |n: &str| k.led_names.iter().position(|x| x == &format!("Key: {n}")).map(|i| [k.colors[i].0, k.colors[i].1, k.colors[i].2]).unwrap();
            // Only the template colour counts: the installed app also writes this keyboard.
            let lit: Vec<&str> = all.iter().copied().filter(|n| col(n) == tint).collect();
            if lit.len() >= peak.as_ref().map_or(0, |p| p.0.len()) && !lit.is_empty() {
                peak = Some((lit, col("W")));
            }
        }
        match peak {
            Some((l, w)) => println!("   peak keys in the template colour: {} | W={:02x}{:02x}{:02x}", l.join("+"), w[0], w[1], w[2]),
            None => println!("   nothing in the template colour"),
        }
        tokio::time::sleep(Duration::from_millis(2500)).await;
    }
    enabled.store(false, std::sync::atomic::Ordering::Relaxed);
    let _ = mon.await;
    runner_task.abort();
}
