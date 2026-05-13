//! Install / inspect / uninstall blxcode's external-agent hooks
//! (Claude Code + Codex). Two kinds of hooks ship today:
//!
//! - **Title hooks** (`*_title.py`) — UserPromptSubmit; rewrite the
//!   terminal tab title from the current prompt.
//! - **Session-capture hooks** (`*_session_capture.py`) — SessionStart;
//!   record `terminal_key -> session_id` in `sessions.json` so blxcode
//!   can later issue `claude --resume <id>` / `codex resume <id>` for
//!   that exact slot.
//!
//! Scripts are bundled as Tauri resources (`tauri.conf.json` ->
//! `bundle.resources`). On install we copy them to
//! `app_config_dir/hooks/` and patch the agent's settings file
//! (`~/.claude/settings.json`, `~/.codex/hooks.json`) idempotently.
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const CLAUDE_TITLE_SCRIPT: &str = "claude_title.py";
const CODEX_TITLE_SCRIPT: &str = "codex_title.py";
const CLAUDE_CAPTURE_SCRIPT: &str = "claude_session_capture.py";
const CODEX_CAPTURE_SCRIPT: &str = "codex_session_capture.py";

const CLAUDE_TITLE_MARKER: &str = "blxcode:claude-title";
const CLAUDE_CAPTURE_MARKER: &str = "blxcode:claude-session-capture";
const CODEX_CAPTURE_MARKER: &str = "blxcode:codex-session-capture";

#[cfg(target_os = "windows")]
const PYTHON_BIN: &str = "py -3";
#[cfg(not(target_os = "windows"))]
const PYTHON_BIN: &str = "python3";

fn build_hook_command(script_path: &Path) -> String {
    format!("{PYTHON_BIN} \"{}\"", script_path.display())
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentHookEntry {
    pub agent: String,
    pub script_path: Option<String>,
    pub config_path: Option<String>,
    pub installed: bool,
    pub note: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentHooksReport {
    pub hooks_dir: String,
    pub entries: Vec<AgentHookEntry>,
}

fn home_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .home_dir()
        .map_err(|e| format!("home dir unavailable: {e}"))
}

fn hooks_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("app config dir unavailable: {e}"))?;
    Ok(base.join("hooks"))
}

fn resource_script(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let rel = PathBuf::from("hooks").join(name);
    app.path()
        .resolve(&rel, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("resource {name}: {e}"))
}

fn ensure_dir(p: &Path) -> Result<(), String> {
    fs::create_dir_all(p).map_err(|e| format!("mkdir {}: {e}", p.display()))
}

