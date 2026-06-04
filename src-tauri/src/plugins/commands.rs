use super::install::{
    install_from_github, PluginInstallProgress, PluginInstallRequest, PluginInstallState,
};
use super::run_detectors::{
    built_in_detector_rules, discover_for_plugin, parse_detector_file, RunDetectorRule,
    WorkspaceScan,
};
use super::store;
use super::types::{
    PluginCapability, PluginInstallKind, PluginRegistry, PluginRegistryEntry, RunCommand,
};
use crate::pty_host::{sh_quote, PtyManager};
use crate::ssh_exec::{RemoteExecManager, EXEC_TIMEOUT_MS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use tauri::{AppHandle, State};

const REMOTE_TEXT_READ_LIMIT: usize = 120;
const REMOTE_TEXT_READ_BYTES: u64 = 256 * 1024;

#[tauri::command]
pub fn plugins_list() -> Result<PluginRegistry, String> {
    store::load_registry_with_builtins()
}

#[tauri::command]
pub async fn plugins_install_from_github(
    state: State<'_, PluginInstallState>,
    request: PluginInstallRequest,
) -> Result<PluginRegistry, String> {
    let state = state.inner().clone();
    crate::proc::run_blocking(move || {
        install_from_github(&state, request)?;
        store::load_registry_with_builtins()
    })
    .await
}

#[tauri::command]
pub fn plugins_install_progress(
    state: State<'_, PluginInstallState>,
) -> Result<PluginInstallProgress, String> {
    state.snapshot()
}

#[tauri::command]
pub fn plugins_set_enabled(plugin_id: String, enabled: bool) -> Result<PluginRegistry, String> {
    store::set_enabled(&plugin_id, enabled)
}

#[tauri::command]
pub fn plugins_remove(plugin_id: String) -> Result<PluginRegistry, String> {
    store::remove_entry(&plugin_id)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommandsDiscoverRequest {
    pub workspace_root: String,
    #[serde(default)]
    pub connection_id: Option<String>,
}

#[tauri::command]
pub fn run_commands_discover(
    app: AppHandle,
    pty: State<'_, PtyManager>,
    exec: State<'_, RemoteExecManager>,
    request: RunCommandsDiscoverRequest,
) -> Result<Vec<RunCommand>, String> {
    let scan = match request.connection_id.as_deref() {
        Some(connection_id) => {
            remote_workspace_scan(&app, &pty, &exec, connection_id, &request.workspace_root)?
        }
        None => WorkspaceScan::from_root(&request.workspace_root)?,
    };
    let registry = store::load_registry_with_builtins()?;
    discover_commands_from_scan(&scan, &registry)
}

fn discover_commands_from_scan(
    scan: &WorkspaceScan,
    registry: &PluginRegistry,
) -> Result<Vec<RunCommand>, String> {
    let mut commands = Vec::new();
    for entry in registry.enabled_runtime_plugins() {
        let rules = detector_rules_for_entry(entry)?;
        commands.extend(discover_for_plugin(scan, &entry.manifest.id, &rules));
    }
    commands.sort_by(|a, b| {
        a.cwd_rel
            .cmp(&b.cwd_rel)
            .then(a.kind.cmp(&b.kind))
            .then(a.label.cmp(&b.label))
            .then(a.command.cmp(&b.command))
    });
    Ok(commands)
}

fn detector_rules_for_entry(entry: &PluginRegistryEntry) -> Result<Vec<RunDetectorRule>, String> {
    if entry.source.kind == PluginInstallKind::BuiltIn {
        return Ok(built_in_detector_rules(&entry.manifest.id));
    }

    let Some(plugin_root) = entry.path.as_deref() else {
        return Ok(Vec::new());
    };
    let mut rules = Vec::new();
    for contribution in entry
        .manifest
        .commands
        .iter()
        .filter(|contribution| contribution.capability == PluginCapability::RunCommands)
    {
        let path = std::path::Path::new(plugin_root).join(&contribution.path);
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("read detector {}: {e}", path.display()))?;
        rules.extend(parse_detector_file(&raw)?.detectors);
    }
    Ok(rules)
}

fn remote_workspace_scan(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    workspace_root: &str,
) -> Result<WorkspaceScan, String> {
    let root = normalize_remote_root(workspace_root)?;
    let files = remote_list_files(app, pty, exec, connection_id, &root)?;
    let texts = remote_read_detector_texts(app, pty, exec, connection_id, &root, &files)?;
    Ok(WorkspaceScan::from_files_and_texts(root, files, texts))
}

fn remote_list_files(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    root: &str,
) -> Result<Vec<String>, String> {
    let prunes = [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".cache",
        "vendor",
    ]
    .iter()
    .map(|name| format!("-name {}", sh_quote(name)))
    .collect::<Vec<_>>()
    .join(" -o ");
    let cmd = format!(
        "find {} \\( {} \\) -prune -o -type f -print 2>/dev/null | head -n 5000",
        sh_quote(root),
        prunes
    );
    let stdout = exec.run_text(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
    let prefix = format!("{}/", root.trim_end_matches('/'));
    let mut files = stdout
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter_map(|line| line.strip_prefix(&prefix))
        .filter(|rel| safe_remote_rel(rel))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    files.sort();
    files.dedup();
    Ok(files)
}

fn remote_read_detector_texts(
    app: &AppHandle,
    pty: &PtyManager,
    exec: &RemoteExecManager,
    connection_id: &str,
    root: &str,
    files: &[String],
) -> Result<BTreeMap<String, String>, String> {
    let mut texts = BTreeMap::new();
    for rel in files
        .iter()
        .filter(|rel| detector_text_candidate(rel))
        .take(REMOTE_TEXT_READ_LIMIT)
    {
        let target = format!("{}/{}", root.trim_end_matches('/'), rel);
        let cmd = format!("head -c {REMOTE_TEXT_READ_BYTES} -- {}", sh_quote(&target));
        let output = exec.run(app, pty, connection_id, &cmd, EXEC_TIMEOUT_MS)?;
        if output.ok() {
            if let Ok(text) = String::from_utf8(output.stdout) {
                texts.insert(rel.clone(), text);
            }
        }
    }
    Ok(texts)
}

fn detector_text_candidate(rel: &str) -> bool {
    matches!(
        rel.rsplit('/').next().unwrap_or(rel),
        "package.json" | "Cargo.toml" | "Makefile" | "makefile" | "GNUmakefile"
    )
}

fn normalize_remote_root(root: &str) -> Result<String, String> {
    let root = root.trim();
    if root.is_empty() || root.contains('\0') {
        return Err("workspace root is empty".into());
    }
    Ok(root.trim_end_matches('/').to_string())
}

fn safe_remote_rel(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains('\0')
        && !rel.split('/').any(|part| part.is_empty() || part == "..")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::builtins::built_in_plugins;

    #[test]
    fn discover_commands_uses_enabled_runtime_plugins() {
        let registry = PluginRegistry {
            version: 1,
            plugins: built_in_plugins()
                .into_iter()
                .map(|mut entry| {
                    entry.enabled = entry.manifest.id == "runtime-node";
                    entry
                })
                .collect(),
        };
        let mut texts = BTreeMap::new();
        texts.insert(
            "package.json".into(),
            r#"{"scripts":{"dev":"vite","build":"vite build"}}"#.into(),
        );
        let scan = WorkspaceScan::from_files_and_texts(
            "/workspace",
            ["package.json".into(), "Cargo.toml".into()],
            texts,
        );
        let commands = discover_commands_from_scan(&scan, &registry).unwrap();
        assert!(commands.iter().any(|cmd| cmd.command == "npm run dev"));
        assert!(!commands.iter().any(|cmd| cmd.command == "cargo build"));
    }

    #[test]
    fn remote_rel_validation_rejects_escaping_paths() {
        assert!(safe_remote_rel("apps/web/package.json"));
        assert!(!safe_remote_rel("../package.json"));
        assert!(!safe_remote_rel("/package.json"));
    }
}
