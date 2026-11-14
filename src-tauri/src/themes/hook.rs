//! The optional Omarchy `theme-set` hook: tells the running app to re-read the palette right
//! after a theme switch (the app also watches the files itself; the hook is a safety net).
//! It is only installed when the user asks for it in Settings, with `omarchy hook install`.

use super::sources::{Env, which};
use crate::config::config_dir;
use crate::error::{ApiError, Res};
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

/// File name inside `~/.config/omarchy/hooks/theme-set.d`.
pub const NAME: &str = "50-chroma-desk";

#[derive(Debug, Clone, Serialize)]
pub struct HookStatus {
    /// `omarchy` is on this machine.
    pub available: bool,
    pub installed: bool,
    /// Where the hook lives (or would live) in Omarchy's hook directory.
    pub path: String,
    /// The script the app keeps in its own config directory.
    pub script: String,
    /// What the Install button runs.
    pub command: String,
}

pub const SCRIPT: &str = r#"#!/bin/bash
# chroma-desk: tell the running app that the Omarchy theme changed.
# (The app also watches the theme files; this is a safety net.) Remove with: rm "$0"
SOCK="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/chroma-desk.sock"
if command -v chroma-desk >/dev/null 2>&1; then
  chroma-desk palette reload >/dev/null 2>&1 || true
elif [[ -S $SOCK ]] && command -v socat >/dev/null 2>&1; then
  printf '{"cmd":"palette_reload"}\n' | socat - "UNIX-CONNECT:$SOCK" >/dev/null 2>&1 || true
fi
"#;

pub fn script_path() -> PathBuf {
    config_dir().join("hooks").join(NAME)
}

pub fn status(env: &Env) -> HookStatus {
    let target = env.omarchy_hook_dir().join(NAME);
    let script = script_path();
    HookStatus {
        available: which("omarchy"),
        installed: target.is_file(),
        path: target.to_string_lossy().into_owned(),
        command: format!("omarchy hook install theme-set {}", script.display()),
        script: script.to_string_lossy().into_owned(),
    }
}

/// `chmod 755` (Omarchy is Linux-only; elsewhere this is a no-op so the module still compiles).
fn make_executable(script: &std::path::Path) -> Res<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755)).map_err(ApiError::io)?;
    }
    let _ = script;
    Ok(())
}

