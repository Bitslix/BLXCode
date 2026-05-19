//! Typisierte Aufrufe von Tauri `invoke` (vgl. `quit.rs`).
use crate::agent_wire::{AgentEvent, BrowserBoundsPayload, TaskSnapshot, UserTurn};
use gloo_timers::future::TimeoutFuture;
use js_sys::Reflect;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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
    #[derive(Serialize)]
    struct Args {
        turn: UserTurn,
    }
    invoke_unit_js("agent_submit_turn", args_value(Args { turn })?).await
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

pub async fn agent_clear_conversation() -> Result<(), String> {
    invoke_unit_js("agent_clear_conversation", JsValue::UNDEFINED).await
}

/// Idempotently creates `{app_data}/sandbox` and returns its absolute path.
/// Used as the always-available workspace root fallback in Phase A.
pub async fn harness_ensure_default_sandbox() -> Result<String, String> {
    invoke_typed("harness_ensure_default_sandbox", serde_json::json!({})).await
}

/// Submits the result of a client-side tool back into the running turn.
/// `call_id` must match the id of the most recent matching `ToolCall`
/// event drained from the agent queue.
pub async fn agent_submit_tool_result(
    call_id: String,
    ok: bool,
    message: Option<String>,
    data: Option<serde_json::Value>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        call_id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    }
    #[derive(Serialize)]
    struct Args {
        payload: Payload,
    }
    invoke_unit_js(
        "agent_submit_tool_result",
        args_value(Args {
            payload: Payload {
                call_id,
                ok,
                message,
                data,
            },
        })?,
    )
    .await
}

pub async fn tasks_list(workspace_cwd: String) -> Result<TaskSnapshot, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        workspace_cwd: String,
    }
    invoke_typed("tasks_list", Args { workspace_cwd }).await
}

pub async fn browser_sync_bounds(
    active_tab_id: Option<u64>,
    payload: BrowserBoundsPayload,
    navigate: Option<&str>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        #[serde(skip_serializing_if = "Option::is_none")]
        active_tab_id: Option<u64>,
        rect: BrowserBoundsPayload,
        #[serde(skip_serializing_if = "Option::is_none")]
        url_optional: Option<&'a str>,
    }
    invoke_unit_js(
        "browser_sync_bounds",
        args_value(Args {
            active_tab_id,
            rect: payload,
            url_optional: navigate,
        })?,
    )
    .await
}

pub async fn browser_navigate(tab_id: u64, url: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        tab_id: u64,
        url: &'a str,
    }
    invoke_unit_js("browser_navigate", args_value(A { tab_id, url })?).await
}

pub async fn browser_close_tab(tab_id: u64) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        tab_id: u64,
    }
    invoke_unit_js("browser_close_tab", args_value(A { tab_id })?).await
}

/// Öffnet eine URL im System-Standardbrowser (Tauri `plugin-opener`; kein `window.open` nach async).
pub async fn open_external_url(url: &str) -> Result<(), String> {
    #[derive(Serialize)]
    struct U<'a> {
        url: &'a str,
    }
    invoke_unit_js("open_external_url", args_value(U { url })?).await
}

#[allow(dead_code)]
pub async fn browser_run_js(tab_id: u64, script: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        tab_id: u64,
        script: &'a str,
    }
    invoke_unit_js("browser_run_js", args_value(A { tab_id, script })?).await
}

pub async fn browser_embedding_kind() -> Result<String, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_typed("browser_embedding_kind", Empty {}).await
}

pub async fn browser_check_iframable(url: &str) -> Result<bool, String> {
    #[derive(Serialize)]
    struct U<'a> {
        url: &'a str,
    }
    invoke_typed("browser_check_iframable", U { url }).await
}

