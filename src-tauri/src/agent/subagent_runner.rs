//! Provider-specific subagent HTTP loops (OpenAI-compatible + Anthropic).

use crate::agent::anthropic::{from_anthropic_name, to_anthropic_name};
use crate::agent::openrouter::Endpoint;
use crate::agent::protocol::AgentEvent;
use crate::agent::state::AgentEngineState;
use crate::agent::subagent_prompts::{self, SubagentRole, truncate_submit_result};
use crate::agent::tool_groups::{openai_tool_name_to_internal, ToolGroup};
use crate::agent::tools::{self, WorkspaceRootGuard};
use crate::agent::tool_dispatch::DispatchContext;
use crate::agent_settings::AgentProviderKind;
use futures_util::TryStreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;

const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_SUBAGENT_ROUNDS: u32 = 8;
const MAX_OUTPUT_TOKENS_ESTIMATE: usize = 20_000;

#[derive(Clone, Copy)]
pub enum SubagentProvider {
    OpenAi(Endpoint),
    Anthropic,
}

impl SubagentProvider {
    pub fn from_settings(provider: AgentProviderKind) -> Option<Self> {
        match provider {
            AgentProviderKind::Anthropic => Some(Self::Anthropic),
            AgentProviderKind::Openrouter => Some(Self::OpenAi(Endpoint::Openrouter)),
            AgentProviderKind::Openai => Some(Self::OpenAi(Endpoint::Openai)),
        }
    }
}

/// Streaming SSE chunk shape used by OpenAI-compatible providers
/// (OpenRouter, OpenAI, Azure-routed). We only model the subset we need:
/// per-choice `delta.content` text, `delta.tool_calls` partials, and the
/// `reasoning` / `reasoning_content` channels used by some providers when
/// extended thinking is enabled.
#[derive(Deserialize, Default)]
struct OpenAiStreamChunk {
    #[serde(default)]
    choices: Vec<OpenAiStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize, Default)]
struct OpenAiStreamChoice {
    #[serde(default)]
    delta: OpenAiStreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct OpenAiStreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiDeltaToolCall>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize, Default)]
struct OpenAiDeltaToolCall {
    #[serde(default)]
    index: u32,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: OpenAiDeltaFn,
}

