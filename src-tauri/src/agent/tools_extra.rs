//! Additional agent tools (environment, shell, git, workspace, subagents).

use crate::agent::tools::{ToolDef, ToolSite};
use serde_json::json;

pub fn extra_tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "environment_detect",
            description: "Detect OS, shells, git availability, and workspace root. Call before shell or git tools.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "shell_exec",
            description: "Run a non-interactive shell command in the workspace directory. Read-only unless shell_write group is allowed.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "writes": { "type": "boolean", "default": false }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "workspace_search",
            description: "Search file contents under the workspace using ripgrep (rg).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "maxResults": { "type": "integer", "minimum": 1, "maximum": 100, "default": 50 }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "workspace_git_status",
            description: "Git status at workspace root (short branch view).",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "workspace_diff",
            description: "Git diff at workspace root.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "staged": { "type": "boolean", "default": false }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_status",
            description: "Git status in workspace (optional cwd relative path).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "cwd": { "type": "string" }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_diff",
            description: "Git diff in workspace (optional cwd).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "cwd": { "type": "string" },
                    "staged": { "type": "boolean", "default": false }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_log",
            description: "Git log oneline (read-only).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_show",
            description: "Git show for a revision.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "rev": { "type": "string" }
                },
                "required": ["rev"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_branch_info",
            description: "List branches with tracking info.",
            parameters: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_ls_files",
            description: "List tracked files (optional path filter).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_apply_patch",
            description: "Apply a unified diff patch in the workspace.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "patch": { "type": "string" }
                },
                "required": ["patch"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_add",
            description: "Stage paths for commit.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "git_commit",
            description: "Create a git commit with message (no push).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "web_search",
            description: "Search the web (requires API key in settings).",
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
            name: "web_fetch",
            description: "Fetch a URL summary (requires API key in settings).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string" }
                },
                "required": ["url"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "subagents.run",
            description: "Run coordinated subagents in parallel when the user explicitly asked for subagents, parallel review, or role-specific analysis.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "agents": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "role": { "type": "string", "enum": ["scout", "review", "security_analyst"] },
                                "title": { "type": "string" },
                                "task": { "type": "string" },
                                "successCriteria": { "type": "array", "items": { "type": "string" } },
                                "allowedToolGroups": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["id", "role", "task"]
                        }
                    },
                    "mode": { "type": "string", "enum": ["parallel"] },
                    "maxConcurrency": { "type": "integer", "minimum": 1, "maximum": 5, "default": 3 }
                },
                "required": ["agents"],
                "additionalProperties": false
            }),
            site: ToolSite::Server,
        },
        ToolDef {
            name: "submit_result",
            description: "Subagent-only: submit structured final result and end the subagent run.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "status": { "type": "string", "enum": ["completed", "blocked", "failed"] },
                    "role": { "type": "string" },
                    "displayName": { "type": "string" },
                    "summary": { "type": "string" },
                    "steps": { "type": "array" },
                    "findings": { "type": "array" },
                    "artifacts": { "type": "array" },
                    "recommendedNextActions": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["status", "summary"],
                "additionalProperties": true
            }),
            site: ToolSite::Server,
        },
    ]
}
