//! HTTP calls for image generation.
//!
//! Two provider paths share the same response shape (`GeneratedImage`) but
//! have completely separate request bodies:
//!
//! - **OpenAI** uses `/v1/images/generations` (text-only) and
//!   `/v1/images/edits` (with reference images, multipart).
//! - **OpenRouter** uses `/v1/chat/completions` with `modalities: ["image"]`;
//!   the response carries one or more data URLs in
//!   `choices[0].message.images[*].image_url.url`.
//!
//! All requests honour `AgentEngineState::cancelled()` between IO steps so
//! the user can abort while waiting on a slow provider.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::agent::protocol::AgentImageContextItem;
use crate::agent::state::AgentEngineState;
use crate::image::settings::{ImageProviderKind, ImageQualityLevel, ImageSettings};

/// Hard ceilings: align with the chat-side limits (see `commands.rs`).
const MAX_REF_IMAGES: usize = 4;
const MAX_REF_BYTES_PER_IMAGE: u64 = 8 * 1024 * 1024;
const HTTP_TIMEOUT_SECS: u64 = 180;

/// The decoded result of one generation request.
pub struct GeneratedImage {
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum GenerateError {
    Cancelled,
    Validation(String),
    Http(String),
    Provider(String),
    Parse(String),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("cancelled"),
            Self::Validation(m) => write!(f, "validation: {m}"),
            Self::Http(m) => write!(f, "http: {m}"),
            Self::Provider(m) => write!(f, "provider: {m}"),
            Self::Parse(m) => write!(f, "parse: {m}"),
        }
    }
}

impl GenerateError {
    pub fn into_message(self) -> String {
        self.to_string()
    }
}

/// Validate reference images against the same caps used by the chat path.
fn validate_refs(refs: &[AgentImageContextItem]) -> Result<(), GenerateError> {
    if refs.len() > MAX_REF_IMAGES {
        return Err(GenerateError::Validation(format!(
            "too many reference images ({} > {MAX_REF_IMAGES})",
            refs.len()
        )));
    }
    for item in refs {
        if item.size_bytes > MAX_REF_BYTES_PER_IMAGE {
            return Err(GenerateError::Validation(format!(
                "reference image '{}' exceeds {} MiB",
                item.label,
                MAX_REF_BYTES_PER_IMAGE / 1024 / 1024
            )));
        }
        match item.mime.as_str() {
            "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp" => {}
            other => {
                return Err(GenerateError::Validation(format!(
                    "unsupported reference MIME: {other}"
                )))
            }
        }
        if BASE64.decode(item.bytes_b64.as_bytes()).is_err() {
            return Err(GenerateError::Validation(format!(
                "reference image '{}' has invalid base64",
                item.label
            )));
        }
    }
    Ok(())
}

fn build_http_client() -> Result<reqwest::Client, GenerateError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()
        .map_err(|e| GenerateError::Http(format!("client: {e}")))
}

/// Dispatch one image-generation request. Returns the raw decoded image
/// bytes (caller decides whether to persist them).
pub async fn generate(
    state: &Arc<AgentEngineState>,
    settings: &ImageSettings,
    api_key: &str,
    prompt: &str,
    refs: &[AgentImageContextItem],
) -> Result<GeneratedImage, GenerateError> {
    if state.cancelled() {
        return Err(GenerateError::Cancelled);
    }
    if prompt.trim().is_empty() {
        return Err(GenerateError::Validation(
            "prompt must not be empty".into(),
        ));
    }
    validate_refs(refs)?;

    let client = build_http_client()?;

    let res = match settings.provider {
        ImageProviderKind::Openai => {
            generate_openai(
                &client,
                &settings.model_id,
                settings.quality,
                api_key,
                prompt,
                refs,
            )
            .await
        }
        ImageProviderKind::Openrouter => {
            generate_openrouter(
                &client,
                &settings.model_id,
                settings.quality,
                api_key,
                prompt,
                refs,
            )
            .await
        }
    };

    if state.cancelled() {
        return Err(GenerateError::Cancelled);
    }
    res
}

// ---------------------------------------------------------------------------
// OpenAI
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OpenaiImageEnvelope {
    #[serde(default)]
    data: Vec<OpenaiImageItem>,
    #[serde(default)]
    error: Option<OpenaiError>,
}

