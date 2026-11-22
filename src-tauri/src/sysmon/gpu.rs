//! GPU readings: NVIDIA through NVML (loaded at run time from the driver; `nvidia-smi` as a
//! fallback) and AMD through sysfs. Intel has no usage counter we can read without privileges,
//! so it is reported as unavailable.

use super::{Metric, Sample};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GpuInfo {
    /// `nvidia:<index>` or `amd:<cardN>`.
    pub id: String,
    pub name: String,
    pub vendor: String,
    /// Discrete cards win `auto`.
    pub dedicated: bool,
    pub vram_total: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Detected {
    pub backend: String,
    pub gpus: Vec<GpuInfo>,
}

/// One reading of everything a GPU offers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GpuSample {
    pub usage: Option<f32>,
    pub temp: Option<f32>,
    pub used: Option<f64>,
    pub total: Option<f64>,
}

impl GpuSample {
    pub fn pick(&self, m: Metric) -> Option<Sample> {
        match m {
            Metric::GpuUsage => self.usage.map(|value| Sample { value, used: None, total: None }),
            Metric::GpuTemp => self.temp.map(|value| Sample { value, used: None, total: None }),
            Metric::GpuVram => {
                let (u, t) = (self.used?, self.total?);
                (t > 0.0).then(|| Sample { value: (u / t * 100.0).clamp(0.0, 100.0) as f32, used: Some(u), total: Some(t) })
            }
            _ => None,
        }
    }
}

/// `auto` = the first NVIDIA card, else the AMD card with the most memory; otherwise by id.
pub fn resolve<'a>(gpus: &'a [GpuInfo], choice: &str) -> Option<&'a GpuInfo> {
    if choice != "auto"
        && !choice.is_empty()
        && let Some(g) = gpus.iter().find(|g| g.id == choice)
    {
        return Some(g);
    }
    gpus.iter()
        .filter(|g| g.dedicated)
        .min_by(|a, b| (a.vendor != "nvidia").cmp(&(b.vendor != "nvidia")).then(b.vram_total.unwrap_or(0.0).total_cmp(&a.vram_total.unwrap_or(0.0))))
        .or_else(|| gpus.first())
}

// ---------- AMD (sysfs) ----------

fn read_u64(p: &Path) -> Option<u64> {
    std::fs::read_to_string(p).ok()?.trim().parse().ok()
}

/// AMD cards under a DRM root (`/sys/class/drm`) that report `gpu_busy_percent`.
pub fn amd_cards(root: &Path) -> Vec<(GpuInfo, PathBuf)> {
    let Ok(rd) = std::fs::read_dir(root) else { return vec![] };
    let mut out = vec![];
    for e in rd.filter_map(Result::ok) {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let dev = e.path().join("device");
        if std::fs::read_to_string(dev.join("vendor")).ok().map(|v| v.trim().to_lowercase()).as_deref() != Some("0x1002") || !dev.join("gpu_busy_percent").exists() {
            continue;
        }
        let total = read_u64(&dev.join("mem_info_vram_total")).map(|v| v as f64);
        // An iGPU carves its memory out of RAM: small VRAM and no discrete flag.
        let dedicated = total.is_some_and(|t| t >= 3.0 * 1024.0 * 1024.0 * 1024.0);
        out.push((GpuInfo { id: format!("amd:{name}"), name: format!("AMD GPU ({name})"), vendor: "amd".into(), dedicated, vram_total: total }, dev));
    }
    out.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    out
}

fn amd_sample(dev: &Path) -> GpuSample {
    let temp = std::fs::read_dir(dev.join("hwmon")).ok().and_then(|rd| rd.filter_map(Result::ok).find_map(|e| read_u64(&e.path().join("temp1_input")))).map(|m| m as f32 / 1000.0);
    GpuSample {
        usage: read_u64(&dev.join("gpu_busy_percent")).map(|v| v as f32),
        temp,
        used: read_u64(&dev.join("mem_info_vram_used")).map(|v| v as f64),
        total: read_u64(&dev.join("mem_info_vram_total")).map(|v| v as f64),
    }
}

// ---------- NVIDIA ----------

type SmiRows = Vec<(u32, String, GpuSample)>;

