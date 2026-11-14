//! CPU usage (`/proc/stat`), CPU temperature (hwmon) and RAM (`/proc/meminfo`) on Linux; the
//! `sysinfo` crate on Windows and macOS (where there is no CPU temperature sensor we can read
//! without extra privileges, so `cpu.temp` is reported as unavailable).
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use super::Sample;
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::time::Duration;

/// Cumulative `(idle, total)` jiffies from the first line of `/proc/stat`.
pub fn parse_stat(text: &str) -> Option<(u64, u64)> {
    let line = text.lines().next()?;
    let mut it = line.split_whitespace();
    if it.next()? != "cpu" {
        return None;
    }
    let v: Vec<u64> = it.take(8).filter_map(|x| x.parse().ok()).collect();
    if v.len() < 5 {
        return None;
    }
    // idle + iowait count as idle.
    Some((v[3] + v[4], v.iter().sum()))
}

/// Usage percentage between two `(idle, total)` readings.
pub fn usage_between(a: (u64, u64), b: (u64, u64)) -> Option<f32> {
    let total = b.1.checked_sub(a.1)?;
    let idle = b.0.checked_sub(a.0)?;
    (total > 0).then(|| ((total - idle.min(total)) as f32 / total as f32 * 100.0).clamp(0.0, 100.0))
}

#[cfg(target_os = "linux")]
fn read_stat() -> Option<(u64, u64)> {
    parse_stat(&std::fs::read_to_string("/proc/stat").ok()?)
}

#[derive(Default)]
pub struct CpuState {
    prev: Option<(u64, u64)>,
    sensor: Option<Option<TempSensor>>,
    #[cfg(not(target_os = "linux"))]
    sys: Option<sysinfo::System>,
}

#[cfg(not(target_os = "linux"))]
impl CpuState {
    pub fn usage(&mut self) -> Option<Sample> {
        let first = self.sys.is_none();
        if first {
            self.sys = Some(sysinfo::System::new().ok()?);
        }
        let sys = self.sys.as_mut()?;
        sys.refresh_cpu_usage();
        if first {
            // The first reading has no interval to measure over.
            std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
            sys.refresh_cpu_usage();
        }
        Some(Sample { value: sys.global_cpu_usage().clamp(0.0, 100.0), used: None, total: None })
    }

    pub fn temp(&mut self) -> Option<Sample> {
        None
    }
}

#[cfg(target_os = "linux")]
impl CpuState {
    pub fn usage(&mut self) -> Option<Sample> {
        let prev = match self.prev {
            Some(p) => p,
            None => {
                let first = read_stat()?;
                std::thread::sleep(Duration::from_millis(120));
                first
            }
        };
        let now = read_stat()?;
        self.prev = Some(now);
        usage_between(prev, now).map(|value| Sample { value, used: None, total: None })
    }

    pub fn temp(&mut self) -> Option<Sample> {
        let s = self.sensor.get_or_insert_with(temp_sensor).as_ref()?;
        let milli: f32 = std::fs::read_to_string(&s.input).ok()?.trim().parse().ok()?;
        Some(Sample { value: milli / 1000.0, used: None, total: None })
    }
}

/// `MemTotal` and `MemAvailable` (bytes) from `/proc/meminfo`.
pub fn parse_meminfo(text: &str) -> Option<(f64, f64)> {
    let field = |name: &str| -> Option<f64> {
        let l = text.lines().find(|l| l.starts_with(name))?;
        Some(l.split_whitespace().nth(1)?.parse::<f64>().ok()? * 1024.0)
    };
    Some((field("MemTotal:")?, field("MemAvailable:")?))
}

#[cfg(not(target_os = "linux"))]
pub fn ram() -> Option<Sample> {
    let mut sys = sysinfo::System::new().ok()?;
    sys.refresh_memory();
    let (total, used) = (sys.total_memory() as f64, sys.used_memory() as f64);
    (total > 0.0).then(|| Sample { value: (used / total * 100.0) as f32, used: Some(used), total: Some(total) })
}

