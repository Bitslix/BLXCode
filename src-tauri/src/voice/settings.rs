//! Persistent voice settings, stored as an optional `voice` sub-object inside
//! the existing `agent_provider_settings.json`. Defaults keep the feature
//! conservative: 16 kHz mic, follow-app language, push-to-talk via Space.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::agent_settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum VoiceProviderKind {
    #[default]
    Openai,
    Openrouter,
    Aws,
}

impl VoiceProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Openrouter => "openrouter",
            Self::Aws => "aws",
        }
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
#[derive(Default)]
pub enum PostSttFlow {
    #[default]
    AutoSend,
    Draft,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "mode")]
#[derive(Default)]
pub enum SttLanguageMode {
    #[default]
    FollowApp,
    AutoDetect,
    Manual {
        code: String,
    },
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

/// Push-to-talk backend selection. Local-first by default for privacy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum PttMode {
    #[default]
    Local,
    Cloud,
}

/// Decode-quality preset for the local whisper engine. This is an *inference*
/// parameter (threads/beam/strategy) and is independent of the chosen model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum WhisperQuality {
    Fast,
    #[default]
    Balanced,
    Best,
}

/// Where a finalized PTT transcript is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum PttInsertTarget {
    #[default]
    Agent,
    Terminal,
    ActiveInput,
    Clipboard,
}

/// Whether the insert target follows the focus at release time, or is pinned
/// at the moment recording starts (so a focus change mid-utterance is ignored).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum PttTargetMode {
    #[default]
    CurrentFocus,
    RememberStart,
}

/// What to do when PTT starts while TTS is still playing — avoids a feedback
/// loop where the mic captures the assistant's own voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum TtsCollision {
    Stop,
    Pause,
    #[default]
    Block,
}

/// Push-to-talk settings. Stored under `voice.ptt`; every field carries a
/// `#[serde(default)]` so older configs without a `ptt` object keep loading.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PttSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub mode: PttMode,
    #[serde(default)]
    pub local_model_path: Option<String>,
    #[serde(default)]
    pub local_quality: WhisperQuality,
    #[serde(default)]
    pub cloud_provider: VoiceProviderKind,
    #[serde(default = "default_cloud_model_id")]
    pub cloud_model_id: String,
    #[serde(default)]
    pub insert_target: PttInsertTarget,
    #[serde(default)]
    pub target_mode: PttTargetMode,
    #[serde(default)]
    pub auto_submit: bool,
    #[serde(default = "default_true")]
    pub partial_transcript: bool,
    #[serde(default)]
    pub tts_collision: TtsCollision,
}

fn default_true() -> bool {
    true
}

fn default_cloud_model_id() -> String {
    "gpt-4o-mini-transcribe".into()
}

impl Default for PttSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: PttMode::default(),
            local_model_path: None,
            local_quality: WhisperQuality::default(),
            cloud_provider: VoiceProviderKind::default(),
            cloud_model_id: default_cloud_model_id(),
            insert_target: PttInsertTarget::default(),
            target_mode: PttTargetMode::default(),
            auto_submit: false,
            partial_transcript: true,
            tts_collision: TtsCollision::default(),
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
    #[serde(default)]
    pub ptt: PttSettings,
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
    match provider {
        VoiceProviderKind::Aws => {
            crate::media_keys::resolve_key(crate::media_keys::MediaKeyKind::AwsPolly).ok_or_else(
                || "AWS API key missing. Add it under Settings → API Keys (Amazon Polly).".into(),
            )
        }
        VoiceProviderKind::Openai => {
            agent_settings::provider_key_pub(app, agent_settings::AgentProviderKind::Openai)
        }
        VoiceProviderKind::Openrouter => {
            agent_settings::provider_key_pub(app, agent_settings::AgentProviderKind::Openrouter)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ptt_defaults_are_conservative_but_partials_on() {
        let p = PttSettings::default();
        assert!(!p.enabled);
        assert_eq!(p.mode, PttMode::Local);
        assert_eq!(p.local_quality, WhisperQuality::Balanced);
        assert_eq!(p.insert_target, PttInsertTarget::Agent);
        assert_eq!(p.target_mode, PttTargetMode::CurrentFocus);
        assert!(!p.auto_submit);
        // Live text out of the box, per design.
        assert!(p.partial_transcript);
        assert_eq!(p.tts_collision, TtsCollision::Block);
        assert!(p.local_model_path.is_none());
    }

    #[test]
    fn ptt_settings_serde_roundtrip() {
        let p = PttSettings {
            enabled: true,
            mode: PttMode::Cloud,
            local_model_path: Some("/models/base.bin".into()),
            local_quality: WhisperQuality::Best,
            cloud_provider: VoiceProviderKind::Openrouter,
            cloud_model_id: "whisper-1".into(),
            insert_target: PttInsertTarget::Terminal,
            target_mode: PttTargetMode::RememberStart,
            auto_submit: true,
            partial_transcript: false,
            tts_collision: TtsCollision::Pause,
        };
        let json = serde_json::to_string(&p).expect("serialize");
        let back: PttSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p.enabled, back.enabled);
        assert_eq!(p.mode, back.mode);
        assert_eq!(p.local_model_path, back.local_model_path);
        assert_eq!(p.local_quality, back.local_quality);
        assert_eq!(p.cloud_provider, back.cloud_provider);
        assert_eq!(p.cloud_model_id, back.cloud_model_id);
        assert_eq!(p.insert_target, back.insert_target);
        assert_eq!(p.target_mode, back.target_mode);
        assert_eq!(p.auto_submit, back.auto_submit);
        assert_eq!(p.partial_transcript, back.partial_transcript);
        assert_eq!(p.tts_collision, back.tts_collision);
    }

    #[test]
    fn old_envelope_without_ptt_still_loads() {
        // Simulates a pre-PTT `voice` object: no `ptt` key present.
        let raw = r#"{
            "stt": {"provider":"openai","modelId":"gpt-4o-mini-transcribe","sampleRateHz":16000},
            "tts": {"provider":"openai","modelId":"gpt-4o-mini-tts","voice":"nova","enabled":true},
            "postSttFlow":"autoSend",
            "sttLanguage":{"mode":"followApp"},
            "pttHotkey":{"enabled":true,"code":"Space"}
        }"#;
        let v: VoiceSettings = serde_json::from_str(raw).expect("legacy voice envelope");
        // `ptt` falls back to defaults — no regression.
        assert!(!v.ptt.enabled);
        assert!(v.ptt.partial_transcript);
        assert_eq!(v.ptt.mode, PttMode::Local);
    }

    #[test]
    fn partial_transcript_defaults_true_when_key_absent() {
        // A partial `ptt` object that omits `partialTranscript`.
        let raw = r#"{"enabled":true,"mode":"local"}"#;
        let p: PttSettings = serde_json::from_str(raw).expect("partial ptt");
        assert!(p.partial_transcript);
        assert_eq!(p.cloud_model_id, "gpt-4o-mini-transcribe");
    }
}
