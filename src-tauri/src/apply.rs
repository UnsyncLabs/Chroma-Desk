//! Applying scenes and profiles to the hardware, and the startup sequence.

use crate::engine::runner::Emit;
use crate::engine::{Scene, SceneMode};
use crate::error::{ApiError, Res};
use crate::builtin;
use crate::engine::Engine;
use crate::profiles::{self, Profile};
use crate::sdk::{self, Rgb};
use crate::state::AppState;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::Emitter;

/// Event emitter forwarding to the webview.
pub fn emitter(app: &tauri::AppHandle) -> Emit {
    let handle = app.clone();
    Arc::new(move |event, payload| {
        let _ = handle.emit(event, payload);
    })
}

impl AppState {
    /// Re-applies the recorded native mode of every device that is in Hardware mode.
    pub async fn apply_hardware_modes(&self) -> Res<()> {
        let scene = self.engine.scene.lock().unwrap().clone();
        let devs = self.engine.devs.lock().unwrap().clone();
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        for d in devs {
            let Some(ds) = scene.devices.get(&d.key) else { continue };
            if ds.mode != SceneMode::Hardware {
                continue;
            }
            let Some(hw) = &ds.hardware else { continue };
            let Ok(c) = conn.group.get_controller(d.index) else { continue };
            if let Some(idx) = c.modes().iter().position(|m| m.name() == hw.mode) {
                let _ = sdk::apply_mode(c, &hw.to_request(idx)).await;
            }
        }
        Ok(())
    }

    /// Lights off / on. Effects devices go black through the engine; hardware-mode devices get
    /// a black colour now and their recorded native mode back when turned on (devices that never
    /// had a mode recorded stay black until touched).
    pub async fn set_blackout(&self, on: bool) -> Res<()> {
        self.engine.blackout.store(on, Ordering::Relaxed);
        if !on {
            return self.apply_hardware_modes().await;
        }
        let scene = self.engine.scene.lock().unwrap().clone();
        let devs = self.engine.devs.lock().unwrap().clone();
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        for d in devs {
            if scene.devices.get(&d.key).is_some_and(|s| s.mode == SceneMode::Effects) {
                continue;
            }
            if let Ok(c) = conn.group.get_controller(d.index) {
                let _ = sdk::set_device_color(c, Rgb(0, 0, 0)).await;
            }
        }
        Ok(())
    }

    /// Connects (starting OpenRGB if needed), restores zone sizes and refreshes the engine's device list.
    pub async fn connect_all(&self) -> Res<()> {
        self.refresh(true).await?;
        if self.apply_saved_zone_sizes().await? {
            self.refresh(true).await?;
        }
        self.devices().await?;
        Ok(())
    }
}

/// Makes the profile's theme and named triggers the live ones.
pub fn adopt_theme(engine: &Engine, profile: &Profile) {
    engine.themes.set_theme(profile.theme.clone());
    *engine.profile_triggers.lock().unwrap() = profile.triggers.clone();
}

/// Applies the profile a theme rule asks for when the Omarchy theme becomes `theme`.
pub async fn apply_theme_rule(state: &AppState, emit: &Emit, theme: &str) {
    let rule = state.config.lock().unwrap().settings.theme_rules.iter().find(|r| r.theme.eq_ignore_ascii_case(theme)).cloned();
    let Some(rule) = rule else { return };
    if state.config.lock().unwrap().settings.current_profile.as_deref() == Some(rule.profile.as_str()) {
        return;
    }
    match apply_profile(state, emit, &rule.profile).await {
        Ok(_) => emit("profile-applied", serde_json::json!({ "name": rule.profile, "reason": "theme_rule", "theme": theme })),
        Err(e) => emit("startup-error", serde_json::json!({ "code": e.code, "detail": e.detail })),
    }
}

/// Applies profile `name`: OpenRGB profile (if any), zone sizes, scene and native modes.
pub async fn apply_profile(state: &AppState, emit: &Emit, name: &str) -> Res<Scene> {
    let mut profile = match builtin::find(name) {
        Some(b) => {
            if !builtin::available(&b, &state.engine.themes) {
                return Err(ApiError::new("profile_unavailable", &b.name));
            }
            if !state.config.lock().unwrap().settings.builtin_on(&b) {
                return Err(ApiError::new("profile_disabled", &b.name));
            }
            b
        }
        None => profiles::load(&profiles::profiles_dir(), name)?,
    };
    if let Some(orgb) = &profile.openrgb_profile {
        let guard = state.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        conn.client.load_profile(orgb.clone()).await.map_err(ApiError::sdk)?;
    }
    let resized = state.apply_zone_sizes(&profile.zone_sizes).await?;
    if !profile.zone_sizes.is_empty() {
        let mut cfg = state.config.lock().unwrap();
        cfg.zone_sizes_by_key.extend(profile.zone_sizes.clone());
        cfg.save()?;
    }
    if resized {
        state.refresh(true).await?;
        emit("devices-changed", serde_json::Value::Null);
    }
    state.devices().await?;
    if profile.builtin {
        let devs = state.engine.devs.lock().unwrap().clone();
        profile.scene = builtin::scene_for(&profile.name, &devs);
    }
    adopt_theme(&state.engine, &profile);
    *state.engine.scene.lock().unwrap() = profile.scene.clone();
    state.engine.blackout.store(false, Ordering::Relaxed);
    state.apply_hardware_modes().await?;
    state.set_current_profile(Some(profile.name))?;
    emit("scene-changed", serde_json::json!(profile.scene));
    Ok(profile.scene)
}

/// Startup sequence: connect, then apply `requested` (CLI) or the default profile.
/// Returns the applied scene, if any. Always marks the state ready.
pub async fn startup(state: AppState, emit: Emit, requested: Option<String>) -> Option<Scene> {
    let fps = state.config.lock().unwrap().settings.fps_by_type.clone();
    *state.engine.fps_by_type.lock().unwrap() = fps;
    {
        let cfg = state.config.lock().unwrap();
        state.engine.themes.set_name_watch(!cfg.settings.theme_rules.is_empty());
        *state.engine.global_triggers.lock().unwrap() = cfg.settings.global_notifications.clone();
        state.engine.desktop_enabled.store(cfg.settings.desktop_notifications, Ordering::Relaxed);
        crate::sysmon::set_gpu(&cfg.settings.sysmon_gpu);
    }
    let mut applied = None;
    match state.connect_all().await {
        Ok(()) => {
            let name = requested.or_else(|| state.config.lock().unwrap().settings.default_profile.clone());
            if let Some(name) = name {
                match apply_profile(&state, &emit, &name).await {
                    Ok(s) => applied = Some(s),
                    Err(e) => emit("startup-error", serde_json::json!({ "code": e.code, "detail": e.detail })),
                }
            }
        }
        Err(e) => emit("startup-error", serde_json::json!({ "code": e.code, "detail": e.detail })),
    }
    state.mark_ready();
    applied
}
