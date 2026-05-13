//! Typisierte Aufrufe von Tauri `invoke` (vgl. `quit.rs`).
use crate::agent_wire::{AgentEvent, BrowserBoundsPayload, UserTurn};
use gloo_timers::future::TimeoutFuture;
use js_sys::Reflect;
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_raw(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

#[must_use]
pub fn is_tauri_shell() -> bool {
    web_sys::window()
        .map(|w| Reflect::has(&w, &JsValue::from_str("__TAURI__")).unwrap_or(false))
        .unwrap_or(false)
}

async fn invoke_js(cmd: &'static str, args: JsValue) -> Result<JsValue, String> {
    if !is_tauri_shell() {
        return Err("Nicht in einer Tauri-Webview – IPC fehlt.".into());
    }
    invoke_raw(cmd, args)
        .await
        .map_err(|e| format!("invoke {cmd}: {}", js_error_to_string(e)))
}

fn js_error_to_string(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            Reflect::get(&value, &JsValue::from_str("message"))
                .ok()
                .and_then(|v| v.as_string())
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

fn args_value(args: impl Serialize) -> Result<JsValue, String> {
    serde_wasm_bindgen::to_value(&args).map_err(|e| format!("serde args: {e}"))
}

pub async fn invoke_unit_js(cmd: &'static str, args: JsValue) -> Result<(), String> {
    let v = invoke_js(cmd, args).await?;
    if v.is_null() || v.is_undefined() {
        return Ok(());
    }
    let _: serde_json::Value =
        serde_wasm_bindgen::from_value(v).map_err(|e| format!("deserialize {}: {}", cmd, e))?;
    Ok(())
}

pub async fn invoke_typed<T: DeserializeOwned>(
    cmd: &'static str,
    args: impl Serialize,
) -> Result<T, String> {
    let v = invoke_js(cmd, args_value(args)?).await?;
    serde_wasm_bindgen::from_value(v).map_err(|e| format!("deserialize {}: {}", cmd, e))
}

pub async fn agent_submit_turn(turn: UserTurn) -> Result<(), String> {
    invoke_unit_js("agent_submit_turn", args_value(turn)?).await
}

pub async fn agent_poll_events(max: usize) -> Result<Vec<AgentEvent>, String> {
    #[derive(Serialize)]
    struct MaxArgs {
        max: usize,
    }
    invoke_typed("agent_poll_events", MaxArgs { max }).await
}

pub async fn agent_abort() -> Result<(), String> {
    invoke_unit_js("agent_abort", JsValue::UNDEFINED).await
}

pub async fn browser_sync_bounds(
    payload: BrowserBoundsPayload,
    navigate: Option<&str>,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args<'a> {
        rect: BrowserBoundsPayload,
        #[serde(skip_serializing_if = "Option::is_none")]
        url_optional: Option<&'a str>,
    }
    invoke_unit_js(
        "browser_sync_bounds",
        args_value(Args {
            rect: payload,
            url_optional: navigate,
        })?,
    )
    .await
}

pub async fn browser_navigate(url: &str) -> Result<(), String> {
    #[derive(Serialize)]
    struct U<'a> {
        url: &'a str,
    }
    invoke_unit_js("browser_navigate", args_value(U { url })?).await
}

/// Öffnet eine URL im System-Standardbrowser (Tauri `plugin-opener`; kein `window.open` nach async).
pub async fn open_external_url(url: &str) -> Result<(), String> {
    #[derive(Serialize)]
    struct U<'a> {
        url: &'a str,
    }
    invoke_unit_js("open_external_url", args_value(U { url })?).await
}

pub async fn browser_run_js(script: &str) -> Result<(), String> {
    #[derive(Serialize)]
    struct S<'a> {
        script: &'a str,
    }
    invoke_unit_js("browser_run_js", args_value(S { script })?).await
}

pub async fn browser_embedding_kind() -> Result<String, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_typed("browser_embedding_kind", Empty {}).await
}

pub async fn agent_provider_status() -> Result<serde_json::Value, String> {
    #[derive(Serialize)]
    struct Empty {}

    invoke_typed("agent_provider_status", Empty {}).await
}

pub async fn exit_app_ipc() -> Result<(), String> {
    invoke_unit_js("exit_app", JsValue::UNDEFINED).await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNavResult {
    pub cwd: String,
    pub log_line: String,
}

#[derive(Serialize)]
struct PathNavArgs {
    base: String,
    line: String,
}

pub async fn path_nav_invoke(base: String, line: String) -> Result<PathNavResult, String> {
    invoke_typed("path_nav_exec_cmd", PathNavArgs { base, line }).await
}

#[derive(Serialize)]
struct PtySpawnArgs {
    cwd: String,
}

pub async fn pty_spawn(cwd: String) -> Result<u64, String> {
    invoke_typed("pty_spawn", PtySpawnArgs { cwd }).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyWriteArgs {
    session_id: u64,
    data_b64: String,
}

pub async fn pty_write(session_id: u64, data_b64: String) -> Result<(), String> {
    invoke_unit_js(
        "pty_write",
        args_value(PtyWriteArgs {
            session_id,
            data_b64,
        })?,
    )
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyResizeArgs {
    session_id: u64,
    rows: u16,
    cols: u16,
}

pub async fn pty_resize(session_id: u64, rows: u16, cols: u16) -> Result<(), String> {
    invoke_unit_js(
        "pty_resize",
        args_value(PtyResizeArgs {
            session_id,
            rows,
            cols,
        })?,
    )
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyDrainArgs {
    session_id: u64,
    max_bytes: usize,
}

pub async fn pty_drain(session_id: u64, max_bytes: usize) -> Result<String, String> {
    invoke_typed(
        "pty_drain",
        PtyDrainArgs {
            session_id,
            max_bytes,
        },
    )
    .await
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHookEntry {
    pub agent: String,
    pub script_path: Option<String>,
    pub config_path: Option<String>,
    pub installed: bool,
    pub note: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHooksReport {
    pub hooks_dir: String,
    pub entries: Vec<AgentHookEntry>,
}

pub async fn install_agent_hooks() -> Result<AgentHooksReport, String> {
    invoke_typed("install_agent_hooks", serde_json::json!({})).await
}

pub async fn agent_hooks_status() -> Result<AgentHooksReport, String> {
    invoke_typed("agent_hooks_status", serde_json::json!({})).await
}

pub async fn uninstall_agent_hooks() -> Result<AgentHooksReport, String> {
    invoke_typed("uninstall_agent_hooks", serde_json::json!({})).await
}

pub async fn git_branch(cwd: String) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    struct Args {
        cwd: String,
    }
    invoke_typed("git_branch", Args { cwd }).await
}

pub async fn pty_kill(session_id: u64) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct KillArgs {
        session_id: u64,
    }
    invoke_unit_js("pty_kill", args_value(KillArgs { session_id })?).await
}
/// Draint Events bis `Done`/`Error`; bei leeren Batches kurz warten (Streaming).
pub async fn agent_drain_turn(on_batch: impl Fn(Vec<AgentEvent>)) -> Result<(), String> {
    loop {
        let batch = agent_poll_events(64).await?;
        if batch.is_empty() {
            TimeoutFuture::new(50).await;
            continue;
        }
        let done = batch
            .iter()
            .any(|e| matches!(e, AgentEvent::Done | AgentEvent::Error { .. }));
        on_batch(batch);
        if done {
            break;
        }
    }
    Ok(())
}
