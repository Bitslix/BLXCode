//! Anthropic Messages API client with streaming + extended thinking + tools.
//!
//! Mirrors the structure of [`crate::agent::openrouter`] but speaks
//! Anthropic's native `/v1/messages` SSE protocol:
//!   - `thinking: { type: "enabled", budget_tokens }` → streamed `thinking_delta`
//!   - Tool blocks via `content_block_start` + `input_json_delta`
//!   - `role:"user"` follow-up messages carry `tool_result` content blocks
//!
//! Cancellation, multi-turn history, and tool dispatch follow the same
//! contracts as the OpenAI-compatible path.

use super::system_prompt::system_prompt;
use crate::agent::protocol::{AgentEvent, AgentImageContextItem};
use crate::agent::state::AgentEngineState;
use crate::agent::tool_dispatch::{dispatch_tool, DispatchContext};
use crate::agent::tools::{self, WorkspaceRootGuard};
use crate::agent_settings::{AgentProviderSettings, ThinkingLevel};
use crate::tasks;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_ROUNDS: u32 = 12;
const DEFAULT_MAX_TOKENS: u64 = 8192;

/// Anthropic restricts tool names to `^[a-zA-Z0-9_-]{1,64}$` — no dots.
/// Our harness tools use dotted names (`harness.open_terminal`), so we
/// translate `.` ↔ `__` at the API boundary.
fn to_anthropic_name(name: &str) -> String {
    name.replace('.', "__")
}

fn from_anthropic_name(name: &str) -> String {
    name.replace("__", ".")
}

fn thinking_budget(level: ThinkingLevel) -> Option<u64> {
    match level {
        ThinkingLevel::Off => None,
        ThinkingLevel::Low => Some(1024),
        ThinkingLevel::Medium => Some(4096),
        ThinkingLevel::High => Some(8192),
        ThinkingLevel::Max => Some(16384),
    }
}

#[derive(Default)]
struct AggregatedToolCall {
    id: String,
    name: String,
    arguments: String,
}

#[derive(Default)]
struct RoundResult {
    /// Visible assistant text accumulated this round.
    text: String,
    /// Reasoning text accumulated this round (for the assistant memory block).
    thinking_text: String,
    /// Anthropic signature for the thinking block (must be echoed back on the
    /// next request if we want to preserve cached reasoning context).
    thinking_signature: Option<String>,
    tool_calls: Vec<AggregatedToolCall>,
    stop_reason: Option<String>,
}

