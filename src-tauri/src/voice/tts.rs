//! Text-to-speech against the OpenAI speech endpoint. Returns raw MP3 bytes
//! so the caller can deliver them through whatever transport it has
//! (frontend Blob, file write, etc.).

use serde_json::json;
use std::time::Duration;

use super::settings::VoiceProviderKind;

pub async fn synthesize(
    provider: VoiceProviderKind,
    model: &str,
    voice: &str,
    text: &str,
    api_key: &str,
) -> Result<Vec<u8>, String> {
    if !matches!(provider, VoiceProviderKind::Openai) {
        return Err(format!(
            "TTS provider {} ist (noch) nicht unterstützt.",
            provider.as_str()
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let body = json!({
        "model": model,
        "voice": voice,
        "input": text,
        "format": "mp3",
    });
    let res = client
        .post("https://api.openai.com/v1/audio/speech")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("tts request: {e}"))?;
    let status = res.status();
    if !status.is_success() {
        let body = res
            .text()
            .await
            .unwrap_or_else(|e| format!("<body unavailable: {e}>"));
        return Err(format!("tts {status}: {body}"));
    }
    let bytes = res
        .bytes()
        .await
        .map_err(|e| format!("tts body: {e}"))?;
    Ok(bytes.to_vec())
}
