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
const USAGE_FILE: &str = "usage.json";
const AGENT_NOTIFICATIONS_MAX: usize = 200;

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

fn usage_path_impl(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(USAGE_FILE))
}

fn load_notifications_document(target: &Path) -> Result<Value, String> {
    let raw = match fs::read_to_string(target) {
        Ok(s) if s.trim().is_empty() => {
            return Ok(json!({ "version": 2, "terminals": {}, "agentNotifications": [] }));
        }
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(json!({ "version": 2, "terminals": {}, "agentNotifications": [] }));
        }
        Err(e) => return Err(format!("read {}: {e}", target.display())),
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(v) => Ok(normalize_notifications_document(v)),
        Err(_) => {
            let empty = json!({ "version": 2, "terminals": {}, "agentNotifications": [] });
            atomic_write_json(target, &empty)?;
            Ok(empty)
        }
    }
}

fn normalize_notifications_document(mut v: Value) -> Value {
    if !v.is_object() {
        return json!({ "version": 2, "terminals": {}, "agentNotifications": [] });
    }
    let obj = v.as_object_mut().expect("checked object");
    obj.insert("version".into(), json!(2));
    if !obj.get("terminals").map(|t| t.is_object()).unwrap_or(false) {
        obj.insert("terminals".into(), json!({}));
    }
    if !obj
        .get("agentNotifications")
        .map(|n| n.is_array())
        .unwrap_or(false)
    {
        obj.insert("agentNotifications".into(), json!([]));
    }
    v
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

/// Returns the absolute path used by terminal-agent usage capture helpers.
#[tauri::command]
pub fn workbench_usage_path(app: AppHandle) -> Result<String, String> {
    Ok(usage_path_impl(&app)?.to_string_lossy().into_owned())
}

/// Read the latest cached usage snapshot for one terminal, if a provider hook
/// has written one. Currently used by Claude's status-line capture wrapper.
#[tauri::command]
pub fn workbench_load_usage_snapshot(
    app: AppHandle,
    terminal_key: String,
) -> Result<Option<String>, String> {
    let key = terminal_key.trim();
    if key.is_empty()
        || key.len() > 512
        || key.contains('/')
        || key.contains('\\')
        || key.chars().any(char::is_control)
    {
        return Ok(None);
    }
    let target = usage_path_impl(&app)?;
    let raw = match fs::read_to_string(&target) {
        Ok(s) if s.trim().is_empty() => return Ok(None),
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", target.display())),
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => {
            atomic_write_json(&target, &json!({ "version": 1, "terminals": {} }))?;
            return Ok(None);
        }
    };
    let Some(snapshot) = parsed.get("terminals").and_then(|t| t.get(key)) else {
        return Ok(None);
    };
    serde_json::to_string(snapshot)
        .map(Some)
        .map_err(|e| format!("serialize usage snapshot: {e}"))
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSessionLookup {
    pub agent: String,
    pub cwd: String,
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
        "claude" => {
            claude_session_path(&home, &probe.cwd, id).is_file()
                || claude_session_present(&home, id)
        }
        "codex" => codex_session_present(&home, id),
        "gemini" => gemini_session_present(&home, id),
        "opencode" | "cursor" => true,
        _ => false,
    }
}

#[tauri::command]
pub fn agent_latest_session_id(app: AppHandle, probe: AgentSessionLookup) -> Option<String> {
    let Ok(home) = app.path().home_dir() else {
        return None;
    };
    match probe.agent.as_str() {
        "claude" => latest_claude_session_id(&home, &probe.cwd),
        _ => None,
    }
}

/// `~/.claude/projects/<cwd-with-non-alnum-replaced-by-dashes>/<id>.jsonl`.
fn claude_session_path(home: &Path, cwd: &str, session_id: &str) -> PathBuf {
    claude_project_dir(home, cwd).join(format!("{session_id}.jsonl"))
}

fn claude_project_dir(home: &Path, cwd: &str) -> PathBuf {
    let encoded: String = cwd
        .trim_end_matches(['/', '\\'])
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    home.join(".claude").join("projects").join(encoded)
}

fn latest_claude_session_id(home: &Path, cwd: &str) -> Option<String> {
    let dir = claude_project_dir(home, cwd);
    let entries = fs::read_dir(dir).ok()?;
    let mut newest: Option<(SystemTime, String)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !is_safe_storage_session_id(stem) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(UNIX_EPOCH);
        if newest
            .as_ref()
            .map(|(current, _)| modified > *current)
            .unwrap_or(true)
        {
            newest = Some((modified, stem.to_string()));
        }
    }
    newest.map(|(_, session_id)| session_id)
}

