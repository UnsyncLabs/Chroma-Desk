//! Shared application state: the SDK connection, persisted config and the animation engine.

use crate::config::{Config, LegacyDevice, migrate_zone_sizes, zone_key};
use crate::engine::{DevInfo, Engine};
use crate::error::{ApiError, Res};
use crate::identity;
use crate::sdk::{self, DeviceDto};
use openrgb2::{ControllerGroup, OpenRgbClient};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
use tokio::sync::Mutex;

pub struct Conn {
    pub client: OpenRgbClient,
    pub group: ControllerGroup,
}

/// Cheap to clone: every field is shared.
#[derive(Clone, Default)]
pub struct AppState {
    pub conn: Arc<Mutex<Option<Conn>>>,
    pub config: Arc<std::sync::Mutex<Config>>,
    pub engine: Arc<Engine>,
    /// Set once the startup sequence has finished.
    ready: Arc<(AtomicBool, Notify)>,
    pub tray: Arc<crate::tray::TrayState>,
}

impl AppState {
    pub fn mark_ready(&self) {
        self.ready.0.store(true, Ordering::SeqCst);
        self.ready.1.notify_waiters();
    }

    pub async fn wait_ready(&self) {
        loop {
            let waiter = self.ready.1.notified();
            if self.ready.0.load(Ordering::SeqCst) {
                return;
            }
            waiter.await;
        }
    }

    /// Remembers which profile is applied (shown in the UI).
    pub fn set_current_profile(&self, name: Option<String>) -> Res<()> {
        let mut cfg = self.config.lock().unwrap();
        cfg.settings.current_profile = name;
        cfg.save()
    }

    /// A profile was deleted: drop references to it.
    pub fn forget_profile(&self, name: &str) -> Res<()> {
        let mut cfg = self.config.lock().unwrap();
        if cfg.settings.current_profile.as_deref() == Some(name) {
            cfg.settings.current_profile = None;
        }
        if cfg.settings.default_profile.as_deref() == Some(name) {
            cfg.settings.default_profile = None;
        }
        cfg.save()
    }

    pub fn rename_profile_refs(&self, old: &str, new: &str) -> Res<()> {
        let mut guard = self.config.lock().unwrap();
        let cfg = &mut *guard;
        for slot in [&mut cfg.settings.current_profile, &mut cfg.settings.default_profile] {
            if slot.as_deref() == Some(old) {
                *slot = Some(new.to_string());
            }
        }
        cfg.save()
    }

    pub fn new(config: Config) -> Self {
        Self { config: Arc::new(std::sync::Mutex::new(config)), ..Default::default() }
    }

    /// Connects if needed and re-reads every device. `spawn` allows starting the server.
    pub async fn refresh(&self, spawn: bool) -> Res<()> {
        let mut guard = self.conn.lock().await;
        if let Some(conn) = guard.as_mut() {
            match conn.client.get_all_controllers().await {
                Ok(g) => {
                    conn.group = g;
                    return Ok(());
                }
                // Connection dropped: fall through and reconnect.
                Err(_) => *guard = None,
            }
        }
        let client = sdk::connect(spawn).await?;
        let group = client.get_all_controllers().await.map_err(ApiError::sdk)?;
        *guard = Some(Conn { client, group });
        Ok(())
    }

    /// Identity keys of the connected devices, in server order.
    pub fn keys_of(conn: &Conn) -> Vec<String> {
        identity::device_keys(conn.group.iter().map(|c| (c.name(), c.serial())))
    }

    /// Re-applies remembered zone sizes (OpenRGB forgets them on restart), migrating the
    /// phase 1-3 config format first. True if any zone changed size.
    pub async fn apply_saved_zone_sizes(&self) -> Res<bool> {
        self.migrate_legacy_sizes().await?;
        let sizes = self.config.lock().unwrap().zone_sizes_by_key.clone();
        self.apply_zone_sizes(&sizes).await
    }

    async fn migrate_legacy_sizes(&self) -> Res<()> {
        if self.config.lock().unwrap().zone_sizes.is_empty() {
            return Ok(());
        }
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let keys = Self::keys_of(conn);
        let zone_names: Vec<Vec<String>> = conn.group.iter().map(|c| c.get_all_zones().map(|z| z.name().to_string()).collect()).collect();
        let locations: Vec<String> = conn.group.iter().map(|c| c.location().to_string()).collect();
        let names: Vec<String> = conn.group.iter().map(|c| c.name().to_string()).collect();
        let devs: Vec<LegacyDevice> = (0..keys.len())
            .map(|i| LegacyDevice { name: &names[i], location: &locations[i], key: &keys[i], zones: zone_names[i].iter().map(String::as_str).collect() })
            .collect();
        let mut cfg = self.config.lock().unwrap();
        let migrated = migrate_zone_sizes(&cfg.zone_sizes, &devs);
        cfg.zone_sizes_by_key.extend(migrated);
        cfg.zone_sizes.clear();
        cfg.save()
    }

