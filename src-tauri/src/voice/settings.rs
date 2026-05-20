//! Persistent voice settings, stored as an optional `voice` sub-object inside
//! the existing `agent_provider_settings.json`. Defaults keep the feature
//! conservative: 16 kHz mic, follow-app language, push-to-talk via Space.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::agent_settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceProviderKind {
    Openai,
    Openrouter,
}

impl VoiceProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
        }
    }
}

impl Default for VoiceProviderKind {
    fn default() -> Self {
        Self::Openai
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SttSettings {
    pub provider: VoiceProviderKind,
    pub model_id: String,
    pub sample_rate_hz: u32,
}

impl Default for SttSettings {
    fn default() -> Self {
        Self {
            provider: VoiceProviderKind::Openai,
            model_id: "gpt-4o-mini-transcribe".into(),
            sample_rate_hz: 16_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsSettings {
    pub provider: VoiceProviderKind,
    pub model_id: String,
    pub voice: String,
    pub enabled: bool,
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            provider: VoiceProviderKind::Openai,
            model_id: "gpt-4o-mini-tts".into(),
            voice: "nova".into(),
            enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PostSttFlow {
    AutoSend,
    Draft,
}

impl Default for PostSttFlow {
    fn default() -> Self {
        Self::AutoSend
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum SttLanguageMode {
    FollowApp,
    AutoDetect,
    Manual { code: String },
}

impl Default for SttLanguageMode {
    fn default() -> Self {
        Self::FollowApp
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

impl Default for PttHotkey {
    fn default() -> Self {
        Self {
            enabled: true,
            code: "Space".into(),
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettings {
    #[serde(default)]
    pub stt: SttSettings,
    #[serde(default)]
    pub tts: TtsSettings,
    #[serde(default)]
    pub post_stt_flow: PostSttFlow,
    #[serde(default)]
    pub stt_language: SttLanguageMode,
    #[serde(default)]
    pub ptt_hotkey: PttHotkey,
}

pub fn load(app: &AppHandle) -> Result<VoiceSettings, String> {
    let envelope = agent_settings::read_envelope(app)?;
    Ok(envelope
        .get("voice")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default())
}

pub fn save(app: &AppHandle, settings: &VoiceSettings) -> Result<VoiceSettings, String> {
    let mut envelope = agent_settings::read_envelope(app)?;
    let value =
        serde_json::to_value(settings).map_err(|e| format!("serialize voice settings: {e}"))?;
    envelope.insert("voice".into(), value);
    agent_settings::write_envelope(app, &envelope)?;
    Ok(settings.clone())
}

/// Resolve the API key used for a voice provider, piggybacking on the
/// existing agent provider keyring entries.
pub fn provider_key(app: &AppHandle, provider: VoiceProviderKind) -> Result<String, String> {
    let agent_provider = match provider {
        VoiceProviderKind::Openai => agent_settings::AgentProviderKind::Openai,
        VoiceProviderKind::Openrouter => agent_settings::AgentProviderKind::Openrouter,
    };
    agent_settings::provider_key_pub(app, agent_provider)
}
