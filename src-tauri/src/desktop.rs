//! Desktop notifications as a notification source: a *monitor* connection on the session bus
//! watches `org.freedesktop.Notifications.Notify` calls (the messages other applications send to
//! the notification daemon). Opt-in (Settings). Privacy: only the application name, the urgency
//! and the summary are looked at to decide whether a notification matches; the message body is
//! never read into memory beyond the D-Bus decoding and nothing is stored or logged.

use crate::engine::Engine;
use crate::triggers::{self, DesktopNote};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use zbus::zvariant::OwnedValue;

const RULE: &str = "type='method_call',interface='org.freedesktop.Notifications',member='Notify'";

/// `Notify(app_name, replaces_id, app_icon, summary, body, actions, hints, expire_timeout)`.
type NotifyArgs = (String, u32, String, String, String, Vec<String>, HashMap<String, OwnedValue>, i32);

/// The matchable part of a `Notify` call.
pub fn note_of(args: &NotifyArgs) -> DesktopNote {
    let urgency = args.6.get("urgency").and_then(|v| u8::try_from(v).ok()).unwrap_or(1);
    DesktopNote { app: args.0.clone(), urgency, summary: args.3.clone() }
}

/// Listens until the connection fails or `enabled` goes false; every matched call is sent on `tx`.
pub async fn monitor(address: Option<&str>, enabled: &AtomicBool, tx: &UnboundedSender<DesktopNote>) -> zbus::Result<()> {
    let conn = match address {
        Some(a) => zbus::connection::Builder::address(a)?.build().await?,
        None => zbus::Connection::session().await?,
    };
    let rule = zbus::MatchRule::try_from(RULE)?;
    zbus::fdo::MonitoringProxy::builder(&conn).destination("org.freedesktop.DBus")?.path("/org/freedesktop/DBus")?.build().await?.become_monitor(&[rule], 0).await?;
    let mut stream = zbus::MessageStream::from(&conn);
    loop {
        let next = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
        if !enabled.load(Ordering::Relaxed) {
            return Ok(());
        }
        let Ok(Some(msg)) = next else {
            if matches!(next, Ok(None)) {
                return Ok(());
            }
            continue;
        };
        let Ok(msg) = msg else { continue };
        if msg.header().member().is_none_or(|m| m.as_str() != "Notify") {
            continue;
        }
        if let Ok(args) = msg.body().deserialize::<NotifyArgs>() {
            let _ = tx.send(note_of(&args));
        }
    }
}

/// Status shown in Settings: `off`, `listening` or `error: ...`.
pub fn set_status(engine: &Engine, s: &str) {
    *engine.desktop_status.lock().unwrap() = s.to_string();
}

/// Background task: runs the monitor while `enabled` is set and fires notifications for matches.
pub async fn run(engine: Arc<Engine>, dir: PathBuf, enabled: Arc<AtomicBool>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let fire = {
        let engine = engine.clone();
        tokio::spawn(async move {
            while let Some(note) = rx.recv().await {
                triggers::fire_desktop(&engine, &dir, &note, Instant::now());
            }
        })
    };
    loop {
        if !enabled.load(Ordering::Relaxed) {
            set_status(&engine, "off");
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }
        set_status(&engine, "listening");
        match monitor(None, &enabled, &tx).await {
            Ok(()) => {}
            Err(e) => {
                set_status(&engine, &format!("error: {e}"));
                // Wait before trying again (no session bus, or the daemon refused monitoring).
                for _ in 0..30 {
                    if !enabled.load(Ordering::Relaxed) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        if fire.is_finished() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    /// A private session bus (`dbus-daemon`), a monitor on it and a `Notify` call made with
    /// `busctl`. Skipped when the tools are missing.
    #[tokio::test]
    async fn sees_notify_calls_on_a_private_bus() {
        let have = |p: &str| Command::new(p).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok();
        if !have("dbus-daemon") || !have("busctl") {
            eprintln!("skipped: dbus-daemon/busctl not installed");
            return;
        }
        let dir = crate::fsutil::test_dir("dbus");
        let sock = dir.join("bus");
        let mut daemon = Command::new("dbus-daemon").args(["--session", "--nofork", "--nopidfile"]).arg(format!("--address=unix:path={}", sock.display())).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
        for _ in 0..50 {
            if sock.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let addr = format!("unix:path={}", sock.display());
        let enabled = Arc::new(AtomicBool::new(true));
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (a2, e2) = (addr.clone(), enabled.clone());
        let task = tokio::spawn(async move { monitor(Some(&a2), &e2, &tx).await });
        tokio::time::sleep(Duration::from_millis(600)).await;
        // Three calls: different app, urgency and summary.
        for (app, summary, urgency) in [("Discord", "New message from Ana", 1u8), ("mail", "Build failed", 2u8)] {
            let _ = Command::new("busctl")
                .args(["--address", &addr, "call", "org.freedesktop.Notifications", "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "Notify", "susssasa{sv}i", app, "0", "", summary, "secret body text", "0", "1", "urgency", "y", &urgency.to_string(), "5000"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let mut got = vec![];
        for _ in 0..2 {
            if let Ok(Some(n)) = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
                got.push(n);
            }
        }
        enabled.store(false, Ordering::Relaxed);
        let _ = tokio::time::timeout(Duration::from_secs(3), task).await;
        let _ = daemon.kill();
        let _ = daemon.wait();
        assert_eq!(got.len(), 2, "both Notify calls are seen: {got:?}");
        assert_eq!(got[0], DesktopNote { app: "Discord".into(), urgency: 1, summary: "New message from Ana".into() });
        assert_eq!((got[1].app.as_str(), got[1].urgency), ("mail", 2));
    }
}
