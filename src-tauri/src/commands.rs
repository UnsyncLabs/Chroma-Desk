//! Tauri commands exposed to the frontend.

use crate::apply;
use crate::server::{self, ServerStatus};
use crate::tray::{self, TrayLabels};
use crate::config::{Settings, zone_key};
use crate::profiles::{self, Profile, ProfileInfo, ProfileList};
use crate::engine::effects::{self, EffectDef, Params, Pos};
use crate::engine::Scene;
use crate::error::{ApiError, Res};
use crate::sdk::{self, DeviceDto, ModeRequest, Rgb};
use crate::state::AppState;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use tauri::State;

#[tauri::command]
pub async fn connect(state: State<'_, AppState>) -> Res<Vec<DeviceDto>> {
    state.refresh(true).await?;
    if state.apply_saved_zone_sizes().await? {
        state.refresh(true).await?;
    }
    state.devices().await
}

#[tauri::command]
pub async fn list_devices(state: State<'_, AppState>) -> Res<Vec<DeviceDto>> {
    state.refresh(true).await?;
    state.devices().await
}

#[tauri::command]
pub async fn rescan(state: State<'_, AppState>) -> Res<Vec<DeviceDto>> {
    {
        let guard = state.conn.lock().await;
        if let Some(conn) = guard.as_ref() {
            conn.client.rescan_devices().await.map_err(ApiError::sdk)?;
        }
    }
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    connect(state).await
}

/// Sends all LED colours of a device (sets Direct first if `ensure_direct`).
#[tauri::command]
pub async fn set_leds(state: State<'_, AppState>, device: usize, colors: Vec<Rgb>, ensure_direct: Option<bool>) -> Res<()> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
    let c = conn.group.get_controller(device).map_err(ApiError::sdk)?;
    sdk::set_leds(c, &colors, ensure_direct.unwrap_or(false)).await
}

#[tauri::command]
pub async fn set_mode(state: State<'_, AppState>, device: usize, req: ModeRequest) -> Res<()> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
    let c = conn.group.get_controller(device).map_err(ApiError::sdk)?;
    sdk::apply_mode(c, &req).await
}

/// Resizes a zone and remembers it so it is re-applied on every start.
#[tauri::command]
pub async fn resize_zone(state: State<'_, AppState>, device: usize, zone: usize, size: usize) -> Res<Vec<DeviceDto>> {
    {
        let guard = state.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let c = conn.group.get_controller(device).map_err(ApiError::sdk)?;
        let z = c.get_zone(zone).map_err(ApiError::sdk)?;
        if size < z.leds_min() || size > z.leds_max() {
            return Err(ApiError::new("size_out_of_range", format!("{}-{}", z.leds_min(), z.leds_max())));
        }
        z.resize(size).await.map_err(ApiError::sdk)?;
        let pos = conn.group.iter().position(|x| x.id() == device).ok_or_else(ApiError::no_connection)?;
        let key = zone_key(&AppState::keys_of(conn)[pos], z.name());
        let mut cfg = state.config.lock().unwrap();
        cfg.zone_sizes_by_key.insert(key, size);
        cfg.save()?;
    }
    state.refresh(true).await?;
    state.devices().await
}

// ---------- engine ----------

#[tauri::command]
pub fn get_scene(state: State<'_, AppState>) -> Scene {
    state.engine.scene.lock().unwrap().clone()
}

/// Replaces the whole scene (the frontend owns it and pushes every change).
#[tauri::command]
pub fn set_scene(state: State<'_, AppState>, scene: Scene) {
    *state.engine.scene.lock().unwrap() = scene;
}

/// Physical LED positions of a device (normalised), used by position-based effects.
#[tauri::command]
pub fn set_led_positions(state: State<'_, AppState>, key: String, positions: Vec<Pos>) {
    state.engine.positions.lock().unwrap().insert(key, positions);
}

/// Devices whose live preview (`frame` events) the UI is showing.
#[tauri::command]
pub fn set_visible_devices(state: State<'_, AppState>, keys: Vec<String>) {
    *state.engine.visible.lock().unwrap() = keys.into_iter().collect();
}

#[tauri::command]
pub fn effect_defs() -> Vec<EffectDef> {
    effects::effect_defs().to_vec()
}

#[tauri::command]
pub fn effect_defaults() -> HashMap<&'static str, Params> {
    effects::all_defaults()
}

#[tauri::command]
pub fn set_fps_by_type(state: State<'_, AppState>, fps: HashMap<String, u32>) {
    *state.engine.fps_by_type.lock().unwrap() = fps;
}

#[tauri::command]
pub fn get_blackout(state: State<'_, AppState>) -> bool {
    state.engine.blackout.load(Ordering::Relaxed)
}

/// Lights off / on. Effects devices go black through the engine; hardware-mode devices get a
/// black colour now and their recorded native mode back when turned on.
#[tauri::command]
pub async fn set_blackout(state: State<'_, AppState>, on: bool) -> Res<()> {
    state.set_blackout(on).await
}