#[allow(dead_code)]
pub async fn agent_provider_status() -> Result<serde_json::Value, String> {
    #[derive(Serialize)]
    struct Empty {}

    invoke_typed("agent_provider_status", Empty {}).await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentProviderKind {
    Openrouter,
    Anthropic,
    Openai,
}

impl AgentProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openrouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThinkingLevel {
    Off,
    Low,
    Medium,
    High,
    Max,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelEntry {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyStatus {
    pub provider: AgentProviderKind,
    pub configured: bool,
    pub masked_value: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderSettingsView {
    pub provider: AgentProviderKind,
    pub model_id: String,
    pub thinking_level: ThinkingLevel,
    pub model_cache_openrouter: Vec<ProviderModelEntry>,
    pub model_cache_anthropic: Vec<ProviderModelEntry>,
    pub model_cache_openai: Vec<ProviderModelEntry>,
    pub key_statuses: Vec<ProviderKeyStatus>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsResponse {
    pub provider: AgentProviderKind,
    pub entries: Vec<ProviderModelEntry>,
    pub source: String,
    pub used_fallback: bool,
    pub message: Option<String>,
}

pub async fn agent_settings_get() -> Result<AgentProviderSettingsView, String> {
    invoke_typed("agent_settings_get", serde_json::json!({})).await
}

pub async fn agent_settings_save(
    provider: AgentProviderKind,
    model_id: String,
    thinking_level: ThinkingLevel,
) -> Result<AgentProviderSettingsView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        patch: Patch,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Patch {
        provider: AgentProviderKind,
        model_id: String,
        thinking_level: ThinkingLevel,
    }

    invoke_typed(
        "agent_settings_save",
        Args {
            patch: Patch {
                provider,
                model_id,
                thinking_level,
            },
        },
    )
    .await
}

pub async fn agent_api_key_set(
    provider: AgentProviderKind,
    api_key: String,
) -> Result<AgentProviderSettingsView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        provider: AgentProviderKind,
        api_key: String,
    }

    invoke_typed(
        "agent_api_key_set",
        Args {
            payload: Payload { provider, api_key },
        },
    )
    .await
}

pub async fn agent_api_key_delete(
    provider: AgentProviderKind,
) -> Result<AgentProviderSettingsView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        provider: AgentProviderKind,
    }

    invoke_typed(
        "agent_api_key_delete",
        Args {
            payload: Payload { provider },
        },
    )
    .await
}

pub async fn agent_provider_models(
    provider: AgentProviderKind,
) -> Result<ProviderModelsResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        provider: AgentProviderKind,
    }

    invoke_typed(
        "agent_provider_models",
        Args {
            payload: Payload { provider },
        },
    )
    .await
}

pub async fn exit_app_ipc() -> Result<(), String> {
    invoke_unit_js("exit_app", JsValue::UNDEFINED).await
}

#[allow(dead_code)]
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

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntryBrief {
    pub name: String,
    pub hidden: bool,
}

pub async fn list_directory(path: String) -> Result<Vec<DirEntryBrief>, String> {
    #[derive(Serialize)]
    struct A {
        path: String,
    }
    invoke_typed("list_directory", A { path }).await
}

pub async fn create_directory(parent: String, name: String) -> Result<String, String> {
    #[derive(Serialize)]
    struct A {
        parent: String,
        name: String,
    }
    invoke_typed("create_directory", A { parent, name }).await
}

pub async fn default_cwd() -> Result<String, String> {
    #[derive(Serialize)]
    struct Empty {}
    invoke_typed("default_cwd", Empty {}).await
}

#[derive(Serialize)]
struct PtySpawnArgs {
    cwd: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    env: Vec<(String, String)>,
}

