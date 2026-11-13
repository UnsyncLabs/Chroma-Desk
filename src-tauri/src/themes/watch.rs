//! Change detection for the active theme source: `notify` (inotify) on the source's
//! directories with a 300 ms debounce, plus a once-a-second fingerprint poll as a safety net
//! (a theme switch may replace whole directories, and inotify can be unavailable).

use super::ctl::ThemeCtl;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(300);
const POLL: Duration = Duration::from_secs(1);

/// Starts the watcher thread; it stops when `stop` is set (or the process exits).
pub fn spawn(ctl: Arc<ThemeCtl>, stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new().name("theme-watch".into()).spawn(move || run(&ctl, &stop)).expect("spawn theme watcher")
}

/// Points `watcher` at `want` (directories that exist), forgetting `old`.
fn resync(watcher: &mut Option<RecommendedWatcher>, old: &mut Vec<PathBuf>, want: Vec<PathBuf>) {
    if let Some(w) = watcher {
        for p in old.drain(..) {
            let _ = w.unwatch(&p);
        }
        for p in &want {
            if p.is_dir() && w.watch(p, RecursiveMode::NonRecursive).is_ok() {
                old.push(p.clone());
            }
        }
    }
}

fn run(ctl: &ThemeCtl, stop: &AtomicBool) {
    let (tx, rx) = channel::<()>();
    let tx2 = tx.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            let _ = tx2.send(());
        }
    })
    .map_err(|e| eprintln!("theme watcher unavailable, polling only: {e}"))
    .ok();
    let mut watched = Vec::new();
    let mut seen_epoch = 0;
    while !stop.load(Ordering::Relaxed) {
        let epoch = ctl.watch_epoch();
        let want = ctl.watch_paths();
        // Re-point the watches when the source changed or a directory appeared/vanished.
        if epoch != seen_epoch || want.iter().filter(|p| p.is_dir()).count() != watched.len() {
            seen_epoch = epoch;
            resync(&mut watcher, &mut watched, want);
        }
        match rx.recv_timeout(POLL) {
            Ok(()) => {
                // Let the burst of writes of a theme switch settle.
                while rx.recv_timeout(DEBOUNCE).is_ok() {}
                ctl.reload();
                // Replaced directories need fresh watches.
                resync(&mut watcher, &mut watched, ctl.watch_paths());
            }
            Err(RecvTimeoutError::Timeout) => {
                ctl.poll();
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    drop(tx);
}
