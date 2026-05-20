//! Reine Datenstrukturen für die Agent-Chat-Timeline (serde-fähig, ohne Leptos).
//! Wird von [`crate::workbench::state::WorkspaceEntry`] und [`agent_panel::timeline`] genutzt.

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
    pub fn from_call(tool: &str, args: Option<&Value>) -> Self {
        Self {
            tool: tool.to_owned(),
            label: friendly_label(tool).to_owned(),
            args_summary: summarize_args(tool, args),
            status: ActivityStatus::Pending,
            detail: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineItem {
    User { text: String },
    Assistant { text: String },
    Tool(ToolActivity),
    Thinking { text: String, done: bool },
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

fn friendly_label(tool: &str) -> &str {
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
        other => other,
    }
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
