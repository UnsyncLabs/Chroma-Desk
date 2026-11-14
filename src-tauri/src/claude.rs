//! "Set up Claude Code": adds (and removes) a `Notification` hook to Claude Code's
//! `settings.json` that runs `chroma-desk event ai:attention --from claude`, so the keyboard
//! lights up when Claude asks for permission or waits for an answer.
//!
//! The file is merged, never replaced: every other setting and hook stays, in its original order.
//! Before any change the file is copied next to itself (`settings.json.chroma-desk-backup`), a file
//! that is not valid JSON is left untouched, and installing twice changes nothing.
//! Paths are always a parameter here so tests only ever touch temporary files.

use crate::error::{ApiError, Res};
use crate::fsutil::write_atomic;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// What the hook runs.
pub const COMMAND: &str = "chroma-desk event ai:attention --from claude";

/// `$CLAUDE_CONFIG_DIR/settings.json`, else `~/.claude/settings.json`.
pub fn default_path() -> Option<PathBuf> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| Some(crate::fsutil::home_dir()).filter(|h| !h.as_os_str().is_empty()).map(|h| h.join(".claude")))?;
    Some(dir.join("settings.json"))
}

/// The JSON this adds, exactly as it appears in `settings.json`.
pub fn snippet() -> Value {
    json!({ "hooks": { "Notification": [ { "hooks": [ { "type": "command", "command": COMMAND } ] } ] } })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClaudeStatus {
    pub path: String,
    pub exists: bool,
    /// The file parses (a missing file counts as fine).
    pub valid: bool,
    pub installed: bool,
    pub command: &'static str,
    /// Pretty-printed [`snippet`].
    pub snippet: String,
    /// Where the previous version was saved by the last change made through this call.
    pub backup: Option<String>,
}

fn is_ours(command: &str) -> bool {
    // Our command, possibly with extra flags; a hook for another tool (`--from codex`) is never touched.
    let c = command.trim();
    c == COMMAND || c.strip_prefix(COMMAND).is_some_and(|rest| rest.starts_with(char::is_whitespace))
}

fn read(path: &Path) -> Res<Option<Map<String, Value>>> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ApiError::io(e)),
    };
    if text.trim().is_empty() {
        return Ok(Some(Map::new()));
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => Ok(Some(m)),
        Ok(_) => Err(ApiError::new("claude_settings_invalid", "the file is not a JSON object")),
        Err(e) => Err(ApiError::new("claude_settings_invalid", e)),
    }
}

/// The hook entries of our command inside `doc`: `(group index, hook index)`.
fn find_ours(doc: &Map<String, Value>) -> Vec<(usize, usize)> {
    let mut out = vec![];
    if let Some(groups) = doc.get("hooks").and_then(|h| h.get("Notification")).and_then(Value::as_array) {
        for (g, group) in groups.iter().enumerate() {
            for (h, hook) in group.get("hooks").and_then(Value::as_array).into_iter().flatten().enumerate() {
                if hook.get("command").and_then(Value::as_str).is_some_and(is_ours) {
                    out.push((g, h));
                }
            }
        }
    }
    out
}

pub fn status(path: &Path) -> ClaudeStatus {
    let doc = read(path);
    ClaudeStatus {
        path: path.display().to_string(),
        exists: path.exists(),
        valid: doc.is_ok(),
        installed: doc.as_ref().is_ok_and(|d| d.as_ref().is_some_and(|d| !find_ours(d).is_empty())),
        command: COMMAND,
        snippet: serde_json::to_string_pretty(&snippet()).unwrap_or_default(),
        backup: None,
    }
}

/// Copies `path` to a backup next to it (never overwriting an earlier backup).
fn backup(path: &Path) -> Res<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut dest = path.with_file_name(format!("{name}.chroma-desk-backup"));
    let mut i = 2;
    while dest.exists() {
        dest = path.with_file_name(format!("{name}.chroma-desk-backup-{i}"));
        i += 1;
    }
    std::fs::copy(path, &dest).map_err(ApiError::io)?;
    Ok(dest)
}

fn save(path: &Path, doc: Map<String, Value>) -> Res<()> {
    // A settings.json symlinked from a dotfiles repo stays a symlink: write to its target.
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = resolved.as_path();
    let mut text = serde_json::to_string_pretty(&Value::Object(doc)).map_err(ApiError::io)?;
    text.push('\n');
    let mode = std::fs::metadata(path).ok().map(|m| m.permissions());
    write_atomic(path, text.as_bytes()).map_err(ApiError::io)?;
    if let Some(m) = mode {
        let _ = std::fs::set_permissions(path, m);
    }
    Ok(())
}

