//! The live palette: combines the active profile's theme (source, manual palette, overrides)
//! with colours sent by games/scripts, and keeps the last good palette when a source breaks.

use super::sources::{Env, Source, fingerprint, source};
use super::{Palette, ProfileTheme, Rgb3, ThemeSource, is_slot};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    theme: ProfileTheme,
    /// `palette source <id>` from the CLI: wins over the profile's source until another profile is applied.
    runtime_source: Option<ThemeSource>,
    /// Slots sent by a game or script (`set_palette`).
    external: BTreeMap<String, Rgb3>,
    last_good: HashMap<ThemeSource, Palette>,
    /// Why the active source could not be read (None when fine).
    error: Option<String>,
    fingerprint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SourceStatus {
    pub id: &'static str,
    /// The desktop/tool is present on this machine.
    pub detected: bool,
    /// Palette name of the source right now (e.g. the Omarchy theme).
    pub name: Option<String>,
    pub error: Option<String>,
    /// A few colours of the source's palette (cards show them live).
    pub strip: Vec<Rgb3>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeStatus {
    /// Source in use (profile's, or the CLI override).
    pub active: &'static str,
    pub runtime_override: bool,
    /// A game/script palette is layered on top.
    pub external: bool,
    pub error: Option<String>,
    pub sources: Vec<SourceStatus>,
    pub palette: Palette,
}

/// What `palette-changed` carries.
#[derive(Debug, Clone, Serialize)]
pub struct PaletteEvent {
    pub palette: Palette,
    pub active: &'static str,
    pub external: bool,
    pub error: Option<String>,
}

pub struct ThemeCtl {
    pub env: Env,
    state: Mutex<State>,
    palette: Mutex<Arc<Palette>>,
    /// Bumped whenever the palette or the error changes (the runner turns it into an event).
    epoch: AtomicU64,
    /// Bumped when the set of watched paths may have changed.
    watch_epoch: AtomicU64,
    /// Watch Omarchy's theme name even when no profile uses it (theme rules need it).
    watch_name: AtomicBool,
    /// `(last seen Omarchy theme name, changed and not yet taken)`.
    name: Mutex<(Option<String>, Option<String>)>,
}

impl Default for ThemeCtl {
    fn default() -> Self {
        Self::new(Env::from_env())
    }
}

impl ThemeCtl {
    pub fn new(env: Env) -> ThemeCtl {
        let c = ThemeCtl { env, state: Mutex::default(), palette: Mutex::new(Arc::new(Palette::default_manual())), epoch: AtomicU64::new(1), watch_epoch: AtomicU64::new(1), watch_name: AtomicBool::new(false), name: Mutex::default() };
        c.recompute();
        c
    }

    /// The theme of the active profile.
    pub fn theme(&self) -> ProfileTheme {
        self.state.lock().unwrap().theme.clone()
    }

    pub fn palette(&self) -> Arc<Palette> {
        self.palette.lock().unwrap().clone()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Relaxed)
    }

    pub fn watch_epoch(&self) -> u64 {
        self.watch_epoch.load(Ordering::Relaxed)
    }

    fn active_of(st: &State) -> ThemeSource {
        st.runtime_source.unwrap_or(st.theme.source)
    }

    /// The profile changed: new theme, and the CLI override and game palette are dropped.
    pub fn set_theme(&self, theme: ProfileTheme) {
        {
            let mut st = self.state.lock().unwrap();
            st.theme = theme;
            st.runtime_source = None;
            st.external.clear();
            st.error = None;
            st.fingerprint.clear();
        }
        self.watch_epoch.fetch_add(1, Ordering::Relaxed);
        self.recompute();
    }

    /// Updates the theme of the active profile in place (the editor changed a slot): keeps game colours.
    pub fn update_theme(&self, theme: ProfileTheme) {
        let src_changed = {
            let mut st = self.state.lock().unwrap();
            let changed = st.theme.source != theme.source;
            st.theme = theme;
            changed
        };
        if src_changed {
            self.watch_epoch.fetch_add(1, Ordering::Relaxed);
        }
        self.recompute();
    }

    pub fn set_external(&self, slots: BTreeMap<String, Rgb3>) -> Result<bool, String> {
        if let Some(k) = slots.keys().find(|k| !is_slot(k)) {
            return Err(format!("unknown palette slot {k:?}"));
        }
        self.state.lock().unwrap().external.extend(slots);
        Ok(self.recompute())
    }

    pub fn clear_external(&self) -> bool {
        self.state.lock().unwrap().external.clear();
        self.recompute()
    }

    pub fn set_runtime_source(&self, src: ThemeSource) -> bool {
        {
            let mut st = self.state.lock().unwrap();
            st.runtime_source = Some(src);
            st.error = None;
            st.fingerprint.clear();
        }
        self.watch_epoch.fetch_add(1, Ordering::Relaxed);
        self.recompute()
    }

    /// Re-reads the active source. True if the palette changed.
    pub fn reload(&self) -> bool {
        self.check_name();
        self.recompute()
    }

    /// Theme rules need to know when the Omarchy theme changes, whatever the active source is.
    pub fn set_name_watch(&self, on: bool) {
        if self.watch_name.swap(on, Ordering::Relaxed) != on {
            *self.name.lock().unwrap() = (None, None);
            self.watch_epoch.fetch_add(1, Ordering::Relaxed);
            self.check_name();
        }
    }

    fn check_name(&self) {
        if !self.watch_name.load(Ordering::Relaxed) {
            return;
        }
        let now = super::omarchy::theme_name(&self.env);
        let mut n = self.name.lock().unwrap();
        if n.0 != now {
            // The first observation is a baseline, not a change.
            if n.0.is_some() || now.is_none() {
                n.1 = now.clone();
            }
            n.0 = now;
        }
    }

    /// The Omarchy theme name, once per change.
    pub fn take_theme_change(&self) -> Option<String> {
        self.name.lock().unwrap().1.take()
    }

    /// Recomputes the palette; true (and an epoch bump) when the result differs from the current one.
    fn recompute(&self) -> bool {
        let mut st = self.state.lock().unwrap();
        let active = Self::active_of(&st);
        let manual = st.theme.manual();
        let mut error = None;
        let base = match source(active) {
            None => manual.clone(),
            Some(src) => match src.read(&self.env) {
                Ok(p) => {
                    st.last_good.insert(active, p.clone());
                    p
                }
                Err(e) => {
                    error = Some(e);
                    st.last_good.get(&active).cloned().unwrap_or_else(|| manual.clone())
                }
            },
        };
        let mut slots = base.slots.clone();
        slots.extend(st.theme.overrides.clone());
        slots.extend(st.external.clone());
        let name = if st.external.is_empty() { base.name.clone() } else { format!("{} + external", base.name) };
        let next = Palette::new(&name, active.id(), slots);
        st.fingerprint = src_fingerprint(source(active), &self.env);
        let err_changed = st.error != error;
        st.error = error;
        let mut cur = self.palette.lock().unwrap();
        let changed = **cur != next;
        if changed {
            *cur = Arc::new(next);
        }
        if changed || err_changed {
            self.epoch.fetch_add(1, Ordering::Relaxed);
        }
        changed
    }

    /// Reloads when the active source's files changed since the last read (poll fallback for inotify).
    pub fn poll(&self) -> bool {
        self.check_name();
        let (src, last) = {
            let st = self.state.lock().unwrap();
            (source(Self::active_of(&st)), st.fingerprint.clone())
        };
        let Some(src) = src else { return false };
        let now = src_fingerprint(Some(src), &self.env);
        if now == last {
            return false;
        }
        self.recompute()
    }

    /// Directories to watch for the active source.
    pub fn watch_paths(&self) -> Vec<PathBuf> {
        let st = self.state.lock().unwrap();
        let mut paths = source(Self::active_of(&st)).map(|s| s.watch_paths(&self.env)).unwrap_or_default();
        if self.watch_name.load(Ordering::Relaxed) {
            for p in super::omarchy::Omarchy.watch_paths(&self.env) {
                if !paths.contains(&p) {
                    paths.push(p);
                }
            }
        }
        paths
    }

    pub fn event(&self) -> PaletteEvent {
        let st = self.state.lock().unwrap();
        PaletteEvent { palette: (*self.palette()).clone(), active: Self::active_of(&st).id(), external: !st.external.is_empty(), error: st.error.clone() }
    }

    /// Full picture for the Settings page (runs detection of every source).
    pub fn status(&self) -> ThemeStatus {
        let ev = self.event();
        let (runtime_override, error) = {
            let st = self.state.lock().unwrap();
            (st.runtime_source.is_some(), st.error.clone())
        };
        let sources = ThemeSource::ALL.iter().map(|&id| self.source_status(id)).collect();
        ThemeStatus { active: ev.active, runtime_override, external: ev.external, error, sources, palette: ev.palette }
    }

    pub fn source_status(&self, id: ThemeSource) -> SourceStatus {
        match source(id) {
            None => SourceStatus { id: id.id(), detected: true, name: None, error: None, strip: vec![] },
            Some(s) => {
                let detected = s.detect(&self.env);
                let (name, error, strip) = if detected {
                    match s.read(&self.env) {
                        Ok(p) => {
                            let strip = ["background", "accent", "primary", "secondary", "tertiary", "alert", "ok"].iter().map(|k| p.get(k)).collect();
                            (Some(p.name), None, strip)
                        }
                        Err(e) => (None, Some(e), vec![]),
                    }
                } else {
                    (None, None, vec![])
                };
                SourceStatus { id: id.id(), detected, name, error, strip }
            }
        }
    }

    pub fn detected(&self, id: ThemeSource) -> bool {
        source(id).is_none_or(|s| s.detect(&self.env))
    }
}

fn src_fingerprint(src: Option<&dyn Source>, env: &Env) -> String {
    src.map(|s| fingerprint(&s.poll_files(env))).unwrap_or_default()
}
