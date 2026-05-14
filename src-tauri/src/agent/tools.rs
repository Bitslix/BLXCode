//! Tool registry + sandboxed read-only file access.
//!
//! Each tool advertises a JSON schema and a server/client execution kind.
//! The orchestrator renders the registry for the provider, then dispatches
//! incoming tool calls back through here.

use crate::memory;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceRootGuard {
    /// Canonical filesystem path to project root (UTF-8 for MVP).
    path: PathBuf,
}

impl WorkspaceRootGuard {
    pub fn parse(user_input: &str) -> Result<Option<Self>, String> {
        let p = PathBuf::from(user_input.trim());
        if user_input.trim().is_empty() {
            return Ok(None);
        }
        let canon = fs::canonicalize(&p).map_err(|e| format!("canonicalize workspace: {e}"))?;
        Ok(Some(Self { path: canon }))
    }

    pub fn contains(&self, abs: &Path) -> bool {
        abs.strip_prefix(&self.path).is_ok() || abs == self.path
    }

    pub fn as_str(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScopedReadOps;

#[derive(Clone, Debug, thiserror::Error)]
pub enum ReadToolError {
    #[error("no workspace configured")]
    NoWorkspace,
    #[error("path escapes workspace root")]
    PathEscape,
    #[error("invalid path")]
    InvalidPath,
    #[error("{0}")]
    Io(String),
}

impl ScopedReadOps {
    pub fn read_text(
        root: Option<&WorkspaceRootGuard>,
        relative: &str,
    ) -> Result<String, ReadToolError> {
        let guard = root.ok_or(ReadToolError::NoWorkspace)?;
        let rel = RelativePath::normalize(relative).ok_or(ReadToolError::InvalidPath)?;
        let full = guard.path.join(&rel);

        guard
            .contains(&full)
            .then_some(())
            .ok_or(ReadToolError::PathEscape)?;

        fs::read_to_string(&full).map_err(|e| ReadToolError::Io(e.to_string()))
    }
}

struct RelativePath;

impl RelativePath {
    fn normalize(rel: &str) -> Option<PathBuf> {
        let trimmed = rel.trim_start_matches('/');
        if trimmed.contains("..") {
            return None;
        }
        if trimmed.is_empty() {
            return None;
        }
        Some(PathBuf::from(trimmed))
    }
}

// ---------------------------------------------------------------------
// Tool registry

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolSite {
    /// Executed in-process by the orchestrator.
    Server,
    /// Executed by the UI; the orchestrator emits `ToolCall` and awaits
    /// `agent_submit_tool_result`.
    Client,
}

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
    pub site: ToolSite,
}

/// Output of an in-process server-tool execution.
pub struct ToolOutcome {
    pub ok: bool,
    /// Text payload sent both to the user-facing `ToolResult` event and
    /// (verbatim) back into the LLM as the tool-message content.
    pub content: String,
}

pub fn registry() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read_workspace_file",
            description: "Read a UTF-8 text file under the workspace root. Path is relative to the workspace; absolute paths and `..` are rejected.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative path within the workspace." }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_list",
            description: "List Markdown notes under <workspace>/.blxcode/memory/. Returns at most 200 entries (path, name, size).",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_read",
            description: "Read a Markdown note under <workspace>/.blxcode/memory/. Path is relative to that root and must end in .md.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_create",
            description: "Create a new Markdown note under <workspace>/.blxcode/memory/. Path is relative, must end in .md, must not exist. Content is capped at 32 KiB per call.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_write",
            description: "Overwrite an existing Markdown note under <workspace>/.blxcode/memory/. Path is relative, must end in .md. Content is capped at 32 KiB per call.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_search",
            description: "Full-text search across <workspace>/.blxcode/memory/. Returns up to 50 hits (path, line, snippet).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "harness.list_terminals",
            description: "List terminal slots in the active workspace. Each entry has `slotId`, `agentSlug` (one of claude/codex/gemini/opencode/cursor or empty for plain shell), and `running` (whether a PTY session is currently attached). Use this before targeting a slot.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "harness.send_terminal_keys",
            description: "Send keystrokes to a terminal slot in the active workspace. Address the slot by either `slotId` (preferred — get it from `harness.list_terminals`) or `agentSlug` (first slot matching that CLI agent). Set `submit:true` to append a newline so the command is executed. Use this to drive a running `claude`/`codex`/`gemini`/`opencode`/`cursor` CLI: ask it questions, send `/status`, paste prompts, etc.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "slotId":    { "type": "integer", "minimum": 1 },
                    "agentSlug": { "type": "string", "enum": ["claude", "codex", "gemini", "opencode", "cursor"] },
                    "text":      { "type": "string", "description": "Raw text to type into the terminal." },
                    "submit":    { "type": "boolean", "description": "Append a carriage return after the text. Default false." }
                },
                "required": ["text"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "harness.read_terminal_output",
            description: "Read recent output from a terminal slot non-destructively (does not steal bytes from the user's view). Returns the last `maxBytes` of the slot's rolling tail buffer (cap 64 KiB). Use this AFTER `harness.send_terminal_keys` to see how the CLI agent responded.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "slotId":    { "type": "integer", "minimum": 1 },
                    "agentSlug": { "type": "string", "enum": ["claude", "codex", "gemini", "opencode", "cursor"] },
                    "maxBytes":  { "type": "integer", "minimum": 1, "maximum": 65536, "default": 4096 }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "harness.open_terminal",
            description: "Open a new terminal slot in the active workspace. Optionally launches a CLI agent (`claude`, `codex`, `gemini`, `opencode`, `cursor`). When omitted, a plain shell is used.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "agentSlug": {
                        "type": "string",
                        "enum": ["claude", "codex", "gemini", "opencode", "cursor"],
                        "description": "CLI agent to auto-launch in the new slot."
                    }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
    ]
}

