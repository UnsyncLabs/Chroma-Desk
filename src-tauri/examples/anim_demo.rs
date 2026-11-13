//! Verifies animations on the real hardware by reading colours back from the OpenRGB server.
//!
//! `cargo run --example anim_demo engine`   frames / path / palette / overlay through the engine
//! `cargo run --example anim_demo profile`  writes the profile `zz-anim-test` (keyboard, solid blue)
//! `cargo run --example anim_demo readback` prints WASD/Q/E colours of the keyboard
//! `cargo run --example anim_demo cleanup`  deletes the profile `zz-anim-test`
use chroma_desk_lib::animations::{self, AnimKind, Animation, Frame, LoopMode, Stop, Target};
use chroma_desk_lib::engine::compositor::{Blend, Layer, Mask};
use chroma_desk_lib::engine::effects::Params;
use chroma_desk_lib::engine::{DeviceScene, Scene, SceneMode, runner};
use chroma_desk_lib::{config::Config, profiles, socket, state::AppState};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

fn anim(name: &str, kind: AnimKind) -> Animation {
    Animation { schema_version: 1, id: name.into(), name: name.into(), description: None, tags: vec![], created: 0, updated: 1, target: Target::default(), builtin: false, kind }
}

fn key(k: &str) -> String {
    format!("name:Key: {k}")
}

fn solid_blue() -> Layer {
    Layer { id: "bg".into(), name: String::new(), enabled: true, opacity: 1.0, blend: Blend::Normal, mask: Mask::All, effect: "solid".into(), params: Params(json!({"color": [0, 0, 255]}).as_object().unwrap().clone()) }
}

async fn colors(state: &AppState, keys: &[&str]) -> Vec<(String, [u8; 3])> {
    state.refresh(false).await.expect("refresh");
    let devs = state.devices().await.expect("devices");
    let kb = devs.iter().find(|d| d.device_type == "Keyboard").expect("a keyboard");
    keys.iter()
        .map(|k| {
            let i = kb.led_names.iter().position(|n| n == &format!("Key: {k}")).unwrap_or_else(|| panic!("no key {k}"));
            (k.to_string(), [kb.colors[i].0, kb.colors[i].1, kb.colors[i].2])
        })
        .collect()
}

fn show(label: &str, c: &[(String, [u8; 3])]) {
    println!("  {label:<22} {}", c.iter().map(|(k, v)| format!("{k}={:02x}{:02x}{:02x}", v[0], v[1], v[2])).collect::<Vec<_>>().join(" "));
}