fn copy_script(app: &AppHandle, name: &str, dest_dir: &Path) -> Result<PathBuf, String> {
    let src = resource_script(app, name)?;
    let dest = dest_dir.join(name);
    fs::copy(&src, &dest)
        .map_err(|e| format!("copy {} -> {}: {e}", src.display(), dest.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
    }
    Ok(dest)
}

fn read_json_or_empty(path: &Path) -> Result<serde_json::Value, String> {
    match fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(serde_json::json!({})),
        Ok(s) => serde_json::from_str(&s).map_err(|e| format!("parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

fn write_json_pretty(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let body = serde_json::to_string_pretty(value)
        .map_err(|e| format!("serialize {}: {e}", path.display()))?;
    fs::write(path, body).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Returns true if `<settings>.hooks.<event>` already references our
/// hook script. We match strictly by path/script-filename so the
/// settings entry stays schema-clean — Claude/Codex reject unknown
/// fields at the entry level in some versions, which silently disables
/// the whole hook.
fn hook_already_installed(
    settings: &serde_json::Value,
    event: &str,
    script_path: &Path,
) -> bool {
    let Some(arr) = settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(|v| v.as_array())
    else {
        return false;
    };
    let needle = script_path.to_string_lossy();
    let filename = script_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    arr.iter().any(|group| {
        let Some(inner) = group.get("hooks").and_then(|v| v.as_array()) else {
            return false;
        };
        inner.iter().any(|h| {
            h.get("command")
                .and_then(|v| v.as_str())
                .map(|c| c.contains(needle.as_ref()) || (!filename.is_empty() && c.contains(&filename)))
                .unwrap_or(false)
        })
    })
}

/// Idempotent: add one hook entry under `.hooks.<event>` pointing at
/// `script_path`. The marker argument is no longer written into the
/// settings file — we keep it in the function signature only so the
/// uninstall path can still strip legacy `_blxcode_marker` entries from
/// older installs.
fn patch_settings(
    settings_path: &Path,
    event: &str,
    matcher: &str,
    _legacy_marker: &str,
    script_path: &Path,
) -> Result<bool, String> {
    let mut settings = read_json_or_empty(settings_path)?;
    if hook_already_installed(&settings, event, script_path) {
        return Ok(false);
    }
    let new_entry = serde_json::json!({
        "matcher": matcher,
        "hooks": [{
            "type": "command",
            "command": build_hook_command(script_path),
        }],
    });
    let root = settings
        .as_object_mut()
        .ok_or_else(|| format!("{} is not a JSON object", settings_path.display()))?;
    let hooks = root
        .entry("hooks".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| format!("{}: .hooks is not an object", settings_path.display()))?;
    let arr = hooks_obj
        .entry(event.to_string())
        .or_insert_with(|| serde_json::json!([]));
    let arr_vec = arr.as_array_mut().ok_or_else(|| {
        format!(
            "{}: .hooks.{} is not an array",
            settings_path.display(),
            event
        )
    })?;
    arr_vec.push(new_entry);
    write_json_pretty(settings_path, &settings)?;
    Ok(true)
}

/// Remove every entry under `.hooks.<event>` carrying our marker (or a
/// command string mentioning the script filename / marker). Returns
/// `true` if anything was removed.
fn unpatch_settings(
    settings_path: &Path,
    event: &str,
    marker: &str,
    script_name: &str,
) -> Result<bool, String> {
    if !settings_path.exists() {
        return Ok(false);
    }
    let mut settings = read_json_or_empty(settings_path)?;
    let removed = {
        let Some(arr) = settings
            .get_mut("hooks")
            .and_then(|h| h.get_mut(event))
            .and_then(|v| v.as_array_mut())
        else {
            return Ok(false);
        };
        let before = arr.len();
        arr.retain(|group| {
            if group
                .get("_blxcode_marker")
                .and_then(|v| v.as_str())
                .map(|s| s == marker)
                .unwrap_or(false)
            {
                return false;
            }
            let Some(inner) = group.get("hooks").and_then(|v| v.as_array()) else {
                return true;
            };
            !inner.iter().any(|h| {
                h.get("command")
                    .and_then(|v| v.as_str())
                    .map(|c| c.contains(script_name) || c.contains(marker))
                    .unwrap_or(false)
            })
        });
        before != arr.len()
    };
    if removed {
        write_json_pretty(settings_path, &settings)?;
    }
    Ok(removed)
}

fn claude_settings_path(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

fn codex_hooks_path(home: &Path) -> PathBuf {
    home.join(".codex").join("hooks.json")
}

/// Aggregate: title + session-capture for Claude.
fn install_claude(home: &Path, hooks_dir: &Path, app: &AppHandle) -> AgentHookEntry {
    let settings = claude_settings_path(home);
    let mut notes: Vec<String> = Vec::new();
    let mut last_script: Option<PathBuf> = None;

    for (script, event, matcher, marker) in [
        (CLAUDE_TITLE_SCRIPT, "UserPromptSubmit", "", CLAUDE_TITLE_MARKER),
        (
            CLAUDE_CAPTURE_SCRIPT,
            "SessionStart",
            "startup|resume|clear",
            CLAUDE_CAPTURE_MARKER,
        ),
    ] {
        match copy_script(app, script, hooks_dir) {
            Ok(path) => {
                last_script = Some(path.clone());
                match patch_settings(&settings, event, matcher, marker, &path) {
                    Ok(true) => notes.push(format!("{event} hook installed")),
                    Ok(false) => notes.push(format!("{event} hook already installed")),
                    Err(e) => notes.push(format!("{event} patch failed: {e}")),
                }
            }
            Err(e) => notes.push(format!("{script}: {e}")),
        }
    }

    AgentHookEntry {
        agent: "claude".into(),
        script_path: last_script.map(|p| p.to_string_lossy().into_owned()),
        config_path: Some(settings.to_string_lossy().into_owned()),
        installed: true,
        note: Some(notes.join("; ")),
    }
}

/// Aggregate: title (best-effort) + session-capture for Codex.
fn install_codex(home: &Path, hooks_dir: &Path, app: &AppHandle) -> AgentHookEntry {
    let cfg = codex_hooks_path(home);
    let mut notes: Vec<String> = Vec::new();
    let mut last_script: Option<PathBuf> = None;

    // Title hook for Codex: we ship the script but its wiring depends on
    // Codex's still-evolving UserPromptSubmit support. Best-effort patch;
    // failures are reported, not fatal.
    if let Ok(path) = copy_script(app, CODEX_TITLE_SCRIPT, hooks_dir) {
        last_script = Some(path.clone());
        // Same marker scheme; Codex hook config follows the Claude shape.
        match patch_settings(
            &cfg,
            "UserPromptSubmit",
            "",
            "blxcode:codex-title",
            &path,
        ) {
            Ok(true) => notes.push("UserPromptSubmit hook installed".into()),
            Ok(false) => notes.push("UserPromptSubmit hook already installed".into()),
            Err(e) => notes.push(format!("UserPromptSubmit best-effort: {e}")),
        }
    } else {
        notes.push("codex_title.py copy failed".into());
    }

    match copy_script(app, CODEX_CAPTURE_SCRIPT, hooks_dir) {
        Ok(path) => {
            last_script = Some(path.clone());
            match patch_settings(
                &cfg,
                "SessionStart",
                "startup|resume|clear",
                CODEX_CAPTURE_MARKER,
                &path,
            ) {
                Ok(true) => notes.push("SessionStart hook installed".into()),
                Ok(false) => notes.push("SessionStart hook already installed".into()),
                Err(e) => notes.push(format!("SessionStart patch failed: {e}")),
            }
        }
        Err(e) => notes.push(format!("codex_session_capture.py: {e}")),
    }

    AgentHookEntry {
        agent: "codex".into(),
        script_path: last_script.map(|p| p.to_string_lossy().into_owned()),
        config_path: Some(cfg.to_string_lossy().into_owned()),
        installed: true,
        note: Some(notes.join("; ")),
    }
}

#[tauri::command]
pub fn install_agent_hooks(app: AppHandle) -> Result<AgentHooksReport, String> {
    let home = home_dir(&app)?;
    let dir = hooks_dir(&app)?;
    ensure_dir(&dir)?;

    let entries = vec![install_claude(&home, &dir, &app), install_codex(&home, &dir, &app)];

    Ok(AgentHooksReport {
        hooks_dir: dir.to_string_lossy().into_owned(),
        entries,
    })
}

#[tauri::command]
pub fn agent_hooks_status(app: AppHandle) -> Result<AgentHooksReport, String> {
    let home = home_dir(&app)?;
    let dir = hooks_dir(&app)?;

    let claude_title = dir.join(CLAUDE_TITLE_SCRIPT);
    let claude_capture = dir.join(CLAUDE_CAPTURE_SCRIPT);
    let codex_title = dir.join(CODEX_TITLE_SCRIPT);
    let codex_capture = dir.join(CODEX_CAPTURE_SCRIPT);

    let claude_cfg = claude_settings_path(&home);
    let codex_cfg = codex_hooks_path(&home);

    let claude_installed = read_json_or_empty(&claude_cfg)
        .map(|v| {
            hook_already_installed(&v, "UserPromptSubmit", &claude_title)
                && hook_already_installed(&v, "SessionStart", &claude_capture)
        })
        .unwrap_or(false);
    let codex_installed = read_json_or_empty(&codex_cfg)
        .map(|v| hook_already_installed(&v, "SessionStart", &codex_capture))
        .unwrap_or(false);

    let entries = vec![
        AgentHookEntry {
            agent: "claude".into(),
            script_path: claude_capture
                .exists()
                .then(|| claude_capture.to_string_lossy().into_owned()),
            config_path: claude_cfg
                .exists()
                .then(|| claude_cfg.to_string_lossy().into_owned()),
            installed: claude_installed,
            note: None,
        },
        AgentHookEntry {
            agent: "codex".into(),
            script_path: codex_capture
                .exists()
                .then(|| codex_capture.to_string_lossy().into_owned()),
            config_path: codex_cfg
                .exists()
                .then(|| codex_cfg.to_string_lossy().into_owned()),
            installed: codex_installed,
            note: None,
        },
    ];
    let _ = (codex_title, claude_title); // silence unused on some cfgs

    Ok(AgentHooksReport {
        hooks_dir: dir.to_string_lossy().into_owned(),
        entries,
    })
}

#[tauri::command]
pub fn uninstall_agent_hooks(app: AppHandle) -> Result<AgentHooksReport, String> {
    let home = home_dir(&app)?;
    let dir = hooks_dir(&app)?;
    let claude_cfg = claude_settings_path(&home);
    let codex_cfg = codex_hooks_path(&home);

    let _ = unpatch_settings(
        &claude_cfg,
        "UserPromptSubmit",
        CLAUDE_TITLE_MARKER,
        CLAUDE_TITLE_SCRIPT,
    );
    let _ = unpatch_settings(
        &claude_cfg,
        "SessionStart",
        CLAUDE_CAPTURE_MARKER,
        CLAUDE_CAPTURE_SCRIPT,
    );
    let _ = unpatch_settings(
        &codex_cfg,
        "UserPromptSubmit",
        "blxcode:codex-title",
        CODEX_TITLE_SCRIPT,
    );
    let _ = unpatch_settings(
        &codex_cfg,
        "SessionStart",
        CODEX_CAPTURE_MARKER,
        CODEX_CAPTURE_SCRIPT,
    );

    for name in [
        CLAUDE_TITLE_SCRIPT,
        CLAUDE_CAPTURE_SCRIPT,
        CODEX_TITLE_SCRIPT,
        CODEX_CAPTURE_SCRIPT,
    ] {
        let p = dir.join(name);
        if p.exists() {
            let _ = fs::remove_file(p);
        }
    }

    Ok(AgentHooksReport {
        hooks_dir: dir.to_string_lossy().into_owned(),
        entries: vec![
            AgentHookEntry {
                agent: "claude".into(),
                script_path: None,
                config_path: Some(claude_cfg.to_string_lossy().into_owned()),
                installed: false,
                note: Some("hooks removed".into()),
            },
            AgentHookEntry {
                agent: "codex".into(),
                script_path: None,
                config_path: Some(codex_cfg.to_string_lossy().into_owned()),
                installed: false,
                note: Some("hooks removed".into()),
            },
        ],
    })
}
