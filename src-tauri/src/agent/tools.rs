//! Tool registry + sandboxed read-only file access.
//!
//! Each tool advertises a JSON schema and a server/client execution kind.
//! The orchestrator renders the registry for the provider, then dispatches
//! incoming tool calls back through here.

use crate::memory;
use crate::plans;
use crate::skills_rules::{self, types::SkillSourceInput};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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

/// JSON array of `{ name, description, site, parameters }` for every registered tool.
pub fn catalog_json() -> Result<String, String> {
    let items: Vec<Value> = registry()
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "site": t.site,
                "parameters": t.parameters,
            })
        })
        .collect();
    serde_json::to_string(&items).map_err(|e| format!("serialize tool catalog: {e}"))
}

pub fn registry() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_tools",
            description: "Return a JSON array of every BLXCode Agent tool (name, description, server|client site, and parameters JSON Schema). Call when unsure which tools exist or what arguments they accept.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
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
            description: "List Markdown notes under <workspace>/.agents/memory/ and learnings under .agents/learnings/ (API paths: learnings/…). Returns at most 200 entries.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_read",
            description: "Read a Markdown note (API path: `notes.md` or `learnings/topic.md`). Must end in .md.",
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
            description: "Create a new Markdown note under .agents/memory/ or learnings via `learnings/…` path. Must end in .md, must not exist. Content capped at 32 KiB.",
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
            description: "Overwrite an existing Markdown note (memory or `learnings/…` path). Must end in .md. Content capped at 32 KiB.",
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
            description: "Full-text search across .agents/memory/ and .agents/learnings/. Returns up to 50 hits (path, line, snippet).",
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
            name: "memory_delete",
            description: "Delete one Markdown note (memory or `learnings/…` API path). Removes empty parent folders under the note root.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "API path ending in .md." }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_rename",
            description: "Rename or move a note by changing its API path (same memory/learnings root only). Optionally rewrite `[[wikilinks]]` in other notes that pointed at the old path.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "oldPath": { "type": "string" },
                    "newPath": { "type": "string" },
                    "rewriteLinks": { "type": "boolean", "default": true }
                },
                "required": ["oldPath", "newPath"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_graph",
            description: "Build Obsidian-style graph data (nodes, edges, tags) across memory and learnings. Use to inspect link structure before editing notes.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "memory_backlinks",
            description: "List API paths of notes that link to the given note (wikilinks / graph edges).",
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
            name: "memory_category_list",
            description: "List Memory sidebar categories (`memory`, `learnings`) with display label, color, and sidebar/graph visibility for the active workspace.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "memory_category_update",
            description: "Update display settings for a Memory category (`memory` or `learnings`). Omitted fields stay unchanged. Color must be `#rrggbb`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "enum": ["memory", "learnings"] },
                    "label": { "type": "string" },
                    "color": { "type": "string", "description": "Hex color, e.g. #7dd3fc" },
                    "showInSidebar": { "type": "boolean" },
                    "showInGraph": { "type": "boolean" }
                },
                "required": ["category"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "memory_context_list",
            description: "List Memory/Learnings context items attached to the BLXCode Agent for the active workspace (sent with upcoming turns).",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "memory_context_attach",
            description: "Attach a Memory category or note to BLXCode Agent context. Categories auto-include all note paths in that root.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["memory_category", "learning_category", "memory_note", "learning_note"]
                    },
                    "path": { "type": "string", "description": "Required for note kinds; API path ending in .md." },
                    "label": { "type": "string", "description": "Optional display label; defaults from path or category." }
                },
                "required": ["kind"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "memory_context_detach",
            description: "Remove one attached context item by id (from `memory_context_list`).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "plan_list",
            description: "List Markdown plans in `<workspace>/.agents/plans/`. Each entry has `path`, `name`, `title`, `size`, `modified`, `isIndex`, and `taskSummary` ({ total, pending, inProgress, blocked, completed, cancelled }). Call before guessing about plans.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "plan_read",
            description: "Read the Markdown body of one plan in `.agents/plans/` (path ends in `.md`). Returns `{ path, content, modified, isIndex }`. Output is truncated at 6000 chars.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Plan path relative to `.agents/plans/`." }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "plan_create",
            description: "Create a new plan under `.agents/plans/`. Path must end in `.md` and not exist. If `content` is omitted, the plan is seeded with `# <name>` and an empty `## Tasks` section. Content capped at 64 KiB.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path":    { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "plan_write",
            description: "Overwrite an existing plan Markdown file. Content capped at 64 KiB. Use `plan_sync_from_tasks` if you just want to update the task section.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path":    { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "plan_delete",
            description: "Delete a plan Markdown file (cannot delete `PLANS.md`).",
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
            name: "plan_rename",
            description: "Rename or move a plan within `.agents/plans/`. Cannot rename `PLANS.md`. Task records pointing at the old path are rewritten to the new path.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "oldPath": { "type": "string" },
                    "newPath": { "type": "string" }
                },
                "required": ["oldPath", "newPath"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "plan_load",
            description: "Parse a plan's `## Tasks` (or `## Todos`) section and load it into the workspace task manager. Replaces only tasks where `planPath == path`; free tasks stay untouched. Sets the snapshot's `activePlanPath` to this plan. Call after writing a plan or whenever you want to act from a plan.",
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
            name: "plan_sync_from_tasks",
            description: "Write the current state of plan-linked tasks back into the plan Markdown's `## Tasks` section. Use after re-ordering or batch-status-changing plan tasks via `task_*` tools.",
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
            name: "plan_context_list",
            description: "List plans currently attached to BLXCode Agent context for the active workspace.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "plan_context_attach",
            description: "Attach a plan file to BLXCode Agent context (kind `plan_file`). Use `plan_load` first if you also want the plan's tasks in the task manager.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path":  { "type": "string", "description": "Plan path relative to `.agents/plans/`." },
                    "label": { "type": "string" }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "plan_context_detach",
            description: "Remove one attached plan context item by id (from `plan_context_list`).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "image_context_list",
            description: "List image context items attached to the active BLXCode Agent workspace. Pending images are sent with the next turn; read images are visible but not sent again unless the user reactivates them in the UI.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Client,
        },
        ToolDef {
            name: "image_context_detach",
            description: "Remove one attached image context item by id.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            site: ToolSite::Client,
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
            name: "rules_list",
            description: "List `.agents/rules/*.md` files for the active workspace. Each entry has `name`, `title`, `summary`, `enabled`, `sizeBytes`, `updatedAt`. Disabled rules are returned too — ignore them when shaping behaviour.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "rules_read",
            description: "Read the markdown body of one rule under `.agents/rules/`. `name` must look like `rule-foo.md`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Filename, e.g. `rule-foo.md`." }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "rules_write",
            description: "Create or overwrite one rule file under `.agents/rules/`. `name` MUST start with `rule-` and end with `.md`. New files default to enabled.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name":    { "type": "string", "description": "Filename, e.g. `rule-foo.md`." },
                    "content": { "type": "string" }
                },
                "required": ["name", "content"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "rules_set_enabled",
            description: "Toggle a rule's `enabled` flag in `.agents/rules/index.json`. Disabled rules are kept on disk but should be treated as inactive.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name":    { "type": "string" },
                    "enabled": { "type": "boolean" }
                },
                "required": ["name", "enabled"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "rules_remove",
            description: "Delete a rule file under `.agents/rules/` and clear its `index.json` entry.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "skills_list",
            description: "List `.agents/skills/<name>/` skills for the active workspace. Each entry has `name`, `title`, `summary`, `enabled`, `source` ({kind, url?, ref?, package?, version?, path?}), `installedAt`, `updatedAt`, and `missingSkillMd` when `SKILL.md` is absent.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "skills_read",
            description: "Read the `SKILL.md` body of one skill under `.agents/skills/<name>/`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "skills_write",
            description: "Create or overwrite `SKILL.md` for a skill (creates the folder if needed). Marks the skill as `agent-created` in the manifest.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name":    { "type": "string", "description": "Skill folder name (lowercase a-z 0-9 - _)." },
                    "content": { "type": "string" }
                },
                "required": ["name", "content"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "skills_set_enabled",
            description: "Toggle a skill's `enabled` flag in `.agents/skills/index.json`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name":    { "type": "string" },
                    "enabled": { "type": "boolean" }
                },
                "required": ["name", "enabled"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "skills_remove",
            description: "Delete a skill folder under `.agents/skills/<name>/` and clear its `index.json` entry.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" }
                },
                "required": ["name"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "skills_install",
            description: "Install a new skill into `.agents/skills/<name>/`. Source kinds: `git` (clone via https/ssh url + optional ref), `npm` (npm pack of package@version), or `local` (copy from a workspace-relative path). The source folder MUST contain `SKILL.md` at the top level. Only call when the user explicitly asks to install a skill — confirm name + source back to the user.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "name":   { "type": "string", "description": "Lowercase a-z, 0-9, dash, underscore. Must not already exist." },
                    "source": {
                        "type": "object",
                        "properties": {
                            "kind":    { "type": "string", "enum": ["git", "npm", "local"] },
                            "url":     { "type": "string", "description": "Required for kind=git." },
                            "ref":     { "type": "string", "description": "Optional git ref (default `main`)." },
                            "package": { "type": "string", "description": "Required for kind=npm." },
                            "version": { "type": "string", "description": "Optional npm version." },
                            "path":    { "type": "string", "description": "Required for kind=local; workspace-relative." }
                        },
                        "required": ["kind"],
                        "additionalProperties": false
                    }
                },
                "required": ["name", "source"],
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
            name: "harness.send_agent_context",
            description: "Hand off the current BLXCode Agent context to a terminal CLI session (claude/codex/gemini/opencode/cursor). Renders a Markdown context block (workspace root, attached Memory/Learnings/Rules, attached plans + plan-linked tasks, image metadata, optional instruction), exports any selected images to `<workspace>/.blxcode/agent-context/images/`, and writes the block into the terminal's PTY. Address the slot by `slotId` (preferred) or unique `agentSlug`. Set `submit:false` to leave the block at the prompt without pressing Enter. `includeKinds` defaults to `[\"memory\", \"plans\", \"tasks\", \"images\"]`. Image base64 is never written into the prompt.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "slotId":      { "type": "integer", "minimum": 1, "description": "Preferred. Target terminal slot id from `harness.list_terminals`." },
                    "agentSlug":   { "type": "string", "enum": ["claude", "codex", "gemini", "opencode", "cursor"], "description": "Fallback when `slotId` is omitted; resolves only if exactly one running slot matches." },
                    "instruction": { "type": "string", "description": "Optional task or instruction appended after the rendered context block." },
                    "includeKinds": {
                        "type": "array",
                        "items": { "type": "string", "enum": ["memory", "plans", "tasks", "images"] },
                        "description": "Which kinds of attached context to include. Defaults to all four."
                    },
                    "submit":      { "type": "boolean", "description": "Append a carriage return after the block. Default true." }
                },
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
            description: "Open one or more new terminal slots in the active workspace. Call with no arguments (`{}`) for a single plain shell — the default. Set `count` to open multiple terminals in one call (default 1, max 16). Use `agentSlug` for a single CLI agent applied to all opened slots, or `agentSlugs` (array, length == count) to assign different agents per slot. Only set agent slugs when the user explicitly names one of: `claude`, `codex`, `gemini`, `opencode`, `cursor`.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "count": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 16,
                        "default": 1,
                        "description": "Number of terminal slots to open. Defaults to 1."
                    },
                    "agentSlug": {
                        "type": "string",
                        "enum": ["claude", "codex", "gemini", "opencode", "cursor"],
                        "description": "OPTIONAL. Applied to every new slot. Omit for plain shells."
                    },
                    "agentSlugs": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["claude", "codex", "gemini", "opencode", "cursor"]
                        },
                        "description": "OPTIONAL. Per-slot agent slugs. Length must equal `count`. Takes precedence over `agentSlug`."
                    }
                },
                "required": [],
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

