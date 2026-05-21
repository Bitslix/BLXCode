//! OpenAI-compatible Chat Completions streaming client with a tool-call loop.
//!
//! One call to [`run_chat_turn`] drives a complete user turn:
//!   1. POST `messages` + `tools[]` with `stream:true`,
//!   2. accumulate streamed text + tool_calls,
//!   3. for each tool_call: run server tool in-process OR emit a client
//!      `ToolCall` event and await the matching `agent_submit_tool_result`,
//!   4. append the resulting `role:"tool"` messages and loop until the
//!      assistant finishes with `stop` (or hits the round budget).
//!
//! Cancellation (`state.cancelled()`) is polled between SSE lines and
//! between rounds; pending oneshots are dropped on cancel.

use crate::agent::protocol::{AgentEvent, AgentImageContextItem};
use crate::agent::state::AgentEngineState;
use crate::agent::system_prompt::system_prompt;
use crate::agent::tool_dispatch::{dispatch_tool, DispatchContext};
use crate::agent::tool_groups::openai_tool_name_to_internal;
use crate::agent::tools::WorkspaceRootGuard;
use crate::agent::pricing;
use crate::agent_settings::{AgentProviderKind, AgentProviderSettings, ThinkingLevel};
use crate::tasks;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncBufReadExt;

/// Hard upper bound on tool-call rounds per turn. Stops runaway loops if
/// the model keeps invoking tools without ever finishing.
const MAX_ROUNDS: u32 = 36;

#[derive(Clone, Copy, Debug)]
pub enum Endpoint {
    Openrouter,
    Openai,
}

impl Endpoint {
    pub(crate) fn url(self) -> &'static str {
        match self {
            Self::Openrouter => "https://openrouter.ai/api/v1/chat/completions",
            Self::Openai => "https://api.openai.com/v1/chat/completions",
        }
    }

    pub fn from_provider(p: AgentProviderKind) -> Option<Self> {
        match p {
            AgentProviderKind::Openrouter => Some(Self::Openrouter),
            AgentProviderKind::Openai => Some(Self::Openai),
            AgentProviderKind::Anthropic => None,
        }
    }
}

/// Endpoint-specific reasoning payload. The request body shape differs:
///   - OpenRouter: nested object `reasoning: { effort, exclude: false }`
///   - OpenAI Chat Completions: flat string `reasoning_effort: "low|medium|high"`
struct ReasoningPayload {
    key: &'static str,
    value: Value,
}

fn reasoning_for(level: ThinkingLevel, endpoint: Endpoint) -> Option<ReasoningPayload> {
    let effort = match level {
        ThinkingLevel::Off => return None,
        ThinkingLevel::Low => "low",
        ThinkingLevel::Medium => "medium",
        ThinkingLevel::High | ThinkingLevel::Max => "high",
    };
    Some(match endpoint {
        Endpoint::Openrouter => ReasoningPayload {
            key: "reasoning",
            value: json!({ "effort": effort, "exclude": false }),
        },
        Endpoint::Openai => ReasoningPayload {
            key: "reasoning_effort",
            value: Value::String(effort.to_owned()),
        },
    })
}

#[derive(Deserialize, Default)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    /// Final chunk in OpenAI-compatible streams when `stream_options.include_usage`
    /// is set carries the usage block. OpenRouter mirrors this convention.
    #[serde(default)]
    usage: Option<StreamUsage>,
}

#[derive(Deserialize, Default)]
struct StreamUsage {
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
    /// OpenRouter-native USD cost. Only present when the request set
    /// `usage: { include: true }`. Falls back to local token×price math
    /// when missing.
    #[serde(default)]
    cost: Option<f64>,
}

#[derive(Deserialize, Default)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<DeltaToolCall>,
    /// Reasoning text streamed by some providers when extended thinking is
    /// enabled. We surface it via dedicated `ThinkingDelta` events so the UI
    /// can show a collapsible thinking panel without polluting the transcript.
    /// `reasoning` is OpenRouter; `reasoning_content` is the DeepSeek/OpenAI
    /// compatible alternative — we accept either.
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize, Default)]
struct DeltaToolCall {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    function: DeltaFunction,
}

