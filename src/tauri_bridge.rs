//! Typisierte Aufrufe von Tauri `invoke` (vgl. `quit.rs`).
use crate::agent_wire::{
    AgentEvent, BrowserBoundsPayload, EventEnvelope, TaskSnapshot, TaskStatus, UserTurn,
};
use crate::skills_rules_wire::{RuleEntry, SkillEntry, SkillSourceInput};
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

pub async fn agent_submit_turn(session_id: Option<String>, turn: UserTurn) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        session_id: Option<String>,
        turn: UserTurn,
    }
    invoke_unit_js("agent_submit_turn", args_value(Args { session_id, turn })?).await
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedChatTitle {
    pub title: String,
}

pub async fn agent_generate_chat_title(prompt: String) -> Result<GeneratedChatTitle, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        prompt: String,
    }
    invoke_typed("agent_generate_chat_title", Args { prompt }).await
}

pub async fn agent_poll_events(
    session_id: Option<String>,
    max: usize,
) -> Result<Vec<EventEnvelope>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct MaxArgs {
        session_id: Option<String>,
        max: usize,
    }
    invoke_typed("agent_poll_events", MaxArgs { session_id, max }).await
}

pub async fn agent_abort(session_id: Option<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        session_id: Option<String>,
    }
    invoke_unit_js("agent_abort", args_value(Args { session_id })?).await
}

pub async fn agent_clear_conversation(session_id: Option<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        session_id: Option<String>,
    }
    invoke_unit_js("agent_clear_conversation", args_value(Args { session_id })?).await
}

// ---------------------------------------------------------------------------
// MCP server registry (mirrors `src-tauri/src/mcp`)
// ---------------------------------------------------------------------------

/// Transport for an MCP server. Mirror of the backend `McpTransport`; the
/// `kind` tag must match the backend's `#[serde(tag = "kind")]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum McpTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: std::collections::BTreeMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: std::collections::BTreeMap<String, String>,
    },
}

impl McpTransport {
    pub fn label(&self) -> &'static str {
        match self {
            McpTransport::Stdio { .. } => "stdio",
            McpTransport::Http { .. } => "http",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    pub transport: McpTransport,
}

#[derive(Clone, Debug, Deserialize)]
pub struct McpTestResult {
    pub ok: bool,
    pub tool_count: usize,
    pub error: Option<String>,
}

pub async fn mcp_list() -> Result<Vec<McpServer>, String> {
    invoke_typed("mcp_list", serde_json::json!({})).await
}

pub async fn mcp_upsert(server: McpServer) -> Result<McpServer, String> {
    #[derive(Serialize)]
    struct Args {
        server: McpServer,
    }
    invoke_typed("mcp_upsert", Args { server }).await
}

pub async fn mcp_remove(id: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_unit_js("mcp_remove", args_value(Args { id })?).await
}

pub async fn mcp_test(id: String) -> Result<McpTestResult, String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_typed("mcp_test", Args { id }).await
}

/// Fire-and-forget: the caller does not inspect the per-CLI export results, so
/// the response payload is discarded.
pub async fn mcp_export_cli_configs(workspace_root: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        workspace_root: String,
    }
    invoke_unit_js(
        "mcp_export_cli_configs",
        args_value(Args { workspace_root })?,
    )
    .await
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogSettingsView {
    pub log_path: Option<String>,
    pub default_log_path: String,
    pub effective_log_path: String,
}

pub async fn app_log_settings_get() -> Result<AppLogSettingsView, String> {
    invoke_typed("app_log_settings_get", serde_json::json!({})).await
}

pub async fn app_log_settings_save(log_path: Option<String>) -> Result<AppLogSettingsView, String> {
    #[derive(Serialize)]
    struct Args {
        log_path: Option<String>,
    }
    invoke_typed("app_log_settings_save", Args { log_path }).await
}

pub async fn app_log_event(
    level: String,
    source: String,
    event: String,
    metadata: serde_json::Value,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        level: String,
        source: String,
        event: String,
        metadata: serde_json::Value,
    }
    invoke_unit_js(
        "app_log_event",
        args_value(Args {
            level,
            source,
            event,
            metadata,
        })?,
    )
    .await
}

pub async fn app_log_clear() -> Result<(), String> {
    invoke_unit_js("app_log_clear", JsValue::UNDEFINED).await
}

pub async fn app_log_delete() -> Result<(), String> {
    invoke_unit_js("app_log_delete", JsValue::UNDEFINED).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentImageFilePayload {
    pub label: String,
    pub mime: String,
    pub bytes_b64: String,
    pub size_bytes: u64,
}

pub async fn agent_read_image_file(path: String) -> Result<AgentImageFilePayload, String> {
    #[derive(Serialize)]
    struct Args {
        path: String,
    }
    invoke_typed("agent_read_image_file", Args { path }).await
}

/// Idempotently creates `{app_data}/sandbox` and returns its absolute path.
/// Used as the always-available workspace root fallback in Phase A.
pub async fn harness_ensure_default_sandbox() -> Result<String, String> {
    invoke_typed("harness_ensure_default_sandbox", serde_json::json!({})).await
}

/// Returns the user's home directory. Default for the "default project
/// directory" setting that seeds new workspace cwds.
pub async fn harness_user_home_dir() -> Result<String, String> {
    invoke_typed("harness_user_home_dir", serde_json::json!({})).await
}

pub async fn app_version() -> Result<String, String> {
    invoke_typed("app_version", serde_json::json!({})).await
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsView {
    pub channel: UpdateChannel,
}

pub async fn updater_settings_get() -> Result<UpdateSettingsView, String> {
    invoke_typed("updater_settings_get", serde_json::json!({})).await
}

pub async fn updater_settings_save(channel: UpdateChannel) -> Result<UpdateSettingsView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        patch: UpdateSettingsView,
    }
    invoke_typed(
        "updater_settings_save",
        Args {
            patch: UpdateSettingsView { channel },
        },
    )
    .await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    pub status: String,
    pub channel: UpdateChannel,
    pub current_version: String,
    pub available_version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub target: Option<String>,
    pub download_url: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: String,
    pub busy: bool,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub updated_at_ms: u64,
}

pub async fn updater_check() -> Result<UpdateCheckResponse, String> {
    invoke_typed("updater_check", serde_json::json!({})).await
}

pub async fn updater_install_start() -> Result<UpdateProgress, String> {
    invoke_typed("updater_install_start", serde_json::json!({})).await
}

pub async fn updater_poll_progress() -> Result<UpdateProgress, String> {
    invoke_typed("updater_poll_progress", serde_json::json!({})).await
}

