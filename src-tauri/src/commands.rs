//! Tauri commands exposed to the frontend.

use crate::animations::{self, AnimList, Animation};
use crate::apply;
use crate::triggers::{self, TriggerResult, TriggerSpec};
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
pub fn list_profiles(state: State<'_, AppState>) -> ProfileList {
    list_with_builtins(&state)
}

/// Built-ins this machine offers, then the user's profiles.
pub fn list_with_builtins(state: &AppState) -> ProfileList {
    let mut l = profiles::list(&profiles::profiles_dir());
    let mut all = crate::builtin::infos(&state.config.lock().unwrap().settings, &state.engine.themes);
    all.append(&mut l.profiles);
    l.profiles = all;
    l
}

/// One profile with its theme, presets and triggers (the scene of a built-in is generated when applied).
#[tauri::command]
pub fn profile_get(state: State<'_, AppState>, name: String) -> Res<Profile> {
    match crate::builtin::find(&name) {
        Some(mut b) => {
            b.enabled = state.config.lock().unwrap().settings.builtin_on(&b);
            Ok(b)
        }
        None => profiles::load(&profiles::profiles_dir(), name.trim()),
    }
}

/// Replaces the theme, presets and triggers of a user profile (built-ins are read-only). When it is
/// the active profile the change is live at once.
#[tauri::command]
pub fn profile_update(state: State<'_, AppState>, name: String, theme: crate::themes::ProfileTheme, presets: Vec<profiles::AnimRef>, triggers: std::collections::BTreeMap<String, profiles::NamedTrigger>) -> Res<()> {
    if crate::builtin::is_builtin(&name) {
        return Err(ApiError::new("profile_builtin", name));
    }
    let dir = profiles::profiles_dir();
    let mut p = profiles::load(&dir, name.trim())?;
    p.theme = theme;
    p.presets = presets;
    p.triggers = triggers;
    profiles::save(&dir, &p)?;
    if state.config.lock().unwrap().settings.current_profile.as_deref() == Some(p.name.as_str()) {
        state.engine.themes.update_theme(p.theme.clone());
        *state.engine.profile_triggers.lock().unwrap() = p.triggers.clone();
    }
    Ok(())
}

#[tauri::command]
pub fn set_builtin_enabled(state: State<'_, AppState>, name: String, on: bool) -> Res<()> {
    let b = crate::builtin::find(&name).ok_or_else(|| ApiError::new("profile_not_found", &name))?;
    let mut cfg = state.config.lock().unwrap();
    cfg.settings.builtin_enabled.insert(b.name, on);
    cfg.save()
}

// ---------- themes ----------

#[tauri::command]
pub fn get_palette(state: State<'_, AppState>) -> crate::themes::ctl::PaletteEvent {
    state.engine.themes.event()
}

#[tauri::command]
pub fn theme_status(state: State<'_, AppState>) -> crate::themes::ctl::ThemeStatus {
    state.engine.themes.status()
}

#[tauri::command]
pub fn palette_reload(state: State<'_, AppState>) -> bool {
    state.engine.themes.reload()
}

/// The built-in automation templates (read-only; the UI makes a copy to edit).
#[tauri::command]
pub fn automation_templates() -> Vec<crate::templates::Template> {
    crate::templates::templates()
}

fn claude_path() -> Res<std::path::PathBuf> {
    crate::claude::default_path().ok_or_else(|| ApiError::new("claude_settings_invalid", "HOME is not set"))
}

/// Whether Claude Code's settings.json has the chroma-desk Notification hook.
#[tauri::command]
pub fn claude_status() -> Res<crate::claude::ClaudeStatus> {
    Ok(crate::claude::status(&claude_path()?))
}

/// Adds the hook (merging, with a backup).
#[tauri::command]
pub fn claude_install() -> Res<crate::claude::ClaudeStatus> {
    crate::claude::install(&claude_path()?)
}

/// Removes the hook again.
#[tauri::command]
pub fn claude_remove() -> Res<crate::claude::ClaudeStatus> {
    crate::claude::remove(&claude_path()?)
}

/// What the system monitors can read on this machine (Settings).
#[tauri::command]
pub async fn sysmon_status() -> Res<crate::sysmon::Status> {
    // Probing loads NVML and may run `nvidia-smi`: keep it off the async threads.
    tokio::task::spawn_blocking(crate::sysmon::status).await.map_err(|e| ApiError::new("internal", e))
}

/// Current values of metrics (`cpu.usage`, `gpu.vram`...) for the editor; keeps the sampler awake.
#[tauri::command]
pub fn sysmon_read(metrics: Vec<String>) -> Vec<crate::sysmon::LiveValue> {
    crate::sysmon::read_many(&metrics)
}

#[tauri::command]
pub fn desktop_status(state: State<'_, AppState>) -> String {
    state.engine.desktop_status.lock().unwrap().clone()
}

#[tauri::command]
pub fn omarchy_event_hooks(state: State<'_, AppState>) -> Vec<crate::themes::hook::EventHookStatus> {
    crate::themes::hook::event_status(&state.engine.themes.env)
}

