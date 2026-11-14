//! Drives the real hardware through the engine for a few seconds and reports frame rates.
//! Usage: cargo run --release --example engine_demo [seconds]
use chroma_desk_lib::engine::compositor::{Blend, Layer, Mask};
use chroma_desk_lib::engine::effects::default_params;
use chroma_desk_lib::engine::runner;
use chroma_desk_lib::engine::{DeviceScene, SceneMode};
use chroma_desk_lib::{config::Config, state::AppState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn layer(id: &str, effect: &str) -> Layer {
    Layer { id: id.into(), name: id.into(), enabled: true, opacity: 1.0, blend: Blend::Normal, mask: Mask::All, effect: effect.into(), params: default_params(effect), role: None }
}

#[tokio::main]
async fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    let state = AppState::new(Config::default());
    state.refresh(false).await.expect("connect");
    let devices = state.devices().await.expect("devices");
    let counts: Arc<Mutex<HashMap<String, u32>>> = Arc::default();
    let c2 = counts.clone();
    let emit: runner::Emit = Arc::new(move |ev, payload| {
        let key = if ev == "frame" { payload["key"].as_str().unwrap_or("?").to_string() } else { format!("event:{ev}") };
        *c2.lock().unwrap().entry(key).or_default() += 1;
    });
    {
        let mut scene = state.engine.scene.lock().unwrap();
        scene.sync = std::env::var("SYNC").is_ok();
        scene.global_layers = vec![layer("g", "rainbow")];
        for d in &devices {
            let mut ls = vec![layer("bg", if d.device_type == "Cooler" || d.device_type == "LEDStrip" { "chase" } else { "rainbow" })];
            if d.device_type == "Keyboard" {
                let mut red = layer("wasd", "solid");
                red.mask = Mask::Leds { leds: d.led_names.iter().enumerate().filter(|(_, n)| ["Key: W", "Key: A", "Key: S", "Key: D"].contains(&n.as_str())).map(|(i, _)| i).collect() };
                ls.push(red);
            }
            scene.devices.insert(d.key.clone(), DeviceScene { mode: SceneMode::Effects, layers: ls, ..Default::default() });
        }
        state.engine.visible.lock().unwrap().extend(devices.iter().map(|d| d.key.clone()));
    }
    let r = tokio::spawn(runner::run(emit, state.clone()));
    let cpu0 = cpu_secs();
    tokio::time::sleep(Duration::from_secs(secs)).await;
    let cpu = cpu_secs() - cpu0;
    r.abort();
    println!("ran {secs}s, process CPU {:.2}s ({:.1}% of one core)", cpu, cpu / secs as f64 * 100.0);
    // Read the keyboard back from the server to confirm what the hardware received.
    state.refresh(false).await.unwrap();
    for d in state.devices().await.unwrap().iter().filter(|d| d.device_type == "Keyboard") {
        for key in ["Key: W", "Key: A", "Key: S", "Key: D", "Key: Q", "Key: E"] {
            let i = d.led_names.iter().position(|n| n == key).unwrap();
            println!("{key}: {:?}", d.colors[i]);
        }
    }
    for (k, v) in counts.lock().unwrap().iter() {
        println!("{k}: {:.1} preview/s", *v as f64 / secs as f64);
    }
}

/// Process CPU time (user + system) from /proc/self/stat.
fn cpu_secs() -> f64 {
    let s = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let f: Vec<&str> = s.rsplit_once(')').map(|x| x.1).unwrap_or("").split_whitespace().collect();
    let tick = 100.0;
    (f.get(11).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) + f.get(12).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)) / tick
}
