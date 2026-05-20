//! Tauri command surface for the image subsystem.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::settings::{self, CuratedImageModel, ImageProviderKind, ImageSettings};

/// Size cap when reading a saved image back for preview. Generated PNGs are
/// usually well under 20 MiB, but we want a hard ceiling so a malformed
/// `saved_path` cannot make the UI hang.
const PREVIEW_MAX_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageProviderRef {
    pub provider: ImageProviderKind,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageModelsResponse {
    pub provider: ImageProviderKind,
    pub entries: Vec<CuratedImageModel>,
}

#[tauri::command]
pub fn image_settings_get(app: AppHandle) -> Result<ImageSettings, String> {
    settings::load(&app)
}

#[tauri::command]
pub fn image_settings_save(app: AppHandle, patch: ImageSettings) -> Result<ImageSettings, String> {
    settings::save(&app, &patch)
}

#[tauri::command]
pub fn image_curated_models(payload: ImageProviderRef) -> ImageModelsResponse {
    ImageModelsResponse {
        provider: payload.provider,
        entries: settings::curated_image_models(payload.provider),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImagePreviewArgs {
    pub path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImagePreviewResponse {
    pub mime: String,
    pub bytes_b64: String,
}

/// Re-read a previously saved generated image so the timeline can re-render
/// it after a workspace reload (WASM cannot read arbitrary files itself).
#[tauri::command]
pub fn generated_image_preview(
    payload: GeneratedImagePreviewArgs,
) -> Result<GeneratedImagePreviewResponse, String> {
    let path = std::path::PathBuf::from(payload.path);
    let meta = std::fs::metadata(&path).map_err(|e| format!("preview metadata: {e}"))?;
    if !meta.is_file() {
        return Err("preview path is not a file".into());
    }
    if meta.len() > PREVIEW_MAX_BYTES {
        return Err(format!(
            "generated image exceeds {} MiB cap",
            PREVIEW_MAX_BYTES / 1024 / 1024
        ));
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("read preview: {e}"))?;
    let mime = sniff_mime(&bytes, &path).unwrap_or("application/octet-stream");
    Ok(GeneratedImagePreviewResponse {
        mime: mime.to_owned(),
        bytes_b64: BASE64.encode(&bytes),
    })
}

fn sniff_mime<'a>(bytes: &[u8], path: &std::path::Path) -> Option<&'a str> {
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
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        _ => None,
    }
}
