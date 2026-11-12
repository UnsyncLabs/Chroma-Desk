//! Shared application state: the SDK connection, persisted config and the animation engine.

use crate::config::{Config, zone_key};
use crate::engine::{DevInfo, Engine};
use crate::error::{ApiError, Res};
use crate::identity;
use crate::sdk::{self, DeviceDto};
use openrgb2::{ControllerGroup, OpenRgbClient};
use std::sync::Arc;
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
}

impl AppState {
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

    /// Re-applies saved zone sizes (OpenRGB forgets them on restart). True if anything changed.
    pub async fn apply_saved_zone_sizes(&self) -> Res<bool> {
        let sizes = self.config.lock().unwrap().zone_sizes.clone();
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let mut changed = false;
        for c in conn.group.iter() {
            for z in c.get_all_zones() {
                let key = zone_key(c.name(), c.location(), z.name());
                if let Some(&n) = sizes.get(&key)
                    && n != z.num_leds()
                    && n >= z.leds_min()
                    && n <= z.leds_max()
                {
                    z.resize(n).await.map_err(ApiError::sdk)?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }

    /// Device list with stable keys; also refreshes the engine's view of the devices.
    pub async fn devices(&self) -> Res<Vec<DeviceDto>> {
        let guard = self.conn.lock().await;
        let conn = guard.as_ref().ok_or_else(ApiError::no_connection)?;
        let mut dtos: Vec<DeviceDto> = conn.group.iter().map(sdk::device_dto).collect();
        let keys = identity::device_keys(dtos.iter().map(|d| (d.name.as_str(), d.serial.as_str())));
        for (d, k) in dtos.iter_mut().zip(keys) {
            d.key = k;
        }
        *self.engine.devs.lock().unwrap() = dtos.iter().map(DevInfo::from_dto).collect();
        Ok(dtos)
    }
}