fn claude_session_present(home: &Path, session_id: &str) -> bool {
    let root = home.join(".claude").join("projects");
    if !root.is_dir() {
        return false;
    }
    let Ok(projects) = fs::read_dir(root) else {
        return false;
    };
    let file_name = format!("{session_id}.jsonl");
    for project in projects.flatten() {
        let Ok(ft) = project.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        if project.path().join(&file_name).is_file() {
            return true;
        }
    }
    false
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

/// Drop `sessions.json` terminal entries whose key has no matching slot
/// in any open workspace. Mirrors `workbench_prune_notifications`. Used
/// to wipe legacy numeric-prefix keys after the storage_key (UUID)
/// migration, and to keep the file from accumulating ghost session
/// resume-ids forever as workspaces are opened and closed.
#[tauri::command]
pub fn workbench_prune_sessions(
    app: AppHandle,
    valid_terminal_keys: Vec<String>,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let valid: std::collections::HashSet<String> = valid_terminal_keys.into_iter().collect();
    let target = sessions_path_impl(&app)?;
    let mut state = load_sessions_document(&target)?;
    let mut changed = false;
    if let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) {
        let stale: Vec<String> = terminals
            .keys()
            .filter(|k| !valid.contains(k.as_str()))
            .cloned()
            .collect();
        if !stale.is_empty() {
            for key in stale {
                terminals.remove(&key);
            }
            changed = true;
        }
    }
    if changed {
        atomic_write_json(&target, &state)
    } else {
        Ok(())
    }
}

