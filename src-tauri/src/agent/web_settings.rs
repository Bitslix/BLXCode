//! Web search/fetch settings (`agent.web` envelope) and keyring-backed API keys.

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use tauri::AppHandle;

use crate::agent_settings::{self, read_envelope};

const KEYRING_SERVICE: &str = "BLXCode";

const WEB_KEY_TAVILY: &str = "agent:web:tavily";
const WEB_KEY_BRAVE: &str = "agent:web:brave";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebProviderKind {
    None,
    Tavily,
    Brave,
}

impl Default for WebProviderKind {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebKeyKind {
    Tavily,
    Brave,
}

impl WebKeyKind {
    fn account(self) -> &'static str {
        match self {
            Self::Tavily => WEB_KEY_TAVILY,
            Self::Brave => WEB_KEY_BRAVE,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWebSettings {
    pub provider: WebProviderKind,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebKeyStatus {
    pub kind: String,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_value: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWebSettingsView {
    pub settings: AgentWebSettings,
    pub key_statuses: Vec<WebKeyStatus>,
}

static RUNTIME: OnceLock<Mutex<AgentWebSettings>> = OnceLock::new();

fn runtime() -> &'static Mutex<AgentWebSettings> {
    RUNTIME.get_or_init(|| Mutex::new(AgentWebSettings::default()))
}

pub fn refresh_runtime_from_app(app: &AppHandle) {
    if let Ok(s) = load(app) {
        if let Ok(mut g) = runtime().lock() {
            *g = s;
        }
    }
}

pub fn runtime_provider() -> WebProviderKind {
    runtime().lock().map(|g| g.provider).unwrap_or_default()
}

pub fn load(app: &AppHandle) -> Result<AgentWebSettings, String> {
    let envelope = read_envelope(app)?;
    Ok(envelope
        .get("web")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default())
}

pub fn save(app: &AppHandle, settings: &AgentWebSettings) -> Result<AgentWebSettingsView, String> {
    let mut envelope = read_envelope(app)?;
    let value =
        serde_json::to_value(settings).map_err(|e| format!("serialize web settings: {e}"))?;
    envelope.insert("web".into(), value);
    agent_settings::write_envelope(app, &envelope)?;
    if let Ok(mut g) = runtime().lock() {
        *g = settings.clone();
    }
    view(app)
}

pub fn view(app: &AppHandle) -> Result<AgentWebSettingsView, String> {
    let settings = load(app)?;
    Ok(AgentWebSettingsView {
        key_statuses: vec![
            key_status(WebKeyKind::Tavily)?,
            key_status(WebKeyKind::Brave)?,
        ],
        settings,
    })
}

fn key_status(kind: WebKeyKind) -> Result<WebKeyStatus, String> {
    let configured = resolve_key(kind).is_some();
    let masked = key_masked(kind)?;
    Ok(WebKeyStatus {
        kind: match kind {
            WebKeyKind::Tavily => "tavily".into(),
            WebKeyKind::Brave => "brave".into(),
        },
        configured,
        masked_value: masked,
    })
}

fn keyring_entry(kind: WebKeyKind) -> Result<keyring_core::Entry, String> {
    keyring_core::Entry::new(KEYRING_SERVICE, kind.account())
        .map_err(|e| format!("keyring init {}: {e}", kind.account()))
}

fn key_masked(kind: WebKeyKind) -> Result<Option<String>, String> {
    let entry = keyring_entry(kind)?;
    match entry.get_password() {
        Ok(secret) => Ok(agent_settings::mask_secret_pub(&secret)),
        Err(keyring_core::Error::NoEntry) => {
            Ok(env_key(kind).and_then(|s| agent_settings::mask_secret_pub(&s)))
        }
        Err(_) if cfg!(target_os = "linux") => {
            Ok(env_key(kind).and_then(|s| agent_settings::mask_secret_pub(&s)))
        }
        Err(e) => Err(format!("keyring get {}: {e}", kind.account())),
    }
}

pub fn env_var_name(kind: WebKeyKind) -> &'static str {
    match kind {
        WebKeyKind::Tavily => "BLX_TAVILY_API_KEY",
        WebKeyKind::Brave => "BLX_BRAVE_API_KEY",
    }
}

fn env_key(kind: WebKeyKind) -> Option<String> {
    std::env::var(env_var_name(kind))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// (masked_value, via_env). Mirrors the LLM provider equivalent in
/// `agent_settings::provider_key_with_source`. Order: keyring → env.
pub fn key_with_source(kind: WebKeyKind) -> Result<(Option<String>, bool), String> {
    let entry = keyring_entry(kind)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => {
            Ok((agent_settings::mask_secret_pub(&secret), false))
        }
        Ok(_) | Err(keyring_core::Error::NoEntry) => match env_key(kind) {
            Some(secret) => Ok((agent_settings::mask_secret_pub(&secret), true)),
            None => Ok((None, false)),
        },
        Err(_) if cfg!(target_os = "linux") => match env_key(kind) {
            Some(secret) => Ok((agent_settings::mask_secret_pub(&secret), true)),
            None => Ok((None, false)),
        },
        Err(e) => Err(format!("keyring get {}: {e}", kind.account())),
    }
}

pub fn resolve_key(kind: WebKeyKind) -> Option<String> {
    env_key(kind).or_else(|| {
        keyring_entry(kind)
            .ok()?
            .get_password()
            .ok()
            .filter(|s| !s.trim().is_empty())
    })
}

pub fn resolve_active_key() -> Option<(WebProviderKind, String)> {
    match runtime_provider() {
        WebProviderKind::Tavily => {
            resolve_key(WebKeyKind::Tavily).map(|k| (WebProviderKind::Tavily, k))
        }
        WebProviderKind::Brave => {
            resolve_key(WebKeyKind::Brave).map(|k| (WebProviderKind::Brave, k))
        }
        WebProviderKind::None => env_key(WebKeyKind::Tavily)
            .map(|k| (WebProviderKind::Tavily, k))
            .or_else(|| env_key(WebKeyKind::Brave).map(|k| (WebProviderKind::Brave, k))),
    }
}

pub fn web_tools_enabled() -> bool {
    resolve_active_key().is_some()
}

pub fn set_key(
    app: &AppHandle,
    kind: WebKeyKind,
    api_key: String,
) -> Result<AgentWebSettingsView, String> {
    let secret = api_key.trim();
    if secret.is_empty() {
        return Err("API key is empty".into());
    }
    let entry = keyring_entry(kind)?;
    entry
        .set_password(secret)
        .map_err(|e| format!("keyring set {}: {e}", kind.account()))?;
    view(app)
}

pub fn delete_key(app: &AppHandle, kind: WebKeyKind) -> Result<AgentWebSettingsView, String> {
    let entry = keyring_entry(kind)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => {}
        Err(e) => return Err(format!("keyring delete {}: {e}", kind.account())),
    }
    view(app)
}