#[cfg(target_os = "linux")]
pub fn ram() -> Option<Sample> {
    let (total, avail) = parse_meminfo(&std::fs::read_to_string("/proc/meminfo").ok()?)?;
    let used = (total - avail).max(0.0);
    Some(Sample { value: (used / total * 100.0) as f32, used: Some(used), total: Some(total) })
}

#[derive(Debug, Clone, PartialEq)]
pub struct TempSensor {
    pub input: PathBuf,
    /// `k10temp Tctl`, `coretemp Package id 0`...
    pub label: String,
}

/// The CPU temperature input inside a hwmon root (`/sys/class/hwmon`): `k10temp` (Tctl),
/// `zenpower`, `coretemp` (Package) in that order of preference.
pub fn find_temp_sensor(root: &Path) -> Option<TempSensor> {
    let mut hwmons: Vec<(String, PathBuf)> = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|e| Some((std::fs::read_to_string(e.path().join("name")).ok()?.trim().to_string(), e.path())))
        .collect();
    hwmons.sort_by(|a, b| a.1.cmp(&b.1));
    for want in ["k10temp", "zenpower", "coretemp"] {
        let Some((name, dir)) = hwmons.iter().find(|(n, _)| n == want) else { continue };
        // Prefer the labelled package/control temperature, else temp1.
        let mut best: Option<(u8, usize, String)> = None;
        for i in 1..=16 {
            if !dir.join(format!("temp{i}_input")).exists() {
                continue;
            }
            let label = std::fs::read_to_string(dir.join(format!("temp{i}_label"))).unwrap_or_default().trim().to_string();
            let rank = match label.as_str() {
                "Tctl" | "Tdie" => 0,
                l if l.starts_with("Package") => 0,
                _ if i == 1 => 1,
                _ => 2,
            };
            if best.as_ref().is_none_or(|b| rank < b.0) {
                best = Some((rank, i, label));
            }
        }
        if let Some((_, i, label)) = best {
            let label = if label.is_empty() { name.clone() } else { format!("{name} {label}") };
            return Some(TempSensor { input: dir.join(format!("temp{i}_input")), label });
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub fn temp_sensor() -> Option<TempSensor> {
    find_temp_sensor(Path::new("/sys/class/hwmon"))
}

#[cfg(not(target_os = "linux"))]
pub fn temp_sensor() -> Option<TempSensor> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_and_usage() {
        let a = parse_stat("cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 1 2 3 4").unwrap();
        assert_eq!(a, (800, 1000));
        let b = parse_stat("cpu  150 0 150 900 0 0 0 0 0 0").unwrap();
        // 100 idle of 200 total -> 50 %.
        assert_eq!(usage_between(a, b), Some(50.0));
        assert_eq!(usage_between(a, a), None);
        assert_eq!(parse_stat("intr 1 2"), None);
        assert_eq!(parse_stat(""), None);
    }

    #[test]
    fn meminfo() {
        let (t, a) = parse_meminfo("MemTotal:       1000 kB\nMemFree: 1 kB\nMemAvailable:    250 kB\n").unwrap();
        assert_eq!((t, a), (1024000.0, 256000.0));
        assert_eq!(parse_meminfo("MemTotal: 1 kB"), None);
    }

    #[test]
    fn finds_k10temp_tctl_in_a_fake_hwmon() {
        let dir = crate::fsutil::test_dir("hwmon");
        let w = |p: &str, v: &str| {
            let f = dir.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, v).unwrap();
        };
        w("hwmon0/name", "nvme\n");
        w("hwmon0/temp1_input", "30000\n");
        w("hwmon6/name", "k10temp\n");
        w("hwmon6/temp1_input", "49750\n");
        w("hwmon6/temp1_label", "Tctl\n");
        w("hwmon6/temp3_input", "40000\n");
        w("hwmon6/temp3_label", "Tccd1\n");
        let s = find_temp_sensor(&dir).unwrap();
        assert_eq!(s.input, dir.join("hwmon6/temp1_input"));
        assert_eq!(s.label, "k10temp Tctl");
        assert_eq!(find_temp_sensor(&dir.join("hwmon0")), None);
    }
}
