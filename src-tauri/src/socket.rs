//! Local control socket: `$XDG_RUNTIME_DIR/chroma-desk.sock`, one JSON object per line.
//!
//! Local only (a Unix socket, mode 0600, no network). Every line is size-limited and validated;
//! see `triggers::handle_line` for the protocol.

use crate::engine::Engine;
use crate::triggers;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Semaphore;

/// Longest accepted request line (bytes).
pub const MAX_LINE: usize = 8 * 1024;
const MAX_CONNECTIONS: usize = 16;
const MAX_LINES_PER_CONNECTION: usize = 100;
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// `$XDG_RUNTIME_DIR/chroma-desk.sock`; `None` without a runtime dir (no insecure fallback).
pub fn socket_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty())?;
    Some(PathBuf::from(dir).join("chroma-desk.sock"))
}

/// Binds the socket (replacing a stale one) with mode 0600.
pub async fn bind(path: &Path) -> std::io::Result<UnixListener> {
    if path.exists() {
        if UnixStream::connect(path).await.is_ok() {
            return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "another instance is listening"));
        }
        std::fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Applies a profile by name (the application provides it); returns the profile's name.
pub type ProfileFn = Arc<dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::error::Res<String>> + Send>> + Send + Sync>;

/// Accepts connections forever. `profiles` handles `{"cmd":"set_profile","name":...}`.
pub async fn serve(listener: UnixListener, engine: Arc<Engine>, dir: PathBuf, profiles: Option<ProfileFn>) {
    let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        };
        let Ok(permit) = slots.clone().try_acquire_owned() else { continue };
        let (engine, dir, profiles) = (engine.clone(), dir.clone(), profiles.clone());
        tokio::spawn(async move {
            let _permit = permit;
            let _ = tokio::time::timeout(Duration::from_secs(60), connection(stream, engine, dir, profiles)).await;
        });
    }
}

async fn connection(stream: UnixStream, engine: Arc<Engine>, dir: PathBuf, profiles: Option<ProfileFn>) {
    let (r, mut w) = stream.into_split();
    let mut r = BufReader::new(r);
    for _ in 0..MAX_LINES_PER_CONNECTION {
        let mut buf = Vec::new();
        let read = tokio::time::timeout(READ_TIMEOUT, (&mut r).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut buf)).await;
        match read {
            Ok(Ok(n)) if n > 0 => {}
            _ => return,
        }
        let complete = buf.last() == Some(&b'\n');
        let reply = if buf.len() > MAX_LINE || (!complete && buf.len() == MAX_LINE + 1) {
            serde_json::json!({ "ok": false, "error": "too_large", "detail": format!("max {MAX_LINE} bytes per line") })
        } else {
            match std::str::from_utf8(&buf) {
                Ok(s) if s.trim().is_empty() => continue,
                Ok(s) => match set_profile_name(s.trim()) {
                    Some(name) => set_profile(profiles.as_ref(), name).await,
                    None => triggers::handle_line(&engine, &dir, s.trim(), Instant::now()),
                },
                Err(_) => serde_json::json!({ "ok": false, "error": "bad_utf8" }),
            }
        };
        let fatal = reply["error"] == "too_large";
        let mut out = reply.to_string();
        out.push('\n');
        if w.write_all(out.as_bytes()).await.is_err() || fatal {
            return;
        }
    }
}

/// The requested name when `line` is a `set_profile` command.
fn set_profile_name(line: &str) -> Option<Result<String, String>> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    (v["cmd"] == "set_profile").then(|| match v["name"].as_str().map(str::trim) {
        Some(n) if !n.is_empty() && n.chars().count() <= 64 => Ok(n.to_string()),
        _ => Err("name must be a profile name (1 to 64 characters)".to_string()),
    })
}

