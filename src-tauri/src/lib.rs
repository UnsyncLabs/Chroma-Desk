mod config;
mod sdk;

use config::{zone_key, Config};
use openrgb2::{ControllerGroup, OpenRgbClient};
use sdk::{DeviceDto, ModeRequest, Rgb};
use tokio::sync::Mutex;

struct Conn {
    client: OpenRgbClient,
    group: ControllerGroup,
}

#[derive(Default)]
struct AppState {
    conn: Mutex<Option<Conn>>,
    config: std::sync::Mutex<Config>,
}

type Res<T> = Result<T, String>;

impl AppState {
    /// Conecta si hace falta y vuelve a leer todos los dispositivos.
    async fn refresh(&self) -> Res<()> {
        let mut guard = self.conn.lock().await;
        if guard.is_none() {
            let client = sdk::connect().await?;
            let group = client.get_all_controllers().await.map_err(|e| e.to_string())?;
            *guard = Some(Conn { client, group });
        } else {
            let conn = guard.as_mut().unwrap();
            match conn.client.get_all_controllers().await {
                Ok(g) => conn.group = g,
                Err(_) => {
                    // Conexión caída: reconectar.
                    *guard = None;
                    drop(guard);
                    return Box::pin(self.refresh()).await;
                }
            }
        }
        Ok(())
    }

    /// Vuelve a aplicar los tamaños de zona guardados (OpenRGB los olvida al reiniciar).
    async fn apply_saved_zone_sizes(&self) -> Res<bool> {
        let sizes = self.config.lock().unwrap().zone_sizes.clone();
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or("Sin conexión")?;
        let mut changed = false;
        for c in conn.group.iter() {
            for z in c.get_all_zones() {
                let key = zone_key(c.name(), c.location(), z.name());
                if let Some(&n) = sizes.get(&key) {
                    if n != z.num_leds() && n >= z.leds_min() && n <= z.leds_max() {
                        z.resize(n).await.map_err(|e| e.to_string())?;
                        changed = true;
                    }
                }
            }
        }
        Ok(changed)
    }

    async fn devices(&self) -> Res<Vec<DeviceDto>> {
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or("Sin conexión")?;
        Ok(conn.group.iter().map(sdk::device_dto).collect())
    }
}

#[tauri::command]
async fn connect(state: tauri::State<'_, AppState>) -> Res<Vec<DeviceDto>> {
    state.refresh().await?;
    if state.apply_saved_zone_sizes().await? {
        state.refresh().await?;
    }
    state.devices().await
}

#[tauri::command]
async fn list_devices(state: tauri::State<'_, AppState>) -> Res<Vec<DeviceDto>> {
    state.refresh().await?;
    state.devices().await
}

#[tauri::command]
async fn rescan(state: tauri::State<'_, AppState>) -> Res<Vec<DeviceDto>> {
    {
        let guard = state.conn.lock().await;
        if let Some(conn) = guard.as_ref() {
            conn.client.rescan_devices().await.map_err(|e| e.to_string())?;
        }
    }
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    connect(state).await
}

#[tauri::command]
async fn set_color_all(state: tauri::State<'_, AppState>, color: Rgb) -> Res<Vec<String>> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or("Sin conexión")?;
    let mut errors = vec![];
    for c in conn.group.iter() {
        if let Err(e) = sdk::set_device_color(c, color).await {
            errors.push(format!("{}: {e}", c.name()));
        }
    }
    Ok(errors)
}

#[tauri::command]
async fn set_device_color(state: tauri::State<'_, AppState>, device: usize, color: Rgb) -> Res<()> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or("Sin conexión")?;
    let c = conn.group.get_controller(device).map_err(|e| e.to_string())?;
    sdk::set_device_color(c, color).await
}

/// Envía todos los colores de LED de un dispositivo (pone Direct si `ensure_direct`).
#[tauri::command]
async fn set_leds(
    state: tauri::State<'_, AppState>,
    device: usize,
    colors: Vec<Rgb>,
    ensure_direct: Option<bool>,
) -> Res<()> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or("Sin conexión")?;
    let c = conn.group.get_controller(device).map_err(|e| e.to_string())?;
    sdk::set_leds(c, &colors, ensure_direct.unwrap_or(false)).await
}

#[tauri::command]
async fn set_mode(state: tauri::State<'_, AppState>, device: usize, req: ModeRequest) -> Res<()> {
    let guard = state.conn.lock().await;
    let conn = guard.as_ref().ok_or("Sin conexión")?;
    let c = conn.group.get_controller(device).map_err(|e| e.to_string())?;
    sdk::apply_mode(c, &req).await
}

/// Cambia el tamaño de una zona y lo guarda para volver a aplicarlo en cada arranque.
#[tauri::command]
async fn resize_zone(
    state: tauri::State<'_, AppState>,
    device: usize,
    zone: usize,
    size: usize,
) -> Res<Vec<DeviceDto>> {
    {
        let guard = state.conn.lock().await;
        let conn = guard.as_ref().ok_or("Sin conexión")?;
        let c = conn.group.get_controller(device).map_err(|e| e.to_string())?;
        let z = c.get_zone(zone).map_err(|e| e.to_string())?;
        if size < z.leds_min() || size > z.leds_max() {
            return Err(format!("Tamaño fuera de rango ({}–{})", z.leds_min(), z.leds_max()));
        }
        z.resize(size).await.map_err(|e| e.to_string())?;
        let mut cfg = state.config.lock().unwrap();
        cfg.zone_sizes.insert(zone_key(c.name(), c.location(), z.name()), size);
        cfg.save()?;
    }
    list_devices(state).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState {
        config: std::sync::Mutex::new(Config::load()),
        ..Default::default()
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            connect,
            list_devices,
            rescan,
            set_color_all,
            set_device_color,
            set_leds,
            set_mode,
            resize_zone
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
