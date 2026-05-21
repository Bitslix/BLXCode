//! Unified tool dispatch for OpenAI-compatible and Anthropic agent loops.

use crate::agent::protocol::AgentEvent;
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
    if name == "subagents.run" {
        return match ctx {
            Some(c) => crate::agent::subagents::run(state, args, root, c).await,
            None => tools::ToolOutcome {
                ok: false,
                content: "subagents.run requires dispatch context".into(),
            },
        };
    }
    if name == "submit_result" {
        return tools::ToolOutcome {
            ok: true,
            content: args.to_string(),
        };
    }
    state.push(AgentEvent::ToolCall {
        tool: name.to_owned(),
        call_id: Some(call_id.to_owned()),
        args: Some(args.clone()),
    });

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