#[derive(Deserialize)]
struct OpenaiImageItem {
    #[serde(default)]
    b64_json: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct OpenaiError {
    #[serde(default)]
    message: Option<String>,
}

fn openai_image_request_body(model: &str, prompt: &str, quality: ImageQualityLevel) -> Value {
    let model_lc = model.to_ascii_lowercase();
    let mut body = json!({
        "model": model,
        "prompt": prompt,
        "n": 1,
        "size": quality.openai_size(),
    });
    let quality_param = if model_lc.contains("dall-e") || model_lc.contains("dalle") {
        quality.openai_dalle_quality()
    } else {
        quality.openai_gpt_quality()
    };
    if let Some(obj) = body.as_object_mut() {
        obj.insert("quality".into(), json!(quality_param));
    }
    body
}

async fn generate_openai(
    client: &reqwest::Client,
    model: &str,
    quality: ImageQualityLevel,
    api_key: &str,
    prompt: &str,
    refs: &[AgentImageContextItem],
) -> Result<GeneratedImage, GenerateError> {
    if refs.is_empty() {
        generate_openai_text(client, model, quality, api_key, prompt).await
    } else {
        generate_openai_edit(client, model, quality, api_key, prompt, refs).await
    }
}

async fn generate_openai_text(
    client: &reqwest::Client,
    model: &str,
    quality: ImageQualityLevel,
    api_key: &str,
    prompt: &str,
) -> Result<GeneratedImage, GenerateError> {
    let body = openai_image_request_body(model, prompt, quality);
    let res = client
        .post("https://api.openai.com/v1/images/generations")
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| GenerateError::Http(format!("openai generations: {e}")))?;
    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|e| GenerateError::Http(format!("openai body: {e}")))?;
    if !status.is_success() {
        return Err(provider_error_from_openai(&text, status.as_u16()));
    }
    parse_openai_envelope(&text)
}

async fn generate_openai_edit(
    client: &reqwest::Client,
    model: &str,
    quality: ImageQualityLevel,
    api_key: &str,
    prompt: &str,
    refs: &[AgentImageContextItem],
) -> Result<GeneratedImage, GenerateError> {
    let model_lc = model.to_ascii_lowercase();
    let quality_param = if model_lc.contains("dall-e") || model_lc.contains("dalle") {
        quality.openai_dalle_quality()
    } else {
        quality.openai_gpt_quality()
    };
    let mut form = reqwest::multipart::Form::new()
        .text("model", model.to_owned())
        .text("prompt", prompt.to_owned())
        .text("n", "1")
        .text("size", quality.openai_size().to_owned())
        .text("quality", quality_param.to_owned());
    for (idx, item) in refs.iter().enumerate() {
        let bytes = BASE64
            .decode(item.bytes_b64.as_bytes())
            .map_err(|e| GenerateError::Validation(format!("ref b64 {}: {e}", item.label)))?;
        let filename = if item.label.trim().is_empty() {
            format!("ref_{idx}.png")
        } else {
            item.label.clone()
        };
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename)
            .mime_str(&item.mime)
            .map_err(|e| GenerateError::Validation(format!("ref mime {}: {e}", item.label)))?;
        // OpenAI accepts the first ref as `image`; additional refs use
        // `image[]` per their docs. We send the first as `image` and the
        // rest as `image[]` for compatibility.
        let field = if idx == 0 { "image" } else { "image[]" };
        form = form.part(field, part);
    }
    let res = client
        .post("https://api.openai.com/v1/images/edits")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| GenerateError::Http(format!("openai edits: {e}")))?;
    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|e| GenerateError::Http(format!("openai body: {e}")))?;
    if !status.is_success() {
        return Err(provider_error_from_openai(&text, status.as_u16()));
    }
    parse_openai_envelope(&text)
}

fn parse_openai_envelope(body: &str) -> Result<GeneratedImage, GenerateError> {
    let env: OpenaiImageEnvelope = serde_json::from_str(body)
        .map_err(|e| GenerateError::Parse(format!("openai json: {e}")))?;
    let first = env
        .data
        .into_iter()
        .next()
        .ok_or_else(|| GenerateError::Provider("openai returned no images".into()))?;
    if let Some(b64) = first.b64_json {
        let bytes = BASE64
            .decode(b64.as_bytes())
            .map_err(|e| GenerateError::Parse(format!("openai b64: {e}")))?;
        return Ok(GeneratedImage {
            mime: detect_image_mime(&bytes).unwrap_or("image/png").to_owned(),
            bytes,
        });
    }
    if let Some(url) = first.url {
        // OpenAI may return a URL only — we fetch it once and inline.
        return fetch_url_bytes(&url).map_err(|e| GenerateError::Http(format!("openai url: {e}")));
    }
    Err(GenerateError::Provider(
        "openai response had neither b64_json nor url".into(),
    ))
}