pub async fn run_chat_turn(
    state: Arc<AgentEngineState>,
    api_key: String,
    settings: AgentProviderSettings,
    prompt: String,
    image_context_items: Vec<AgentImageContextItem>,
    workspace_root: Option<String>,
) {
    state.clear_cancel();
    state.set_busy(true);

    if settings.model_id.trim().is_empty() {
        state.push(AgentEvent::Error {
            message: "Kein Modell konfiguriert.".into(),
        });
        state.push(AgentEvent::Done);
        state.set_busy(false);
        return;
    }

    let root_guard = match workspace_root.as_deref() {
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
    let workspace_string = workspace_root
        .as_ref()
        .map(|s| s.clone())
        .filter(|s| !s.trim().is_empty());

    let system = system_prompt(workspace_string.as_deref());

    // Anthropic stores `system` separately from `messages`. Persisted
    // history is `user` / `assistant` messages only.
    let mut messages: Vec<Value> = state.conversation_snapshot();
    let consumed_image_ids = image_context_items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    messages.push(json!({
        "role": "user",
        "content": anthropic_user_content(&prompt, &image_context_items),
    }));

    let mut tools_json = tools::render_for_anthropic();
    if let Some(arr) = tools_json.as_array_mut() {
        for entry in arr {
            if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                let safe = to_anthropic_name(name);
                entry["name"] = Value::String(safe);
            }
        }
    }
    let thinking_cfg = thinking_budget(settings.thinking_level)
        .map(|budget| json!({ "type": "enabled", "budget_tokens": budget }));

    let dispatch_ctx = DispatchContext {
        settings: settings.clone(),
        api_key: api_key.clone(),
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            state.push(AgentEvent::Error {
                message: format!("http client: {e}"),
            });
            state.push(AgentEvent::Done);
            state.set_busy(false);
            return;
        }
    };

    for round in 0..MAX_ROUNDS {
        if state.cancelled() {
            emit_aborted(&state);
            return;
        }

        let mut body = json!({
            "model": settings.model_id,
            "max_tokens": DEFAULT_MAX_TOKENS,
            "system": system,
            "messages": messages,
            "tools": tools_json,
            "stream": true,
        });
        if let Some(t) = &thinking_cfg {
            body["thinking"] = t.clone();
        }

        let image_ids_for_round = if round == 0 {
            Some(consumed_image_ids.as_slice())
        } else {
            None
        };
        let round_res =
            match run_one_round(&client, &api_key, &body, &state, image_ids_for_round).await {
                Ok(r) => r,
                Err(e) => {
                    state.push(AgentEvent::Error { message: e });
                    state.push(AgentEvent::Done);
                    state.set_busy(false);
                    return;
                }
            };

        // Assemble the assistant message in Anthropic's content-block form so
        // tool_use ids resolve on the next round.
        let mut assistant_blocks: Vec<Value> = Vec::new();
        if !round_res.thinking_text.is_empty() {
            let mut block = json!({
                "type": "thinking",
                "thinking": round_res.thinking_text.clone(),
            });
            if let Some(sig) = &round_res.thinking_signature {
                block["signature"] = Value::String(sig.clone());
            }
            assistant_blocks.push(block);
        }
        if !round_res.text.is_empty() {
            assistant_blocks.push(json!({ "type": "text", "text": round_res.text }));
        }
        for call in &round_res.tool_calls {
            let input: Value = serde_json::from_str(&call.arguments).unwrap_or_else(|_| json!({}));
            assistant_blocks.push(json!({
                "type": "tool_use",
                "id": call.id,
                // Echo the dot-free form Anthropic accepts.
                "name": to_anthropic_name(&call.name),
                "input": input,
            }));
        }
        if !assistant_blocks.is_empty() {
            messages.push(json!({
                "role": "assistant",
                "content": assistant_blocks,
            }));
        }

        let finished = round_res.tool_calls.is_empty()
            || matches!(
                round_res.stop_reason.as_deref(),
                Some("end_turn") | Some("stop_sequence")
            );

        if round_res.tool_calls.is_empty() {
            break;
        }

        // Execute tools and build the next user message carrying tool_result
        // blocks (Anthropic requires them inside a single user turn).
        let mut tool_result_blocks: Vec<Value> = Vec::new();
        for call in round_res.tool_calls {
            if state.cancelled() {
                emit_aborted(&state);
                return;
            }
            let args_val: Value =
                serde_json::from_str(&call.arguments).unwrap_or_else(|_| json!({}));
            let outcome =
                dispatch_tool(
                    &state,
                    &call.id,
                    &call.name,
                    &args_val,
                    root_guard.as_ref(),
                    Some(&dispatch_ctx),
                )
                .await;

            if outcome.ok && call.name.starts_with("task_") {
                maybe_emit_task_snapshot(&state, root_guard.as_ref());
            }

            state.push(AgentEvent::ToolResult {
                tool: call.name.clone(),
                ok: outcome.ok,
                message: Some(truncate_for_ui(&outcome.content)),
            });

            tool_result_blocks.push(json!({
                "type": "tool_result",
                "tool_use_id": call.id,
                "content": outcome.content,
                "is_error": !outcome.ok,
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": tool_result_blocks,
        }));

        if finished {
            break;
        }

        if round + 1 == MAX_ROUNDS {
            state.push(AgentEvent::Error {
                message: format!("Tool-Loop-Limit erreicht ({MAX_ROUNDS} Runden)."),
            });
            break;
        }
    }

    state.set_conversation(sanitize_conversation_images(messages));

    state.push(AgentEvent::Done);
    state.set_busy(false);
}

fn maybe_emit_task_snapshot(state: &Arc<AgentEngineState>, root: Option<&WorkspaceRootGuard>) {
    let Some(root) = root else {
        return;
    };
    if let Ok(snapshot) = tasks::tasks_snapshot(&root.as_str()) {
        state.push(AgentEvent::TaskSnapshot { snapshot });
    }
}