#[derive(Deserialize, Default)]
struct DeltaFunction {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

/// Accumulator for one assistant turn: visible text + per-index tool calls.
#[derive(Default)]
struct RoundResult {
    text: String,
    tool_calls: Vec<AggregatedToolCall>,
    finish_reason: Option<String>,
    /// Wall-clock ms from request send to first content/thinking delta.
    /// `None` when the round produced no streamed text (e.g. tool-only).
    ttft_ms: Option<u64>,
    /// Cumulative prompt_tokens reported by the provider for this round.
    prompt_tokens: Option<u64>,
    /// Cumulative completion_tokens reported by the provider for this round.
    completion_tokens: Option<u64>,
    /// OpenRouter-native cost for this round when the provider returned one.
    cost_usd: Option<f64>,
}

#[derive(Default, Clone, Debug)]
struct AggregatedToolCall {
    id: String,
    name: String,
    arguments: String,
}

pub async fn run_chat_turn(
    state: Arc<AgentEngineState>,
    endpoint: Endpoint,
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

    let sys = system_prompt(workspace_string.as_deref());
    let mut messages: Vec<Value> = Vec::with_capacity(8);
    messages.push(json!({ "role": "system", "content": sys }));
    // Carry prior turns so the model has multi-turn memory.
    messages.extend(state.conversation_snapshot());
    let consumed_image_ids = image_context_items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    messages.push(json!({
        "role": "user",
        "content": openai_user_content(&prompt, &image_context_items),
    }));

    let tools = crate::agent::tools::render_for_openai();
    let reasoning = reasoning_for(settings.thinking_level, endpoint);
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

    let provider_kind = match endpoint {
        Endpoint::Openrouter => AgentProviderKind::Openrouter,
        Endpoint::Openai => AgentProviderKind::Openai,
    };

    for round in 0..MAX_ROUNDS {
        if state.cancelled() {
            emit_aborted(&state);
            return;
        }

        let mut body = json!({
            "model": settings.model_id,
            "messages": messages,
            "stream": true,
            "stream_options": { "include_usage": true },
            "tools": tools,
        });
        // OpenRouter exposes a native `usage.cost` field but only when the
        // request opts in via `usage: { include: true }`. Cheaper than
        // computing locally and avoids drift when models reprice.
        if matches!(endpoint, Endpoint::Openrouter) {
            body["usage"] = json!({ "include": true });
        }
        if let Some(r) = &reasoning {
            body[r.key] = r.value.clone();
        }

        let image_ids_for_round = if round == 0 {
            Some(consumed_image_ids.as_slice())
        } else {
            None
        };
        let round_start = Instant::now();
        let round_res = match run_one_round(
            &client,
            endpoint,
            &api_key,
            &body,
            &state,
            image_ids_for_round,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                state.push(AgentEvent::Error { message: e });
                state.push(AgentEvent::Done);
                state.set_busy(false);
                return;
            }
        };
        let round_elapsed_ms =
            round_start.elapsed().as_millis().min(u64::MAX as u128) as u64;

        // Resolve USD cost: prefer OpenRouter's native number, fall back
        // to local token×price math via the cached pricing table.
        let round_cost = round_res.cost_usd.or_else(|| {
            pricing::resolve_cost(
                &settings,
                provider_kind,
                &settings.model_id,
                round_res.prompt_tokens,
                round_res.completion_tokens,
            )
        });

        // Per-round metric — UI attaches to the assistant block produced
        // by this round, or to a synthetic `ModelDecision` row when the
        // round was tool-only.
        state.push(AgentEvent::TurnUsage {
            kind: crate::agent::protocol::TurnUsageKind::ModelRound,
            agent_id: None,
            call_id: None,
            round_index: Some(round),
            // Placeholder — replaced by a real monotonic counter in the
            // `late-event-guard` task.
            turn_generation: 0,
            input_tokens: round_res.prompt_tokens,
            output_tokens: round_res.completion_tokens,
            ttft_ms: round_res.ttft_ms,
            elapsed_ms: round_elapsed_ms,
            cost_usd: round_cost,
        });

        // Record the assistant message verbatim — providers require the
        // exact tool_calls block on the assistant turn before tool replies.
        let mut assistant_msg = json!({ "role": "assistant" });
        if !round_res.text.is_empty() {
            assistant_msg["content"] = Value::String(round_res.text.clone());
        }
        if !round_res.tool_calls.is_empty() {
            assistant_msg["tool_calls"] = Value::Array(
                round_res
                    .tool_calls
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.id,
                            "type": "function",
                            "function": {
                                "name": c.name,
                                "arguments": if c.arguments.is_empty() { "{}".to_string() } else { c.arguments.clone() },
                            }
                        })
                    })
                    .collect(),
            );
        }
        messages.push(assistant_msg);

        if round_res.tool_calls.is_empty() {
            break; // Done — assistant finished without invoking tools.
        }

        // Execute each tool call sequentially and append the result.
        // Provider sees sanitized names (no dots, Azure-compatible) but the
        // registry and UI work with the internal dotted form.
        for call in round_res.tool_calls {
            if state.cancelled() {
                emit_aborted(&state);
                return;
            }
            let internal_name = openai_tool_name_to_internal(&call.name);
            let args_val: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
            let tool_start = Instant::now();
            let outcome = dispatch_tool(
                &state,
                &call.id,
                &internal_name,
                &args_val,
                root_guard.as_ref(),
                Some(&dispatch_ctx),
            )
            .await;
            let tool_elapsed_ms =
                tool_start.elapsed().as_millis().min(u64::MAX as u128) as u64;

            if outcome.ok && internal_name.starts_with("task_") {
                maybe_emit_task_snapshot(&state, root_guard.as_ref());
            }

            state.push(AgentEvent::ToolResult {
                tool: internal_name.clone(),
                ok: outcome.ok,
                message: Some(truncate_for_ui(&outcome.content)),
            });

            // Per-tool metric — `cost_usd` stays `None` because tool
            // dispatch happens in-process and carries no provider cost.
            state.push(AgentEvent::TurnUsage {
                kind: crate::agent::protocol::TurnUsageKind::ToolExec,
                agent_id: None,
                call_id: Some(call.id.clone()),
                round_index: Some(round),
                turn_generation: 0,
                input_tokens: None,
                output_tokens: None,
                ttft_ms: None,
                elapsed_ms: tool_elapsed_ms,
                cost_usd: None,
            });

            messages.push(json!({
                "role": "tool",
                "tool_call_id": call.id,
                "content": outcome.content,
            }));
        }

        if round + 1 == MAX_ROUNDS {
            state.push(AgentEvent::Error {
                message: format!("Tool-Loop-Limit erreicht ({MAX_ROUNDS} Runden)."),
            });
            break;
        }
    }

    // Persist non-system messages so the next turn keeps multi-turn context.
    if messages.len() > 1 {
        state.set_conversation(sanitize_conversation_images(messages.split_off(1)));
    }

    // Per-round + per-tool `TurnUsage` events were emitted inside the
    // round loop. Nothing aggregated to send here.
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