/// Find a tool definition by name. Tool names from the model are matched
/// case-sensitively to keep parity with the schema we sent.
pub fn find(name: &str) -> Option<&'static ToolDef> {
    // SAFETY: the registry is built fresh each call but returned slots live for
    // the duration of this lookup only. We Box::leak a single instance so the
    // 'static lifetime is honoured at zero risk (small one-time leak).
    static INIT: std::sync::OnceLock<Vec<ToolDef>> = std::sync::OnceLock::new();
    let reg = INIT.get_or_init(registry);
    reg.iter().find(|t| t.name == name)
}

/// Render every tool in the registry as an OpenAI Chat Completions `tools[]` entry.
pub fn render_for_openai() -> Value {
    let reg = registry();
    let items: Vec<Value> = reg
        .into_iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        })
        .collect();
    Value::Array(items)
}

/// Execute a server-tool synchronously. Returns the textual `content`
/// that should both be reported to the UI and fed back to the LLM.
pub fn execute_server_tool(
    name: &str,
    args: &Value,
    root: Option<&WorkspaceRootGuard>,
) -> ToolOutcome {
    match name {
        "read_workspace_file" => tool_read_workspace_file(args, root),
        "memory_list" => tool_memory_list(root),
        "memory_read" => tool_memory_read(args, root),
        "memory_search" => tool_memory_search(args, root),
        "memory_create" => tool_memory_create(args, root),
        "memory_write" => tool_memory_write(args, root),
        other => ToolOutcome {
            ok: false,
            content: format!("unknown server tool: {other}"),
        },
    }
}

fn workspace_string(root: Option<&WorkspaceRootGuard>) -> Result<String, ToolOutcome> {
    root.map(|g| g.as_str()).ok_or(ToolOutcome {
        ok: false,
        content: "no workspace configured".into(),
    })
}

fn tool_read_workspace_file(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing path".into(),
        };
    };
    match ScopedReadOps::read_text(root, path) {
        Ok(text) => {
            let trimmed = truncate_chars(&text, 4000);
            ToolOutcome {
                ok: true,
                content: trimmed,
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: e.to_string(),
        },
    }
}

fn tool_memory_list(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_list(ws) {
        Ok(mut notes) => {
            notes.truncate(200);
            let body = serde_json::to_string(&notes).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
            ToolOutcome {
                ok: true,
                content: body,
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_memory_read(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing path".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_read(ws, path.to_owned()) {
        Ok(note) => {
            let body = truncate_chars(&note.content, 4000);
            ToolOutcome {
                ok: true,
                content: body,
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_memory_search(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(query) = args.get("query").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing query".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_search(ws, query.to_owned()) {
        Ok(mut hits) => {
            hits.truncate(50);
            let body = serde_json::to_string(&hits).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
            ToolOutcome {
                ok: true,
                content: body,
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

const MEMORY_WRITE_MAX_BYTES: usize = 32 * 1024;

fn tool_memory_create(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing path".into(),
        };
    };
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    if content.len() > MEMORY_WRITE_MAX_BYTES {
        return ToolOutcome {
            ok: false,
            content: format!(
                "content exceeds {MEMORY_WRITE_MAX_BYTES} byte limit ({} bytes)",
                content.len()
            ),
        };
    }
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_create(ws, path.to_owned(), Some(content)) {
        Ok(meta) => {
            let body = serde_json::to_string(&meta).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
            ToolOutcome {
                ok: true,
                content: body,
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_memory_write(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing path".into(),
        };
    };
    let Some(content) = args.get("content").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing content".into(),
        };
    };
    if content.len() > MEMORY_WRITE_MAX_BYTES {
        return ToolOutcome {
            ok: false,
            content: format!(
                "content exceeds {MEMORY_WRITE_MAX_BYTES} byte limit ({} bytes)",
                content.len()
            ),
        };
    }
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_write(ws, path.to_owned(), content.to_owned()) {
        Ok(note) => ToolOutcome {
            ok: true,
            content: format!("wrote {} ({} bytes)", note.path, note.content.len()),
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}… (truncated)")
}
