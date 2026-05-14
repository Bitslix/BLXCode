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

use serde::Deserialize;
use tauri::{AppHandle, Manager};

const STATE_FILE: &str = "workbench.json";
const SESSIONS_FILE: &str = "sessions.json";

fn state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(STATE_FILE))
}

fn sessions_path_impl(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(SESSIONS_FILE))
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

/// Returns the absolute path the SessionStart hook scripts write to. We
/// expose this so the frontend can pass it as an env var when spawning
/// PTYs, instead of mirroring Tauri's path logic in Python.
#[tauri::command]
pub fn workbench_sessions_path(app: AppHandle) -> Result<String, String> {
    Ok(sessions_path_impl(&app)?.to_string_lossy().into_owned())
}

/// Read the SessionStart-hook output (terminal_key → agent/session_id
/// mapping). The frontend consults this before auto-launching an agent
/// CLI to decide between `<agent>` and `<agent> --resume <id>`.
#[tauri::command]
pub fn workbench_load_sessions(app: AppHandle) -> Result<Option<String>, String> {
    let target = sessions_path_impl(&app)?;
    match fs::read_to_string(&target) {
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", target.display())),
    }
}

/// Drop every `terminals.*` entry whose key starts with `prefix`. Used
/// when a workspace or terminal slot is closed in the UI, to keep
/// `sessions.json` from accumulating stale references that point at
/// agent sessions no slot will ever resume.
///
/// Missing file is a no-op; a corrupt file is rewritten as empty rather
/// than crashing the close flow.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionProbe {
    pub agent: String,
    pub cwd: String,
    pub session_id: String,
}

/// Validate that the on-disk transcript for an `(agent, cwd, session_id)`
/// triple actually exists. Used by the frontend before issuing `--resume`
/// to avoid the "No conversation found with session ID …" error path
/// when the captured id belongs to a session that never had any turns
/// (Claude/Codex only persist the JSONL once at least one message is
/// committed).
#[tauri::command]
pub fn agent_session_exists(app: AppHandle, probe: AgentSessionProbe) -> bool {
    let Ok(home) = app.path().home_dir() else {
        return false;
    };
    let id = probe.session_id.trim();
    if id.is_empty() {
        return false;
    }
    match probe.agent.as_str() {
        "claude" => claude_session_path(&home, &probe.cwd, id).is_file(),
        "codex" => codex_session_present(&home, id),
        "gemini" => gemini_session_present(&home, id),
        // OpenCode and Cursor don't expose a stable, documented
        // transcript layout we can probe from the outside. Be permissive:
        // let the CLI tell the user if the id is gone, rather than
        // silently dropping every resume.
        "opencode" | "cursor" => true,
        _ => false,
    }
}

/// `~/.claude/projects/<cwd-with-slashes-replaced-by-dashes>/<id>.jsonl`.
/// Mirrors the directory layout Claude Code uses; replicating this here
/// is fragile but cheaper than spawning the CLI just to verify a session.
fn claude_session_path(home: &Path, cwd: &str, session_id: &str) -> PathBuf {
    let encoded = cwd.trim_end_matches('/').replace('/', "-");
    home.join(".claude")
        .join("projects")
        .join(encoded)
        .join(format!("{session_id}.jsonl"))
}

/// Gemini's per-session transcript lives at
/// `~/.gemini/sessions/<session_id>/transcript.json` (matches the path
/// the SessionStart hook payload reports).
fn gemini_session_present(home: &Path, session_id: &str) -> bool {
    home.join(".gemini")
        .join("sessions")
        .join(session_id)
        .join("transcript.json")
        .is_file()
}

/// Codex stores transcripts under
/// `~/.codex/sessions/<year>/<month>/<day>/rollout-<timestamp>-<id>.jsonl`,
/// so a precise path is not derivable from `session_id` alone. Walk the
/// tree until we find a file whose name contains the id. Depth is capped
/// to avoid pathological traversal.
fn codex_session_present(home: &Path, session_id: &str) -> bool {
    let root = home.join(".codex").join("sessions");
    if !root.is_dir() {
        return false;
    }
    let mut stack: Vec<(PathBuf, u32)> = vec![(root, 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > 5 {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push((p, depth + 1));
            } else if ft.is_file() {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.contains(session_id) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[tauri::command]
pub fn workbench_drop_sessions(app: AppHandle, prefix: String) -> Result<u32, String> {
    if prefix.is_empty() {
        return Ok(0);
    }
    let target = sessions_path_impl(&app)?;
    let raw = match fs::read_to_string(&target) {
        Ok(s) if s.trim().is_empty() => return Ok(0),
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("read {}: {e}", target.display())),
    };

    let mut state: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return Ok(0), // corrupt; nothing to remove
    };
    let removed = {
        let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) else {
            return Ok(0);
        };
        let before = terminals.len();
        terminals.retain(|k, _| !k.starts_with(&prefix));
        (before - terminals.len()) as u32
    };
    if removed == 0 {
        return Ok(0);
    }

    let tmp = target.with_extension("json.tmp");
    let body =
        serde_json::to_string_pretty(&state).map_err(|e| format!("serialize sessions: {e}"))?;
    {
        let mut f =
            fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &target)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), target.display()))?;
    Ok(removed)
}
