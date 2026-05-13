//! Workbench-Snapshot persistence (Phase 1 of state-persistence plan).
//!
//! The frontend serialises its `WorkbenchSnapshot` to JSON and hands it to
//! [`workbench_save_state`]; [`workbench_load_state`] returns whatever was
//! previously persisted. Storage lives in the OS-specific app config dir
//! (`~/.config/<id>/`, `~/Library/Application Support/<id>/`,
//! `%APPDATA%\<id>\`). Writes are atomic (temp + rename) so a crash mid-
//! flush never leaves a half-written file.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

const STATE_FILE: &str = "workbench.json";

fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(STATE_FILE))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("mkdir {}: {e}", path.display()))
}

#[tauri::command]
pub fn workbench_save_state(app: AppHandle, json: String) -> Result<(), String> {
    let target = state_path(&app)?;
    if let Some(parent) = target.parent() {
        ensure_dir(parent)?;
    }
    // Sanity: must parse as JSON; refuse to write garbage.
    let _: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| format!("snapshot is not valid JSON: {e}"))?;

    let tmp = target.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(json.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &target)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), target.display()))?;
    Ok(())
}

#[tauri::command]
pub fn workbench_load_state(app: AppHandle) -> Result<Option<String>, String> {
    let target = state_path(&app)?;
    match fs::read_to_string(&target) {
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", target.display())),
    }
}
