//! Global app logfile, persisted under the app-data directory by default.
//!
//! The logger is intentionally metadata-only: callers should pass event names
//! and small diagnostic fields, never prompts, terminal output, API keys, or
//! file contents.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, State};

const SETTINGS_FILE: &str = "app_logging_settings.json";
const DEFAULT_LOG_FILE: &str = "blxcode.log";
const MAX_STRING_CHARS: usize = 360;
const MAX_OBJECT_FIELDS: usize = 32;
const MAX_ARRAY_ITEMS: usize = 24;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogSettings {
    #[serde(default)]
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogSettingsView {
    pub log_path: Option<String>,
    pub default_log_path: String,
    pub effective_log_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppLogLine {
    timestamp_ms: u128,
    level: String,
    source: String,
    event: String,
    metadata: Value,
}

struct AppLogInner {
    path: PathBuf,
    file: Option<File>,
}

pub struct AppLogState {
    inner: Mutex<Option<AppLogInner>>,
}

impl Default for AppLogState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

impl AppLogState {
    pub fn initialize(&self) -> Result<AppLogSettingsView, String> {
        let settings = load_settings()?;
        let view = settings_view(&settings)?;
        self.open_path(PathBuf::from(&view.effective_log_path), true)?;
        self.write_event(
            "info",
            "backend",
            "app_start",
            json!({ "logPath": view.effective_log_path }),
        )?;
        Ok(view)
    }

    fn open_path(&self, path: PathBuf, truncate: bool) -> Result<(), String> {
        let file = open_log_file(&path, truncate)?;
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("app log state lock poisoned: {e}"))?;
        *guard = Some(AppLogInner {
            path,
            file: Some(file),
        });
        Ok(())
    }

    pub fn write_event(
        &self,
        level: impl AsRef<str>,
        source: impl AsRef<str>,
        event: impl AsRef<str>,
        metadata: Value,
    ) -> Result<(), String> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|e| format!("app log state lock poisoned: {e}"))?;
        let Some(inner) = guard.as_mut() else {
            return Err("app log not initialized".into());
        };
        if inner.file.is_none() {
            inner.file = Some(open_log_file(&inner.path, false)?);
        }
        let line = AppLogLine {
            timestamp_ms: now_ms(),
            level: sanitize_token(level.as_ref(), "info"),
            source: sanitize_token(source.as_ref(), "app"),
            event: sanitize_token(event.as_ref(), "event"),
            metadata: sanitize_metadata(metadata),
        };
        let raw = serde_json::to_string(&line).map_err(|e| format!("encode log line: {e}"))?;
        let file = inner.file.as_mut().expect("log file was reopened");
        writeln!(file, "{raw}")
            .and_then(|_| file.flush())
            .map_err(|e| format!("write log file {}: {e}", inner.path.display()))
    }

    fn clear(&self) -> Result<(), String> {
        let path = {
            let guard = self
                .inner
                .lock()
                .map_err(|e| format!("app log state lock poisoned: {e}"))?;
            guard
                .as_ref()
                .map(|inner| inner.path.clone())
                .ok_or_else(|| "app log not initialized".to_string())?
        };
        self.open_path(path.clone(), true)?;
        self.write_event(
            "info",
            "backend",
            "log_cleared",
            json!({ "logPath": path.display().to_string() }),
        )
    }

    fn delete(&self) -> Result<(), String> {
        let path = {
            let mut guard = self
                .inner
                .lock()
                .map_err(|e| format!("app log state lock poisoned: {e}"))?;
            let Some(inner) = guard.as_mut() else {
                return Err("app log not initialized".into());
            };
            let path = inner.path.clone();
            inner.file = None;
            path
        };
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("delete log file {}: {e}", path.display())),
        }
        Ok(())
    }
}

fn open_log_file(path: &PathBuf, truncate: bool) -> Result<File, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid log path {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create log dir {}: {e}", parent.display()))?;
    OpenOptions::new()
        .create(true)
        .write(true)
        .append(!truncate)
        .truncate(truncate)
        .open(path)
        .map_err(|e| format!("open log file {}: {e}", path.display()))
}

