use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const SETTINGS_FILE: &str = "agent_provider_settings.json";
const SECRETS_DIR: &str = "secrets";
const KEYRING_SERVICE: &str = "BLXCode";

/// Hard upper bound on tool-call rounds per turn. Caps a runaway loop when
/// the model keeps calling tools without finishing. User-configurable via
/// Settings → Agent; the value is clamped to [`MIN_TOOL_LOOP_LIMIT`,
/// `MAX_TOOL_LOOP_LIMIT`] on save and again at use time (settings on disk
/// may have been hand-edited).
pub const DEFAULT_TOOL_LOOP_LIMIT: u32 = 36;
pub const MIN_TOOL_LOOP_LIMIT: u32 = 1;
pub const MAX_TOOL_LOOP_LIMIT: u32 = 500;

fn default_tool_loop_limit() -> u32 {
    DEFAULT_TOOL_LOOP_LIMIT
}

/// Clamp a tool-loop limit into the supported range. Applied on save and at
/// the call site so an out-of-range on-disk value can never produce a
/// zero-round (or absurdly large) loop.
pub fn clamp_tool_loop_limit(value: u32) -> u32 {
    value.clamp(MIN_TOOL_LOOP_LIMIT, MAX_TOOL_LOOP_LIMIT)
}

/// Default auto-compaction trigger as a percent of the context window.
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

fn default_onboarding_seen() -> bool {
    false
}

