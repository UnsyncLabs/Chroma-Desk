//! System monitors: CPU, GPU and RAM readings for the `meter` effect and threshold automations.
//!
//! Sampling is demand-driven. Nothing runs until something *asks* for a metric (a meter being
//! drawn, a threshold automation, the editor's live value); a sampler thread then reads the
//! sensors twice a second, and parks itself (no wake-ups, no GPU handles) a few seconds after the
//! last request. A steady scene without meters costs exactly nothing.
//!
//! Units: usage and VRAM are percentages (0..100), temperatures are degrees Celsius.

pub mod cpu;
pub mod gpu;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Something that can be measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Metric {
    #[serde(rename = "cpu.usage")]
    CpuUsage,
    #[serde(rename = "cpu.temp")]
    CpuTemp,
    #[serde(rename = "gpu.usage")]
    GpuUsage,
    #[serde(rename = "gpu.vram")]
    GpuVram,
    #[serde(rename = "gpu.temp")]
    GpuTemp,
    #[serde(rename = "ram.usage")]
    RamUsage,
}

pub const ALL: [Metric; 6] = [Metric::CpuUsage, Metric::CpuTemp, Metric::GpuUsage, Metric::GpuVram, Metric::GpuTemp, Metric::RamUsage];

impl Metric {
    pub fn id(self) -> &'static str {
        match self {
            Metric::CpuUsage => "cpu.usage",
            Metric::CpuTemp => "cpu.temp",
            Metric::GpuUsage => "gpu.usage",
            Metric::GpuVram => "gpu.vram",
            Metric::GpuTemp => "gpu.temp",
            Metric::RamUsage => "ram.usage",
        }
    }

    pub fn parse(s: &str) -> Option<Metric> {
        ALL.into_iter().find(|m| m.id() == s.trim())
    }

    pub fn is_temp(self) -> bool {
        matches!(self, Metric::CpuTemp | Metric::GpuTemp)
    }

    pub fn unit(self) -> &'static str {
        if self.is_temp() { "°C" } else { "%" }
    }

    /// The `(min, max)` that map to an empty and a full meter.
    pub fn default_range(self) -> (f32, f32) {
        if self.is_temp() { (30.0, 95.0) } else { (0.0, 100.0) }
    }

    fn is_gpu(self) -> bool {
        matches!(self, Metric::GpuUsage | Metric::GpuVram | Metric::GpuTemp)
    }
}

/// The latest sample of a metric. `value` is what the sensor said at `at`; `prev` the sample
/// before it, so readers can glide from one to the other instead of stepping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub value: f32,
    pub prev: f32,
    pub at: Instant,
    /// Bytes, for the memory metrics (`used`/`total`).
    pub used: Option<f64>,
    pub total: Option<f64>,
}

impl Reading {
    /// The value glided from `prev` to `value` over one sampling interval.
    pub fn smooth(&self, now: Instant) -> f32 {
        let k = (now.saturating_duration_since(self.at).as_secs_f32() / INTERVAL.as_secs_f32()).clamp(0.0, 1.0);
        self.prev + (self.value - self.prev) * k
    }
}

/// A raw sample before it becomes a [`Reading`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub value: f32,
    pub used: Option<f64>,
    pub total: Option<f64>,
}

/// How often sensors are read while something is watching.
pub const INTERVAL: Duration = Duration::from_millis(500);
/// A request keeps the sampler awake this long.
pub const TTL: Duration = Duration::from_secs(3);

#[derive(Default)]
struct Inner {
    touched: HashMap<Metric, Instant>,
    values: HashMap<Metric, Reading>,
    gpu: String,
    started: bool,
    parked: bool,
    /// Sensor reads performed so far (tests check that it stops when idle).
    samples: u64,
}

struct Hub {
    inner: Mutex<Inner>,
    cv: Condvar,
}

fn hub() -> &'static Hub {
    static H: OnceLock<Hub> = OnceLock::new();
    H.get_or_init(|| Hub { inner: Mutex::new(Inner { gpu: "auto".into(), parked: true, ..Default::default() }), cv: Condvar::new() })
}

/// Chooses the GPU the `gpu.*` metrics read: `auto` (the dedicated one) or an id from [`gpu::list`].
pub fn set_gpu(choice: &str) {
    let h = hub();
    let mut g = h.inner.lock().unwrap();
    let c = if choice.trim().is_empty() { "auto" } else { choice.trim() };
    if g.gpu != c {
        g.gpu = c.to_string();
        g.values.retain(|m, _| !m.is_gpu());
    }
}

/// Registers interest in `m`: the sampler wakes up if needed and keeps reading it for [`TTL`].
pub fn touch(m: Metric) {
    let h = hub();
    let mut g = h.inner.lock().unwrap();
    g.touched.insert(m, Instant::now());
    if !g.started {
        g.started = true;
        std::thread::Builder::new().name("sysmon".into()).spawn(sampler).ok();
    }
    if g.parked {
        h.cv.notify_one();
    }
}

/// The latest reading of `m` (also keeps it sampled). `None` until the first sample arrives or
/// when the sensor does not exist on this machine.
pub fn reading(m: Metric) -> Option<Reading> {
    touch(m);
    hub().inner.lock().unwrap().values.get(&m).copied()
}

/// The smoothed current value of the metric named `id`, in its own unit.
pub fn live(id: &str) -> Option<f32> {
    let m = Metric::parse(id)?;
    reading(m).map(|r| r.smooth(Instant::now()))
}

/// Whether the sampler is awake (something asked for a metric in the last [`TTL`]).
pub fn awake() -> bool {
    !hub().inner.lock().unwrap().parked
}

/// Number of sensor reads so far.
pub fn samples_taken() -> u64 {
    hub().inner.lock().unwrap().samples
}