/// Rows of `nvidia-smi --query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu --format=csv,noheader,nounits`.
pub fn parse_smi(text: &str) -> SmiRows {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').map(str::trim).collect();
            if f.len() < 6 {
                return None;
            }
            let mib = |s: &str| s.parse::<f64>().ok().map(|v| v * 1024.0 * 1024.0);
            Some((f[0].parse().ok()?, f[1].to_string(), GpuSample { usage: f[2].parse().ok(), used: mib(f[3]), total: mib(f[4]), temp: f[5].parse().ok() }))
        })
        .collect()
}

fn run_smi() -> Option<SmiRows> {
    let out = crate::fsutil::quiet_command("nvidia-smi").args(["--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu", "--format=csv,noheader,nounits"]).output().ok()?;
    out.status.success().then(|| parse_smi(&String::from_utf8_lossy(&out.stdout)))
}

fn nvml_infos(nvml: &nvml_wrapper::Nvml) -> Vec<GpuInfo> {
    let n = nvml.device_count().unwrap_or(0);
    (0..n)
        .filter_map(|i| {
            let d = nvml.device_by_index(i).ok()?;
            Some(GpuInfo { id: format!("nvidia:{i}"), name: d.name().unwrap_or_else(|_| format!("NVIDIA GPU {i}")), vendor: "nvidia".into(), dedicated: true, vram_total: d.memory_info().ok().map(|m| m.total as f64) })
        })
        .collect()
}

fn nvml_sample(nvml: &nvml_wrapper::Nvml, index: u32) -> Option<GpuSample> {
    let d = nvml.device_by_index(index).ok()?;
    let mem = d.memory_info().ok();
    Some(GpuSample {
        usage: d.utilization_rates().ok().map(|u| u.gpu as f32),
        temp: d.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu).ok().map(|t| t as f32),
        used: mem.as_ref().map(|m| m.used as f64),
        total: mem.as_ref().map(|m| m.total as f64),
    })
}

fn smi_infos(list: &[(u32, String, GpuSample)]) -> Vec<GpuInfo> {
    list.iter().map(|(i, n, s)| GpuInfo { id: format!("nvidia:{i}"), name: n.clone(), vendor: "nvidia".into(), dedicated: true, vram_total: s.total }).collect()
}

/// Every GPU we can read and how (a short-lived probe: Settings uses it, the sampler does not).
pub fn detect() -> Detected {
    let mut gpus = vec![];
    let mut backend = "none".to_string();
    if let Ok(nvml) = nvml_wrapper::Nvml::init() {
        gpus = nvml_infos(&nvml);
        if !gpus.is_empty() {
            backend = "nvml".into();
        }
    }
    if gpus.is_empty()
        && let Some(list) = run_smi()
    {
        gpus = smi_infos(&list);
        if !gpus.is_empty() {
            backend = "nvidia-smi".into();
        }
    }
    let amd = amd_cards(Path::new("/sys/class/drm"));
    if !amd.is_empty() && backend == "none" {
        backend = "sysfs".into();
    }
    gpus.extend(amd.into_iter().map(|(g, _)| g));
    Detected { backend, gpus }
}

/// Handles held while the sampler is awake.
#[derive(Default)]
pub struct GpuState {
    nvml: Option<Option<nvml_wrapper::Nvml>>,
    gpus: Option<Vec<GpuInfo>>,
    smi: Option<(Instant, SmiRows)>,
    /// The sample of the current tick (one device read serves all three metrics).
    tick: Option<(Instant, GpuSample)>,
}

impl GpuState {
    fn nvml(&mut self) -> Option<&nvml_wrapper::Nvml> {
        self.nvml.get_or_insert_with(|| nvml_wrapper::Nvml::init().ok()).as_ref()
    }

    fn infos(&mut self) -> Vec<GpuInfo> {
        if self.gpus.is_none() {
            let mut gpus = self.nvml().map(nvml_infos).unwrap_or_default();
            if gpus.is_empty()
                && let Some(list) = run_smi()
            {
                gpus = smi_infos(&list);
            }
            gpus.extend(amd_cards(Path::new("/sys/class/drm")).into_iter().map(|(g, _)| g));
            self.gpus = Some(gpus);
        }
        self.gpus.clone().unwrap_or_default()
    }

