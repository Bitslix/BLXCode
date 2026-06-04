use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

const SETTINGS_FILE: &str = "heartbeat_settings.json";
const SERVICE_MEMORY_INDEXER: &str = "memory_indexer";
const DEFAULT_INTERVAL_MINUTES: u32 = 60;
const MIN_INTERVAL_MINUTES: u32 = 10;
const MAX_INTERVAL_MINUTES: u32 = 24 * 60;
const HEARTBEAT_EVENT: &str = "heartbeat_services_changed";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_interval_minutes")]
    pub interval_minutes: u32,
    #[serde(default)]
    pub service_enabled: BTreeMap<String, bool>,
}

impl Default for HeartbeatSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: DEFAULT_INTERVAL_MINUTES,
            service_enabled: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum HeartbeatServiceStatus {
    #[default]
    Idle,
    Running,
    Stalled,
    Error,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatServiceView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub capabilities: Vec<String>,
    pub enabled: bool,
    pub status: HeartbeatServiceStatus,
    pub last_call: Option<u64>,
    pub next_call: Option<u64>,
    pub last_response: Option<String>,
    pub skip_count: u32,
}

#[derive(Clone, Default)]
pub struct HeartbeatState {
    inner: Arc<Mutex<HeartbeatInner>>,
}

#[derive(Default)]
struct HeartbeatInner {
    scheduler_started: bool,
    open_workspaces: BTreeSet<String>,
    services: BTreeMap<String, ServiceRuntime>,
}

#[derive(Debug, Clone, Default)]
struct ServiceRuntime {
    status: HeartbeatServiceStatus,
    last_call: Option<u64>,
    next_call: Option<u64>,
    last_response: Option<String>,
    workspace_runs: BTreeMap<String, WorkspaceRun>,
}

#[derive(Debug, Clone)]
struct WorkspaceRun {
    status: HeartbeatServiceStatus,
    run_id: u64,
    skip_count: u32,
}

impl Default for WorkspaceRun {
    fn default() -> Self {
        Self {
            status: HeartbeatServiceStatus::Idle,
            run_id: 0,
            skip_count: 0,
        }
    }
}

pub fn ensure_scheduler_started(app: AppHandle) {
    let state = app.state::<HeartbeatState>().inner.clone();
    {
        let Ok(mut inner) = state.lock() else {
            return;
        };
        if inner.scheduler_started {
            return;
        }
        inner.scheduler_started = true;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            scheduler_tick(app.clone()).await;
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}

#[tauri::command]
pub fn heartbeat_settings_get() -> Result<HeartbeatSettings, String> {
    load_settings()
}

#[tauri::command]
pub fn heartbeat_settings_save(
    app: AppHandle,
    settings: HeartbeatSettings,
) -> Result<HeartbeatSettings, String> {
    let mut settings = normalize_settings(settings);
    ensure_known_services(&mut settings);
    save_settings(&settings)?;
    refresh_next_call(&app, &settings)?;
    emit_services_changed(&app);
    Ok(settings)
}

#[tauri::command]
pub fn heartbeat_services_list(
    state: State<'_, HeartbeatState>,
) -> Result<Vec<HeartbeatServiceView>, String> {
    let settings = load_settings()?;
    Ok(service_views(&state, &settings))
}

#[tauri::command]
pub fn heartbeat_service_set_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<Vec<HeartbeatServiceView>, String> {
    if id != SERVICE_MEMORY_INDEXER {
        return Err(format!("unknown heartbeat service: {id}"));
    }
    let mut settings = load_settings()?;
    settings
        .service_enabled
        .insert(SERVICE_MEMORY_INDEXER.into(), enabled);
    save_settings(&settings)?;
    refresh_next_call(&app, &settings)?;
    let state = app.state::<HeartbeatState>();
    emit_services_changed(&app);
    Ok(service_views(&state, &settings))
}

#[tauri::command]
pub fn heartbeat_set_open_workspaces(
    state: State<'_, HeartbeatState>,
    workspaces: Vec<String>,
) -> Result<(), String> {
    let mut clean = BTreeSet::new();
    for workspace in workspaces {
        let trimmed = workspace.trim();
        if !trimmed.is_empty() {
            clean.insert(trimmed.to_string());
        }
    }
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "heartbeat state lock poisoned".to_string())?;
    inner.open_workspaces = clean;
    Ok(())
}

#[tauri::command]
pub fn heartbeat_service_run_now(
    app: AppHandle,
    id: String,
) -> Result<Vec<HeartbeatServiceView>, String> {
    if id != SERVICE_MEMORY_INDEXER {
        return Err(format!("unknown heartbeat service: {id}"));
    }
    let settings = load_settings()?;
    start_memory_indexer(app.clone(), true)?;
    let state = app.state::<HeartbeatState>();
    emit_services_changed(&app);
    Ok(service_views(&state, &settings))
}