pub async fn app_relaunch() -> Result<(), String> {
    invoke_unit_js("app_relaunch", JsValue::UNDEFINED).await
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateReleaseNotesResponse {
    pub version: String,
    pub title: String,
    pub summary: String,
    pub sections: Vec<PostUpdateReleaseNotesSection>,
    pub source: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateReleaseNotesSection {
    pub title: String,
    pub items: Vec<PostUpdateReleaseNotesItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostUpdateReleaseNotesItem {
    pub title: Option<String>,
    pub body: String,
}

pub async fn post_update_release_notes(
    version: String,
    channel: UpdateChannel,
) -> Result<PostUpdateReleaseNotesResponse, String> {
    #[derive(Serialize)]
    struct Args {
        version: String,
        channel: UpdateChannel,
    }
    invoke_typed("post_update_release_notes", Args { version, channel }).await
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
    agent_submit_tool_result_for_session(None, call_id, ok, message, data).await
}

pub async fn agent_submit_tool_result_for_session(
    session_id: Option<String>,
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
    #[serde(rename_all = "camelCase")]
    struct Args {
        session_id: Option<String>,
        payload: Payload,
    }
    invoke_unit_js(
        "agent_submit_tool_result",
        args_value(Args {
            session_id,
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
    Ollama,
    LmStudio,
    HuggingFace,
    Cloudflare,
    Together,
    Portkey,
}

impl AgentProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openrouter => "openrouter",
            Self::Anthropic => "anthropic",
            Self::Openai => "openai",
            Self::Ollama => "ollama",
            Self::LmStudio => "lmStudio",
            Self::HuggingFace => "huggingFace",
            Self::Cloudflare => "cloudflare",
            Self::Together => "together",
            Self::Portkey => "portkey",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentOrbMode {
    #[serde(rename = "3d")]
    ThreeD,
    #[serde(rename = "2d")]
    TwoD,
}

/// Mirrors `agent_settings::DEFAULT_TOOL_LOOP_LIMIT` on the backend. Used as
/// the serde default so older settings payloads without the field decode.
pub const DEFAULT_TOOL_LOOP_LIMIT: u32 = 36;
/// Supported UI range for the tool-loop limit (matches backend clamp).
pub const MIN_TOOL_LOOP_LIMIT: u32 = 1;
pub const MAX_TOOL_LOOP_LIMIT: u32 = 500;

fn default_tool_loop_limit() -> u32 {
    DEFAULT_TOOL_LOOP_LIMIT
}

/// Auto-compaction defaults / range (mirrors `agent_settings`).
pub const DEFAULT_AUTO_COMPACT_THRESHOLD_PCT: u8 = 85;
pub const MIN_AUTO_COMPACT_THRESHOLD_PCT: u8 = 50;
pub const MAX_AUTO_COMPACT_THRESHOLD_PCT: u8 = 95;

fn default_auto_compact_enabled() -> bool {
    true
}

fn default_auto_compact_threshold_pct() -> u8 {
    DEFAULT_AUTO_COMPACT_THRESHOLD_PCT
}

fn default_orb_mode() -> AgentOrbMode {
    AgentOrbMode::ThreeD
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(PartialEq)]
pub struct ProviderModelEntry {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    /// USD-per-token rates from OpenRouter `/models`. Direct-provider
    /// entries leave this `None`; the backend resolves their cost via
    /// the id-mapping table in `agent/pricing.rs`.
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    /// Max context window in tokens from OpenRouter `/models`. `None` for
    /// direct providers (resolved server-side via the fallback table).
    #[serde(default)]
    pub context_length: Option<u64>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(PartialEq)]
pub struct ModelPricing {
    pub prompt: f64,
    pub completion: f64,
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
    #[serde(default = "default_tool_loop_limit")]
    pub tool_loop_limit: u32,
    #[serde(default = "default_auto_compact_enabled")]
    pub auto_compact_enabled: bool,
    #[serde(default = "default_auto_compact_threshold_pct")]
    pub auto_compact_threshold_pct: u8,
    #[serde(default = "default_orb_mode")]
    pub orb_mode: AgentOrbMode,
    #[serde(default)]
    pub agent_nickname: String,
    #[serde(default)]
    pub onboarding_seen: bool,
    #[serde(default)]
    pub default_session_role: Option<String>,
    pub model_cache_openrouter: Vec<ProviderModelEntry>,
    pub model_cache_anthropic: Vec<ProviderModelEntry>,
    pub model_cache_openai: Vec<ProviderModelEntry>,
    #[serde(default)]
    pub model_caches: std::collections::BTreeMap<String, Vec<ProviderModelEntry>>,
    #[serde(default)]
    pub provider_base_urls: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub cloudflare_account_id: String,
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

// ---------- Centralized API keys (Settings → API Keys) ----------

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApiKeyCategory {
    Llm,
    Search,
    ImageVideo,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyEntry {
    pub kind: String,
    pub label: String,
    pub category: ApiKeyCategory,
    pub configured: bool,
    #[serde(default)]
    pub masked_value: Option<String>,
    #[serde(default)]
    pub via_env: bool,
    #[serde(default)]
    pub env_var: Option<String>,
    #[serde(default)]
    pub coming_soon: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeysStatus {
    pub entries: Vec<ApiKeyEntry>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ApiKeyAction {
    Set { kind: String, value: String },
    Delete { kind: String },
}

#[allow(dead_code)]
pub async fn api_keys_status() -> Result<ApiKeysStatus, String> {
    invoke_typed("api_keys_status", serde_json::json!({})).await
}

#[allow(dead_code)]
pub async fn api_keys_apply(actions: Vec<ApiKeyAction>) -> Result<ApiKeysStatus, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        actions: Vec<ApiKeyAction>,
    }
    invoke_typed(
        "api_keys_apply",
        Args {
            payload: Payload { actions },
        },
    )
    .await
}

// ---------- Legacy per-provider settings ----------

pub async fn agent_settings_get() -> Result<AgentProviderSettingsView, String> {
    invoke_typed("agent_settings_get", serde_json::json!({})).await
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActiveContextWindow {
    pub provider: AgentProviderKind,
    pub model_id: String,
    /// Max context window in tokens; `None` when the model is unknown.
    #[serde(default)]
    pub context_length: Option<u64>,
}

/// Resolve the active model's context-window size (tokens). Drives the chat
/// header occupancy meter.
pub async fn agent_active_context_window() -> Result<ActiveContextWindow, String> {
    invoke_typed("agent_active_context_window", serde_json::json!({})).await
}

#[allow(clippy::too_many_arguments)]
pub async fn agent_settings_save(
    provider: AgentProviderKind,
    model_id: String,
    thinking_level: ThinkingLevel,
    tool_loop_limit: u32,
    auto_compact_enabled: bool,
    auto_compact_threshold_pct: u8,
    orb_mode: AgentOrbMode,
    agent_nickname: String,
    default_session_role: Option<String>,
    provider_base_urls: std::collections::BTreeMap<String, String>,
    cloudflare_account_id: String,
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
        tool_loop_limit: u32,
        auto_compact_enabled: bool,
        auto_compact_threshold_pct: u8,
        orb_mode: AgentOrbMode,
        agent_nickname: String,
        default_session_role: Option<String>,
        provider_base_urls: std::collections::BTreeMap<String, String>,
        cloudflare_account_id: String,
    }

    invoke_typed(
        "agent_settings_save",
        Args {
            patch: Patch {
                provider,
                model_id,
                thinking_level,
                tool_loop_limit,
                auto_compact_enabled,
                auto_compact_threshold_pct,
                orb_mode,
                agent_nickname,
                default_session_role,
                provider_base_urls,
                cloudflare_account_id,
            },
        },
    )
    .await
}

pub async fn agent_onboarding_complete(
    agent_nickname: String,
    default_session_role: Option<String>,
) -> Result<AgentProviderSettingsView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        agent_nickname: String,
        default_session_role: Option<String>,
    }

    invoke_typed(
        "agent_onboarding_complete",
        Args {
            agent_nickname,
            default_session_role,
        },
    )
    .await
}

/// Validate a candidate agent nickname without saving. `Ok(())` = acceptable
/// (blank means "use default"); `Err(code)` is a stable reason code
/// (`tooLong` / `invalidChars` / `badWord`) for i18n mapping in the UI.
pub async fn agent_validate_nickname(name: String) -> Result<(), String> {
    invoke_typed(
        "agent_validate_nickname",
        serde_json::json!({ "name": name }),
    )
    .await
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResult {
    pub summary: String,
    #[serde(default)]
    pub before_tokens: u64,
    #[serde(default)]
    pub after_tokens_estimate: u64,
    #[serde(default)]
    pub messages_before: usize,
}

/// Summarize the running conversation and replace it with a compact briefing.
/// `current_tokens` is the meter's live occupancy (for an accurate before/after).
pub async fn agent_compact_conversation(
    session_id: Option<String>,
    current_tokens: Option<u64>,
) -> Result<CompactionResult, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        session_id: Option<String>,
        current_tokens: Option<u64>,
    }
    invoke_typed(
        "agent_compact_conversation",
        Args {
            session_id,
            current_tokens,
        },
    )
    .await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebProviderKind {
    None,
    Tavily,
    Brave,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebKeyStatus {
    pub kind: String,
    pub configured: bool,
    pub masked_value: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWebSettings {
    pub provider: WebProviderKind,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWebSettingsView {
    pub settings: AgentWebSettings,
    pub key_statuses: Vec<WebKeyStatus>,
}

pub async fn agent_web_settings_get() -> Result<AgentWebSettingsView, String> {
    invoke_typed("agent_web_settings_get", serde_json::json!({})).await
}

pub async fn agent_web_settings_save(
    provider: WebProviderKind,
) -> Result<AgentWebSettingsView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        patch: Patch,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Patch {
        provider: WebProviderKind,
    }

    invoke_typed(
        "agent_web_settings_save",
        Args {
            patch: Patch { provider },
        },
    )
    .await
}

pub async fn agent_environment_invalidate() -> Result<(), String> {
    invoke_unit_js("agent_environment_invalidate", JsValue::NULL).await
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

// ---------- HeartBeat + Memory Indexer ----------

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatSettings {
    pub enabled: bool,
    pub interval_minutes: u32,
    #[serde(default)]
    pub service_enabled: std::collections::BTreeMap<String, bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HeartbeatServiceStatus {
    Idle,
    Running,
    Stalled,
    Error,
    Disabled,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatServiceView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub status: HeartbeatServiceStatus,
    pub last_call: Option<u64>,
    pub next_call: Option<u64>,
    pub last_response: Option<String>,
    pub skip_count: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryIndexSettings {
    pub provider: AgentProviderKind,
    pub model_id: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryIndexStats {
    pub workspace_count: usize,
    pub global_count: usize,
    pub last_indexed_at: Option<u64>,
    #[serde(default)]
    pub generated_files: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub async fn heartbeat_settings_get() -> Result<HeartbeatSettings, String> {
    invoke_typed("heartbeat_settings_get", serde_json::json!({})).await
}

pub async fn heartbeat_settings_save(
    settings: HeartbeatSettings,
) -> Result<HeartbeatSettings, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        settings: HeartbeatSettings,
    }
    invoke_typed("heartbeat_settings_save", Args { settings }).await
}

pub async fn heartbeat_services_list() -> Result<Vec<HeartbeatServiceView>, String> {
    invoke_typed("heartbeat_services_list", serde_json::json!({})).await
}

pub async fn heartbeat_service_set_enabled(
    id: String,
    enabled: bool,
) -> Result<Vec<HeartbeatServiceView>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        id: String,
        enabled: bool,
    }
    invoke_typed("heartbeat_service_set_enabled", Args { id, enabled }).await
}

pub async fn heartbeat_service_run_now(id: String) -> Result<Vec<HeartbeatServiceView>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        id: String,
    }
    invoke_typed("heartbeat_service_run_now", Args { id }).await
}

pub async fn heartbeat_set_open_workspaces(workspaces: Vec<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        workspaces: Vec<String>,
    }
    invoke_unit_js(
        "heartbeat_set_open_workspaces",
        args_value(Args { workspaces })?,
    )
    .await
}

pub async fn memory_index_settings_get() -> Result<MemoryIndexSettings, String> {
    invoke_typed("memory_index_settings_get", serde_json::json!({})).await
}

pub async fn memory_index_settings_save(
    settings: MemoryIndexSettings,
) -> Result<MemoryIndexSettings, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        settings: MemoryIndexSettings,
    }
    invoke_typed("memory_index_settings_save", Args { settings }).await
}

pub async fn memory_index_stats() -> Result<MemoryIndexStats, String> {
    invoke_typed("memory_index_stats", serde_json::json!({})).await
}

pub fn listen_heartbeat_services_changed(
    callback: impl FnMut(Vec<HeartbeatServiceView>) + 'static,
) -> Option<TauriEventListener> {
    listen_tauri_event::<Vec<HeartbeatServiceView>>("heartbeat_services_changed", callback)
}

pub async fn exit_app_ipc() -> Result<(), String> {
    invoke_unit_js("exit_app", JsValue::UNDEFINED).await
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum PopoutPayload {
    Terminal {
        workspace_id: u64,
        slot_id: u64,
        pane_id: u64,
        terminal_key: String,
    },
    Memory {
        workspace_id: u64,
        initial_view: Option<String>,
    },
    MemoryGraph {
        workspace_id: u64,
    },
    MermaidFile {
        workspace_id: u64,
        rel_path: String,
    },
    DiagramGallery {
        workspace_id: u64,
        scope: serde_json::Value,
    },
    FileDiff {
        workspace_id: u64,
        rel_path: String,
        staged: bool,
    },
}

impl PopoutPayload {
    #[must_use]
    pub fn fallback_title(&self) -> String {
        match self {
            Self::Terminal { slot_id, .. } => format!("Terminal #{slot_id}"),
            Self::Memory { .. } => "Memory".into(),
            Self::MemoryGraph { .. } => "Memory Graph".into(),
            Self::MermaidFile { rel_path, .. } => format!("Mermaid: {rel_path}"),
            Self::DiagramGallery { .. } => "Mermaid Diagrams".into(),
            Self::FileDiff {
                rel_path, staged, ..
            } => {
                let state = if *staged { "staged" } else { "unstaged" };
                format!("Diff: {rel_path} ({state})")
            }
        }
    }
}

pub async fn popout_open(payload: PopoutPayload) -> Result<String, String> {
    #[derive(Serialize)]
    struct Args {
        payload: PopoutPayload,
    }
    invoke_typed("popout_open", Args { payload }).await
}

pub async fn popout_focus(label: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        label: String,
    }
    invoke_unit_js("popout_focus", args_value(Args { label })?).await
}

pub async fn popout_close_current() -> Result<(), String> {
    invoke_unit_js("popout_close_current", JsValue::UNDEFINED).await
}

// ---------------------------------------------------------------------------
// Custom title bar — window controls (decorations:false). The privileged
// min/max/close/fullscreen calls live in the Rust backend; the frontend only
// holds the drag permission. All wrappers are guarded by `is_tauri_shell()`
// via `invoke_js`, so they degrade to an `Err` in the browser preview.
// ---------------------------------------------------------------------------

pub async fn window_minimize() -> Result<(), String> {
    invoke_unit_js("window_minimize", JsValue::UNDEFINED).await
}

/// Toggles maximize/restore; returns the resulting `is_maximized` flag.
pub async fn window_toggle_maximize() -> Result<bool, String> {
    invoke_typed("window_toggle_maximize", serde_json::json!({})).await
}

pub async fn window_is_maximized() -> Result<bool, String> {
    invoke_typed("window_is_maximized", serde_json::json!({})).await
}

pub async fn window_close() -> Result<(), String> {
    invoke_unit_js("window_close", JsValue::UNDEFINED).await
}

/// Toggles fullscreen; returns the resulting `is_fullscreen` flag.
pub async fn window_toggle_fullscreen() -> Result<bool, String> {
    invoke_typed("window_toggle_fullscreen", serde_json::json!({})).await
}

#[allow(dead_code)]
pub async fn window_is_fullscreen() -> Result<bool, String> {
    invoke_typed("window_is_fullscreen", serde_json::json!({})).await
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowStatePayload {
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
    pub fullscreen: bool,
}

pub async fn window_state() -> Result<WindowStatePayload, String> {
    invoke_typed("window_state", serde_json::json!({})).await
}

pub async fn window_set_size(width: u32, height: u32) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        width: u32,
        height: u32,
    }
    invoke_unit_js("window_set_size", args_value(Args { width, height })?).await
}

pub async fn window_set_fullscreen(enabled: bool) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        enabled: bool,
    }
    invoke_unit_js("window_set_fullscreen", args_value(Args { enabled })?).await
}

pub async fn clipboard_read_text() -> Result<String, String> {
    invoke_typed("clipboard_read_text", ()).await
}

pub async fn clipboard_write_text(text: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        text: String,
    }
    invoke_unit_js("clipboard_write_text", args_value(Args { text })?).await
}