fn provider_error_from_openai(body: &str, status: u16) -> GenerateError {
    if let Ok(env) = serde_json::from_str::<OpenaiImageEnvelope>(body) {
        if let Some(err) = env.error.and_then(|e| e.message) {
            return GenerateError::Provider(format!("openai {status}: {err}"));
        }
    }
    GenerateError::Provider(format!("openai {status}: {}", truncate(body, 240)))
}

fn fetch_url_bytes(_url: &str) -> Result<GeneratedImage, String> {
    // Reachable when OpenAI returns a hosted URL. We do not chase URLs in
    // v1 — the model defaults are configured to return b64_json. If a
    // provider unexpectedly returns a URL, surface a clear error so the
    // user can switch models.
    Err("provider returned a hosted URL; configure a model that emits b64_json".into())
}

// ---------------------------------------------------------------------------
// OpenRouter
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct OpenrouterEnvelope {
    #[serde(default)]
    choices: Vec<OpenrouterChoice>,
    #[serde(default)]
    error: Option<OpenrouterError>,
}

#[derive(Deserialize)]
struct OpenrouterChoice {
    #[serde(default)]
    message: Option<OpenrouterMessage>,
}

#[derive(Deserialize)]
struct OpenrouterMessage {
    #[serde(default)]
    images: Vec<OpenrouterImage>,
    #[serde(default)]
    content: Option<Value>,
}

#[derive(Deserialize)]
struct OpenrouterImage {
    #[serde(default)]
    image_url: Option<OpenrouterImageUrl>,
}

#[derive(Deserialize)]
struct OpenrouterImageUrl {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
struct OpenrouterError {
    #[serde(default)]
    message: Option<String>,
}

async fn generate_openrouter(
    client: &reqwest::Client,
    model: &str,
    quality: ImageQualityLevel,
    api_key: &str,
    prompt: &str,
    refs: &[AgentImageContextItem],
) -> Result<GeneratedImage, GenerateError> {
    let content = openrouter_user_content(prompt, refs);
    let body = json!({
        "model": model,
        "modalities": ["image", "text"],
        "messages": [{ "role": "user", "content": content }],
        "stream": false,
        "image_config": {
            "quality": quality.openrouter_image_quality(),
        },
    });
    let res = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://bitslix.com/blxcode")
        .header("X-Title", "blxcode")
        .json(&body)
        .send()
        .await
        .map_err(|e| GenerateError::Http(format!("openrouter: {e}")))?;
    let status = res.status();
    let text = res
        .text()
        .await
        .map_err(|e| GenerateError::Http(format!("openrouter body: {e}")))?;
    if !status.is_success() {
        if let Ok(env) = serde_json::from_str::<OpenrouterEnvelope>(&text) {
            if let Some(err) = env.error.and_then(|e| e.message) {
                return Err(GenerateError::Provider(format!(
                    "openrouter {status}: {err}"
                )));
            }
        }
        return Err(GenerateError::Provider(format!(
            "openrouter {status}: {}",
            truncate(&text, 240)
        )));
    }
    parse_openrouter_envelope(&text)
}

fn openrouter_user_content(prompt: &str, refs: &[AgentImageContextItem]) -> Value {
    if refs.is_empty() {
        return Value::String(prompt.to_owned());
    }
    let mut blocks = Vec::with_capacity(refs.len() + 1);
    blocks.push(json!({ "type": "text", "text": prompt }));
    for image in refs {
        blocks.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", image.mime, image.bytes_b64),
            },
        }));
    }
    Value::Array(blocks)
}

fn parse_openrouter_envelope(body: &str) -> Result<GeneratedImage, GenerateError> {
    let env: OpenrouterEnvelope = serde_json::from_str(body)
        .map_err(|e| GenerateError::Parse(format!("openrouter json: {e}")))?;
    let message = env
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message)
        .ok_or_else(|| GenerateError::Provider("openrouter returned no choices".into()))?;
    let image_url = message
        .images
        .into_iter()
        .find_map(|img| img.image_url.and_then(|u| u.url))
        .ok_or_else(|| {
            GenerateError::Provider(
                openrouter_text_fallback(message.content.as_ref())
                    .unwrap_or_else(|| "openrouter response had no image".into()),
            )
        })?;
    let (mime, bytes) = decode_data_url(&image_url)?;
    Ok(GeneratedImage { mime, bytes })
}

