//! What this build/platform can do, so the UI can hide what is not available.
//!
//! The ids are stable strings the frontend checks (`src/lib/capabilities.svelte.ts`).

/// Capability ids available on the current platform.
pub fn list() -> Vec<&'static str> {
    let mut v = vec!["claude_hook", "nvidia_gpu"];
    if cfg!(target_os = "linux") {
        // D-Bus notification monitor, Omarchy/KDE/pywal/GNOME theme readers, systemd user unit, AMD sysfs.
        v.extend(["desktop_notifications", "omarchy", "kde", "pywal", "gnome", "systemd_service", "amd_gpu", "cpu_temp"]);
    }
    if cfg!(unix) {
        v.push("unix_socket");
    } else {
        v.push("named_pipe");
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_keeps_every_feature_and_exactly_one_socket_kind() {
        let v = list();
        assert!(v.contains(&"claude_hook"));
        assert_eq!(v.contains(&"unix_socket") as u8 + v.contains(&"named_pipe") as u8, 1);
        if cfg!(target_os = "linux") {
            for c in ["desktop_notifications", "omarchy", "kde", "pywal", "gnome", "systemd_service"] {
                assert!(v.contains(&c), "{c}");
            }
        } else {
            assert!(!v.contains(&"omarchy") && !v.contains(&"systemd_service"));
        }
    }
}
