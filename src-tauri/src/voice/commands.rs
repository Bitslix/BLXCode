//! Tauri command surface for the voice subsystem.

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use super::recorder::{self, VoiceRecorderState};
use super::settings::{self, VoiceProviderKind, VoiceSettings};
use super::stt;
use super::tts;
use super::voices::{self, VoiceEntry};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStartResponse {
    pub turn_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStartPayload {
    pub sample_rate_hz: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStopPayload {
    pub turn_id: String,
    #[serde(default)]
    pub locale_hint: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCancelPayload {
    pub turn_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStopResponse {
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProviderRef {
    pub provider: VoiceProviderKind,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProviderVoicesResponse {
    pub provider: VoiceProviderKind,
    pub voices: Vec<VoiceEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTtsPreviewPayload {
    pub provider: VoiceProviderKind,
    pub model_id: String,
    pub voice: String,
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTtsPreviewResponse {
    pub audio_b64: String,
    pub mime: String,
}

#[tauri::command]
pub fn voice_start_recording(
    app: AppHandle,
    state: State<Arc<VoiceRecorderState>>,
    payload: VoiceStartPayload,
) -> Result<VoiceStartResponse, String> {
    let rate = match payload.sample_rate_hz {
        0 => 16_000,
        r => r,
    };
    let turn_id = recorder::start(&app, state.inner(), rate)?;
    Ok(VoiceStartResponse { turn_id })
}

#[tauri::command]
pub async fn voice_stop_and_transcribe(
    app: AppHandle,
    state: State<'_, Arc<VoiceRecorderState>>,
    payload: VoiceStopPayload,
) -> Result<VoiceStopResponse, String> {
    let wav_path = recorder::stop(state.inner(), &payload.turn_id)?;
    let voice_settings = settings::load(&app)?;
    let api_key = settings::provider_key(&app, voice_settings.stt.provider)?;
    let language = payload
        .locale_hint
        .as_deref()
        .map(reduce_to_iso639_1)
        .filter(|s| !s.is_empty());

    let text = stt::transcribe_wav(
        voice_settings.stt.provider,
        &voice_settings.stt.model_id,
        &api_key,
        &wav_path,
        language.as_deref(),
    )
    .await;

    // Always delete the WAV — privacy + cache size.
    let _ = std::fs::remove_file(&wav_path);

    match text {
        Ok(t) => Ok(VoiceStopResponse { text: t }),
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub fn voice_cancel_recording(
    state: State<Arc<VoiceRecorderState>>,
    payload: VoiceCancelPayload,
) -> Result<(), String> {
    recorder::cancel(state.inner(), &payload.turn_id)
}

#[tauri::command]
pub fn voice_settings_get(app: AppHandle) -> Result<VoiceSettings, String> {
    settings::load(&app)
}

#[tauri::command]
pub fn voice_settings_save(app: AppHandle, patch: VoiceSettings) -> Result<VoiceSettings, String> {
    settings::save(&app, &patch)
}

#[tauri::command]
pub fn voice_provider_voices(
    payload: VoiceProviderRef,
) -> Result<VoiceProviderVoicesResponse, String> {
    Ok(VoiceProviderVoicesResponse {
        provider: payload.provider,
        voices: voices::voices_for(payload.provider),
    })
}

#[tauri::command]
pub async fn voice_tts_preview(
    app: AppHandle,
    payload: VoiceTtsPreviewPayload,
) -> Result<VoiceTtsPreviewResponse, String> {
    let api_key = settings::provider_key(&app, payload.provider)?;
    let bytes = tts::synthesize(
        payload.provider,
        &payload.model_id,
        &payload.voice,
        &payload.text,
        &api_key,
    )
    .await?;
    Ok(VoiceTtsPreviewResponse {
        audio_b64: BASE64.encode(&bytes),
        mime: "audio/mpeg".into(),
    })
}

fn reduce_to_iso639_1(tag: &str) -> String {
    let t = tag.trim();
    if t.is_empty() {
        return String::new();
    }
    // BCP-47 like `de-DE`, `zh-CN` → primary subtag.
    let primary = t.split(|c| c == '-' || c == '_').next().unwrap_or("");
    primary.to_ascii_lowercase()
}

