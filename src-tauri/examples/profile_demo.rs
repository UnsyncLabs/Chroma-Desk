//! Exercises zone-size migration and profile save/apply against the live server.
//! Usage: cargo run --example profile_demo
use chroma_desk_lib::{apply, config::Config, profiles, state::AppState};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let state = AppState::new(Config::load());
    state.connect_all().await.expect("connect");
    let sizes = state.resizable_zone_sizes().await.unwrap();
    println!("resizable zones now: {sizes:?}");
    println!("config after migration: {:?}", state.config.lock().unwrap().zone_sizes_by_key);
    let scene = state.engine.scene.lock().unwrap().clone();
    let p = profiles::Profile { zone_sizes: sizes, ..profiles::Profile::new("zz-demo", scene) };
    profiles::save(&profiles::profiles_dir(), &p).unwrap();
    let emit: chroma_desk_lib::engine::runner::Emit = Arc::new(|e, _| println!("event {e}"));
    apply::apply_profile(&state, &emit, "zz-demo").await.expect("apply");
    println!("list: {:?}", profiles::list(&profiles::profiles_dir()).profiles);
    profiles::delete(&profiles::profiles_dir(), "zz-demo").unwrap();
}
