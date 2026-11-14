//! OpenRGB server integration: is the OpenRGB window open, and an optional systemd user service
//! that keeps `openrgb --server` alive.

use serde::Serialize;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub const UNIT_NAME: &str = "chroma-desk-openrgb.service";

/// systemd user unit text for the server.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn unit_text(openrgb_path: &str) -> String {
    format!(
        "[Unit]\nDescription=OpenRGB SDK server (chroma-desk)\nAfter=graphical-session.target\nPartOf=graphical-session.target\n\n\
         [Service]\nExecStart={openrgb_path} --server --startminimized\nRestart=on-failure\nRestartSec=3\n\n\
         [Install]\nWantedBy=graphical-session.target\n"
    )
}

/// Whether a `/proc/<pid>/cmdline` (NUL-separated) is an OpenRGB instance whose window is open
/// (started without `--startminimized`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn is_visible_openrgb(cmdline: &str) -> bool {
    let mut parts = cmdline.split('\0').filter(|s| !s.is_empty());
    let Some(exe) = parts.next() else { return false };
    let is_openrgb = exe.rsplit('/').next().is_some_and(|n| n == "openrgb");
    is_openrgb && !parts.any(|a| a == "--startminimized" || a == "--noautoconnect")
}

/// True if an OpenRGB instance with a visible window is running.
#[cfg(target_os = "linux")]
pub fn openrgb_gui_open() -> bool {
    let Ok(rd) = std::fs::read_dir("/proc") else { return false };
    rd.filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().chars().all(|c| c.is_ascii_digit()))
        .filter_map(|e| std::fs::read(e.path().join("cmdline")).ok())
        .any(|c| is_visible_openrgb(&String::from_utf8_lossy(&c)))
}

/// Windows/macOS: an OpenRGB process started without `--startminimized` (window open).
#[cfg(not(target_os = "linux"))]
pub fn openrgb_gui_open() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let Ok(mut sys) = System::new() else { return false };
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always));
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        name.trim_end_matches(".exe") == "openrgb" && !p.cmd().iter().any(|a| a == "--startminimized" || a == "--noautoconnect")
    })
}

#[cfg(target_os = "linux")]
fn user_unit_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("systemd/user").join(UNIT_NAME)
}

#[cfg(target_os = "linux")]
fn systemctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("systemctl").arg("--user").args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct ServerStatus {
    /// The SDK port answers.
    pub running: bool,
    /// An OpenRGB window is open (it and chroma-desk would fight over the devices).
    pub gui_open: bool,
    pub service_installed: bool,
    pub service_enabled: bool,
    pub protocol: Option<u32>,
}

/// Where this platform's OpenRGB usually lives, in order of preference (the first existing
/// file wins; `PATH` is searched before these).
pub fn openrgb_candidates() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = vec![];
    if cfg!(windows) {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                let base = PathBuf::from(base);
                v.push(base.join("OpenRGB").join("OpenRGB.exe"));
                v.push(base.join("Programs").join("OpenRGB").join("OpenRGB.exe"));
            }
        }
    } else if cfg!(target_os = "macos") {
        v.push("/Applications/OpenRGB.app/Contents/MacOS/OpenRGB".into());
        v.push(crate::fsutil::home_dir().join("Applications/OpenRGB.app/Contents/MacOS/OpenRGB"));
        v.push("/opt/homebrew/bin/openrgb".into());
        v.push("/usr/local/bin/openrgb".into());
    } else {
        v.push("/usr/bin/openrgb".into());
        v.push("/usr/local/bin/openrgb".into());
        v.push(crate::fsutil::home_dir().join(".local/bin/openrgb"));
    }
    v
}

/// The OpenRGB executable: on `PATH`, else one of [`openrgb_candidates`].
pub fn find_openrgb() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) { &["OpenRGB.exe", "openrgb.exe"] } else { &["openrgb"] };
    std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).flat_map(|d| names.iter().map(move |n| d.join(n))).find(|c| c.is_file()))
        .or_else(|| openrgb_candidates().into_iter().find(|c| c.is_file()))
}

#[cfg(target_os = "linux")]
pub fn status(protocol: Option<u32>) -> ServerStatus {
    let installed = user_unit_path().exists() || std::path::Path::new("/usr/lib/systemd/user").join(UNIT_NAME).exists();
    ServerStatus {
        running: crate::sdk::server_running(),
        gui_open: openrgb_gui_open(),
        service_installed: installed,
        service_enabled: systemctl(&["is-enabled", UNIT_NAME]).is_ok_and(|s| s == "enabled"),
        protocol,
    }
}

/// No systemd service outside Linux: chroma-desk starts the server itself when it is needed.
#[cfg(not(target_os = "linux"))]
pub fn status(protocol: Option<u32>) -> ServerStatus {
    ServerStatus { running: crate::sdk::server_running(), gui_open: openrgb_gui_open(), service_installed: false, service_enabled: false, protocol }
}

/// Installs (if not provided by the package) and enables, or disables, the server service.
#[cfg(target_os = "linux")]
pub fn set_service(enable: bool) -> Result<(), String> {
    if enable {
        if !std::path::Path::new("/usr/lib/systemd/user").join(UNIT_NAME).exists() {
            let exe = which_openrgb();
            crate::fsutil::write_atomic(&user_unit_path(), unit_text(&exe).as_bytes()).map_err(|e| e.to_string())?;
            systemctl(&["daemon-reload"])?;
        }
        systemctl(&["enable", "--now", UNIT_NAME]).map(|_| ())
    } else {
        systemctl(&["disable", "--now", UNIT_NAME]).map(|_| ())
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_service(_enable: bool) -> Result<(), String> {
    Err("the OpenRGB server service is only available on Linux (systemd)".into())
}

#[cfg(target_os = "linux")]
fn which_openrgb() -> String {
    find_openrgb().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "/usr/bin/openrgb".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_text_runs_the_server_and_restarts() {
        let u = unit_text("/usr/bin/openrgb");
        assert!(u.contains("ExecStart=/usr/bin/openrgb --server --startminimized"));
        assert!(u.contains("Restart=on-failure"));
        assert!(u.contains("WantedBy=graphical-session.target"));
    }

    #[test]
    fn detects_visible_openrgb_window() {
        assert!(is_visible_openrgb("/usr/bin/openrgb\0--server\0"));
        assert!(is_visible_openrgb("openrgb\0"));
        assert!(!is_visible_openrgb("/usr/bin/openrgb\0--server\0--startminimized\0"));
        assert!(!is_visible_openrgb("/usr/bin/chroma-desk\0--hidden\0"));
        assert!(!is_visible_openrgb(""));
    }

    #[test]
    fn candidates_are_absolute_paths_for_this_platform() {
        let c = openrgb_candidates();
        assert!(!c.is_empty() && c.iter().all(|p| p.is_absolute() || cfg!(windows)));
    }
}
