use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct BlxUpdaterState {
    inner: Mutex<BlxUpdaterInner>,
}

#[derive(Default)]
struct BlxUpdaterInner {
    pending_update: Option<Update>,
    progress: UpdateProgress,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResponse {
    pub status: String,
    pub current_version: String,
    pub available_version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
    pub target: Option<String>,
    pub download_url: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: String,
    pub busy: bool,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub error: Option<String>,
    pub updated_at_ms: u64,
}

impl Default for UpdateProgress {
    fn default() -> Self {
        Self {
            phase: "idle".into(),
            busy: false,
            downloaded_bytes: 0,
            total_bytes: None,
            error: None,
            updated_at_ms: now_ms(),
        }
    }
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn updater_check(
    app: AppHandle,
    state: tauri::State<'_, BlxUpdaterState>,
) -> Result<UpdateCheckResponse, String> {
    if cfg!(debug_assertions) {
        let current_version = app.package_info().version.to_string();
        let mut inner = state.inner.lock().map_err(lock_err)?;
        inner.pending_update = None;
        inner.progress = UpdateProgress {
            phase: "devUnavailable".into(),
            busy: false,
            error: None,
            ..UpdateProgress::default()
        };
        return Ok(UpdateCheckResponse {
            status: "devUnavailable".into(),
            current_version,
            available_version: None,
            notes: None,
            date: None,
            target: None,
            download_url: None,
            message: Some("Updater is disabled in development builds.".into()),
        });
    }

    let current_version = app.package_info().version.to_string();
    set_progress(&state, "checking", true, 0, None, None)?;

    let builder = app.updater_builder().on_before_exit(|| {
        eprintln!("BLXCode is exiting before installing the update");
    });
    #[cfg(target_os = "macos")]
    let builder = builder.target("darwin-universal");

    let update = builder
        .build()
        .map_err(|err| updater_error(&state, err))?
        .check()
        .await
        .map_err(|err| updater_error(&state, err))?;

    match update {
        Some(update) => {
            let response = UpdateCheckResponse {
                status: "available".into(),
                current_version,
                available_version: Some(update.version.clone()),
                notes: update.body.clone(),
                date: update.date.map(|date| date.to_string()),
                target: Some(format!("{}-{}", update.target, std::env::consts::ARCH)),
                download_url: Some(update.download_url.to_string()),
                message: None,
            };
            let mut inner = state.inner.lock().map_err(lock_err)?;
            inner.pending_update = Some(update);
            inner.progress = UpdateProgress {
                phase: "available".into(),
                busy: false,
                updated_at_ms: now_ms(),
                ..UpdateProgress::default()
            };
            Ok(response)
        }
        None => {
            let mut inner = state.inner.lock().map_err(lock_err)?;
            inner.pending_update = None;
            inner.progress = UpdateProgress {
                phase: "upToDate".into(),
                busy: false,
                updated_at_ms: now_ms(),
                ..UpdateProgress::default()
            };
            Ok(UpdateCheckResponse {
                status: "upToDate".into(),
                current_version,
                available_version: None,
                notes: None,
                date: None,
                target: None,
                download_url: None,
                message: None,
            })
        }
    }
}

#[tauri::command]
pub fn updater_install_start(
    app: AppHandle,
    state: tauri::State<'_, BlxUpdaterState>,
) -> Result<UpdateProgress, String> {
    let update = {
        let mut inner = state.inner.lock().map_err(lock_err)?;
        if inner.progress.busy {
            return Err("An update task is already running.".into());
        }
        let Some(update) = inner.pending_update.take() else {
            return Err("No pending update is available.".into());
        };
        inner.progress = UpdateProgress {
            phase: "downloading".into(),
            busy: true,
            updated_at_ms: now_ms(),
            ..UpdateProgress::default()
        };
        update
    };

    let app_for_task = app.clone();
    tauri::async_runtime::spawn(async move {
        let app_for_progress = app_for_task.clone();
        let result = update
            .download_and_install(
                move |chunk_len, total_bytes| {
                    let state = app_for_progress.state::<BlxUpdaterState>();
                    if let Ok(mut inner) = state.inner.lock() {
                        inner.progress.phase = "downloading".into();
                        inner.progress.busy = true;
                        inner.progress.downloaded_bytes += chunk_len as u64;
                        inner.progress.total_bytes = total_bytes;
                        inner.progress.error = None;
                        inner.progress.updated_at_ms = now_ms();
                    };
                },
                move || {
                    let state = app_for_task.state::<BlxUpdaterState>();
                    if let Ok(mut inner) = state.inner.lock() {
                        inner.progress.phase = "installing".into();
                        inner.progress.busy = true;
                        inner.progress.updated_at_ms = now_ms();
                    };
                },
            )
            .await;

        let state = app.state::<BlxUpdaterState>();
        if let Ok(mut inner) = state.inner.lock() {
            match result {
                Ok(()) => {
                    inner.progress.phase = "done".into();
                    inner.progress.busy = false;
                    inner.progress.error = None;
                }
                Err(err) => {
                    inner.progress.phase = "error".into();
                    inner.progress.busy = false;
                    inner.progress.error = Some(err.to_string());
                }
            }
            inner.progress.updated_at_ms = now_ms();
        };
    });

    updater_poll_progress(state)
}

#[tauri::command]
pub fn updater_poll_progress(
    state: tauri::State<'_, BlxUpdaterState>,
) -> Result<UpdateProgress, String> {
    let inner = state.inner.lock().map_err(lock_err)?;
    Ok(inner.progress.clone())
}

#[tauri::command]
pub fn app_relaunch(app: AppHandle) -> Result<(), String> {
    app.restart();
}

fn set_progress(
    state: &tauri::State<'_, BlxUpdaterState>,
    phase: &str,
    busy: bool,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    error: Option<String>,
) -> Result<(), String> {
    let mut inner = state.inner.lock().map_err(lock_err)?;
    inner.progress = UpdateProgress {
        phase: phase.into(),
        busy,
        downloaded_bytes,
        total_bytes,
        error,
        updated_at_ms: now_ms(),
    };
    Ok(())
}

fn updater_error<E: std::fmt::Display>(
    state: &tauri::State<'_, BlxUpdaterState>,
    err: E,
) -> String {
    let message = err.to_string();
    let _ = set_progress(state, "error", false, 0, None, Some(message.clone()));
    message
}

fn lock_err<E: std::fmt::Display>(err: E) -> String {
    format!("updater state lock poisoned: {err}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