/// Tauri native clipboard when available; otherwise Web Clipboard API.
pub async fn clipboard_read_text_compat() -> Result<String, String> {
    if is_tauri_shell() {
        return clipboard_read_text().await;
    }
    let Some(window) = web_sys::window() else {
        return Err("no window".into());
    };
    let clipboard = window.navigator().clipboard();
    let promise = clipboard.read_text();
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| js_error_to_string(e))
        .and_then(|v| {
            v.as_string()
                .ok_or_else(|| "clipboard read returned non-string".into())
        })
}

/// Tauri native clipboard when available; otherwise Web Clipboard API.
pub async fn clipboard_write_text_compat(text: String) -> Result<(), String> {
    if is_tauri_shell() {
        return clipboard_write_text(text).await;
    }
    let Some(window) = web_sys::window() else {
        return Err("no window".into());
    };
    let clipboard = window.navigator().clipboard();
    let promise = clipboard.write_text(&text);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| js_error_to_string(e))?;
    Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntryBrief {
    pub name: String,
    pub is_dir: bool,
    pub hidden: bool,
}

pub async fn list_path_entries(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<Vec<FsEntryBrief>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "list_path_entries",
        A {
            workspace_root,
            path,
            connection_id,
        },
    )
    .await
}

/// Lists workspace files (relative paths) for the fuzzy file finder. Skips
/// protected/vendor/build dirs and is capped backend-side. Mirrors
/// `fs_entries::list_workspace_files`.
pub async fn list_workspace_files(
    workspace_root: String,
    connection_id: Option<String>,
) -> Result<Vec<String>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "list_workspace_files",
        A {
            workspace_root,
            connection_id,
        },
    )
    .await
}

/// Creates an empty file at `path` (relative to `workspace_root`). Errors if it
/// already exists. Mirrors `fs_entries::create_workspace_file`.
pub async fn create_workspace_file(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js(
        "create_workspace_file",
        args_value(A {
            workspace_root,
            path,
            connection_id,
        })?,
    )
    .await
}

/// Creates an empty directory at `path` (relative to `workspace_root`). Errors
/// if it already exists. Mirrors `fs_entries::create_workspace_dir`.
pub async fn create_workspace_dir(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js(
        "create_workspace_dir",
        args_value(A {
            workspace_root,
            path,
            connection_id,
        })?,
    )
    .await
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextFilePreview {
    pub content: String,
    pub truncated: bool,
    pub byte_len: u64,
    /// Modification timestamp (Unix ms) when available; `None` for remote reads.
    #[serde(default)]
    pub modified_ms: Option<i64>,
    /// FNV-1a content hash of the raw bytes — the conflict-guard baseline.
    #[serde(default)]
    pub hash: String,
}

pub async fn read_workspace_text_file(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<TextFilePreview, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "read_workspace_text_file",
        A {
            workspace_root,
            path,
            connection_id,
        },
    )
    .await
}

/// Result of a successful [`write_workspace_text_file`] — mirrors
/// `fs_entries::WriteResult`. Used to reset the editor's conflict baseline.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    #[serde(default)]
    pub modified_ms: Option<i64>,
    pub hash: String,
    pub byte_len: u64,
}

/// Writes `content` over an existing workspace file. When `expected_hash` is
/// `Some`, the backend refuses the write if the on-disk content changed
/// (the returned error string starts with `conflict:`). Mirrors
/// `fs_entries::write_workspace_text_file`.
pub async fn write_workspace_text_file(
    workspace_root: String,
    path: String,
    content: String,
    expected_hash: Option<String>,
    connection_id: Option<String>,
) -> Result<WriteResult, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        expected_hash: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "write_workspace_text_file",
        A {
            workspace_root,
            path,
            content,
            expected_hash,
            connection_id,
        },
    )
    .await
}

/// `true` when a save error string denotes an on-disk conflict (vs a generic
/// I/O / permission error). Matches the backend `CONFLICT_PREFIX`.
#[must_use]
pub fn is_conflict_error(err: &str) -> bool {
    err.starts_with("conflict:")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Image,
    Video,
    Markdown,
    Mermaid,
    Code,
    Text,
    Binary,
}

/// Mirrors `src-tauri/src/fs_entries.rs::PolicyKind`. Set when the file's
/// stem matches a well-known repository policy document; the frontend renders
/// a hero banner above the markdown body for these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyKind {
    License,
    Contributing,
    Contributors,
    CodeOfConduct,
    Security,
    Authors,
    Changelog,
    Readme,
    Support,
    Agents,
    Claude,
    Codex,
    Gemini,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub name: String,
    pub rel_path: String,
    pub byte_len: u64,
    pub modified_ms: Option<i64>,
    pub kind: FileKind,
    pub mime: Option<String>,
    #[serde(default)]
    pub policy_kind: Option<PolicyKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryFilePreview {
    pub base64: String,
    pub mime: String,
    pub byte_len: u64,
    pub truncated: bool,
}

pub async fn stat_workspace_file(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<FileMeta, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "stat_workspace_file",
        A {
            workspace_root,
            path,
            connection_id,
        },
    )
    .await
}

pub async fn read_workspace_image_file(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<BinaryFilePreview, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "read_workspace_image_file",
        A {
            workspace_root,
            path,
            connection_id,
        },
    )
    .await
}

pub async fn read_workspace_video_file(
    workspace_root: String,
    path: String,
    connection_id: Option<String>,
) -> Result<BinaryFilePreview, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        workspace_root: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "read_workspace_video_file",
        A {
            workspace_root,
            path,
            connection_id,
        },
    )
    .await
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

// ---------------------------------------------------------------------------
// SSH remote connections
// ---------------------------------------------------------------------------

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
    #[default]
    KeepaliveOnly,
}

/// Mirror of `src-tauri/src/ssh_remotes.rs::RemoteConnection`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnection {
    #[serde(default)]
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

/// Preset + secret-presence flags returned by `ssh_remotes_list`/`ssh_remote_save`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConnectionView {
    pub connection: RemoteConnection,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub has_passphrase: bool,
}

pub async fn ssh_remotes_list() -> Result<Vec<RemoteConnectionView>, String> {
    invoke_typed("ssh_remotes_list", serde_json::json!({})).await
}

/// One built-in harness session role (specialized skill), mirrors the backend
/// `agent::session_roles::RoleMeta`. Drives the Create-Workspace session-mode
/// picker and the colored role sub-line in the agent name badge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRoleView {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub terminal_agent_swarm: bool,
    #[serde(default)]
    pub enabled: bool,
}

pub async fn agent_session_roles_list() -> Result<Vec<SessionRoleView>, String> {
    invoke_typed("agent_session_roles_list", serde_json::json!({})).await
}

/// One saved workspace fleet preset, mirrors the backend
/// `workspace_presets::WorkspacePreset`. Stored globally in the app-data dir.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePresetView {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub terminal_count: u8,
    #[serde(default)]
    pub agent_counts: [u8; 5],
    #[serde(default)]
    pub agent_models: [String; 5],
    #[serde(default)]
    pub agent_efforts: [String; 5],
    #[serde(default)]
    pub slot_names: Vec<String>,
    #[serde(default)]
    pub session_role: Option<String>,
}

pub async fn workspace_presets_list() -> Result<Vec<WorkspacePresetView>, String> {
    invoke_typed("workspace_presets_list", serde_json::json!({})).await
}

pub async fn workspace_presets_save(
    preset: WorkspacePresetView,
) -> Result<Vec<WorkspacePresetView>, String> {
    #[derive(Serialize)]
    struct Args {
        preset: WorkspacePresetView,
    }
    invoke_typed("workspace_presets_save", Args { preset }).await
}