#[derive(Deserialize, Default)]
struct OpenAiDeltaFn {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiUsage {
    #[serde(default)]
    completion_tokens: Option<u64>,
}

#[derive(Default)]
struct OpenAiRoundResult {
    text: String,
    tool_calls: Vec<OpenAiAggregatedCall>,
    completion_tokens: Option<u64>,
}

#[derive(Default, Clone, Debug)]
struct OpenAiAggregatedCall {
    id: String,
    name: String,
    arguments: String,
}

pub async fn run_one_subagent(
    state: &Arc<AgentEngineState>,
    ctx: &DispatchContext,
    provider: SubagentProvider,
    workspace_root: Option<&str>,
    agent_id: &str,
    role: SubagentRole,
    display_name: &str,
    task: &str,
    success_criteria: &[String],
    groups: &[ToolGroup],
    tools_openai: Value,
    tools_anthropic: Value,
) -> Value {
    state.push(AgentEvent::SubagentStarted {
        agent_id: agent_id.to_owned(),
        role: subagent_prompts::role_id(role).to_owned(),
        display_name: display_name.to_owned(),
    });

    let root_guard = workspace_root.and_then(|r| WorkspaceRootGuard::parse(r).ok().flatten());
    let ws = workspace_root.unwrap_or("<no workspace>");
    let system = subagent_prompts::subagent_system_prompt(
        ws,
        role,
        display_name,
        task,
        success_criteria,
        groups,
    );

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let result = blocked_result(role, display_name, &format!("http client: {e}"));
            finish_subagent(state, agent_id, &result);
            return result;
        }
    };

    let mut output_estimate = 0usize;
    let mut iterations = 0u32;
    let mut final_submit: Option<Value> = None;
    // Names of all non-`submit_result` tool calls the subagent has actually
    // made in this run. Used by `validate_submit` to detect "blocked without
    // trying", where a model returns `status:"blocked"` claiming missing
    // workspace tools without ever calling `list_workspace_files` etc.
    let mut tools_called: std::collections::HashSet<String> = std::collections::HashSet::new();
    let has_workspace_read = groups.iter().any(|g| matches!(g, ToolGroup::WorkspaceRead));

    match provider {
        SubagentProvider::OpenAi(endpoint) => {
            let mut messages = vec![
                json!({ "role": "system", "content": system }),
                json!({ "role": "user", "content": task }),
            ];
            for _round in 0..MAX_SUBAGENT_ROUNDS {
                if state.cancelled() {
                    crate::agent::shell_exec::kill_all_children();
                    let result = blocked_result(role, display_name, "cancelled");
                    finish_subagent(state, agent_id, &result);
                    return result;
                }
                iterations += 1;
                let body = json!({
                    "model": ctx.settings.model_id,
                    "messages": messages,
                    "tools": tools_openai,
                    "stream": true,
                });
                let round = match stream_openai_subagent_round(
                    state, &client, endpoint, &ctx.api_key, &body, agent_id,
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let result = blocked_result(role, display_name, &format!("stream: {e}"));
                        finish_subagent(state, agent_id, &result);
                        return result;
                    }
                };
                if let Some(u) = round.completion_tokens {
                    output_estimate = output_estimate.saturating_add(u as usize);
                } else {
                    output_estimate += round.text.len().div_ceil(4);
                }
                if output_estimate > MAX_OUTPUT_TOKENS_ESTIMATE {
                    let result = blocked_result(role, display_name, "output token cap reached");
                    finish_subagent(state, agent_id, &result);
                    return result;
                }
                let mut assistant = json!({ "role": "assistant" });
                if !round.text.is_empty() {
                    assistant["content"] = Value::String(round.text.clone());
                }
                if !round.tool_calls.is_empty() {
                    assistant["tool_calls"] = Value::Array(
                        round.tool_calls.iter().map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.name,
                                    "arguments": if tc.arguments.is_empty() {
                                        "{}".to_string()
                                    } else {
                                        tc.arguments.clone()
                                    },
                                }
                            })
                        }).collect(),
                    );
                }
                messages.push(assistant);
                if round.tool_calls.is_empty() {
                    break;
                }
                let mut accepted_submit = false;
                for tc in round.tool_calls {
                    let internal_name = openai_tool_name_to_internal(&tc.name);
                    match handle_tool_call(
                        state,
                        agent_id,
                        &tc.id,
                        &internal_name,
                        &tc.arguments,
                        groups,
                        root_guard.as_ref(),
                        &mut tools_called,
                        has_workspace_read,
                        &mut final_submit,
                    ) {
                        ToolCallOutcome::SubmitAccepted => {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tc.id,
                                "content": "submit_result accepted",
                            }));
                            accepted_submit = true;
                            break;
                        }
                        ToolCallOutcome::SubmitRejected(message) => {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tc.id,
                                "content": message,
                            }));
                            // Loop continues so the model retries with a real
                            // workspace probe before re-submitting.
                            continue;
                        }
                        ToolCallOutcome::NotSubmit => {
                            let args: Value =
                                serde_json::from_str(&tc.arguments).unwrap_or(json!({}));
                            let outcome = execute_subagent_tool(
                                &internal_name,
                                &args,
                                groups,
                                root_guard.as_ref(),
                            );
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tc.id,
                                "content": outcome.content,
                            }));
                        }
                    }
                }
                if accepted_submit {
                    break;
                }
            }
        }
        SubagentProvider::Anthropic => {
            let mut messages: Vec<Value> = vec![json!({
                "role": "user",
                "content": task,
            })];
            for _round in 0..MAX_SUBAGENT_ROUNDS {
                if state.cancelled() {
                    crate::agent::shell_exec::kill_all_children();
                    let result = blocked_result(role, display_name, "cancelled");
                    finish_subagent(state, agent_id, &result);
                    return result;
                }
                iterations += 1;
                let body = json!({
                    "model": ctx.settings.model_id,
                    "max_tokens": 8192,
                    "system": system,
                    "messages": messages,
                    "tools": tools_anthropic,
                    "stream": true,
                });
                let round = match stream_anthropic_subagent_round(
                    state, &client, &ctx.api_key, &body, agent_id,
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        let result = blocked_result(role, display_name, &format!("stream: {e}"));
                        finish_subagent(state, agent_id, &result);
                        return result;
                    }
                };
                if let Some(u) = round.output_tokens {
                    output_estimate = output_estimate.saturating_add(u as usize);
                } else {
                    output_estimate += round.text.len().div_ceil(4);
                }
                if output_estimate > MAX_OUTPUT_TOKENS_ESTIMATE {
                    let result = blocked_result(role, display_name, "output token cap reached");
                    finish_subagent(state, agent_id, &result);
                    return result;
                }
                if !round.assistant_blocks.is_empty() {
                    messages.push(json!({ "role": "assistant", "content": round.assistant_blocks }));
                }
                if round.tool_uses.is_empty() {
                    break;
                }
                let mut result_blocks: Vec<Value> = Vec::new();
                let mut accepted_submit = false;
                for (id, name, args_str) in round.tool_uses {
                    match handle_tool_call(
                        state,
                        agent_id,
                        &id,
                        &name,
                        &args_str,
                        groups,
                        root_guard.as_ref(),
                        &mut tools_called,
                        has_workspace_read,
                        &mut final_submit,
                    ) {
                        ToolCallOutcome::SubmitAccepted => {
                            result_blocks.push(json!({
                                "type": "tool_result",
                                "tool_use_id": id,
                                "content": "submit_result accepted",
                            }));
                            accepted_submit = true;
                            break;
                        }
                        ToolCallOutcome::SubmitRejected(message) => {
                            result_blocks.push(json!({
                                "type": "tool_result",
                                "tool_use_id": id,
                                "content": message,
                                "is_error": true,
                            }));
                            continue;
                        }
                        ToolCallOutcome::NotSubmit => {
                            let args: Value = serde_json::from_str(&args_str).unwrap_or(json!({}));
                            let outcome =
                                execute_subagent_tool(&name, &args, groups, root_guard.as_ref());
                            result_blocks.push(json!({
                                "type": "tool_result",
                                "tool_use_id": id,
                                "content": outcome.content,
                                "is_error": !outcome.ok,
                            }));
                        }
                    }
                }
                messages.push(json!({ "role": "user", "content": result_blocks }));
                if accepted_submit {
                    break;
                }
                if round.stop_reason.as_deref() == Some("end_turn") {
                    break;
                }
            }
        }
    }

    let mut result = final_submit.unwrap_or_else(|| {
        blocked_result(
            role,
            display_name,
            "max iterations reached without submit_result",
        )
    });
    if let Some(obj) = result.as_object_mut() {
        obj.insert(
            "meta".into(),
            json!({
                "iterations": iterations,
                "outputTokensEstimate": output_estimate,
            }),
        );
    }
    finish_subagent(state, agent_id, &result);
    result
}

