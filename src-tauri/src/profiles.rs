//! Profiles: JSON files in ~/.config/chroma-desk/profiles with a schema version, atomic writes
//! and tolerant loading (a corrupt file is reported and skipped, never fatal).

use crate::config::config_dir;
use crate::engine::Scene;
use crate::error::{ApiError, Res};
use crate::fsutil::write_atomic;
use crate::themes::{Color, ProfileTheme};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 2;

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_false(b: &bool) -> bool {
    !*b
}

/// A library animation a profile pins (shown first in Quick).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimRef {
    pub id: String,
    #[serde(default)]
    pub name: String,
}

/// A trigger name a game or script can fire (`chroma-desk trigger low_health`); each profile
/// decides what it looks like, so the same command looks different under another theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedTrigger {
    /// Animation id or name.
    pub anim: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    /// `@slot` or `#rrggbb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

pub const MAX_PRESETS: usize = 64;
pub const MAX_TRIGGERS: usize = 64;

/// Trigger names are short and filename-like so they are easy to type in a shell.
pub fn valid_trigger_name(n: &str) -> bool {
    !n.is_empty() && n.chars().count() <= 32 && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub schema_version: u32,
    pub name: String,
    /// An OpenRGB profile to load on apply (imported from OpenRGB's own list).
    #[serde(default)]
    pub openrgb_profile: Option<String>,
    /// Zone sizes by `<device identity>|<zone>`.
    #[serde(default)]
    pub zone_sizes: BTreeMap<String, usize>,
    pub scene: Scene,
    /// Where the colours come from (and the profile's own palette).
    #[serde(default)]
    pub theme: ProfileTheme,
    /// Animations shown first in Quick while this profile is active.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<AnimRef>,
    /// Named triggers (`chroma-desk trigger <name>`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub triggers: BTreeMap<String, NamedTrigger>,
    /// Built-in profiles can be switched off (Omarchy ships disabled).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Shipped in the binary, read-only (never stored with this flag).
    #[serde(default, skip_serializing_if = "is_false")]
    pub builtin: bool,
    /// Desktop that must be present for the profile to be offered (`omarchy`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<String>,
}

impl Profile {
    /// An empty user profile.
    pub fn new(name: &str, scene: Scene) -> Profile {
        Profile {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            openrgb_profile: None,
            zone_sizes: BTreeMap::new(),
            scene,
            theme: ProfileTheme::default(),
            presets: vec![],
            triggers: BTreeMap::new(),
            enabled: true,
            builtin: false,
            requires: None,
        }
    }