#[tauri::command]
pub fn omarchy_event_hook_install(state: State<'_, AppState>, kind: String) -> Res<Vec<crate::themes::hook::EventHookStatus>> {
    crate::themes::hook::install_event(&state.engine.themes.env, &kind)?;
    Ok(crate::themes::hook::event_status(&state.engine.themes.env))
}

#[tauri::command]
pub fn omarchy_event_hook_remove(state: State<'_, AppState>, kind: String) -> Res<Vec<crate::themes::hook::EventHookStatus>> {
    crate::themes::hook::remove_event(&state.engine.themes.env, &kind)?;
    Ok(crate::themes::hook::event_status(&state.engine.themes.env))
}

#[tauri::command]
pub fn omarchy_hook_status(state: State<'_, AppState>) -> crate::themes::hook::HookStatus {
    crate::themes::hook::status(&state.engine.themes.env)
}

#[tauri::command]
pub fn omarchy_hook_install(state: State<'_, AppState>) -> Res<crate::themes::hook::HookStatus> {
    crate::themes::hook::install(&state.engine.themes.env)?;
    Ok(crate::themes::hook::status(&state.engine.themes.env))
}

#[tauri::command]
pub fn omarchy_hook_remove(state: State<'_, AppState>) -> Res<crate::themes::hook::HookStatus> {
    crate::themes::hook::remove(&state.engine.themes.env)?;
    Ok(crate::themes::hook::status(&state.engine.themes.env))
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
    // Saving the scene keeps what the profile already has besides it (theme, presets, triggers).
    let mut profile = profiles::load(&profiles::profiles_dir(), name.trim()).unwrap_or_else(|_| {
        // A new profile keeps the live theme and triggers: its scene is made of slots of that theme.
        let mut p = Profile::new(&name, Scene::default());
        p.theme = state.engine.themes.theme();
        p.triggers = state.engine.profile_triggers.lock().unwrap().clone();
        p
    });
    profile.name = name;
    profile.zone_sizes = zone_sizes;
    profile.scene = scene;
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
    if crate::builtin::is_builtin(&name) {
        return Err(ApiError::new("profile_builtin", name));
    }
    profiles::delete(&profiles::profiles_dir(), &name)?;
    state.forget_profile(&name)
}

#[tauri::command]
pub fn rename_profile(state: State<'_, AppState>, old: String, new: String) -> Res<ProfileInfo> {
    if crate::builtin::is_builtin(&old) {
        return Err(ApiError::new("profile_builtin", old));
    }
    let info = profiles::rename(&profiles::profiles_dir(), &old, &new)?;
    state.rename_profile_refs(&old, &info.name)?;
    Ok(info)
}

#[tauri::command]
pub fn duplicate_profile(state: State<'_, AppState>, name: String, new: String) -> Res<ProfileInfo> {
    let dir = profiles::profiles_dir();
    if crate::builtin::find(&name).is_some() {
        // "Duplicate to edit": freeze the generated scene for the connected devices.
        let new = profiles::validate_name(&new)?;
        let devs = state.engine.devs.lock().unwrap().clone();
        let p = crate::builtin::materialize(&name, &devs, &new).ok_or_else(|| ApiError::new("profile_not_found", &name))?;
        return profiles::save(&dir, &p);
    }
    profiles::duplicate(&dir, &name, &new)
}

#[tauri::command]
pub fn export_profile(name: String, path: String) -> Res<()> {
    if crate::builtin::is_builtin(&name) {
        return Err(ApiError::new("profile_builtin", name));
    }
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
    let mut profile = Profile::new(&pname, Scene::default());
    profile.openrgb_profile = Some(name);
    profiles::save(&dir, &profile)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.config.lock().unwrap().settings.clone()
}

#[tauri::command]
pub fn set_settings(state: State<'_, AppState>, settings: Settings) -> Res<()> {
    profiles::validate_triggers(&settings.global_notifications)?;
    *state.engine.fps_by_type.lock().unwrap() = settings.fps_by_type.clone();
    state.engine.themes.set_name_watch(!settings.theme_rules.is_empty());
    *state.engine.global_triggers.lock().unwrap() = settings.global_notifications.clone();
    state.engine.desktop_enabled.store(settings.desktop_notifications, std::sync::atomic::Ordering::Relaxed);
    crate::sysmon::set_gpu(&settings.sysmon_gpu);
    let mut cfg = state.config.lock().unwrap();
    // The UI's copy of `current_profile` can be stale (the tray applies profiles too); only the
    // apply/save paths own that field.
    let current = cfg.settings.current_profile.clone();
    cfg.settings = Settings { current_profile: current, ..settings };
    cfg.save()
}

/// Resolves once the startup sequence (connect, default profile) has finished.
#[tauri::command]
pub async fn wait_startup(state: State<'_, AppState>) -> Res<()> {
    state.wait_ready().await;
    Ok(())
}

// ---------- background, autostart and server ----------