/// Outcome reported back by [`handle_tool_call`] so the caller knows whether
/// to finalize the loop, inject a corrective tool response, or proceed to
/// regular tool execution.
#[derive(Debug)]
enum ToolCallOutcome {
    /// `submit_result` accepted — caller should record it and break the loop.
    SubmitAccepted,
    /// `submit_result` rejected by [`validate_submit`]; caller must push the
    /// embedded message back to the model as the tool response and keep
    /// looping so the model retries.
    SubmitRejected(String),
    /// Not a `submit_result` call — caller should run the regular tool.
    NotSubmit,
}

fn handle_tool_call(
    state: &Arc<AgentEngineState>,
    agent_id: &str,
    call_id: &str,
    name: &str,
    args_str: &str,
    groups: &[ToolGroup],
    root: Option<&WorkspaceRootGuard>,
    tools_called: &mut std::collections::HashSet<String>,
    has_workspace_read: bool,
    final_submit: &mut Option<Value>,
) -> ToolCallOutcome {
    state.push(AgentEvent::SubagentToolCall {
        agent_id: agent_id.to_owned(),
        tool: name.to_owned(),
        call_id: Some(call_id.to_owned()),
        args: serde_json::from_str(args_str).ok(),
    });
    if name == "submit_result" {
        let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
        match validate_submit(&args, tools_called, has_workspace_read) {
            SubmitVerdict::Accept => {
                *final_submit = Some(truncate_submit_result(args));
                ToolCallOutcome::SubmitAccepted
            }
            SubmitVerdict::RejectBlockedWithoutTrying { message } => {
                ToolCallOutcome::SubmitRejected(message)
            }
        }
    } else {
        tools_called.insert(name.to_owned());
        let _ = (groups, root);
        ToolCallOutcome::NotSubmit
    }
}

