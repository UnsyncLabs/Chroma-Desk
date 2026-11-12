//! System tray: Show, Profiles >, Lights off, Quit; plus close-to-tray behaviour.

use crate::apply;
use crate::profiles;
use crate::state::AppState;
use serde::Deserialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

const TRAY_ID: &str = "main";

/// Translated labels, supplied by the frontend (so the tray follows the UI language).
#[derive(Debug, Clone, Deserialize)]
pub struct TrayLabels {
    pub show: String,
    pub profiles: String,
    pub no_profiles: String,
    pub lights_off: String,
    pub lights_on: String,
    pub quit: String,
}

impl Default for TrayLabels {
    fn default() -> Self {
        Self {
            show: "Show".into(),
            profiles: "Profiles".into(),
            no_profiles: "No profiles".into(),
            lights_off: "Lights off".into(),
            lights_on: "Lights on".into(),
            quit: "Quit".into(),
        }
    }
}

#[derive(Default)]
pub struct TrayState {
    pub labels: Mutex<TrayLabels>,
    /// The tray icon was created (a StatusNotifier host may still be missing, which we cannot detect).
    pub available: AtomicBool,
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let state = app.state::<AppState>();
    let l = state.tray.labels.lock().unwrap().clone();
    let off = state.engine.blackout.load(Ordering::Relaxed);
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "show", &l.show, true, None::<&str>)?)?;
    let sub = Submenu::with_id(app, "profiles", &l.profiles, true)?;
    let list = profiles::list(&profiles::profiles_dir()).profiles;
    if list.is_empty() {
        sub.append(&MenuItem::with_id(app, "none", &l.no_profiles, false, None::<&str>)?)?;
    }
    for p in list {
        sub.append(&MenuItem::with_id(app, format!("profile:{}", p.name), &p.name, true, None::<&str>)?)?;
    }
    menu.append(&sub)?;
    menu.append(&MenuItem::with_id(app, "toggle-off", if off { &l.lights_on } else { &l.lights_off }, true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", &l.quit, true, None::<&str>)?)?;
    Ok(menu)
}

/// Rebuilds the tray menu (profile list, language, lights state).
pub fn refresh(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

/// Creates the tray icon; failure leaves `available` false so closing minimises instead.
pub fn init(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(menu) = build_menu(app) else { return };
    let mut b = TrayIconBuilder::with_id(TRAY_ID).menu(&menu).tooltip("chroma-desk").show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    let built = b
        .on_menu_event(|app, ev| {
            let id = ev.id().as_ref().to_string();
            match id.as_str() {
                "show" => show_main(app),
                "quit" => {
                    // The UI checks for unsaved changes, then calls `quit_app`.
                    let _ = app.emit("request-quit", ());
                }
                "toggle-off" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app.state::<AppState>();
                        let on = !state.engine.blackout.load(Ordering::Relaxed);
                        if state.set_blackout(on).await.is_ok() {
                            let _ = app.emit("blackout", on);
                            refresh(&app);
                        }
                    });
                }
                other => {
                    if let Some(name) = other.strip_prefix("profile:") {
                        let (app, name) = (app.clone(), name.to_string());
                        tauri::async_runtime::spawn(async move {
                            let state = app.state::<AppState>();
                            if let Err(e) = apply::apply_profile(&state, &apply::emitter(&app), &name).await {
                                let _ = app.emit("startup-error", serde_json::json!({ "code": e.code, "detail": e.detail }));
                            }
                        });
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, ev| {
            if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, button_state: tauri::tray::MouseButtonState::Up, .. } = ev {
                show_main(tray.app_handle());
            }
        })
        .build(app);
    state.tray.available.store(built.is_ok(), Ordering::Relaxed);
    if let Err(e) = built {
        eprintln!("tray unavailable: {e}");
    }
}

/// Close button: hide to the tray (animations need the app alive); minimise if there is no tray.
pub fn on_close(window: &tauri::Window) {
    let app = window.app_handle();
    if app.state::<AppState>().tray.available.load(Ordering::Relaxed) {
        let _ = window.hide();
    } else {
        let _ = window.minimize();
        let _ = app.emit("tray-unavailable", ());
    }
}
