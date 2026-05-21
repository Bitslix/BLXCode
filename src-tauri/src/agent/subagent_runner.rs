//! Provider-specific subagent HTTP loops (OpenAI-compatible + Anthropic).

use crate::agent::anthropic::{from_anthropic_name, to_anthropic_name};
use crate::agent::openrouter::Endpoint;
use crate::agent::protocol::AgentEvent;
use crate::agent::state::AgentEngineState;
use crate::agent::subagent_prompts::{self, SubagentRole, truncate_submit_result};
use crate::agent::tool_groups::ToolGroup;
use crate::agent::tools::{self, WorkspaceRootGuard};
use crate::agent::tool_dispatch::DispatchContext;
use crate::agent_settings::AgentProviderKind;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

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

#[derive(Debug, Deserialize)]
struct OpenAiCompletion {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAiToolCall {
    id: String,
    function: OpenAiFn,
}

#[derive(Debug, Deserialize)]
struct OpenAiFn {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    #[serde(default)]
    completion_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessage {
    #[serde(default)]
    content: Vec<AnthropicBlock>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    output_tokens: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AnthropicBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<Value>,
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
                    "stream": false,
                });
                let mut req = client
                    .post(endpoint.url())
                    .bearer_auth(&ctx.api_key)
                    .header("Content-Type", "application/json");
                if matches!(endpoint, Endpoint::Openrouter) {
                    req = req
                        .header("HTTP-Referer", "https://bitslix.com/blxcode")
                        .header("X-Title", "blxcode");
                }
                let text = match req.json(&body).send().await.and_then(|r| r.error_for_status()) {
                    Ok(r) => match r.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            let result = blocked_result(role, display_name, &format!("read body: {e}"));
                            finish_subagent(state, agent_id, &result);
                            return result;
                        }
                    },
                    Err(e) => {
                        let result = blocked_result(role, display_name, &format!("request failed: {e}"));
                        finish_subagent(state, agent_id, &result);
                        return result;
                    }
                };
                let parsed: OpenAiCompletion = match serde_json::from_str(&text) {
                    Ok(p) => p,
                    Err(e) => {
                        let result = blocked_result(role, display_name, &format!("parse response: {e}"));
                        finish_subagent(state, agent_id, &result);
                        return result;
                    }
                };
                let Some(choice) = parsed.choices.into_iter().next() else {
                    let result = blocked_result(role, display_name, "empty choices");
                    finish_subagent(state, agent_id, &result);
                    return result;
                };
                if let Some(u) = parsed.usage.and_then(|u| u.completion_tokens) {
                    output_estimate = output_estimate.saturating_add(u as usize);
                } else if let Some(c) = &choice.message.content {
                    output_estimate += c.len().div_ceil(4);
                }
                if output_estimate > MAX_OUTPUT_TOKENS_ESTIMATE {
                    let result = blocked_result(role, display_name, "output token cap reached");
                    finish_subagent(state, agent_id, &result);
                    return result;
                }
                let mut assistant = json!({ "role": "assistant" });
                if let Some(c) = &choice.message.content {
                    assistant["content"] = Value::String(c.clone());
                }
                if !choice.message.tool_calls.is_empty() {
                    assistant["tool_calls"] = Value::Array(
                        choice.message.tool_calls.iter().map(|tc| {
                            json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.function.name,
                                    "arguments": tc.function.arguments,
                                }
                            })
                        }).collect(),
                    );
                }
                messages.push(assistant);
                if choice.message.tool_calls.is_empty() {
                    break;
                }
                for tc in choice.message.tool_calls {
                    if handle_tool_call(
                        state,
                        agent_id,
                        &tc.id,
                        &tc.function.name,
                        &tc.function.arguments,
                        groups,
                        root_guard.as_ref(),
                        &mut final_submit,
                    ) {
                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": tc.id,
                            "content": "submit_result accepted",
                        }));
                        break;
                    }
                    let args: Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                    let outcome = execute_subagent_tool(&tc.function.name, &args, groups, root_guard.as_ref());
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tc.id,
                        "content": outcome.content,
                    }));
                }
                if final_submit.is_some() {
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
                });
                let resp = client
                    .post(ANTHROPIC_URL)
                    .header("x-api-key", &ctx.api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await;
                let text = match resp.and_then(|r| r.error_for_status()) {
                    Ok(r) => match r.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            let result = blocked_result(role, display_name, &format!("read body: {e}"));
                            finish_subagent(state, agent_id, &result);
                            return result;
                        }
                    },
                    Err(e) => {
                        let result = blocked_result(role, display_name, &format!("request failed: {e}"));
                        finish_subagent(state, agent_id, &result);
                        return result;
                    }
                };
                let parsed: AnthropicMessage = match serde_json::from_str(&text) {
                    Ok(p) => p,
                    Err(e) => {
                        let result = blocked_result(role, display_name, &format!("parse response: {e}"));
                        finish_subagent(state, agent_id, &result);
                        return result;
                    }
                };
                if let Some(u) = parsed.usage.and_then(|u| u.output_tokens) {
                    output_estimate = output_estimate.saturating_add(u as usize);
                }
                if output_estimate > MAX_OUTPUT_TOKENS_ESTIMATE {
                    let result = blocked_result(role, display_name, "output token cap reached");
                    finish_subagent(state, agent_id, &result);
                    return result;
                }
                let mut tool_uses: Vec<(String, String, String)> = Vec::new();
                let mut assistant_blocks: Vec<Value> = Vec::new();
                for block in &parsed.content {
                    match block.kind.as_str() {
                        "text" => {
                            if let Some(t) = &block.text {
                                assistant_blocks.push(json!({ "type": "text", "text": t }));
                                output_estimate += t.len().div_ceil(4);
                            }
                        }
                        "tool_use" => {
                            let id = block.id.clone().unwrap_or_default();
                            let name = block
                                .name
                                .as_ref()
                                .map(|n| from_anthropic_name(n))
                                .unwrap_or_default();
                            let args = block
                                .input
                                .clone()
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "{}".into());
                            tool_uses.push((id.clone(), name.clone(), args.clone()));
                            assistant_blocks.push(json!({
                                "type": "tool_use",
                                "id": id,
                                "name": to_anthropic_name(&name),
                                "input": block.input.clone().unwrap_or(json!({})),
                            }));
                        }
                        _ => {}
                    }
                }
                if !assistant_blocks.is_empty() {
                    messages.push(json!({ "role": "assistant", "content": assistant_blocks }));
                }
                if tool_uses.is_empty() {
                    break;
                }
                let mut result_blocks: Vec<Value> = Vec::new();
                for (id, name, args_str) in tool_uses {
                    if handle_tool_call(
                        state,
                        agent_id,
                        &id,
                        &name,
                        &args_str,
                        groups,
                        root_guard.as_ref(),
                        &mut final_submit,
                    ) {
                        result_blocks.push(json!({
                            "type": "tool_result",
                            "tool_use_id": id,
                            "content": "submit_result accepted",
                        }));
                        break;
                    }
                    let args: Value = serde_json::from_str(&args_str).unwrap_or(json!({}));
                    let outcome = execute_subagent_tool(&name, &args, groups, root_guard.as_ref());
                    result_blocks.push(json!({
                        "type": "tool_result",
                        "tool_use_id": id,
                        "content": outcome.content,
                        "is_error": !outcome.ok,
                    }));
                }
                messages.push(json!({ "role": "user", "content": result_blocks }));
                if final_submit.is_some() {
                    break;
                }
                if parsed.stop_reason.as_deref() == Some("end_turn") {
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

fn handle_tool_call(
    state: &Arc<AgentEngineState>,
    agent_id: &str,
    call_id: &str,
    name: &str,
    args_str: &str,
    groups: &[ToolGroup],
    root: Option<&WorkspaceRootGuard>,
    final_submit: &mut Option<Value>,
) -> bool {
    state.push(AgentEvent::SubagentToolCall {
        agent_id: agent_id.to_owned(),
        tool: name.to_owned(),
        call_id: Some(call_id.to_owned()),
        args: serde_json::from_str(args_str).ok(),
    });
    if name == "submit_result" {
        let args: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
        *final_submit = Some(truncate_submit_result(args));
        return true;
    }
    let _ = (groups, root);
    false
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
