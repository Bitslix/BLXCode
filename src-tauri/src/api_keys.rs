//! Centralized API-key catalog: a single Tauri surface that lists every
//! provider key the app understands (LLM + Search), dispatches batch
//! set/delete actions, and surfaces masked values + env-var fallback
//! status. Storage is delegated to the existing per-subsystem helpers
//! (`agent_settings` for LLM keys, `agent::web_settings` for search keys)
//! so no migration is needed.
//!
//! Wired into the frontend via `api_keys_status` and `api_keys_apply`.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::agent::web_settings::{self, WebKeyKind};
use crate::agent_settings::{
    self, delete_provider_key_secret, provider_env_var, provider_key_with_source,
    set_provider_key_secret, AgentProviderKind, KeySource,
};
use crate::media_keys::{self, MediaKeyKind};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiKeyCategory {
    Llm,
    Search,
    ImageVideo,
}

/// One row in the API-keys pane. `kind` is the stable identifier the UI
/// echoes back in `api_keys_apply` actions.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyEntry {
    pub kind: String,
    pub label: String,
    pub category: ApiKeyCategory,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_value: Option<String>,
    pub via_env: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    pub coming_soon: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeysStatus {
    pub entries: Vec<ApiKeyEntry>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ApiKeyAction {
    Set { kind: String, value: String },
    Delete { kind: String },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeysApplyRequest {
    pub actions: Vec<ApiKeyAction>,
}

const LLM_KINDS: &[(AgentProviderKind, &str, &str)] = &[
    (AgentProviderKind::Openrouter, "openrouter", "OpenRouter"),
    (AgentProviderKind::Anthropic, "anthropic", "Anthropic"),
    (AgentProviderKind::Openai, "openai", "OpenAI"),
];

const SEARCH_KINDS: &[(WebKeyKind, &str, &str)] = &[
    (WebKeyKind::Tavily, "tavily", "Tavily"),
    (WebKeyKind::Brave, "brave", "Brave Search"),
];

const COMING_SOON_LLM: &[(&str, &str)] = &[
    ("google", "Google"),
    ("mistral", "Mistral"),
    ("grok", "Grok xAI"),
];

fn llm_provider_from_kind(kind: &str) -> Option<AgentProviderKind> {
    LLM_KINDS
        .iter()
        .find(|(_, k, _)| *k == kind)
        .map(|(p, _, _)| *p)
}

fn search_kind_from_kind(kind: &str) -> Option<WebKeyKind> {
    SEARCH_KINDS
        .iter()
        .find(|(_, k, _)| *k == kind)
        .map(|(k, _, _)| *k)
}

fn media_kind_from_kind(kind: &str) -> Option<MediaKeyKind> {
    media_keys::kind_from_id(kind)
}

fn build_status(app: &AppHandle) -> Result<ApiKeysStatus, String> {
    let mut entries = Vec::with_capacity(
        LLM_KINDS.len()
            + SEARCH_KINDS.len()
            + COMING_SOON_LLM.len()
            + media_keys::MEDIA_KEY_KINDS.len(),
    );

    for (provider, kind, label) in LLM_KINDS {
        let (masked, source) = provider_key_with_source(app, *provider)?;
        entries.push(ApiKeyEntry {
            kind: (*kind).into(),
            label: (*label).into(),
            category: ApiKeyCategory::Llm,
            configured: masked.is_some(),
            masked_value: masked,
            via_env: matches!(source, KeySource::Env),
            env_var: Some(provider_env_var(*provider).into()),
            coming_soon: false,
        });
    }

    for (search_kind, kind, label) in SEARCH_KINDS {
        let (masked, via_env) = web_settings::key_with_source(*search_kind)?;
        entries.push(ApiKeyEntry {
            kind: (*kind).into(),
            label: (*label).into(),
            category: ApiKeyCategory::Search,
            configured: masked.is_some(),
            masked_value: masked,
            via_env,
            env_var: Some(web_settings::env_var_name(*search_kind).into()),
            coming_soon: false,
        });
    }

    for (kind, label) in COMING_SOON_LLM {
        entries.push(ApiKeyEntry {
            kind: (*kind).into(),
            label: (*label).into(),
            category: ApiKeyCategory::Llm,
            configured: false,
            masked_value: None,
            via_env: false,
            env_var: None,
            coming_soon: true,
        });
    }

    for media_kind in media_keys::MEDIA_KEY_KINDS {
        let row = media_keys::catalog_entry(media_kind)?;
        entries.push(ApiKeyEntry {
            kind: row.kind,
            label: row.label,
            category: ApiKeyCategory::ImageVideo,
            configured: row.configured,
            masked_value: row.masked_value,
            via_env: row.via_env,
            env_var: Some(row.env_var),
            coming_soon: false,
        });
    }

    Ok(ApiKeysStatus { entries })
}

fn apply_one(app: &AppHandle, action: &ApiKeyAction) -> Result<(), String> {
    match action {
        ApiKeyAction::Set { kind, value } => {
            if let Some(provider) = llm_provider_from_kind(kind) {
                set_provider_key_secret(app, provider, value)?;
                return Ok(());
            }
            if let Some(search_kind) = search_kind_from_kind(kind) {
                web_settings::set_key(app, search_kind, value.clone())?;
                return Ok(());
            }
            if let Some(media_kind) = media_kind_from_kind(kind) {
                media_keys::set_key(app, media_kind, value)?;
                return Ok(());
            }
            Err(format!("unknown api key kind: {kind}"))
        }
        ApiKeyAction::Delete { kind } => {
            if let Some(provider) = llm_provider_from_kind(kind) {
                delete_provider_key_secret(app, provider)?;
                return Ok(());
            }
            if let Some(search_kind) = search_kind_from_kind(kind) {
                web_settings::delete_key(app, search_kind)?;
                return Ok(());
            }
            if let Some(media_kind) = media_kind_from_kind(kind) {
                media_keys::delete_key(app, media_kind)?;
                return Ok(());
            }
            Err(format!("unknown api key kind: {kind}"))
        }
    }
}

#[tauri::command]
pub fn api_keys_status(app: AppHandle) -> Result<ApiKeysStatus, String> {
    build_status(&app)
}

#[tauri::command]
pub fn api_keys_apply(
    app: AppHandle,
    payload: ApiKeysApplyRequest,
) -> Result<ApiKeysStatus, String> {
    for action in &payload.actions {
        apply_one(&app, action)?;
    }
    // Keep the web runtime cache in sync if any search key changed.
    web_settings::refresh_runtime_from_app(&app);
    // agent_settings has its own load_settings cache-free path, no refresh needed.
    let _ = agent_settings::load_settings_pub(&app);
    build_status(&app)
}
