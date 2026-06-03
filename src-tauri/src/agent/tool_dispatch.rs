//! Unified tool dispatch for OpenAI-compatible and Anthropic agent loops.

use crate::agent::protocol::{AgentChatMode, AgentEvent, ToolPermissionKind};
use crate::agent::state::{AgentEngineState, ClientToolResult};
use crate::agent::tools::{self, ToolSite, WorkspaceRootGuard};
use crate::agent_settings::AgentProviderSettings;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Clone)]
pub struct DispatchContext {
    pub settings: AgentProviderSettings,
    pub api_key: String,
    pub chat_mode: AgentChatMode,
}

/// Dispatch one tool call: emit `ToolCall`, run server tool in-process or await client result.
pub async fn dispatch_tool(
    state: &Arc<AgentEngineState>,
    call_id: &str,
    name: &str,
    args: &Value,
    root: Option<&WorkspaceRootGuard>,
    ctx: Option<&DispatchContext>,
) -> tools::ToolOutcome {
    if name == "submit_result" {
        return tools::ToolOutcome {
            ok: true,
            content: args.to_string(),
        };
    }
    let chat_mode = effective_chat_mode(state, ctx);
    if let Some(outcome) = enforce_chat_mode(state, call_id, name, args, chat_mode).await {
        return outcome;
    }

    state.push(AgentEvent::ToolCall {
        tool: name.to_owned(),
        call_id: Some(call_id.to_owned()),
        args: Some(args.clone()),
    });

    state.push_parent(call_id.to_owned());
    let outcome = if name == "subagents.run" {
        match ctx {
            Some(c) => crate::agent::subagents::run(state, args, root, c).await,
            None => tools::ToolOutcome {
                ok: false,
                content: "subagents.run requires dispatch context".into(),
            },
        }
    } else {
        dispatch_regular_tool(state, call_id, name, args, root).await
    };
    state.pop_parent();
    outcome
}

fn effective_chat_mode(
    state: &Arc<AgentEngineState>,
    ctx: Option<&DispatchContext>,
) -> AgentChatMode {
    state
        .chat_mode_override()
        .or_else(|| ctx.map(|c| c.chat_mode))
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolPermissionClass {
    Read,
    MutatingEdit,
    Command,
    SettingsWindow,
    NavigationView,
}

async fn enforce_chat_mode(
    state: &Arc<AgentEngineState>,
    call_id: &str,
    name: &str,
    args: &Value,
    mode: AgentChatMode,
) -> Option<tools::ToolOutcome> {
    let class = classify_tool_call(name, args);
    if mode == AgentChatMode::Plan && blocks_in_plan(class, name, args) {
        return Some(tools::ToolOutcome {
            ok: false,
            content: format!("{name} blocked: Plan mode is non-mutating"),
        });
    }
    if mode != AgentChatMode::AskEdits || !requires_ask_edits_prompt(class) {
        return None;
    }

    let kind = match class {
        ToolPermissionClass::Command => ToolPermissionKind::Command,
        ToolPermissionClass::SettingsWindow | ToolPermissionClass::NavigationView => {
            ToolPermissionKind::SettingsWindow
        }
        ToolPermissionClass::MutatingEdit => ToolPermissionKind::MutatingEdit,
        ToolPermissionClass::Read => return None,
    };
    let summary = permission_summary(name, args, class);
    state.push(AgentEvent::ToolPermissionRequest {
        tool: name.to_owned(),
        call_id: call_id.to_owned(),
        mode,
        kind,
        summary,
        args: Some(args.clone()),
    });

    let (tx, rx) = oneshot::channel();
    state.register_client_tool(call_id.to_owned(), tx);
    match rx.await {
        Ok(res) if res.ok => None,
        Ok(res) => Some(tools::ToolOutcome {
            ok: false,
            content: res
                .message
                .unwrap_or_else(|| format!("{name} denied by user")),
        }),
        Err(_) => Some(tools::ToolOutcome {
            ok: false,
            content: format!("{name}: permission channel closed"),
        }),
    }
}

fn requires_ask_edits_prompt(class: ToolPermissionClass) -> bool {
    matches!(
        class,
        ToolPermissionClass::MutatingEdit
            | ToolPermissionClass::Command
            | ToolPermissionClass::SettingsWindow
    )
}

fn blocks_in_plan(class: ToolPermissionClass, name: &str, args: &Value) -> bool {
    match class {
        ToolPermissionClass::MutatingEdit | ToolPermissionClass::SettingsWindow => true,
        ToolPermissionClass::NavigationView => matches!(
            name,
            "harness.workspace_switch" | "harness.workspace_prev" | "harness.workspace_next"
        ),
        ToolPermissionClass::Command => {
            name != "shell_exec"
                || args
                    .get("writes")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
        }
        ToolPermissionClass::Read => false,
    }
}

fn classify_tool_call(name: &str, args: &Value) -> ToolPermissionClass {
    match name {
        "shell_exec" => ToolPermissionClass::Command,
        "harness.send_terminal_keys" => {
            if args
                .get("submit")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                ToolPermissionClass::Command
            } else {
                ToolPermissionClass::NavigationView
            }
        }
        "harness.terminal_interrupt" => ToolPermissionClass::Command,
        "git_apply_patch"
        | "git_add"
        | "git_commit"
        | "memory_create"
        | "memory_write"
        | "memory_delete"
        | "memory_rename"
        | "memory_rebuild_architecture"
        | "task_create"
        | "task_update"
        | "task_delete"
        | "task_reorder"
        | "plan_create"
        | "plan_write"
        | "plan_delete"
        | "plan_rename"
        | "plan_load"
        | "plan_sync_from_tasks"
        | "mermaid_create"
        | "mermaid_create_many"
        | "kanban_layout_save"
        | "kanban_task_create"
        | "kanban_task_update"
        | "kanban_task_delete"
        | "kanban_import_layout"
        | "rules_write"
        | "rules_set_enabled"
        | "rules_remove"
        | "skills_write"
        | "skills_set_enabled"
        | "skills_remove"
        | "skills_install"
        | "harness.notifications_update"
        | "harness.notifications_remove"
        | "workspace_file_write"
        | "workspace_file_delete"
        | "workspace_dir_create"
        | "workspace_entry_rename" => ToolPermissionClass::MutatingEdit,
        "memory_category_update"
        | "memory_context_attach"
        | "memory_context_detach"
        | "plan_context_attach"
        | "plan_context_detach"
        | "image_context_detach"
        | "harness.create_workspace"
        | "harness.open_terminal"
        | "harness.send_agent_context"
        | "harness.window_set_size"
        | "harness.window_set_fullscreen" => ToolPermissionClass::SettingsWindow,
        "harness.workspace_switch"
        | "harness.workspace_prev"
        | "harness.workspace_next"
        | "harness.view_show"
        | "harness.open_settings"
        | "harness.open_memory"
        | "harness.open_plan"
        | "harness.open_file"
        | "harness.open_diff"
        | "harness.notifications_create"
        | "harness.notifications_send"
        | "harness.notifications_mark_read" => ToolPermissionClass::NavigationView,
        _ => ToolPermissionClass::Read,
    }
}