pub async fn pty_spawn_with_env(cwd: String, env: Vec<(String, String)>) -> Result<u64, String> {
    invoke_typed("pty_spawn", PtySpawnArgs { cwd, env }).await
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyDrainWaitArgs {
    session_id: u64,
    max_bytes: usize,
    timeout_ms: u64,
}

pub async fn pty_drain_wait(
    session_id: u64,
    max_bytes: usize,
    timeout_ms: u64,
) -> Result<String, String> {
    invoke_typed(
        "pty_drain_wait",
        PtyDrainWaitArgs {
            session_id,
            max_bytes,
            timeout_ms,
        },
    )
    .await
}

/// Non-destructive read of the last `max_bytes` bytes of a PTY session's
/// output. Safe to call concurrently with the terminal's own drain.
pub async fn pty_peek_output(session_id: u64, max_bytes: usize) -> Result<String, String> {
    invoke_typed(
        "pty_peek_output",
        PtyDrainArgs {
            session_id,
            max_bytes,
        },
    )
    .await
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHookEntry {
    pub agent: String,
    pub script_path: Option<String>,
    pub config_path: Option<String>,
    pub installed: bool,
    pub note: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
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

pub async fn workbench_save_state(json: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct A {
        json: String,
    }
    invoke_unit_js("workbench_save_state", args_value(A { json })?).await
}

pub async fn workbench_load_state() -> Result<Option<String>, String> {
    invoke_typed("workbench_load_state", serde_json::json!({})).await
}

pub async fn workbench_sessions_path() -> Result<String, String> {
    invoke_typed("workbench_sessions_path", serde_json::json!({})).await
}

pub async fn workbench_load_sessions() -> Result<Option<String>, String> {
    invoke_typed("workbench_load_sessions", serde_json::json!({})).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalNotification {
    pub unread: u32,
    pub agent: Option<String>,
    pub updated_at: Option<String>,
}

pub async fn workbench_notifications_path() -> Result<String, String> {
    invoke_typed("workbench_notifications_path", serde_json::json!({})).await
}

pub async fn workbench_load_notifications(
) -> Result<std::collections::HashMap<String, TerminalNotification>, String> {
    invoke_typed("workbench_load_notifications", serde_json::json!({})).await
}

pub async fn workbench_clear_terminal_notifications(terminal_key: String) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        terminal_key: String,
    }
    invoke_unit_js(
        "workbench_clear_terminal_notifications",
        args_value(A { terminal_key })?,
    )
    .await
}

pub async fn workbench_prune_notifications(valid_terminal_keys: Vec<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        valid_terminal_keys: Vec<String>,
    }
    invoke_unit_js(
        "workbench_prune_notifications",
        args_value(A {
            valid_terminal_keys,
        })?,
    )
    .await
}

pub async fn workbench_prune_sessions(valid_terminal_keys: Vec<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        valid_terminal_keys: Vec<String>,
    }
    invoke_unit_js(
        "workbench_prune_sessions",
        args_value(A {
            valid_terminal_keys,
        })?,
    )
    .await
}

pub async fn workbench_drop_sessions(prefix: String) -> Result<u32, String> {
    #[derive(Serialize)]
    struct A {
        prefix: String,
    }
    invoke_typed("workbench_drop_sessions", A { prefix }).await
}

pub async fn workbench_extract_sessions_prefix(prefix: String) -> Result<String, String> {
    #[derive(Serialize)]
    struct A {
        prefix: String,
    }
    invoke_typed("workbench_extract_sessions_prefix", A { prefix }).await
}

pub async fn workbench_merge_sessions_workspace(
    old_workspace_key: String,
    new_workspace_key: String,
    terminals_json: String,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        old_workspace_key: String,
        new_workspace_key: String,
        terminals_json: String,
    }
    invoke_unit_js(
        "workbench_merge_sessions_workspace",
        args_value(A {
            old_workspace_key,
            new_workspace_key,
            terminals_json,
        })?,
    )
    .await
}

pub async fn agent_session_exists(
    agent: String,
    cwd: String,
    session_id: String,
) -> Result<bool, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        agent: String,
        cwd: String,
        session_id: String,
    }
    #[derive(Serialize)]
    struct Args {
        probe: Probe,
    }
    invoke_typed(
        "agent_session_exists",
        Args {
            probe: Probe {
                agent,
                cwd,
                session_id,
            },
        },
    )
    .await
}

pub async fn agent_latest_session_id(agent: String, cwd: String) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Probe {
        agent: String,
        cwd: String,
    }
    #[derive(Serialize)]
    struct Args {
        probe: Probe,
    }
    invoke_typed(
        "agent_latest_session_id",
        Args {
            probe: Probe { agent, cwd },
        },
    )
    .await
}

// ---------------------------------------------------------------------
// Memory (workspace-scoped Markdown notes, Obsidian-style)

