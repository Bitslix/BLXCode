//! Shared data + helpers for the Agent settings pane: provider/level catalogs,
//! label/icon lookups, model fetching, API-key status text, the reply-voice
//! catalog and TTS preview playback.
//!
//! These are consolidated here (rule-reusable-components) so the four section
//! components and the orchestrator share one source of truth.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use js_sys::Uint8Array;
use leptos::prelude::*;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement};

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, image_curated_models, AgentProviderKind, AgentProviderSettingsView,
    ApiKeyEntry, ApiKeysStatus, ImageProviderKind, ImageQualityLevel, ProviderModelEntry,
    ThinkingLevel, VoiceEntry, VoiceGender, VoiceProviderKind,
};

// ---------------------------------------------------------------------------
// Catalogs
// ---------------------------------------------------------------------------

pub(crate) const AGENT_PROVIDERS: &[AgentProviderKind] = &[
    AgentProviderKind::Ollama,
    AgentProviderKind::LmStudio,
    AgentProviderKind::Openrouter,
    AgentProviderKind::Anthropic,
    AgentProviderKind::Openai,
    AgentProviderKind::HuggingFace,
    AgentProviderKind::Cloudflare,
    AgentProviderKind::Together,
    AgentProviderKind::Portkey,
];

pub(crate) fn thinking_levels() -> [ThinkingLevel; 5] {
    [
        ThinkingLevel::Off,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Max,
    ]
}

pub(crate) fn image_providers() -> [ImageProviderKind; 3] {
    [
        ImageProviderKind::Openrouter,
        ImageProviderKind::Openai,
        ImageProviderKind::Fal,
    ]
}

pub(crate) fn image_quality_levels() -> [ImageQualityLevel; 4] {
    [
        ImageQualityLevel::Low,
        ImageQualityLevel::Medium,
        ImageQualityLevel::High,
        ImageQualityLevel::Max,
    ]
}

pub(crate) fn voice_providers() -> [VoiceProviderKind; 3] {
    [
        VoiceProviderKind::Openai,
        VoiceProviderKind::Openrouter,
        VoiceProviderKind::Aws,
    ]
}

/// Client-side gender filter for the reply-voice grid (not persisted).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenderFilter {
    All,
    Male,
    Female,
    Neutral,
}

impl GenderFilter {
    pub(crate) fn matches(self, g: VoiceGender) -> bool {
        matches!(
            (self, g),
            (Self::All, _)
                | (Self::Male, VoiceGender::Male)
                | (Self::Female, VoiceGender::Female)
                | (Self::Neutral, VoiceGender::Neutral)
        )
    }
}

// ---------------------------------------------------------------------------
// Labels / icons
// ---------------------------------------------------------------------------

pub(crate) fn provider_label(i18n: &I18nService, provider: AgentProviderKind) -> String {
    let key = match provider {
        AgentProviderKind::Openrouter => I18nKey::AgProviderOpenrouter,
        AgentProviderKind::Anthropic => I18nKey::AgProviderAnthropic,
        AgentProviderKind::Openai => I18nKey::AgProviderOpenai,
        AgentProviderKind::Ollama => I18nKey::AgProviderOllama,
        AgentProviderKind::LmStudio => I18nKey::AgProviderLmStudio,
        AgentProviderKind::HuggingFace => I18nKey::AgProviderHuggingFace,
        AgentProviderKind::Cloudflare => I18nKey::AgProviderCloudflare,
        AgentProviderKind::Together => I18nKey::AgProviderTogether,
        AgentProviderKind::Portkey => I18nKey::AgProviderPortkey,
    };
    i18n.tr(key)().to_string()
}

pub(crate) fn provider_icon_url(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        AgentProviderKind::Anthropic => "/public/brand-icons/anthropic.svg",
        AgentProviderKind::Openai => "/public/brand-icons/openai.svg",
        _ => "/public/brand-icons/provider.svg",
    }
}

