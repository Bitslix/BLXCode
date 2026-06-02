//! Tauri command surface for the voice subsystem.

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use super::recorder::{self, VoiceRecorderState};
use super::settings::{self, VoiceProviderKind, VoiceSettings};
use super::stt;
use super::tts;

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
    crate::app_logging::write_app_event(
        &app,
        "info",
        "voice",
        "stt_recording_start_requested",
        serde_json::json!({ "sampleRateHz": rate }),
    );
    let turn_id = match recorder::start(&app, state.inner(), rate) {
        Ok(turn_id) => turn_id,
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "stt_recording_start_failed",
                serde_json::json!({ "error": error.clone() }),
            );
            return Err(error);
        }
    };
    crate::app_logging::write_app_event(
        &app,
        "info",
        "voice",
        "stt_recording_started",
        serde_json::json!({ "sampleRateHz": rate }),
    );
    Ok(VoiceStartResponse { turn_id })
}

#[tauri::command]
pub async fn voice_stop_and_transcribe(
    app: AppHandle,
    state: State<'_, Arc<VoiceRecorderState>>,
    payload: VoiceStopPayload,
) -> Result<VoiceStopResponse, String> {
    crate::app_logging::write_app_event(
        &app,
        "info",
        "voice",
        "stt_transcription_requested",
        serde_json::json!({ "localeHint": payload.locale_hint }),
    );
    let wav_path = match recorder::stop(state.inner(), &payload.turn_id) {
        Ok(path) => path,
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "stt_recording_stop_failed",
                serde_json::json!({ "error": error.clone() }),
            );
            return Err(error);
        }
    };
    let voice_settings = match settings::load(&app) {
        Ok(settings) => settings,
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "settings_load_failed",
                serde_json::json!({ "scope": "stt", "error": error.clone() }),
            );
            return Err(error);
        }
    };
    let api_key = match settings::provider_key(&app, voice_settings.stt.provider) {
        Ok(key) => key,
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "stt_provider_key_failed",
                serde_json::json!({
                    "provider": voice_settings.stt.provider.as_str(),
                    "error": error.clone(),
                }),
            );
            return Err(error);
        }
    };
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
        Ok(t) => {
            crate::app_logging::write_app_event(
                &app,
                "info",
                "voice",
                "stt_transcription_finished",
                serde_json::json!({
                    "provider": voice_settings.stt.provider.as_str(),
                    "model": voice_settings.stt.model_id,
                    "chars": t.chars().count(),
                }),
            );
            Ok(VoiceStopResponse { text: t })
        }
        Err(e) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "stt_transcription_failed",
                serde_json::json!({
                    "provider": voice_settings.stt.provider.as_str(),
                    "model": voice_settings.stt.model_id,
                    "error": e.clone(),
                }),
            );
            Err(e)
        }
    }
}

#[tauri::command]
pub fn voice_cancel_recording(
    app: AppHandle,
    state: State<Arc<VoiceRecorderState>>,
    payload: VoiceCancelPayload,
) -> Result<(), String> {
    match recorder::cancel(state.inner(), &payload.turn_id) {
        Ok(()) => {
            crate::app_logging::write_app_event(
                &app,
                "info",
                "voice",
                "stt_recording_cancelled",
                serde_json::json!({}),
            );
            Ok(())
        }
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "stt_recording_cancel_failed",
                serde_json::json!({ "error": error.clone() }),
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub fn voice_settings_get(app: AppHandle) -> Result<VoiceSettings, String> {
    match settings::load(&app) {
        Ok(settings) => Ok(settings),
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "settings_load_failed",
                serde_json::json!({ "scope": "voice", "error": error.clone() }),
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub fn voice_settings_save(app: AppHandle, patch: VoiceSettings) -> Result<VoiceSettings, String> {
    match settings::save(&app, &patch) {
        Ok(settings) => {
            crate::app_logging::write_app_event(
                &app,
                "info",
                "voice",
                "settings_saved",
                serde_json::json!({
                    "sttProvider": settings.stt.provider.as_str(),
                    "ttsProvider": settings.tts.provider.as_str(),
                    "pttEnabled": settings.ptt.enabled,
                }),
            );
            Ok(settings)
        }
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "settings_save_failed",
                serde_json::json!({ "error": error.clone() }),
            );
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn voice_tts_preview(
    app: AppHandle,
    payload: VoiceTtsPreviewPayload,
) -> Result<VoiceTtsPreviewResponse, String> {
    crate::app_logging::write_app_event(
        &app,
        "info",
        "voice",
        "tts_preview_requested",
        serde_json::json!({
            "provider": payload.provider.as_str(),
            "model": payload.model_id,
            "voice": payload.voice,
            "textChars": payload.text.chars().count(),
        }),
    );
    let api_key = match settings::provider_key(&app, payload.provider) {
        Ok(key) => key,
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "tts_provider_key_failed",
                serde_json::json!({
                    "provider": payload.provider.as_str(),
                    "error": error.clone(),
                }),
            );
            return Err(error);
        }
    };
    let bytes = match tts::synthesize(
        payload.provider,
        &payload.model_id,
        &payload.voice,
        &payload.text,
        &api_key,
    )
    .await
    {
        Ok(bytes) => bytes,
        Err(error) => {
            crate::app_logging::write_app_event(
                &app,
                "error",
                "voice",
                "tts_preview_failed",
                serde_json::json!({
                    "provider": payload.provider.as_str(),
                    "model": payload.model_id,
                    "voice": payload.voice,
                    "error": error.clone(),
                }),
            );
            return Err(error);
        }
    };
    crate::app_logging::write_app_event(
        &app,
        "info",
        "voice",
        "tts_preview_finished",
        serde_json::json!({
            "provider": payload.provider.as_str(),
            "model": payload.model_id,
            "voice": payload.voice,
            "bytes": bytes.len(),
        }),
    );
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
