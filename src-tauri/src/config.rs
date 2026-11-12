//! Configuración persistente de chroma-desk (~/.config/chroma-desk/config.json).
//!
//! Guarda lo que OpenRGB "olvida", como el tamaño de las zonas redimensionables
//! (p. ej. el Corsair Lighting Node Core con 3 ventiladores = 24 LEDs).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Tamaño de zona por clave estable "dispositivo|ubicación|zona".
    #[serde(default)]
    pub zone_sizes: HashMap<String, usize>,
}

pub fn zone_key(device: &str, location: &str, zone: &str) -> String {
    format!("{device}|{location}|{zone}")
}

fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("chroma-desk").join("config.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> crate::error::Res<()> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(crate::error::ApiError::io)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(crate::error::ApiError::io)?;
        std::fs::write(p, json).map_err(crate::error::ApiError::io)
    }
}
