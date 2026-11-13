pub mod animations;
pub mod automations;
pub mod apply;
pub mod builtin;
pub mod claude;
pub mod cli;
pub mod commands;
pub mod config;
pub mod desktop;
pub mod engine;
pub mod error;
pub mod fsutil;
pub mod identity;
pub mod profiles;
pub mod sdk;
pub mod server;
pub mod socket;
pub mod state;
pub mod sysmon;
pub mod templates;
pub mod themes;
pub mod tray;
pub mod triggers;

use config::Config;
use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cli = cli::Cli::parse(std::env::args().skip(1));
    // `chroma-desk trigger ...` only talks to the running instance: no window, no Tauri.
    if let Some(t) = &cli.trigger {
        std::process::exit(cli::run_trigger(t));
    }
    if let Some(c) = &cli.control {
        std::process::exit(cli::run_control(c));
    }
    let state = AppState::new(Config::load());
    let runner_state = state.clone();
    let start_hidden = state.config.lock().unwrap().settings.start_hidden;
    tauri::Builder::default()
        // A second launch focuses this instance (or applies the requested profile).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let cli = cli::Cli::parse(argv.into_iter().skip(1));
            match cli.apply_profile {
                Some(name) => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app.state::<AppState>();
                        let _ = apply::apply_profile(&state, &apply::emitter(&app), &name).await;
                        tray::refresh(&app);
                    });
                }
                None if !cli.hidden => tray::show_main(app),
                None => {}
            }
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                tray::on_close(window);
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            let emit = apply::emitter(&handle);
            tray::init(&handle);
            if !cli.windowless(start_hidden) {
                tray::show_main(&handle);
            }
            tauri::async_runtime::spawn(engine::runner::run(emit.clone(), runner_state.clone()));
            tauri::async_runtime::spawn(desktop::run(runner_state.engine.clone(), animations::animations_dir(), runner_state.engine.desktop_enabled.clone()));
            themes::watch::spawn(runner_state.engine.themes.clone(), std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));
            if let Some(path) = socket::socket_path() {
                let engine = runner_state.engine.clone();
                let (st, em, app) = (runner_state.clone(), emit.clone(), handle.clone());
                let profiles: socket::ProfileFn = std::sync::Arc::new(move |name: String| {
                    let (st, em, app) = (st.clone(), em.clone(), app.clone());
                    Box::pin(async move {
                        apply::apply_profile(&st, &em, &name).await?;
                        tray::refresh(&app);
                        Ok(name)
                    })
                });
                tauri::async_runtime::spawn(async move {
                    match socket::bind(&path).await {
                        Ok(listener) => socket::serve(listener, engine, animations::animations_dir(), Some(profiles)).await,
                        Err(e) => eprintln!("trigger socket not available ({}): {e}", path.display()),
                    }
                });
            }
            let requested = cli.apply_profile.clone();
            let exit_if_static = requested.is_some();
            let st = runner_state.clone();
            tauri::async_runtime::spawn(async move {
                let applied = apply::startup(st, emit, requested).await;
                tray::refresh(&handle);
                // `--apply-profile` with a hardware-only profile has nothing left to animate.
                if exit_if_static && applied.is_some_and(|s| !s.devices.values().any(|d| d.mode == engine::SceneMode::Effects)) {
                    handle.exit(0);
                }
            });
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
            commands::get_autostart,
            commands::set_autostart,
            commands::server_status,
            commands::set_server_service,
            commands::set_tray_labels,
            commands::refresh_tray,
            commands::show_window,
            commands::quit_app,
            commands::default_fps,
            commands::anim_list,
            commands::anim_get,
            commands::anim_save,
            commands::anim_delete,
            commands::anim_duplicate,
            commands::anim_import,
            commands::anim_export,
            commands::anim_apply,
            commands::trigger,
            commands::test_automation,
            commands::stop_triggers,
            commands::trigger_socket_path,
            commands::profile_get,
            commands::profile_update,
            commands::set_builtin_enabled,
            commands::get_palette,
            commands::theme_status,
            commands::palette_reload,
            commands::desktop_status,
            commands::automation_templates,
            commands::claude_status,
            commands::claude_install,
            commands::claude_remove,
            commands::sysmon_status,
            commands::sysmon_read,
            commands::omarchy_event_hooks,
            commands::omarchy_event_hook_install,
            commands::omarchy_event_hook_remove,
            commands::omarchy_hook_status,
            commands::omarchy_hook_install,
            commands::omarchy_hook_remove,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