pub(crate) fn provider_requires_key(provider: AgentProviderKind) -> bool {
    !matches!(
        provider,
        AgentProviderKind::Ollama | AgentProviderKind::LmStudio
    )
}

pub(crate) fn local_provider_default_url(provider: AgentProviderKind) -> Option<&'static str> {
    match provider {
        AgentProviderKind::Ollama => Some("http://localhost:11434/v1"),
        AgentProviderKind::LmStudio => Some("http://localhost:1234/v1"),
        _ => None,
    }
}

pub(crate) fn thinking_label(i18n: &I18nService, level: ThinkingLevel) -> String {
    let key = match level {
        ThinkingLevel::Off => I18nKey::AgThinkingOff,
        ThinkingLevel::Low => I18nKey::AgThinkingLow,
        ThinkingLevel::Medium => I18nKey::AgThinkingMedium,
        ThinkingLevel::High => I18nKey::AgThinkingHigh,
        ThinkingLevel::Max => I18nKey::AgThinkingMax,
    };
    i18n.tr(key)().to_string()
}

pub(crate) fn thinking_icon(level: ThinkingLevel) -> icondata::Icon {
    match level {
        ThinkingLevel::Off => icondata::LuCircleOff,
        ThinkingLevel::Low => icondata::LuGauge,
        ThinkingLevel::Medium => icondata::LuActivity,
        ThinkingLevel::High => icondata::LuFlame,
        ThinkingLevel::Max => icondata::LuZap,
    }
}

pub(crate) fn image_provider_label(i18n: &I18nService, provider: ImageProviderKind) -> String {
    let key = match provider {
        ImageProviderKind::Openrouter => I18nKey::AgProviderOpenrouter,
        ImageProviderKind::Openai => I18nKey::AgProviderOpenai,
        ImageProviderKind::Fal => I18nKey::AgProviderFal,
    };
    i18n.tr(key)().to_string()
}

pub(crate) fn image_provider_icon_url(provider: ImageProviderKind) -> &'static str {
    match provider {
        ImageProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        ImageProviderKind::Openai => "/public/brand-icons/openai.svg",
        ImageProviderKind::Fal => "/public/brand-icons/fal.svg",
    }
}

pub(crate) fn image_quality_label(i18n: &I18nService, level: ImageQualityLevel) -> String {
    let key = match level {
        ImageQualityLevel::Low => I18nKey::AgImageQualityLow,
        ImageQualityLevel::Medium => I18nKey::AgImageQualityMedium,
        ImageQualityLevel::High => I18nKey::AgImageQualityHigh,
        ImageQualityLevel::Max => I18nKey::AgImageQualityMax,
    };
    i18n.tr(key)().to_string()
}

pub(crate) fn image_quality_icon(level: ImageQualityLevel) -> icondata::Icon {
    match level {
        ImageQualityLevel::Low => icondata::LuGauge,
        ImageQualityLevel::Medium => icondata::LuImage,
        ImageQualityLevel::High => icondata::LuSparkles,
        ImageQualityLevel::Max => icondata::LuZap,
    }
}

pub(crate) fn voice_provider_label(i18n: &I18nService, provider: VoiceProviderKind) -> String {
    match provider {
        VoiceProviderKind::Openai => i18n.tr(I18nKey::AgProviderOpenai)().to_string(),
        VoiceProviderKind::Openrouter => i18n.tr(I18nKey::AgProviderOpenrouter)().to_string(),
        VoiceProviderKind::Aws => i18n.tr(I18nKey::AgProviderAws)().to_string(),
    }
}

pub(crate) fn voice_provider_icon_url(provider: VoiceProviderKind) -> &'static str {
    match provider {
        VoiceProviderKind::Openai => "/public/brand-icons/openai.svg",
        VoiceProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        VoiceProviderKind::Aws => "/public/brand-icons/aws.svg",
    }
}

