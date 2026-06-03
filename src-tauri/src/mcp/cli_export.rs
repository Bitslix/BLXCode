//! Translate the central MCP registry into each bundled terminal CLI's native,
//! project-scoped config and write it into the workspace root at launch time.
//!
//! We never touch the user's *global* CLI configs; we only write project-scoped
//! files (`.mcp.json`, `.cursor/mcp.json`, `.gemini/settings.json`,
//! `.codex/config.toml`, `opencode.json`). Writes are merge-safe: entries we did
//! not create are preserved, and the set of keys we manage is tracked in a
//! sidecar manifest (`.blxcode/mcp-managed.json`) so pure-JSON formats without
//! comments can still be cleaned up deterministically.

use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::registry::{McpServer, McpTransport};

const MANIFEST_REL: &str = ".blxcode/mcp-managed.json";

/// Relative path (from workspace root) of the config file for a CLI slug.
fn config_rel_path(slug: &str) -> Option<&'static str> {
    match slug {
        "claude" => Some(".mcp.json"),
        "cursor" => Some(".cursor/mcp.json"),
        "gemini" => Some(".gemini/settings.json"),
        "codex" => Some(".codex/config.toml"),
        "opencode" => Some("opencode.json"),
        _ => None,
    }
}

/// Write the project-scoped MCP config for `slug` into `workspace_root`, merging
/// with any pre-existing file. Returns the path written, or `None` when the CLI
/// is unsupported. Disabled servers are skipped.
pub fn export_for_cli(slug: &str, workspace_root: &Path, servers: &[McpServer]) -> Result<Option<PathBuf>, String> {
    let Some(rel) = config_rel_path(slug) else {
        return Ok(None);
    };
    let enabled: Vec<&McpServer> = servers.iter().filter(|s| s.enabled).collect();
    let target = workspace_root.join(rel);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }

    let managed_keys: Vec<String> = enabled.iter().map(|s| s.id.clone()).collect();
    let previous_keys = read_manifest(workspace_root, slug);

    let existing = fs::read_to_string(&target).ok();
    let body = match slug {
        "codex" => render_codex_toml(&enabled, existing.as_deref(), &previous_keys)?,
        _ => render_json(slug, &enabled, existing.as_deref(), &previous_keys)?,
    };
    fs::write(&target, body).map_err(|e| format!("write {}: {e}", target.display()))?;
    write_manifest(workspace_root, slug, &managed_keys)?;
    Ok(Some(target))
}

/// Export configs for every supported CLI. Best-effort: errors are collected
/// and returned but do not stop the other exports.
pub fn export_all(workspace_root: &Path, servers: &[McpServer]) -> Vec<(String, Result<Option<PathBuf>, String>)> {
    ["claude", "codex", "gemini", "opencode", "cursor"]
        .into_iter()
        .map(|slug| (slug.to_string(), export_for_cli(slug, workspace_root, servers)))
        .collect()
}

// ---------------------------------------------------------------------------
// JSON formats (claude / cursor / gemini / opencode)
// ---------------------------------------------------------------------------

fn render_json(
    slug: &str,
    enabled: &[&McpServer],
    existing: Option<&str>,
    previous_keys: &[String],
) -> Result<String, String> {
    let mut root: Map<String, Value> = match existing {
        Some(raw) if !raw.trim().is_empty() => serde_json::from_str(raw)
            .map_err(|e| format!("parse existing {slug} config: {e}"))?,
        _ => Map::new(),
    };

    // opencode nests servers under `mcp`; the others use `mcpServers`.
    let section_key = if slug == "opencode" { "mcp" } else { "mcpServers" };
    let mut section: Map<String, Value> = root
        .get(section_key)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    // Drop keys we managed previously (so removed/renamed servers vanish) but
    // keep foreign entries the user added by hand.
    for key in previous_keys {
        section.remove(key);
    }
    for server in enabled {
        section.insert(server.id.clone(), server_to_json(slug, server));
    }

    if slug == "opencode" && !root.contains_key("$schema") {
        root.insert("$schema".into(), json!("https://opencode.ai/config.json"));
    }
    root.insert(section_key.to_string(), Value::Object(section));
    serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| format!("serialise {slug} config: {e}"))
}

