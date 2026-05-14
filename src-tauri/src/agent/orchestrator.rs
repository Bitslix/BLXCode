use crate::agent::protocol::AgentEvent;
use crate::agent::state::AgentEngineState;
use crate::agent::tools::{ScopedReadOps, WorkspaceRootGuard};
use serde_json::json;
use std::sync::Arc;
use tauri::async_runtime;

/// Mock stream + optional scoped read via `READ:relative/path` in the prompt.
pub fn spawn_mock_turn(state: Arc<AgentEngineState>, prompt: String, root: Option<String>) {
    async_runtime::spawn(run_mock_turn_inner(state, prompt, root));
}

async fn run_mock_turn_inner(state: Arc<AgentEngineState>, prompt: String, root: Option<String>) {
    state.clear_cancel();
    state.set_busy(true);

    let root_guard: Option<WorkspaceRootGuard> = match root.as_deref() {
        None | Some("") => None,
        Some(raw) => match WorkspaceRootGuard::parse(raw) {
            Ok(g) => g,
            Err(err) => {
                state.push(AgentEvent::Error { message: err });
                state.push(AgentEvent::Done);
                state.set_busy(false);
                return;
            }
        },
    };

    crate::agent::provider::maybe_emit_network_hint(Arc::clone(&state)).await;

    let prelude = format!("(Harness‑Mock) Ziel: «{prompt}»\n");
    for chunk in split_chunks(&prelude, 48) {
        if state.cancelled() {
            finish_aborted(&state).await;
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(35)).await;
        state.push(AgentEvent::AssistantDelta { delta: chunk });
    }

    if let Some(rel) = parse_read_inline(&prompt) {
        if state.cancelled() {
            finish_aborted(&state).await;
            return;
        }
        state.push(AgentEvent::ToolCall {
            tool: "read_workspace_file".into(),
            call_id: None,
            args: Some(json!({ "relativePath": rel })),
        });
        tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

        match ScopedReadOps::read_text(root_guard.as_ref(), rel) {
            Ok(text) => {
                state.push(AgentEvent::ToolResult {
                    tool: "read_workspace_file".into(),
                    ok: true,
                    message: Some(truncate_chars(&text, 4000)),
                });
                let preview: String = text.lines().take(24).collect::<Vec<_>>().join("\n");
                let summarize = summarize_read(rel, preview);
                for chunk in split_chunks(&summarize, 72) {
                    if state.cancelled() {
                        finish_aborted(&state).await;
                        return;
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(40)).await;
                    state.push(AgentEvent::AssistantDelta { delta: chunk });
                }
            }
            Err(e) => {
                state.push(AgentEvent::ToolResult {
                    tool: "read_workspace_file".into(),
                    ok: false,
                    message: Some(e.to_string()),
                });
            }
        }
    }

    if state.cancelled() {
        finish_aborted(&state).await;
        return;
    }

    state.push(AgentEvent::Done);
    state.set_busy(false);
}

async fn finish_aborted(state: &Arc<AgentEngineState>) {
    state.push(AgentEvent::AssistantDelta {
        delta: "\n_Abgebrochen._\n".into(),
    });
    state.push(AgentEvent::Done);
    state.clear_cancel();
    state.set_busy(false);
}

fn parse_read_inline(prompt: &str) -> Option<&str> {
    const KEY: &str = "READ:";
    prompt.find(KEY).map(|i| prompt[i + KEY.len()..].trim())
}

fn split_chunks(s: &str, chunk: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        cur.push(ch);
        if cur.chars().count() >= chunk {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}… (truncated)")
}

fn summarize_read(path: &str, preview: String) -> String {
    format!("\n**Vorschau von `{path}`:**\n```\n{preview}\n```\n")
}
