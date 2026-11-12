//! Persistent configuration (~/.config/chroma-desk/config.json): settings and the zone sizes
//! OpenRGB forgets on restart (e.g. a Corsair Lighting Node with 3 fans = 24 LEDs).

use crate::error::{ApiError, Res};
use crate::fsutil::write_atomic;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// UI language code; `None` = detect from the system.
    #[serde(default)]
    pub language: Option<String>,
    /// Start in the tray without showing the window.
    #[serde(default)]
    pub start_hidden: bool,
    /// Profile applied automatically at startup.
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Profile currently applied (shown in the UI).
    #[serde(default)]
    pub current_profile: Option<String>,
    /// fps caps by device type.
    #[serde(default)]
    pub fps_by_type: HashMap<String, u32>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Legacy (phases 1-3) sizes keyed `name|location|zone`; migrated into `zone_sizes_by_key`.
    #[serde(default)]
    pub zone_sizes: HashMap<String, usize>,
    /// Remembered zone sizes keyed `<device identity>|<zone name>`.
    #[serde(default)]
    pub zone_sizes_by_key: BTreeMap<String, usize>,
    #[serde(default)]
    pub settings: Settings,
}

/// Legacy zone key (phases 1-3).
pub fn legacy_zone_key(device: &str, location: &str, zone: &str) -> String {
    format!("{device}|{location}|{zone}")
}

/// Key of a zone inside a device, by stable identity.
pub fn zone_key(device_key: &str, zone: &str) -> String {
    format!("{device_key}|{zone}")
}

/// A connected device as seen by the legacy-size migration.
pub struct LegacyDevice<'a> {
    pub name: &'a str,
    pub location: &'a str,
    pub key: &'a str,
    pub zones: Vec<&'a str>,
}

/// Converts legacy `name|location|zone` sizes into identity-based keys. An exact match wins;
/// if the location changed (hidraw paths do) a name that is unique among devices still matches.
pub fn migrate_zone_sizes(legacy: &HashMap<String, usize>, devices: &[LegacyDevice]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for d in devices {
        let unique = devices.iter().filter(|o| o.name == d.name).count() == 1;
        for z in &d.zones {
            let exact = legacy.get(&legacy_zone_key(d.name, d.location, z));
            let loose = || {
                let prefix = format!("{}|", d.name);
                let suffix = format!("|{z}");
                legacy.iter().find(|(k, _)| k.starts_with(&prefix) && k.ends_with(&suffix) && k.matches('|').count() == 2).map(|(_, v)| v)
            };
            if let Some(&n) = exact.or_else(|| if unique { loose() } else { None }) {
                out.insert(zone_key(d.key, z), n);
            }
        }
    }
    out
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("chroma-desk")
}

fn path() -> PathBuf {
    config_dir().join("config.json")
}

impl Config {
    pub fn load() -> Self {
        Self::load_from(&path())
    }

    /// Missing or corrupt files give the defaults (a corrupt one is kept as `.corrupt` for inspection).
    pub fn load_from(p: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(p) else { return Self::default() };
        serde_json::from_str(&text).unwrap_or_else(|_| {
            let _ = std::fs::rename(p, p.with_extension("json.corrupt"));
            Self::default()
        })
    }

    pub fn save(&self) -> Res<()> {
        self.save_to(&path())
    }

    pub fn save_to(&self, p: &Path) -> Res<()> {
        let json = serde_json::to_vec_pretty(self).map_err(ApiError::io)?;
        write_atomic(p, &json).map_err(ApiError::io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir;

    #[test]
    fn roundtrip_and_defaults() {
        let d = test_dir("cfg");
        let p = d.join("config.json");
        assert_eq!(Config::load_from(&p).settings, Settings::default());
        let mut c = Config::default();
        c.settings.language = Some("es".into());
        c.zone_sizes_by_key.insert("Node#1|Header".into(), 24);
        c.save_to(&p).unwrap();
        let back = Config::load_from(&p);
        assert_eq!(back.settings.language.as_deref(), Some("es"));
        assert_eq!(back.zone_sizes_by_key["Node#1|Header"], 24);
    }

    #[test]
    fn old_config_without_new_fields_loads() {
        let d = test_dir("cfg-old");
        let p = d.join("config.json");
        std::fs::write(&p, r#"{"zone_sizes": {"Node|loc|Header 1": 24}}"#).unwrap();
        let c = Config::load_from(&p);
        assert_eq!(c.zone_sizes["Node|loc|Header 1"], 24);
        assert!(c.zone_sizes_by_key.is_empty());
    }

    #[test]
    fn corrupt_config_is_replaced_by_defaults_and_kept() {
        let d = test_dir("cfg-bad");
        let p = d.join("config.json");
        std::fs::write(&p, "{ not json").unwrap();
        assert!(Config::load_from(&p).zone_sizes.is_empty());
        assert!(d.join("config.json.corrupt").exists());
    }

    #[test]
    fn legacy_sizes_migrate_to_identity_keys() {
        let mut legacy = HashMap::new();
        legacy.insert("Node|HID: /dev/hidraw3|Header 1".to_string(), 24);
        legacy.insert("Mouse|HID: /dev/hidraw5|Logo".to_string(), 3);
        let devs = [
            LegacyDevice { name: "Node", location: "HID: /dev/hidraw7", key: "Node#ABC", zones: vec!["Header 1", "Header 2"] },
            LegacyDevice { name: "Mouse", location: "HID: /dev/hidraw5", key: "Mouse@0", zones: vec!["Logo"] },
            LegacyDevice { name: "Mouse", location: "HID: /dev/hidraw9", key: "Mouse@1", zones: vec!["Logo"] },
        ];
        let m = migrate_zone_sizes(&legacy, &devs);
        // Node: location changed but the name is unique -> still migrated.
        assert_eq!(m["Node#ABC|Header 1"], 24);
        assert!(!m.contains_key("Node#ABC|Header 2"));
        // Mouse: two with the same name; only the exact location matches.
        assert_eq!(m["Mouse@0|Logo"], 3);
        assert!(!m.contains_key("Mouse@1|Logo"));
    }
}
