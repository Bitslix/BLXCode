//! Tool registry + sandboxed read-only file access.
//!
//! Each tool advertises a JSON schema and a server/client execution kind.
//! The orchestrator renders the registry for the provider, then dispatches
//! incoming tool calls back through here.

use crate::memory;
use crate::tasks::{
    self, TaskCreateInput, TaskReorderInput, TaskSnapshot, TaskStatus, TaskUpdatePatch,
};
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntryMeta {
    pub path: String,
    pub kind: String,
}

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

    pub fn list_entries(
        root: Option<&WorkspaceRootGuard>,
        relative: Option<&str>,
        recursive: bool,
        max_entries: usize,
    ) -> Result<Vec<WorkspaceEntryMeta>, ReadToolError> {
        let guard = root.ok_or(ReadToolError::NoWorkspace)?;
        let base = match relative {
            None | Some("") => guard.path.clone(),
            Some(rel) => {
                let rel = RelativePath::normalize(rel).ok_or(ReadToolError::InvalidPath)?;
                let full = guard.path.join(&rel);
                guard
                    .contains(&full)
                    .then_some(())
                    .ok_or(ReadToolError::PathEscape)?;
                full
            }
        };

        let meta = fs::metadata(&base).map_err(|e| ReadToolError::Io(e.to_string()))?;
        if !meta.is_dir() {
            return Err(ReadToolError::Io(format!(
                "path is not a directory: {}",
                base.display()
            )));
        }

        let mut out = Vec::new();
        collect_entries(&guard.path, &base, recursive, max_entries.max(1), &mut out)?;
        Ok(out)
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
            name: "list_workspace_files",
            description: "List files and directories under the workspace root or a relative subdirectory. Use this before `read_workspace_file` when you are not sure of the exact path or when exploring the codebase structure.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Optional relative directory within the workspace. Defaults to the workspace root." },
                    "recursive": { "type": "boolean", "default": false },
                    "maxEntries": { "type": "integer", "minimum": 1, "maximum": 500, "default": 100 }
                },
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
            name: "task_list",
            description: "List tracked tasks under <workspace>/.blxcode/tasks/. Returns a stable JSON snapshot sorted by task position. Optional filters: `status` and `includeCompleted`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "blocked", "completed", "cancelled"]
                    },
                    "includeCompleted": { "type": "boolean", "default": true }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "task_get",
            description: "Read one tracked task by id from <workspace>/.blxcode/tasks/.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "task_create",
            description: "Create a new tracked task under <workspace>/.blxcode/tasks/.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "blocked", "completed", "cancelled"]
                    },
                    "parentId": { "type": "string" },
                    "notes": { "type": "string" }
                },
                "required": ["title"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "task_update",
            description: "Update fields on an existing tracked task. Omitted fields stay unchanged.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "blocked", "completed", "cancelled"]
                    },
                    "parentId": {
                        "anyOf": [
                            { "type": "string" },
                            { "type": "null" }
                        ]
                    },
                    "notes": {
                        "anyOf": [
                            { "type": "string" },
                            { "type": "null" }
                        ]
                    }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "task_delete",
            description: "Delete a tracked task by id. Returns the post-delete task snapshot.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "task_reorder",
            description: "Rewrite task ordering using a complete list of task ids.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "orderedIds": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["orderedIds"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "harness.create_workspace",
            description: "Create and select a new workspace in the workbench. Defaults `cwd` to the active workspace cwd or the configured harness workspace root. `agentSlugs` maps one label per terminal slot and may contain `claude`, `codex`, `gemini`, `opencode`, `cursor`, or empty strings.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Workspace title shown in the sidebar." },
                    "cwd": { "type": "string", "description": "Workspace root path. Optional; defaults to current workspace cwd or harness root." },
                    "terminalCount": { "type": "integer", "minimum": 1, "maximum": 16, "default": 1 },
                    "agentSlugs": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["", "claude", "codex", "gemini", "opencode", "cursor"]
                        },
                        "description": "Optional per-slot CLI agent labels. If shorter than terminalCount, remaining slots stay plain."
                    }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Client,
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
        "list_workspace_files" => tool_list_workspace_files(args, root),
        "memory_list" => tool_memory_list(root),
        "memory_read" => tool_memory_read(args, root),
        "memory_search" => tool_memory_search(args, root),
        "memory_create" => tool_memory_create(args, root),
        "memory_write" => tool_memory_write(args, root),
        "task_list" => tool_task_list(args, root),
        "task_get" => tool_task_get(args, root),
        "task_create" => tool_task_create(args, root),
        "task_update" => tool_task_update(args, root),
        "task_delete" => tool_task_delete(args, root),
        "task_reorder" => tool_task_reorder(args, root),
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