// ---------------------------------------------------------------------------
// API-key status text
// ---------------------------------------------------------------------------

fn masked(i18n: &I18nService, mask: Option<&String>) -> String {
    match mask {
        Some(m) => format!("{} ({m})", i18n.tr(I18nKey::AgApiKeyConfigured)()),
        None => i18n.tr(I18nKey::AgApiKeyConfigured)().to_string(),
    }
}

pub(crate) fn agent_key_status_text(
    i18n: &I18nService,
    view: Option<&AgentProviderSettingsView>,
    provider: AgentProviderKind,
) -> String {
    if !provider_requires_key(provider) {
        return i18n.tr(I18nKey::AgProviderNoApiKeyRequired)().to_string();
    }
    let Some(view) = view else {
        return i18n.tr(I18nKey::AgApiKeyMissing)().to_string();
    };
    match view.key_statuses.iter().find(|s| s.provider == provider) {
        Some(s) if s.configured => masked(i18n, s.masked_value.as_ref()),
        _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
    }
}

fn media_key_entry<'a>(api_keys: &'a ApiKeysStatus, kind: &str) -> Option<&'a ApiKeyEntry> {
    api_keys.entries.iter().find(|e| e.kind == kind)
}

pub(crate) fn image_to_agent_provider(p: ImageProviderKind) -> Option<AgentProviderKind> {
    match p {
        ImageProviderKind::Openai => Some(AgentProviderKind::Openai),
        ImageProviderKind::Openrouter => Some(AgentProviderKind::Openrouter),
        ImageProviderKind::Fal => None,
    }
}

pub(crate) fn voice_to_agent_provider(v: VoiceProviderKind) -> Option<AgentProviderKind> {
    match v {
        VoiceProviderKind::Openai => Some(AgentProviderKind::Openai),
        VoiceProviderKind::Openrouter => Some(AgentProviderKind::Openrouter),
        VoiceProviderKind::Aws => None,
    }
}

pub(crate) fn image_key_status_text(
    i18n: &I18nService,
    agent_settings: Option<&AgentProviderSettingsView>,
    api_keys: Option<&ApiKeysStatus>,
    provider: ImageProviderKind,
) -> String {
    if provider == ImageProviderKind::Fal {
        return match api_keys.and_then(|s| media_key_entry(s, "fal")) {
            Some(e) if e.configured => masked(i18n, e.masked_value.as_ref()),
            _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
        };
    }
    let agent = image_to_agent_provider(provider);
    match (agent_settings, agent) {
        (Some(view), Some(agent)) => match view.key_statuses.iter().find(|s| s.provider == agent) {
            Some(s) if s.configured => masked(i18n, s.masked_value.as_ref()),
            _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
        },
        _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
    }
}

