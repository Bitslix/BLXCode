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

use crate::agent::protocol::AgentEvent;
use crate::agent::state::{AgentEngineState, ClientToolResult};
use crate::agent::system_prompt::system_prompt;
use crate::agent::tools::{self, ToolSite, WorkspaceRootGuard};
use crate::agent_settings::{AgentProviderKind, AgentProviderSettings, ThinkingLevel};
use crate::tasks;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::sync::oneshot;

/// Hard upper bound on tool-call rounds per turn. Stops runaway loops if
/// the model keeps invoking tools without ever finishing.
const MAX_ROUNDS: u32 = 12;

#[derive(Clone, Copy, Debug)]
pub enum Endpoint {
    Openrouter,
    Openai,
}

impl Endpoint {
    fn url(self) -> &'static str {
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
    messages.push(json!({ "role": "user", "content": prompt }));

    let tools = tools::render_for_openai();
    let reasoning = reasoning_for(settings.thinking_level, endpoint);

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
            "messages": messages,
            "stream": true,
            "tools": tools,
        });
        if let Some(r) = &reasoning {
            body[r.key] = r.value.clone();
        }

        let round_res = match run_one_round(&client, endpoint, &api_key, &body, &state).await {
            Ok(r) => r,
            Err(e) => {
                state.push(AgentEvent::Error { message: e });
                state.push(AgentEvent::Done);
                state.set_busy(false);
                return;
            }
        };

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
        for call in round_res.tool_calls {
            if state.cancelled() {
                emit_aborted(&state);
                return;
            }
            let args_val: Value = serde_json::from_str(&call.arguments).unwrap_or(json!({}));
            let outcome =
                dispatch_tool(&state, &call.id, &call.name, &args_val, root_guard.as_ref()).await;

            if outcome.ok && call.name.starts_with("task_") {
                maybe_emit_task_snapshot(&state, root_guard.as_ref());
            }

            state.push(AgentEvent::ToolResult {
                tool: call.name.clone(),
                ok: outcome.ok,
                message: Some(truncate_for_ui(&outcome.content)),
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
        state.set_conversation(messages.split_off(1));
    }

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
        for choice in chunk.choices {
            let reasoning_chunk = choice.delta.reasoning.or(choice.delta.reasoning_content);
            if let Some(reasoning) = reasoning_chunk {
                if !reasoning.is_empty() {
                    thinking_active = true;
                    state.push(AgentEvent::ThinkingDelta { delta: reasoning });
                }
            }
            if let Some(text) = choice.delta.content {
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

async fn dispatch_tool(
    state: &Arc<AgentEngineState>,
    call_id: &str,
    name: &str,
    args: &Value,
    root: Option<&WorkspaceRootGuard>,
) -> tools::ToolOutcome {
    // Always announce the call to the UI so the user sees what the model
    // is doing — including server-side tools.
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
        ToolSite::Server => tools::execute_server_tool(name, args, root),
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
                    format!("{name}: ok")
                } else {
                    format!("{name}: failed")
                };
            }
            tools::ToolOutcome {
                ok: res.ok,
                content: body,
            }
        }
        Err(_) => tools::ToolOutcome {
            ok: false,
            content: format!("{name}: cancelled before result"),
        },
    }
}

fn truncate_for_ui(s: &str) -> String {
    let max = 600;
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max).collect();
    format!("{cut}… (truncated)")
}
