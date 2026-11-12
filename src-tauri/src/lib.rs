pub mod commands;
pub mod config;
pub mod engine;
pub mod error;
pub mod identity;
pub mod sdk;
pub mod state;

use config::Config;
use state::AppState;
use std::sync::Arc;
use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new(Config::load());
    let runner_state = state.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let emit: engine::runner::Emit = Arc::new(move |event, payload| {
                let _ = handle.emit(event, payload);
            });
            tauri::async_runtime::spawn(engine::runner::run(emit, runner_state.clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::list_devices,
            commands::rescan,
            commands::set_leds,
            commands::set_mode,
            commands::resize_zone,
            commands::get_scene,
            commands::set_scene,
            commands::set_led_positions,
            commands::set_visible_devices,
            commands::effect_defs,
            commands::effect_defaults,
            commands::set_fps_by_type,
            commands::get_blackout,
            commands::set_blackout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