pub async fn workspace_ensure_agents(ws: &str) -> Result<(), String> {
    invoke_typed("workspace_ensure_agents", WsArg {
        workspace_cwd: ws,
    })
    .await
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: i64,
    pub is_template: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub path: String,
    pub content: String,
    pub modified: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub tags: Vec<String>,
    pub orphan: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub path: String,
    pub line: u32,
    pub snippet: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerResult {
    pub agent: String,
    pub path: String,
    pub installed: bool,
    pub note: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameReport {
    pub old_path: String,
    pub new_path: String,
    pub link_rewrites: u32,
    pub files_changed: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WsArg<'a> {
    workspace_cwd: &'a str,
}

pub async fn memory_list(ws: &str) -> Result<Vec<NoteMeta>, String> {
    invoke_typed("memory_list", WsArg { workspace_cwd: ws }).await
}

pub async fn memory_read(ws: &str, path: &str) -> Result<NoteContent, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_typed(
        "memory_read",
        A {
            workspace_cwd: ws,
            path,
        },
    )
    .await
}

pub async fn memory_write(ws: &str, path: &str, content: &str) -> Result<NoteContent, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
        content: &'a str,
    }
    invoke_typed(
        "memory_write",
        A {
            workspace_cwd: ws,
            path,
            content,
        },
    )
    .await
}

pub async fn memory_create(
    ws: &str,
    path: &str,
    content: Option<&str>,
) -> Result<NoteMeta, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<&'a str>,
    }
    invoke_typed(
        "memory_create",
        A {
            workspace_cwd: ws,
            path,
            content,
        },
    )
    .await
}

pub async fn memory_delete(ws: &str, path: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_unit_js(
        "memory_delete",
        args_value(A {
            workspace_cwd: ws,
            path,
        })?,
    )
    .await
}

pub async fn memory_rename(
    ws: &str,
    old_path: &str,
    new_path: &str,
    rewrite_links: bool,
) -> Result<RenameReport, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        old_path: &'a str,
        new_path: &'a str,
        rewrite_links: bool,
    }
    invoke_typed(
        "memory_rename",
        A {
            workspace_cwd: ws,
            old_path,
            new_path,
            rewrite_links,
        },
    )
    .await
}

pub async fn memory_graph(ws: &str) -> Result<GraphData, String> {
    invoke_typed("memory_graph", WsArg { workspace_cwd: ws }).await
}

pub async fn memory_backlinks(ws: &str, path: &str) -> Result<Vec<String>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_typed(
        "memory_backlinks",
        A {
            workspace_cwd: ws,
            path,
        },
    )
    .await
}

pub async fn memory_search(ws: &str, query: &str) -> Result<Vec<SearchHit>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        query: &'a str,
    }
    invoke_typed(
        "memory_search",
        A {
            workspace_cwd: ws,
            query,
        },
    )
    .await
}

#[allow(dead_code)]
pub async fn memory_install_pointers(
    ws: &str,
    agents: Vec<String>,
) -> Result<Vec<PointerResult>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        agents: Vec<String>,
    }
    invoke_typed(
        "memory_install_pointers",
        A {
            workspace_cwd: ws,
            agents,
        },
    )
    .await
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
#[allow(dead_code)]
pub async fn agent_drain_turn(on_batch: impl Fn(Vec<AgentEvent>)) -> Result<(), String> {
    agent_drain_turn_opts(false, on_batch).await
}