/// Test aid: pretend `m` was sampled with `value`.
#[cfg(test)]
pub fn inject(m: Metric, value: f32) {
    let mut g = hub().inner.lock().unwrap();
    g.values.insert(m, Reading { value, prev: value, at: Instant::now(), used: None, total: None });
}

#[derive(Default)]
struct Backend {
    cpu: cpu::CpuState,
    gpu: gpu::GpuState,
}

fn sampler() {
    let h = hub();
    let mut backend = Backend::default();
    loop {
        let (wanted, choice) = {
            let mut g = h.inner.lock().unwrap();
            let now = Instant::now();
            g.touched.retain(|_, at| now.saturating_duration_since(*at) < TTL);
            if g.touched.is_empty() {
                // Nothing watches: let go of the sensors and sleep until somebody asks.
                g.parked = true;
                backend = Backend::default();
                while g.touched.is_empty() {
                    g = h.cv.wait(g).unwrap();
                }
                g.parked = false;
                continue;
            }
            g.parked = false;
            (g.touched.keys().copied().collect::<Vec<_>>(), g.gpu.clone())
        };
        let mut out: Vec<(Metric, Sample)> = Vec::new();
        for m in &wanted {
            let s = match m {
                Metric::CpuUsage => backend.cpu.usage(),
                Metric::CpuTemp => backend.cpu.temp(),
                Metric::RamUsage => cpu::ram(),
                Metric::GpuUsage | Metric::GpuVram | Metric::GpuTemp => backend.gpu.sample(&choice, *m),
            };
            if let Some(s) = s {
                out.push((*m, s));
            }
        }
        let mut g = h.inner.lock().unwrap();
        g.samples += 1;
        let now = Instant::now();
        for (m, s) in out {
            let prev = g.values.get(&m).map_or(s.value, |r| r.smooth(now));
            g.values.insert(m, Reading { value: s.value, prev, at: now, used: s.used, total: s.total });
        }
        let _ = h.cv.wait_timeout(g, INTERVAL).unwrap();
    }
}

// ---------- status for Settings ----------

#[derive(Debug, Clone, Serialize)]
pub struct MetricStatus {
    pub id: &'static str,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// `nvml`, `nvidia-smi`, `sysfs` or `none`.
    pub gpu_backend: String,
    pub gpus: Vec<gpu::GpuInfo>,
    /// The id `auto` currently resolves to.
    pub selected: Option<String>,
    pub choice: String,
    pub cpu_temp_sensor: Option<String>,
    pub metrics: Vec<MetricStatus>,
}

/// What this machine offers. Does its own short-lived probing; does not wake the sampler.
pub fn status() -> Status {
    let choice = hub().inner.lock().unwrap().gpu.clone();
    let found = gpu::detect();
    let selected = gpu::resolve(&found.gpus, &choice).map(|g| g.id.clone());
    let cpu_temp_sensor = cpu::temp_sensor().map(|s| s.label);
    let metrics = ALL
        .iter()
        .map(|m| MetricStatus {
            id: m.id(),
            available: match m {
                Metric::CpuUsage | Metric::RamUsage => true,
                Metric::CpuTemp => cpu_temp_sensor.is_some(),
                Metric::GpuUsage | Metric::GpuVram | Metric::GpuTemp => selected.is_some(),
            },
        })
        .collect();
    Status { gpu_backend: found.backend, gpus: found.gpus, selected, choice, cpu_temp_sensor, metrics }
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveValue {
    pub metric: &'static str,
    pub value: Option<f32>,
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub unit: &'static str,
}

/// Current values for the editor (keeps the sampler awake while the editor polls).
pub fn read_many(ids: &[String]) -> Vec<LiveValue> {
    ids.iter()
        .filter_map(|id| Metric::parse(id))
        .map(|m| {
            let r = reading(m);
            LiveValue { metric: m.id(), value: r.map(|r| r.value), used: r.and_then(|r| r.used), total: r.and_then(|r| r.total), unit: m.unit() }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for m in ALL {
            assert_eq!(Metric::parse(m.id()), Some(m));
            assert_eq!(serde_json::to_string(&m).unwrap(), format!("\"{}\"", m.id()));
        }
        assert_eq!(Metric::parse("nope"), None);
        assert_eq!(Metric::CpuTemp.default_range(), (30.0, 95.0));
    }

    #[test]
    fn smooth_glides_between_samples() {
        let at = Instant::now();
        let r = Reading { value: 100.0, prev: 0.0, at, used: None, total: None };
        assert_eq!(r.smooth(at), 0.0);
        assert!((r.smooth(at + INTERVAL / 2) - 50.0).abs() < 0.01);
        assert_eq!(r.smooth(at + INTERVAL * 3), 100.0);
    }

    /// The sampler runs only while something asks for a metric and parks itself afterwards.
    #[test]
    fn sampler_sleeps_when_nobody_asks() {
        // Nothing asked yet in this process for ram: the thread may not even exist.
        let before = samples_taken();
        std::thread::sleep(Duration::from_millis(1200));
        assert_eq!(samples_taken(), before, "sampled without being asked");
        let r = {
            reading(Metric::RamUsage);
            std::thread::sleep(Duration::from_millis(1300));
            reading(Metric::RamUsage)
        };
        let r = r.expect("ram is always readable");
        assert!(r.value > 0.0 && r.value <= 100.0 && r.total.is_some());
        assert!(awake());
        assert!(samples_taken() >= before + 2);
        std::thread::sleep(TTL + Duration::from_millis(1500));
        assert!(!awake(), "must park after the last request");
        let parked = samples_taken();
        std::thread::sleep(Duration::from_millis(1500));
        assert_eq!(samples_taken(), parked, "must not sample while parked");
    }
}
