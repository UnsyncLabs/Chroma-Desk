//! Prints the device list as the frontend receives it (JSON), for layout debugging.
//! Usage: cargo run --release --example dump_devices > devices.json
use chroma_desk_lib::{config::Config, state::AppState};

#[tokio::main]
async fn main() {
    let state = AppState::new(Config::load());
    state.refresh(false).await.expect("connect");
    let devs = state.devices().await.expect("devices");
    println!("{}", serde_json::to_string(&devs).unwrap());
}