async fn engine() {
    let state = AppState::new(Config::default());
    state.refresh(false).await.expect("connect (is the OpenRGB server running?)");
    let devs = state.devices().await.unwrap();
    let kb = devs.iter().find(|d| d.device_type == "Keyboard").expect("keyboard").clone();
    println!("keyboard: {} ({} LEDs)", kb.name, kb.colors.len());
    // Real key positions come from the UI normally; a plain left-to-right line is enough here.
    let emit: runner::Emit = Arc::new(|_, _| {});
    let runner = tokio::spawn(runner::run(emit, state.clone()));
    let play = |a: Animation| {
        let mut ds = DeviceScene { mode: SceneMode::Effects, layers: vec![animations::layer_for(&a)], ..Default::default() };
        ds.fps = Some(60);
        state.engine.scene.lock().unwrap().devices.insert(kb.key.clone(), ds);
    };
    let wasd = ["W", "A", "S", "D", "Q"];

    println!("FRAMES: W red, then A red, then D red (400 ms each, no crossfade)");
    let f = |k: &str| Frame { leds: [(key(k), chroma_desk_lib::themes::Color::Rgb([255, 0, 0]))].into(), duration_ms: None };
    play(anim("f", AnimKind::Frames { frames: vec![f("W"), f("A"), f("D")], frame_ms: 400, loop_mode: LoopMode::Loop, crossfade: 0.0 }));
    for i in 0..6 {
        tokio::time::sleep(Duration::from_millis(if i == 0 { 250 } else { 400 })).await;
        show(&format!("t~{:.1}s", 0.25 + 0.4 * i as f32), &colors(&state, &wasd).await);
    }

    println!("FRAMES with crossfade 1.0: W fades into A");
    play(anim("f2", AnimKind::Frames { frames: vec![f("W"), f("A")], frame_ms: 2000, loop_mode: LoopMode::Loop, crossfade: 1.0 }));
    for _ in 0..4 {
        tokio::time::sleep(Duration::from_millis(450)).await;
        show("W/A during fade", &colors(&state, &["W", "A"]).await);
    }

    println!("PATH: an S through the keyboard, white head travelling in stroke order");
    let route = ["Q", "W", "E", "R", "T", "G", "F", "D", "S", "Z", "X", "C", "V"];
    play(anim("p", AnimKind::Path {
        path: route.iter().map(|k| key(k)).collect(),
        palette: vec![Stop { pos: 0.0, color: chroma_desk_lib::themes::Color::Rgb([255, 255, 255]) }, Stop { pos: 1.0, color: chroma_desk_lib::themes::Color::Rgb([255, 255, 255]) }],
        speed: 0.5, length: 0.2, gap: 1.0, reverse: false, tail_fade: 0.0,
    }));
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let c = colors(&state, &route).await;
        let lit: Vec<&str> = c.iter().filter(|(_, v)| v[0] > 40).map(|(k, _)| k.as_str()).collect();
        println!("  lit along the path: {lit:?}");
    }

    println!("PALETTE: WASD cells rotate through red -> green -> red while staying in place");
    let cells: BTreeMap<String, f32> = [("A", 0.0), ("S", 0.33), ("D", 0.66), ("W", 0.5)].iter().map(|(k, p)| (key(k), *p)).collect();
    play(anim("c", AnimKind::Palette {
        cells,
        palette: vec![Stop { pos: 0.0, color: chroma_desk_lib::themes::Color::Rgb([255, 0, 0]) }, Stop { pos: 0.5, color: chroma_desk_lib::themes::Color::Rgb([0, 255, 0]) }, Stop { pos: 1.0, color: chroma_desk_lib::themes::Color::Rgb([255, 0, 0]) }],
        speed: 0.5, reverse: false,
    }));
    for i in 0..4 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        show(&format!("t~{:.1}s", 0.5 * (i + 1) as f32), &colors(&state, &wasd).await);
    }

    println!("OVERLAY via the socket: blue scene, then Alert on WASD for 2 s");
    state.engine.scene.lock().unwrap().devices.insert(kb.key.clone(), DeviceScene { mode: SceneMode::Effects, layers: vec![solid_blue()], ..Default::default() });
    let dir = std::env::temp_dir().join(format!("anim-demo-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("c.sock");
    let listener = socket::bind(&path).await.unwrap();
    use std::os::unix::fs::PermissionsExt;
    println!("  socket mode: {:o}", std::fs::metadata(&path).unwrap().permissions().mode() & 0o777);
    tokio::spawn(socket::serve(listener, state.engine.clone(), animations::animations_dir()));
    tokio::time::sleep(Duration::from_millis(500)).await;
    show("before", &colors(&state, &wasd).await);
    let p2 = path.clone();
    let reply = tokio::task::spawn_blocking(move || socket::send(&p2, r#"{"animation":"Alert","keys":["W","A","S","D"],"duration":"2s"}"#)).await.unwrap().unwrap();
    println!("  reply: {reply}");
    for _ in 0..3 {
        tokio::time::sleep(Duration::from_millis(350)).await;
        show("during (strobe)", &colors(&state, &wasd).await);
    }
    tokio::time::sleep(Duration::from_millis(1800)).await;
    show("after expiry", &colors(&state, &wasd).await);

    runner.abort();
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::main]
async fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("engine") => engine().await,
        Some("profile") => {
            let mut scene = Scene::default();
            // The key is only known once connected: use the device list.
            let state = AppState::new(Config::default());
            state.refresh(false).await.expect("connect");
            let devs = state.devices().await.unwrap();
            let kb = devs.iter().find(|d| d.device_type == "Keyboard").expect("keyboard");
            scene.devices.insert(kb.key.clone(), DeviceScene { mode: SceneMode::Effects, layers: vec![solid_blue()], ..Default::default() });
            let p = profiles::Profile::new("zz-anim-test", scene);
            profiles::save(&profiles::profiles_dir(), &p).unwrap();
            println!("profile zz-anim-test written");
        }
        Some("readback") => {
            let state = AppState::new(Config::default());
            state.refresh(false).await.expect("connect");
            show("keyboard", &colors(&state, &["W", "A", "S", "D", "Q", "E"]).await);
        }
        Some("cleanup") => {
            profiles::delete(&profiles::profiles_dir(), "zz-anim-test").unwrap();
            println!("profile removed");
        }
        _ => eprintln!("usage: anim_demo engine|profile|readback|cleanup"),
    }
}
