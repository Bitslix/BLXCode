//! MCP (Model Context Protocol) server support.
//!
//! - [`registry`] — persistent list of user-registered servers.
//! - [`client`] — minimal JSON-RPC client (stdio + HTTP).
//! - [`runtime`] — live clients + tool specs for the in-app agent loop.
//! - [`cli_export`] — translate the registry into bundled terminal CLI configs.

pub mod cli_export;
pub mod client;
pub mod registry;
pub mod runtime;

use serde::Serialize;

use tauri::AppHandle;

pub use registry::McpServer;

/// Source tag used for all MCP entries in the app log.
const LOG_SRC: &str = "mcp";

/// Result of a connection test against a single server.
#[derive(Debug, Serialize)]
pub struct McpTestResult {
    pub ok: bool,
    pub tool_count: usize,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn mcp_list() -> Result<Vec<McpServer>, String> {
    Ok(registry::load()?.servers)
}

#[tauri::command]
pub fn mcp_upsert(app: AppHandle, server: McpServer) -> Result<McpServer, String> {
    let mut reg = registry::load()?;
    let others: Vec<McpServer> = reg
        .servers
        .iter()
        .filter(|s| s.id != server.id)
        .cloned()
        .collect();
    let existed = reg.servers.iter().any(|s| s.id == server.id);
    let normalized = match registry::normalize_for_upsert(server, &others) {
        Ok(n) => n,
        Err(e) => {
            crate::app_logging::write_app_event(
                &app,
                "warn",
                LOG_SRC,
                "upsert_rejected",
                serde_json::json!({ "error": e }),
            );
            return Err(e);
        }
    };
    if let Some(slot) = reg.servers.iter_mut().find(|s| s.id == normalized.id) {
        *slot = normalized.clone();
    } else {
        reg.servers.push(normalized.clone());
    }
    registry::save(&reg)?;
    // Registry changed -> the in-app agent must reconnect on the next turn.
    runtime::reset_blocking();
    crate::app_logging::write_app_event(
        &app,
        "info",
        LOG_SRC,
        if existed {
            "server_updated"
        } else {
            "server_added"
        },
        serde_json::json!({
            "id": normalized.id,
            "transport": normalized.transport.label(),
            "enabled": normalized.enabled,
        }),
    );
    Ok(normalized)
}

#[tauri::command]
pub fn mcp_remove(app: AppHandle, id: String) -> Result<(), String> {
    let mut reg = registry::load()?;
    let before = reg.servers.len();
    reg.servers.retain(|s| s.id != id);
    if reg.servers.len() != before {
        registry::save(&reg)?;
        runtime::reset_blocking();
        crate::app_logging::write_app_event(
            &app,
            "info",
            LOG_SRC,
            "server_removed",
            serde_json::json!({ "id": id }),
        );
    }
    Ok(())
}

/// Outcome of exporting the registry into every supported terminal CLI's
/// project-scoped config inside a workspace.
#[derive(Debug, Serialize)]
pub struct McpExportEntry {
    pub slug: String,
    pub path: Option<String>,
    pub error: Option<String>,
}

/// Write project-scoped MCP configs for all bundled terminal CLIs into
/// `workspace_root`. Called by the frontend right before launching a terminal
/// agent so the CLI picks up the same servers the in-app agent uses.
#[tauri::command]
pub fn mcp_export_cli_configs(
    app: AppHandle,
    workspace_root: String,
) -> Result<Vec<McpExportEntry>, String> {
    let root = std::path::PathBuf::from(workspace_root.trim());
    if !root.is_dir() {
        return Err("workspace_root is not a directory".into());
    }
    let servers = registry::load()?.servers;
    let enabled = servers.iter().filter(|s| s.enabled).count();
    let entries: Vec<McpExportEntry> = cli_export::export_all(&root, &servers)
        .into_iter()
        .map(|(slug, res)| match res {
            Ok(path) => McpExportEntry {
                slug,
                path: path.map(|p| p.to_string_lossy().into_owned()),
                error: None,
            },
            Err(e) => McpExportEntry {
                slug,
                path: None,
                error: Some(e),
            },
        })
        .collect();
    let failed: Vec<&McpExportEntry> = entries.iter().filter(|e| e.error.is_some()).collect();
    if failed.is_empty() {
        crate::app_logging::write_app_event(
            &app,
            "info",
            LOG_SRC,
            "cli_configs_exported",
            serde_json::json!({ "enabledServers": enabled, "clis": entries.len() }),
        );
    } else {
        crate::app_logging::write_app_event(
            &app,
            "warn",
            LOG_SRC,
            "cli_configs_export_partial",
            serde_json::json!({
                "enabledServers": enabled,
                "failed": failed.iter().map(|e| {
                    serde_json::json!({ "cli": e.slug, "error": e.error })
                }).collect::<Vec<_>>(),
            }),
        );
    }
    Ok(entries)
}

#[tauri::command]
pub async fn mcp_test(app: AppHandle, id: String) -> Result<McpTestResult, String> {
    let reg = registry::load()?;
    let server = reg
        .servers
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| format!("unknown mcp server: {id}"))?;
    let result = match client::McpClient::connect(&server).await {
        Ok(c) => match c.list_tools(&server.id).await {
            Ok(tools) => McpTestResult {
                ok: true,
                tool_count: tools.len(),
                error: None,
            },
            Err(e) => McpTestResult {
                ok: false,
                tool_count: 0,
                error: Some(e),
            },
        },
        Err(e) => McpTestResult {
            ok: false,
            tool_count: 0,
            error: Some(e),
        },
    };
    crate::app_logging::write_app_event(
        &app,
        if result.ok { "info" } else { "warn" },
        LOG_SRC,
        "server_tested",
        serde_json::json!({
            "id": id,
            "ok": result.ok,
            "toolCount": result.tool_count,
            "error": result.error,
        }),
    );
    Ok(result)
}
