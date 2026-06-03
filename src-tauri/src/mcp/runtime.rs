//! Process-wide live MCP state for the in-app agent.
//!
//! Holds the connected clients and their discovered tool specs for the current
//! chat session. Built lazily at turn start from the enabled registry; reset on
//! `agent_clear_conversation` and on any registry mutation so the next turn
//! reconnects. This mirrors the UI contract: registry edits require a session
//! reset before the in-app agent sees them.

use serde_json::{json, Value};
use std::collections::HashMap;
use tokio::sync::Mutex;

use super::client::{McpClient, McpToolSpec};
use super::registry;

struct ActiveServer {
    client: McpClient,
    /// Map of qualified tool name -> remote tool name.
    tools: Vec<McpToolSpec>,
}

#[derive(Default)]
struct ActiveMcp {
    /// `None` until built for the current session.
    servers: Option<HashMap<String, ActiveServer>>,
}

fn cell() -> &'static Mutex<ActiveMcp> {
    static CELL: std::sync::OnceLock<Mutex<ActiveMcp>> = std::sync::OnceLock::new();
    CELL.get_or_init(|| Mutex::new(ActiveMcp::default()))
}

/// Drop all live clients so the next turn rebuilds from the registry. Called on
/// session reset and registry mutations.
pub async fn reset() {
    let mut guard = cell().lock().await;
    guard.servers = None;
}

/// Synchronous best-effort reset usable from non-async command handlers.
pub fn reset_blocking() {
    if let Ok(mut guard) = cell().try_lock() {
        guard.servers = None;
    } else {
        // Fall back to spawning the async reset on the shared runtime.
        tauri::async_runtime::block_on(reset());
    }
}

/// Ensure clients are connected for the current session. Connection failures
/// are swallowed per-server (logged) so one broken server cannot break a turn.
pub async fn ensure_built() {
    {
        let guard = cell().lock().await;
        if guard.servers.is_some() {
            return;
        }
    }
    let registry = registry::load().unwrap_or_default();
    let mut built: HashMap<String, ActiveServer> = HashMap::new();
    for server in registry.enabled() {
        match McpClient::connect(server).await {
            Ok(client) => match client.list_tools(&server.id).await {
                Ok(tools) => {
                    built.insert(server.id.clone(), ActiveServer { client, tools });
                }
                Err(e) => {
                    eprintln!("[mcp] tools/list failed for {}: {e}", server.id);
                }
            },
            Err(e) => {
                eprintln!("[mcp] connect failed for {}: {e}", server.id);
            }
        }
    }
    let mut guard = cell().lock().await;
    // Another task may have built concurrently; keep the first result.
    if guard.servers.is_none() {
        guard.servers = Some(built);
    }
}

/// Collect all active tool specs (flattened across servers).
pub async fn active_tool_specs() -> Vec<McpToolSpec> {
    let guard = cell().lock().await;
    let Some(servers) = guard.servers.as_ref() else {
        return Vec::new();
    };
    servers
        .values()
        .flat_map(|s| s.tools.iter().cloned())
        .collect()
}

/// Render the active MCP tools as Anthropic `tools[]` entries.
pub async fn anthropic_tool_specs() -> Vec<Value> {
    active_tool_specs()
        .await
        .into_iter()
        .map(|t| {
            json!({
                // Anthropic forbids dots in tool names; the agent loop maps
                // `.` <-> `__` at the API boundary, so mirror that here.
                "name": t.qualified_name.replace('.', "__"),
                "description": t.description,
                "input_schema": t.input_schema,
            })
        })
        .collect()
}

/// Render the active MCP tools as OpenAI Chat Completions `tools[]` entries.
pub async fn openai_tool_specs() -> Vec<Value> {
    active_tool_specs()
        .await
        .into_iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.qualified_name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            })
        })
        .collect()
}

/// True when `name` could target an MCP tool in any of the encodings the agent
/// loops use: dotted (`mcp.fs.read`), Anthropic (`mcp__fs__read`) or OpenAI
/// (`mcp_fs_read`).
pub fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp.") || name.starts_with("mcp__") || name.starts_with("mcp_")
}

/// Resolve a tool name in any encoding back to its canonical qualified form by
/// matching against the active specs, then route the call to the right client.
pub async fn call(name: &str, args: &Value) -> Result<String, String> {
    let guard = cell().lock().await;
    let servers = guard
        .servers
        .as_ref()
        .ok_or_else(|| "mcp not initialised".to_string())?;
    // Find the (server, spec) whose qualified name matches `name` under any
    // encoding the agent loops use, then call via the server's own tool name.
    for srv in servers.values() {
        for tool in &srv.tools {
            let q = &tool.qualified_name;
            if name == q || name == q.replace('.', "__") || name == q.replace('.', "_") {
                return srv.client.call_tool(&tool.remote_name, args).await;
            }
        }
    }
    Err(format!("unknown mcp tool: {name}"))
}

/// `mcp.<server>.<tool>` -> (server, tool). The tool segment may itself contain
/// dots, so we split on the first separator after the `mcp.` prefix only.
#[cfg(test)]
fn split_qualified(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp.")?;
    let idx = rest.find('.')?;
    let server = &rest[..idx];
    let tool = &rest[idx + 1..];
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server, tool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_qualified_names_including_dotted_tools() {
        assert_eq!(split_qualified("mcp.fs.read_file"), Some(("fs", "read_file")));
        assert_eq!(
            split_qualified("mcp.gh.repos.list"),
            Some(("gh", "repos.list"))
        );
        assert_eq!(split_qualified("mcp.fs"), None);
        assert_eq!(split_qualified("read_file"), None);
    }

    #[test]
    fn is_mcp_tool_matches_prefix() {
        assert!(is_mcp_tool("mcp.fs.read_file"));
        assert!(!is_mcp_tool("read_workspace_file"));
    }
}