pub async fn workspace_presets_delete(id: String) -> Result<Vec<WorkspacePresetView>, String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_typed("workspace_presets_delete", Args { id }).await
}

pub async fn ssh_remote_save(
    connection: RemoteConnection,
    password: Option<String>,
    passphrase: Option<String>,
) -> Result<RemoteConnectionView, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        connection: RemoteConnection,
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        passphrase: Option<String>,
    }
    #[derive(Serialize)]
    struct Args {
        payload: Payload,
    }
    invoke_typed(
        "ssh_remote_save",
        Args {
            payload: Payload {
                connection,
                password,
                passphrase,
            },
        },
    )
    .await
}

pub async fn ssh_remote_delete(id: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_unit_js("ssh_remote_delete", args_value(Args { id })?).await
}

pub async fn ssh_remote_test(
    connection: RemoteConnection,
    password: Option<String>,
    passphrase: Option<String>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        connection: RemoteConnection,
        #[serde(skip_serializing_if = "Option::is_none")]
        password: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        passphrase: Option<String>,
    }
    #[derive(Serialize)]
    struct Args {
        payload: Payload,
    }
    invoke_unit_js(
        "ssh_remote_test",
        args_value(Args {
            payload: Payload {
                connection,
                password,
                passphrase,
            },
        })?,
    )
    .await
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirEntry {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDirListing {
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<RemoteDirEntry>,
}

pub async fn ssh_remote_list_dirs(
    connection_id: String,
    path: String,
) -> Result<RemoteDirListing, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        connection_id: String,
        path: String,
    }
    invoke_typed(
        "ssh_remote_list_dirs",
        Args {
            connection_id,
            path,
        },
    )
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PtySpawnRemoteArgs {
    connection_id: String,
    terminal_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    remote_dir: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    env: Vec<(String, String)>,
}

/// Close the SSH exec channel for a connection (last remote workspace closed).
pub async fn remote_exec_close(connection_id: String) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        connection_id: String,
    }
    invoke_unit_js("remote_exec_close", args_value(Args { connection_id })?).await
}

/// Spawn an ssh terminal bound to a saved remote connection. Secrets stay in
/// the Rust backend; only the connection id crosses the bridge.
pub async fn pty_spawn_remote(
    connection_id: String,
    terminal_key: String,
    remote_dir: Option<String>,
    env: Vec<(String, String)>,
) -> Result<u64, String> {
    invoke_typed(
        "pty_spawn_remote",
        PtySpawnRemoteArgs {
            connection_id,
            terminal_key,
            remote_dir,
            env,
        },
    )
    .await
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextImageInput {
    pub id: String,
    pub label: String,
    pub mime: String,
    pub bytes_b64: String,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextImageExport {
    pub id: String,
    #[allow(dead_code)]
    pub label: String,
    #[allow(dead_code)]
    pub mime: String,
    #[allow(dead_code)]
    pub size_bytes: u64,
    pub path: String,
    #[allow(dead_code)]
    pub filename: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentContextExportReport {
    pub dir: String,
    pub manifest_path: String,
    pub images: Vec<AgentContextImageExport>,
}

pub async fn agent_export_context_images(
    workspace_cwd: String,
    items: Vec<AgentContextImageInput>,
) -> Result<AgentContextExportReport, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        workspace_cwd: String,
        items: Vec<AgentContextImageInput>,
    }
    invoke_typed(
        "agent_export_context_images",
        Args {
            workspace_cwd,
            items,
        },
    )
    .await
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

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOutputSnapshot {
    pub session_id: u64,
    pub seq: u64,
    pub bytes: usize,
    pub text: String,
    pub timed_out: bool,
    pub last_output_ms: Option<u128>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PtyWaitOutputArgs {
    session_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_seq: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contains: Option<String>,
}

pub async fn pty_wait_output(
    session_id: u64,
    after_seq: Option<u64>,
    timeout_ms: Option<u64>,
    idle_ms: Option<u64>,
    max_bytes: Option<usize>,
    contains: Option<String>,
) -> Result<PtyOutputSnapshot, String> {
    invoke_typed(
        "pty_wait_output",
        PtyWaitOutputArgs {
            session_id,
            after_seq,
            timeout_ms,
            idle_ms,
            max_bytes,
            contains,
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

pub async fn workbench_usage_path() -> Result<String, String> {
    invoke_typed("workbench_usage_path", serde_json::json!({})).await
}

pub async fn workbench_load_sessions() -> Result<Option<String>, String> {
    invoke_typed("workbench_load_sessions", serde_json::json!({})).await
}

pub async fn workbench_load_usage_snapshot(terminal_key: String) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        terminal_key: String,
    }
    invoke_typed("workbench_load_usage_snapshot", A { terminal_key }).await
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
    pub target: Option<serde_json::Value>,
    pub read: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub sent_at: Option<i64>,
    pub dedupe_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentNotificationInput {
    pub id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub kind: String,
    pub severity: Option<String>,
    pub source: Option<String>,
    pub target: Option<serde_json::Value>,
    pub dedupe_key: Option<String>,
    pub read: Option<bool>,
    pub sent: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentNotificationPatch {
    pub id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub kind: Option<String>,
    pub severity: Option<String>,
    pub source: Option<String>,
    pub target: Option<serde_json::Value>,
    pub read: Option<bool>,
    pub sent: Option<bool>,
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

pub async fn workbench_list_agent_notifications(
    include_read: bool,
    limit: usize,
) -> Result<Vec<AgentNotification>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        include_read: Option<bool>,
        limit: Option<usize>,
    }
    invoke_typed(
        "workbench_list_agent_notifications",
        A {
            include_read: Some(include_read),
            limit: Some(limit),
        },
    )
    .await
}

pub async fn workbench_upsert_agent_notification(
    input: AgentNotificationInput,
) -> Result<AgentNotification, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        input: AgentNotificationInput,
    }
    invoke_typed("workbench_upsert_agent_notification", A { input }).await
}

pub async fn workbench_update_agent_notification(
    patch: AgentNotificationPatch,
) -> Result<AgentNotification, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        patch: AgentNotificationPatch,
    }
    invoke_typed("workbench_update_agent_notification", A { patch }).await
}

pub async fn workbench_remove_agent_notification(id: String) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        id: String,
    }
    invoke_unit_js("workbench_remove_agent_notification", args_value(A { id })?).await
}

pub async fn workbench_mark_agent_notifications_read(
    id: Option<String>,
    all: bool,
) -> Result<Vec<AgentNotification>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A {
        id: Option<String>,
        all: Option<bool>,
    }
    invoke_typed(
        "workbench_mark_agent_notifications_read",
        A { id, all: Some(all) },
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

pub async fn workbench_rewrite_terminal_keys(pairs: Vec<(String, String)>) -> Result<(), String> {
    #[derive(Serialize)]
    struct A {
        pairs: Vec<(String, String)>,
    }
    invoke_unit_js("workbench_rewrite_terminal_keys", args_value(A { pairs })?).await
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

/// Newest agent session id for a remote cwd, discovered over the SSH exec
/// channel (no remote hooks). Used for remote-workspace resume.
pub async fn agent_remote_latest_session_id(
    connection_id: String,
    agent: String,
    cwd: String,
) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        connection_id: String,
        agent: String,
        cwd: String,
    }
    invoke_typed(
        "agent_remote_latest_session_id",
        Args {
            connection_id,
            agent,
            cwd,
        },
    )
    .await
}

// ---------------------------------------------------------------------
// Memory (workspace + global Markdown notes, Obsidian-style)

pub async fn workspace_ensure_agents(ws: &str) -> Result<(), String> {
    invoke_typed("workspace_ensure_agents", WsArg { workspace_cwd: ws }).await
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsLayoutStatus {
    pub missing_dirs: Vec<String>,
    pub missing_files: Vec<String>,
}

impl AgentsLayoutStatus {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.missing_dirs.is_empty() && self.missing_files.is_empty()
    }
}

pub async fn workspace_agents_layout_status(ws: &str) -> Result<AgentsLayoutStatus, String> {
    invoke_typed(
        "workspace_agents_layout_status",
        WsArg { workspace_cwd: ws },
    )
    .await
}

// ── Scope ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScope {
    Workspace,
    Global,
}

/// Composite key used as node ID and for addressing notes across scopes.
pub fn note_key(scope: &MemoryScope, path: &str) -> String {
    let s = match scope {
        MemoryScope::Workspace => "workspace",
        MemoryScope::Global => "global",
    };
    format!("{s}:{path}")
}

pub fn parse_note_key(key: &str) -> Option<(MemoryScope, String)> {
    let idx = key.find(':')?;
    if idx == 0 {
        return None;
    }
    let scope = match &key[..idx] {
        "workspace" => MemoryScope::Workspace,
        "global" => MemoryScope::Global,
        _ => return None,
    };
    Some((scope, key[idx + 1..].to_owned()))
}