fn tool_list_workspace_files(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = args.get("path").and_then(|v| v.as_str());
    let recursive = args
        .get("recursive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_entries = args
        .get("maxEntries")
        .and_then(|v| v.as_u64())
        .unwrap_or(100)
        .clamp(1, 500) as usize;
    match ScopedReadOps::list_entries(root, path, recursive, max_entries) {
        Ok(entries) => match serde_json::to_string(&entries) {
            Ok(body) => ToolOutcome {
                ok: true,
                content: body,
            },
            Err(e) => ToolOutcome {
                ok: false,
                content: format!("serialize workspace listing: {e}"),
            },
        },
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
            let body =
                serde_json::to_string(&notes).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
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
            let body =
                serde_json::to_string(&hits).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
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
            let body =
                serde_json::to_string(&meta).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
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

fn parse_task_status(raw: Option<&str>) -> Result<Option<TaskStatus>, String> {
    match raw {
        None => Ok(None),
        Some("pending") => Ok(Some(TaskStatus::Pending)),
        Some("in_progress") => Ok(Some(TaskStatus::InProgress)),
        Some("blocked") => Ok(Some(TaskStatus::Blocked)),
        Some("completed") => Ok(Some(TaskStatus::Completed)),
        Some("cancelled") => Ok(Some(TaskStatus::Cancelled)),
        Some(other) => Err(format!("invalid task status: {other}")),
    }
}

fn task_snapshot_json(snapshot: &TaskSnapshot) -> Result<String, String> {
    serde_json::to_string(snapshot).map_err(|e| format!("serialize task snapshot: {e}"))
}

fn task_json<T: Serialize>(value: &T, what: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| format!("serialize {what}: {e}"))
}

fn filter_snapshot(
    mut snapshot: TaskSnapshot,
    status_filter: Option<TaskStatus>,
    include_completed: bool,
) -> TaskSnapshot {
    snapshot.tasks.retain(|task| {
        let status_ok = status_filter
            .as_ref()
            .is_none_or(|wanted| task.status == *wanted);
        let completed_ok = include_completed || !matches!(task.status, TaskStatus::Completed);
        status_ok && completed_ok
    });
    if snapshot
        .active_task_id
        .as_ref()
        .is_some_and(|id| !snapshot.tasks.iter().any(|task| &task.id == id))
    {
        snapshot.active_task_id = None;
    }
    snapshot
}

fn tool_task_list(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    let status_filter = match parse_task_status(args.get("status").and_then(|v| v.as_str())) {
        Ok(v) => v,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: e,
            };
        }
    };
    let include_completed = args
        .get("includeCompleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    match tasks::tasks_snapshot(&ws) {
        Ok(snapshot) => {
            match task_snapshot_json(&filter_snapshot(snapshot, status_filter, include_completed)) {
                Ok(body) => ToolOutcome {
                    ok: true,
                    content: body,
                },
                Err(e) => ToolOutcome {
                    ok: false,
                    content: e,
                },
            }
        }
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_task_get(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(id) = args.get("id").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing id".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match tasks::tasks_get_inner(&ws, id) {
        Ok(task) => match task_json(&task, "task") {
            Ok(body) => ToolOutcome {
                ok: true,
                content: body,
            },
            Err(e) => ToolOutcome {
                ok: false,
                content: e,
            },
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_task_create(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(title) = args.get("title").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing title".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    let status = match parse_task_status(args.get("status").and_then(|v| v.as_str())) {
        Ok(v) => v,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: e,
            };
        }
    };
    let input = TaskCreateInput {
        title: title.to_owned(),
        description: args
            .get("description")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        status,
        parent_id: args
            .get("parentId")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        notes: args
            .get("notes")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
    };
    match tasks::tasks_create_inner(&ws, input) {
        Ok(task) => match task_json(&task, "task") {
            Ok(body) => ToolOutcome {
                ok: true,
                content: body,
            },
            Err(e) => ToolOutcome {
                ok: false,
                content: e,
            },
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_task_update(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(id) = args.get("id").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing id".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    let status = match parse_task_status(args.get("status").and_then(|v| v.as_str())) {
        Ok(v) => v,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: e,
            };
        }
    };
    let patch = TaskUpdatePatch {
        title: args
            .get("title")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        description: args
            .get("description")
            .and_then(|v| v.as_str())
            .map(ToOwned::to_owned),
        status,
        parent_id: args.get("parentId").map(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(ToOwned::to_owned)
            }
        }),
        notes: args.get("notes").map(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(ToOwned::to_owned)
            }
        }),
    };
    match tasks::tasks_update_inner(&ws, id, patch) {
        Ok(task) => match task_json(&task, "task") {
            Ok(body) => ToolOutcome {
                ok: true,
                content: body,
            },
            Err(e) => ToolOutcome {
                ok: false,
                content: e,
            },
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_task_delete(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(id) = args.get("id").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing id".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match tasks::tasks_delete_inner(&ws, id) {
        Ok(snapshot) => match task_snapshot_json(&snapshot) {
            Ok(body) => ToolOutcome {
                ok: true,
                content: body,
            },
            Err(e) => ToolOutcome {
                ok: false,
                content: e,
            },
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_task_reorder(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(ordered_ids) = args.get("orderedIds").and_then(|v| v.as_array()) else {
        return ToolOutcome {
            ok: false,
            content: "missing orderedIds".into(),
        };
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    let input = TaskReorderInput {
        ordered_ids: ordered_ids
            .iter()
            .filter_map(|v| v.as_str().map(ToOwned::to_owned))
            .collect(),
    };
    match tasks::tasks_reorder_inner(&ws, input) {
        Ok(snapshot) => match task_snapshot_json(&snapshot) {
            Ok(body) => ToolOutcome {
                ok: true,
                content: body,
            },
            Err(e) => ToolOutcome {
                ok: false,
                content: e,
            },
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

fn collect_entries(
    workspace_root: &Path,
    dir: &Path,
    recursive: bool,
    max_entries: usize,
    out: &mut Vec<WorkspaceEntryMeta>,
) -> Result<(), ReadToolError> {
    let read = fs::read_dir(dir).map_err(|e| ReadToolError::Io(e.to_string()))?;
    let mut entries: Vec<_> = read
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ReadToolError::Io(e.to_string()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        if out.len() >= max_entries {
            break;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(workspace_root)
            .map_err(|_| ReadToolError::PathEscape)?
            .to_string_lossy()
            .replace('\\', "/");
        let file_type = entry
            .file_type()
            .map_err(|e| ReadToolError::Io(e.to_string()))?;
        let kind = if file_type.is_dir() {
            "directory"
        } else {
            "file"
        };
        out.push(WorkspaceEntryMeta {
            path: rel.clone(),
            kind: kind.to_string(),
        });
        if recursive && file_type.is_dir() && out.len() < max_entries {
            collect_entries(workspace_root, &path, recursive, max_entries, out)?;
        }
    }
    Ok(())
}