async fn run_one_round(
    client: &reqwest::Client,
    endpoint: Endpoint,
    api_key: &str,
    body: &Value,
    state: &Arc<AgentEngineState>,
    consumed_image_ids: Option<&[String]>,
) -> Result<RoundResult, String> {
    let mut req = client
        .post(endpoint.url())
        .bearer_auth(api_key)
        .header("Accept", "text/event-stream")
        .header("Content-Type", "application/json");
    if matches!(endpoint, Endpoint::Openrouter) {
        req = req
            .header("HTTP-Referer", "https://bitslix.com/blxcode")
            .header("X-Title", "blxcode");
    }

    let request_sent = Instant::now();
    let resp = req
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
    let mut tool_by_index: Vec<AggregatedToolCall> = Vec::new();
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
        let chunk: StreamChunk = match serde_json::from_str(payload) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(u) = chunk.usage {
            if let Some(p) = u.prompt_tokens {
                acc.prompt_tokens = Some(p);
            }
            if let Some(c) = u.completion_tokens {
                acc.completion_tokens = Some(c);
            }
            if let Some(cost) = u.cost {
                acc.cost_usd = Some(cost);
            }
        }
        for choice in chunk.choices {
            let reasoning_chunk = choice.delta.reasoning.or(choice.delta.reasoning_content);
            if let Some(reasoning) = reasoning_chunk {
                if !reasoning.is_empty() {
                    if acc.ttft_ms.is_none() {
                        acc.ttft_ms = Some(request_sent.elapsed().as_millis().min(u64::MAX as u128) as u64);
                    }
                    thinking_active = true;
                    state.push(AgentEvent::ThinkingDelta { delta: reasoning });
                }
            }
            if let Some(text) = choice.delta.content {
                if !text.is_empty() {
                    if acc.ttft_ms.is_none() {
                        acc.ttft_ms = Some(request_sent.elapsed().as_millis().min(u64::MAX as u128) as u64);
                    }
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
            for tc in choice.delta.tool_calls {
                let idx = tc.index as usize;
                while tool_by_index.len() <= idx {
                    tool_by_index.push(AggregatedToolCall::default());
                }
                let slot = &mut tool_by_index[idx];
                if let Some(id) = tc.id {
                    if !id.is_empty() {
                        slot.id = id;
                    }
                }
                if let Some(name) = tc.function.name {
                    if !name.is_empty() {
                        slot.name = name;
                    }
                }
                if let Some(args) = tc.function.arguments {
                    slot.arguments.push_str(&args);
                }
                let _ = tc.kind; // always "function"
            }
            if let Some(reason) = choice.finish_reason {
                acc.finish_reason = Some(reason);
            }
        }
    }

    if thinking_active && !thinking_closed {
        state.push(AgentEvent::ThinkingDone);
    }

    acc.tool_calls = tool_by_index
        .into_iter()
        .filter(|c| !c.name.is_empty())
        .collect();
    Ok(acc)
}

fn openai_user_content(prompt: &str, images: &[AgentImageContextItem]) -> Value {
    if images.is_empty() {
        return Value::String(prompt.to_owned());
    }
    let mut blocks = Vec::with_capacity(images.len() + 1);
    blocks.push(json!({ "type": "text", "text": prompt }));
    for image in images {
        blocks.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{};base64,{}", image.mime, image.bytes_b64),
            },
        }));
    }
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
            if block.get("type").and_then(Value::as_str) == Some("image_url") {
                *block = json!({
                    "type": "text",
                    "text": "[BLXCode image context was attached for the previous turn and is now marked read.]",
                });
            }
        }
    }
    messages
}

fn truncate_for_ui(s: &str) -> String {
    let max = 600;
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}… (truncated)")
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
    fn openai_user_content_includes_text_then_image_url() {
        let content = openai_user_content("look", &[image()]);
        let blocks = content.as_array().expect("content array");
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[0]["text"], "look");
        assert_eq!(blocks[1]["type"], "image_url");
        assert_eq!(
            blocks[1]["image_url"]["url"],
            "data:image/png;base64,aGVsbG8="
        );
    }

    #[test]
    fn openai_sanitizer_removes_image_data_urls() {
        let messages = vec![json!({
            "role": "user",
            "content": openai_user_content("look", &[image()]),
        })];
        let sanitized = sanitize_conversation_images(messages);
        let encoded = serde_json::to_string(&sanitized).unwrap();
        assert!(!encoded.contains("aGVsbG8="));
        assert!(!encoded.contains("image_url"));
        assert!(encoded.contains("marked read"));
    }
}