    /// Size and name checks for what the UI or a file supplies.
    pub fn validate_extras(&self) -> Res<()> {
        self.theme.validate().map_err(|e| ApiError::new("profile_invalid", e))?;
        if self.presets.len() > MAX_PRESETS || self.triggers.len() > MAX_TRIGGERS {
            return Err(ApiError::new("profile_invalid", "too many presets or triggers"));
        }
        for (n, t) in &self.triggers {
            if !valid_trigger_name(n) {
                return Err(ApiError::new("profile_invalid", format!("bad trigger name {n:?}")));
            }
            if t.anim.trim().is_empty()
                || t.anim.chars().count() > 128
                || t.keys.len() > 64
                || t.color.as_deref().is_some_and(|c| Color::parse(c).is_none())
                || t.duration.is_some_and(|d| !d.is_finite() || !(0.05..=3600.0).contains(&d))
            {
                return Err(ApiError::new("profile_invalid", format!("bad trigger {n:?}")));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProfileInfo {
    pub name: String,
    pub file: String,
    pub openrgb: bool,
}

#[derive(Debug, Default, Serialize)]
pub struct ProfileList {
    pub profiles: Vec<ProfileInfo>,
    /// Files that could not be read.
    pub corrupt: Vec<String>,
}

pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
}

/// File-name-safe version of a profile name.
pub fn slug(name: &str) -> String {
    let s: String = name.trim().chars().flat_map(|c| if c.is_alphanumeric() { c.to_lowercase().collect::<Vec<_>>() } else { vec!['-'] }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "profile".into() } else { s }
}

pub fn validate_name(name: &str) -> Res<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 64 {
        return Err(ApiError::new("profile_name_invalid", n));
    }
    Ok(n.to_string())
}

/// Upgrades an older profile document to `SCHEMA_VERSION`.
pub fn migrate(mut v: Value) -> Res<Value> {
    let version = v.get("schema_version").and_then(Value::as_u64).unwrap_or(0);
    if version > SCHEMA_VERSION as u64 {
        return Err(ApiError::new("profile_too_new", version));
    }
    let obj = v.as_object_mut().ok_or_else(|| ApiError::new("profile_corrupt", "not an object"))?;
    if version < 1 {
        // v0 (no version field) -> v1: zone sizes became part of the profile.
        obj.entry("zone_sizes").or_insert_with(|| Value::Object(Default::default()));
        obj.insert("schema_version".into(), 1.into());
    }
    if version < 2 {
        // v1 -> v2: themes, presets and named triggers. Old colours are plain RGB and stay valid.
        obj.entry("theme").or_insert_with(|| serde_json::json!({ "source": "manual" }));
        obj.insert("schema_version".into(), 2.into());
    }
    Ok(v)
}

pub fn parse(text: &str) -> Res<Profile> {
    let v: Value = serde_json::from_str(text).map_err(|e| ApiError::new("profile_corrupt", e))?;
    let p: Profile = serde_json::from_value(migrate(v)?).map_err(|e| ApiError::new("profile_corrupt", e))?;
    p.validate_extras().map_err(|e| ApiError::new("profile_corrupt", e.detail))?;
    Ok(p)
}

fn read_file(p: &Path) -> Res<Profile> {
    parse(&std::fs::read_to_string(p).map_err(ApiError::io)?)
}

pub fn list(dir: &Path) -> ProfileList {
    let mut out = ProfileList::default();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut files: Vec<_> = rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
    files.sort();
    for p in files {
        let file = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        match read_file(&p) {
            Ok(pr) => out.profiles.push(ProfileInfo { name: pr.name, file, openrgb: pr.openrgb_profile.is_some() }),
            Err(_) => out.corrupt.push(file),
        }
    }
    out.profiles.sort_by_key(|p| p.name.to_lowercase());
    out
}

/// Path of the file holding profile `name`, if any.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    rd.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).find(|p| read_file(p).is_ok_and(|pr| pr.name == name))
}

pub fn load(dir: &Path, name: &str) -> Res<Profile> {
    let p = find(dir, name).ok_or_else(|| ApiError::new("profile_not_found", name))?;
    read_file(&p)
}

/// A file name for a new profile that does not clash with an existing one.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let base = slug(name);
    let mut p = dir.join(format!("{base}.json"));
    let mut i = 2;
    while p.exists() {
        p = dir.join(format!("{base}-{i}.json"));
        i += 1;
    }
    p
}

/// Saves `profile`, overwriting the file of the profile with the same name if it exists.
pub fn save(dir: &Path, profile: &Profile) -> Res<ProfileInfo> {
    let name = validate_name(&profile.name)?;
    let path = find(dir, &name).unwrap_or_else(|| free_path(dir, &name));
    profile.validate_extras()?;
    let mut pr = profile.clone();
    pr.name = name;
    pr.schema_version = SCHEMA_VERSION;
    pr.builtin = false;
    let json = serde_json::to_vec_pretty(&pr).map_err(ApiError::io)?;
    write_atomic(&path, &json).map_err(ApiError::io)?;
    Ok(ProfileInfo { name: pr.name, file: path.file_name().unwrap_or_default().to_string_lossy().into_owned(), openrgb: pr.openrgb_profile.is_some() })
}

pub fn delete(dir: &Path, name: &str) -> Res<()> {
    let p = find(dir, name).ok_or_else(|| ApiError::new("profile_not_found", name))?;
    std::fs::remove_file(p).map_err(ApiError::io)
}

pub fn rename(dir: &Path, old: &str, new: &str) -> Res<ProfileInfo> {
    let new = validate_name(new)?;
    if old != new && find(dir, &new).is_some() {
        return Err(ApiError::new("profile_exists", new));
    }
    let mut pr = load(dir, old)?;
    pr.name = new;
    let info = save_as_new_file(dir, &pr, old)?;
    Ok(info)
}

