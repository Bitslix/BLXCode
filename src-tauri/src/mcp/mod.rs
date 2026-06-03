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

pub use registry::McpServer;

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
pub fn mcp_upsert(server: McpServer) -> Result<McpServer, String> {
    let mut reg = registry::load()?;
    let others: Vec<McpServer> = reg
        .servers
        .iter()
        .filter(|s| s.id != server.id)
        .cloned()
        .collect();
    let normalized = registry::normalize_for_upsert(server, &others)?;
    if let Some(slot) = reg.servers.iter_mut().find(|s| s.id == normalized.id) {
        *slot = normalized.clone();
    } else {
        reg.servers.push(normalized.clone());
    }
    registry::save(&reg)?;
    // Registry changed -> the in-app agent must reconnect on the next turn.
    runtime::reset_blocking();
    Ok(normalized)
}

#[tauri::command]
pub fn mcp_remove(id: String) -> Result<(), String> {
    let mut reg = registry::load()?;
    let before = reg.servers.len();
    reg.servers.retain(|s| s.id != id);
    if reg.servers.len() != before {
        registry::save(&reg)?;
        runtime::reset_blocking();
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
pub fn mcp_export_cli_configs(workspace_root: String) -> Result<Vec<McpExportEntry>, String> {
    let root = std::path::PathBuf::from(workspace_root.trim());
    if !root.is_dir() {
        return Err("workspace_root is not a directory".into());
    }
    let servers = registry::load()?.servers;
    let entries = cli_export::export_all(&root, &servers)
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
    Ok(entries)
}

#[tauri::command]
pub async fn mcp_test(id: String) -> Result<McpTestResult, String> {
    let reg = registry::load()?;
    let server = reg
        .servers
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or_else(|| format!("unknown mcp server: {id}"))?;
    match client::McpClient::connect(&server).await {
        Ok(c) => match c.list_tools(&server.id).await {
            Ok(tools) => Ok(McpTestResult {
                ok: true,
                tool_count: tools.len(),
                error: None,
            }),
            Err(e) => Ok(McpTestResult {
                ok: false,
                tool_count: 0,
                error: Some(e),
            }),
        },
        Err(e) => Ok(McpTestResult {
            ok: false,
            tool_count: 0,
            error: Some(e),
        }),
    }
}
