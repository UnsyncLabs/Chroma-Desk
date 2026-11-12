pub mod apply;
pub mod commands;
pub mod config;
pub mod engine;
pub mod error;
pub mod fsutil;
pub mod identity;
pub mod profiles;
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
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle().clone();
            let emit: engine::runner::Emit = Arc::new(move |event, payload| {
                let _ = handle.emit(event, payload);
            });
            tauri::async_runtime::spawn(engine::runner::run(emit.clone(), runner_state.clone()));
            tauri::async_runtime::spawn(apply::startup(runner_state.clone(), emit, None));
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
            commands::list_profiles,
            commands::profiles_path,
            commands::save_profile,
            commands::apply_profile,
            commands::delete_profile,
            commands::rename_profile,
            commands::duplicate_profile,
            commands::export_profile,
            commands::import_profile,
            commands::list_openrgb_profiles,
            commands::import_openrgb_profile,
            commands::get_settings,
            commands::set_settings,
            commands::wait_startup,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