// ── Types ─────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    pub scope: MemoryScope,
    pub path: String,
    pub name: String,
    pub title: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub size: u64,
    pub modified: i64,
    pub is_template: bool,
    pub is_learning: bool,
    pub is_overview: bool,
    pub category: String,
    pub managed: Option<String>,
    pub stale: Option<bool>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteContent {
    pub scope: MemoryScope,
    pub path: String,
    pub content: String,
    pub modified: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub scope: MemoryScope,
    pub path: String,
    pub label: String,
    pub tags: Vec<String>,
    pub orphan: bool,
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_category_hub: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hub_scopes: Option<Vec<MemoryScope>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub cross_scope: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub scope: MemoryScope,
    pub path: String,
    pub line: u32,
    pub snippet: String,
    pub category: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BacklinkRef {
    pub scope: MemoryScope,
    pub path: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFolderStatus {
    pub memory: bool,
    pub learnings: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStatusResponse {
    pub workspace: MemoryFolderStatus,
    pub global: MemoryFolderStatus,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySubcategories {
    pub workspace: Vec<String>,
    pub global: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryListResponse {
    pub notes: Vec<NoteMeta>,
    pub memory_subcategories: MemorySubcategories,
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

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReport {
    pub git_rev: Option<String>,
    pub crate_count: u32,
    pub unit_count: u32,
    pub module_count: u32,
    pub files_changed: u32,
    pub kinds: Vec<String>,
    pub warnings: Vec<String>,
    pub generated_paths: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchitectureLintReport {
    pub git_rev: Option<String>,
    pub state_git_rev: Option<String>,
    pub stale: bool,
    pub stale_paths: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WsArg<'a> {
    workspace_cwd: &'a str,
}

// ── Commands ──────────────────────────────────────────────────────────────────

pub async fn memory_status(ws: &str) -> Result<MemoryStatusResponse, String> {
    invoke_typed("memory_status", WsArg { workspace_cwd: ws }).await
}

pub async fn memory_bootstrap(ws: &str, target: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        target: &'a str,
    }
    invoke_unit_js(
        "memory_bootstrap",
        args_value(A {
            workspace_cwd: ws,
            target,
        })?,
    )
    .await
}

pub async fn memory_rebuild_architecture(ws: &str) -> Result<RebuildReport, String> {
    invoke_typed("memory_rebuild_architecture", WsArg { workspace_cwd: ws }).await
}

#[allow(dead_code)]
pub async fn memory_lint_architecture(ws: &str) -> Result<ArchitectureLintReport, String> {
    invoke_typed("memory_lint_architecture", WsArg { workspace_cwd: ws }).await
}

pub async fn memory_list(ws: &str) -> Result<MemoryListResponse, String> {
    invoke_typed("memory_list", WsArg { workspace_cwd: ws }).await
}

pub async fn memory_read(ws: &str, scope: &MemoryScope, path: &str) -> Result<NoteContent, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        path: &'a str,
    }
    invoke_typed(
        "memory_read",
        A {
            workspace_cwd: ws,
            scope,
            path,
        },
    )
    .await
}

pub async fn memory_write(
    ws: &str,
    scope: &MemoryScope,
    path: &str,
    content: &str,
) -> Result<NoteContent, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        path: &'a str,
        content: &'a str,
    }
    invoke_typed(
        "memory_write",
        A {
            workspace_cwd: ws,
            scope,
            path,
            content,
        },
    )
    .await
}

pub async fn memory_create(
    ws: &str,
    scope: &MemoryScope,
    path: &str,
    content: Option<&str>,
) -> Result<NoteMeta, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        path: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<&'a str>,
    }
    invoke_typed(
        "memory_create",
        A {
            workspace_cwd: ws,
            scope,
            path,
            content,
        },
    )
    .await
}

pub async fn memory_create_category(
    ws: &str,
    scope: &MemoryScope,
    name: &str,
) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        name: &'a str,
    }
    invoke_typed(
        "memory_create_category",
        A {
            workspace_cwd: ws,
            scope,
            name,
        },
    )
    .await
}

pub async fn memory_delete(ws: &str, scope: &MemoryScope, path: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        path: &'a str,
    }
    invoke_unit_js(
        "memory_delete",
        args_value(A {
            workspace_cwd: ws,
            scope,
            path,
        })?,
    )
    .await
}

pub async fn memory_rename(
    ws: &str,
    scope: &MemoryScope,
    old_path: &str,
    new_path: &str,
    rewrite_links: bool,
) -> Result<RenameReport, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        old_path: &'a str,
        new_path: &'a str,
        rewrite_links: bool,
    }
    invoke_typed(
        "memory_rename",
        A {
            workspace_cwd: ws,
            scope,
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

pub async fn memory_backlinks(
    ws: &str,
    scope: &MemoryScope,
    path: &str,
) -> Result<Vec<BacklinkRef>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        scope: &'a MemoryScope,
        path: &'a str,
    }
    invoke_typed(
        "memory_backlinks",
        A {
            workspace_cwd: ws,
            scope,
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

pub async fn memory_pointer_status(ws: &str) -> Result<Vec<PointerResult>, String> {
    invoke_typed("memory_pointer_status", WsArg { workspace_cwd: ws }).await
}

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

pub async fn memory_uninstall_pointers(
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
        "memory_uninstall_pointers",
        A {
            workspace_cwd: ws,
            agents,
        },
    )
    .await
}

pub async fn rules_pointer_status(ws: &str) -> Result<Vec<PointerResult>, String> {
    invoke_typed("rules_pointer_status", WsArg { workspace_cwd: ws }).await
}

pub async fn rules_install_pointers(
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
        "rules_install_pointers",
        A {
            workspace_cwd: ws,
            agents,
        },
    )
    .await
}

pub async fn rules_uninstall_pointers(
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
        "rules_uninstall_pointers",
        A {
            workspace_cwd: ws,
            agents,
        },
    )
    .await
}

// ---------------------------------------------------------------------
// Plans (workspace-scoped Markdown plans under .agents/plans/)

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskSummaryWire {
    pub total: u32,
    pub pending: u32,
    pub in_progress: u32,
    pub blocked: u32,
    pub completed: u32,
    pub cancelled: u32,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMeta {
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub folder_path: String,
    pub title: String,
    pub size: u64,
    pub modified: i64,
    pub is_index: bool,
    pub task_summary: PlanTaskSummaryWire,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanContent {
    pub path: String,
    pub content: String,
    #[allow(dead_code)]
    pub modified: i64,
    pub is_index: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLoadReport {
    pub path: String,
    pub tasks_replaced: u32,
    pub tasks_added: u32,
    pub free_tasks_kept: u32,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSyncReport {
    pub path: String,
    pub tasks_written: u32,
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanMigrationProgress {
    pub phase: String,
    pub busy: bool,
    pub total: u32,
    pub processed: u32,
    pub migrated: u32,
    pub skipped: u32,
    pub error: Option<String>,
    pub updated_at_ms: u64,
}

pub async fn plan_list(ws: &str) -> Result<Vec<PlanMeta>, String> {
    invoke_typed("plan_list", WsArg { workspace_cwd: ws }).await
}

// ---------------------------------------------------------------------------
// Mermaid diagrams (mirrors `src-tauri/src/agent/mermaid/`)
// ---------------------------------------------------------------------------

/// One stored diagram with its Mermaid source. Mirrors
/// `agent::mermaid::store::DiagramRecord` (flattened metadata).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramRecord {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub task_id: Option<String>,
    pub created_ms: u64,
    pub code: String,
    /// Provider/model that generated the diagram (absent for older files).
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// One diagram as returned inline by the `mermaid_create` / `mermaid_create_many`
/// agent tools. Mirrors the backend `DiagramOut` envelope
/// (`src-tauri/src/agent/mermaid/tool.rs`). Unlike [`DiagramRecord`] these may be
/// ephemeral (`persisted == false`, no `plan_slug`); they are embedded into an
/// opened center tab, hence `Serialize`/`Deserialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineDiagram {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub kind: String,
    pub code: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub plan_slug: Option<String>,
    #[serde(default)]
    pub persisted: bool,
    /// Provider/model that generated the diagram (when recorded by the backend).
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TimelineDiagramsEnvelope {
    diagrams: Vec<TimelineDiagram>,
}

/// Parse the JSON `content` of a `mermaid_create*` tool result into its diagram
/// list. Returns `None` when the text is not a diagrams envelope (e.g. an error
/// string), so callers can fall back to the raw detail view.
#[must_use]
pub fn parse_timeline_diagrams(detail: &str) -> Option<Vec<TimelineDiagram>> {
    let env: TimelineDiagramsEnvelope = serde_json::from_str(detail.trim()).ok()?;
    if env.diagrams.is_empty() {
        return None;
    }
    Some(env.diagrams)
}

pub async fn mermaid_list_diagrams(ws: &str, slug: &str) -> Result<Vec<DiagramRecord>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        workspace_cwd: &'a str,
        slug: &'a str,
    }
    invoke_typed(
        "mermaid_list_diagrams",
        Args {
            workspace_cwd: ws,
            slug,
        },
    )
    .await
}

pub async fn mermaid_delete_diagram(ws: &str, slug: &str, id: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        workspace_cwd: &'a str,
        slug: &'a str,
        id: &'a str,
    }
    invoke_unit_js(
        "mermaid_delete_diagram",
        args_value(Args {
            workspace_cwd: ws,
            slug,
            id,
        })?,
    )
    .await
}

pub async fn mermaid_update_diagram(
    ws: &str,
    slug: &str,
    id: &str,
    code: &str,
) -> Result<DiagramRecord, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args<'a> {
        workspace_cwd: &'a str,
        slug: &'a str,
        id: &'a str,
        code: &'a str,
    }
    invoke_typed(
        "mermaid_update_diagram",
        Args {
            workspace_cwd: ws,
            slug,
            id,
            code,
        },
    )
    .await
}

/// Export a diagram as Markdown via a native Save dialog. `Ok(None)` on cancel.
pub async fn mermaid_export_markdown(
    title: &str,
    kind: &str,
    code: &str,
    landscape: bool,
) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    struct Args<'a> {
        title: &'a str,
        kind: &'a str,
        code: &'a str,
        landscape: bool,
    }
    invoke_typed(
        "mermaid_export_markdown",
        Args {
            title,
            kind,
            code,
            landscape,
        },
    )
    .await
}

/// Export a diagram as PDF from its rendered SVG. `Ok(None)` on cancel.
pub async fn mermaid_export_pdf(title: &str, svg: &str) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    struct Args<'a> {
        title: &'a str,
        svg: &'a str,
    }
    invoke_typed("mermaid_export_pdf", Args { title, svg }).await
}

pub async fn plan_migration_ensure_started(ws: &str) -> Result<PlanMigrationProgress, String> {
    invoke_typed("plan_migration_ensure_started", WsArg { workspace_cwd: ws }).await
}

pub async fn plan_migration_poll(ws: &str) -> Result<PlanMigrationProgress, String> {
    invoke_typed("plan_migration_poll", WsArg { workspace_cwd: ws }).await
}

pub async fn plan_read(ws: &str, path: &str) -> Result<PlanContent, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_typed(
        "plan_read",
        A {
            workspace_cwd: ws,
            path,
        },
    )
    .await
}

pub async fn plan_write(ws: &str, path: &str, content: &str) -> Result<PlanContent, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
        content: &'a str,
    }
    invoke_typed(
        "plan_write",
        A {
            workspace_cwd: ws,
            path,
            content,
        },
    )
    .await
}

pub async fn plan_create(ws: &str, path: &str, content: Option<&str>) -> Result<PlanMeta, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<&'a str>,
    }
    invoke_typed(
        "plan_create",
        A {
            workspace_cwd: ws,
            path,
            content,
        },
    )
    .await
}