// ---------- profiles and settings ----------

#[tauri::command]
pub fn list_profiles() -> ProfileList {
    profiles::list(&profiles::profiles_dir())
}

#[tauri::command]
pub fn profiles_path() -> String {
    profiles::profiles_dir().to_string_lossy().into_owned()
}

/// Saves the live scene (and current zone sizes) as profile `name`.
#[tauri::command]
pub async fn save_profile(state: State<'_, AppState>, name: String) -> Res<ProfileInfo> {
    let zone_sizes = state.resizable_zone_sizes().await.unwrap_or_default();
    let scene = state.engine.scene.lock().unwrap().clone();
    let existing = profiles::load(&profiles::profiles_dir(), name.trim()).ok();
    let profile = Profile {
        schema_version: profiles::SCHEMA_VERSION,
        name,
        openrgb_profile: existing.and_then(|p| p.openrgb_profile),
        zone_sizes,
        scene,
    };
    let info = profiles::save(&profiles::profiles_dir(), &profile)?;
    state.set_current_profile(Some(info.name.clone()))?;
    Ok(info)
}

#[tauri::command]
pub async fn apply_profile(app: tauri::AppHandle, state: State<'_, AppState>, name: String) -> Res<Scene> {
    apply::apply_profile(&state, &apply::emitter(&app), &name).await
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, name: String) -> Res<()> {
    profiles::delete(&profiles::profiles_dir(), &name)?;
    state.forget_profile(&name)
}

#[tauri::command]
pub fn rename_profile(state: State<'_, AppState>, old: String, new: String) -> Res<ProfileInfo> {
    let info = profiles::rename(&profiles::profiles_dir(), &old, &new)?;
    state.rename_profile_refs(&old, &info.name)?;
    Ok(info)
}

#[tauri::command]
pub fn duplicate_profile(name: String, new: String) -> Res<ProfileInfo> {
    profiles::duplicate(&profiles::profiles_dir(), &name, &new)
}

#[tauri::command]
pub fn export_profile(name: String, path: String) -> Res<()> {
    profiles::export(&profiles::profiles_dir(), &name, std::path::Path::new(&path))
}

#[tauri::command]
pub fn import_profile(path: String) -> Res<ProfileInfo> {
    profiles::import(&profiles::profiles_dir(), std::path::Path::new(&path))
}

#[tauri::command]
pub async fn list_openrgb_profiles(state: State<'_, AppState>) -> Res<Vec<String>> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
    conn.client.get_profiles().await.map_err(ApiError::sdk)
}

/// Creates a chroma-desk profile that loads the OpenRGB profile `name` when applied.
#[tauri::command]
pub fn import_openrgb_profile(name: String) -> Res<ProfileInfo> {
    let dir = profiles::profiles_dir();
    let pname = format!("OpenRGB: {name}");
    let profile = Profile {
        schema_version: profiles::SCHEMA_VERSION,
        name: pname,
        openrgb_profile: Some(name),
        zone_sizes: Default::default(),
        scene: Scene::default(),
    };
    profiles::save(&dir, &profile)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.config.lock().unwrap().settings.clone()
}

#[tauri::command]
pub fn set_settings(state: State<'_, AppState>, settings: Settings) -> Res<()> {
    *state.engine.fps_by_type.lock().unwrap() = settings.fps_by_type.clone();
    let mut cfg = state.config.lock().unwrap();
    cfg.settings = settings;
    cfg.save()
}

/// Resolves once the startup sequence (connect, default profile) has finished.
#[tauri::command]
pub async fn wait_startup(state: State<'_, AppState>) -> Res<()> {
    state.wait_ready().await;
    Ok(())
}

// ---------- background, autostart and server ----------

#[tauri::command]
pub fn get_autostart(app: tauri::AppHandle) -> Res<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(ApiError::io)
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Res<()> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(ApiError::io)
}

#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> Res<ServerStatus> {
    let protocol = state.conn.lock().await.as_mut().map(|c| c.client.get_protocol_version());
    Ok(server::status(protocol))
}

#[tauri::command]
pub fn set_server_service(enabled: bool) -> Res<()> {
    server::set_service(enabled).map_err(|e| ApiError::new("service_failed", e))
}

/// Labels for the tray menu in the UI language; rebuilds the menu.
#[tauri::command]
pub fn set_tray_labels(app: tauri::AppHandle, state: State<'_, AppState>, labels: TrayLabels) {
    *state.tray.labels.lock().unwrap() = labels;
    tray::refresh(&app);
}

#[tauri::command]
pub fn refresh_tray(app: tauri::AppHandle) {
    tray::refresh(&app);
}

#[tauri::command]
pub fn show_window(app: tauri::AppHandle) {
    tray::show_main(&app);
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Default fps cap for each of the given device types (Settings placeholders).
#[tauri::command]
pub fn default_fps(types: Vec<String>) -> HashMap<String, u32> {
    types.into_iter().map(|t| (crate::engine::default_fps(&t), t)).map(|(f, t)| (t, f)).collect()
}
