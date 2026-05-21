use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const SETTINGS_FILE: &str = "agent_provider_settings.json";
const SECRETS_DIR: &str = "secrets";
const KEYRING_SERVICE: &str = "BLXCode";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    fn keyring_account(self) -> String {
        format!("agent:{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThinkingLevel {
    Off,
    Low,
    Medium,
    High,
    Max,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelEntry {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderSettings {
    pub provider: AgentProviderKind,
    pub model_id: String,
    pub thinking_level: ThinkingLevel,
    #[serde(default)]
    pub model_cache_openrouter: Vec<ProviderModelEntry>,
    #[serde(default)]
    pub model_cache_anthropic: Vec<ProviderModelEntry>,
    #[serde(default)]
    pub model_cache_openai: Vec<ProviderModelEntry>,
}

impl Default for AgentProviderSettings {
    fn default() -> Self {
        Self {
            provider: AgentProviderKind::Openrouter,
            model_id: "openai/gpt-5".into(),
            thinking_level: ThinkingLevel::Medium,
            model_cache_openrouter: curated_models(AgentProviderKind::Openrouter),
            model_cache_anthropic: curated_models(AgentProviderKind::Anthropic),
            model_cache_openai: curated_models(AgentProviderKind::Openai),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyStatus {
    pub provider: AgentProviderKind,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_value: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderSettingsView {
    #[serde(flatten)]
    pub settings: AgentProviderSettings,
    pub key_statuses: Vec<ProviderKeyStatus>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelsResponse {
    pub provider: AgentProviderKind,
    pub entries: Vec<ProviderModelEntry>,
    pub source: String,
    pub used_fallback: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderSettingsPatch {
    pub provider: AgentProviderKind,
    pub model_id: String,
    pub thinking_level: ThinkingLevel,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderKeyPayload {
    pub provider: AgentProviderKind,
    pub api_key: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderRef {
    pub provider: AgentProviderKind,
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(SETTINGS_FILE))
}

fn secrets_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join(SECRETS_DIR))
}

fn fallback_secret_path(app: &AppHandle, provider: AgentProviderKind) -> Result<PathBuf, String> {
    Ok(secrets_dir(app)?.join(format!("{}.secret", provider.as_str())))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("mkdir {}: {e}", path.display()))
}

#[cfg(unix)]
fn ensure_private_dir(path: &Path) -> Result<(), String> {
    ensure_dir(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("chmod 700 {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn ensure_private_dir(path: &Path) -> Result<(), String> {
    ensure_dir(path)
}

pub(crate) fn load_settings_pub(app: &AppHandle) -> Result<AgentProviderSettings, String> {
    load_settings(app)
}

pub(crate) fn provider_key_pub(
    app: &AppHandle,
    provider: AgentProviderKind,
) -> Result<String, String> {
    provider_key(app, provider)
}

/// Shared envelope reader: parses `agent_provider_settings.json` as a JSON
/// object so sub-modules (`voice`, `image`) can insert their keys without
/// clobbering each other.
pub(crate) fn read_envelope(
    app: &AppHandle,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let path = settings_path(app)?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(serde_json::Map::new()),
        Ok(raw) => {
            let val: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
            match val {
                serde_json::Value::Object(m) => Ok(m),
                _ => Ok(serde_json::Map::new()),
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::Map::new()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// Shared envelope writer (atomic via temp + rename).
pub(crate) fn write_envelope(
    app: &AppHandle,
    envelope: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(envelope)
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
    Ok(())
}

/// List of envelope keys reserved by sibling subsystems. The agent-settings
/// writer must preserve these on every save.
const RESERVED_SIBLING_KEYS: &[&str] = &["voice", "image"];

fn load_settings(app: &AppHandle) -> Result<AgentProviderSettings, String> {
    let envelope = read_envelope(app)?;
    if envelope.is_empty() {
        return Ok(AgentProviderSettings::default());
    }
    serde_json::from_value(serde_json::Value::Object(envelope.clone()))
        .map_err(|e| format!("parse agent settings: {e}"))
}

fn save_settings(app: &AppHandle, settings: &AgentProviderSettings) -> Result<(), String> {
    // Merge the agent settings into the existing envelope so sibling keys
    // (voice/image) are preserved.
    let mut envelope = read_envelope(app)?;
    let value = serde_json::to_value(settings)
        .map_err(|e| format!("serialize agent settings: {e}"))?;
    let merged = match value {
        serde_json::Value::Object(map) => map,
        _ => return Err("agent settings did not serialize to a JSON object".into()),
    };
    // Drop keys belonging to the agent settings shape that no longer exist
    // (none today, but defensive), then write each field.
    let preserved: serde_json::Map<String, serde_json::Value> = envelope
        .iter()
        .filter(|(k, _)| RESERVED_SIBLING_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    envelope.clear();
    envelope.extend(merged);
    for (k, v) in preserved {
        envelope.insert(k, v);
    }
    write_envelope(app, &envelope)?;
    Ok(())
}

fn keyring_entry(provider: AgentProviderKind) -> Result<keyring_core::Entry, String> {
    keyring_core::Entry::new(KEYRING_SERVICE, &provider.keyring_account())
        .map_err(|e| format!("keyring init {}: {e}", provider.as_str()))
}

fn read_fallback_secret(
    app: &AppHandle,
    provider: AgentProviderKind,
) -> Result<Option<String>, String> {
    let path = fallback_secret_path(app, provider)?;
    match fs::read_to_string(&path) {
        Ok(raw) => Ok(mask_secret(&raw)
            .map(|_| raw.trim().to_string())
            .filter(|s| !s.is_empty())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read fallback {}: {e}", path.display())),
    }
}

fn write_fallback_secret(
    app: &AppHandle,
    provider: AgentProviderKind,
    secret: &str,
) -> Result<(), String> {
    let dir = secrets_dir(app)?;
    ensure_private_dir(&dir)?;
    let path = fallback_secret_path(app, provider)?;
    #[cfg(unix)]
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .map_err(|e| format!("create fallback {}: {e}", path.display()))?;
    #[cfg(not(unix))]
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .map_err(|e| format!("create fallback {}: {e}", path.display()))?;
    file.write_all(secret.as_bytes())
        .map_err(|e| format!("write fallback {}: {e}", path.display()))?;
    file.sync_all().ok();
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("chmod 600 {}: {e}", path.display()))?;
    Ok(())
}

fn delete_fallback_secret(app: &AppHandle, provider: AgentProviderKind) -> Result<(), String> {
    let path = fallback_secret_path(app, provider)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("delete fallback {}: {e}", path.display())),
    }
}

fn key_masked_value(
    app: &AppHandle,
    provider: AgentProviderKind,
) -> Result<Option<String>, String> {
    let entry = keyring_entry(provider)?;
    match entry.get_password() {
        Ok(secret) => Ok(mask_secret(&secret)),
        Err(keyring_core::Error::NoEntry) => {
            Ok(read_fallback_secret(app, provider)?.and_then(|secret| mask_secret(&secret)))
        }
        Err(_) if cfg!(target_os = "linux") => {
            Ok(read_fallback_secret(app, provider)?.and_then(|secret| mask_secret(&secret)))
        }
        Err(e) => Err(format!("keyring get {}: {e}", provider.as_str())),
    }
}

#[allow(dead_code)]
fn key_is_configured(app: &AppHandle, provider: AgentProviderKind) -> Result<bool, String> {
    Ok(key_masked_value(app, provider)?.is_some())
}

fn provider_key(app: &AppHandle, provider: AgentProviderKind) -> Result<String, String> {
    let entry = keyring_entry(provider)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(secret),
        Ok(_) => read_fallback_secret(app, provider)?
            .ok_or_else(|| format!("keyring get {}: empty secret", provider.as_str())),
        Err(keyring_core::Error::NoEntry) => read_fallback_secret(app, provider)?
            .ok_or_else(|| format!("keyring get {}: no stored secret", provider.as_str())),
        Err(_) if cfg!(target_os = "linux") => read_fallback_secret(app, provider)?
            .ok_or_else(|| format!("keyring get {}: no stored secret", provider.as_str())),
        Err(e) => Err(format!("keyring get {}: {e}", provider.as_str())),
    }
}

fn curated_models(provider: AgentProviderKind) -> Vec<ProviderModelEntry> {
    match provider {
        AgentProviderKind::Openrouter => vec![
            ProviderModelEntry {
                id: "openai/gpt-5".into(),
                label: "GPT-5".into(),
                description: Some("Default via OpenRouter".into()),
            },
            ProviderModelEntry {
                id: "anthropic/claude-sonnet-4.5".into(),
                label: "Claude Sonnet 4.5".into(),
                description: Some("Anthropic via OpenRouter".into()),
            },
            ProviderModelEntry {
                id: "google/gemini-2.5-pro".into(),
                label: "Gemini 2.5 Pro".into(),
                description: Some("Google via OpenRouter".into()),
            },
        ],
        AgentProviderKind::Anthropic => vec![
            ProviderModelEntry {
                id: "claude-sonnet-4-5".into(),
                label: "Claude Sonnet 4.5".into(),
                description: Some("Balanced model".into()),
            },
            ProviderModelEntry {
                id: "claude-opus-4-1".into(),
                label: "Claude Opus 4.1".into(),
                description: Some("Highest capability".into()),
            },
        ],
        AgentProviderKind::Openai => vec![
            ProviderModelEntry {
                id: "gpt-5".into(),
                label: "GPT-5".into(),
                description: Some("Reasoning flagship".into()),
            },
            ProviderModelEntry {
                id: "gpt-5-mini".into(),
                label: "GPT-5 Mini".into(),
                description: Some("Faster/cost-lean variant".into()),
            },
        ],
    }
}

fn cache_for_provider(
    settings: &AgentProviderSettings,
    provider: AgentProviderKind,
) -> Vec<ProviderModelEntry> {
    match provider {
        AgentProviderKind::Openrouter => settings.model_cache_openrouter.clone(),
        AgentProviderKind::Anthropic => settings.model_cache_anthropic.clone(),
        AgentProviderKind::Openai => settings.model_cache_openai.clone(),
    }
}

fn set_cache_for_provider(
    settings: &mut AgentProviderSettings,
    provider: AgentProviderKind,
    entries: Vec<ProviderModelEntry>,
) {
    match provider {
        AgentProviderKind::Openrouter => settings.model_cache_openrouter = entries,
        AgentProviderKind::Anthropic => settings.model_cache_anthropic = entries,
        AgentProviderKind::Openai => settings.model_cache_openai = entries,
    }
}

fn settings_view(
    app: &AppHandle,
    settings: AgentProviderSettings,
) -> Result<AgentProviderSettingsView, String> {
    let key_statuses = [
        AgentProviderKind::Openrouter,
        AgentProviderKind::Anthropic,
        AgentProviderKind::Openai,
    ]
    .into_iter()
    .map(|provider| {
        let masked_value = key_masked_value(app, provider)?;
        Ok(ProviderKeyStatus {
            provider,
            configured: masked_value.is_some(),
            masked_value,
        })
    })
    .collect::<Result<Vec<_>, String>>()?;

    Ok(AgentProviderSettingsView {
        settings,
        key_statuses,
    })
}

fn provider_configured_in_view(
    view: &AgentProviderSettingsView,
    provider: AgentProviderKind,
) -> bool {
    view.key_statuses
        .iter()
        .find(|status| status.provider == provider)
        .map(|status| status.configured)
        .unwrap_or(false)
}

#[derive(Deserialize)]
struct OpenrouterModelsEnvelope {
    data: Vec<OpenrouterModel>,
}

#[derive(Deserialize)]
struct OpenrouterModel {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Deserialize)]
struct OpenaiModelsEnvelope {
    data: Vec<OpenaiModel>,
}

#[derive(Deserialize)]
struct OpenaiModel {
    id: String,
}

#[derive(Deserialize)]
struct AnthropicModelsEnvelope {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
    #[serde(default)]
    display_name: Option<String>,
}

async fn fetch_models_live(
    app: &AppHandle,
    provider: AgentProviderKind,
) -> Result<Vec<ProviderModelEntry>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| format!("http client: {e}"))?;

    match provider {
        AgentProviderKind::Openrouter => {
            let res = client
                .get("https://openrouter.ai/api/v1/models")
                .send()
                .await
                .map_err(|e| format!("openrouter models: {e}"))?;
            let res = res
                .error_for_status()
                .map_err(|e| format!("openrouter models: {e}"))?;
            let body: OpenrouterModelsEnvelope = res
                .json()
                .await
                .map_err(|e| format!("openrouter parse: {e}"))?;
            let mut items = body
                .data
                .into_iter()
                .map(|entry| ProviderModelEntry {
                    label: entry.name.clone().unwrap_or_else(|| entry.id.clone()),
                    id: entry.id,
                    description: entry.description,
                })
                .collect::<Vec<_>>();
            items.sort_by(|a, b| a.label.cmp(&b.label));
            Ok(items)
        }
        AgentProviderKind::Openai => {
            let key = provider_key(app, provider)?;
            let res = client
                .get("https://api.openai.com/v1/models")
                .bearer_auth(key)
                .send()
                .await
                .map_err(|e| format!("openai models: {e}"))?;
            let res = res
                .error_for_status()
                .map_err(|e| format!("openai models: {e}"))?;
            let body: OpenaiModelsEnvelope =
                res.json().await.map_err(|e| format!("openai parse: {e}"))?;
            let mut items = body
                .data
                .into_iter()
                .filter(|entry| entry.id.starts_with("gpt-") || entry.id.contains("o"))
                .map(|entry| ProviderModelEntry {
                    label: entry.id.clone(),
                    id: entry.id,
                    description: None,
                })
                .collect::<Vec<_>>();
            items.sort_by(|a, b| a.label.cmp(&b.label));
            Ok(items)
        }
        AgentProviderKind::Anthropic => {
            let key = provider_key(app, provider)?;
            let res = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| format!("anthropic models: {e}"))?;
            let res = res
                .error_for_status()
                .map_err(|e| format!("anthropic models: {e}"))?;
            let body: AnthropicModelsEnvelope = res
                .json()
                .await
                .map_err(|e| format!("anthropic parse: {e}"))?;
            let mut items = body
                .data
                .into_iter()
                .map(|entry| ProviderModelEntry {
                    label: entry
                        .display_name
                        .clone()
                        .unwrap_or_else(|| entry.id.clone()),
                    id: entry.id,
                    description: None,
                })
                .collect::<Vec<_>>();
            items.sort_by(|a, b| a.label.cmp(&b.label));
            Ok(items)
        }
    }
}

#[tauri::command]
pub fn agent_settings_get(app: AppHandle) -> Result<AgentProviderSettingsView, String> {
    settings_view(&app, load_settings(&app)?)
}

#[tauri::command]
pub fn agent_settings_save(
    app: AppHandle,
    patch: AgentProviderSettingsPatch,
) -> Result<AgentProviderSettingsView, String> {
    let mut settings = load_settings(&app)?;
    settings.provider = patch.provider;
    settings.model_id = patch.model_id.trim().to_string();
    settings.thinking_level = patch.thinking_level;
    save_settings(&app, &settings)?;
    settings_view(&app, settings)
}

#[tauri::command]
pub fn agent_api_key_set(
    app: AppHandle,
    payload: ProviderKeyPayload,
) -> Result<AgentProviderSettingsView, String> {
    let trimmed = payload.api_key.trim();
    if trimmed.is_empty() {
        return Err("API key must not be empty".into());
    }
    let entry = keyring_entry(payload.provider)?;
    let keyring_write = entry.set_password(trimmed);
    match keyring_write {
        Ok(()) => {}
        Err(_e) if cfg!(target_os = "linux") => {
            write_fallback_secret(&app, payload.provider, trimmed)?;
            let view = settings_view(&app, load_settings(&app)?)?;
            if !provider_configured_in_view(&view, payload.provider) {
                return Err(format!(
                    "linux fallback verify {}: fallback secret was written, but a fresh lookup still reports no stored secret",
                    payload.provider.as_str()
                ));
            }
            return Ok(view);
        }
        Err(e) => return Err(format!("keyring set {}: {e}", payload.provider.as_str())),
    }
    match entry.get_password() {
        Ok(saved) if saved.trim().is_empty() => {
            if cfg!(target_os = "linux") {
                write_fallback_secret(&app, payload.provider, trimmed)?;
            } else {
                return Err(format!(
                    "keyring verify {}: secret was written, but readback returned an empty value",
                    payload.provider.as_str()
                ));
            }
        }
        Ok(_) => {}
        Err(e) => {
            if cfg!(target_os = "linux") {
                write_fallback_secret(&app, payload.provider, trimmed)?;
            } else {
                return Err(format!(
                    "keyring verify {}: readback failed after write: {e}",
                    payload.provider.as_str()
                ));
            }
        }
    }
    let view = settings_view(&app, load_settings(&app)?)?;
    if !provider_configured_in_view(&view, payload.provider) {
        if cfg!(target_os = "linux") {
            write_fallback_secret(&app, payload.provider, trimmed)?;
            let view = settings_view(&app, load_settings(&app)?)?;
            if !provider_configured_in_view(&view, payload.provider) {
                return Err(format!(
                    "linux fallback verify {}: fallback secret was written, but a fresh lookup still reports no stored secret",
                    payload.provider.as_str()
                ));
            }
            return Ok(view);
        } else {
            return Err(format!(
                "keyring verify {}: readback succeeded on the immediate handle, but a fresh lookup still reports no stored secret",
                payload.provider.as_str()
            ));
        }
    }
    let _ = delete_fallback_secret(&app, payload.provider);
    Ok(view)
}

#[tauri::command]
pub fn agent_api_key_delete(
    app: AppHandle,
    payload: ProviderRef,
) -> Result<AgentProviderSettingsView, String> {
    let entry = keyring_entry(payload.provider)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => {}
        Err(_) if cfg!(target_os = "linux") => {}
        Err(e) => return Err(format!("keyring delete {}: {e}", payload.provider.as_str())),
    }
    delete_fallback_secret(&app, payload.provider)?;
    match entry.get_password() {
        Ok(_) => {
            if !cfg!(target_os = "linux") {
                return Err(format!(
                    "keyring verify {}: secret still present after delete",
                    payload.provider.as_str()
                ));
            }
        }
        Err(keyring_core::Error::NoEntry) => {}
        Err(_) if cfg!(target_os = "linux") => {}
        Err(e) => {
            return Err(format!(
                "keyring verify {}: readback after delete failed unexpectedly: {e}",
                payload.provider.as_str()
            ));
        }
    }
    let view = settings_view(&app, load_settings(&app)?)?;
    if provider_configured_in_view(&view, payload.provider) {
        return Err(format!(
            "keyring verify {}: delete returned successfully, but a fresh lookup still reports a stored secret",
            payload.provider.as_str()
        ));
    }
    Ok(view)
}

#[tauri::command]
pub async fn agent_provider_models(
    app: AppHandle,
    payload: ProviderRef,
) -> Result<ProviderModelsResponse, String> {
    let provider = payload.provider;
    let mut settings = load_settings(&app)?;

    match fetch_models_live(&app, provider).await {
        Ok(entries) if !entries.is_empty() => {
            set_cache_for_provider(&mut settings, provider, entries.clone());
            save_settings(&app, &settings)?;
            Ok(ProviderModelsResponse {
                provider,
                entries,
                source: "live".into(),
                used_fallback: false,
                message: None,
            })
        }
        Ok(_) => {
            let cached = cache_for_provider(&settings, provider);
            let entries = if cached.is_empty() {
                curated_models(provider)
            } else {
                cached
            };
            Ok(ProviderModelsResponse {
                provider,
                entries,
                source: "fallback".into(),
                used_fallback: true,
                message: Some("Provider returned an empty model list.".into()),
            })
        }
        Err(err) => {
            let cached = cache_for_provider(&settings, provider);
            let entries = if cached.is_empty() {
                curated_models(provider)
            } else {
                cached
            };
            Ok(ProviderModelsResponse {
                provider,
                entries,
                source: if cache_for_provider(&settings, provider).is_empty() {
                    "curated".into()
                } else {
                    "cache".into()
                },
                used_fallback: true,
                message: Some(err),
            })
        }
    }
}

pub fn provider_status_json() -> serde_json::Value {
    let key_statuses = [
        AgentProviderKind::Openrouter,
        AgentProviderKind::Anthropic,
        AgentProviderKind::Openai,
    ]
    .into_iter()
    .map(|provider| {
        serde_json::json!({
            "provider": provider.as_str(),
            "configured": false,
        })
    })
    .collect::<Vec<_>>();

    serde_json::json!({
        "phase": "mock_engine",
        "defaultProvider": AgentProviderKind::Openrouter.as_str(),
        "keyStatuses": key_statuses,
    })
}

fn mask_secret(secret: &str) -> Option<String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return None;
    }
    let suffix: String = trimmed
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if suffix.is_empty() {
        Some("********".into())
    } else {
        Some(format!("********{}", suffix))
    }
}
