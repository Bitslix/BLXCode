//! Persistent image-mode settings, stored as the `image` envelope key inside
//! `agent_provider_settings.json` alongside `voice`. API keys are reused from
//! the existing agent provider keyring entries.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::agent_settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl Default for ImageProviderKind {
    fn default() -> Self {
        Self::Openai
    }
}

/// Output quality / resolution tier for image generation (UI: parallel to text
/// `ThinkingLevel`, without an "off" step).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ImageQualityLevel {
    Low,
    #[default]
    Medium,
    High,
    Max,
}

impl ImageQualityLevel {
    /// OpenAI `quality` for GPT Image / similar (`low` | `medium` | `high`).
    pub fn openai_gpt_quality(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High | Self::Max => "high",
        }
    }

    /// OpenAI DALL·E 3 `quality` (`standard` | `hd`).
    pub fn openai_dalle_quality(self) -> &'static str {
        match self {
            Self::Low | Self::Medium => "standard",
            Self::High | Self::Max => "hd",
        }
    }

    /// OpenAI `size` for generations / edits.
    pub fn openai_size(self) -> &'static str {
        match self {
            Self::Low => "1024x1024",
            Self::Medium => "1024x1024",
            Self::High => "1536x1024",
            Self::Max => "1792x1024",
        }
    }

    /// OpenRouter `image_config.quality` hint when supported by the model.
    pub fn openrouter_image_quality(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "ultra",
        }
    }

    /// fal.ai `image_size` preset.
    pub fn fal_image_size(self) -> &'static str {
        match self {
            Self::Low => "square",
            Self::Medium => "square_hd",
            Self::High => "landscape_4_3",
            Self::Max => "landscape_16_9",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

/// Load the image settings (or defaults). Reads the shared envelope.
pub fn load(app: &AppHandle) -> Result<ImageSettings, String> {
    let envelope = agent_settings::read_envelope(app)?;
    Ok(envelope
        .get("image")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default())
}

/// Save image settings, preserving every other envelope key (e.g. `voice`,
/// agent-provider fields).
pub fn save(app: &AppHandle, settings: &ImageSettings) -> Result<ImageSettings, String> {
    let mut envelope = agent_settings::read_envelope(app)?;
    let value =
        serde_json::to_value(settings).map_err(|e| format!("serialize image settings: {e}"))?;
    envelope.insert("image".into(), value);
    agent_settings::write_envelope(app, &envelope)?;
    Ok(settings.clone())
}

/// Resolve the API key used for an image provider, piggybacking on the
/// existing agent provider keyring entries.
pub fn provider_key(app: &AppHandle, provider: ImageProviderKind) -> Result<String, String> {
    match provider {
        ImageProviderKind::Openai => {
            agent_settings::provider_key_pub(app, agent_settings::AgentProviderKind::Openai)
        }
        ImageProviderKind::Openrouter => {
            agent_settings::provider_key_pub(app, agent_settings::AgentProviderKind::Openrouter)
        }
        ImageProviderKind::Fal => {
            crate::media_keys::resolve_key(crate::media_keys::MediaKeyKind::Fal).ok_or_else(|| {
                format!(
                    "fal.ai API key missing — set it in Settings → API Keys (env: {})",
                    crate::media_keys::MediaKeyKind::Fal.env_var()
                )
            })
        }
    }
}

/// Curated fallback model lists per provider, used when the live model
/// catalog filter returns no candidates.
pub fn curated_image_models(provider: ImageProviderKind) -> Vec<CuratedImageModel> {
    match provider {
        ImageProviderKind::Openai => vec![
            CuratedImageModel {
                id: "gpt-image-1".into(),
                label: "GPT Image 1".into(),
            },
            CuratedImageModel {
                id: "dall-e-3".into(),
                label: "DALL·E 3".into(),
            },
        ],
        ImageProviderKind::Openrouter => vec![
            CuratedImageModel {
                id: "google/gemini-2.5-flash-image".into(),
                label: "Gemini 2.5 Flash Image".into(),
            },
            CuratedImageModel {
                id: "openai/gpt-image-1".into(),
                label: "GPT Image 1 (via OpenRouter)".into(),
            },
        ],
        ImageProviderKind::Fal => vec![
            CuratedImageModel {
                id: "fal-ai/flux/schnell".into(),
                label: "FLUX Schnell".into(),
            },
            CuratedImageModel {
                id: "fal-ai/flux/dev".into(),
                label: "FLUX.1 [dev]".into(),
            },
            CuratedImageModel {
                id: "fal-ai/nano-banana-2".into(),
                label: "Nano Banana 2".into(),
            },
            CuratedImageModel {
                id: "fal-ai/gpt-image-1.5".into(),
                label: "GPT Image 1.5".into(),
            },
        ],
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuratedImageModel {
    pub id: String,
    pub label: String,
}

/// Cheap heuristic for filtering a regular text-model list down to
/// likely image-generation candidates. Mirrors the same idea used by
/// the voice pane's `ModelKind`. Used by the frontend via an exposed
/// command when we add live model fetching; covered by unit tests today.
#[allow(dead_code)]
pub fn looks_like_image_model(model_id_or_label: &str) -> bool {
    let lower = model_id_or_label.to_ascii_lowercase();
    lower.contains("image")
        || lower.contains("dall-e")
        || lower.contains("dalle")
        || lower.contains("gpt-image")
        || lower.contains("flux")
        || lower.contains("stable-diffusion")
        || lower.contains("sdxl")
        || lower.contains("imagen")
        || lower.contains("gemini-2.5-flash-image")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_matches_known_image_models() {
        assert!(looks_like_image_model("gpt-image-1"));
        assert!(looks_like_image_model("openai/dall-e-3"));
        assert!(looks_like_image_model("black-forest-labs/flux-1.1-pro"));
        assert!(looks_like_image_model("stability-ai/sdxl"));
        assert!(looks_like_image_model("google/gemini-2.5-flash-image"));
    }

    #[test]
    fn heuristic_rejects_text_models() {
        assert!(!looks_like_image_model("gpt-5"));
        assert!(!looks_like_image_model("claude-sonnet-4-5"));
        assert!(!looks_like_image_model("openai/gpt-4o-mini-transcribe"));
    }

    #[test]
    fn default_settings_use_openai_gpt_image() {
        let s = ImageSettings::default();
        assert!(matches!(s.provider, ImageProviderKind::Openai));
        assert_eq!(s.model_id, "gpt-image-1");
        assert_eq!(s.quality, ImageQualityLevel::Medium);
    }

    #[test]
    fn quality_maps_openai_tiers() {
        assert_eq!(ImageQualityLevel::Low.openai_gpt_quality(), "low");
        assert_eq!(ImageQualityLevel::Max.openai_dalle_quality(), "hd");
        assert_eq!(ImageQualityLevel::High.openai_size(), "1536x1024");
    }
}