    fn read(&mut self, choice: &str) -> Option<GpuSample> {
        let now = Instant::now();
        if let Some((at, s)) = self.tick
            && now.saturating_duration_since(at) < Duration::from_millis(200)
        {
            return Some(s);
        }
        let infos = self.infos();
        let g = resolve(&infos, choice)?.clone();
        let s = if let Some(i) = g.id.strip_prefix("nvidia:").and_then(|i| i.parse::<u32>().ok()) {
            match self.nvml().and_then(|n| nvml_sample(n, i)) {
                Some(s) => Some(s),
                None => {
                    // nvidia-smi is slow (a process per call): at most one call every 2 s.
                    if self.smi.as_ref().is_none_or(|(at, _)| now.saturating_duration_since(*at) >= Duration::from_secs(2)) {
                        self.smi = Some((now, run_smi().unwrap_or_default()));
                    }
                    self.smi.as_ref().and_then(|(_, l)| l.iter().find(|(n, _, _)| *n == i).map(|(_, _, s)| *s))
                }
            }
        } else {
            g.id.strip_prefix("amd:").and_then(|c| amd_cards(Path::new("/sys/class/drm")).into_iter().find(|(i, _)| i.id.ends_with(c)).map(|(_, dev)| amd_sample(&dev)))
        }?;
        self.tick = Some((now, s));
        Some(s)
    }

    pub fn sample(&mut self, choice: &str, m: Metric) -> Option<Sample> {
        self.read(choice)?.pick(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smi_output() {
        let v = parse_smi("0, NVIDIA GeForce RTX 4070 SUPER, 17, 2048, 12282, 41\n1, X, [N/A], 1, 2, 3\nbad line\n");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].1, "NVIDIA GeForce RTX 4070 SUPER");
        let s = v[0].2;
        assert_eq!(s.usage, Some(17.0));
        assert_eq!(s.temp, Some(41.0));
        assert_eq!(s.pick(Metric::GpuVram).unwrap().used, Some(2048.0 * 1048576.0));
        let pct = s.pick(Metric::GpuVram).unwrap().value;
        assert!((pct - 2048.0 / 12282.0 * 100.0).abs() < 0.01);
        assert_eq!(v[1].2.usage, None);
    }

    fn gi(id: &str, vendor: &str, dedicated: bool, vram: f64) -> GpuInfo {
        GpuInfo { id: id.into(), name: id.into(), vendor: vendor.into(), dedicated, vram_total: Some(vram) }
    }

    #[test]
    fn auto_prefers_the_dedicated_card() {
        let g = [gi("amd:card2", "amd", false, 5e8), gi("nvidia:0", "nvidia", true, 1.2e10), gi("amd:card1", "amd", true, 8e9)];
        assert_eq!(resolve(&g, "auto").unwrap().id, "nvidia:0");
        assert_eq!(resolve(&g, "amd:card2").unwrap().id, "amd:card2");
        assert_eq!(resolve(&g, "gone").unwrap().id, "nvidia:0");
        assert_eq!(resolve(&g[..1], "auto").unwrap().id, "amd:card2");
        assert!(resolve(&[], "auto").is_none());
    }

    #[test]
    fn amd_sysfs_fixture() {
        let dir = crate::fsutil::test_dir("drm");
        let w = |p: &str, v: &str| {
            let f = dir.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, v).unwrap();
        };
        w("card2/device/vendor", "0x1002\n");
        w("card2/device/gpu_busy_percent", "12\n");
        w("card2/device/mem_info_vram_used", "100\n");
        w("card2/device/mem_info_vram_total", "400\n");
        w("card2/device/hwmon/hwmon2/temp1_input", "55000\n");
        w("card2-DP-1/device/vendor", "0x1002\n");
        w("card0/device/vendor", "0x10de\n");
        let cards = amd_cards(&dir);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].0.id, "amd:card2");
        assert!(!cards[0].0.dedicated);
        let s = amd_sample(&cards[0].1);
        assert_eq!((s.usage, s.temp), (Some(12.0), Some(55.0)));
        assert_eq!(s.pick(Metric::GpuVram).unwrap().value, 25.0);
    }
}
