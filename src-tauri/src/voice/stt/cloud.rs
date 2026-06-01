//! Cloud speech-to-text against OpenAI / OpenRouter audio transcription
//! endpoints. Carries the original file-based `transcribe_wav` (reused by the
//! existing voice-orb flow) plus an in-memory `transcribe_pcm` for push-to-talk
//! that never touches disk.
//!
//! Wire format:
//! ```text
//! POST {base}/audio/transcriptions
//! Authorization: Bearer <key>
//! multipart/form-data:
//!   model: <model>
//!   file:  <wav bytes>
//!   response_format: text
//!   language: <iso639-1>  (optional)
//! ```

use reqwest::multipart::{Form, Part};
use std::path::Path;
use std::time::Duration;

use crate::voice::settings::VoiceProviderKind;

fn base_url(provider: VoiceProviderKind) -> &'static str {
    match provider {
        VoiceProviderKind::Openai => "https://api.openai.com/v1",
        VoiceProviderKind::Openrouter => "https://openrouter.ai/api/v1",
        VoiceProviderKind::Aws => unreachable!("AWS is TTS-only; rejected before this point"),
    }
}

/// Transcribe a WAV file on disk (existing voice-orb path).
pub async fn transcribe_wav(
    provider: VoiceProviderKind,
    model: &str,
    api_key: &str,
    wav_path: &Path,
    language: Option<&str>,
) -> Result<String, String> {
    if provider == VoiceProviderKind::Aws {
        let _ = (model, api_key, wav_path, language);
        return Err(
            "AWS Polly ist ein TTS-Dienst und kann nicht zur Transkription verwendet werden."
                .into(),
        );
    }
    let bytes = tokio::fs::read(wav_path)
        .await
        .map_err(|e| format!("read {}: {e}", wav_path.display()))?;
    let file_name = wav_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio.wav".into());
    post_transcription(provider, model, api_key, bytes, file_name, language).await
}

/// Transcribe an in-memory mono f32 PCM buffer at `sample_rate` Hz. The buffer
/// is encoded to a WAV blob in memory (no temp file) and posted.
pub async fn transcribe_pcm(
    provider: VoiceProviderKind,
    model: &str,
    api_key: &str,
    pcm: &[f32],
    sample_rate: u32,
    language: Option<&str>,
) -> Result<String, String> {
    if provider == VoiceProviderKind::Aws {
        return Err(
            "AWS Polly ist ein TTS-Dienst und kann nicht zur Transkription verwendet werden."
                .into(),
        );
    }
    let wav = super::pcm_to_wav_bytes(pcm, sample_rate)?;
    post_transcription(provider, model, api_key, wav, "audio.wav".into(), language).await
}

async fn post_transcription(
    provider: VoiceProviderKind,
    model: &str,
    api_key: &str,
    bytes: Vec<u8>,
    file_name: String,
    language: Option<&str>,
) -> Result<String, String> {
    let part = Part::bytes(bytes)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(|e| format!("mime: {e}"))?;
    let mut form = Form::new()
        .text("model", model.to_owned())
        .text("response_format", "text")
        .part("file", part);
    if let Some(lang) = language {
        if !lang.is_empty() {
            form = form.text("language", lang.to_owned());
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let res = client
        .post(format!("{}/audio/transcriptions", base_url(provider)))
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("stt request: {e}"))?;

    let status = res.status();
    let body = res.text().await.map_err(|e| format!("stt body: {e}"))?;
    if !status.is_success() {
        return Err(format!("stt {status}: {body}"));
    }

    // response_format=text returns plain text; some providers still wrap in JSON.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(s) = v.get("text").and_then(|t| t.as_str()) {
            return Ok(s.trim().to_string());
        }
    }
    Ok(body.trim().to_string())
}
