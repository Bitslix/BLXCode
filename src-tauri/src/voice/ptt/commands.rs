//! Tauri commands for the push-to-talk capture flow.
//!
//! Lifecycle: `ptt_start` (collision-checked) → optional `ptt_partial` polls
//! while the key is held → `ptt_finalize` returns the committed transcript →
//! or `ptt_cancel` discards it. Transcription is dispatched to the local
//! whisper engine or the cloud provider per `VoiceSettings.ptt.mode`.
//!
//! Target routing (agent composer / terminal / active input / clipboard) and
//! the partial-poll cadence live on the frontend; the backend only produces
//! text and owns the shared runtime state.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::voice::models;
use crate::voice::recorder::{self, VoiceRecorderState};
use crate::voice::settings::{self, PttMode, VoiceSettings};
use crate::voice::stt::{cloud, WhisperEngine};

use super::{PttStartDecision, VoiceRuntimeState, VoiceRuntimeStateHandle};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PttStartResponse {
    pub turn_id: Option<String>,
    pub started: bool,
    /// Resolved collision decision, so the frontend can stop/pause its own TTS
    /// audio element or show a "blocked" hint.
    pub decision: &'static str,
}

fn decision_str(d: PttStartDecision) -> &'static str {
    match d {
        PttStartDecision::Start => "start",
        PttStartDecision::StopTtsThenStart => "stopTts",
        PttStartDecision::PauseTtsThenStart => "pauseTts",
        PttStartDecision::RejectBusy => "rejectBusy",
        PttStartDecision::RejectTtsPlaying => "rejectTtsPlaying",
    }
}

fn sample_rate(settings: &VoiceSettings) -> u32 {
    match settings.stt.sample_rate_hz {
        0 => 16_000,
        r => r,
    }
}

/// Begin a push-to-talk recording, subject to the collision policy.
///
/// Async + `spawn_blocking`: settings load (JSON file), cpal device
/// enumeration, and the model warm-up are all blocking and must never run on
/// the Tauri main thread — a frozen UI on key-down is exactly what PTT must
/// avoid.
#[tauri::command]
pub async fn ptt_start(
    app: AppHandle,
    recorder: State<'_, Arc<VoiceRecorderState>>,
    runtime: State<'_, Arc<VoiceRuntimeStateHandle>>,
    whisper: State<'_, Arc<WhisperEngine>>,
) -> Result<PttStartResponse, String> {
    let recorder = recorder.inner().clone();
    let runtime = runtime.inner().clone();
    let whisper = whisper.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings = settings::load(&app)?;
        let decision = runtime.decide_ptt_start(settings.ptt.tts_collision);
        match decision {
            PttStartDecision::RejectBusy | PttStartDecision::RejectTtsPlaying => {
                Ok(PttStartResponse {
                    turn_id: None,
                    started: false,
                    decision: decision_str(decision),
                })
            }
            _ => {
                // Warm the model early in local mode so finalize is fast. Best
                // effort: a load failure surfaces on finalize with a clear error.
                if settings.ptt.mode == PttMode::Local {
                    if let Some(path) = settings.ptt.local_model_path.as_deref() {
                        let _ = whisper.ensure_loaded(path);
                    }
                }
                let turn_id = recorder::start_pcm(&recorder, sample_rate(&settings))?;
                runtime.set(VoiceRuntimeState::RecordingPtt);
                Ok(PttStartResponse {
                    turn_id: Some(turn_id),
                    started: true,
                    decision: decision_str(decision),
                })
            }
        }
    })
    .await
    .map_err(|e| format!("ptt_start task join: {e}"))?
}

/// Live partial transcript: snapshot the audio so far and decode it. Local
/// mode only (cloud re-decode per poll would be too slow/expensive). The
/// frontend throttles how often this is called.
#[tauri::command]
pub async fn ptt_partial(
    app: AppHandle,
    recorder: State<'_, Arc<VoiceRecorderState>>,
    whisper: State<'_, Arc<WhisperEngine>>,
    turn_id: String,
    locale_hint: Option<String>,
) -> Result<String, String> {
    let settings = settings::load(&app)?;
    if settings.ptt.mode != PttMode::Local || !settings.ptt.partial_transcript {
        return Ok(String::new());
    }
    let pcm = recorder::snapshot_pcm(recorder.inner(), &turn_id)?;
    // Skip until there's enough audio to be worth decoding (~0.4 s @ 16 kHz).
    if pcm.len() < 6_400 {
        return Ok(String::new());
    }
    transcribe_local(&app, whisper.inner().clone(), pcm, &settings, locale_hint).await
}

