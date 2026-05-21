//! Reine Datenstrukturen für die Agent-Chat-Timeline (serde-fähig, ohne Leptos).
//! Wird von [`crate::workbench::state::WorkspaceEntry`] und [`agent_panel::timeline`] genutzt.

use crate::i18n::{lookup, I18nKey, Locale};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    Pending,
    Ok,
    Fail,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolActivity {
    pub tool: String,
    pub label: String,
    pub args_summary: String,
    pub status: ActivityStatus,
    pub detail: Option<String>,
}

impl ToolActivity {
    pub fn from_call(tool: &str, args: Option<&Value>, loc: Locale) -> Self {
        Self {
            tool: tool.to_owned(),
            label: tool_label(tool, loc),
            args_summary: summarize_args(tool, args),
            status: ActivityStatus::Pending,
            detail: None,
        }
    }
}

/// Lokalisiertes Kurzlabel für einen Tool-Namen in der Timeline.
#[must_use]
pub fn tool_label(tool: &str, loc: Locale) -> String {
    let key = match tool {
        "environment_detect" => Some(I18nKey::AgToolEnvironmentDetect),
        "shell_exec" => Some(I18nKey::AgToolShellExec),
        "workspace_search" => Some(I18nKey::AgToolWorkspaceSearch),
        "workspace_git_status" => Some(I18nKey::AgToolWorkspaceGitStatus),
        "workspace_diff" => Some(I18nKey::AgToolWorkspaceDiff),
        "git_status" => Some(I18nKey::AgToolGitStatus),
        "git_diff" => Some(I18nKey::AgToolGitDiff),
        "git_log" => Some(I18nKey::AgToolGitLog),
        "git_show" => Some(I18nKey::AgToolGitShow),
        "git_branch_info" => Some(I18nKey::AgToolGitBranch),
        "git_ls_files" => Some(I18nKey::AgToolGitLsFiles),
        "git_apply_patch" => Some(I18nKey::AgToolGitApplyPatch),
        "git_add" => Some(I18nKey::AgToolGitAdd),
        "git_commit" => Some(I18nKey::AgToolGitCommit),
        "web_search" => Some(I18nKey::AgToolWebSearch),
        "web_fetch" => Some(I18nKey::AgToolWebFetch),
        "subagents.run" => Some(I18nKey::AgToolSubagentsRun),
        "submit_result" => Some(I18nKey::AgToolSubmitResult),
        _ => None,
    };
    if let Some(k) = key {
        return lookup(loc, k).to_string();
    }
    legacy_tool_label(tool)
}

fn legacy_tool_label(tool: &str) -> String {
    match tool {
        "harness.create_workspace" => "Create workspace",
        "list_workspace_files" => "List files",
        "read_workspace_file" => "Read file",
        "memory_list" => "List memory notes",
        "memory_read" => "Read memory note",
        "memory_search" => "Search memory",
        "memory_create" => "Create memory note",
        "memory_write" => "Update memory note",
        "memory_delete" => "Delete memory note",
        "memory_rename" => "Rename memory note",
        "memory_graph" => "Memory graph",
        "memory_backlinks" => "Memory backlinks",
        "memory_category_list" => "List memory categories",
        "memory_category_update" => "Update memory category",
        "memory_context_list" => "List agent context",
        "memory_context_attach" => "Attach memory context",
        "memory_context_detach" => "Detach memory context",
        "list_tools" => "List tools",
        "task_list" => "List tasks",
        "task_get" => "Read task",
        "task_create" => "Create task",
        "task_update" => "Update task",
        "task_delete" => "Delete task",
        "task_reorder" => "Reorder tasks",
        "harness.open_terminal" => "Open terminal",
        "harness.list_terminals" => "List terminals",
        "harness.send_terminal_keys" => "Send keys to terminal",
        "harness.send_agent_context" => "Send agent context to terminal",
        "harness.read_terminal_output" => "Read terminal output",
        other => return other.to_string(),
    }
    .to_string()
}

