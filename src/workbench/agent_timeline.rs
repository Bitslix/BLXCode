//! Reine Datenstrukturen für die Agent-Chat-Timeline (serde-fähig, ohne Leptos).
//! Wird von [`crate::workbench::state::WorkspaceEntry`] und [`agent_panel::timeline`] genutzt.

use crate::agent_wire::TurnMetrics;
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolActivity {
    pub tool: String,
    pub label: String,
    pub args_summary: String,
    pub status: ActivityStatus,
    pub detail: Option<String>,
    /// Provider-issued call id (echoed from `AgentEvent::ToolCall`). Used to
    /// correlate `TurnUsage { kind: ToolExec, .. }` events back to the
    /// matching row. Optional for legacy / mock events.
    #[serde(default)]
    pub call_id: Option<String>,
    /// Per-row metrics — populated by a matching `TurnUsage` event.
    #[serde(default)]
    pub metrics: TurnMetrics,
    /// Workspace-relative paths accessed by this tool call (file-reading tools
    /// only). Populated from tool args at call time; accumulates entries when
    /// consecutive same-tool calls are grouped in a ModelRound.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Number of individual tool calls merged into this row (1 = ungrouped).
    #[serde(default = "default_merged_count")]
    pub merged_count: usize,
}

fn default_merged_count() -> usize {
    1
}

impl ToolActivity {
    pub fn from_call(tool: &str, args: Option<&Value>, loc: Locale) -> Self {
        let paths = file_arg_path(tool, args).into_iter().collect();
        Self {
            tool: tool.to_owned(),
            label: tool_label(tool, loc),
            args_summary: summarize_args(tool, args),
            status: ActivityStatus::Pending,
            detail: None,
            call_id: None,
            metrics: TurnMetrics::default(),
            paths,
            merged_count: 1,
        }
    }

    pub fn from_call_with_id(
        tool: &str,
        args: Option<&Value>,
        loc: Locale,
        call_id: Option<String>,
    ) -> Self {
        let mut row = Self::from_call(tool, args, loc);
        row.call_id = call_id;
        row
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Aggregated metrics across the subagent's model rounds. Tool-execution
    /// metrics live per-row in `tools[..].metrics`.
    #[serde(default)]
    pub metrics: TurnMetrics,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubagentGroup {
    #[serde(default)]
    pub agents: Vec<SubagentCard>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AskUserOption {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskUserState {
    Open,
    Answered {
        selected: Vec<String>,
        #[serde(default)]
        other: Option<String>,
    },
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TimelineItem {
    User {
        text: String,
    },
    Assistant {
        text: String,
        /// Per-row metrics aggregated from `ModelRound` events whose visible
        /// output landed in this assistant block. Empty until the first
        /// `TurnUsage` arrives.
        #[serde(default)]
        metrics: TurnMetrics,
    },
    Tool(ToolActivity),
    Thinking {
        text: String,
        done: bool,
    },
    SubagentGroup(SubagentGroup),
    /// Synthetic row inserted for tool-only model rounds (no assistant
    /// text was emitted). Carries the round's metrics so cost / tokens
    /// still surface to the operator instead of vanishing under the tools.
    ModelDecision {
        #[serde(default)]
        metrics: TurnMetrics,
    },
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
    /// Interactive clarifying question emitted by the agent via the
    /// `harness.ask_user` client-tool. The UI renders this as a card with
    /// selectable buttons; on user action the answer is sent back through
    /// `agent_submit_tool_result` and `state` transitions away from `Open`.
    AskUser {
        call_id: String,
        question: String,
        #[serde(default)]
        header: Option<String>,
        options: Vec<AskUserOption>,
        #[serde(default)]
        multi_select: bool,
        #[serde(default)]
        allow_other: bool,
        #[serde(default = "ask_user_state_default")]
        state: AskUserState,
    },
}

fn ask_user_state_default() -> AskUserState {
    AskUserState::Cancelled
}

fn summarize_args(tool: &str, args: Option<&Value>) -> String {
    let Some(args) = args else {
        return String::new();
    };
    let pick = match tool {
        "harness.create_workspace" => Some("title"),
        "list_workspace_files" => Some("path"),
        "read_workspace_file"
        | "memory_read"
        | "memory_create"
        | "memory_write"
        | "memory_delete"
        | "memory_backlinks" => Some("path"),
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
        "rules_read" | "skills_read" => Some("name"),
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

/// Returns the workspace-relative path that a file-reading tool accesses,
/// constructed from its arguments. Returns `None` for non-file tools.
fn file_arg_path(tool: &str, args: Option<&Value>) -> Option<String> {
    let args = args?;
    match tool {
        "rules_read" => {
            let name = args.get("name")?.as_str()?;
            Some(format!(".agents/rules/{name}"))
        }
        "skills_read" => {
            let name = args.get("name")?.as_str()?;
            Some(format!(".agents/skills/{name}/SKILL.md"))
        }
        "read_workspace_file"
        | "memory_read"
        | "memory_create"
        | "memory_write"
        | "memory_delete"
        | "memory_backlinks"
        | "list_workspace_files" => args.get("path")?.as_str().map(|s| s.to_owned()),
        "memory_rename" => args.get("newPath")?.as_str().map(|s| s.to_owned()),
        _ => None,
    }
}