/// Writes `pr` (renamed from `old`) and removes the old file.
fn save_as_new_file(dir: &Path, pr: &Profile, old: &str) -> Res<ProfileInfo> {
    let old_path = find(dir, old);
    let path = free_path(dir, &pr.name);
    let json = serde_json::to_vec_pretty(pr).map_err(ApiError::io)?;
    write_atomic(&path, &json).map_err(ApiError::io)?;
    if let Some(o) = old_path
        && o != path
    {
        std::fs::remove_file(o).map_err(ApiError::io)?;
    }
    Ok(ProfileInfo { name: pr.name.clone(), file: path.file_name().unwrap_or_default().to_string_lossy().into_owned(), openrgb: pr.openrgb_profile.is_some() })
}

pub fn duplicate(dir: &Path, name: &str, new: &str) -> Res<ProfileInfo> {
    let new = validate_name(new)?;
    if find(dir, &new).is_some() {
        return Err(ApiError::new("profile_exists", new));
    }
    let mut pr = load(dir, name)?;
    pr.name = new;
    save(dir, &pr)
}

pub fn export(dir: &Path, name: &str, dest: &Path) -> Res<()> {
    let pr = load(dir, name)?;
    let json = serde_json::to_vec_pretty(&pr).map_err(ApiError::io)?;
    write_atomic(dest, &json).map_err(ApiError::io)
}