fn execute_subagent_tool(
    name: &str,
    args: &Value,
    groups: &[ToolGroup],
    root: Option<&WorkspaceRootGuard>,
) -> tools::ToolOutcome {
    let shell_write = groups.contains(&ToolGroup::ShellWrite);
    if name == "shell_exec" {
        tools::execute_server_tool(
            name,
            args,
            root,
            Some(tools::ToolExecOpts {
                shell_writes: shell_write,
            }),
        )
    } else {
        tools::execute_server_tool(name, args, root, None)
    }
}

fn finish_subagent(state: &Arc<AgentEngineState>, agent_id: &str, result: &Value) {
    let status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("blocked")
        .to_owned();
    state.push(AgentEvent::SubagentFinished {
        agent_id: agent_id.to_owned(),
        status,
        summary: result
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned(),
    });
}

/// Names of workspace-read tools whose use proves the subagent actually
/// attempted to access the file system before submitting a result.
const WORKSPACE_READ_TOOLS: &[&str] = &[
    "list_workspace_files",
    "read_workspace_file",
    "workspace_search",
];

/// Verdict for a candidate `submit_result` call from a subagent.
#[derive(Debug, PartialEq, Eq)]
enum SubmitVerdict {
    /// Accept this submission as the final result.
    Accept,
    /// Reject because the subagent claimed "blocked" without ever attempting
    /// a workspace read. `message` is fed back to the model as the tool
    /// response so it tries again instead of finalizing the run.
    RejectBlockedWithoutTrying { message: String },
}

/// Decide whether a `submit_result` payload should be accepted or sent back
/// to the model for another attempt. The only currently-enforced rule is
/// the "blocked without trying" guard: if the role had `workspace_read`
/// provisioned but the subagent never called any of the workspace tools
/// and now wants to return `status:"blocked"`, we reject so the model can
/// be forced to verify access first.
fn validate_submit(
    args: &Value,
    tools_called: &std::collections::HashSet<String>,
    has_workspace_read: bool,
) -> SubmitVerdict {
    let status = args.get("status").and_then(|v| v.as_str()).unwrap_or("");
    if status != "blocked" || !has_workspace_read {
        return SubmitVerdict::Accept;
    }
    let attempted = WORKSPACE_READ_TOOLS
        .iter()
        .any(|t| tools_called.contains(*t));
    if attempted {
        return SubmitVerdict::Accept;
    }
    SubmitVerdict::RejectBlockedWithoutTrying {
        message: "Rejected: you submitted status:\"blocked\" without ever calling \
            list_workspace_files, read_workspace_file, or workspace_search. These tools \
            ARE provisioned in this run and ARE present in your tool schema. \
            Call `list_workspace_files` with {\"path\":\".\"} now to verify access, \
            then proceed with the task. Do not return a blocked status until you have \
            actually attempted a workspace read and reported the concrete error string."
            .to_owned(),
    }
}

fn blocked_result(role: SubagentRole, display_name: &str, summary: &str) -> Value {
    json!({
        "status": "blocked",
        "role": subagent_prompts::role_id(role),
        "displayName": display_name,
        "summary": summary,
        "steps": [],
        "findings": [],
        "artifacts": [],
        "recommendedNextActions": [],
    })
}

