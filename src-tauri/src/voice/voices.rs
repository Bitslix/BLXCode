//! Static catalog of TTS voices per provider with a Gender hint.
//!
//! Sources: <https://platform.openai.com/docs/guides/text-to-speech>.

use serde::{Deserialize, Serialize};

use super::settings::VoiceProviderKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceGender {
    Male,
    Female,
    Neutral,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceEntry {
    pub id: String,
    pub label: String,
    pub gender: VoiceGender,
}

pub fn voices_for(provider: VoiceProviderKind) -> Vec<VoiceEntry> {
    match provider {
        VoiceProviderKind::Openai => openai_voices(),
        VoiceProviderKind::Openrouter => Vec::new(),
    }
}

fn openai_voices() -> Vec<VoiceEntry> {
    let raw: &[(&str, &str, VoiceGender)] = &[
        ("alloy", "Alloy", VoiceGender::Neutral),
        ("ash", "Ash", VoiceGender::Male),
        ("ballad", "Ballad", VoiceGender::Female),
        ("coral", "Coral", VoiceGender::Female),
        ("echo", "Echo", VoiceGender::Male),
        ("fable", "Fable", VoiceGender::Neutral),
        ("nova", "Nova", VoiceGender::Female),
        ("onyx", "Onyx", VoiceGender::Male),
        ("sage", "Sage", VoiceGender::Female),
        ("shimmer", "Shimmer", VoiceGender::Female),
    ];
    raw.iter()
        .map(|(id, label, gender)| VoiceEntry {
            id: (*id).to_string(),
            label: (*label).to_string(),
            gender: *gender,
        })
        .collect()
}
