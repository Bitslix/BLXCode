//! Central MCP-server registry.
//!
//! Single source of truth for the user-registered MCP servers, persisted as
//! `{app_data_dir}/mcp/servers.json`. The registry feeds two consumers: the
//! in-app agent (which spins up MCP clients and injects their tools into the
//! tool loop) and the terminal CLI exporters (which translate it into each
//! bundled CLI's native project-scoped config). Secrets (stdio env values,
//! HTTP auth headers) are stored in plaintext here, like other local configs;
//! moving them into the OS keyring is a follow-up.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::app_paths;

const MCP_DIR: &str = "mcp";
const SERVERS_FILE: &str = "servers.json";

/// Transport used to reach an MCP server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum McpTransport {
    /// Local process speaking JSON-RPC over stdio.
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    /// Remote server reached over HTTP (Streamable HTTP / SSE endpoint).
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

/// A single registered MCP server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    /// Stable identifier, also used for tool namespacing (`mcp.<id>.<tool>`).
    pub id: String,
    /// Human-friendly display name.
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub description: String,
    pub transport: McpTransport,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct McpRegistry {
    #[serde(default)]
    pub servers: Vec<McpServer>,
}

impl McpRegistry {
    pub fn enabled(&self) -> impl Iterator<Item = &McpServer> {
        self.servers.iter().filter(|s| s.enabled)
    }
}

/// `{app_data_dir}/mcp` (created on demand).
fn mcp_dir() -> Result<PathBuf, String> {
    let dir = app_paths::app_data_dir()?.join(MCP_DIR);
    fs::create_dir_all(&dir).map_err(|e| format!("create mcp dir {}: {e}", dir.display()))?;
    Ok(dir)
}

/// `{app_data_dir}/mcp/servers.json`.
pub fn servers_path() -> Result<PathBuf, String> {
    Ok(mcp_dir()?.join(SERVERS_FILE))
}

/// Load the registry, returning an empty one when the file does not exist.
pub fn load() -> Result<McpRegistry, String> {
    let path = servers_path()?;
    match fs::read_to_string(&path) {
        Ok(raw) if raw.trim().is_empty() => Ok(McpRegistry::default()),
        Ok(raw) => serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(McpRegistry::default()),
        Err(e) => Err(format!("read {}: {e}", path.display())),
    }
}

/// Persist the registry atomically (tmp file + rename).
pub fn save(registry: &McpRegistry) -> Result<(), String> {
    let path = servers_path()?;
    let body = serde_json::to_string_pretty(registry)
        .map_err(|e| format!("serialize mcp registry: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, body.as_bytes()).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} -> {}: {e}", tmp.display(), path.display()))?;
    Ok(())
}

/// Validate and normalise a server before it is stored. Assigns an id when the
/// caller left it empty (derived from the name, with a uniqueness suffix).
pub fn normalize_for_upsert(
    mut server: McpServer,
    existing: &[McpServer],
) -> Result<McpServer, String> {
    server.name = server.name.trim().to_string();
    if server.name.is_empty() {
        return Err("server name must not be empty".into());
    }
    server.id = server.id.trim().to_string();
    if server.id.is_empty() {
        server.id = derive_unique_id(&server.name, existing);
    } else if !is_valid_id(&server.id) {
        return Err("server id may only contain [a-z0-9_-]".into());
    }
    match &server.transport {
        McpTransport::Stdio { command, .. } => {
            if command.trim().is_empty() {
                return Err("stdio transport requires a command".into());
            }
        }
        McpTransport::Http { url, .. } => {
            let url = url.trim();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("http transport requires an http(s) url".into());
            }
        }
    }
    Ok(server)
}

fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn derive_unique_id(name: &str, existing: &[McpServer]) -> String {
    let base: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let base = base.trim_matches('-');
    let base = if base.is_empty() { "server" } else { base };
    let mut candidate = base.to_string();
    let mut n = 2;
    while existing.iter().any(|s| s.id == candidate) {
        candidate = format!("{base}-{n}");
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio(cmd: &str) -> McpTransport {
        McpTransport::Stdio {
            command: cmd.into(),
            args: vec![],
            env: BTreeMap::new(),
        }
    }

    #[test]
    fn roundtrip_serialises_transport_tag() {
        let reg = McpRegistry {
            servers: vec![McpServer {
                id: "fs".into(),
                name: "Filesystem".into(),
                enabled: true,
                description: String::new(),
                transport: stdio("npx"),
            }],
        };
        let json = serde_json::to_string(&reg).unwrap();
        assert!(json.contains("\"kind\":\"stdio\""));
        let back: McpRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.servers, reg.servers);
    }

    #[test]
    fn upsert_assigns_unique_id_from_name() {
        let existing = vec![McpServer {
            id: "my-server".into(),
            name: "My Server".into(),
            enabled: true,
            description: String::new(),
            transport: stdio("a"),
        }];
        let fresh = McpServer {
            id: String::new(),
            name: "My Server".into(),
            enabled: true,
            description: String::new(),
            transport: stdio("b"),
        };
        let out = normalize_for_upsert(fresh, &existing).unwrap();
        assert_eq!(out.id, "my-server-2");
    }

    #[test]
    fn upsert_rejects_empty_name_and_bad_transport() {
        let blank = McpServer {
            id: String::new(),
            name: "  ".into(),
            enabled: true,
            description: String::new(),
            transport: stdio("x"),
        };
        assert!(normalize_for_upsert(blank, &[]).is_err());

        let bad_http = McpServer {
            id: String::new(),
            name: "Remote".into(),
            enabled: true,
            description: String::new(),
            transport: McpTransport::Http {
                url: "ftp://nope".into(),
                headers: BTreeMap::new(),
            },
        };
        assert!(normalize_for_upsert(bad_http, &[]).is_err());
    }

    #[test]
    fn enabled_filters_disabled_servers() {
        let reg = McpRegistry {
            servers: vec![
                McpServer {
                    id: "on".into(),
                    name: "On".into(),
                    enabled: true,
                    description: String::new(),
                    transport: stdio("a"),
                },
                McpServer {
                    id: "off".into(),
                    name: "Off".into(),
                    enabled: false,
                    description: String::new(),
                    transport: stdio("b"),
                },
            ],
        };
        let ids: Vec<&str> = reg.enabled().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["on"]);
    }
}