/// Lokalisiertes Subagent-Status-Label (`running`, `completed`, …).
#[must_use]
pub fn subagent_status_label(loc: Locale, status: &str) -> String {
    let key = match status {
        "running" => I18nKey::AgSubagentStatusRunning,
        "completed" => I18nKey::AgSubagentStatusCompleted,
        "blocked" => I18nKey::AgSubagentStatusBlocked,
        "failed" => I18nKey::AgSubagentStatusFailed,
        _ => return status.to_string(),
    };
    lookup(loc, key).to_string()
}

/// Lokalisiertes Rollen-Label (`scout`, `review`, …).
#[must_use]
pub fn subagent_role_label(loc: Locale, role: &str) -> String {
    let key = match role {
        "scout" => Some(I18nKey::AgRoleScout),
        "review" => Some(I18nKey::AgRoleReview),
        "security_analyst" => Some(I18nKey::AgRoleSecurityAnalyst),
        _ => None,
    };
    key.map(|k| lookup(loc, k).to_string())
        .unwrap_or_else(|| role.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentStepRow {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentCard {
    pub agent_id: String,
    pub role: String,
    pub display_name: String,
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<SubagentStepRow>,
    #[serde(default)]
    pub tools: Vec<ToolActivity>,
    /// Live assistant text streamed from the subagent. Cleared when the
    /// subagent finishes and the `summary` takes over.
    #[serde(default)]
    pub live_text: String,
    /// Live reasoning text streamed from the subagent. Rendered as a
    /// collapsed "thinking" block.
    #[serde(default)]
    pub live_thinking: String,
    /// When true, the model's current thinking burst is complete and the
    /// thinking block can be collapsed in the UI.
    #[serde(default)]
    pub thinking_done: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentGroup {
    #[serde(default)]
    pub agents: Vec<SubagentCard>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineItem {
    User { text: String },
    Assistant { text: String },
    Tool(ToolActivity),
    Thinking { text: String, done: bool },
    SubagentGroup(SubagentGroup),
    /// Output of an image-mode turn. `preview_src` is a data URL suitable
    /// for `<img src>`; after a workspace reload we hydrate it lazily from
    /// `saved_path` via the `generated_image_preview` Tauri command.
    GeneratedImage {
        prompt: String,
        mime: String,
        preview_src: String,
        saved_path: Option<String>,
        filename: Option<String>,
    },
}

fn summarize_args(tool: &str, args: Option<&Value>) -> String {
    let Some(args) = args else {
        return String::new();
    };
    let pick = match tool {
        "harness.create_workspace" => Some("title"),
        "list_workspace_files" => Some("path"),
        "read_workspace_file" | "memory_read" | "memory_create" | "memory_write" | "memory_delete" | "memory_backlinks" => Some("path"),
        "memory_rename" => Some("newPath"),
        "memory_search" => Some("query"),
        "memory_category_update" => Some("category"),
        "memory_context_attach" => Some("kind"),
        "memory_context_detach" => Some("id"),
        "task_get" | "task_update" | "task_delete" => Some("id"),
        "task_create" => Some("title"),
        "harness.open_terminal" => Some("agentSlug"),
        "harness.send_terminal_keys" => Some("text"),
        "harness.send_agent_context" => Some("instruction"),
        "workspace_search" | "web_search" => Some("query"),
        "shell_exec" => Some("command"),
        "git_show" => Some("rev"),
        "git_commit" => Some("message"),
        "web_fetch" => Some("url"),
        _ => None,
    };
    if let Some(key) = pick {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            return v.to_owned();
        }
    }
    if tool == "task_reorder" {
        if let Some(ids) = args.get("orderedIds").and_then(|v| v.as_array()) {
            return format!("{} ids", ids.len());
        }
    }
    String::new()
}