fn permission_summary(name: &str, args: &Value, class: ToolPermissionClass) -> String {
    if name == "shell_exec" {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("<missing command>");
        return format!("Run command:\n\n```sh\n{command}\n```");
    }
    if name == "harness.send_terminal_keys" {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("<empty>");
        return format!("Send terminal input and submit:\n\n```text\n{text}\n```");
    }
    if name == "harness.terminal_interrupt" {
        return format!("Send Ctrl+C to terminal with args `{args}`");
    }
    let action = match class {
        ToolPermissionClass::MutatingEdit => "Run mutating edit",
        ToolPermissionClass::Command => "Run command",
        ToolPermissionClass::SettingsWindow => "Change app/workspace state",
        ToolPermissionClass::NavigationView => "Navigate the workbench",
        ToolPermissionClass::Read => "Run tool",
    };
    format!("{action}: `{name}` with args `{args}`")
}

async fn dispatch_regular_tool(
    state: &Arc<AgentEngineState>,
    call_id: &str,
    name: &str,
    args: &Value,
    root: Option<&WorkspaceRootGuard>,
) -> tools::ToolOutcome {
    // MCP-server tools are not in the static registry; route them to the live
    // client runtime before the normal lookup.
    if crate::mcp::runtime::is_mcp_tool(name) {
        return match crate::mcp::runtime::call(name, args).await {
            Ok(content) => tools::ToolOutcome { ok: true, content },
            Err(e) => tools::ToolOutcome {
                ok: false,
                content: e,
            },
        };
    }

    let Some(def) = tools::find(name) else {
        return tools::ToolOutcome {
            ok: false,
            content: format!("unknown tool: {name}"),
        };
    };

    match def.site {
        ToolSite::Server => tools::execute_server_tool(name, args, root, None),
        ToolSite::Client => wait_for_client_tool(state, call_id, name).await,
    }
}

