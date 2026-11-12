//! Tauri commands exposed to the frontend.

use crate::config::zone_key;
use crate::engine::effects::{self, EffectDef, Params, Pos};
use crate::engine::{Scene, SceneMode};
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
        let mut cfg = state.config.lock().unwrap();
        cfg.zone_sizes.insert(zone_key(c.name(), c.location(), z.name()), size);
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

impl AppState {
    pub async fn set_blackout(&self, on: bool) -> Res<()> {
        self.engine.blackout.store(on, Ordering::Relaxed);
        let scene = self.engine.scene.lock().unwrap().clone();
        let devs = self.engine.devs.lock().unwrap().clone();
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        for d in devs {
            let ds = scene.devices.get(&d.key);
            if ds.is_some_and(|s| s.mode == SceneMode::Effects) {
                continue;
            }
            let Ok(c) = conn.group.get_controller(d.index) else { continue };
            if on {
                let _ = sdk::set_device_color(c, Rgb(0, 0, 0)).await;
            } else if let Some(hw) = ds.and_then(|s| s.hardware.as_ref())
                && let Some(idx) = c.modes().iter().position(|m| m.name() == hw.mode)
            {
                let _ = sdk::apply_mode(c, &hw.to_request(idx)).await;
            }
        }
        Ok(())
    }
}