#[tauri::command]
pub fn app_log_settings_get() -> Result<AppLogSettingsView, String> {
    let settings = load_settings()?;
    settings_view(&settings)
}

#[tauri::command]
pub fn app_log_settings_save(
    app: AppHandle,
    state: State<'_, AppLogState>,
    log_path: Option<String>,
) -> Result<AppLogSettingsView, String> {
    let settings = AppLogSettings {
        log_path: normalize_log_path(log_path),
    };
    save_settings(&settings)?;
    let view = settings_view(&settings)?;
    state.open_path(PathBuf::from(&view.effective_log_path), true)?;
    state.write_event(
        "info",
        "settings",
        "log_path_changed",
        json!({
            "default": view.log_path.is_none(),
            "logPath": view.effective_log_path,
        }),
    )?;
    let _ = app.emit("app_log_settings_changed", &view);
    Ok(view)
}

#[tauri::command]
pub fn app_log_event(
    state: State<'_, AppLogState>,
    level: String,
    source: String,
    event: String,
    metadata: Value,
) -> Result<(), String> {
    state.write_event(level, source, event, metadata)
}

#[tauri::command]
pub fn app_log_clear(state: State<'_, AppLogState>) -> Result<(), String> {
    state.clear()
}

#[tauri::command]
pub fn app_log_delete(state: State<'_, AppLogState>) -> Result<(), String> {
    state.delete()
}

pub fn log_frontend_console(
    state: State<'_, AppLogState>,
    level: String,
    message: String,
) -> Result<(), String> {
    state.write_event(level, "frontend", "console", json!({ "message": message }))
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::app_paths::app_data_dir()?.join(SETTINGS_FILE))
}

fn default_log_path() -> Result<PathBuf, String> {
    Ok(crate::app_paths::app_data_dir()?.join(DEFAULT_LOG_FILE))
}

fn load_settings() -> Result<AppLogSettings, String> {
    let path = settings_path()?;
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppLogSettings::default());
        }
        Err(e) => return Err(format!("read app log settings {}: {e}", path.display())),
    };
    if raw.trim().is_empty() {
        return Ok(AppLogSettings::default());
    }
    serde_json::from_str(&raw)
        .map_err(|e| format!("parse app log settings {}: {e}", path.display()))
}

fn save_settings(settings: &AppLogSettings) -> Result<(), String> {
    let path = settings_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid app log settings path {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("create app log settings dir {}: {e}", parent.display()))?;
    let json =
        serde_json::to_vec_pretty(settings).map_err(|e| format!("encode app log settings: {e}"))?;
    let tmp = parent.join(format!(".{SETTINGS_FILE}.{}.tmp", std::process::id()));
    {
        let mut file =
            File::create(&tmp).map_err(|e| format!("create tmp {}: {e}", tmp.display()))?;
        file.write_all(&json)
            .map_err(|e| format!("write tmp {}: {e}", tmp.display()))?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, &path).map_err(|e| format!("rename tmp -> app log settings: {e}"))
}

fn settings_view(settings: &AppLogSettings) -> Result<AppLogSettingsView, String> {
    let default_path = default_log_path()?;
    let effective = settings
        .log_path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_path.clone());
    Ok(AppLogSettingsView {
        log_path: settings.log_path.clone(),
        default_log_path: default_path.display().to_string(),
        effective_log_path: effective.display().to_string(),
    })
}

fn normalize_log_path(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn sanitize_token(raw: &str, fallback: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ':' | '.'))
        .take(80)
        .collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

fn sanitize_metadata(value: Value) -> Value {
    match value {
        Value::String(s) => Value::String(redact_string(&s)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .take(MAX_ARRAY_ITEMS)
                .map(sanitize_metadata)
                .collect(),
        ),
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, value) in map.into_iter().take(MAX_OBJECT_FIELDS) {
                let safe_key = sanitize_token(&key, "field");
                let safe_value = if looks_sensitive_key(&key) {
                    Value::String("[redacted]".into())
                } else {
                    sanitize_metadata(value)
                };
                out.insert(safe_key, safe_value);
            }
            Value::Object(out)
        }
        other => other,
    }
}