/// Merges validated terminal entries from `terminals_json` into
/// `sessions.json`, rewriting each key's leading `old_workspace_key:` to
/// `new_workspace_key:`. Both keys are the workspace's stable UUID
/// `storage_key` (used as the prefix of every `terminal_key`); when
/// `old == new` the function reinjects sessions under their original
/// keys without rewriting.
#[tauri::command]
pub fn workbench_merge_sessions_workspace(
    app: AppHandle,
    old_workspace_key: String,
    new_workspace_key: String,
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
    if old_workspace_key.is_empty() || new_workspace_key.is_empty() {
        return Err("workspace keys must be non-empty".into());
    }
    let old_prefix = format!("{old_workspace_key}:");
    let new_prefix = format!("{new_workspace_key}:");
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
    Ok(notifications_path_impl(&app)?
        .to_string_lossy()
        .into_owned())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalNotification {
    pub unread: u32,
    pub agent: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentNotification {
    pub id: String,
    pub kind: String,
    pub severity: String,
    pub title: String,
    pub body: Option<String>,
    pub source: Option<String>,
    pub target: Option<Value>,
    pub read: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub sent_at: Option<i64>,
    pub dedupe_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentNotificationInput {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub kind: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub target: Option<Value>,
    #[serde(default)]
    pub dedupe_key: Option<String>,
    #[serde(default)]
    pub read: Option<bool>,
    #[serde(default)]
    pub sent: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentNotificationPatch {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub target: Option<Value>,
    #[serde(default)]
    pub read: Option<bool>,
    #[serde(default)]
    pub sent: Option<bool>,
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn sanitize_notification_token(raw: &str, fallback: &str) -> String {
    let s = raw.trim();
    if s.is_empty()
        || s.len() > 64
        || !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        fallback.to_string()
    } else {
        s.to_string()
    }
}

fn sanitize_notification_text(raw: &str, max: usize) -> String {
    raw.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .take(max)
        .collect::<String>()
        .trim()
        .to_string()
}

fn agent_notifications_from_state(state: &Value) -> Vec<AgentNotification> {
    let mut items: Vec<AgentNotification> = state
        .get("agentNotifications")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect();
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    items
}

fn store_agent_notifications(
    state: &mut Value,
    mut items: Vec<AgentNotification>,
) -> Result<(), String> {
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    if items.len() > AGENT_NOTIFICATIONS_MAX {
        let mut kept = Vec::with_capacity(AGENT_NOTIFICATIONS_MAX);
        let mut unread_overflow = Vec::new();
        for item in items {
            if kept.len() < AGENT_NOTIFICATIONS_MAX {
                kept.push(item);
            } else if !item.read {
                unread_overflow.push(item);
            }
        }
        kept.extend(unread_overflow);
        kept.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        kept.truncate(AGENT_NOTIFICATIONS_MAX);
        items = kept;
    }
    let arr = serde_json::to_value(items).map_err(|e| format!("serialize notifications: {e}"))?;
    let obj = state
        .as_object_mut()
        .ok_or_else(|| "notifications document is not an object".to_string())?;
    obj.insert("version".into(), json!(2));
    obj.insert("agentNotifications".into(), arr);
    Ok(())
}

fn build_agent_notification(
    input: AgentNotificationInput,
    existing: Option<&AgentNotification>,
) -> AgentNotification {
    let now = now_millis();
    let title = sanitize_notification_text(&input.title, 160);
    AgentNotification {
        id: input
            .id
            .filter(|s| !s.trim().is_empty())
            .or_else(|| existing.map(|n| n.id.clone()))
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        kind: sanitize_notification_token(&input.kind, "info"),
        severity: sanitize_notification_token(input.severity.as_deref().unwrap_or("info"), "info"),
        title: if title.is_empty() {
            "BLXCode Agent".into()
        } else {
            title
        },
        body: input
            .body
            .map(|s| sanitize_notification_text(&s, 1000))
            .filter(|s| !s.is_empty()),
        source: input
            .source
            .map(|s| sanitize_notification_text(&s, 120))
            .filter(|s| !s.is_empty()),
        target: input.target,
        read: input.read.unwrap_or(false),
        created_at: existing.map(|n| n.created_at).unwrap_or(now),
        updated_at: now,
        sent_at: input.sent.unwrap_or(false).then_some(now),
        dedupe_key: input
            .dedupe_key
            .map(|s| sanitize_notification_text(&s, 180))
            .filter(|s| !s.is_empty()),
    }
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

/// Clears unread for one terminal slot (focus-only UX in the workbench).
#[tauri::command]
pub fn workbench_clear_terminal_notifications(
    app: AppHandle,
    terminal_key: String,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let key = terminal_key.trim();
    if key.is_empty() {
        return Ok(());
    }
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    if let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) {
        terminals.remove(key);
    }
    atomic_write_json(&target, &state)
}

#[tauri::command]
pub fn workbench_list_agent_notifications(
    app: AppHandle,
    include_read: Option<bool>,
    limit: Option<usize>,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<Vec<AgentNotification>, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = notifications_path_impl(&app)?;
    let state = load_notifications_document(&target)?;
    let mut items = agent_notifications_from_state(&state);
    if !include_read.unwrap_or(true) {
        items.retain(|n| !n.read);
    }
    items.truncate(
        limit
            .unwrap_or(AGENT_NOTIFICATIONS_MAX)
            .clamp(1, AGENT_NOTIFICATIONS_MAX),
    );
    Ok(items)
}

#[tauri::command]
pub fn workbench_upsert_agent_notification(
    app: AppHandle,
    input: AgentNotificationInput,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<AgentNotification, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    let mut items = agent_notifications_from_state(&state);
    let existing_idx = input
        .id
        .as_ref()
        .and_then(|id| items.iter().position(|n| &n.id == id))
        .or_else(|| {
            input.dedupe_key.as_ref().and_then(|key| {
                items
                    .iter()
                    .position(|n| n.dedupe_key.as_ref() == Some(key))
            })
        });
    let existing = existing_idx.and_then(|idx| items.get(idx));
    let next = build_agent_notification(input, existing);
    if let Some(idx) = existing_idx {
        items[idx] = next.clone();
    } else {
        items.push(next.clone());
    }
    store_agent_notifications(&mut state, items)?;
    atomic_write_json(&target, &state)?;
    Ok(next)
}

#[tauri::command]
pub fn workbench_update_agent_notification(
    app: AppHandle,
    patch: AgentNotificationPatch,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<AgentNotification, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    let mut items = agent_notifications_from_state(&state);
    let Some(item) = items.iter_mut().find(|n| n.id == patch.id) else {
        return Err(format!("notification not found: {}", patch.id));
    };
    if let Some(title) = patch.title {
        let title = sanitize_notification_text(&title, 160);
        if !title.is_empty() {
            item.title = title;
        }
    }
    if let Some(body) = patch.body {
        item.body = Some(sanitize_notification_text(&body, 1000)).filter(|s| !s.is_empty());
    }
    if let Some(kind) = patch.kind {
        item.kind = sanitize_notification_token(&kind, &item.kind);
    }
    if let Some(severity) = patch.severity {
        item.severity = sanitize_notification_token(&severity, &item.severity);
    }
    if let Some(source) = patch.source {
        item.source = Some(sanitize_notification_text(&source, 120)).filter(|s| !s.is_empty());
    }
    if patch.target.is_some() {
        item.target = patch.target;
    }
    if let Some(read) = patch.read {
        item.read = read;
    }
    if patch.sent.unwrap_or(false) {
        item.sent_at = Some(now_millis());
    }
    item.updated_at = now_millis();
    let out = item.clone();
    store_agent_notifications(&mut state, items)?;
    atomic_write_json(&target, &state)?;
    Ok(out)
}

#[tauri::command]
pub fn workbench_remove_agent_notification(
    app: AppHandle,
    id: String,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    let mut items = agent_notifications_from_state(&state);
    items.retain(|n| n.id != id);
    store_agent_notifications(&mut state, items)?;
    atomic_write_json(&target, &state)
}

#[tauri::command]
pub fn workbench_mark_agent_notifications_read(
    app: AppHandle,
    id: Option<String>,
    all: Option<bool>,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<Vec<AgentNotification>, String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    let mut items = agent_notifications_from_state(&state);
    let now = now_millis();
    if all.unwrap_or(false) {
        for item in &mut items {
            item.read = true;
            item.updated_at = now;
        }
    } else if let Some(id) = id {
        let Some(item) = items.iter_mut().find(|n| n.id == id) else {
            return Err(format!("notification not found: {id}"));
        };
        item.read = true;
        item.updated_at = now;
    }
    store_agent_notifications(&mut state, items.clone())?;
    atomic_write_json(&target, &state)?;
    Ok(items)
}

/// Rewrite terminal-key entries in both `sessions.json` and
/// `notifications.json` from each `old` key to its paired `new` key in
/// one locked operation. Used by cross-workspace terminal slot moves so
/// the resumed agent CLI session and the unread badge follow the slot
/// across workspaces without losing on-disk state. No-op for pairs whose
/// `old` key isn't present, and pairs whose `new` key is already taken
/// are skipped (we never silently overwrite an unrelated entry).
#[tauri::command]
pub fn workbench_rewrite_terminal_keys(
    app: AppHandle,
    pairs: Vec<(String, String)>,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    if pairs.is_empty() {
        return Ok(());
    }

    // sessions.json
    let sessions_target = sessions_path_impl(&app)?;
    let mut sessions_state = load_sessions_document(&sessions_target)?;
    let mut sessions_changed = false;
    if let Some(terminals) = sessions_state
        .get_mut("terminals")
        .and_then(|t| t.as_object_mut())
    {
        for (old_key, new_key) in &pairs {
            if old_key == new_key || old_key.is_empty() || new_key.is_empty() {
                continue;
            }
            if terminals.contains_key(new_key) {
                continue;
            }
            if let Some(entry) = terminals.remove(old_key) {
                terminals.insert(new_key.clone(), entry);
                sessions_changed = true;
            }
        }
    }
    if sessions_changed {
        atomic_write_json(&sessions_target, &sessions_state)?;
    }

    // notifications.json
    let notifications_target = notifications_path_impl(&app)?;
    let mut notifications_state = load_notifications_document(&notifications_target)?;
    let mut notifications_changed = false;
    if let Some(terminals) = notifications_state
        .get_mut("terminals")
        .and_then(|t| t.as_object_mut())
    {
        for (old_key, new_key) in &pairs {
            if old_key == new_key || old_key.is_empty() || new_key.is_empty() {
                continue;
            }
            if terminals.contains_key(new_key) {
                continue;
            }
            if let Some(entry) = terminals.remove(old_key) {
                terminals.insert(new_key.clone(), entry);
                notifications_changed = true;
            }
        }
    }
    if notifications_changed {
        atomic_write_json(&notifications_target, &notifications_state)?;
    }
    Ok(())
}

/// Drop notification entries whose terminal key no longer exists in any
/// open workspace. Called from the workbench auto-save effect after a
/// terminal slot is closed or a workspace is rebuilt; without it, stale
/// keys remain in `notifications.json` and surface as a generic
/// "unknown agent" badge in the sidebar.
#[tauri::command]
pub fn workbench_prune_notifications(
    app: AppHandle,
    valid_terminal_keys: Vec<String>,
    lock: tauri::State<'_, WorkbenchSessionsFileLock>,
) -> Result<(), String> {
    let _guard = lock
        .0
        .lock()
        .map_err(|e| format!("sessions file lock poisoned: {e}"))?;
    let valid: std::collections::HashSet<String> = valid_terminal_keys.into_iter().collect();
    let target = notifications_path_impl(&app)?;
    let mut state = load_notifications_document(&target)?;
    let mut changed = false;
    if let Some(terminals) = state.get_mut("terminals").and_then(|t| t.as_object_mut()) {
        let stale: Vec<String> = terminals
            .keys()
            .filter(|k| !valid.contains(k.as_str()))
            .cloned()
            .collect();
        if !stale.is_empty() {
            for key in stale {
                terminals.remove(&key);
            }
            changed = true;
        }
    }
    if changed {
        atomic_write_json(&target, &state)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_json_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("blxcode-notifications-{label}-{nanos}.json"))
    }

    #[test]
    fn notification_v1_load_preserves_terminal_entries() {
        let path = temp_json_path("v1");
        fs::write(
            &path,
            r#"{"version":1,"terminals":{"ws:1:1":{"unread":2,"agent":"codex","updated_at":"now"}}}"#,
        )
        .unwrap();
        let state = load_notifications_document(&path).unwrap();
        assert_eq!(state.get("version").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(
            state
                .get("terminals")
                .and_then(|t| t.get("ws:1:1"))
                .and_then(terminal_unread_count_value),
            Some(2)
        );
        assert!(state
            .get("agentNotifications")
            .and_then(|v| v.as_array())
            .unwrap()
            .is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn agent_notification_dedupe_update_mark_and_remove() {
        let mut state = json!({ "version": 2, "terminals": {}, "agentNotifications": [] });
        let first = build_agent_notification(
            AgentNotificationInput {
                id: None,
                title: "Task done".into(),
                body: Some("One task completed".into()),
                kind: "task_completed".into(),
                severity: Some("success".into()),
                source: Some("agent".into()),
                target: None,
                dedupe_key: Some("task:one".into()),
                read: Some(false),
                sent: Some(true),
            },
            None,
        );
        store_agent_notifications(&mut state, vec![first.clone()]).unwrap();
        let mut items = agent_notifications_from_state(&state);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].dedupe_key.as_deref(), Some("task:one"));
        assert!(items[0].sent_at.is_some());

        let updated = build_agent_notification(
            AgentNotificationInput {
                id: None,
                title: "Task really done".into(),
                body: None,
                kind: "task_completed".into(),
                severity: Some("success".into()),
                source: None,
                target: None,
                dedupe_key: Some("task:one".into()),
                read: Some(false),
                sent: Some(false),
            },
            Some(&items[0]),
        );
        items[0] = updated.clone();
        store_agent_notifications(&mut state, items).unwrap();
        let mut items = agent_notifications_from_state(&state);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, first.id);
        assert_eq!(items[0].title, "Task really done");

        items[0].read = true;
        store_agent_notifications(&mut state, items.clone()).unwrap();
        assert!(agent_notifications_from_state(&state)[0].read);

        items.clear();
        store_agent_notifications(&mut state, items).unwrap();
        assert!(agent_notifications_from_state(&state).is_empty());
    }

    fn terminal_unread_count_value(v: &Value) -> Option<u32> {
        Some(terminal_unread_count(v))
    }
}
