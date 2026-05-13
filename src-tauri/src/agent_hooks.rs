//! Install / inspect / uninstall blxcode's terminal-title hooks for
//! external agent CLIs (Claude Code, Codex). The hooks ship as resources
//! (see `tauri.conf.json` -> `bundle.resources`); the installer copies
//! them into the user's config dir and wires them into each agent's
//! settings file when possible.
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const CLAUDE_HOOK_NAME: &str = "claude_title.py";
const CODEX_HOOK_NAME: &str = "codex_title.py";
const CLAUDE_HOOK_MARKER: &str = "blxcode:claude-title";

/// Python interpreter the installed Claude hook command should invoke.
/// On Windows `python` is the canonical launcher; on Unix Claude users
/// will have `python3` (the `python` symlink is no longer universal).
#[cfg(target_os = "windows")]
const PYTHON_BIN: &str = "python";
#[cfg(not(target_os = "windows"))]
const PYTHON_BIN: &str = "python3";

/// Build the shell command Claude executes for the UserPromptSubmit hook.
/// Quoting handles paths with spaces on every supported shell (sh on
/// Unix, cmd.exe on Windows).
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
    fs::copy(&src, &dest).map_err(|e| format!("copy {} -> {}: {e}", src.display(), dest.display()))?;
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

/// Returns true if `~/.claude/settings.json` already references our hook script.
fn claude_hook_installed(settings: &serde_json::Value, script_path: &Path) -> bool {
    let Some(arr) = settings
        .get("hooks")
        .and_then(|h| h.get("UserPromptSubmit"))
        .and_then(|v| v.as_array())
    else {
        return false;
    };
    let needle = script_path.to_string_lossy();
    arr.iter().any(|group| {
        let Some(inner) = group.get("hooks").and_then(|v| v.as_array()) else {
            return false;
        };
        inner.iter().any(|h| {
            h.get("command")
                .and_then(|v| v.as_str())
                .map(|c| c.contains(needle.as_ref()) || c.contains(CLAUDE_HOOK_MARKER))
                .unwrap_or(false)
        })
    })
}

fn install_claude_entry(home: &Path, script_path: &Path) -> Result<(PathBuf, bool, Option<String>), String> {
    let settings_path = home.join(".claude").join("settings.json");
    let mut settings = read_json_or_empty(&settings_path)?;
    if claude_hook_installed(&settings, script_path) {
        return Ok((settings_path, true, Some("already installed".into())));
    }
    let new_entry = serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": build_hook_command(script_path),
        }],
        "_blxcode_marker": CLAUDE_HOOK_MARKER,
    });

    let root = settings.as_object_mut().ok_or_else(|| {
        format!(
            "{} is not a JSON object",
            settings_path.display()
        )
    })?;
    let hooks = root
        .entry("hooks".to_string())
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks.as_object_mut().ok_or_else(|| {
        format!("{}: .hooks is not an object", settings_path.display())
    })?;
    let arr = hooks_obj
        .entry("UserPromptSubmit".to_string())
        .or_insert_with(|| serde_json::json!([]));
    let arr_vec = arr.as_array_mut().ok_or_else(|| {
        format!(
            "{}: .hooks.UserPromptSubmit is not an array",
            settings_path.display()
        )
    })?;
    arr_vec.push(new_entry);

    write_json_pretty(&settings_path, &settings)?;
    Ok((settings_path, true, None))
}

fn uninstall_claude_entry(home: &Path) -> Result<(PathBuf, bool, Option<String>), String> {
    let settings_path = home.join(".claude").join("settings.json");
    if !settings_path.exists() {
        return Ok((settings_path, false, Some("settings.json not found".into())));
    }
    let mut settings = read_json_or_empty(&settings_path)?;
    let removed = {
        let Some(arr) = settings
            .get_mut("hooks")
            .and_then(|h| h.get_mut("UserPromptSubmit"))
            .and_then(|v| v.as_array_mut())
        else {
            return Ok((settings_path, false, Some("no hooks entry".into())));
        };
        let before = arr.len();
        arr.retain(|group| {
            if group
                .get("_blxcode_marker")
                .and_then(|v| v.as_str())
                .map(|s| s == CLAUDE_HOOK_MARKER)
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
                    .map(|c| c.contains(CLAUDE_HOOK_NAME) || c.contains(CLAUDE_HOOK_MARKER))
                    .unwrap_or(false)
            })
        });
        before != arr.len()
    };
    if removed {
        write_json_pretty(&settings_path, &settings)?;
    }
    Ok((settings_path, !removed, None))
}