fn emit_aborted(state: &Arc<AgentEngineState>) {
    crate::agent::shell_exec::kill_all_children();
    state.push(AgentEvent::AssistantDelta {
        delta: "\n_Abgebrochen._\n".into(),
    });
    state.push(AgentEvent::Done);
    state.clear_cancel();
    state.set_busy(false);
}

fn truncate_for_ui(s: &str) -> String {
    const LIMIT: usize = 4000;
    if s.len() <= LIMIT {
        s.to_owned()
    } else {
        let mut t = s[..LIMIT].to_owned();
        t.push_str("\n…(truncated)");
        t
    }
}

// ─── SSE event types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    ContentBlockStart {
        index: u32,
        content_block: BlockStart,
    },
    ContentBlockDelta {
        index: u32,
        delta: BlockDelta,
    },
    #[allow(dead_code)]
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDeltaInner,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BlockStart {
    Text {
        #[serde(default)]
        #[allow(dead_code)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        #[allow(dead_code)]
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BlockDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    SignatureDelta {
        signature: String,
    },
    InputJsonDelta {
        partial_json: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Default)]
struct MessageDeltaInner {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Default)]
struct BlockState {
    kind: BlockKind,
    tool_id: String,
    tool_name: String,
    tool_args: String,
    thinking_signature: Option<String>,
}

#[derive(Default, PartialEq, Eq)]
enum BlockKind {
    #[default]
    Unknown,
    Text,
    Thinking,
    Tool,
}

async fn run_one_round(
    client: &reqwest::Client,
    api_key: &str,
    body: &Value,
    state: &Arc<AgentEngineState>,
    consumed_image_ids: Option<&[String]>,
) -> Result<RoundResult, String> {
    let resp = client
        .post(ANTHROPIC_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("provider request: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let snippet = resp
            .text()
            .await
            .unwrap_or_else(|e| format!("(body read failed: {e})"));
        let trimmed = if snippet.len() > 400 {
            format!("{}…", &snippet[..400])
        } else {
            snippet
        };
        return Err(format!("provider HTTP {status}: {trimmed}"));
    }

    if let Some(ids) = consumed_image_ids {
        if !ids.is_empty() {
            state.push(AgentEvent::ImageContextConsumed { ids: ids.to_vec() });
        }
    }

    let stream = resp.bytes_stream();
    use futures_util::TryStreamExt;
    let reader = tokio_util::io::StreamReader::new(
        stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
    );
    let mut lines = tokio::io::BufReader::new(reader).lines();

    let mut acc = RoundResult::default();
    let mut blocks: Vec<BlockState> = Vec::new();
    let mut thinking_active = false;
    let mut thinking_closed = false;

    loop {
        if state.cancelled() {
            return Err("cancelled".to_string());
        }
        let line = match lines
            .next_line()
            .await
            .map_err(|e| format!("stream read: {e}"))?
        {
            Some(l) => l,
            None => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(payload) = trimmed.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            break;
        }
        let event: StreamEvent = match serde_json::from_str(payload) {
            Ok(e) => e,
            Err(_) => continue,
        };

        match event {
            StreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let idx = index as usize;
                while blocks.len() <= idx {
                    blocks.push(BlockState::default());
                }
                let slot = &mut blocks[idx];
                match content_block {
                    BlockStart::Text { .. } => slot.kind = BlockKind::Text,
                    BlockStart::Thinking { .. } => {
                        slot.kind = BlockKind::Thinking;
                        thinking_active = true;
                    }
                    BlockStart::ToolUse { id, name } => {
                        slot.kind = BlockKind::Tool;
                        slot.tool_id = id;
                        // Translate back from the dot-free form we sent on the
                        // tools list so internal dispatch finds the real tool.
                        slot.tool_name = from_anthropic_name(&name);
                        // First non-thinking, non-text block closes the
                        // thinking phase as far as the UI is concerned.
                        if thinking_active && !thinking_closed {
                            thinking_closed = true;
                            state.push(AgentEvent::ThinkingDone);
                        }
                    }
                    BlockStart::Other => {}
                }
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                let idx = index as usize;
                if idx >= blocks.len() {
                    continue;
                }
                let slot = &mut blocks[idx];
                match delta {
                    BlockDelta::TextDelta { text } => {
                        if !text.is_empty() {
                            if thinking_active && !thinking_closed {
                                thinking_closed = true;
                                state.push(AgentEvent::ThinkingDone);
                            }
                            state.push(AgentEvent::AssistantDelta {
                                delta: text.clone(),
                            });
                            acc.text.push_str(&text);
                        }
                    }
                    BlockDelta::ThinkingDelta { thinking } => {
                        if !thinking.is_empty() {
                            state.push(AgentEvent::ThinkingDelta {
                                delta: thinking.clone(),
                            });
                            acc.thinking_text.push_str(&thinking);
                        }
                    }
                    BlockDelta::SignatureDelta { signature } => {
                        slot.thinking_signature = Some(signature.clone());
                        acc.thinking_signature = Some(signature);
                    }
                    BlockDelta::InputJsonDelta { partial_json } => {
                        slot.tool_args.push_str(&partial_json);
                    }
                    BlockDelta::Other => {}
                }
            }
            StreamEvent::ContentBlockStop { .. } => {}
            StreamEvent::MessageDelta { delta } => {
                if let Some(reason) = delta.stop_reason {
                    acc.stop_reason = Some(reason);
                }
            }
            StreamEvent::Other => {}
        }
    }

    if thinking_active && !thinking_closed {
        state.push(AgentEvent::ThinkingDone);
    }

    acc.tool_calls = blocks
        .into_iter()
        .filter(|b| b.kind == BlockKind::Tool && !b.tool_name.is_empty())
        .map(|b| AggregatedToolCall {
            id: b.tool_id,
            name: b.tool_name,
            arguments: if b.tool_args.is_empty() {
                "{}".to_string()
            } else {
                b.tool_args
            },
        })
        .collect();

    Ok(acc)
}