/// Variante mit `expect_voice`: drain läuft nach `Done` weiter, bis ein
/// `VoiceReady` oder ein zusätzlicher `Error` ankommt (max. ~30 s leerlauf).
/// Damit holen wir den TTS-Output, der vom Orchestrator nach dem
/// regulären `Done` gepusht wird.
pub async fn agent_drain_turn_opts(
    expect_voice: bool,
    on_batch: impl Fn(Vec<AgentEvent>),
) -> Result<(), String> {
    let mut seen_done = false;
    let mut idle_after_done: u32 = 0;
    const VOICE_TAIL_IDLE_MAX: u32 = 600; // 600 * 50ms ≈ 30s
    loop {
        let batch = agent_poll_events(64).await?;
        if batch.is_empty() {
            if seen_done && expect_voice {
                idle_after_done += 1;
                if idle_after_done >= VOICE_TAIL_IDLE_MAX {
                    break;
                }
            }
            TimeoutFuture::new(50).await;
            continue;
        }
        let has_done = batch
            .iter()
            .any(|e| matches!(e, AgentEvent::Done | AgentEvent::Error { .. }));
        let has_voice = batch
            .iter()
            .any(|e| matches!(e, AgentEvent::VoiceReady { .. }));
        on_batch(batch);
        if has_done {
            if !expect_voice || has_voice {
                break;
            }
            seen_done = true;
            idle_after_done = 0;
            continue;
        }
        if has_voice {
            // Voice arrived (possibly because drain was called expecting
            // voice without a preceding text path); we're done.
            break;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Voice subsystem bridge
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceProviderKind {
    Openai,
    Openrouter,
}

impl VoiceProviderKind {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PostSttFlow {
    AutoSend,
    Draft,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum SttLanguageMode {
    FollowApp,
    AutoDetect,
    Manual { code: String },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PttHotkey {
    pub enabled: bool,
    pub code: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub shift: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub meta: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttSettings {
    pub provider: VoiceProviderKind,
    pub model_id: String,
    pub sample_rate_hz: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsSettings {
    pub provider: VoiceProviderKind,
    pub model_id: String,
    pub voice: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettings {
    pub stt: SttSettings,
    pub tts: TtsSettings,
    pub post_stt_flow: PostSttFlow,
    pub stt_language: SttLanguageMode,
    pub ptt_hotkey: PttHotkey,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            stt: SttSettings {
                provider: VoiceProviderKind::Openai,
                model_id: "gpt-4o-mini-transcribe".into(),
                sample_rate_hz: 16_000,
            },
            tts: TtsSettings {
                provider: VoiceProviderKind::Openai,
                model_id: "gpt-4o-mini-tts".into(),
                voice: "nova".into(),
                enabled: true,
            },
            post_stt_flow: PostSttFlow::AutoSend,
            stt_language: SttLanguageMode::FollowApp,
            ptt_hotkey: PttHotkey {
                enabled: true,
                code: "Space".into(),
                ctrl: false,
                shift: false,
                alt: false,
                meta: false,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceGender {
    Male,
    Female,
    Neutral,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceEntry {
    pub id: String,
    pub label: String,
    pub gender: VoiceGender,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProviderVoicesResponse {
    #[allow(dead_code)]
    pub provider: VoiceProviderKind,
    pub voices: Vec<VoiceEntry>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStartResponse {
    pub turn_id: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStopResponse {
    pub text: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTtsPreviewResponse {
    pub audio_b64: String,
    pub mime: String,
}

pub async fn voice_start_recording(sample_rate_hz: u32) -> Result<VoiceStartResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        sample_rate_hz: u32,
    }
    invoke_typed(
        "voice_start_recording",
        Args {
            payload: Payload { sample_rate_hz },
        },
    )
    .await
}

pub async fn voice_stop_and_transcribe(
    turn_id: String,
    locale_hint: Option<String>,
) -> Result<VoiceStopResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        turn_id: String,
        locale_hint: Option<String>,
    }
    invoke_typed(
        "voice_stop_and_transcribe",
        Args {
            payload: Payload {
                turn_id,
                locale_hint,
            },
        },
    )
    .await
}

pub async fn voice_cancel_recording(turn_id: String) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        turn_id: String,
    }
    invoke_unit_js(
        "voice_cancel_recording",
        args_value(Args {
            payload: Payload { turn_id },
        })?,
    )
    .await
}

pub async fn voice_settings_get() -> Result<VoiceSettings, String> {
    invoke_typed("voice_settings_get", serde_json::json!({})).await
}

pub async fn voice_settings_save(patch: VoiceSettings) -> Result<VoiceSettings, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        patch: VoiceSettings,
    }
    invoke_typed("voice_settings_save", Args { patch }).await
}

pub async fn voice_provider_voices(
    provider: VoiceProviderKind,
) -> Result<VoiceProviderVoicesResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        provider: VoiceProviderKind,
    }
    invoke_typed(
        "voice_provider_voices",
        Args {
            payload: Payload { provider },
        },
    )
    .await
}

pub async fn voice_tts_preview(
    provider: VoiceProviderKind,
    model_id: String,
    voice: String,
    text: String,
) -> Result<VoiceTtsPreviewResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        provider: VoiceProviderKind,
        model_id: String,
        voice: String,
        text: String,
    }
    invoke_typed(
        "voice_tts_preview",
        Args {
            payload: Payload {
                provider,
                model_id,
                voice,
                text,
            },
        },
    )
    .await
}