#[tauri::command]
pub fn install_agent_hooks(app: AppHandle) -> Result<AgentHooksReport, String> {
    let home = home_dir(&app)?;
    let dir = hooks_dir(&app)?;
    ensure_dir(&dir)?;

    let mut entries: Vec<AgentHookEntry> = Vec::new();

    // Claude
    match copy_script(&app, CLAUDE_HOOK_NAME, &dir) {
        Ok(script_path) => match install_claude_entry(&home, &script_path) {
            Ok((cfg, _installed, note)) => entries.push(AgentHookEntry {
                agent: "claude".into(),
                script_path: Some(script_path.to_string_lossy().into_owned()),
                config_path: Some(cfg.to_string_lossy().into_owned()),
                installed: true,
                note,
            }),
            Err(e) => entries.push(AgentHookEntry {
                agent: "claude".into(),
                script_path: Some(script_path.to_string_lossy().into_owned()),
                config_path: None,
                installed: false,
                note: Some(e),
            }),
        },
        Err(e) => entries.push(AgentHookEntry {
            agent: "claude".into(),
            script_path: None,
            config_path: None,
            installed: false,
            note: Some(e),
        }),
    }

    // Codex — script-only. Codex's hook schema is not stable enough to patch
    // automatically; ship the script and surface its path so the user (or a
    // future UI step) can wire it into their codex config.
    match copy_script(&app, CODEX_HOOK_NAME, &dir) {
        Ok(script_path) => entries.push(AgentHookEntry {
            agent: "codex".into(),
            script_path: Some(script_path.to_string_lossy().into_owned()),
            config_path: None,
            installed: false,
            note: Some(
                "script installed; wire into ~/.codex/ hook config manually".into(),
            ),
        }),
        Err(e) => entries.push(AgentHookEntry {
            agent: "codex".into(),
            script_path: None,
            config_path: None,
            installed: false,
            note: Some(e),
        }),
    }

    Ok(AgentHooksReport {
        hooks_dir: dir.to_string_lossy().into_owned(),
        entries,
    })
}

#[tauri::command]
pub fn agent_hooks_status(app: AppHandle) -> Result<AgentHooksReport, String> {
    let home = home_dir(&app)?;
    let dir = hooks_dir(&app)?;

    let claude_script = dir.join(CLAUDE_HOOK_NAME);
    let codex_script = dir.join(CODEX_HOOK_NAME);

    let claude_settings_path = home.join(".claude").join("settings.json");
    let claude_installed = read_json_or_empty(&claude_settings_path)
        .map(|v| claude_hook_installed(&v, &claude_script))
        .unwrap_or(false);

    let entries = vec![
        AgentHookEntry {
            agent: "claude".into(),
            script_path: claude_script.exists().then(|| claude_script.to_string_lossy().into_owned()),
            config_path: claude_settings_path.exists()
                .then(|| claude_settings_path.to_string_lossy().into_owned()),
            installed: claude_installed,
            note: None,
        },
        AgentHookEntry {
            agent: "codex".into(),
            script_path: codex_script.exists().then(|| codex_script.to_string_lossy().into_owned()),
            config_path: None,
            installed: false,
            note: Some("manual wiring required".into()),
        },
    ];

    Ok(AgentHooksReport {
        hooks_dir: dir.to_string_lossy().into_owned(),
        entries,
    })
}

#[tauri::command]
pub fn uninstall_agent_hooks(app: AppHandle) -> Result<AgentHooksReport, String> {
    let home = home_dir(&app)?;
    let dir = hooks_dir(&app)?;

    let mut entries: Vec<AgentHookEntry> = Vec::new();

    let (cfg, still_installed, note) = uninstall_claude_entry(&home)
        .unwrap_or_else(|e| (home.join(".claude/settings.json"), false, Some(e)));
    entries.push(AgentHookEntry {
        agent: "claude".into(),
        script_path: None,
        config_path: Some(cfg.to_string_lossy().into_owned()),
        installed: still_installed,
        note,
    });

    let claude_script = dir.join(CLAUDE_HOOK_NAME);
    if claude_script.exists() {
        let _ = fs::remove_file(&claude_script);
    }
    let codex_script = dir.join(CODEX_HOOK_NAME);
    if codex_script.exists() {
        let _ = fs::remove_file(&codex_script);
    }
    entries.push(AgentHookEntry {
        agent: "codex".into(),
        script_path: None,
        config_path: None,
        installed: false,
        note: Some("script removed".into()),
    });

    Ok(AgentHooksReport {
        hooks_dir: dir.to_string_lossy().into_owned(),
        entries,
    })
}