pub async fn plan_delete(ws: &str, path: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_unit_js(
        "plan_delete",
        args_value(A {
            workspace_cwd: ws,
            path,
        })?,
    )
    .await
}

pub async fn plan_rename(ws: &str, old_path: &str, new_path: &str) -> Result<PlanMeta, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        old_path: &'a str,
        new_path: &'a str,
    }
    invoke_typed(
        "plan_rename",
        A {
            workspace_cwd: ws,
            old_path,
            new_path,
        },
    )
    .await
}

pub async fn plan_load(ws: &str, path: &str) -> Result<PlanLoadReport, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_typed(
        "plan_load",
        A {
            workspace_cwd: ws,
            path,
        },
    )
    .await
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct GeneratedPlan {
    pub title: String,
    pub markdown: String,
}

/// Generates a Skill-conformant plan (and optionally tasks) from a prompt via
/// the agent tab's configured provider. Returns the title + cleaned Markdown.
pub async fn plan_generate_ai(prompt: String, with_tasks: bool) -> Result<GeneratedPlan, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        prompt: String,
        with_tasks: bool,
    }
    invoke_typed("plan_generate_ai", Args { prompt, with_tasks }).await
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedPrompt {
    pub prompt: String,
}

/// Enhances a draft prompt through an isolated one-shot provider request. This
/// does not mutate the Agent chat session, timeline, tasks, tools, or memory.
pub async fn agent_enhance_prompt(prompt: String) -> Result<EnhancedPrompt, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        prompt: String,
    }
    invoke_typed("agent_enhance_prompt", Args { prompt }).await
}

#[allow(dead_code)]
pub async fn plan_sync_from_tasks(ws: &str, path: &str) -> Result<PlanSyncReport, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        path: &'a str,
    }
    invoke_typed(
        "plan_sync_from_tasks",
        A {
            workspace_cwd: ws,
            path,
        },
    )
    .await
}

// ---------------------------------------------------------------------
// Workspace Multi-Kanban (layout metadata + plan task mutations)

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KanbanPlanState {
    Blocked,
    InProgress,
    Pending,
    Completed,
    Cancelled,
    Empty,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanFilters {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub show_completed: bool,
    #[serde(default)]
    pub show_cancelled: bool,
    #[serde(default)]
    pub active_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanLayout {
    pub version: u32,
    #[serde(default)]
    pub workspace_root: Option<String>,
    pub plan_section_order: Vec<KanbanPlanState>,
    pub collapsed_plan_sections: Vec<KanbanPlanState>,
    pub expanded_plans: Vec<String>,
    pub task_lane_order: Vec<TaskStatus>,
    pub collapsed_task_lanes: Vec<TaskStatus>,
    pub plan_order: std::collections::BTreeMap<String, u32>,
    pub task_order: std::collections::BTreeMap<String, u32>,
    pub filters: KanbanFilters,
    pub updated_at: i64,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanBoard {
    pub layout: KanbanLayout,
    pub plans: Vec<KanbanPlanNode>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanPlanNode {
    pub meta: PlanMeta,
    pub state: KanbanPlanState,
    pub tasks: Vec<KanbanTaskCard>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskCard {
    pub plan_path: String,
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub runtime_task_id: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskCreateInput {
    pub plan_path: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskUpdatePatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanPlanMoveInput {
    pub plan_path: String,
    pub target_state: KanbanPlanState,
    pub ordered_plan_paths: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanTaskMoveInput {
    pub plan_path: String,
    pub task_id: String,
    pub target_status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_task_id: Option<String>,
}

pub async fn kanban_board_load(ws: &str) -> Result<KanbanBoard, String> {
    invoke_typed("kanban_board_load", WsArg { workspace_cwd: ws }).await
}

pub async fn kanban_layout_save(ws: &str, layout: KanbanLayout) -> Result<KanbanLayout, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        layout: KanbanLayout,
    }
    invoke_typed(
        "kanban_layout_save",
        A {
            workspace_cwd: ws,
            layout,
        },
    )
    .await
}

pub async fn kanban_task_create(
    ws: &str,
    input: KanbanTaskCreateInput,
) -> Result<KanbanTaskCard, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        input: KanbanTaskCreateInput,
    }
    invoke_typed(
        "kanban_task_create",
        A {
            workspace_cwd: ws,
            input,
        },
    )
    .await
}

pub async fn kanban_task_update(
    ws: &str,
    plan_path: &str,
    task_id: &str,
    patch: KanbanTaskUpdatePatch,
) -> Result<KanbanTaskCard, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        plan_path: &'a str,
        task_id: &'a str,
        patch: KanbanTaskUpdatePatch,
    }
    invoke_typed(
        "kanban_task_update",
        A {
            workspace_cwd: ws,
            plan_path,
            task_id,
            patch,
        },
    )
    .await
}

pub async fn kanban_task_delete(ws: &str, plan_path: &str, task_id: &str) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        plan_path: &'a str,
        task_id: &'a str,
    }
    invoke_unit_js(
        "kanban_task_delete",
        args_value(A {
            workspace_cwd: ws,
            plan_path,
            task_id,
        })?,
    )
    .await
}

pub async fn kanban_plan_move(ws: &str, input: KanbanPlanMoveInput) -> Result<KanbanBoard, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        input: KanbanPlanMoveInput,
    }
    invoke_typed(
        "kanban_plan_move",
        A {
            workspace_cwd: ws,
            input,
        },
    )
    .await
}

pub async fn kanban_task_move(
    ws: &str,
    input: KanbanTaskMoveInput,
) -> Result<KanbanTaskCard, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        input: KanbanTaskMoveInput,
    }
    invoke_typed(
        "kanban_task_move",
        A {
            workspace_cwd: ws,
            input,
        },
    )
    .await
}

pub async fn kanban_export_layout(ws: &str) -> Result<String, String> {
    invoke_typed("kanban_export_layout", WsArg { workspace_cwd: ws }).await
}

pub async fn kanban_import_layout(ws: &str, json: &str) -> Result<KanbanLayout, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct A<'a> {
        workspace_cwd: &'a str,
        json: &'a str,
    }
    invoke_typed(
        "kanban_import_layout",
        A {
            workspace_cwd: ws,
            json,
        },
    )
    .await
}

// ---------------------------------------------------------------------
// Skills & Rules

/// Idempotently creates `.agents/{rules,skills}/` and their `index.json`
/// manifests, importing any pre-existing files as `enabled: true`. Safe
/// to call on every workspace activation.
pub async fn skills_rules_bootstrap(ws: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
    }
    invoke_unit_js("skills_rules_bootstrap", args_value(A { ws })?).await
}

pub async fn rules_list(ws: String) -> Result<Vec<RuleEntry>, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
    }
    invoke_typed("rules_list", A { ws }).await
}

#[allow(dead_code)]
pub async fn rules_read(ws: String, name: String) -> Result<String, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
    }
    invoke_typed("rules_read", A { ws, name }).await
}

#[allow(dead_code)]
pub async fn rules_write(ws: String, name: String, content: String) -> Result<RuleEntry, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
        content: String,
    }
    invoke_typed("rules_write", A { ws, name, content }).await
}

pub async fn rules_set_enabled(
    ws: String,
    name: String,
    enabled: bool,
) -> Result<RuleEntry, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
        enabled: bool,
    }
    invoke_typed("rules_set_enabled", A { ws, name, enabled }).await
}

pub async fn rules_remove(ws: String, name: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
    }
    invoke_unit_js("rules_remove", args_value(A { ws, name })?).await
}

pub async fn skills_list(ws: String) -> Result<Vec<SkillEntry>, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
    }
    invoke_typed("skills_list", A { ws }).await
}

#[allow(dead_code)]
pub async fn skills_read(ws: String, name: String) -> Result<String, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
    }
    invoke_typed("skills_read", A { ws, name }).await
}

#[allow(dead_code)]
pub async fn skills_write(ws: String, name: String, content: String) -> Result<SkillEntry, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
        content: String,
    }
    invoke_typed("skills_write", A { ws, name, content }).await
}

pub async fn skills_set_enabled(
    ws: String,
    name: String,
    enabled: bool,
) -> Result<SkillEntry, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
        enabled: bool,
    }
    invoke_typed("skills_set_enabled", A { ws, name, enabled }).await
}

pub async fn skills_remove(ws: String, name: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
    }
    invoke_unit_js("skills_remove", args_value(A { ws, name })?).await
}

pub async fn skills_install(
    ws: String,
    name: String,
    source: SkillSourceInput,
) -> Result<SkillEntry, String> {
    #[derive(Serialize)]
    struct A {
        ws: String,
        name: String,
        source: SkillSourceInput,
    }
    invoke_typed("skills_install", A { ws, name, source }).await
}

// ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginCapability {
    RunCommands,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginInstallKind {
    BuiltIn,
    GitHub,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommandContribution {
    pub capability: PluginCapability,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub commands: Vec<PluginCommandContribution>,
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallSource {
    pub kind: PluginInstallKind,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub git_ref: Option<String>,
    #[serde(default)]
    pub package_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistryEntry {
    pub manifest: PluginManifest,
    #[serde(default)]
    pub enabled: bool,
    pub source: PluginInstallSource,
    #[serde(default)]
    pub installed_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub path: Option<String>,
}

impl PluginRegistryEntry {
    pub fn removable(&self) -> bool {
        self.source.kind != PluginInstallKind::BuiltIn
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRegistry {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub plugins: Vec<PluginRegistryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallRequest {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInstallProgress {
    pub busy: bool,
    pub phase: String,
    pub message: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub plugin_id: Option<String>,
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunCommandKind {
    Dev,
    Run,
    Debug,
    Test,
    Build,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommandSource {
    pub plugin_id: String,
    pub detector_id: String,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub package_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommand {
    pub id: String,
    pub label: String,
    pub command: String,
    #[serde(default)]
    pub cwd_rel: String,
    pub kind: RunCommandKind,
    pub source: RunCommandSource,
}

pub async fn plugins_list() -> Result<PluginRegistry, String> {
    invoke_typed("plugins_list", serde_json::json!({})).await
}

pub async fn plugins_install_from_github(
    request: PluginInstallRequest,
) -> Result<PluginRegistry, String> {
    #[derive(Serialize)]
    struct Args {
        request: PluginInstallRequest,
    }
    invoke_typed("plugins_install_from_github", Args { request }).await
}

pub async fn plugins_install_progress() -> Result<PluginInstallProgress, String> {
    invoke_typed("plugins_install_progress", serde_json::json!({})).await
}

pub async fn plugins_set_enabled(
    plugin_id: String,
    enabled: bool,
) -> Result<PluginRegistry, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        plugin_id: String,
        enabled: bool,
    }
    invoke_typed("plugins_set_enabled", Args { plugin_id, enabled }).await
}

pub async fn plugins_remove(plugin_id: String) -> Result<PluginRegistry, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        plugin_id: String,
    }
    invoke_typed("plugins_remove", Args { plugin_id }).await
}

pub async fn run_commands_discover(
    workspace_root: String,
    connection_id: Option<String>,
) -> Result<Vec<RunCommand>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Request {
        workspace_root: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    #[derive(Serialize)]
    struct Args {
        request: Request,
    }
    invoke_typed(
        "run_commands_discover",
        Args {
            request: Request {
                workspace_root,
                connection_id,
            },
        },
    )
    .await
}

// ---------------------------------------------------------------------

pub async fn git_branch(
    cwd: String,
    connection_id: Option<String>,
) -> Result<Option<String>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_branch", Args { cwd, connection_id }).await
}

pub async fn git_is_repository(cwd: String, connection_id: Option<String>) -> Result<bool, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_is_repository", Args { cwd, connection_id }).await
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRefDecoration {
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitNode {
    pub oid: String,
    pub short_oid: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub body: String,
    pub author: String,
    pub author_email: String,
    pub author_time: String,
    pub rel_time: String,
    pub decorations: Vec<GitRefDecoration>,
    pub files_changed: Option<u32>,
    pub insertions: Option<u32>,
    pub deletions: Option<u32>,
    pub remote_url: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphEdge {
    pub from_lane: usize,
    pub to_lane: usize,
    pub color_index: usize,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphEntry {
    pub row_index: usize,
    pub lane: usize,
    pub lanes: usize,
    pub active_lanes: Vec<usize>,
    pub edges: Vec<GitGraphEdge>,
    pub is_merge: bool,
    pub is_head: bool,
    pub commit: GitCommitNode,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitGraphLayout {
    pub entries: Vec<GitGraphEntry>,
    pub lane_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitFileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub added: Option<u32>,
    pub removed: Option<u32>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitDetails {
    pub oid: String,
    pub short_oid: String,
    pub subject: String,
    pub body: String,
    pub author: String,
    pub author_email: String,
    pub author_time: String,
    pub rel_time: String,
    pub decorations: Vec<GitRefDecoration>,
    pub files_changed: u32,
    pub insertions: u32,
    pub deletions: u32,
    pub remote_url: Option<String>,
    pub files: Vec<GitCommitFileChange>,
}

pub const GIT_MISSING_CODE: &str = "git_missing";

pub async fn git_commit_graph(
    cwd: String,
    limit: Option<u32>,
    connection_id: Option<String>,
) -> Result<GitGraphLayout, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        limit: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "git_commit_graph",
        Args {
            cwd,
            limit,
            connection_id,
        },
    )
    .await
}

pub async fn git_commit_details(
    cwd: String,
    oid: String,
    connection_id: Option<String>,
) -> Result<GitCommitDetails, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        oid: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "git_commit_details",
        Args {
            cwd,
            oid,
            connection_id,
        },
    )
    .await
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeEntry {
    pub path: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
    pub locked_reason: Option<String>,
    pub prunable: bool,
    pub prunable_reason: Option<String>,
    pub is_main: bool,
    pub git_common_dir: Option<String>,
    pub main_worktree_cwd: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeCreateOutcome {
    pub entry: GitWorktreeEntry,
    pub created: bool,
    pub matched_existing: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreeRemoveOutcome {
    pub path: String,
    pub removed: bool,
}

pub async fn git_worktree_list(
    cwd: String,
    connection_id: Option<String>,
) -> Result<Vec<GitWorktreeEntry>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_worktree_list", Args { cwd, connection_id }).await
}

pub async fn git_worktree_open_info(
    cwd: String,
    connection_id: Option<String>,
) -> Result<GitWorktreeEntry, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_worktree_open_info", Args { cwd, connection_id }).await
}

pub async fn git_worktree_create(
    base_cwd: String,
    branch: String,
    start_point: Option<String>,
    path: String,
    connection_id: Option<String>,
) -> Result<GitWorktreeCreateOutcome, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        base_cwd: String,
        branch: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        start_point: Option<String>,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "git_worktree_create",
        Args {
            base_cwd,
            branch,
            start_point,
            path,
            connection_id,
        },
    )
    .await
}

pub async fn git_worktree_remove(
    base_cwd: String,
    path: String,
    connection_id: Option<String>,
) -> Result<GitWorktreeRemoveOutcome, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        base_cwd: String,
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "git_worktree_remove",
        Args {
            base_cwd,
            path,
            connection_id,
        },
    )
    .await
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineStats {
    pub added: u32,
    pub removed: u32,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub rel_path: String,
    /// `"modified" | "added" | "deleted" | "renamed" | "untracked" | "conflicted"`.
    pub status: String,
    pub staged: bool,
    pub unstaged: bool,
    pub staged_stats: Option<LineStats>,
    pub unstaged_stats: Option<LineStats>,
}

pub async fn git_status_changes(
    cwd: String,
    connection_id: Option<String>,
) -> Result<Vec<ChangedFile>, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_status_changes", Args { cwd, connection_id }).await
}

pub async fn git_file_diff(
    cwd: String,
    rel_path: String,
    staged: bool,
    connection_id: Option<String>,
) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        rel_path: String,
        staged: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "git_file_diff",
        Args {
            cwd,
            rel_path,
            staged,
            connection_id,
        },
    )
    .await
}

pub async fn git_stage_file(
    cwd: String,
    rel_path: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        rel_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js(
        "git_stage_file",
        args_value(Args {
            cwd,
            rel_path,
            connection_id,
        })?,
    )
    .await
}

pub async fn git_unstage_file(
    cwd: String,
    rel_path: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        rel_path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js(
        "git_unstage_file",
        args_value(Args {
            cwd,
            rel_path,
            connection_id,
        })?,
    )
    .await
}

pub async fn git_stage_all(cwd: String, connection_id: Option<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js("git_stage_all", args_value(Args { cwd, connection_id })?).await
}

pub async fn git_unstage_all(cwd: String, connection_id: Option<String>) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js("git_unstage_all", args_value(Args { cwd, connection_id })?).await
}

pub async fn git_commit(
    cwd: String,
    message: String,
    connection_id: Option<String>,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_unit_js(
        "git_commit",
        args_value(Args {
            cwd,
            message,
            connection_id,
        })?,
    )
    .await
}

/// Generates a commit message from the staged diff via the agent tab's
/// configured provider. Returns the message text (already cleaned).
pub async fn git_generate_commit_message(
    cwd: String,
    connection_id: Option<String>,
) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_generate_commit_message", Args { cwd, connection_id }).await
}

pub async fn git_status_watch_start(
    cwd: String,
    connection_id: Option<String>,
) -> Result<u64, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_status_watch_start", Args { cwd, connection_id }).await
}

pub async fn git_status_watch_stop(token: u64) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        token: u64,
    }
    invoke_unit_js("git_status_watch_stop", args_value(Args { token })?).await
}

/// Mirrors `git_sync::SyncStatus`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub has_remote: bool,
    pub detached: bool,
    pub dirty: bool,
}

/// Mirrors `git_sync::SyncOutcome`. `kind` is a stable code (see backend doc).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub kind: String,
    pub detail: String,
}

pub async fn git_sync_status(
    cwd: String,
    connection_id: Option<String>,
) -> Result<SyncStatus, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_sync_status", Args { cwd, connection_id }).await
}

pub async fn git_fetch(cwd: String, connection_id: Option<String>) -> Result<SyncOutcome, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_fetch", Args { cwd, connection_id }).await
}

pub async fn git_pull(cwd: String, connection_id: Option<String>) -> Result<SyncOutcome, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed("git_pull", Args { cwd, connection_id }).await
}

pub async fn git_push(
    cwd: String,
    set_upstream: bool,
    connection_id: Option<String>,
) -> Result<SyncOutcome, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        cwd: String,
        set_upstream: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        connection_id: Option<String>,
    }
    invoke_typed(
        "git_push",
        Args {
            cwd,
            set_upstream,
            connection_id,
        },
    )
    .await
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatusDirtyPayload {
    /// The repo cwd that triggered the event. Listeners with multiple
    /// active watchers compare this against their own state to decide
    /// whether to act; the sidebar today only watches one repo at a time.
    #[allow(dead_code)]
    pub cwd: String,
}

/// Subscribes to the backend `git_status_dirty` window event. The returned
/// `TauriEventListener` removes the underlying listener on `Drop`. Returns
/// `None` outside a Tauri shell or when the event API is unavailable.
pub fn listen_git_status_dirty(
    callback: impl FnMut(GitStatusDirtyPayload) + 'static,
) -> Option<TauriEventListener> {
    listen_tauri_event::<GitStatusDirtyPayload>("git_status_dirty", callback)
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"], js_name = listen)]
    async fn tauri_listen_raw(event: &str, callback: &js_sys::Function)
        -> Result<JsValue, JsValue>;
}

/// RAII handle returned by [`listen_tauri_event`]. Drop calls the
/// asynchronous unlisten function returned by `tauri.event.listen`. We
/// store the JS callback inside so it stays alive (forgetting the closure
/// would leak it forever; using `into_js_value` is essentially the same
/// once the unlisten fires, but we keep the binding scoped here).
pub struct TauriEventListener {
    unlisten: std::rc::Rc<std::cell::RefCell<Option<js_sys::Function>>>,
    _callback: send_wrapper::SendWrapper<wasm_bindgen::closure::Closure<dyn FnMut(JsValue)>>,
}

impl Drop for TauriEventListener {
    fn drop(&mut self) {
        if let Some(unlisten) = self.unlisten.borrow_mut().take() {
            let _ = unlisten.call0(&JsValue::NULL);
        }
    }
}