fn server_to_json(slug: &str, server: &McpServer) -> Value {
    match &server.transport {
        McpTransport::Stdio { command, args, env } => {
            if slug == "opencode" {
                // opencode: type "local", command is a single array.
                let mut cmd = vec![Value::String(command.clone())];
                cmd.extend(args.iter().map(|a| Value::String(a.clone())));
                let mut obj = json!({ "type": "local", "command": cmd, "enabled": true });
                if !env.is_empty() {
                    obj["environment"] = map_to_json(env);
                }
                obj
            } else {
                let mut obj = json!({ "command": command, "args": args });
                if !env.is_empty() {
                    obj["env"] = map_to_json(env);
                }
                obj
            }
        }
        McpTransport::Http { url, headers } => {
            if slug == "opencode" {
                let mut obj = json!({ "type": "remote", "url": url, "enabled": true });
                if !headers.is_empty() {
                    obj["headers"] = map_to_json(headers);
                }
                obj
            } else {
                // claude/cursor/gemini accept type "http" with url + headers.
                let mut obj = json!({ "type": "http", "url": url });
                if !headers.is_empty() {
                    obj["headers"] = map_to_json(headers);
                }
                obj
            }
        }
    }
}

fn map_to_json(map: &BTreeMap<String, String>) -> Value {
    Value::Object(
        map.iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// TOML format (codex)
// ---------------------------------------------------------------------------

/// Codex uses `~/.codex/config.toml` style with `[mcp_servers.<name>]` tables.
/// We do not depend on a TOML crate; instead we keep any lines that are not
/// inside a BLXCode-managed `[mcp_servers.*]` block and append fresh blocks.
fn render_codex_toml(
    enabled: &[&McpServer],
    existing: Option<&str>,
    previous_keys: &[String],
) -> Result<String, String> {
    let mut preserved = String::new();
    if let Some(raw) = existing {
        preserved = strip_managed_codex_tables(raw, previous_keys);
    }
    let mut out = preserved.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    for server in enabled {
        out.push_str(&codex_table(server));
        out.push('\n');
    }
    Ok(out)
}

/// Remove `[mcp_servers.<key>]` tables for keys we previously managed, leaving
/// everything else (other tables, top-level config) untouched.
fn strip_managed_codex_tables(raw: &str, managed: &[String]) -> String {
    let managed_headers: Vec<String> = managed
        .iter()
        .map(|k| format!("[mcp_servers.{k}]"))
        .collect();
    let mut out = String::new();
    let mut skipping = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            // A new table header ends any skip region.
            skipping = managed_headers.iter().any(|h| trimmed == h);
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn codex_table(server: &McpServer) -> String {
    let mut s = format!("[mcp_servers.{}]\n", server.id);
    match &server.transport {
        McpTransport::Stdio { command, args, env } => {
            s.push_str(&format!("command = {}\n", toml_string(command)));
            s.push_str(&format!("args = {}\n", toml_string_array(args)));
            if !env.is_empty() {
                s.push_str(&format!("[mcp_servers.{}.env]\n", server.id));
                for (k, v) in env {
                    s.push_str(&format!("{} = {}\n", toml_key(k), toml_string(v)));
                }
            }
        }
        McpTransport::Http { url, headers } => {
            s.push_str(&format!("url = {}\n", toml_string(url)));
            if !headers.is_empty() {
                s.push_str(&format!("[mcp_servers.{}.headers]\n", server.id));
                for (k, v) in headers {
                    s.push_str(&format!("{} = {}\n", toml_key(k), toml_string(v)));
                }
            }
        }
    }
    s
}

fn toml_string(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn toml_string_array(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|i| toml_string(i)).collect();
    format!("[{}]", inner.join(", "))
}

/// Bare key when it is a simple identifier, quoted otherwise.
fn toml_key(k: &str) -> String {
    if !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        k.to_string()
    } else {
        toml_string(k)
    }
}

// ---------------------------------------------------------------------------
// Managed-keys manifest
// ---------------------------------------------------------------------------

fn manifest_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join(MANIFEST_REL)
}

fn read_manifest(workspace_root: &Path, slug: &str) -> Vec<String> {
    let path = manifest_path(workspace_root);
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    value
        .get(slug)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn write_manifest(workspace_root: &Path, slug: &str, keys: &[String]) -> Result<(), String> {
    let path = manifest_path(workspace_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let mut root: Map<String, Value> = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    root.insert(
        slug.to_string(),
        Value::Array(keys.iter().map(|k| Value::String(k.clone())).collect()),
    );
    let body = serde_json::to_string_pretty(&Value::Object(root))
        .map_err(|e| format!("serialise manifest: {e}"))?;
    fs::write(&path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio_server(id: &str) -> McpServer {
        let mut env = BTreeMap::new();
        env.insert("TOKEN".into(), "secret".into());
        McpServer {
            id: id.into(),
            name: id.into(),
            enabled: true,
            description: String::new(),
            transport: McpTransport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "server".into()],
                env,
            },
        }
    }

    #[test]
    fn claude_json_uses_mcp_servers_and_preserves_foreign() {
        let existing = r#"{ "mcpServers": { "hand": { "command": "x" } }, "other": 1 }"#;
        let out = render_json("claude", &[&stdio_server("fs")], Some(existing), &[]).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["other"], json!(1));
        assert!(v["mcpServers"]["hand"].is_object()); // foreign kept
        assert_eq!(v["mcpServers"]["fs"]["command"], json!("npx"));
        assert_eq!(v["mcpServers"]["fs"]["env"]["TOKEN"], json!("secret"));
    }

    #[test]
    fn previous_managed_keys_are_dropped_on_reexport() {
        let existing = r#"{ "mcpServers": { "old": { "command": "x" }, "hand": { "command": "y" } } }"#;
        let out = render_json("claude", &[&stdio_server("fs")], Some(existing), &["old".into()]).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["mcpServers"].get("old").is_none()); // managed -> removed
        assert!(v["mcpServers"]["hand"].is_object()); // foreign -> kept
        assert!(v["mcpServers"]["fs"].is_object());
    }

    #[test]
    fn opencode_uses_local_type_and_schema() {
        let out = render_json("opencode", &[&stdio_server("fs")], None, &[]).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["$schema"], json!("https://opencode.ai/config.json"));
        assert_eq!(v["mcp"]["fs"]["type"], json!("local"));
        assert_eq!(v["mcp"]["fs"]["command"][0], json!("npx"));
    }

    #[test]
    fn codex_renders_toml_tables_and_strips_managed() {
        let out = render_codex_toml(&[&stdio_server("fs")], None, &[]).unwrap();
        assert!(out.contains("[mcp_servers.fs]"));
        assert!(out.contains("command = \"npx\""));
        assert!(out.contains("[mcp_servers.fs.env]"));
        assert!(out.contains("TOKEN = \"secret\""));

        let existing = "[some.other]\nkeep = true\n\n[mcp_servers.fs]\ncommand = \"old\"\n";
        let out2 = render_codex_toml(&[&stdio_server("fs")], Some(existing), &["fs".into()]).unwrap();
        assert!(out2.contains("[some.other]"));
        assert!(out2.contains("keep = true"));
        assert_eq!(out2.matches("[mcp_servers.fs]").count(), 1);
        assert!(!out2.contains("command = \"old\""));
    }
}
