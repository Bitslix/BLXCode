//! Persistent **workspace presets** for the Create-Workspace flow.
//!
//! A preset is a reusable fleet configuration — terminal count, per-agent
//! counts, per-slot names, and an optional harness session role — that the user
//! can launch in one click. Presets are *global per installation*, stored in
//! `{app_data_dir}/workspace_presets.json` (not committed with any workspace).
//!
//! Writes are atomic (tmp + rename), matching the pattern used by the
//! skills/rules store, so a partial JSON file never lands on disk.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const PRESETS_FILE: &str = "workspace_presets.json";
/// Number of fleet agent rows; must match the frontend
/// `WORKSPACE_FLEET_AGENT_SLUGS` length.
const FLEET_AGENT_ROWS: usize = 5;
const MAX_TERMINALS: u8 = 16;
const MAX_NAME_LEN: usize = 80;

/// One saved fleet configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePreset {
    /// Stable id (UUID v4 simple hex). Empty on save → generated server-side.
    #[serde(default)]
    pub id: String,
    /// User-facing preset label.
    pub name: String,
    /// Terminal/grid count (1..=16).
    pub terminal_count: u8,
    /// Per-agent-row counts, parallel to `WORKSPACE_FLEET_AGENT_SLUGS`.
    #[serde(default)]
    pub agent_counts: [u8; FLEET_AGENT_ROWS],
    /// Per-agent-row CLI model ids, parallel to `agent_counts`. Empty = default.
    #[serde(default)]
    pub agent_models: [String; FLEET_AGENT_ROWS],
    /// Optional per-slot friendly names (index-aligned to slots).
    #[serde(default)]
    pub slot_names: Vec<String>,
    /// Optional harness session-role slug to apply when launching.
    #[serde(default)]
    pub session_role: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct PresetsFile {
    #[serde(default)]
    presets: Vec<WorkspacePreset>,
}

fn presets_path() -> Result<PathBuf, String> {
    Ok(crate::app_paths::app_data_dir()?.join(PRESETS_FILE))
}

fn read_file() -> PresetsFile {
    let Ok(path) = presets_path() else {
        return PresetsFile::default();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<PresetsFile>(&s).ok())
        .unwrap_or_default()
}

