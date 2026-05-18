//! Workbench-Snapshot persistence (Phase 1 of state-persistence plan).
//!
//! The frontend serialises its `WorkbenchSnapshot` to JSON and hands it to
//! [`workbench_save_state`]; [`workbench_load_state`] returns whatever was
//! previously persisted. Storage lives in the OS-specific app config dir
//! (`~/.config/<id>/`, `~/Library/Application Support/<id>/`,
//! `%APPDATA%\<id>\`). Writes are atomic (temp + rename) so a crash mid-
//! flush never leaves a half-written file.
//!
//! All read-modify-write on [`SESSIONS_FILE`] from this process must hold
//! [`WorkbenchSessionsFileLock`] (via [`tauri::State`]) so concurrent
//! `invoke` calls cannot interleave and corrupt JSON.
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Manager};

const STATE_FILE: &str = "workbench.json";
const SESSIONS_FILE: &str = "sessions.json";
const NOTIFICATIONS_FILE: &str = "notifications.json";

/// Serialises every `sessions.json` load / update from this process so
/// overlapping Tauri commands cannot clobber each other's read-modify-write.
pub struct WorkbenchSessionsFileLock(pub Mutex<()>);

impl Default for WorkbenchSessionsFileLock {
    fn default() -> Self {
        Self(Mutex::new(()))
    }
}

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

fn notifications_path_impl(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(NOTIFICATIONS_FILE))
}

fn load_notifications_document(target: &Path) -> Result<Value, String> {
    let raw = match fs::read_to_string(target) {
        Ok(s) if s.trim().is_empty() => return Ok(json!({ "version": 1, "terminals": {} })),
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({ "version": 1, "terminals": {} }));
        }
        Err(e) => return Err(format!("read {}: {e}", target.display())),
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(v) => Ok(v),
        Err(_) => {
            atomic_write_json(target, &json!({ "version": 1, "terminals": {} }))?;
            Ok(json!({ "version": 1, "terminals": {} }))
        }
    }
}

fn terminal_unread_count(v: &Value) -> u32 {
    v.get("unread")
        .and_then(|x| x.as_u64())
        .or_else(|| {
            v.get("unread")
                .and_then(|x| x.as_i64())
                .filter(|&n| n >= 0)
                .map(|n| n as u64)
        })
        .unwrap_or(0) as u32
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("mkdir {}: {e}", path.display()))
}

/// Session ids stored in `sessions.json` must not contain path separators
/// or control characters so they cannot escape into filesystem paths or
/// shell injection when resumed.
fn is_safe_storage_session_id(id: &str) -> bool {
    let id = id.trim();
    if id.is_empty() || id.len() > 512 {
        return false;
    }
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return false;
    }
    !id.chars().any(|c| c.is_control())
}

fn validate_terminal_entry(v: &Value) -> bool {
    let Some(obj) = v.as_object() else {
        return false;
    };
    let agent = obj
        .get("agent")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim();
    let cwd = obj.get("cwd").and_then(|x| x.as_str()).unwrap_or("");
    let session_id = obj.get("session_id").and_then(|x| x.as_str()).unwrap_or("");
    if !is_safe_storage_session_id(session_id) {
        return false;
    }
    if cwd.len() > 8192 || cwd.chars().any(|c| c.is_control()) {
        return false;
    }
    matches!(
        agent,
        "" | "claude" | "codex" | "gemini" | "opencode" | "cursor"
    )
}

fn atomic_write_json(path: &Path, value: &Value) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let body =
        serde_json::to_string_pretty(value).map_err(|e| format!("serialize sessions: {e}"))?;
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
    Ok(())
}

/// If `sessions.json` is corrupt, back it up and replace with an empty shell.
fn recover_corrupt_sessions(target: &Path) -> Result<(), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let bak = target.with_file_name(format!("sessions.json.corrupt-{stamp}.bak"));
    let _ = fs::copy(target, &bak);
    atomic_write_json(target, &json!({ "terminals": {} }))
}

fn load_sessions_document(target: &Path) -> Result<Value, String> {
    let raw = match fs::read_to_string(target) {
        Ok(s) if s.trim().is_empty() => return Ok(json!({ "terminals": {} })),
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({ "terminals": {} }));
        }
        Err(e) => return Err(format!("read {}: {e}", target.display())),
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(v) => Ok(v),
        Err(_) => {
            recover_corrupt_sessions(target)?;
            Ok(json!({ "terminals": {} }))
        }
    }
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
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
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
pub fn workbench_load_sessions(
    app: AppHandle,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<Option<String>, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = sessions_path_impl(&app)?;
    match fs::read_to_string(&target) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read {}: {e}", target.display())),
        Ok(s) if s.trim().is_empty() => Ok(None),
        Ok(s) => {
            let doc: Value = match serde_json::from_str(&s) {
                Ok(v) => v,
                Err(_) => {
                    recover_corrupt_sessions(&target)?;
                    json!({ "terminals": {} })
                }
            };
            let body = serde_json::to_string(&doc).map_err(|e| format!("serialize: {e}"))?;
            Ok(Some(body))
        }
    }
}

/// Drop every `terminals.*` entry whose key starts with `prefix`. Used
/// when a workspace or terminal slot is closed in the UI, to keep
/// `sessions.json` from accumulating stale references that point at
/// agent sessions no slot will ever resume.
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
    if id.is_empty() || !is_safe_storage_session_id(id) {
        return false;
    }
    match probe.agent.as_str() {
        "claude" => claude_session_path(&home, &probe.cwd, id).is_file(),
        "codex" => codex_session_present(&home, id),
        "gemini" => gemini_session_present(&home, id),
        "opencode" | "cursor" => true,
        _ => false,
    }
}