/// Stop recording and return the committed transcript.
#[tauri::command]
pub async fn ptt_finalize(
    app: AppHandle,
    recorder: State<'_, Arc<VoiceRecorderState>>,
    runtime: State<'_, Arc<VoiceRuntimeStateHandle>>,
    whisper: State<'_, Arc<WhisperEngine>>,
    turn_id: String,
    locale_hint: Option<String>,
) -> Result<String, String> {
    let settings = settings::load(&app)?;
    let pcm = recorder::stop_pcm(recorder.inner(), &turn_id)?;
    runtime.set(VoiceRuntimeState::TranscribingPtt);

    let result = match settings.ptt.mode {
        PttMode::Local => {
            transcribe_local(&app, whisper.inner().clone(), pcm, &settings, locale_hint).await
        }
        PttMode::Cloud => transcribe_cloud(&app, pcm, &settings, locale_hint).await,
    };

    runtime.set(VoiceRuntimeState::Idle);
    result
}

/// Frontend signal: TTS playback started/stopped. Lets push-to-talk avoid a
/// feedback loop (capturing the assistant's own voice). Only toggles between
/// `Idle` and `PlayingTts` so it never clobbers an active capture.
#[tauri::command]
pub fn voice_tts_playing(
    runtime: State<'_, Arc<VoiceRuntimeStateHandle>>,
    playing: bool,
) -> Result<(), String> {
    match (playing, runtime.current()) {
        (true, VoiceRuntimeState::Idle) => runtime.set(VoiceRuntimeState::PlayingTts),
        (false, VoiceRuntimeState::PlayingTts) => runtime.set(VoiceRuntimeState::Idle),
        _ => {}
    }
    Ok(())
}

/// Frontend signal: the existing agent voice-orb capture started/stopped, so
/// push-to-talk won't open a second microphone session.
#[tauri::command]
pub fn voice_agent_input_active(
    runtime: State<'_, Arc<VoiceRuntimeStateHandle>>,
    active: bool,
) -> Result<(), String> {
    match (active, runtime.current()) {
        (true, VoiceRuntimeState::Idle) => runtime.set(VoiceRuntimeState::AgentVoiceInputActive),
        (false, VoiceRuntimeState::AgentVoiceInputActive) => runtime.set(VoiceRuntimeState::Idle),
        _ => {}
    }
    Ok(())
}

/// Discard an in-flight recording.
#[tauri::command]
pub fn ptt_cancel(
    recorder: State<'_, Arc<VoiceRecorderState>>,
    runtime: State<'_, Arc<VoiceRuntimeStateHandle>>,
    turn_id: String,
) -> Result<(), String> {
    let res = recorder::cancel_pcm(recorder.inner(), &turn_id);
    runtime.set(VoiceRuntimeState::Idle);
    res
}

async fn transcribe_local(
    app: &AppHandle,
    whisper: Arc<WhisperEngine>,
    pcm: Vec<f32>,
    settings: &VoiceSettings,
    locale_hint: Option<String>,
) -> Result<String, String> {
    let path = settings
        .ptt
        .local_model_path
        .clone()
        .ok_or_else(|| "Kein lokales Whisper-Modell ausgewählt.".to_string())?;
    // Resolve to an installed model if the stored value is a catalog id.
    let resolved = resolve_model_path(&path);
    let quality = settings.ptt.local_quality;
    let lang = locale_hint
        .and_then(|h| reduce_to_iso639_1(&h))
        .filter(|s| !s.is_empty());
    let _ = app;

    tokio::task::spawn_blocking(move || {
        whisper.ensure_loaded(&resolved)?;
        whisper.transcribe_pcm(&pcm, quality, lang.as_deref())
    })
    .await
    .map_err(|e| format!("whisper task join: {e}"))?
}

async fn transcribe_cloud(
    app: &AppHandle,
    pcm: Vec<f32>,
    settings: &VoiceSettings,
    locale_hint: Option<String>,
) -> Result<String, String> {
    let provider = settings.ptt.cloud_provider;
    let api_key = settings::provider_key(app, provider)?;
    let lang = locale_hint
        .and_then(|h| reduce_to_iso639_1(&h))
        .filter(|s| !s.is_empty());
    cloud::transcribe_pcm(
        provider,
        &settings.ptt.cloud_model_id,
        &api_key,
        &pcm,
        sample_rate(settings),
        lang.as_deref(),
    )
    .await
}

/// If `value` is a bare catalog id, map it to its installed path; otherwise
/// treat it as an explicit filesystem path (power-user "custom path").
fn resolve_model_path(value: &str) -> String {
    if value.contains('/') || value.contains('\\') {
        return value.to_string();
    }
    if let Ok(dir) = crate::app_paths::app_data_dir() {
        let p = models::model_path(&dir.join("voice").join("models"), value);
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    value.to_string()
}

fn reduce_to_iso639_1(tag: &str) -> Option<String> {
    let primary = tag.trim().split(['-', '_']).next().unwrap_or("");
    if primary.is_empty() {
        None
    } else {
        Some(primary.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_reduction() {
        assert_eq!(reduce_to_iso639_1("de-DE").as_deref(), Some("de"));
        assert_eq!(reduce_to_iso639_1("zh_CN").as_deref(), Some("zh"));
        assert_eq!(reduce_to_iso639_1("  "), None);
    }

    #[test]
    fn explicit_path_passthrough() {
        assert_eq!(resolve_model_path("/models/base.bin"), "/models/base.bin");
    }
}
