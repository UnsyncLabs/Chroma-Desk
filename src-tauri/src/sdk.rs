//! Capa sobre el SDK de OpenRGB (crate `openrgb2`, vendorizado con parches).
//!
//! Convierte los datos del servidor en estructuras serializables para el frontend
//! y ofrece operaciones de alto nivel (color, LEDs, modos, tamaño de zonas).

use crate::error::{ApiError, Res};
use openrgb2::{Color, Controller, Direction, ModeFlag, OpenRgbClient};
use serde::{Deserialize, Serialize};
use std::net::TcpStream;
use std::time::Duration;

pub const SDK_PORT: u16 = 6742;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl From<Rgb> for Color {
    fn from(c: Rgb) -> Self {
        Color { r: c.0, g: c.1, b: c.2 }
    }
}
impl From<Color> for Rgb {
    fn from(c: Color) -> Self {
        Rgb(c.r, c.g, c.b)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MatrixDto {
    pub width: u32,
    pub height: u32,
    /// Índices de LED (relativos a la zona) por fila; `null` = hueco.
    pub map: Vec<Option<u32>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZoneDto {
    pub id: usize,
    pub name: String,
    pub zone_type: String,
    pub num_leds: usize,
    pub leds_min: usize,
    pub leds_max: usize,
    pub resizable: bool,
    /// Índice del primer LED de esta zona dentro del dispositivo.
    pub offset: usize,
    pub matrix: Option<MatrixDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeDto {
    pub id: usize,
    pub name: String,
    pub has_speed: bool,
    pub speed: Option<u32>,
    pub speed_min: Option<u32>,
    pub speed_max: Option<u32>,
    pub has_brightness: bool,
    pub brightness: Option<u32>,
    pub brightness_min: Option<u32>,
    pub brightness_max: Option<u32>,
    pub has_direction: bool,
    pub direction: Option<u32>,
    /// 0 ninguno, 1 por LED, 2 colores del modo, 3 aleatorio.
    pub color_mode: u32,
    pub colors: Vec<Rgb>,
    pub colors_min: Option<u32>,
    pub colors_max: Option<u32>,
    pub can_save: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceDto {
    pub id: usize,
    /// Stable identity (see `identity`), filled in by the caller.
    pub key: String,
    pub name: String,
    pub vendor: String,
    pub description: String,
    pub version: String,
    pub serial: String,
    pub location: String,
    pub device_type: String,
    pub active_mode: usize,
    pub modes: Vec<ModeDto>,
    pub zones: Vec<ZoneDto>,
    pub led_names: Vec<String>,
    pub colors: Vec<Rgb>,
    /// A phantom duplicate OpenRGB reports for the same physical device (see `phantom_flags`).
    pub phantom: bool,
}

/// Comprueba si el servidor de OpenRGB escucha en el puerto del SDK.
pub fn server_running() -> bool {
    TcpStream::connect_timeout(&([127, 0, 0, 1], SDK_PORT).into(), Duration::from_millis(300)).is_ok()
}

/// Arranca `openrgb --server` en segundo plano si no está corriendo y espera a que responda.
pub async fn ensure_server() -> Res<()> {
    if server_running() {
        return Ok(());
    }
    std::process::Command::new("openrgb")
        .args(["--server", "--startminimized"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| ApiError::new("server_start_failed", e))?;
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if server_running() {
            // La detección de dispositivos continúa unos segundos tras abrir el puerto.
            tokio::time::sleep(Duration::from_secs(4)).await;
            return Ok(());
        }
    }
    Err(ApiError::new("server_timeout", ""))
}

/// Connects to the SDK server. With `spawn`, starts `openrgb --server` if it is not running;
/// without it (background reconnects) a missing server is reported as `server_down`.
pub async fn connect(spawn: bool) -> Res<OpenRgbClient> {
    if spawn {
        ensure_server().await?;
    } else if !server_running() {
        return Err(ApiError::new("server_down", ""));
    }
    let mut client = OpenRgbClient::connect().await.map_err(ApiError::sdk)?;
    let _ = client.set_name("chroma-desk").await;
    Ok(client)
}

fn direction_from(v: u32) -> Direction {
    match v {
        1 => Direction::Right,
        2 => Direction::Up,
        3 => Direction::Down,
        4 => Direction::Horizontal,
        5 => Direction::Vertical,
        _ => Direction::Left,
    }
}

fn opt<T>(cond: bool, v: T) -> Option<T> {
    cond.then_some(v)
}

pub fn device_dto(c: &Controller) -> DeviceDto {
    let modes = c
        .modes()
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let f = m.flags();
            let has_speed = f.contains(ModeFlag::HasSpeed);
            let has_brightness = f.contains(ModeFlag::HasBrightness);
            let has_direction = f.contains(ModeFlag::HasDirectionLR)
                || f.contains(ModeFlag::HasDirectionUD)
                || f.contains(ModeFlag::HasDirectionHV);
            ModeDto {
                id: i,
                name: m.name().to_string(),
                has_speed,
                speed: opt(has_speed, m.speed()).flatten(),
                speed_min: opt(has_speed, m.speed_min()).flatten(),
                speed_max: opt(has_speed, m.speed_max()).flatten(),
                has_brightness,
                brightness: opt(has_brightness, m.brightness()).flatten(),
                brightness_min: opt(has_brightness, m.brightness_min()).flatten(),
                brightness_max: opt(has_brightness, m.brightness_max()).flatten(),
                has_direction,
                direction: m.direction().map(|d| d as u32),
                color_mode: m.color_mode_raw(),
                colors: m.colors().iter().map(|c| (*c).into()).collect(),
                colors_min: m.colors_min(),
                colors_max: m.colors_max(),
                can_save: f.contains(ModeFlag::ManualSave),
            }
        })
        .collect();

    let zones = c
        .get_all_zones()
        .map(|z| {
            let matrix = z.matrix_data().map(|m| MatrixDto {
                width: m.width(),
                height: m.height(),
                map: m.data().iter().map(|&v| (v != u32::MAX).then_some(v)).collect(),
            });
            ZoneDto {
                id: z.id(),
                name: z.name().to_string(),
                zone_type: format!("{:?}", z.zone_type()),
                num_leds: z.num_leds(),
                leds_min: z.leds_min(),
                leds_max: z.leds_max(),
                resizable: z.leds_min() != z.leds_max(),
                offset: z.offset(),
                matrix,
            }
        })
        .collect();

    DeviceDto {
        id: c.id(),
        key: String::new(),
        name: c.name().to_string(),
        vendor: c.vendor().to_string(),
        description: c.description().to_string(),
        version: c.version().to_string(),
        serial: c.serial().to_string(),
        location: c.location().to_string(),
        device_type: format!("{:?}", c.device_type()),
        active_mode: c.mode_iter().position(|m| m.is_active()).unwrap_or(0),
        modes,
        zones,
        led_names: c.led_iter().map(|l| l.name().to_string()).collect(),
        colors: c.colors().iter().map(|c| (*c).into()).collect(),
        phantom: false,
    }
}

/// Pone el dispositivo en modo Direct (o Static si no tiene Direct).
/// Devuelve `true` si quedó en Direct (control por LED).
pub async fn controllable(c: &Controller) -> Res<bool> {
    if c.set_controllable_mode().await.is_ok() {
        return Ok(true);
    }
    if let Some(m) = c.modes().iter().find(|m| m.name().eq_ignore_ascii_case("static")) {
        c.update_mode(m).await.map_err(ApiError::sdk)?;
    }
    Ok(false)
}

/// Pinta todo el dispositivo de un color (usa Static con color si no hay Direct).
pub async fn set_device_color(c: &Controller, color: Rgb) -> Res<()> {
    if controllable(c).await? {
        c.set_all_leds(color).await.map_err(ApiError::sdk)
    } else if let Some(m) = c.modes().iter().find(|m| m.name().eq_ignore_ascii_case("static")) {
        let mut m = m.clone();
        let n = m.colors().len().max(1);
        m.set_colors(vec![color.into(); n]);
        c.update_mode(&m).await.map_err(ApiError::sdk)
    } else {
        Err(ApiError::new("no_static_color", c.name()))
    }
}

pub async fn set_leds(c: &Controller, colors: &[Rgb], ensure_direct: bool) -> Res<()> {
    if ensure_direct {
        controllable(c).await?;
    }
    c.set_leds(colors.iter().copied()).await.map_err(ApiError::sdk)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModeRequest {
    pub mode: usize,
    pub speed: Option<u32>,
    pub brightness: Option<u32>,
    pub direction: Option<u32>,
    pub colors: Option<Vec<Rgb>>,
    #[serde(default)]
    pub save: bool,
}

pub async fn apply_mode(c: &Controller, req: &ModeRequest) -> Res<()> {
    let mut m = c
        .modes()
        .get(req.mode)
        .ok_or_else(|| ApiError::new("mode_not_found", ""))?
        .clone();
    if let Some(s) = req.speed {
        m.set_speed(s);
    }
    if let Some(b) = req.brightness {
        m.set_brightness(b);
    }
    if let Some(d) = req.direction {
        m.set_direction(direction_from(d));
    }
    if let Some(cols) = &req.colors {
        m.set_colors(cols.iter().map(|&c| c.into()).collect());
    }
    c.update_mode(&m).await.map_err(ApiError::sdk)?;
    if req.save {
        c.save_mode().await.map_err(ApiError::sdk)?;
    }
    Ok(())
}

/// Flags phantom duplicates: OpenRGB's Logitech Lightspeed detector reports a wired mouse a second
/// time as a "receiver" device. A device is a phantom when it has no serial, its location is
/// `<location of another device with the same name> (Receiver)...` and such a device exists.
pub fn phantom_flags<'a>(devices: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) -> Vec<bool> {
    let list: Vec<_> = devices.into_iter().collect();
    list.iter()
        .enumerate()
        .map(|(i, (name, location, serial))| {
            let Some((base, _)) = location.split_once(" (Receiver)") else { return false };
            serial.trim().is_empty()
                && list.iter().enumerate().any(|(j, (n2, l2, _))| j != i && n2 == name && l2.trim() == base.trim())
        })
        .collect()
}

#[cfg(test)]
mod phantom_tests {
    use super::*;

    #[test]
    fn lightspeed_phantom_is_detected() {
        let f = phantom_flags([
            ("ENE DRAM", "I2C: /dev/i2c-5, address 0x71", ""),
            ("G502 HERO Gaming Mouse", "HID: /dev/hidraw3", "34394708"),
            ("G502 HERO Gaming Mouse", "HID: /dev/hidraw3 (Receiver) \nWireless Index: 255", ""),
            ("G502 HERO Gaming Mouse", "HID: /dev/hidraw7 (Receiver) \nWireless Index: 255", ""),
        ]);
        // The 4th has no wired twin at hidraw7: it is a real (wireless) device and stays.
        assert_eq!(f, vec![false, false, true, false]);
    }

    #[test]
    fn serial_or_different_name_is_never_a_phantom() {
        let f = phantom_flags([("A", "HID: /dev/hidraw1", "1"), ("A", "HID: /dev/hidraw1 (Receiver) \nWireless Index: 1", "S2"), ("B", "HID: /dev/hidraw1 (Receiver)", "")]);
        assert_eq!(f, vec![false, false, false]);
    }
}