/// Imports a profile file; a name clash gets a numeric suffix.
pub fn import(dir: &Path, src: &Path) -> Res<ProfileInfo> {
    let mut pr = read_file(src)?;
    pr.name = validate_name(&pr.name)?;
    let base = pr.name.clone();
    let mut i = 2;
    while find(dir, &pr.name).is_some() {
        pr.name = format!("{base} ({i})");
        i += 1;
    }
    save(dir, &pr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{DeviceScene, SceneMode};
    use crate::fsutil::test_dir;

    fn sample(name: &str) -> Profile {
        let mut scene = Scene { sync: true, master_brightness: 0.7, ..Default::default() };
        scene.devices.insert("Node#1".into(), DeviceScene { mode: SceneMode::Effects, ..Default::default() });
        let mut zone_sizes = BTreeMap::new();
        zone_sizes.insert("Node#1|Header 1".to_string(), 24);
        Profile { zone_sizes, ..Profile::new(name, scene) }
    }

    #[test]
    fn slug_is_filename_safe() {
        assert_eq!(slug("  Gaming / Night!  "), "gaming-night");
        assert_eq!(slug("¡¡"), "profile");
        assert_eq!(slug("Ñandú 2"), "ñandú-2");
    }

    #[test]
    fn save_load_roundtrip() {
        let d = test_dir("prof");
        let p = sample("Gaming");
        save(&d, &p).unwrap();
        assert_eq!(load(&d, "Gaming").unwrap(), p);
        // Saving again overwrites instead of duplicating.
        save(&d, &p).unwrap();
        assert_eq!(list(&d).profiles.len(), 1);
    }

    #[test]
    fn names_with_same_slug_get_distinct_files() {
        let d = test_dir("prof-slug");
        let a = save(&d, &sample("A b")).unwrap();
        let b = save(&d, &sample("a-B")).unwrap();
        assert_ne!(a.file, b.file);
        assert_eq!(list(&d).profiles.len(), 2);
    }

    #[test]
    fn corrupt_files_are_reported_not_fatal() {
        let d = test_dir("prof-bad");
        save(&d, &sample("Good")).unwrap();
        std::fs::write(d.join("bad.json"), "{ nope").unwrap();
        std::fs::write(d.join("array.json"), "[1,2]").unwrap();
        let l = list(&d);
        assert_eq!(l.profiles.len(), 1);
        assert_eq!(l.corrupt.len(), 2);
        assert_eq!(load(&d, "Good").unwrap().name, "Good");
        assert_eq!(load(&d, "Missing").unwrap_err().code, "profile_not_found");
    }

    #[test]
    fn migrates_v0_documents() {
        let v0 = r#"{"name": "Old", "scene": {"master_brightness": 0.5}}"#;
        let p = parse(v0).unwrap();
        assert_eq!(p.schema_version, 2);
        assert!(p.zone_sizes.is_empty());
        assert_eq!(p.scene.master_brightness, 0.5);
    }

    #[test]
    fn newer_schema_is_rejected_clearly() {
        let e = parse(r#"{"schema_version": 99, "name": "x", "scene": {}}"#).unwrap_err();
        assert_eq!(e.code, "profile_too_new");
    }

    #[test]
    fn rename_duplicate_delete() {
        let d = test_dir("prof-ops");
        save(&d, &sample("One")).unwrap();
        duplicate(&d, "One", "Two").unwrap();
        assert_eq!(duplicate(&d, "One", "Two").unwrap_err().code, "profile_exists");
        rename(&d, "Two", "Three").unwrap();
        let names: Vec<_> = list(&d).profiles.into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["One", "Three"]);
        assert_eq!(rename(&d, "One", "Three").unwrap_err().code, "profile_exists");
        delete(&d, "One").unwrap();
        assert_eq!(list(&d).profiles.len(), 1);
        assert_eq!(delete(&d, "One").unwrap_err().code, "profile_not_found");
    }

    #[test]
    fn export_import_roundtrip_with_name_clash() {
        let d = test_dir("prof-exp");
        save(&d, &sample("Mine")).unwrap();
        let out = d.join("exported.json.bak");
        export(&d, "Mine", &out).unwrap();
        let info = import(&d, &out).unwrap();
        assert_eq!(info.name, "Mine (2)");
        assert_eq!(load(&d, "Mine (2)").unwrap().scene, load(&d, "Mine").unwrap().scene);
    }

    #[test]
    fn invalid_names_rejected() {
        assert_eq!(validate_name("   ").unwrap_err().code, "profile_name_invalid");
        assert_eq!(validate_name(&"x".repeat(65)).unwrap_err().code, "profile_name_invalid");
    }

    #[test]
    fn no_temp_files_left_after_save() {
        let d = test_dir("prof-tmp");
        save(&d, &sample("T")).unwrap();
        let hidden = std::fs::read_dir(&d).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with('.')).count();
        assert_eq!(hidden, 0);
    }

    #[test]
    fn v1_profile_migrates_to_v2_unchanged() {
        // A schema-1 file as the previous release wrote it (plain RGB colours, no theme).
        let old = r#"{ "schema_version": 1, "name": "Old", "zone_sizes": {}, "scene": { "master_brightness": 0.5, "devices": { "k": { "mode": "effects", "layers": [
            { "id": "a", "name": "", "effect": "solid", "params": { "color": [10, 20, 30] } } ] } } } }"#;
        let p = parse(old).unwrap();
        assert_eq!(p.schema_version, 2);
        assert_eq!(p.theme, ProfileTheme::default());
        assert!(p.enabled && !p.builtin && p.presets.is_empty() && p.triggers.is_empty());
        assert_eq!(p.scene.devices["k"].layers[0].params.0["color"], serde_json::json!([10, 20, 30]));
        // Stored as v2, and loadable again.
        let d = test_dir("prof-mig");
        std::fs::write(d.join("old.json"), old).unwrap();
        assert_eq!(load(&d, "Old").unwrap().scene, p.scene);
        save(&d, &p).unwrap();
        let text = std::fs::read_to_string(d.join("old.json")).unwrap();
        assert!(text.contains("\"schema_version\": 2"), "{text}");
        // Future versions are refused.
        assert_eq!(parse(r#"{"schema_version": 99, "name": "x", "scene": {}}"#).unwrap_err().code, "profile_too_new");
    }

    #[test]
    fn theme_presets_and_triggers_roundtrip_and_validate() {
        let d = test_dir("prof-theme");
        let mut p = sample("Themed");
        p.theme.source = crate::themes::ThemeSource::Omarchy;
        p.theme.overrides.insert("alert".into(), [255, 0, 0]);
        p.presets.push(AnimRef { id: "builtin:rainbow".into(), name: "Rainbow".into() });
        p.triggers.insert("low_health".into(), NamedTrigger { anim: "Alert".into(), keys: vec!["W".into()], color: Some("@alert".into()), duration: Some(2.0), device: None });
        save(&d, &p).unwrap();
        assert_eq!(load(&d, "Themed").unwrap(), p);
        let mut bad = p.clone();
        bad.triggers.insert("bad name!".into(), NamedTrigger { anim: "x".into(), keys: vec![], color: None, duration: None, device: None });
        assert_eq!(save(&d, &bad).unwrap_err().code, "profile_invalid");
        let mut bad = p.clone();
        bad.theme.overrides.insert("nope".into(), [0, 0, 0]);
        assert!(save(&d, &bad).is_err());
        let mut bad = p;
        bad.triggers.get_mut("low_health").unwrap().color = Some("@nope".into());
        assert!(save(&d, &bad).is_err());
    }
}