/// Adds the hook. Nothing changes (and no backup is made) when it is already there.
pub fn install(path: &Path) -> Res<ClaudeStatus> {
    let existing = read(path)?;
    let mut doc = existing.clone().unwrap_or_default();
    if !find_ours(&doc).is_empty() {
        return Ok(status(path));
    }
    let hooks = doc.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks.as_object_mut().ok_or_else(|| ApiError::new("claude_settings_invalid", "\"hooks\" is not an object"))?;
    let list = hooks.entry("Notification").or_insert_with(|| json!([]));
    let list = list.as_array_mut().ok_or_else(|| ApiError::new("claude_settings_invalid", "\"hooks.Notification\" is not a list"))?;
    list.push(snippet()["hooks"]["Notification"][0].clone());
    let saved = if existing.is_some() { Some(backup(path)?) } else { None };
    save(path, doc)?;
    Ok(ClaudeStatus { backup: saved.map(|p| p.display().to_string()), ..status(path) })
}

/// Removes our hook and whatever became empty because of it; everything else stays.
pub fn remove(path: &Path) -> Res<ClaudeStatus> {
    let Some(mut doc) = read(path)? else { return Ok(status(path)) };
    let ours = find_ours(&doc);
    if ours.is_empty() {
        return Ok(status(path));
    }
    if let Some(groups) = doc.get_mut("hooks").and_then(|h| h.get_mut("Notification")).and_then(Value::as_array_mut) {
        for (g, h) in ours.into_iter().rev() {
            if let Some(hooks) = groups[g].get_mut("hooks").and_then(Value::as_array_mut) {
                hooks.remove(h);
            }
        }
        groups.retain(|g| g.get("hooks").and_then(Value::as_array).is_none_or(|h| !h.is_empty()));
        if groups.is_empty()
            && let Some(h) = doc.get_mut("hooks").and_then(Value::as_object_mut)
        {
            h.shift_remove("Notification");
        }
    }
    if doc.get("hooks").and_then(Value::as_object).is_some_and(Map::is_empty) {
        doc.shift_remove("hooks");
    }
    let saved = backup(path)?;
    save(path, doc)?;
    Ok(ClaudeStatus { backup: Some(saved.display().to_string()), ..status(path) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir;

    const USER: &str = r#"{
  "theme": "dark",
  "permissions": { "allow": ["Bash(ls:*)"] },
  "hooks": {
    "Stop": [ { "hooks": [ { "type": "command", "command": "echo done" } ] } ],
    "Notification": [ { "matcher": "idle_prompt", "hooks": [ { "type": "command", "command": "notify-send idle" } ] } ]
  },
  "model": "opus"
}
"#;

    fn file(tag: &str, text: &str) -> PathBuf {
        let p = test_dir(tag).join("settings.json");
        std::fs::write(&p, text).unwrap();
        p
    }

    fn json_of(p: &Path) -> Value {
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    }

    #[test]
    fn install_merges_keeps_everything_and_backs_up() {
        let p = file("claude-merge", USER);
        assert!(!status(&p).installed);
        let st = install(&p).unwrap();
        assert!(st.installed && st.valid);
        let backup = PathBuf::from(st.backup.clone().unwrap());
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), USER, "backup is the untouched original");
        let doc = json_of(&p);
        let before: Value = serde_json::from_str(USER).unwrap();
        for k in ["theme", "permissions", "model"] {
            assert_eq!(doc[k], before[k]);
        }
        assert_eq!(doc["hooks"]["Stop"], before["hooks"]["Stop"]);
        let notif = doc["hooks"]["Notification"].as_array().unwrap();
        assert_eq!(notif.len(), 2);
        assert_eq!(notif[0], before["hooks"]["Notification"][0], "the user's hook is first and unchanged");
        assert_eq!(notif[1]["hooks"][0], json!({"type": "command", "command": COMMAND}));
        // The user's key order survives.
        let keys: Vec<&String> = doc.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["theme", "permissions", "hooks", "model"]);
        // Idempotent: no second copy, no second backup.
        let again = install(&p).unwrap();
        assert!(again.installed && again.backup.is_none());
        assert_eq!(json_of(&p)["hooks"]["Notification"].as_array().unwrap().len(), 2);
        assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 2, "settings + one backup");
    }

    #[test]
    fn remove_restores_the_original_content() {
        let p = file("claude-remove", USER);
        install(&p).unwrap();
        let st = remove(&p).unwrap();
        assert!(!st.installed);
        assert_eq!(json_of(&p), serde_json::from_str::<Value>(USER).unwrap());
        assert!(st.backup.is_some());
        // Removing again is a no-op.
        assert!(remove(&p).unwrap().backup.is_none());
    }

    #[test]
    fn creates_a_missing_file_and_removal_leaves_no_empty_hooks() {
        let dir = test_dir("claude-new");
        let p = dir.join("sub").join("settings.json");
        let st = install(&p).unwrap();
        assert!(st.installed && st.backup.is_none());
        assert_eq!(json_of(&p), snippet());
        let st = remove(&p).unwrap();
        assert!(!st.installed);
        assert_eq!(json_of(&p), json!({}));
        // A file with only an empty hooks object is also fine.
        let q = file("claude-empty", "{}");
        install(&q).unwrap();
        assert_eq!(json_of(&q), snippet());
        let blank = file("claude-blank", "  \n");
        assert!(install(&blank).unwrap().installed);
    }

    #[test]
    fn invalid_or_unexpected_files_are_left_alone() {
        for (tag, text) in [("claude-bad1", "{ not json"), ("claude-bad2", "[1, 2]"), ("claude-bad3", r#"{"hooks": []}"#), ("claude-bad4", r#"{"hooks": {"Notification": {}}}"#)] {
            let p = file(tag, text);
            let st = status(&p);
            assert!(!st.installed);
            let e = install(&p).unwrap_err();
            assert_eq!(e.code, "claude_settings_invalid", "{text}");
            assert_eq!(std::fs::read_to_string(&p).unwrap(), text, "untouched");
            assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1, "no backup, no temp file");
            let _ = remove(&p);
            assert_eq!(std::fs::read_to_string(&p).unwrap(), text, "remove leaves it alone too");
            assert_eq!(std::fs::read_dir(p.parent().unwrap()).unwrap().count(), 1);
        }
        assert!(!status(&file("claude-bad5", "{ nope")).valid);
    }

    #[test]
    fn existing_backups_are_never_overwritten_and_other_chroma_commands_count_as_installed() {
        let custom = r#"{"hooks":{"Notification":[{"hooks":[{"type":"command","command":"chroma-desk event ai:attention --from claude --kind idle"}]}]}}"#;
        let p = file("claude-custom", custom);
        assert!(status(&p).installed, "an edited variant of our command is recognised");
        assert!(install(&p).unwrap().backup.is_none());
        let q = file("claude-backups", USER);
        std::fs::write(q.with_file_name("settings.json.chroma-desk-backup"), "old").unwrap();
        let st = install(&q).unwrap();
        assert!(st.backup.unwrap().ends_with("backup-2"));
        assert_eq!(std::fs::read_to_string(q.with_file_name("settings.json.chroma-desk-backup")).unwrap(), "old");
    }

    #[test]
    #[cfg(unix)]
    fn other_tools_and_symlinked_files_are_respected() {
        let other = r#"{"hooks":{"Notification":[{"hooks":[{"type":"command","command":"chroma-desk event ai:attention --from codex"},{"type":"command","command":"chroma-desk event ai:attention --from claudette"}]}]}}"#;
        let p = file("claude-other", other);
        assert!(!status(&p).installed);
        install(&p).unwrap();
        remove(&p).unwrap();
        assert_eq!(json_of(&p), serde_json::from_str::<Value>(other).unwrap(), "user's hooks for other tools survive");
        // A dotfiles symlink stays a symlink and its target is what changes.
        let dir = test_dir("claude-link");
        let target = dir.join("dotfiles.json");
        std::fs::write(&target, USER).unwrap();
        let link = dir.join("settings.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        install(&link).unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert!(status(&target).installed);
    }

    #[test]
    fn the_snippet_is_what_install_adds() {
        let s = status(&file("claude-snip", "{}"));
        assert_eq!(serde_json::from_str::<Value>(&s.snippet).unwrap(), snippet());
        assert!(s.snippet.contains(COMMAND));
    }
}