pub(crate) fn voice_key_status_text(
    i18n: &I18nService,
    agent_settings: Option<&AgentProviderSettingsView>,
    api_keys: Option<&ApiKeysStatus>,
    provider: VoiceProviderKind,
) -> String {
    if provider == VoiceProviderKind::Aws {
        return match api_keys.and_then(|s| media_key_entry(s, "aws_polly")) {
            Some(e) if e.configured => masked(i18n, e.masked_value.as_ref()),
            _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
        };
    }
    let agent = voice_to_agent_provider(provider);
    match (agent_settings, agent) {
        (Some(view), Some(agent)) => match view.key_statuses.iter().find(|s| s.provider == agent) {
            Some(s) if s.configured => masked(i18n, s.masked_value.as_ref()),
            _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
        },
        _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Model selection / fetching
// ---------------------------------------------------------------------------

pub(crate) fn provider_cache(
    view: &AgentProviderSettingsView,
    provider: AgentProviderKind,
) -> Vec<ProviderModelEntry> {
    view.model_caches
        .get(provider.as_str())
        .cloned()
        .unwrap_or_else(|| match provider {
            AgentProviderKind::Openrouter => view.model_cache_openrouter.clone(),
            AgentProviderKind::Anthropic => view.model_cache_anthropic.clone(),
            AgentProviderKind::Openai => view.model_cache_openai.clone(),
            _ => Vec::new(),
        })
}

pub(crate) fn choose_model_for_provider(
    current: &str,
    previous_entries: &[ProviderModelEntry],
    entries: &[ProviderModelEntry],
) -> Option<String> {
    let trimmed = current.trim();
    if !trimmed.is_empty() && entries.iter().any(|entry| entry.id == trimmed) {
        return Some(trimmed.to_string());
    }
    if !trimmed.is_empty() && !previous_entries.iter().any(|entry| entry.id == trimmed) {
        return Some(trimmed.to_string());
    }
    entries.first().map(|entry| entry.id.clone())
}

fn looks_like_image_model(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("image")
        || l.contains("dall-e")
        || l.contains("dalle")
        || l.contains("gpt-image")
        || l.contains("flux")
        || l.contains("stable-diffusion")
        || l.contains("sdxl")
        || l.contains("imagen")
}

pub(crate) async fn fetch_image_models(
    provider: ImageProviderKind,
    out: RwSignal<Vec<ProviderModelEntry>>,
) {
    if provider == ImageProviderKind::Fal {
        if let Ok(resp) = image_curated_models(provider).await {
            out.set(
                resp.entries
                    .into_iter()
                    .map(|m| ProviderModelEntry {
                        id: m.id,
                        label: m.label,
                        description: None,
                        pricing: None,
                        context_length: None,
                    })
                    .collect(),
            );
        }
        return;
    }
    let Some(agent_provider) = image_to_agent_provider(provider) else {
        return;
    };
    let all = match agent_provider_models(agent_provider).await {
        Ok(resp) => resp.entries,
        Err(_) => Vec::new(),
    };
    let filtered: Vec<_> = all
        .iter()
        .filter(|m| looks_like_image_model(&m.id))
        .cloned()
        .collect();
    let mut entries = if filtered.is_empty() { all } else { filtered };
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    out.set(entries);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpeechKind {
    Stt,
    Tts,
}

fn speech_kind_matches(kind: SpeechKind, id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    match kind {
        SpeechKind::Stt => lower.contains("transcribe") || lower.contains("whisper"),
        SpeechKind::Tts => lower.contains("tts") || lower.contains("speech"),
    }
}

fn model_entry(id: &str, label: &str, description: &str) -> ProviderModelEntry {
    ProviderModelEntry {
        id: id.into(),
        label: label.into(),
        description: Some(description.into()),
        pricing: None,
        context_length: None,
    }
}

pub(crate) async fn fetch_voice_models(
    provider: VoiceProviderKind,
    kind: SpeechKind,
    out: RwSignal<Vec<ProviderModelEntry>>,
) {
    if provider == VoiceProviderKind::Aws {
        out.set(match kind {
            SpeechKind::Stt => vec![model_entry(
                "amazon-transcribe",
                "Amazon Transcribe",
                "AWS speech-to-text.",
            )],
            SpeechKind::Tts => vec![
                model_entry("neural", "Polly Neural", "Neural TTS engine."),
                model_entry("standard", "Polly Standard", "Standard TTS engine."),
            ],
        });
        return;
    }
    let Some(agent_provider) = voice_to_agent_provider(provider) else {
        out.set(Vec::new());
        return;
    };
    let all = match agent_provider_models(agent_provider).await {
        Ok(resp) => resp.entries,
        Err(_) => Vec::new(),
    };
    let filtered: Vec<_> = all
        .iter()
        .filter(|m| speech_kind_matches(kind, &m.id))
        .cloned()
        .collect();
    let mut entries = if filtered.is_empty() { all } else { filtered };
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    out.set(entries);
}

// ---------------------------------------------------------------------------
// Reply-voice catalog + preview playback
// ---------------------------------------------------------------------------

fn voice_entry(id: &str, label: &str, gender: VoiceGender) -> VoiceEntry {
    VoiceEntry {
        id: id.into(),
        label: label.into(),
        gender,
    }
}

pub(crate) fn voice_catalog_for(provider: VoiceProviderKind) -> Vec<VoiceEntry> {
    match provider {
        VoiceProviderKind::Openai | VoiceProviderKind::Openrouter => vec![
            voice_entry("alloy", "Alloy", VoiceGender::Neutral),
            voice_entry("nova", "Nova", VoiceGender::Female),
            voice_entry("echo", "Echo", VoiceGender::Male),
            voice_entry("shimmer", "Shimmer", VoiceGender::Female),
            voice_entry("onyx", "Onyx", VoiceGender::Male),
            voice_entry("coral", "Coral", VoiceGender::Female),
        ],
        VoiceProviderKind::Aws => vec![
            voice_entry("Joanna", "Joanna", VoiceGender::Female),
            voice_entry("Matthew", "Matthew", VoiceGender::Male),
            voice_entry("Amy", "Amy", VoiceGender::Female),
            voice_entry("Brian", "Brian", VoiceGender::Male),
            voice_entry("Emma", "Emma", VoiceGender::Female),
            voice_entry("Arthur", "Arthur", VoiceGender::Male),
        ],
    }
}

pub(crate) fn voices_pick_enabled(provider: VoiceProviderKind) -> bool {
    matches!(provider, VoiceProviderKind::Openai | VoiceProviderKind::Aws)
}

pub(crate) fn gender_icon(g: VoiceGender) -> &'static str {
    match g {
        VoiceGender::Male => "♂",
        VoiceGender::Female => "♀",
        VoiceGender::Neutral => "○",
    }
}

fn is_openai_voice_id(id: &str) -> bool {
    matches!(id, "alloy" | "nova" | "echo" | "shimmer" | "onyx" | "coral")
}

fn is_aws_voice_id(id: &str) -> bool {
    matches!(
        id,
        "Joanna" | "Matthew" | "Amy" | "Brian" | "Emma" | "Arthur"
    )
}

/// Pick sane STT/TTS model + voice defaults when the voice provider changes.
/// Only mutates the `stt`/`tts` sub-objects — never PTT.
pub(crate) fn apply_voice_provider_defaults(
    stt_model: &mut String,
    tts_model: &mut String,
    tts_voice: &mut String,
    provider: VoiceProviderKind,
) {
    match provider {
        VoiceProviderKind::Aws => {
            if !stt_model.contains("transcribe") {
                *stt_model = "amazon-transcribe".into();
            }
            if !matches!(tts_model.as_str(), "neural" | "standard") {
                *tts_model = "neural".into();
            }
            if tts_voice.is_empty() || is_openai_voice_id(tts_voice) {
                *tts_voice = "Joanna".into();
            }
        }
        VoiceProviderKind::Openai => {
            if tts_voice.is_empty() || is_aws_voice_id(tts_voice) {
                *tts_voice = "nova".into();
            }
        }
        VoiceProviderKind::Openrouter => {}
    }
}

pub(crate) fn nickname_err_key(code: &str) -> I18nKey {
    match code {
        "tooLong" => I18nKey::AgNicknameErrTooLong,
        "invalidChars" => I18nKey::AgNicknameErrInvalidChars,
        _ => I18nKey::AgNicknameErrBadWord,
    }
}

pub(crate) fn play_b64(b64: &str, mime: &str) {
    let Ok(bytes) = BASE64.decode(b64) else {
        return;
    };
    let Ok(el) = HtmlAudioElement::new() else {
        return;
    };
    let arr = Uint8Array::new_with_length(bytes.len() as u32);
    arr.copy_from(&bytes);
    let parts = js_sys::Array::new();
    parts.push(&arr.buffer());
    let opts = BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    el.set_src(&url);
    let _ = el.play();
}
