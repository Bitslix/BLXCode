use crate::agent::{dispatch_user_turn, AgentEngineState, AgentEvent, UserTurn};
use crate::agent_settings::provider_status_json;
use crate::browser_host::BrowserHost;
use crate::pty_host::{path_nav_exec, PathNavResult, PtyManager};
use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn agent_submit_turn(
    app: AppHandle,
    turn: UserTurn,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Result<(), String> {
    dispatch_user_turn(&app, &agent, turn)
}

#[tauri::command]
pub fn agent_poll_events(max: usize, agent: State<'_, Arc<AgentEngineState>>) -> Vec<AgentEvent> {
    agent.drain(max.max(1).min(512))
}

#[tauri::command]
pub fn agent_abort(agent: State<'_, Arc<AgentEngineState>>) {
    agent.request_cancel();
}

#[tauri::command]
pub fn agent_clear_conversation(agent: State<'_, Arc<AgentEngineState>>) -> Result<(), String> {
    if agent.busy() {
        return Err("Agent ist noch beschäftigt. Bitte zuerst abbrechen oder warten.".into());
    }
    agent.clear_conversation();
    Ok(())
}

#[tauri::command]
pub fn agent_provider_status() -> serde_json::Value {
    provider_status_json()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultPayload {
    pub call_id: String,
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[tauri::command]
pub fn agent_submit_tool_result(
    payload: ToolResultPayload,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Result<(), String> {
    agent.deliver_client_tool_result(&payload.call_id, payload.ok, payload.message, payload.data)
}

/// Returns (and idempotently creates) the default sandbox directory under
/// the app data dir. Used as a guaranteed-non-empty fallback workspace
/// root so the agent always has a writable scope to play in.
#[tauri::command]
pub fn harness_ensure_default_sandbox(app: AppHandle) -> Result<String, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir unavailable: {e}"))?;
    let sandbox = base.join("sandbox");
    std::fs::create_dir_all(&sandbox)
        .map_err(|e| format!("create sandbox {}: {e}", sandbox.display()))?;
    Ok(sandbox.to_string_lossy().into_owned())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBoundsPayload {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub visible: bool,
}

#[tauri::command]
pub fn browser_sync_bounds(
    app: tauri::AppHandle,
    host: State<'_, BrowserHost>,
    active_tab_id: Option<u64>,
    rect: BrowserBoundsPayload,
    url_optional: Option<String>,
) -> Result<(), String> {
    host.sync_bounds(&app, active_tab_id, rect, url_optional.as_deref())
}

#[tauri::command]
pub fn browser_run_js(
    app: AppHandle,
    host: State<'_, BrowserHost>,
    tab_id: u64,
    script: String,
) -> Result<(), String> {
    host.eval_embedded(&app, tab_id, script)
}

#[tauri::command]
pub fn browser_navigate(
    app: tauri::AppHandle,
    host: State<'_, BrowserHost>,
    tab_id: u64,
    url: String,
) -> Result<(), String> {
    host.navigate(&app, tab_id, &url)
}

#[tauri::command]
pub fn browser_close_tab(
    app: tauri::AppHandle,
    host: State<'_, BrowserHost>,
    tab_id: u64,
) -> Result<(), String> {
    host.close_tab(&app, tab_id)
}

#[tauri::command]
pub fn browser_embedding_kind() -> &'static str {
    crate::browser_host::browser_embedding_kind_str()
}

/// HEAD-Probe an die URL, prüft `X-Frame-Options` und `Content-Security-Policy`
/// `frame-ancestors`. Liefert `false`, wenn das Embedding in einem `<iframe>`
/// vom Server verweigert wird (z. B. github.com mit `X-Frame-Options: DENY`).
/// Bei Netzwerkfehlern wird `true` (optimistisch) zurückgegeben — der iframe
/// versucht es selbst und das `error`-Event greift ggf.
#[tauri::command]
pub async fn browser_check_iframable(url: String) -> Result<bool, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }
    let parsed = url::Url::parse(trimmed).map_err(|e| format!("URL: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Ok(false);
    }

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(false)
        .timeout(std::time::Duration::from_secs(4))
        .user_agent("blxcode/0.1 (iframable-probe)")
        .build()
        .map_err(|e| e.to_string())?;

    // Manche Server (z. B. github.com) antworten auf HEAD anders als auf GET.
    // GET mit Range bringt die Header zurück ohne den ganzen Body zu ziehen.
    let resp = match client
        .get(parsed.clone())
        .header("Range", "bytes=0-0")
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Ok(true), // Netzwerkfehler — sei optimistisch.
    };

    let headers = resp.headers();

    if let Some(xfo) = headers.get("x-frame-options").and_then(|v| v.to_str().ok()) {
        let v = xfo.trim().to_ascii_lowercase();
        if v == "deny" || v == "sameorigin" || v.starts_with("allow-from") {
            return Ok(false);
        }
    }

    if let Some(csp) = headers
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
    {
        let lc = csp.to_ascii_lowercase();
        if let Some(idx) = lc.find("frame-ancestors") {
            let after = &lc[idx + "frame-ancestors".len()..];
            let directive = after.split(';').next().unwrap_or("").trim();
            // `frame-ancestors 'none'` oder `'self'` (Tauri-Origin ist nicht self) blockt uns.
            if directive.contains("'none'") || directive == "'self'" {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

#[tauri::command]
pub fn path_nav_exec_cmd(base: String, line: String) -> Result<PathNavResult, String> {
    path_nav_exec(base, line)
}

/// Sinnvoller Start-Pfad fürs Verzeichnis-Picker-UI: bevorzugt `$HOME`,
/// fällt auf das Prozess-Arbeitsverzeichnis zurück.
#[tauri::command]
pub fn default_cwd() -> Result<String, String> {
    if let Some(home) = std::env::var_os("HOME") {
        let s = home.to_string_lossy().into_owned();
        if !s.is_empty() {
            return Ok(s);
        }
    }
    if let Some(userprofile) = std::env::var_os("USERPROFILE") {
        let s = userprofile.to_string_lossy().into_owned();
        if !s.is_empty() {
            return Ok(s);
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryBrief {
    pub name: String,
    pub hidden: bool,
}

/// Listet alle Unterverzeichnisse von `path` (alphabetisch). Hidden-Flag ist
/// gesetzt für Einträge, die mit `.` beginnen, damit das UI sie ausblenden oder
/// dimmen kann. Files werden ignoriert — dies ist ein Verzeichnis-Browser.
#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirEntryBrief>, String> {
    let trimmed = path.trim();
    let p = if trimmed.is_empty() {
        std::env::current_dir().map_err(|e| e.to_string())?
    } else {
        std::path::PathBuf::from(trimmed)
    };
    let read = std::fs::read_dir(&p).map_err(|e| e.to_string())?;
    let mut out: Vec<DirEntryBrief> = read
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let hidden = name.starts_with('.');
            Some(DirEntryBrief { name, hidden })
        })
        .collect();
    out.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    Ok(out)
}

/// Erstellt ein neues Unterverzeichnis `name` innerhalb von `parent`. Liefert
/// den vollständigen Pfad des neu angelegten Ordners zurück. `name` darf keine
/// Pfadtrenner enthalten — Sandboxing für den Directory-Picker.
#[tauri::command]
pub fn create_directory(parent: String, name: String) -> Result<String, String> {
    let parent = parent.trim();
    let name = name.trim();
    if parent.is_empty() {
        return Err("parent darf nicht leer sein".into());
    }
    if name.is_empty() {
        return Err("Name darf nicht leer sein".into());
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return Err("Name enthält ungültige Zeichen".into());
    }
    let parent_path = std::path::PathBuf::from(parent);
    if !parent_path.is_dir() {
        return Err("Elternverzeichnis existiert nicht".into());
    }
    let target = parent_path.join(name);
    std::fs::create_dir(&target).map_err(|e| e.to_string())?;
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn pty_spawn(
    manager: State<'_, PtyManager>,
    cwd: String,
    env: Option<Vec<(String, String)>>,
) -> Result<u64, String> {
    manager.spawn_session(cwd, env.unwrap_or_default())
}

#[tauri::command]
pub fn pty_write(
    manager: State<'_, PtyManager>,
    session_id: u64,
    data_b64: String,
) -> Result<(), String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64)
        .map_err(|e| e.to_string())?;
    manager.write(session_id, bytes)
}

#[tauri::command]
pub fn pty_resize(
    manager: State<'_, PtyManager>,
    session_id: u64,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    manager.resize(session_id, rows, cols)
}

#[tauri::command]
pub fn pty_kill(manager: State<'_, PtyManager>, session_id: u64) -> Result<(), String> {
    manager.kill(session_id)
}

#[tauri::command]
pub fn pty_drain(
    manager: State<'_, PtyManager>,
    session_id: u64,
    max_bytes: usize,
) -> Result<String, String> {
    manager.drain_output(session_id, max_bytes)
}

#[tauri::command]
pub fn pty_peek_output(
    manager: State<'_, PtyManager>,
    session_id: u64,
    max_bytes: usize,
) -> Result<String, String> {
    manager.peek_tail(session_id, max_bytes)
}

#[tauri::command]
pub fn git_branch(cwd: String) -> Option<String> {
    crate::git_info::current_branch(std::path::Path::new(&cwd))
}
