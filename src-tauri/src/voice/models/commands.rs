//! Tauri commands for the whisper model manager.
//!
//! `whisper_model_download` returns immediately and runs the transfer on a
//! background task, emitting progress to the frontend via Tauri events:
//! - `whisper_download_progress` → `{ id, received, total, speedBps }` (~200 ms)
//! - `whisper_download_done`      → `{ id, path }`
//! - `whisper_download_error`     → `{ id, message }`

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use super::{WhisperDownloadState, WhisperModelView};

/// `{app_data_dir}/voice/models`, created on demand.
fn models_dir() -> Result<PathBuf, String> {
    let dir = crate::app_paths::app_data_dir()?
        .join("voice")
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    Ok(dir)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    id: String,
    received: u64,
    total: u64,
    speed_bps: f64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    id: String,
    path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEvent {
    id: String,
    message: String,
}

#[tauri::command]
pub async fn whisper_models_list() -> Result<Vec<WhisperModelView>, String> {
    // Directory scan + per-model `metadata` stats: keep off the main thread.
    tauri::async_runtime::spawn_blocking(|| Ok(super::list(&models_dir()?)))
        .await
        .map_err(|e| format!("whisper_models_list task join: {e}"))?
}

/// Kick off a (resumable) download in the background. Progress is emitted as
/// Tauri events; the call returns as soon as the task is spawned.
#[tauri::command]
pub fn whisper_model_download(
    app: AppHandle,
    state: State<'_, Arc<WhisperDownloadState>>,
    id: String,
) -> Result<(), String> {
    let dir = models_dir()?;
    if super::find(&id).is_none() {
        return Err(format!("Unbekanntes Modell: {id}"));
    }
    if state.is_active(&id) {
        return Err(format!("Download für {id} läuft bereits."));
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        let emit_id = id.clone();
        let app_progress = app.clone();
        let result = super::download(&state, &dir, &id, move |received, total, speed_bps| {
            let _ = app_progress.emit(
                "whisper_download_progress",
                ProgressEvent {
                    id: emit_id.clone(),
                    received,
                    total,
                    speed_bps,
                },
            );
        })
        .await;
        match result {
            Ok(path) => {
                let _ = app.emit(
                    "whisper_download_done",
                    DoneEvent {
                        id: id.clone(),
                        path: path.to_string_lossy().to_string(),
                    },
                );
            }
            Err(message) => {
                // A cancellation keeps the `.part` for resume; surface it like
                // any other terminal state so the UI can reset the button.
                let _ = app.emit(
                    "whisper_download_error",
                    ErrorEvent {
                        id: id.clone(),
                        message,
                    },
                );
            }
        }
    });
    Ok(())
}

/// Request cancellation; the `.part` is preserved so a later download resumes.
#[tauri::command]
pub fn whisper_model_cancel(
    state: State<'_, Arc<WhisperDownloadState>>,
    id: String,
) -> Result<bool, String> {
    Ok(state.cancel(&id))
}

#[tauri::command]
pub async fn whisper_model_delete(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || super::delete(&models_dir()?, &id))
        .await
        .map_err(|e| format!("whisper_model_delete task join: {e}"))?
}