/// Render every tool in the registry as an Anthropic Messages API `tools[]` entry.
/// Anthropic uses `input_schema` (vs. OpenAI's `parameters`) and a flat shape.
pub fn render_for_anthropic() -> Value {
    let reg = registry();
    let items: Vec<Value> = reg
        .into_iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters,
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
        "list_tools" => tool_list_tools(),
        "read_workspace_file" => tool_read_workspace_file(args, root),
        "list_workspace_files" => tool_list_workspace_files(args, root),
        "memory_list" => tool_memory_list(root),
        "memory_read" => tool_memory_read(args, root),
        "memory_search" => tool_memory_search(args, root),
        "memory_create" => tool_memory_create(args, root),
        "memory_write" => tool_memory_write(args, root),
        "memory_delete" => tool_memory_delete(args, root),
        "memory_rename" => tool_memory_rename(args, root),
        "memory_graph" => tool_memory_graph(root),
        "memory_backlinks" => tool_memory_backlinks(args, root),
        "task_list" => tool_task_list(args, root),
        "task_get" => tool_task_get(args, root),
        "task_create" => tool_task_create(args, root),
        "task_update" => tool_task_update(args, root),
        "task_delete" => tool_task_delete(args, root),
        "task_reorder" => tool_task_reorder(args, root),
        "plan_list" => tool_plan_list(root),
        "plan_read" => tool_plan_read(args, root),
        "plan_create" => tool_plan_create(args, root),
        "plan_write" => tool_plan_write(args, root),
        "plan_delete" => tool_plan_delete(args, root),
        "plan_rename" => tool_plan_rename(args, root),
        "plan_load" => tool_plan_load(args, root),
        "plan_sync_from_tasks" => tool_plan_sync_from_tasks(args, root),
        "rules_list" => tool_rules_list(root),
        "rules_read" => tool_rules_read(args, root),
        "rules_write" => tool_rules_write(args, root),
        "rules_set_enabled" => tool_rules_set_enabled(args, root),
        "rules_remove" => tool_rules_remove(args, root),
        "skills_list" => tool_skills_list(root),
        "skills_read" => tool_skills_read(args, root),
        "skills_write" => tool_skills_write(args, root),
        "skills_set_enabled" => tool_skills_set_enabled(args, root),
        "skills_remove" => tool_skills_remove(args, root),
        "skills_install" => tool_skills_install(args, root),
        other => ToolOutcome {
            ok: false,
            content: format!("unknown server tool: {other}"),
        },
    }
}