fn write_file(file: &PresetsFile) -> Result<(), String> {
    let path = presets_path()?;
    let parent = path.parent().ok_or("invalid presets path")?;
    fs::create_dir_all(parent).map_err(|e| format!("mkdir app data dir: {e}"))?;
    let json = serde_json::to_vec_pretty(file).map_err(|e| format!("encode presets: {e}"))?;
    let tmp = parent.join(format!(".{PRESETS_FILE}.{}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
        f.write_all(&json).map_err(|e| format!("write tmp: {e}"))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path).map_err(|e| format!("rename tmp -> final: {e}"))
}

/// Normalizes a preset for storage: clamps the count, trims the name and slot
/// names, and rejects an empty name.
fn sanitize(mut preset: WorkspacePreset) -> Result<WorkspacePreset, String> {
    preset.name = preset.name.trim().to_string();
    if preset.name.is_empty() {
        return Err("preset name is empty".into());
    }
    if preset.name.chars().count() > MAX_NAME_LEN {
        preset.name = preset.name.chars().take(MAX_NAME_LEN).collect();
    }
    preset.terminal_count = preset.terminal_count.clamp(1, MAX_TERMINALS);
    preset.slot_names = preset
        .slot_names
        .into_iter()
        .map(|n| n.trim().chars().take(MAX_NAME_LEN).collect::<String>())
        .collect();
    preset.session_role = preset
        .session_role
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(preset)
}

/// Returns all saved presets in stored order.
pub fn list() -> Vec<WorkspacePreset> {
    read_file().presets
}

/// Inserts or updates a preset (by `id`) and returns the full saved list. An
/// empty `id` creates a new preset with a generated id.
pub fn save(preset: WorkspacePreset) -> Result<Vec<WorkspacePreset>, String> {
    let mut preset = sanitize(preset)?;
    let mut file = read_file();
    if preset.id.trim().is_empty() {
        preset.id = uuid::Uuid::new_v4().simple().to_string();
        file.presets.push(preset);
    } else {
        match file.presets.iter_mut().find(|p| p.id == preset.id) {
            Some(existing) => *existing = preset,
            None => file.presets.push(preset),
        }
    }
    write_file(&file)?;
    Ok(file.presets)
}

/// Removes a preset by id and returns the remaining list.
pub fn delete(id: &str) -> Result<Vec<WorkspacePreset>, String> {
    let mut file = read_file();
    file.presets.retain(|p| p.id != id);
    write_file(&file)?;
    Ok(file.presets)
}

// ===========================================================================
// Tauri commands
// ===========================================================================

#[tauri::command]
pub fn workspace_presets_list() -> Vec<WorkspacePreset> {
    list()
}

#[tauri::command]
pub fn workspace_presets_save(preset: WorkspacePreset) -> Result<Vec<WorkspacePreset>, String> {
    save(preset)
}

#[tauri::command]
pub fn workspace_presets_delete(id: String) -> Result<Vec<WorkspacePreset>, String> {
    delete(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_paths::test_support::AppDataDirGuard;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "blx_presets_{tag}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample(name: &str) -> WorkspacePreset {
        WorkspacePreset {
            id: String::new(),
            name: name.to_string(),
            terminal_count: 4,
            agent_counts: [2, 1, 1, 0, 0],
            agent_models: [
                "opus".into(),
                "gpt-5".into(),
                String::new(),
                String::new(),
                String::new(),
            ],
            slot_names: vec!["api".into(), "ui".into()],
            session_role: Some("coordinator".into()),
        }
    }

    #[test]
    fn save_then_list_round_trips() {
        let dir = tmp_dir("roundtrip");
        let _g = AppDataDirGuard::new(dir.clone());
        let saved = save(sample("My Fleet")).unwrap();
        assert_eq!(saved.len(), 1);
        assert!(!saved[0].id.is_empty());
        let listed = list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "My Fleet");
        assert_eq!(listed[0].terminal_count, 4);
        assert_eq!(listed[0].session_role.as_deref(), Some("coordinator"));
        assert_eq!(listed[0].agent_models[0], "opus");
        assert_eq!(listed[0].agent_models[1], "gpt-5");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_upserts_by_id() {
        let dir = tmp_dir("upsert");
        let _g = AppDataDirGuard::new(dir.clone());
        let saved = save(sample("v1")).unwrap();
        let id = saved[0].id.clone();
        let mut edited = saved[0].clone();
        edited.name = "v2".into();
        edited.terminal_count = 8;
        let after = save(edited).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, id);
        assert_eq!(after[0].name, "v2");
        assert_eq!(after[0].terminal_count, 8);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_removes_preset() {
        let dir = tmp_dir("delete");
        let _g = AppDataDirGuard::new(dir.clone());
        let saved = save(sample("gone")).unwrap();
        let id = saved[0].id.clone();
        let after = delete(&id).unwrap();
        assert!(after.is_empty());
        assert!(list().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn count_is_clamped_and_empty_name_rejected() {
        let dir = tmp_dir("clamp");
        let _g = AppDataDirGuard::new(dir.clone());
        let mut p = sample("Clamp");
        p.terminal_count = 99;
        let saved = save(p).unwrap();
        assert_eq!(saved[0].terminal_count, MAX_TERMINALS);
        assert!(save(sample("   ")).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_json_yields_empty_list() {
        let dir = tmp_dir("malformed");
        let _g = AppDataDirGuard::new(dir.clone());
        fs::write(dir.join(PRESETS_FILE), b"{ not json").unwrap();
        assert!(list().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_leaves_no_tmp_file() {
        let dir = tmp_dir("notmp");
        let _g = AppDataDirGuard::new(dir.clone());
        save(sample("x")).unwrap();
        let tmps: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(tmps.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
