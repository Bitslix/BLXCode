//! Workspace environment detection and session cache.

use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use serde_json::{json, Value};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
struct CacheEntry {
    workspace: String,
}

static ENV_CACHE: OnceLock<Mutex<Option<CacheEntry>>> = OnceLock::new();

fn cache() -> &'static Mutex<Option<CacheEntry>> {
    ENV_CACHE.get_or_init(|| Mutex::new(None))
}

pub fn invalidate_cache() {
    if let Ok(mut g) = cache().lock() {
        *g = None;
    }
}

/// Call when the active workspace changes or a new turn starts with a different root.
pub fn note_workspace_change(workspace: Option<&str>) {
    let ws = workspace.unwrap_or("").trim();
    let mut g = match cache().lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    match g.as_ref() {
        None => {}
        Some(entry) if entry.workspace == ws => {}
        Some(_) => *g = None,
    }
}

pub fn workspace_has_cache(workspace: &str) -> bool {
    cache()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|e| e.workspace == workspace))
        .unwrap_or(false)
}

pub fn require_environment(workspace: &str) -> Result<(), ToolOutcome> {
    if workspace_has_cache(workspace) {
        Ok(())
    } else {
        Err(ToolOutcome {
            ok: false,
            content: "call environment_detect first for this workspace".into(),
        })
    }
}

fn detect_os() -> &'static str {
    match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        other => other,
    }
}

fn default_shell() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "bash"
    }
}

fn available_shells() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["powershell", "pwsh", "cmd"]
    } else {
        vec!["bash", "sh"]
    }
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn detect(root: &WorkspaceRootGuard) -> Value {
    json!({
        "os": detect_os(),
        "arch": std::env::consts::ARCH,
        "defaultShell": default_shell(),
        "availableShells": available_shells(),
        "pathSeparator": std::path::MAIN_SEPARATOR.to_string(),
        "lineEnding": if cfg!(windows) { "\r\n" } else { "\n" },
        "gitAvailable": git_available(),
        "workspaceRoot": root.as_str(),
    })
}

pub fn tool_environment_detect(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(root) = root else {
        return ToolOutcome {
            ok: false,
            content: "no workspace configured".into(),
        };
    };
    let snapshot = detect(root);
    let ws = root.as_str();
    if let Ok(mut g) = cache().lock() {
        *g = Some(CacheEntry {
            workspace: ws.clone(),
        });
    }
    match serde_json::to_string(&snapshot) {
        Ok(body) => ToolOutcome {
            ok: true,
            content: body,
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("serialize environment: {e}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_invalidated_on_clear() {
        let dir = std::env::temp_dir().join(format!("blx-env-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = WorkspaceRootGuard::parse(dir.to_str().unwrap())
            .unwrap()
            .unwrap();
        let _ = tool_environment_detect(Some(&root));
        assert!(workspace_has_cache(&root.as_str()));
        invalidate_cache();
        assert!(!workspace_has_cache(&root.as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