async fn scheduler_tick(app: AppHandle) {
    let Ok(mut settings) = load_settings() else {
        return;
    };
    ensure_known_services(&mut settings);
    if !settings.enabled || !service_enabled(&settings, SERVICE_MEMORY_INDEXER) {
        return;
    }
    let now = now_secs();
    let due = {
        let state = app.state::<HeartbeatState>();
        let Ok(mut inner) = state.inner.lock() else {
            return;
        };
        let runtime = inner
            .services
            .entry(SERVICE_MEMORY_INDEXER.into())
            .or_default();
        match runtime.next_call {
            Some(next) if next > now => false,
            _ => {
                runtime.next_call = Some(now + (settings.interval_minutes as u64 * 60));
                true
            }
        }
    };
    if due {
        let _ = start_memory_indexer(app, false);
    }
}

fn start_memory_indexer(app: AppHandle, manual: bool) -> Result<(), String> {
    let workspaces = {
        let state = app.state::<HeartbeatState>();
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "heartbeat state lock poisoned".to_string())?;
        let workspace_list = inner.open_workspaces.iter().cloned().collect::<Vec<_>>();
        let now = now_secs();
        let runtime = inner
            .services
            .entry(SERVICE_MEMORY_INDEXER.into())
            .or_default();
        runtime.last_call = Some(now);
        if manual {
            runtime.next_call = Some(now);
        }
        if workspace_list.is_empty() {
            runtime.status = HeartbeatServiceStatus::Idle;
            runtime.last_response = Some("No open workspaces to index.".into());
        } else {
            runtime.status = HeartbeatServiceStatus::Running;
            runtime.last_response =
                Some(format!("Indexing {} workspace(s).", workspace_list.len()));
        }
        workspace_list
    };

    for workspace in workspaces {
        if !mark_workspace_run_started(&app, &workspace)? {
            continue;
        }
        let app_for_job = app.clone();
        let app_for_finish = app.clone();
        tauri::async_runtime::spawn(async move {
            let workspace_for_job = workspace.clone();
            let result = crate::proc::run_blocking(move || {
                crate::memory::indexer::run_memory_indexer_for_workspace(
                    app_for_job,
                    workspace_for_job,
                )
            })
            .await;
            finish_workspace_run(&app_for_finish, &workspace, result);
        });
    }
    emit_services_changed(&app);
    Ok(())
}

fn mark_workspace_run_started(app: &AppHandle, workspace: &str) -> Result<bool, String> {
    let state = app.state::<HeartbeatState>();
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "heartbeat state lock poisoned".to_string())?;
    let runtime = inner
        .services
        .entry(SERVICE_MEMORY_INDEXER.into())
        .or_default();
    let run = runtime
        .workspace_runs
        .entry(workspace.to_string())
        .or_default();
    if matches!(
        run.status,
        HeartbeatServiceStatus::Running | HeartbeatServiceStatus::Stalled
    ) {
        run.skip_count = run.skip_count.saturating_add(1);
        if run.skip_count >= 3 {
            run.status = HeartbeatServiceStatus::Stalled;
            runtime.status = HeartbeatServiceStatus::Stalled;
            runtime.last_response = Some(format!("Memory Indexer stalled for `{workspace}`."));
        } else {
            runtime.last_response = Some(format!(
                "Skipped `{workspace}` because it is already indexing."
            ));
        }
        return Ok(false);
    }
    run.status = HeartbeatServiceStatus::Running;
    run.run_id = run.run_id.wrapping_add(1);
    run.skip_count = 0;
    Ok(true)
}

fn finish_workspace_run(
    app: &AppHandle,
    workspace: &str,
    result: Result<crate::memory::indexer::MemoryIndexRunReport, String>,
) {
    let state = app.state::<HeartbeatState>();
    if let Ok(mut inner) = state.inner.lock() {
        let runtime = inner
            .services
            .entry(SERVICE_MEMORY_INDEXER.into())
            .or_default();
        let run = runtime
            .workspace_runs
            .entry(workspace.to_string())
            .or_default();
        match result {
            Ok(report) => {
                run.status = HeartbeatServiceStatus::Idle;
                run.skip_count = 0;
                let warning_suffix = if report.warnings.is_empty() {
                    String::new()
                } else {
                    format!(", {} warning(s)", report.warnings.len())
                };
                runtime.last_response = Some(format!(
                    "Indexed `{}`: {} files changed, {} generated{}.",
                    report.workspace_cwd,
                    report.files_changed,
                    report.generated_paths.len(),
                    warning_suffix
                ));
            }
            Err(err) => {
                run.status = HeartbeatServiceStatus::Error;
                runtime.status = HeartbeatServiceStatus::Error;
                runtime.last_response = Some(err);
            }
        }
        if runtime
            .workspace_runs
            .values()
            .any(|run| run.status == HeartbeatServiceStatus::Running)
        {
            runtime.status = HeartbeatServiceStatus::Running;
        } else if runtime
            .workspace_runs
            .values()
            .any(|run| run.status == HeartbeatServiceStatus::Stalled)
        {
            runtime.status = HeartbeatServiceStatus::Stalled;
        } else if runtime.status != HeartbeatServiceStatus::Error {
            runtime.status = HeartbeatServiceStatus::Idle;
        }
    }
    emit_services_changed(app);
}