    /// Resizes zones to `sizes` (by `<device key>|<zone>`) where valid; true if any changed.
    pub async fn apply_zone_sizes(&self, sizes: &std::collections::BTreeMap<String, usize>) -> Res<bool> {
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let keys = Self::keys_of(conn);
        let mut changed = false;
        for (c, key) in conn.group.iter().zip(&keys) {
            for z in c.get_all_zones() {
                if let Some(&n) = sizes.get(&zone_key(key, z.name()))
                    && n != z.num_leds()
                    && n >= z.leds_min()
                    && n <= z.leds_max()
                {
                    // Best effort: one stubborn zone must not abort the whole startup.
                    match z.resize(n).await {
                        Ok(()) => changed = true,
                        Err(e) => eprintln!("could not restore size {n} of {key}|{}: {e}", z.name()),
                    }
                }
            }
        }
        Ok(changed)
    }

    /// Current size of every resizable zone, by `<device key>|<zone>`.
    pub async fn resizable_zone_sizes(&self) -> Res<std::collections::BTreeMap<String, usize>> {
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let keys = Self::keys_of(conn);
        let mut out = std::collections::BTreeMap::new();
        for (c, key) in conn.group.iter().zip(&keys) {
            for z in c.get_all_zones().filter(|z| z.leds_min() != z.leds_max()) {
                out.insert(zone_key(key, z.name()), z.num_leds());
            }
        }
        Ok(out)
    }

    /// Device list with stable keys; also refreshes the engine's view of the devices.
    /// Phantom duplicates are left out (of the list and of the engine) unless the setting asks for them.
    pub async fn devices(&self) -> Res<Vec<DeviceDto>> {
        let show_phantoms = self.config.lock().unwrap().settings.show_phantom_devices;
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let mut dtos: Vec<DeviceDto> = conn.group.iter().map(sdk::device_dto).collect();
        // Keys are computed over *all* devices so hiding one never renames the others.
        let keys = identity::device_keys(dtos.iter().map(|d| (d.name.as_str(), d.serial.as_str())));
        let phantom = sdk::phantom_flags(dtos.iter().map(|d| (d.name.as_str(), d.location.as_str(), d.serial.as_str())));
        for ((d, k), p) in dtos.iter_mut().zip(keys).zip(phantom) {
            d.key = k;
            d.phantom = p;
        }
        if !show_phantoms {
            dtos.retain(|d| !d.phantom);
        }
        *self.engine.devs.lock().unwrap() = dtos.iter().map(DevInfo::from_dto).collect();
        Ok(dtos)
    }

    /// Fingerprint of the controller list: changes when a device appears, disappears or the
    /// server reorders them.
    pub fn signature(group: &ControllerGroup) -> Vec<(usize, String, String, String, usize)> {
        group.iter().map(|c| (c.id(), c.name().to_string(), c.serial().to_string(), c.location().to_string(), c.colors().len())).collect()
    }

    /// Re-reads the controller list from the server; true when it differs from the cached one
    /// (hot-plug). On change the cached group is replaced, so indices are current again; the
    /// caller must then refresh the engine's device list.
    pub async fn poll_device_list(&self) -> Res<bool> {
        let mut guard = self.conn.lock().await;
        let conn = guard.as_mut().ok_or_else(ApiError::no_connection)?;
        let fresh = conn.client.get_all_controllers().await.map_err(ApiError::sdk)?;
        if Self::signature(&fresh) == Self::signature(&conn.group) {
            return Ok(false);
        }
        conn.group = fresh;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::DevInfo;
    use crate::identity;
    use crate::sdk::DeviceDto;

    fn dto(id: usize, name: &str, serial: &str) -> DeviceDto {
        DeviceDto {
            id,
            key: String::new(),
            name: name.into(),
            vendor: String::new(),
            description: String::new(),
            version: String::new(),
            serial: serial.into(),
            location: String::new(),
            device_type: "Keyboard".into(),
            active_mode: 0,
            modes: vec![],
            zones: vec![],
            led_names: vec![],
            colors: vec![],
            phantom: false,
        }
    }

    /// key -> controller index as the engine sees it.
    fn map(list: &[DeviceDto]) -> std::collections::HashMap<String, usize> {
        let keys = identity::device_keys(list.iter().map(|d| (d.name.as_str(), d.serial.as_str())));
        list.iter().zip(keys).map(|(d, k)| (k, DevInfo::from_dto(d).index)).collect()
    }

    #[test]
    fn keys_follow_their_device_when_the_server_reorders() {
        let before = [dto(0, "ENE DRAM", ""), dto(1, "Razer Huntsman V3 Pro TKL", "S1"), dto(2, "G502", "M1")];
        // A keyboard is plugged in: the server now lists it before the Huntsman.
        let after = [dto(0, "ENE DRAM", ""), dto(1, "Razer Ornata Chroma", "SI2050"), dto(2, "Razer Huntsman V3 Pro TKL", "S1"), dto(3, "G502", "M1")];
        let (a, b) = (map(&before), map(&after));
        assert_eq!(a["Razer Huntsman V3 Pro TKL#S1"], 1);
        assert_eq!(b["Razer Huntsman V3 Pro TKL#S1"], 2, "same key, new index");
        assert_eq!(b["G502#M1"], 3);
        assert_eq!(b["Razer Ornata Chroma#SI2050"], 1);
        // Serial-less devices keep their ordinal key.
        assert_eq!(b["ENE DRAM@0"], 0);
    }
}