fn listen_tauri_event<T>(
    event: &'static str,
    mut callback: impl FnMut(T) + 'static,
) -> Option<TauriEventListener>
where
    T: DeserializeOwned + 'static,
{
    if !is_tauri_shell() {
        return None;
    }

    let cb = wasm_bindgen::closure::Closure::wrap(Box::new(move |js_event: JsValue| {
        let payload =
            js_sys::Reflect::get(&js_event, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
        if payload.is_null() || payload.is_undefined() {
            return;
        }
        if let Ok(parsed) = serde_wasm_bindgen::from_value::<T>(payload) {
            callback(parsed);
        }
    }) as Box<dyn FnMut(JsValue)>);
    let cb_function = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
    let unlisten_slot: std::rc::Rc<std::cell::RefCell<Option<js_sys::Function>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));
    let unlisten_slot_for_promise = unlisten_slot.clone();
    let event_name = event.to_string();
    wasm_bindgen_futures::spawn_local(async move {
        if let Ok(unlisten) = tauri_listen_raw(&event_name, &cb_function).await {
            if let Ok(func) = unlisten.dyn_into::<js_sys::Function>() {
                *unlisten_slot_for_promise.borrow_mut() = Some(func);
            }
        }
    });

    Some(TauriEventListener {
        unlisten: unlisten_slot,
        _callback: send_wrapper::SendWrapper::new(cb),
    })
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
pub async fn agent_drain_turn(on_batch: impl Fn(Vec<EventEnvelope>)) -> Result<(), String> {
    agent_drain_turn_opts(None, false, on_batch).await
}

/// Variante mit `expect_voice`: drain läuft nach `Done` weiter, bis ein
/// `VoiceReady` oder ein zusätzlicher `Error` ankommt (max. ~30 s leerlauf).
/// Damit holen wir den TTS-Output, der vom Orchestrator nach dem
/// regulären `Done` gepusht wird.
pub async fn agent_drain_turn_opts(
    session_id: Option<String>,
    expect_voice: bool,
    on_batch: impl Fn(Vec<EventEnvelope>),
) -> Result<(), String> {
    let mut seen_done = false;
    let mut idle_after_done: u32 = 0;
    const VOICE_TAIL_IDLE_MAX: u32 = 600; // 600 * 50ms ≈ 30s
    loop {
        let batch = agent_poll_events(session_id.clone(), 64).await?;
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
            .any(|e| matches!(e.event, AgentEvent::Done | AgentEvent::Error { .. }));
        let has_voice = batch
            .iter()
            .any(|e| matches!(e.event, AgentEvent::VoiceReady { .. }));
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
    Aws,
}

impl VoiceProviderKind {
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
            Self::Aws => "aws",
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PttMode {
    Local,
    Cloud,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WhisperQuality {
    Fast,
    Balanced,
    Best,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PttInsertTarget {
    Agent,
    Terminal,
    ActiveInput,
    Clipboard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PttTargetMode {
    CurrentFocus,
    RememberStart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TtsCollision {
    Stop,
    Pause,
    Block,
}

/// Mirror of the backend `PttSettings` (see `src-tauri/src/voice/settings.rs`).
/// Every field carries `#[serde(default)]` so older envelopes load cleanly.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PttSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "ptt_mode_default")]
    pub mode: PttMode,
    #[serde(default)]
    pub local_model_path: Option<String>,
    #[serde(default = "ptt_quality_default")]
    pub local_quality: WhisperQuality,
    #[serde(default = "ptt_provider_default")]
    pub cloud_provider: VoiceProviderKind,
    #[serde(default = "ptt_cloud_model_default")]
    pub cloud_model_id: String,
    #[serde(default = "ptt_insert_default")]
    pub insert_target: PttInsertTarget,
    #[serde(default = "ptt_target_mode_default")]
    pub target_mode: PttTargetMode,
    #[serde(default)]
    pub auto_submit: bool,
    #[serde(default = "ptt_true")]
    pub partial_transcript: bool,
    #[serde(default = "ptt_collision_default")]
    pub tts_collision: TtsCollision,
}

fn ptt_mode_default() -> PttMode {
    PttMode::Local
}
fn ptt_quality_default() -> WhisperQuality {
    WhisperQuality::Balanced
}
fn ptt_provider_default() -> VoiceProviderKind {
    VoiceProviderKind::Openai
}
fn ptt_cloud_model_default() -> String {
    "gpt-4o-mini-transcribe".into()
}
fn ptt_insert_default() -> PttInsertTarget {
    PttInsertTarget::Agent
}
fn ptt_target_mode_default() -> PttTargetMode {
    PttTargetMode::CurrentFocus
}
fn ptt_collision_default() -> TtsCollision {
    TtsCollision::Block
}
fn ptt_true() -> bool {
    true
}

impl Default for PttSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: ptt_mode_default(),
            local_model_path: None,
            local_quality: ptt_quality_default(),
            cloud_provider: ptt_provider_default(),
            cloud_model_id: ptt_cloud_model_default(),
            insert_target: ptt_insert_default(),
            target_mode: ptt_target_mode_default(),
            auto_submit: false,
            partial_transcript: true,
            tts_collision: ptt_collision_default(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettings {
    pub stt: SttSettings,
    pub tts: TtsSettings,
    pub post_stt_flow: PostSttFlow,
    pub stt_language: SttLanguageMode,
    pub ptt_hotkey: PttHotkey,
    #[serde(default)]
    pub ptt: PttSettings,
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
            ptt: PttSettings::default(),
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

// ---------------------------------------------------------------------------
// Push-to-talk + whisper model manager bridge
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PttStartResponse {
    pub turn_id: Option<String>,
    pub started: bool,
    /// "start" | "stopTts" | "pauseTts" | "rejectBusy" | "rejectTtsPlaying"
    pub decision: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelFamily {
    Standard,
    Quantized,
    Turbo,
    Large,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelView {
    pub id: String,
    pub label: String,
    pub family: ModelFamily,
    pub multilingual: bool,
    pub size_bytes: u64,
    pub speed_rating: u8,
    pub accuracy_rating: u8,
    pub best_for: String,
    pub installed: bool,
    pub installed_path: Option<String>,
    pub partial_bytes: Option<u64>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperDownloadProgress {
    pub id: String,
    pub received: u64,
    pub total: u64,
    pub speed_bps: f64,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperDownloadDone {
    pub id: String,
    /// Final installed path (informational; the UI reloads the list instead).
    #[allow(dead_code)]
    pub path: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperDownloadError {
    pub id: String,
    pub message: String,
}

pub async fn ptt_start() -> Result<PttStartResponse, String> {
    invoke_typed("ptt_start", serde_json::json!({})).await
}

pub async fn ptt_partial(turn_id: String, locale_hint: Option<String>) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        turn_id: String,
        locale_hint: Option<String>,
    }
    invoke_typed(
        "ptt_partial",
        Args {
            turn_id,
            locale_hint,
        },
    )
    .await
}

pub async fn ptt_finalize(turn_id: String, locale_hint: Option<String>) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        turn_id: String,
        locale_hint: Option<String>,
    }
    invoke_typed(
        "ptt_finalize",
        Args {
            turn_id,
            locale_hint,
        },
    )
    .await
}

pub async fn ptt_cancel(turn_id: String) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        turn_id: String,
    }
    invoke_unit_js("ptt_cancel", args_value(Args { turn_id })?).await
}

pub async fn voice_tts_playing(playing: bool) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        playing: bool,
    }
    invoke_unit_js("voice_tts_playing", args_value(Args { playing })?).await
}

#[allow(dead_code)]
pub async fn voice_agent_input_active(active: bool) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        active: bool,
    }
    invoke_unit_js("voice_agent_input_active", args_value(Args { active })?).await
}

pub async fn whisper_models_list() -> Result<Vec<WhisperModelView>, String> {
    invoke_typed("whisper_models_list", serde_json::json!({})).await
}

pub async fn whisper_model_download(id: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_unit_js("whisper_model_download", args_value(Args { id })?).await
}

pub async fn whisper_model_cancel(id: String) -> Result<bool, String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_typed("whisper_model_cancel", Args { id }).await
}

pub async fn whisper_model_delete(id: String) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args {
        id: String,
    }
    invoke_unit_js("whisper_model_delete", args_value(Args { id })?).await
}

pub fn listen_whisper_download_progress(
    callback: impl FnMut(WhisperDownloadProgress) + 'static,
) -> Option<TauriEventListener> {
    listen_tauri_event::<WhisperDownloadProgress>("whisper_download_progress", callback)
}

pub fn listen_whisper_download_done(
    callback: impl FnMut(WhisperDownloadDone) + 'static,
) -> Option<TauriEventListener> {
    listen_tauri_event::<WhisperDownloadDone>("whisper_download_done", callback)
}

pub fn listen_whisper_download_error(
    callback: impl FnMut(WhisperDownloadError) + 'static,
) -> Option<TauriEventListener> {
    listen_tauri_event::<WhisperDownloadError>("whisper_download_error", callback)
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

// ---------------------------------------------------------------------------
// Image subsystem bridge
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageProviderKind {
    Openai,
    Openrouter,
    Fal,
}

impl ImageProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
            Self::Fal => "fal",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageQualityLevel {
    Low,
    #[default]
    Medium,
    High,
    Max,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSettings {
    pub provider: ImageProviderKind,
    pub model_id: String,
    #[serde(default)]
    pub quality: ImageQualityLevel,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            provider: ImageProviderKind::Openai,
            model_id: "gpt-image-1".into(),
            quality: ImageQualityLevel::Medium,
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageCuratedModel {
    pub id: String,
    pub label: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageModelsResponse {
    pub provider: ImageProviderKind,
    pub entries: Vec<ImageCuratedModel>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImagePreviewResponse {
    pub mime: String,
    pub bytes_b64: String,
}

pub async fn image_settings_get() -> Result<ImageSettings, String> {
    invoke_typed("image_settings_get", serde_json::json!({})).await
}

pub async fn image_settings_save(patch: ImageSettings) -> Result<ImageSettings, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        patch: ImageSettings,
    }
    invoke_typed("image_settings_save", Args { patch }).await
}

pub async fn image_curated_models(
    provider: ImageProviderKind,
) -> Result<ImageModelsResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        provider: ImageProviderKind,
    }
    invoke_typed(
        "image_curated_models",
        Args {
            payload: Payload { provider },
        },
    )
    .await
}

pub async fn generated_image_preview(
    path: String,
) -> Result<GeneratedImagePreviewResponse, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Args {
        payload: Payload,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        path: String,
    }
    invoke_typed(
        "generated_image_preview",
        Args {
            payload: Payload { path },
        },
    )
    .await
}