/// Anthropic Messages-API SSE event shapes. Only the subset we need to drive
/// per-agent deltas + reconstruct the assistant message blocks for the next
/// round of the loop.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthroStreamEvent {
    ContentBlockStart {
        index: u32,
        content_block: AnthroBlockStart,
    },
    ContentBlockDelta {
        index: u32,
        delta: AnthroBlockDelta,
    },
    #[allow(dead_code)]
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: AnthroMessageDeltaInner,
        #[serde(default)]
        usage: Option<AnthroStreamUsage>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthroBlockStart {
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
enum AnthroBlockDelta {
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        thinking: String,
    },
    #[allow(dead_code)]
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
struct AnthroMessageDeltaInner {
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct AnthroStreamUsage {
    #[serde(default)]
    output_tokens: Option<u64>,
}

#[derive(Default)]
struct AnthroBlockState {
    kind: AnthroBlockKind,
    text: String,
    tool_id: String,
    tool_name: String,
    tool_args: String,
}

#[derive(Default, PartialEq, Eq)]
enum AnthroBlockKind {
    #[default]
    Unknown,
    Text,
    Thinking,
    Tool,
}

#[derive(Default)]
struct AnthropicRoundResult {
    text: String,
    assistant_blocks: Vec<Value>,
    tool_uses: Vec<(String, String, String)>,
    stop_reason: Option<String>,
    output_tokens: Option<u64>,
}

async fn stream_anthropic_subagent_round(
    state: &Arc<AgentEngineState>,
    client: &reqwest::Client,
    api_key: &str,
    body: &Value,
    agent_id: &str,
) -> Result<AnthropicRoundResult, String> {
    let resp = client
        .post(ANTHROPIC_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?;
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
        return Err(format!("HTTP {status}: {trimmed}"));
    }

    let stream = resp.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(
        stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
    );
    let mut lines = tokio::io::BufReader::new(reader).lines();

    let mut acc = AnthropicRoundResult::default();
    let mut blocks: Vec<AnthroBlockState> = Vec::new();
    let mut thinking_active = false;
    let mut thinking_closed = false;

    loop {
        if state.cancelled() {
            return Err("cancelled".into());
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
        let event: AnthroStreamEvent = match serde_json::from_str(payload) {
            Ok(e) => e,
            Err(_) => continue,
        };
        match event {
            AnthroStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let idx = index as usize;
                while blocks.len() <= idx {
                    blocks.push(AnthroBlockState::default());
                }
                let slot = &mut blocks[idx];
                match content_block {
                    AnthroBlockStart::Text { .. } => slot.kind = AnthroBlockKind::Text,
                    AnthroBlockStart::Thinking { .. } => {
                        slot.kind = AnthroBlockKind::Thinking;
                        thinking_active = true;
                    }
                    AnthroBlockStart::ToolUse { id, name } => {
                        slot.kind = AnthroBlockKind::Tool;
                        slot.tool_id = id;
                        slot.tool_name = from_anthropic_name(&name);
                        if thinking_active && !thinking_closed {
                            thinking_closed = true;
                            state.push(AgentEvent::SubagentThinkingDone {
                                agent_id: agent_id.to_owned(),
                            });
                        }
                    }
                    AnthroBlockStart::Other => {}
                }
            }
            AnthroStreamEvent::ContentBlockDelta { index, delta } => {
                let idx = index as usize;
                if idx >= blocks.len() {
                    continue;
                }
                let slot = &mut blocks[idx];
                match delta {
                    AnthroBlockDelta::TextDelta { text } => {
                        if !text.is_empty() {
                            if thinking_active && !thinking_closed {
                                thinking_closed = true;
                                state.push(AgentEvent::SubagentThinkingDone {
                                    agent_id: agent_id.to_owned(),
                                });
                            }
                            state.push(AgentEvent::SubagentAssistantDelta {
                                agent_id: agent_id.to_owned(),
                                delta: text.clone(),
                            });
                            slot.text.push_str(&text);
                            acc.text.push_str(&text);
                        }
                    }
                    AnthroBlockDelta::ThinkingDelta { thinking } => {
                        if !thinking.is_empty() {
                            state.push(AgentEvent::SubagentThinkingDelta {
                                agent_id: agent_id.to_owned(),
                                delta: thinking,
                            });
                        }
                    }
                    AnthroBlockDelta::SignatureDelta { .. } => {}
                    AnthroBlockDelta::InputJsonDelta { partial_json } => {
                        slot.tool_args.push_str(&partial_json);
                    }
                    AnthroBlockDelta::Other => {}
                }
            }
            AnthroStreamEvent::ContentBlockStop { .. } => {}
            AnthroStreamEvent::MessageDelta { delta, usage } => {
                if let Some(reason) = delta.stop_reason {
                    acc.stop_reason = Some(reason);
                }
                if let Some(u) = usage.and_then(|u| u.output_tokens) {
                    acc.output_tokens = Some(u);
                }
            }
            AnthroStreamEvent::Other => {}
        }
    }

    if thinking_active && !thinking_closed {
        state.push(AgentEvent::SubagentThinkingDone {
            agent_id: agent_id.to_owned(),
        });
    }

    for slot in blocks {
        match slot.kind {
            AnthroBlockKind::Text => {
                if !slot.text.is_empty() {
                    acc.assistant_blocks
                        .push(json!({ "type": "text", "text": slot.text }));
                }
            }
            AnthroBlockKind::Tool => {
                let input: Value = if slot.tool_args.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&slot.tool_args).unwrap_or(json!({}))
                };
                acc.assistant_blocks.push(json!({
                    "type": "tool_use",
                    "id": slot.tool_id,
                    "name": to_anthropic_name(&slot.tool_name),
                    "input": input.clone(),
                }));
                let args_str = if slot.tool_args.trim().is_empty() {
                    "{}".to_string()
                } else {
                    slot.tool_args
                };
                acc.tool_uses.push((slot.tool_id, slot.tool_name, args_str));
            }
            AnthroBlockKind::Thinking | AnthroBlockKind::Unknown => {
                // Thinking blocks must be echoed back in some Anthropic flows,
                // but the subagent loop never needs to replay them — discard.
            }
        }
    }

    Ok(acc)
}

