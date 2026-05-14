use crate::agent::{dispatch_user_turn, AgentEngineState, AgentEvent, ProviderEnv, UserTurn};
use crate::browser_host::BrowserHost;
use crate::pty_host::{path_nav_exec, PathNavResult, PtyManager};
use serde::Deserialize;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn agent_submit_turn(
    turn: UserTurn,
    agent: State<'_, Arc<AgentEngineState>>,
) -> Result<(), String> {
    dispatch_user_turn(&agent, turn)
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
pub fn agent_provider_status() -> serde_json::Value {
    ProviderEnv::from_environment().status_json()
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
    rect: BrowserBoundsPayload,
    url_optional: Option<String>,
) -> Result<(), String> {
    host.sync_bounds(&app, rect, url_optional.as_deref())
}

#[tauri::command]
pub fn browser_run_js(
    app: AppHandle,
    host: State<'_, BrowserHost>,
    script: String,
) -> Result<(), String> {
    host.eval_embedded(&app, script)
}

#[tauri::command]
pub fn browser_navigate(
    app: tauri::AppHandle,
    host: State<'_, BrowserHost>,
    url: String,
) -> Result<(), String> {
    host.navigate(&app, &url)
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

    if let Some(xfo) = headers
        .get("x-frame-options")
        .and_then(|v| v.to_str().ok())
    {
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
pub fn git_branch(cwd: String) -> Option<String> {
    crate::git_info::current_branch(std::path::Path::new(&cwd))
}