fn anthropic_user_content(prompt: &str, images: &[AgentImageContextItem]) -> Value {
    let mut blocks = Vec::with_capacity(images.len() + 1);
    for image in images {
        blocks.push(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": image.mime,
                "data": image.bytes_b64,
            },
        }));
    }
    blocks.push(json!({ "type": "text", "text": prompt }));
    Value::Array(blocks)
}

fn sanitize_conversation_images(mut messages: Vec<Value>) -> Vec<Value> {
    for msg in &mut messages {
        let Some(content) = msg.get_mut("content") else {
            continue;
        };
        let Some(blocks) = content.as_array_mut() else {
            continue;
        };
        for block in blocks {
            if block.get("type").and_then(Value::as_str) == Some("image") {
                *block = json!({
                    "type": "text",
                    "text": "[BLXCode image context was attached for the previous turn and is now marked read.]",
                });
            }
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image() -> AgentImageContextItem {
        AgentImageContextItem {
            id: "image:test".into(),
            label: "test.png".into(),
            mime: "image/png".into(),
            bytes_b64: "aGVsbG8=".into(),
            size_bytes: 5,
            added_at: 1,
        }
    }

    #[test]
    fn anthropic_user_content_includes_image_then_text() {
        let content = anthropic_user_content("look", &[image()]);
        let blocks = content.as_array().expect("content array");
        assert_eq!(blocks[0]["type"], "image");
        assert_eq!(blocks[0]["source"]["type"], "base64");
        assert_eq!(blocks[0]["source"]["media_type"], "image/png");
        assert_eq!(blocks[0]["source"]["data"], "aGVsbG8=");
        assert_eq!(blocks[1]["type"], "text");
        assert_eq!(blocks[1]["text"], "look");
    }

    #[test]
    fn anthropic_sanitizer_removes_image_base64() {
        let messages = vec![json!({
            "role": "user",
            "content": anthropic_user_content("look", &[image()]),
        })];
        let sanitized = sanitize_conversation_images(messages);
        let encoded = serde_json::to_string(&sanitized).unwrap();
        assert!(!encoded.contains("aGVsbG8="));
        assert!(!encoded.contains("\"image\""));
        assert!(encoded.contains("marked read"));
    }
}