/// Stream one OpenAI-compatible chat-completion round and emit per-agent
/// `SubagentAssistantDelta` / `SubagentThinkingDelta` events as the bytes
/// arrive. Returns the aggregated text + tool calls so the caller can
/// replay them into the next round of the conversation.
async fn stream_openai_subagent_round(
    state: &Arc<AgentEngineState>,
    client: &reqwest::Client,
    endpoint: Endpoint,
    api_key: &str,
    body: &Value,
    agent_id: &str,
) -> Result<OpenAiRoundResult, String> {
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
        .map_err(|e| format!("request: {e}"))?;
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
        return Err(format!("HTTP {status}: {trimmed}"));
    }

    let stream = resp.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(
        stream.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
    );
    let mut lines = tokio::io::BufReader::new(reader).lines();

    let mut acc = OpenAiRoundResult::default();
    let mut tool_by_index: Vec<OpenAiAggregatedCall> = Vec::new();
    let mut thinking_active = false;
    let mut thinking_closed = false;

    loop {
        if state.cancelled() {
            return Err("cancelled".into());
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
        let chunk: OpenAiStreamChunk = match serde_json::from_str(payload) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(u) = chunk.usage.and_then(|u| u.completion_tokens) {
            acc.completion_tokens = Some(u);
        }
        for choice in chunk.choices {
            let reasoning_chunk = choice.delta.reasoning.or(choice.delta.reasoning_content);
            if let Some(reasoning) = reasoning_chunk {
                if !reasoning.is_empty() {
                    thinking_active = true;
                    state.push(AgentEvent::SubagentThinkingDelta {
                        agent_id: agent_id.to_owned(),
                        delta: reasoning,
                    });
                }
            }
            if let Some(text) = choice.delta.content {
                if !text.is_empty() {
                    if thinking_active && !thinking_closed {
                        thinking_closed = true;
                        state.push(AgentEvent::SubagentThinkingDone {
                            agent_id: agent_id.to_owned(),
                        });
                    }
                    state.push(AgentEvent::SubagentAssistantDelta {
                        agent_id: agent_id.to_owned(),
                        delta: text.clone(),
                    });
                    acc.text.push_str(&text);
                }
            }
            for tc in choice.delta.tool_calls {
                let idx = tc.index as usize;
                while tool_by_index.len() <= idx {
                    tool_by_index.push(OpenAiAggregatedCall::default());
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
            }
            let _ = choice.finish_reason;
        }
    }

    if thinking_active && !thinking_closed {
        state.push(AgentEvent::SubagentThinkingDone {
            agent_id: agent_id.to_owned(),
        });
    }

    acc.tool_calls = tool_by_index
        .into_iter()
        .filter(|c| !c.name.is_empty())
        .collect();
    Ok(acc)
}