/// Writes the script and installs it with `<program> hook install theme-set <script>`.
pub fn install_with(env: &Env, program: &str, script: &std::path::Path) -> Res<()> {
    crate::fsutil::write_atomic(script, SCRIPT.as_bytes()).map_err(ApiError::io)?;
    make_executable(script)?;
    let out = Command::new(program).args(["hook", "install", "theme-set"]).arg(script).env("HOME", &env.home).output().map_err(|e| ApiError::new("hook_failed", e))?;
    if !out.status.success() {
        return Err(ApiError::new("hook_failed", String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    Ok(())
}

pub fn install(env: &Env) -> Res<()> {
    if !which("omarchy") {
        return Err(ApiError::new("hook_failed", "omarchy was not found"));
    }
    install_with(env, "omarchy", &script_path())
}

/// Deletes the installed hook (only our own file, by exact name).
pub fn remove(env: &Env) -> Res<()> {
    let target = env.omarchy_hook_dir().join(NAME);
    if target.is_file() {
        std::fs::remove_file(&target).map_err(ApiError::io)?;
    }
    Ok(())
}

// ---------- Omarchy event hooks (battery low, theme changed, post-update) ----------

/// The Omarchy hook types a notification can listen to.
pub const EVENTS: [&str; 3] = ["battery-low", "theme-set", "post-update"];

#[derive(Debug, Clone, Serialize)]
pub struct EventHookStatus {
    /// `battery-low`, `theme-set` or `post-update`.
    pub kind: String,
    /// The event id notifications listen to (`omarchy:battery-low`).
    pub event: String,
    pub installed: bool,
    /// Installed hook file.
    pub path: String,
    /// Script that will be installed (shown before installing).
    pub script_text: String,
    /// What the Install button runs.
    pub command: String,
}

/// File name inside `~/.config/omarchy/hooks/<kind>.d` (distinct from the palette hook `50-chroma-desk`).
pub fn event_file(kind: &str) -> String {
    format!("55-chroma-desk-{kind}")
}

pub fn event_script(kind: &str) -> String {
    format!(
        r#"#!/bin/bash
# chroma-desk: fire the notifications that listen to omarchy:{kind}.
# Remove with: rm "$0"
SOCK="${{XDG_RUNTIME_DIR:-/run/user/$(id -u)}}/chroma-desk.sock"
if command -v chroma-desk >/dev/null 2>&1; then
  chroma-desk event omarchy:{kind} >/dev/null 2>&1 || true
elif [[ -S $SOCK ]] && command -v socat >/dev/null 2>&1; then
  printf '{{"cmd":"event","event":"omarchy:{kind}"}}\n' | socat - "UNIX-CONNECT:$SOCK" >/dev/null 2>&1 || true
fi
"#
    )
}

fn event_script_path(kind: &str) -> PathBuf {
    config_dir().join("hooks").join(event_file(kind))
}

fn check_kind(kind: &str) -> Res<()> {
    if EVENTS.contains(&kind) { Ok(()) } else { Err(ApiError::new("hook_failed", format!("unknown hook type {kind:?}"))) }
}

pub fn event_status(env: &Env) -> Vec<EventHookStatus> {
    EVENTS
        .iter()
        .map(|kind| {
            let target = env.home.join(".config/omarchy/hooks").join(format!("{kind}.d")).join(event_file(kind));
            EventHookStatus {
                kind: (*kind).into(),
                event: format!("omarchy:{kind}"),
                installed: target.is_file(),
                path: target.to_string_lossy().into_owned(),
                script_text: event_script(kind),
                command: format!("omarchy hook install {kind} {}", event_script_path(kind).display()),
            }
        })
        .collect()
}

pub fn install_event_with(env: &Env, program: &str, kind: &str, script: &std::path::Path) -> Res<()> {
    check_kind(kind)?;
    crate::fsutil::write_atomic(script, event_script(kind).as_bytes()).map_err(ApiError::io)?;
    make_executable(script)?;
    let out = Command::new(program).args(["hook", "install", kind]).arg(script).env("HOME", &env.home).output().map_err(|e| ApiError::new("hook_failed", e))?;
    if !out.status.success() {
        return Err(ApiError::new("hook_failed", String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    Ok(())
}

pub fn install_event(env: &Env, kind: &str) -> Res<()> {
    check_kind(kind)?;
    if !which("omarchy") {
        return Err(ApiError::new("hook_failed", "omarchy was not found"));
    }
    install_event_with(env, "omarchy", kind, &event_script_path(kind))
}

/// Deletes our hook file of `kind` (exact name only).
pub fn remove_event(env: &Env, kind: &str) -> Res<()> {
    check_kind(kind)?;
    let target = env.home.join(".config/omarchy/hooks").join(format!("{kind}.d")).join(event_file(kind));
    if target.is_file() {
        std::fs::remove_file(&target).map_err(ApiError::io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir;

    #[cfg(unix)]
    #[test]
    fn install_and_remove_with_a_fake_omarchy() {
        use std::os::unix::fs::PermissionsExt;
        let root = test_dir("hook");
        let env = Env::under(&root);
        // A stand-in for `omarchy hook install <type> <file>` that copies like the real one.
        let fake = root.join("omarchy");
        std::fs::write(&fake, "#!/bin/bash\n[[ $1 == hook && $2 == install && $3 == theme-set ]] || exit 3\nmkdir -p \"$HOME/.config/omarchy/hooks/$3.d\"\ncp -- \"$4\" \"$HOME/.config/omarchy/hooks/$3.d/$(basename \"$4\")\"\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let script = root.join("cfg/hooks").join(NAME);
        assert!(!status(&env).installed);
        install_with(&env, fake.to_str().unwrap(), &script).unwrap();
        assert!(status(&env).installed);
        let target = env.omarchy_hook_dir().join(NAME);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), SCRIPT);
        assert_eq!(std::fs::metadata(&script).unwrap().permissions().mode() & 0o777, 0o755);
        remove(&env).unwrap();
        assert!(!status(&env).installed && !target.exists());
        remove(&env).unwrap(); // removing twice is fine
        assert_eq!(install_with(&env, "/nonexistent/omarchy", &script).unwrap_err().code, "hook_failed");
        assert!(status(&env).command.starts_with("omarchy hook install theme-set "));
    }

    #[cfg(unix)]
    #[test]
    fn event_hooks_install_and_remove_only_their_own_file() {
        use std::os::unix::fs::PermissionsExt;
        let root = test_dir("hook-ev");
        let env = Env::under(&root);
        let fake = root.join("omarchy");
        std::fs::write(&fake, "#!/bin/bash\nmkdir -p \"$HOME/.config/omarchy/hooks/$3.d\"\ncp -- \"$4\" \"$HOME/.config/omarchy/hooks/$3.d/$(basename \"$4\")\"\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        // A hook of the user in the same directory must survive.
        let theirs = root.join(".config/omarchy/hooks/battery-low.d/play-warning-sound.sample");
        std::fs::create_dir_all(theirs.parent().unwrap()).unwrap();
        std::fs::write(&theirs, "x").unwrap();
        assert!(event_status(&env).iter().all(|e| !e.installed));
        install_event_with(&env, fake.to_str().unwrap(), "battery-low", &root.join("cfg/55-chroma-desk-battery-low")).unwrap();
        let st = event_status(&env);
        assert!(st.iter().find(|e| e.kind == "battery-low").unwrap().installed);
        assert!(!st.iter().find(|e| e.kind == "post-update").unwrap().installed);
        let text = std::fs::read_to_string(root.join(".config/omarchy/hooks/battery-low.d/55-chroma-desk-battery-low")).unwrap();
        assert!(text.contains("chroma-desk event omarchy:battery-low") && text.contains("\"cmd\":\"event\""));
        remove_event(&env, "battery-low").unwrap();
        assert!(theirs.exists() && event_status(&env).iter().all(|e| !e.installed));
        assert!(install_event_with(&env, fake.to_str().unwrap(), "../etc", &root.join("x")).is_err());
        assert!(remove_event(&env, "nope").is_err());
    }
}