fn refresh_next_call(app: &AppHandle, settings: &HeartbeatSettings) -> Result<(), String> {
    let state = app.state::<HeartbeatState>();
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "heartbeat state lock poisoned".to_string())?;
    let runtime = inner
        .services
        .entry(SERVICE_MEMORY_INDEXER.into())
        .or_default();
    runtime.next_call = if settings.enabled && service_enabled(settings, SERVICE_MEMORY_INDEXER) {
        Some(now_secs() + settings.interval_minutes as u64 * 60)
    } else {
        None
    };
    Ok(())
}

fn service_views(
    state: &HeartbeatState,
    settings: &HeartbeatSettings,
) -> Vec<HeartbeatServiceView> {
    let runtime = state
        .inner
        .lock()
        .ok()
        .and_then(|inner| inner.services.get(SERVICE_MEMORY_INDEXER).cloned())
        .unwrap_or_default();
    let enabled = settings.enabled && service_enabled(settings, SERVICE_MEMORY_INDEXER);
    let status = if enabled {
        runtime.status
    } else {
        HeartbeatServiceStatus::Disabled
    };
    vec![HeartbeatServiceView {
        id: SERVICE_MEMORY_INDEXER.into(),
        name: "Memory Indexer".into(),
        description: "Builds managed Memory graph notes for Rules, Skills, and Plans.".into(),
        kind: "internal".into(),
        source: "blxcode".into(),
        capabilities: vec!["memory-write".into(), "workspace-read".into()],
        enabled,
        status,
        last_call: runtime.last_call,
        next_call: runtime.next_call,
        last_response: runtime.last_response,
        skip_count: runtime
            .workspace_runs
            .values()
            .map(|run| run.skip_count)
            .max()
            .unwrap_or(0),
    }]
}

fn ensure_known_services(settings: &mut HeartbeatSettings) {
    settings
        .service_enabled
        .entry(SERVICE_MEMORY_INDEXER.into())
        .or_insert(false);
    settings.interval_minutes = clamp_interval(settings.interval_minutes);
}

fn normalize_settings(mut settings: HeartbeatSettings) -> HeartbeatSettings {
    settings.interval_minutes = clamp_interval(settings.interval_minutes);
    ensure_known_services(&mut settings);
    settings
}

fn service_enabled(settings: &HeartbeatSettings, id: &str) -> bool {
    settings.service_enabled.get(id).copied().unwrap_or(false)
}

fn load_settings() -> Result<HeartbeatSettings, String> {
    let path = settings_path()?;
    let mut settings = match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => HeartbeatSettings::default(),
        Ok(raw) => serde_json::from_str::<HeartbeatSettings>(&raw)
            .map_err(|e| format!("parse heartbeat settings {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HeartbeatSettings::default(),
        Err(e) => return Err(format!("read heartbeat settings {}: {e}", path.display())),
    };
    ensure_known_services(&mut settings);
    Ok(settings)
}

fn save_settings(settings: &HeartbeatSettings) -> Result<(), String> {
    let path = settings_path()?;
    atomic_write_json(&path, settings)
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::app_paths::app_data_dir()?.join(SETTINGS_FILE))
}

fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| format!("encode json: {e}"))?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

fn clamp_interval(value: u32) -> u32 {
    value.clamp(MIN_INTERVAL_MINUTES, MAX_INTERVAL_MINUTES)
}

fn default_interval_minutes() -> u32 {
    DEFAULT_INTERVAL_MINUTES
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn emit_services_changed(app: &AppHandle) {
    if let Ok(settings) = load_settings() {
        let state = app.state::<HeartbeatState>();
        let views = service_views(&state, &settings);
        let _ = app.emit(HEARTBEAT_EVENT, views);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_is_clamped() {
        assert_eq!(clamp_interval(1), MIN_INTERVAL_MINUTES);
        assert_eq!(clamp_interval(60), 60);
        assert_eq!(clamp_interval(10_000), MAX_INTERVAL_MINUTES);
    }

    #[test]
    fn known_services_default_disabled() {
        let mut settings = HeartbeatSettings::default();
        ensure_known_services(&mut settings);
        assert_eq!(
            settings.service_enabled.get(SERVICE_MEMORY_INDEXER),
            Some(&false)
        );
    }
}