fn openrouter_text_fallback(content: Option<&Value>) -> Option<String> {
    let value = content?;
    if let Some(s) = value.as_str() {
        return Some(format!("openrouter returned no image: {}", truncate(s, 240)));
    }
    if let Some(arr) = value.as_array() {
        for block in arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                    return Some(format!("openrouter returned no image: {}", truncate(t, 240)));
                }
            }
        }
    }
    None
}

fn decode_data_url(url: &str) -> Result<(String, Vec<u8>), GenerateError> {
    let rest = url
        .strip_prefix("data:")
        .ok_or_else(|| GenerateError::Parse("expected data: URL".into()))?;
    let comma = rest
        .find(',')
        .ok_or_else(|| GenerateError::Parse("data URL missing ','".into()))?;
    let (meta, payload) = rest.split_at(comma);
    let payload = &payload[1..];
    let mut mime = "image/png".to_owned();
    let mut is_b64 = false;
    for part in meta.split(';') {
        let part = part.trim();
        if part.eq_ignore_ascii_case("base64") {
            is_b64 = true;
        } else if part.contains('/') {
            mime = part.to_owned();
        }
    }
    if !is_b64 {
        return Err(GenerateError::Parse(
            "data URL not base64-encoded".into(),
        ));
    }
    let bytes = BASE64
        .decode(payload.as_bytes())
        .map_err(|e| GenerateError::Parse(format!("data URL b64: {e}")))?;
    Ok((mime, bytes))
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

/// Saved-image metadata returned to the caller after a successful write.
pub struct SavedImage {
    pub abs_path: PathBuf,
    pub filename: String,
}

/// Persist a generated image under `{workspace_root}/.blxcode/generated/`.
/// Returns the absolute path so the orchestrator can emit it in the
/// `ImageGenerated` event.
pub fn save_to_workspace(
    workspace_root: &Path,
    image: &GeneratedImage,
    prompt: &str,
) -> Result<SavedImage, String> {
    let dir = workspace_root.join(".blxcode").join("generated");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let ts = unix_timestamp_ms();
    let slug = slugify(prompt);
    let ext = extension_for_mime(&image.mime);
    let mut filename = format!("{ts}-{slug}.{ext}");
    let mut suffix = 1u32;
    while dir.join(&filename).exists() {
        suffix += 1;
        filename = format!("{ts}-{slug}-{suffix}.{ext}");
        if suffix > 999 {
            return Err("too many filename collisions".into());
        }
    }
    let abs_path = dir.join(&filename);
    std::fs::write(&abs_path, &image.bytes)
        .map_err(|e| format!("write {}: {e}", abs_path.display()))?;
    Ok(SavedImage { abs_path, filename })
}

/// Stable kebab-style slug. Keeps `[a-z0-9-]`, collapses other chars to
/// `-`, trims to ~40 chars so filenames stay reasonable.
pub fn slugify(text: &str) -> String {
    let lower = text.trim().to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut prev_dash = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_owned();
    let truncated: String = trimmed.chars().take(40).collect();
    let truncated = truncated.trim_matches('-').to_owned();
    if truncated.is_empty() {
        "image".to_owned()
    } else {
        truncated
    }
}

fn extension_for_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn unix_timestamp_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push_str("…");
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_punctuation_and_unicode() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("   "), "image");
        let long = slugify("A_very_long_prompt_that_should_be_truncated_at_some_point_for_safety");
        assert!(long.len() <= 40, "slug too long: {long}");
        assert!(long.starts_with("a-very-long-prompt"));
        assert!(!long.starts_with('-') && !long.ends_with('-'));
    }

    #[test]
    fn ext_maps_known_mime() {
        assert_eq!(extension_for_mime("image/png"), "png");
        assert_eq!(extension_for_mime("image/jpeg"), "jpg");
        assert_eq!(extension_for_mime("image/webp"), "webp");
        assert_eq!(extension_for_mime("application/octet-stream"), "bin");
    }

    #[test]
    fn decode_data_url_basic_png() {
        let png = b"\x89PNG\r\n\x1a\nrest";
        let b64 = BASE64.encode(png);
        let url = format!("data:image/png;base64,{b64}");
        let (mime, bytes) = decode_data_url(&url).expect("decode");
        assert_eq!(mime, "image/png");
        assert_eq!(bytes, png);
    }

    #[test]
    fn decode_data_url_rejects_non_b64() {
        let url = "data:image/png,raw";
        assert!(decode_data_url(url).is_err());
    }
}