/// `~/.claude/projects/<cwd-with-slashes-replaced-by-dashes>/<id>.jsonl`.
fn claude_session_path(home: &Path, cwd: &str, session_id: &str) -> PathBuf {
    let encoded = cwd.trim_end_matches('/').replace('/', "-");
    home.join(".claude")
        .join("projects")
        .join(encoded)
        .join(format!("{session_id}.jsonl"))
}

fn gemini_session_present(home: &Path, session_id: &str) -> bool {
    home.join(".gemini")
        .join("sessions")
        .join(session_id)
        .join("transcript.json")
        .is_file()
}

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
pub fn workbench_drop_sessions(
    app: AppHandle,
    prefix: String,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<u32, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    if prefix.is_empty() {
        return Ok(0);
    }
    let target = sessions_path_impl(&app)?;
    let mut state = load_sessions_document(&target)?;
    let removed = {
        let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) else {
            state["terminals"] = json!({});
            return atomic_write_json(&target, &state).map(|()| 0);
        };
        let before = terminals.len();
        terminals.retain(|k, _| !k.starts_with(&prefix));
        (before - terminals.len()) as u32
    };
    if removed == 0 {
        return Ok(0);
    }
    atomic_write_json(&target, &state)?;
    Ok(removed)
}

/// Returns a JSON object string `{"<terminalKey>": {...}, ...}` for every
/// `terminals` entry whose key starts with `prefix`, and removes those keys
/// from `sessions.json` in the same locked operation.
#[tauri::command]
pub fn workbench_extract_sessions_prefix(
    app: AppHandle,
    prefix: String,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<String, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    if prefix.is_empty() {
        return Ok("{}".into());
    }
    let target = sessions_path_impl(&app)?;
    let mut state = load_sessions_document(&target)?;
    let mut extracted = Map::new();
    if let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) {
        let keys: Vec<String> = terminals
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .cloned()
            .collect();
        for k in keys {
            if let Some(v) = terminals.remove(&k) {
                if validate_terminal_entry(&v) {
                    extracted.insert(k, v);
                }
            }
        }
    }
    atomic_write_json(&target, &state)?;
    let out = Value::Object(extracted);
    serde_json::to_string(&out).map_err(|e| e.to_string())
}

/// Merges validated terminal entries from `terminals_json` into
/// `sessions.json`, rewriting each key's leading `old_workspace_id:` to
/// `new_workspace_id:`.
#[tauri::command]
pub fn workbench_merge_sessions_workspace(
    app: AppHandle,
    old_workspace_id: u64,
    new_workspace_id: u64,
    terminals_json: String,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let parsed: Value = serde_json::from_str(terminals_json.trim())
        .map_err(|e| format!("merge terminals_json: {e}"))?;
    let Some(map) = parsed.as_object() else {
        return Err("terminals_json must be a JSON object".into());
    };
    let old_prefix = format!("{old_workspace_id}:");
    let new_prefix = format!("{new_workspace_id}:");
    let mut to_merge: BTreeMap<String, Value> = BTreeMap::new();
    for (k, v) in map {
        if !k.starts_with(&old_prefix) {
            continue;
        }
        if !validate_terminal_entry(v) {
            continue;
        }
        let suffix = k.strip_prefix(&old_prefix).unwrap_or("");
        let new_key = format!("{new_prefix}{suffix}");
        to_merge.insert(new_key, v.clone());
    }
    let target = sessions_path_impl(&app)?;
    let mut state = load_sessions_document(&target)?;
    let terminals = state
        .get_mut("terminals")
        .and_then(|t| t.as_object_mut())
        .ok_or_else(|| "sessions.json missing terminals object".to_string())?;
    for (k, v) in to_merge {
        terminals.insert(k, v);
    }
    atomic_write_json(&target, &state)
}

/// Absolute path for agent notify hooks (`BLX_NOTIFICATIONS_PATH`).
#[tauri::command]
pub fn workbench_notifications_path(app: AppHandle) -> Result<String, String> {
    Ok(notifications_path_impl(&app)?.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalNotification {
    pub unread: u32,
    pub agent: Option<String>,
    pub updated_at: Option<String>,
}

/// Per-terminal unread counts written by agent Stop/stop hooks.
#[tauri::command]
pub fn workbench_load_notifications(
    app: AppHandle,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<std::collections::HashMap<String, TerminalNotification>, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = notifications_path_impl(&app)?;
    let state = load_notifications_document(&target)?;
    let mut out = std::collections::HashMap::new();
    if let Some(terminals) = state.get("terminals").and_then(|t| t.as_object()) {
        for (k, v) in terminals {
            let unread = terminal_unread_count(v);
            if unread == 0 {
                continue;
            }
            let agent = v
                .get("agent")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let updated_at = v
                .get("updated_at")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            out.insert(
                k.clone(),
                TerminalNotification {
                    unread,
                    agent,
                    updated_at,
                },
            );
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearTerminalNotificationsArgs {
    pub terminal_key: String,
}

/// Clears unread for one terminal slot (focus-only UX in the workbench).
#[tauri::command]
pub fn workbench_clear_terminal_notifications(
    app: AppHandle,
    args: ClearTerminalNotificationsArgs,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let key = args.terminal_key.trim();
    if key.is_empty() {
        return Ok(());
    }
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    if let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) {
        if let Some(entry) = terminals.get_mut(key) {
            if let Some(obj) = entry.as_object_mut() {
                obj.insert("unread".into(), json!(0));
            }
        }
    }
    atomic_write_json(&target, &state)
}