// ---------------------------------------------------------------------
// Skills & Rules dispatch

fn need_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolOutcome> {
    args.get(key).and_then(|v| v.as_str()).ok_or(ToolOutcome {
        ok: false,
        content: format!("missing {key}"),
    })
}

fn need_bool(args: &Value, key: &str) -> Result<bool, ToolOutcome> {
    args.get(key).and_then(|v| v.as_bool()).ok_or(ToolOutcome {
        ok: false,
        content: format!("missing {key}"),
    })
}

fn json_outcome<T: Serialize>(value: &T) -> ToolOutcome {
    match serde_json::to_string(value) {
        Ok(body) => ToolOutcome {
            ok: true,
            content: body,
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: format!("serialize: {e}"),
        },
    }
}

fn err_outcome(e: String) -> ToolOutcome {
    ToolOutcome {
        ok: false,
        content: e,
    }
}

fn tool_rules_list(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::list_rules(&ws) {
        Ok(entries) => json_outcome(&entries),
        Err(e) => err_outcome(e),
    }
}

fn tool_rules_read(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::read_rule(&ws, name) {
        Ok(body) => ToolOutcome {
            ok: true,
            content: truncate_chars(&body, 4000),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_rules_write(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let content = match need_str(args, "content") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::write_rule(&ws, name, content) {
        Ok(entry) => json_outcome(&entry),
        Err(e) => err_outcome(e),
    }
}

fn tool_rules_set_enabled(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let enabled = match need_bool(args, "enabled") {
        Ok(b) => b,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::set_rule_enabled(&ws, name, enabled) {
        Ok(entry) => json_outcome(&entry),
        Err(e) => err_outcome(e),
    }
}

fn tool_rules_remove(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::remove_rule(&ws, name) {
        Ok(()) => ToolOutcome {
            ok: true,
            content: format!("removed rule {name}"),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_skills_list(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::list_skills(&ws) {
        Ok(entries) => json_outcome(&entries),
        Err(e) => err_outcome(e),
    }
}

fn tool_skills_read(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::read_skill(&ws, name) {
        Ok(body) => ToolOutcome {
            ok: true,
            content: truncate_chars(&body, 4000),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_skills_write(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let content = match need_str(args, "content") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::write_skill(&ws, name, content) {
        Ok(entry) => json_outcome(&entry),
        Err(e) => err_outcome(e),
    }
}

fn tool_skills_set_enabled(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let enabled = match need_bool(args, "enabled") {
        Ok(b) => b,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::set_skill_enabled(&ws, name, enabled) {
        Ok(entry) => json_outcome(&entry),
        Err(e) => err_outcome(e),
    }
}

fn tool_skills_remove(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::store::remove_skill(&ws, name) {
        Ok(()) => ToolOutcome {
            ok: true,
            content: format!("removed skill {name}"),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_skills_install(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let name = match need_str(args, "name") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let Some(source_val) = args.get("source") else {
        return ToolOutcome {
            ok: false,
            content: "missing source".into(),
        };
    };
    let source: SkillSourceInput = match serde_json::from_value(source_val.clone()) {
        Ok(v) => v,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: format!("invalid source: {e}"),
            };
        }
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match skills_rules::install::install_skill(&ws, name, source) {
        Ok(entry) => json_outcome(&entry),
        Err(e) => err_outcome(e),
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

fn tool_list_tools() -> ToolOutcome {
    match catalog_json() {
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

fn tool_memory_delete(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
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
    match memory::memory_delete(ws, path.to_owned()) {
        Ok(()) => ToolOutcome {
            ok: true,
            content: format!("deleted {path}"),
        },
        Err(e) => ToolOutcome {
            ok: false,
            content: e,
        },
    }
}

fn tool_memory_rename(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let Some(old_path) = args.get("oldPath").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing oldPath".into(),
        };
    };
    let Some(new_path) = args.get("newPath").and_then(|v| v.as_str()) else {
        return ToolOutcome {
            ok: false,
            content: "missing newPath".into(),
        };
    };
    let rewrite_links = args
        .get("rewriteLinks")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_rename(ws, old_path.to_owned(), new_path.to_owned(), rewrite_links) {
        Ok(report) => {
            let body =
                serde_json::to_string(&report).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
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

fn tool_memory_graph(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match memory::memory_graph(ws) {
        Ok(graph) => {
            let body =
                serde_json::to_string(&graph).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
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

fn tool_memory_backlinks(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
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
    match memory::memory_backlinks(ws, path.to_owned()) {
        Ok(links) => {
            let body =
                serde_json::to_string(&links).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
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

const PLAN_WRITE_MAX_BYTES: usize = 64 * 1024;

fn tool_plan_list(root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_list_inner(&ws) {
        Ok(entries) => json_outcome(&entries),
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_read(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = match need_str(args, "path") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_read_inner(&ws, path) {
        Ok(content) => ToolOutcome {
            ok: true,
            content: truncate_chars(&content.content, 6000),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_create(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = match need_str(args, "path") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let content = args.get("content").and_then(|v| v.as_str());
    if let Some(c) = content {
        if c.len() > PLAN_WRITE_MAX_BYTES {
            return ToolOutcome {
                ok: false,
                content: format!(
                    "content exceeds {PLAN_WRITE_MAX_BYTES} byte limit ({} bytes)",
                    c.len()
                ),
            };
        }
    }
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_create_inner(&ws, path, content) {
        Ok(meta) => json_outcome(&meta),
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_write(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = match need_str(args, "path") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let content = match need_str(args, "content") {
        Ok(s) => s,
        Err(o) => return o,
    };
    if content.len() > PLAN_WRITE_MAX_BYTES {
        return ToolOutcome {
            ok: false,
            content: format!(
                "content exceeds {PLAN_WRITE_MAX_BYTES} byte limit ({} bytes)",
                content.len()
            ),
        };
    }
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_write_inner(&ws, path, content) {
        Ok(c) => ToolOutcome {
            ok: true,
            content: format!("wrote {} ({} bytes)", c.path, c.content.len()),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_delete(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = match need_str(args, "path") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_delete_inner(&ws, path) {
        Ok(()) => ToolOutcome {
            ok: true,
            content: format!("deleted plan {path}"),
        },
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_rename(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let old_path = match need_str(args, "oldPath") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let new_path = match need_str(args, "newPath") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_rename_inner(&ws, old_path, new_path) {
        Ok(meta) => json_outcome(&meta),
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_load(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = match need_str(args, "path") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_load_inner(&ws, path) {
        Ok(report) => json_outcome(&report),
        Err(e) => err_outcome(e),
    }
}

fn tool_plan_sync_from_tasks(args: &Value, root: Option<&WorkspaceRootGuard>) -> ToolOutcome {
    let path = match need_str(args, "path") {
        Ok(s) => s,
        Err(o) => return o,
    };
    let ws = match workspace_string(root) {
        Ok(s) => s,
        Err(out) => return out,
    };
    match plans::plan_sync_from_tasks_inner(&ws, path) {
        Ok(report) => json_outcome(&report),
        Err(e) => err_outcome(e),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_memory_management_tools() {
        let names: Vec<_> = registry().iter().map(|t| t.name).collect();
        for expected in [
            "list_tools",
            "memory_delete",
            "memory_rename",
            "memory_graph",
            "memory_backlinks",
            "memory_category_list",
            "memory_category_update",
            "memory_context_list",
            "memory_context_attach",
            "memory_context_detach",
            "image_context_list",
            "image_context_detach",
            "plan_list",
            "plan_read",
            "plan_create",
            "plan_write",
            "plan_delete",
            "plan_rename",
            "plan_load",
            "plan_sync_from_tasks",
            "plan_context_list",
            "plan_context_attach",
            "plan_context_detach",
        ] {
            assert!(names.contains(&expected), "missing tool {expected}");
        }
    }

    #[test]
    fn list_tools_returns_valid_json() {
        let body = catalog_json().expect("catalog json");
        let parsed: Vec<Value> = serde_json::from_str(&body).expect("parse catalog");
        assert!(!parsed.is_empty());
        assert!(parsed.iter().all(|entry| entry.get("name").is_some()));
    }
}
