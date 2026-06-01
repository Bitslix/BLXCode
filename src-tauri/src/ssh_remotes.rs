//! SSH remote connection presets. The non-secret metadata (label/host/port/
//! user/auth-kind/key-path/resume) lives in a backend-owned
//! `remote_connections.json` so it can be resolved server-side at spawn time
//! without round-tripping the frontend. Secrets (password / key passphrase)
//! are delegated to `ssh_secrets` (OS keychain), never written here.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

use crate::pty_host::{PtyManager, RemoteAuthMode, RemoteSpawnSpec, ResumeMode};
use crate::ssh_exec::{RemoteExecManager, EXEC_TIMEOUT_MS};
use crate::ssh_secrets::{self, SshSecretKind};

const STORE_FILE: &str = "remote_connections.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteAuthKind {
    Password,
    Key,
    Agent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteResume {
    Tmux,
    /// Default: no remote dependency, reconnect starts a fresh shell.
    #[default]
    KeepaliveOnly,
}

/// Persisted preset (no secrets).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnection {
    pub id: String,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_kind: RemoteAuthKind,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default)]
    pub resume: RemoteResume,
    #[serde(default)]
    pub default_remote_dir: Option<String>,
}

/// What the UI renders: the preset plus secret-presence flags (so it can show
/// "stored" / "not set" without ever reading the secret back).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionView {
    pub connection: RemoteConnection,
    pub has_password: bool,
    pub has_passphrase: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct RemoteStore {
    #[serde(default)]
    connections: Vec<RemoteConnection>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSaveRequest {
    pub connection: RemoteConnection,
    /// `Some` to set/replace, `Some("")` to clear, `None` to leave unchanged.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteTestRequest {
    pub connection: RemoteConnection,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub passphrase: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirEntry {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirListing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<RemoteDirEntry>,
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(STORE_FILE))
}

fn load_store(app: &AppHandle) -> Result<RemoteStore, String> {
    let path = store_path(app)?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(RemoteStore::default()),
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(RemoteStore::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn save_store(app: &AppHandle, store: &RemoteStore) -> Result<(), String> {
    let path = store_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let body =
        serde_json::to_string_pretty(store).map_err(|e| format!("serialize remotes: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))
}

fn to_view(app: &AppHandle, connection: RemoteConnection) -> RemoteConnectionView {
    let has_password = ssh_secrets::has_secret(app, &connection.id, SshSecretKind::Password);
    let has_passphrase = ssh_secrets::has_secret(app, &connection.id, SshSecretKind::Passphrase);
    RemoteConnectionView {
        connection,
        has_password,
        has_passphrase,
    }
}

fn sanitize(mut conn: RemoteConnection) -> Result<RemoteConnection, String> {
    conn.label = conn.label.trim().to_string();
    conn.host = conn.host.trim().to_string();
    conn.username = conn.username.trim().to_string();
    conn.key_path = conn
        .key_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    conn.default_remote_dir = conn
        .default_remote_dir
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    if conn.label.is_empty() {
        return Err("connection name must not be empty".into());
    }
    if conn.host.is_empty() {
        return Err("host must not be empty".into());
    }
    if conn.username.is_empty() {
        return Err("username must not be empty".into());
    }
    if conn.port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    if conn.auth_kind == RemoteAuthKind::Key && conn.key_path.is_none() {
        return Err("key authentication requires a private key file".into());
    }
    Ok(conn)
}

/// Build a spawn spec from a preset + provided/stored secrets.
fn build_spec(
    conn: &RemoteConnection,
    password: Option<String>,
    passphrase: Option<String>,
    terminal_key: String,
) -> RemoteSpawnSpec {
    let auth = match conn.auth_kind {
        RemoteAuthKind::Password => RemoteAuthMode::Password(password.unwrap_or_default()),
        RemoteAuthKind::Key => RemoteAuthMode::Key {
            key_path: conn.key_path.clone().unwrap_or_default(),
            passphrase: passphrase.filter(|p| !p.is_empty()),
        },
        RemoteAuthKind::Agent => RemoteAuthMode::Agent,
    };
    let resume = match conn.resume {
        RemoteResume::Tmux => ResumeMode::Tmux,
        RemoteResume::KeepaliveOnly => ResumeMode::KeepaliveOnly,
    };
    RemoteSpawnSpec {
        host: conn.host.clone(),
        port: conn.port,
        username: conn.username.clone(),
        auth,
        resume,
        remote_dir: conn.default_remote_dir.clone(),
        terminal_key,
    }
}

/// Resolve a stored preset by id and build a spawn spec, pulling secrets from
/// the keychain. Used by the `pty_spawn_remote` command so secrets stay in Rust.
pub fn resolve_spec(
    app: &AppHandle,
    connection_id: &str,
    terminal_key: String,
) -> Result<RemoteSpawnSpec, String> {
    let store = load_store(app)?;
    let conn = store
        .connections
        .into_iter()
        .find(|c| c.id == connection_id)
        .ok_or_else(|| format!("unknown remote connection: {connection_id}"))?;
    let password = if conn.auth_kind == RemoteAuthKind::Password {
        ssh_secrets::get_secret(app, &conn.id, SshSecretKind::Password)?
    } else {
        None
    };
    let passphrase = if conn.auth_kind == RemoteAuthKind::Key {
        ssh_secrets::get_secret(app, &conn.id, SshSecretKind::Passphrase)?
    } else {
        None
    };
    Ok(build_spec(&conn, password, passphrase, terminal_key))
}

#[tauri::command]
pub fn ssh_remotes_list(app: AppHandle) -> Result<Vec<RemoteConnectionView>, String> {
    let store = load_store(&app)?;
    Ok(store
        .connections
        .into_iter()
        .map(|c| to_view(&app, c))
        .collect())
}

#[tauri::command]
pub fn ssh_remote_save(
    app: AppHandle,
    payload: RemoteSaveRequest,
) -> Result<RemoteConnectionView, String> {
    let mut conn = sanitize(payload.connection)?;
    if conn.id.trim().is_empty() {
        conn.id = uuid::Uuid::new_v4().simple().to_string();
    }
    let mut store = load_store(&app)?;
    if let Some(slot) = store.connections.iter_mut().find(|c| c.id == conn.id) {
        *slot = conn.clone();
    } else {
        store.connections.push(conn.clone());
    }
    save_store(&app, &store)?;

    // Secret writes: None = leave, Some = set/clear (clearing handled inside).
    if let Some(pw) = payload.password {
        ssh_secrets::set_secret(&app, &conn.id, SshSecretKind::Password, &pw)?;
    }
    if let Some(pp) = payload.passphrase {
        ssh_secrets::set_secret(&app, &conn.id, SshSecretKind::Passphrase, &pp)?;
    }
    Ok(to_view(&app, conn))
}

#[tauri::command]
pub fn ssh_remote_delete(app: AppHandle, id: String) -> Result<(), String> {
    let mut store = load_store(&app)?;
    store.connections.retain(|c| c.id != id);
    save_store(&app, &store)?;
    ssh_secrets::delete_all(&app, &id)
}

/// Test a connection end-to-end through the real spawn path. Uses provided
/// secrets if present, otherwise falls back to stored keychain secrets.
#[tauri::command]
pub fn ssh_remote_test(
    app: AppHandle,
    manager: State<'_, PtyManager>,
    payload: RemoteTestRequest,
) -> Result<(), String> {
    let conn = sanitize(payload.connection)?;
    let password = match (conn.auth_kind, payload.password) {
        (RemoteAuthKind::Password, Some(pw)) if !pw.is_empty() => Some(pw),
        (RemoteAuthKind::Password, _) => {
            ssh_secrets::get_secret(&app, &conn.id, SshSecretKind::Password)?
        }
        _ => None,
    };
    let passphrase = match (conn.auth_kind, payload.passphrase) {
        (RemoteAuthKind::Key, Some(pp)) if !pp.is_empty() => Some(pp),
        (RemoteAuthKind::Key, _) => {
            ssh_secrets::get_secret(&app, &conn.id, SshSecretKind::Passphrase)?
        }
        _ => None,
    };
    let spec = build_spec(&conn, password, passphrase, format!("test:{}", conn.id));
    manager.run_remote_probe(spec, 15_000)
}

#[tauri::command]
pub fn ssh_remote_list_dirs(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    connection_id: String,
    path: String,
) -> Result<RemoteDirListing, String> {
    if connection_id.trim().is_empty() {
        return Err("save the remote connection before browsing directories".into());
    }
    let target = path.trim();
    let cd = if target.is_empty() {
        "cd".to_string()
    } else {
        format!("cd -- {}", sh_quote_local(target))
    };
    let command = format!(
        "{cd} && printf '__BLX_PWD__%s\\n' \"$PWD\" && \
         {{ for d in ./* ./.[!.]* ./..?*; do [ -d \"$d\" ] || continue; b=${{d#./}}; printf '%s\\n' \"$b\"; done | sort; }}"
    );
    let out = exec.run_text(&app, &pty, &connection_id, &command, EXEC_TIMEOUT_MS)?;
    parse_remote_dir_listing(&out)
}

fn parse_remote_dir_listing(raw: &str) -> Result<RemoteDirListing, String> {
    let mut lines = raw.lines();
    let Some(first) = lines.next() else {
        return Err("remote directory listing returned no output".into());
    };
    let Some(path) = first.strip_prefix("__BLX_PWD__") else {
        return Err("remote directory listing returned an unexpected response".into());
    };
    let path = normalize_remote_dir(path);
    let parent = remote_parent(&path);
    let entries = lines
        .filter(|line| !line.trim().is_empty())
        .filter(|line| *line != "." && *line != "..")
        .map(|name| {
            let name = name.to_string();
            let path = remote_join(&path, &name);
            RemoteDirEntry { name, path }
        })
        .collect();
    Ok(RemoteDirListing {
        path,
        parent,
        entries,
    })
}

fn normalize_remote_dir(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".into()
    } else if trimmed.len() > 1 {
        trimmed.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}

fn remote_parent(path: &str) -> Option<String> {
    if path == "/" {
        return None;
    }
    let trimmed = path.trim_end_matches('/');
    let idx = trimmed.rfind('/')?;
    if idx == 0 {
        Some("/".into())
    } else {
        Some(trimmed[..idx].to_string())
    }
}

fn remote_join(base: &str, name: &str) -> String {
    if base == "/" {
        format!("/{name}")
    } else {
        format!("{}/{}", base.trim_end_matches('/'), name)
    }
}

fn sh_quote_local(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