fn looks_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower.contains("key")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("prompt")
        || lower.contains("content")
        || lower.contains("terminaloutput")
}

fn redact_string(raw: &str) -> String {
    let mut s = raw.replace('\n', "\\n").replace('\r', "\\r");
    if s.chars().count() > MAX_STRING_CHARS {
        s = s.chars().take(MAX_STRING_CHARS).collect::<String>();
        s.push_str("...[truncated]");
    }
    redact_known_secret_shapes(&s)
}

fn redact_known_secret_shapes(raw: &str) -> String {
    raw.split_whitespace()
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.starts_with("sk-")
                || lower.starts_with("sk_")
                || lower.starts_with("ghp_")
                || lower.starts_with("github_pat_")
            {
                "[redacted]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_paths::test_support::AppDataDirGuard;

    fn temp_app_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("blxcode-app-logging-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn default_path_uses_app_data_logfile() {
        let dir = temp_app_dir("default-path");
        let _guard = AppDataDirGuard::new(dir.clone());
        let view = settings_view(&AppLogSettings::default()).unwrap();
        assert_eq!(
            view.default_log_path,
            dir.join(DEFAULT_LOG_FILE).display().to_string()
        );
        assert_eq!(view.effective_log_path, view.default_log_path);
        assert!(view.log_path.is_none());
    }

    #[test]
    fn custom_path_is_persisted_and_loaded() {
        let dir = temp_app_dir("custom-path");
        let custom = dir.join("custom").join("events.log");
        let _guard = AppDataDirGuard::new(dir);
        save_settings(&AppLogSettings {
            log_path: Some(custom.display().to_string()),
        })
        .unwrap();
        let loaded = load_settings().unwrap();
        assert_eq!(loaded.log_path, Some(custom.display().to_string()));
    }

    #[test]
    fn clear_truncates_existing_file_and_writes_marker() {
        let dir = temp_app_dir("clear");
        let _guard = AppDataDirGuard::new(dir.clone());
        let state = AppLogState::default();
        state.open_path(dir.join(DEFAULT_LOG_FILE), true).unwrap();
        state
            .write_event("info", "test", "before_clear", json!({ "ok": true }))
            .unwrap();
        state.clear().unwrap();
        let raw = fs::read_to_string(dir.join(DEFAULT_LOG_FILE)).unwrap();
        assert!(raw.contains("log_cleared"));
        assert!(!raw.contains("before_clear"));
    }

    #[test]
    fn startup_open_truncates_previous_contents() {
        let dir = temp_app_dir("truncate");
        let path = dir.join(DEFAULT_LOG_FILE);
        fs::write(&path, "old contents").unwrap();
        let state = AppLogState::default();
        state.open_path(path.clone(), true).unwrap();
        state
            .write_event("info", "test", "after_start", json!({}))
            .unwrap();
        let raw = fs::read_to_string(path).unwrap();
        assert!(raw.contains("after_start"));
        assert!(!raw.contains("old contents"));
    }

    #[test]
    fn delete_removes_file_until_next_write_recreates_it() {
        let dir = temp_app_dir("delete");
        let path = dir.join(DEFAULT_LOG_FILE);
        let state = AppLogState::default();
        state.open_path(path.clone(), true).unwrap();
        state
            .write_event("info", "test", "before_delete", json!({}))
            .unwrap();
        assert!(path.exists());
        state.delete().unwrap();
        assert!(!path.exists());
        state
            .write_event("info", "test", "after_delete", json!({}))
            .unwrap();
        let raw = fs::read_to_string(path).unwrap();
        assert!(raw.contains("after_delete"));
        assert!(!raw.contains("before_delete"));
    }

    #[test]
    fn metadata_redacts_sensitive_keys_and_truncates_strings() {
        let sanitized = sanitize_metadata(json!({
            "apiKey": "sk-secret",
            "message": "x".repeat(MAX_STRING_CHARS + 10),
            "safe": "hello",
        }));
        assert_eq!(sanitized["apiKey"], "[redacted]");
        assert!(sanitized["message"]
            .as_str()
            .unwrap()
            .contains("[truncated]"));
        assert_eq!(sanitized["safe"], "hello");
    }
}