async fn wait_for_client_tool(
    state: &Arc<AgentEngineState>,
    call_id: &str,
    name: &str,
) -> tools::ToolOutcome {
    let (tx, rx) = oneshot::channel::<ClientToolResult>();
    state.register_client_tool(call_id.to_owned(), tx);

    match rx.await {
        Ok(res) => {
            let mut body = res.message.unwrap_or_default();
            if let Some(data) = res.data {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(&data.to_string());
            }
            if body.is_empty() {
                body = if res.ok {
                    format!("{name} ok")
                } else {
                    format!("{name} failed")
                };
            }
            tools::ToolOutcome {
                ok: res.ok,
                content: body,
            }
        }
        Err(_) => tools::ToolOutcome {
            ok: false,
            content: format!("{name}: tool result channel closed"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_mutating_workspace_and_window_tools() {
        assert_eq!(
            classify_tool_call("workspace_file_write", &json!({})),
            ToolPermissionClass::MutatingEdit
        );
        assert_eq!(
            classify_tool_call("harness.window_set_size", &json!({})),
            ToolPermissionClass::SettingsWindow
        );
        assert_eq!(
            classify_tool_call("read_workspace_file", &json!({})),
            ToolPermissionClass::Read
        );
    }

    #[test]
    fn classifies_commands_with_exact_terminal_submit_boundary() {
        assert_eq!(
            classify_tool_call("shell_exec", &json!({ "command": "pwd" })),
            ToolPermissionClass::Command
        );
        assert_eq!(
            classify_tool_call("harness.send_terminal_keys", &json!({ "text": "pwd" })),
            ToolPermissionClass::NavigationView
        );
        assert_eq!(
            classify_tool_call(
                "harness.send_terminal_keys",
                &json!({ "text": "pwd", "submit": true })
            ),
            ToolPermissionClass::Command
        );
        assert_eq!(
            classify_tool_call("harness.terminal_interrupt", &json!({ "slotId": 1 })),
            ToolPermissionClass::Command
        );
        assert_eq!(
            classify_tool_call("harness.wait_terminal_output", &json!({ "slotId": 1 })),
            ToolPermissionClass::Read
        );
        assert_eq!(
            classify_tool_call("harness.read_terminal_output", &json!({ "slotId": 1 })),
            ToolPermissionClass::Read
        );
        assert_eq!(
            classify_tool_call("harness.send_agent_context", &json!({ "slotId": 1 })),
            ToolPermissionClass::SettingsWindow
        );
    }

    #[test]
    fn plan_mode_blocks_mutations_and_write_commands() {
        assert!(blocks_in_plan(
            ToolPermissionClass::MutatingEdit,
            "workspace_file_write",
            &json!({})
        ));
        assert!(blocks_in_plan(
            ToolPermissionClass::SettingsWindow,
            "harness.window_set_size",
            &json!({})
        ));
        assert!(blocks_in_plan(
            ToolPermissionClass::NavigationView,
            "harness.workspace_next",
            &json!({})
        ));
        assert!(blocks_in_plan(
            ToolPermissionClass::Command,
            "shell_exec",
            &json!({ "writes": true })
        ));
        assert!(!blocks_in_plan(
            ToolPermissionClass::Command,
            "shell_exec",
            &json!({ "writes": false })
        ));
        assert!(!blocks_in_plan(
            ToolPermissionClass::NavigationView,
            "harness.view_show",
            &json!({ "target": "plans" })
        ));
        assert!(blocks_in_plan(
            ToolPermissionClass::Command,
            "harness.terminal_interrupt",
            &json!({ "slotId": 1 })
        ));
        assert!(blocks_in_plan(
            ToolPermissionClass::SettingsWindow,
            "harness.send_agent_context",
            &json!({ "slotId": 1 })
        ));
        assert!(!blocks_in_plan(
            ToolPermissionClass::Read,
            "harness.wait_terminal_output",
            &json!({ "slotId": 1 })
        ));
        assert!(!blocks_in_plan(
            ToolPermissionClass::NavigationView,
            "harness.notifications_send",
            &json!({ "title": "Done", "kind": "task_completed" })
        ));
        assert!(blocks_in_plan(
            ToolPermissionClass::MutatingEdit,
            "harness.notifications_remove",
            &json!({ "id": "n1" })
        ));
    }

    #[test]
    fn ask_edits_prompts_only_risky_classes() {
        assert!(requires_ask_edits_prompt(ToolPermissionClass::MutatingEdit));
        assert!(requires_ask_edits_prompt(ToolPermissionClass::Command));
        assert!(requires_ask_edits_prompt(
            ToolPermissionClass::SettingsWindow
        ));
        assert!(!requires_ask_edits_prompt(
            ToolPermissionClass::NavigationView
        ));
        assert!(!requires_ask_edits_prompt(ToolPermissionClass::Read));
    }

    #[test]
    fn effective_chat_mode_prefers_runtime_override() {
        let state = AgentEngineState::new();
        let ctx = DispatchContext {
            settings: AgentProviderSettings::default(),
            api_key: String::new(),
            chat_mode: AgentChatMode::AskEdits,
        };

        assert_eq!(effective_chat_mode(&state, Some(&ctx)), AgentChatMode::AskEdits);

        state.set_chat_mode_override(AgentChatMode::AllowAll);
        assert_eq!(
            effective_chat_mode(&state, Some(&ctx)),
            AgentChatMode::AllowAll
        );
    }

    #[test]
    fn permission_summaries_show_terminal_control_details() {
        assert!(permission_summary(
            "harness.send_terminal_keys",
            &json!({ "text": "npm test", "submit": true }),
            ToolPermissionClass::Command
        )
        .contains("npm test"));
        assert!(permission_summary(
            "harness.terminal_interrupt",
            &json!({ "slotId": 2 }),
            ToolPermissionClass::Command
        )
        .contains("Ctrl+C"));
    }
}