async fn set_profile(hook: Option<&ProfileFn>, name: Result<String, String>) -> serde_json::Value {
    let name = match name {
        Ok(n) => n,
        Err(why) => return serde_json::json!({ "ok": false, "error": "bad_request", "detail": why }),
    };
    let Some(hook) = hook else {
        return serde_json::json!({ "ok": false, "error": "unsupported", "detail": "profiles are not available here" });
    };
    match hook(name).await {
        Ok(applied) => serde_json::json!({ "ok": true, "profile": applied }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.code, "detail": e.detail }),
    }
}

/// Blocking client (the `chroma-desk trigger` CLI): sends one line, returns the reply line.
pub fn send(path: &Path, line: &str) -> std::io::Result<String> {
    let mut s = std::os::unix::net::UnixStream::connect(path)?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    s.set_write_timeout(Some(Duration::from_secs(5)))?;
    s.write_all(line.trim_end().as_bytes())?;
    s.write_all(b"\n")?;
    let mut out = Vec::new();
    let mut byte = [0u8; 1];
    while out.len() < 64 * 1024 && s.read(&mut byte)? == 1 && byte[0] != b'\n' {
        out.push(byte[0]);
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations;
    use crate::engine::{DevInfo, DeviceScene, SceneMode};
    use crate::fsutil::test_dir;

    fn engine() -> Arc<Engine> {
        let e = Engine::default();
        e.devs.lock().unwrap().push(DevInfo {
            index: 0,
            key: "kb".into(),
            name: "Keyboard".into(),
            device_type: "Keyboard".into(),
            led_count: 2,
            led_names: vec!["Key: W".into(), "Key: A".into()],
            zones: vec![(0, 0, 2)],
            default_positions: vec![[0.5, 0.5]; 2],
        });
        e.scene.lock().unwrap().devices.insert("kb".into(), DeviceScene { mode: SceneMode::Effects, ..Default::default() });
        Arc::new(e)
    }

    async fn start() -> (PathBuf, Arc<Engine>) {
        let d = test_dir("sock");
        let path = d.join("c.sock");
        let e = engine();
        let l = bind(&path).await.unwrap();
        tokio::spawn(serve(l, e.clone(), d.join("animations"), None));
        (path, e)
    }

    async fn ask(path: &Path, payload: &[u8]) -> String {
        let mut s = UnixStream::connect(path).await.unwrap();
        s.write_all(payload).await.unwrap();
        let mut r = BufReader::new(s);
        let mut line = String::new();
        let _ = tokio::time::timeout(Duration::from_secs(3), r.read_line(&mut line)).await;
        line
    }

    #[tokio::test]
    async fn socket_is_private_and_answers() {
        let (path, e) = start().await;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "socket must be accessible by the owner only");
        assert_eq!(ask(&path, b"{\"cmd\":\"ping\"}\n").await.trim(), r#"{"ok":true}"#);
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"{\"animation\":\"Alert\",\"keys\":[\"W\"],\"duration\":\"2s\"}\n").await).unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(e.overlays.lock().unwrap().len(), 1);
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"{\"cmd\":\"stop\"}\n").await).unwrap();
        assert_eq!(r["stopped"], 1);
        // A client that sends several lines on one connection gets one reply per line.
        let multi = ask(&path, b"{\"cmd\":\"ping\"}\n{\"cmd\":\"ping\"}\n").await;
        assert!(multi.starts_with(r#"{"ok":true}"#));
    }

    #[tokio::test]
    async fn rejects_oversized_and_garbage_lines() {
        let (path, _e) = start().await;
        let mut big = vec![b'a'; MAX_LINE * 4];
        big.push(b'\n');
        let r: serde_json::Value = serde_json::from_str(&ask(&path, &big).await).unwrap();
        assert_eq!(r["error"], "too_large");
        // Exactly at the limit (with newline) is still parsed.
        let mut limit = b"{\"cmd\":\"ping\"".to_vec();
        limit.resize(MAX_LINE - 2, b' ');
        limit.extend_from_slice(b"}\n");
        assert_eq!(limit.len(), MAX_LINE);
        assert_eq!(ask(&path, &limit).await.trim(), r#"{"ok":true}"#);
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"\xff\xfe\n").await).unwrap();
        assert_eq!(r["error"], "bad_utf8");
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"{oops\n").await).unwrap();
        assert_eq!(r["error"], "bad_json");
        // The server survives all of that.
        assert_eq!(ask(&path, b"{\"cmd\":\"ping\"}\n").await.trim(), r#"{"ok":true}"#);
    }

    #[tokio::test]
    async fn stale_socket_is_replaced_but_live_one_is_not() {
        let d = test_dir("sock-stale");
        let path = d.join("c.sock");
        std::fs::write(&path, "stale").unwrap();
        let l = bind(&path).await.unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        let _keep = tokio::spawn(serve(l, engine(), d.clone(), None));
        assert_eq!(bind(&path).await.unwrap_err().kind(), std::io::ErrorKind::AddrInUse);
        let _ = animations::animations_dir; // keep the import used in all cfgs
    }

    #[tokio::test]
    async fn concurrent_and_stalled_clients_do_not_block_others() {
        let (path, _e) = start().await;
        // A client that connects, sends half a line and then just sits there.
        let mut stalled = UnixStream::connect(&path).await.unwrap();
        stalled.write_all(b"{\"cmd\":\"pi").await.unwrap();
        // Many clients at once (more than the connection limit is fine: extra ones are dropped).
        let tasks: Vec<_> = (0..24)
            .map(|_| {
                let p = path.clone();
                tokio::spawn(async move {
                    let Ok(mut s) = UnixStream::connect(&p).await else { return String::new() };
                    let _ = s.write_all(b"{\"cmd\":\"ping\"}\n").await; // may be refused over the limit
                    let mut line = String::new();
                    let _ = tokio::time::timeout(Duration::from_secs(3), BufReader::new(s).read_line(&mut line)).await;
                    line
                })
            })
            .collect();
        let mut ok = 0;
        for t in tasks {
            if t.await.unwrap().trim() == r#"{"ok":true}"# {
                ok += 1;
            }
        }
        assert!(ok >= MAX_CONNECTIONS - 1, "only {ok} of 24 answered");
        drop(stalled);
        assert_eq!(ask(&path, b"{\"cmd\":\"ping\"}\n").await.trim(), r#"{"ok":true}"#, "server still answers");
    }

    #[test]
    fn blocking_client_roundtrip() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (path, _e) = rt.block_on(start());
        let reply = send(&path, r#"{"cmd":"ping"}"#).unwrap();
        assert_eq!(reply, r#"{"ok":true}"#);
        assert!(send(&path.with_extension("nope"), "{}").is_err());
    }

    #[tokio::test]
    async fn set_profile_goes_through_the_application_hook() {
        let d = test_dir("sock-prof");
        let path = d.join("c.sock");
        let l = bind(&path).await.unwrap();
        let hook: ProfileFn = Arc::new(|name: String| {
            Box::pin(async move { if name == "Known" { Ok(name) } else { Err(crate::error::ApiError::new("profile_not_found", name)) } })
        });
        tokio::spawn(serve(l, engine(), d.clone(), Some(hook)));
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"{\"cmd\":\"set_profile\",\"name\":\"Known\"}\n").await).unwrap();
        assert_eq!((r["ok"].as_bool(), r["profile"].as_str()), (Some(true), Some("Known")), "{r}");
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"{\"cmd\":\"set_profile\",\"name\":\"Nope\"}\n").await).unwrap();
        assert_eq!(r["error"], "profile_not_found");
        let r: serde_json::Value = serde_json::from_str(&ask(&path, b"{\"cmd\":\"set_profile\"}\n").await).unwrap();
        assert_eq!(r["error"], "bad_request");
        // Without a hook (tests, examples) the command is refused clearly.
        let (p2, _e) = start().await;
        let r: serde_json::Value = serde_json::from_str(&ask(&p2, b"{\"cmd\":\"set_profile\",\"name\":\"x\"}\n").await).unwrap();
        assert_eq!(r["error"], "unsupported");
    }
}