/// Restarts the OpenRGB server (unfreezes the LEDs); the UI reconnects afterwards.
#[tauri::command]
pub async fn restart_openrgb() -> Res<()> {
    let by_service = tauri::async_runtime::spawn_blocking(server::stop_openrgb)
        .await
        .map_err(|e| ApiError::new("server_start_failed", e))?
        .map_err(|e| ApiError::new("server_start_failed", e))?;
    if by_service {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    crate::sdk::ensure_server().await
}

/// Downloads the portable OpenRGB (Windows) and starts its server.
#[tauri::command]
pub async fn install_openrgb() -> Res<()> {
    tauri::async_runtime::spawn_blocking(server::install_portable)
        .await
        .map_err(|e| ApiError::new("install_failed", e))?
        .map_err(|e| ApiError::new("install_failed", e))?;
    crate::sdk::ensure_server().await
}

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

// ---------- animation library and triggers ----------

#[tauri::command]
pub fn anim_list() -> AnimList {
    animations::list_all(&animations::animations_dir())
}

/// Layers of a built-in layered theme (e.g. "Cyberpunk") for one device, base look only.
#[tauri::command]
pub fn theme_layers(state: State<'_, AppState>, name: String, device_key: String) -> Res<Vec<crate::engine::compositor::Layer>> {
    let devs = state.engine.devs.lock().unwrap();
    let dev = devs.iter().find(|d| d.key == device_key).ok_or_else(|| ApiError::new("device_not_found", &device_key))?;
    crate::builtin::theme_layers(&name, dev).ok_or_else(|| ApiError::new("anim_not_found", &name))
}

#[tauri::command]
pub fn anim_get(id: String) -> Res<Animation> {
    animations::get(&animations::animations_dir(), &id)
}

#[tauri::command]
pub fn anim_save(anim: Animation) -> Res<Animation> {
    animations::save(&animations::animations_dir(), &anim)
}

#[tauri::command]
pub fn anim_delete(id: String) -> Res<()> {
    animations::delete(&animations::animations_dir(), &id)
}

#[tauri::command]
pub fn anim_duplicate(id: String, name: String) -> Res<Animation> {
    animations::duplicate(&animations::animations_dir(), &id, &name)
}

#[tauri::command]
pub fn anim_import(path: String) -> Res<Animation> {
    animations::import(&animations::animations_dir(), std::path::Path::new(&path))
}

#[tauri::command]
pub fn anim_export(id: String, path: String) -> Res<()> {
    animations::export(&animations::animations_dir(), &id, std::path::Path::new(&path))
}

/// Puts animation `id` on one device (`device_key`) or on all of them (`"all"`), replacing their
/// layers and switching them to Effects mode. Returns the new scene (also sent as `scene-changed`).
#[tauri::command]
pub fn anim_apply(app: tauri::AppHandle, state: State<'_, AppState>, device_key: String, id: String) -> Res<Scene> {
    let anim = animations::get(&animations::animations_dir(), &id)?;
    let keys: Vec<String> = if device_key == "all" { state.engine.devs.lock().unwrap().iter().map(|d| d.key.clone()).collect() } else { vec![device_key] };
    let scene = {
        let mut scene = state.engine.scene.lock().unwrap();
        for key in keys {
            let ds = scene.devices.entry(key).or_default();
            ds.mode = crate::engine::SceneMode::Effects;
            ds.layers = vec![animations::layer_for(&anim)];
        }
        scene.clone()
    };
    use tauri::Emitter;
    let _ = app.emit("scene-changed", &scene);
    Ok(scene)
}

/// Starts a temporary overlay animation (the same thing the CLI and the socket do).
#[tauri::command]
pub fn trigger(state: State<'_, AppState>, spec: TriggerSpec) -> Res<TriggerResult> {
    triggers::start(&state.engine, &animations::animations_dir(), &spec, std::time::Instant::now())
}

/// Fires an automation as edited (not necessarily saved), for the editor's Test button.
#[tauri::command]
pub fn test_automation(state: State<'_, AppState>, name: String, automation: profiles::NamedTrigger) -> Res<TriggerResult> {
    triggers::test_notification(&state.engine, &animations::animations_dir(), name.trim(), &automation, std::time::Instant::now())
}

/// Lets paused always-on automations run again.
#[tauri::command]
pub fn resume_triggers(state: State<'_, AppState>) -> usize {
    triggers::resume_always(&state.engine)
}

/// Always-on automations that "Stop all" paused.
#[tauri::command]
pub fn paused_automations(state: State<'_, AppState>) -> Vec<String> {
    triggers::paused_always(&state.engine)
}

/// Stops every running overlay and pauses always-on automations.
#[tauri::command]
pub fn stop_triggers(state: State<'_, AppState>) -> usize {
    triggers::stop_all(&state.engine)
}

/// Features this platform supports (the UI hides the rest).
#[tauri::command]
pub fn capabilities() -> Vec<&'static str> {
    crate::capabilities::list()
}

/// Where scripts reach the app (`null` without `XDG_RUNTIME_DIR`).
#[tauri::command]
pub fn trigger_socket_path() -> Option<String> {
    crate::socket::socket_path().map(|p| p.to_string_lossy().into_owned())
}