/// Clamp an auto-compact threshold percent into the supported range.
pub fn clamp_auto_compact_threshold_pct(value: u8) -> u8 {
    value.clamp(
        MIN_AUTO_COMPACT_THRESHOLD_PCT,
        MAX_AUTO_COMPACT_THRESHOLD_PCT,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentOrbMode {
    #[serde(rename = "3d")]
    ThreeD,
    #[serde(rename = "2d")]
    TwoD,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelEntry {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    /// USD-per-token pricing. Populated from OpenRouter's `/models`
    /// payload when available; `None` for direct providers (resolved at
    /// cost-lookup time via the static id-mapping table in `pricing.rs`).
    #[serde(default)]
    pub pricing: Option<ModelPricing>,
    /// Max context window in tokens. Populated from OpenRouter's `/models`
    /// `context_length`; `None` for direct providers (resolved via the
    /// static fallback table in `agent/context_window.rs`).
    #[serde(default)]
    pub context_length: Option<u64>,
}

/// USD per-token pricing for one model. OpenRouter exposes both numbers
/// as decimal strings — we parse them into `f64` once at fetch time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricing {
    /// USD per input/prompt token.
    pub prompt: f64,
    /// USD per output/completion token.
    pub completion: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProviderSettings {
    pub provider: AgentProviderKind,
    pub model_id: String,
    pub thinking_level: ThinkingLevel,
    /// Max tool-call rounds per turn (see [`DEFAULT_TOOL_LOOP_LIMIT`]).
    #[serde(default = "default_tool_loop_limit")]
    pub tool_loop_limit: u32,
    /// Auto-compact the session when it nears the context-window limit.
    #[serde(default = "default_auto_compact_enabled")]
    pub auto_compact_enabled: bool,
    /// Context-window occupancy percent that triggers auto-compaction.
    #[serde(default = "default_auto_compact_threshold_pct")]
    pub auto_compact_threshold_pct: u8,
    /// Visual style for the Agent panel voice orb.
    #[serde(default = "default_orb_mode")]
    pub orb_mode: AgentOrbMode,
    /// User-chosen agent name. Empty = use [`DEFAULT_AGENT_NICKNAME`].
    #[serde(default)]
    pub agent_nickname: String,
    /// Whether the one-time startup dialog for name + default role has been
    /// completed. Defaults to false so existing installations see it once.
    #[serde(default = "default_onboarding_seen")]
    pub onboarding_seen: bool,
    /// Default harness session-role slug for newly-created workspaces.
    #[serde(default)]
    pub default_session_role: Option<String>,
    #[serde(default)]
    pub model_cache_openrouter: Vec<ProviderModelEntry>,
    #[serde(default)]
    pub model_cache_anthropic: Vec<ProviderModelEntry>,
    #[serde(default)]
    pub model_cache_openai: Vec<ProviderModelEntry>,
    #[serde(default)]
    pub model_caches: BTreeMap<String, Vec<ProviderModelEntry>>,
    #[serde(default)]
    pub provider_base_urls: BTreeMap<String, String>,
    #[serde(default)]
    pub cloudflare_account_id: String,
}

impl Default for AgentProviderSettings {
    fn default() -> Self {
        Self {
            provider: AgentProviderKind::Openrouter,
            model_id: "openai/gpt-5".into(),
            thinking_level: ThinkingLevel::Medium,
            tool_loop_limit: DEFAULT_TOOL_LOOP_LIMIT,
            auto_compact_enabled: default_auto_compact_enabled(),
            auto_compact_threshold_pct: DEFAULT_AUTO_COMPACT_THRESHOLD_PCT,
            orb_mode: default_orb_mode(),
            agent_nickname: String::new(),
            onboarding_seen: default_onboarding_seen(),
            default_session_role: None,
            model_cache_openrouter: curated_models(AgentProviderKind::Openrouter),
            model_cache_anthropic: curated_models(AgentProviderKind::Anthropic),
            model_cache_openai: curated_models(AgentProviderKind::Openai),
            model_caches: default_model_caches(),
            provider_base_urls: default_provider_base_urls(),
            cloudflare_account_id: String::new(),
        }
    }
}

impl AgentProviderSettings {
    pub fn base_url_for_provider(&self, provider: AgentProviderKind) -> Option<String> {
        self.provider_base_urls
            .get(provider.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
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
    pub default_session_role: Option<String>,
    #[serde(default)]
    pub provider_base_urls: BTreeMap<String, String>,
    #[serde(default)]
    pub cloudflare_account_id: String,
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

fn normalize_session_role(raw: Option<String>) -> Option<String> {
    raw.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
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

/// Source of a successfully resolved key. Surfaces in the UI so users
/// understand why a row shows a masked value (e.g. \"via env\").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeySource {
    Keyring,
    File,
    Env,
    None,
}

pub(crate) fn provider_env_var(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "BLX_OPENROUTER_API_KEY",
        AgentProviderKind::Anthropic => "BLX_ANTHROPIC_API_KEY",
        AgentProviderKind::Openai => "BLX_OPENAI_API_KEY",
        AgentProviderKind::Ollama => "BLX_OLLAMA_API_KEY",
        AgentProviderKind::LmStudio => "BLX_LM_STUDIO_API_KEY",
        AgentProviderKind::HuggingFace => "BLX_HUGGINGFACE_API_KEY",
        AgentProviderKind::Cloudflare => "BLX_CLOUDFLARE_API_TOKEN",
        AgentProviderKind::Together => "BLX_TOGETHER_API_KEY",
        AgentProviderKind::Portkey => "BLX_PORTKEY_API_KEY",
    }
}

fn provider_env_secret(provider: AgentProviderKind) -> Option<String> {
    std::env::var(provider_env_var(provider))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Mask + source resolution for the new central API-keys view.
pub(crate) fn provider_key_with_source(
    app: &AppHandle,
    provider: AgentProviderKind,
) -> Result<(Option<String>, KeySource), String> {
    let entry = keyring_entry(provider)?;
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok((mask_secret(&secret), KeySource::Keyring)),
        Ok(_) | Err(keyring_core::Error::NoEntry) => match read_fallback_secret(app, provider)? {
            Some(secret) => Ok((mask_secret(&secret), KeySource::File)),
            None => match provider_env_secret(provider) {
                Some(secret) => Ok((mask_secret(&secret), KeySource::Env)),
                None => Ok((None, KeySource::None)),
            },
        },
        Err(_) if cfg!(target_os = "linux") => match read_fallback_secret(app, provider)? {
            Some(secret) => Ok((mask_secret(&secret), KeySource::File)),
            None => match provider_env_secret(provider) {
                Some(secret) => Ok((mask_secret(&secret), KeySource::Env)),
                None => Ok((None, KeySource::None)),
            },
        },
        Err(e) => Err(format!("keyring get {}: {e}", provider.as_str())),
    }
}

/// Set a provider key (keyring with file fallback on Linux). Same write +
/// verify logic as the legacy `agent_api_key_set` command body, exposed
/// for the centralized `api_keys_apply` batch command.
pub(crate) fn set_provider_key_secret(
    app: &AppHandle,
    provider: AgentProviderKind,
    secret: &str,
) -> Result<(), String> {
    let trimmed = secret.trim();
    if trimmed.is_empty() {
        return Err("API key must not be empty".into());
    }
    let entry = keyring_entry(provider)?;
    let keyring_write = entry.set_password(trimmed);
    match keyring_write {
        Ok(()) => {}
        Err(_e) if cfg!(target_os = "linux") => {
            write_fallback_secret(app, provider, trimmed)?;
            return Ok(());
        }
        Err(e) => return Err(format!("keyring set {}: {e}", provider.as_str())),
    }
    match entry.get_password() {
        Ok(saved) if saved.trim().is_empty() => {
            if cfg!(target_os = "linux") {
                write_fallback_secret(app, provider, trimmed)?;
                return Ok(());
            }
            return Err(format!(
                "keyring verify {}: secret was written, but readback returned an empty value",
                provider.as_str()
            ));
        }
        Ok(_) => {}
        Err(e) => {
            if cfg!(target_os = "linux") {
                write_fallback_secret(app, provider, trimmed)?;
                return Ok(());
            }
            return Err(format!(
                "keyring verify {}: readback failed after write: {e}",
                provider.as_str()
            ));
        }
    }
    let _ = delete_fallback_secret(app, provider);
    Ok(())
}

/// Delete a provider key from keyring + fallback file. Idempotent.
pub(crate) fn delete_provider_key_secret(
    app: &AppHandle,
    provider: AgentProviderKind,
) -> Result<(), String> {
    let entry = keyring_entry(provider)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => {}
        Err(_) if cfg!(target_os = "linux") => {}
        Err(e) => return Err(format!("keyring delete {}: {e}", provider.as_str())),
    }
    delete_fallback_secret(app, provider)?;
    Ok(())
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
const RESERVED_SIBLING_KEYS: &[&str] = &["voice", "image", "web"];

fn load_settings(app: &AppHandle) -> Result<AgentProviderSettings, String> {
    let envelope = read_envelope(app)?;
    if envelope.is_empty() {
        return Ok(AgentProviderSettings::default());
    }
    let mut merged = match serde_json::to_value(AgentProviderSettings::default())
        .map_err(|e| format!("serialize default agent settings: {e}"))?
    {
        serde_json::Value::Object(map) => map,
        _ => return Err("default agent settings did not serialize to a JSON object".into()),
    };
    for (key, value) in envelope {
        if !RESERVED_SIBLING_KEYS.contains(&key.as_str()) {
            merged.insert(key, value);
        }
    }
    let mut settings: AgentProviderSettings =
        serde_json::from_value(serde_json::Value::Object(merged))
            .map_err(|e| format!("parse agent settings: {e}"))?;
    normalize_provider_settings(&mut settings);
    Ok(settings)
}

fn save_settings(app: &AppHandle, settings: &AgentProviderSettings) -> Result<(), String> {
    // Merge the agent settings into the existing envelope so sibling keys
    // (voice/image) are preserved.
    let mut envelope = read_envelope(app)?;
    let value =
        serde_json::to_value(settings).map_err(|e| format!("serialize agent settings: {e}"))?;
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
    provider_key_with_source(app, provider).map(|(masked, _)| masked)
}

#[allow(dead_code)]
fn key_is_configured(app: &AppHandle, provider: AgentProviderKind) -> Result<bool, String> {
    Ok(key_masked_value(app, provider)?.is_some())
}

fn provider_key(app: &AppHandle, provider: AgentProviderKind) -> Result<String, String> {
    if !crate::agent::provider::provider_requires_key(provider) {
        return Ok(String::new());
    }
    let entry = keyring_entry(provider)?;
    let from_fallback_or_env = || -> Result<String, String> {
        if let Some(secret) = read_fallback_secret(app, provider)? {
            return Ok(secret);
        }
        provider_env_secret(provider).ok_or_else(|| {
            format!(
                "no API key configured for {} (set it in Settings \u{2192} API Keys or via {})",
                provider.as_str(),
                provider_env_var(provider)
            )
        })
    };
    match entry.get_password() {
        Ok(secret) if !secret.trim().is_empty() => Ok(secret),
        Ok(_) | Err(keyring_core::Error::NoEntry) => from_fallback_or_env(),
        Err(_) if cfg!(target_os = "linux") => from_fallback_or_env(),
        Err(e) => Err(format!("keyring get {}: {e}", provider.as_str())),
    }
}

fn curated_models(provider: AgentProviderKind) -> Vec<ProviderModelEntry> {
    crate::agent::provider::curated_models(provider)
}

fn default_model_caches() -> BTreeMap<String, Vec<ProviderModelEntry>> {
    crate::agent::provider::all_providers()
        .iter()
        .map(|spec| (spec.kind.as_str().to_string(), curated_models(spec.kind)))
        .collect()
}

fn default_provider_base_urls() -> BTreeMap<String, String> {
    crate::agent::provider::all_providers()
        .iter()
        .filter(|spec| {
            matches!(
                spec.kind,
                AgentProviderKind::Ollama
                    | AgentProviderKind::LmStudio
                    | AgentProviderKind::Portkey
            )
        })
        .map(|spec| {
            (
                spec.kind.as_str().to_string(),
                spec.default_base_url.to_string(),
            )
        })
        .collect()
}

fn normalize_provider_settings(settings: &mut AgentProviderSettings) {
    if settings.model_caches.is_empty() {
        settings.model_caches = default_model_caches();
    }
    if !settings.model_cache_openrouter.is_empty() {
        settings.model_caches.insert(
            AgentProviderKind::Openrouter.as_str().into(),
            settings.model_cache_openrouter.clone(),
        );
    }
    if !settings.model_cache_anthropic.is_empty() {
        settings.model_caches.insert(
            AgentProviderKind::Anthropic.as_str().into(),
            settings.model_cache_anthropic.clone(),
        );
    }
    if !settings.model_cache_openai.is_empty() {
        settings.model_caches.insert(
            AgentProviderKind::Openai.as_str().into(),
            settings.model_cache_openai.clone(),
        );
    }
    for provider in crate::agent::provider::all_providers()
        .iter()
        .map(|spec| spec.kind)
    {
        settings
            .model_caches
            .entry(provider.as_str().into())
            .or_insert_with(|| curated_models(provider));
    }
    sync_legacy_caches(settings);
    for (key, value) in default_provider_base_urls() {
        settings.provider_base_urls.entry(key).or_insert(value);
    }
}

fn sync_legacy_caches(settings: &mut AgentProviderSettings) {
    settings.model_cache_openrouter = cache_for_provider(settings, AgentProviderKind::Openrouter);
    settings.model_cache_anthropic = cache_for_provider(settings, AgentProviderKind::Anthropic);
    settings.model_cache_openai = cache_for_provider(settings, AgentProviderKind::Openai);
}

fn cache_for_provider(
    settings: &AgentProviderSettings,
    provider: AgentProviderKind,
) -> Vec<ProviderModelEntry> {
    settings
        .model_caches
        .get(provider.as_str())
        .cloned()
        .unwrap_or_else(|| curated_models(provider))
}

fn set_cache_for_provider(
    settings: &mut AgentProviderSettings,
    provider: AgentProviderKind,
    entries: Vec<ProviderModelEntry>,
) {
    settings
        .model_caches
        .insert(provider.as_str().into(), entries);
    sync_legacy_caches(settings);
}

/// Resolve the active model's max context window in tokens. Prefers the
/// cached provider entry's `context_length` (OpenRouter live data), falling
/// back to the static table in `agent/context_window.rs`. `None` when the
/// model is unknown — the UI then shows a raw token count without a percent.
pub fn resolve_context_length(settings: &AgentProviderSettings) -> Option<u64> {
    let provider = settings.provider;
    let model_id = settings.model_id.trim();
    if model_id.is_empty() {
        return None;
    }
    let cached = cache_for_provider(settings, provider)
        .into_iter()
        .find(|e| e.id == model_id)
        .and_then(|e| e.context_length)
        .filter(|&n| n > 0);
    cached.or_else(|| crate::agent::context_window::fallback_context_length(provider, model_id))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveContextWindow {
    pub provider: AgentProviderKind,
    pub model_id: String,
    /// Max context window in tokens, or `None` when unknown.
    pub context_length: Option<u64>,
}

/// Report the active provider/model and its resolved context-window size.
/// Polled by the chat header's occupancy meter.
#[tauri::command]
pub fn agent_active_context_window(app: AppHandle) -> Result<ActiveContextWindow, String> {
    let settings = load_settings(&app)?;
    let context_length = resolve_context_length(&settings);
    Ok(ActiveContextWindow {
        provider: settings.provider,
        model_id: settings.model_id.clone(),
        context_length,
    })
}

fn settings_view(
    app: &AppHandle,
    settings: AgentProviderSettings,
) -> Result<AgentProviderSettingsView, String> {
    let key_statuses = [
        AgentProviderKind::Openrouter,
        AgentProviderKind::Anthropic,
        AgentProviderKind::Openai,
        AgentProviderKind::HuggingFace,
        AgentProviderKind::Cloudflare,
        AgentProviderKind::Together,
        AgentProviderKind::Portkey,
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
    /// OpenRouter encodes pricing as decimal strings. Missing fields are
    /// treated as free (e.g. `request` or `image` for text-only models).
    #[serde(default)]
    pricing: Option<OpenrouterModelPricing>,
    /// Max context window in tokens. Present for virtually every model.
    #[serde(default)]
    context_length: Option<u64>,
}

#[derive(Deserialize)]
struct OpenrouterModelPricing {
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    completion: Option<String>,
}

fn parse_openrouter_pricing(p: Option<OpenrouterModelPricing>) -> Option<ModelPricing> {
    let p = p?;
    let prompt = p.prompt.as_deref().and_then(|s| s.parse::<f64>().ok())?;
    let completion = p
        .completion
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())?;
    Some(ModelPricing { prompt, completion })
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

    let settings = load_settings(app)?;
    let spec = crate::agent::provider::spec(provider);

    match spec.model_discovery {
        crate::agent::provider::ModelDiscovery::OpenRouter => {
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
                    pricing: parse_openrouter_pricing(entry.pricing),
                    context_length: entry.context_length.filter(|&n| n > 0),
                })
                .collect::<Vec<_>>();
            items.sort_by(|a, b| a.label.cmp(&b.label));
            Ok(items)
        }
        crate::agent::provider::ModelDiscovery::OpenAiCompatible
        | crate::agent::provider::ModelDiscovery::Cloudflare => {
            let url = crate::agent::provider::models_url(&settings, provider)?;
            let key = if crate::agent::provider::provider_requires_key(provider) {
                Some(provider_key(app, provider)?)
            } else {
                None
            };
            let mut req = client.get(&url);
            if let Some(key) = key {
                req = req.bearer_auth(key);
            }
            let res = req
                .send()
                .await
                .map_err(|e| format!("{} models: {e}", provider.as_str()))?;
            let res = res
                .error_for_status()
                .map_err(|e| format!("{} models: {e}", provider.as_str()))?;
            let body: OpenaiModelsEnvelope = res
                .json()
                .await
                .map_err(|e| format!("{} parse: {e}", provider.as_str()))?;
            let mut items = body
                .data
                .into_iter()
                .map(|entry| ProviderModelEntry {
                    label: entry.id.clone(),
                    id: entry.id,
                    description: None,
                    pricing: None,
                    context_length: None,
                })
                .collect::<Vec<_>>();
            items.sort_by(|a, b| a.label.cmp(&b.label));
            Ok(items)
        }
        crate::agent::provider::ModelDiscovery::Anthropic => {
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
                    pricing: None,
                    context_length: None,
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
    settings.tool_loop_limit = clamp_tool_loop_limit(patch.tool_loop_limit);
    settings.auto_compact_enabled = patch.auto_compact_enabled;
    settings.auto_compact_threshold_pct =
        clamp_auto_compact_threshold_pct(patch.auto_compact_threshold_pct);
    settings.orb_mode = patch.orb_mode;
    settings.agent_nickname = crate::agent::nickname::validate_nickname(&patch.agent_nickname)
        .map_err(|e| {
            // Surface the stable reason code; the UI maps it to a localized message.
            format!("nickname:{}", e.reason_code())
        })?;
    settings.default_session_role = normalize_session_role(patch.default_session_role);
    settings.provider_base_urls = patch.provider_base_urls;
    settings.cloudflare_account_id = patch.cloudflare_account_id.trim().to_string();
    normalize_provider_settings(&mut settings);
    save_settings(&app, &settings)?;
    settings_view(&app, settings)
}

#[tauri::command]
pub fn agent_onboarding_complete(
    app: AppHandle,
    agent_nickname: String,
    default_session_role: Option<String>,
) -> Result<AgentProviderSettingsView, String> {
    let mut settings = load_settings(&app)?;
    settings.agent_nickname = crate::agent::nickname::validate_nickname(&agent_nickname)
        .map_err(|e| format!("nickname:{}", e.reason_code()))?;
    settings.default_session_role = normalize_session_role(default_session_role);
    settings.onboarding_seen = true;
    normalize_provider_settings(&mut settings);
    save_settings(&app, &settings)?;
    settings_view(&app, settings)
}

/// Validate a candidate agent nickname without persisting it. Returns `Ok(())`
/// when acceptable (including blank = "use default"), or `Err(reason_code)`
/// (`tooLong` / `invalidChars` / `badWord`) for live UI feedback.
#[tauri::command]
pub fn agent_validate_nickname(name: String) -> Result<(), String> {
    crate::agent::nickname::validate_nickname(&name)
        .map(|_| ())
        .map_err(|e| e.reason_code().to_string())
}

/// Lists the built-in BLXCode harness session roles (specialized skills) for
/// the Create-Workspace session-mode picker. Each entry carries the slug,
/// title, description, declared skills/tools, accent color, and suggested model.
#[tauri::command]
pub fn agent_session_roles_list() -> Vec<crate::agent::session_roles::RoleMeta> {
    crate::agent::session_roles::list_roles()
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
    let key_statuses = crate::agent::provider::all_providers()
        .iter()
        .map(|spec| {
            serde_json::json!({
                "id": spec.id,
                "provider": spec.kind.as_str(),
                "label": spec.label,
                "class": crate::agent::provider::class_label(spec.class),
                "configured": false,
                "requiresKey": crate::agent::provider::provider_requires_key(spec.kind),
            })
        })
        .collect::<Vec<_>>();

    serde_json::json!({
        "phase": "mock_engine",
        "defaultProvider": AgentProviderKind::Openrouter.as_str(),
        "keyStatuses": key_statuses,
    })
}

pub(crate) fn mask_secret_pub(secret: &str) -> Option<String> {
    mask_secret(secret)
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
