//! Coordinated subagent runs via `subagents.run`.

use crate::agent::openrouter::Endpoint;
use crate::agent::protocol::AgentEvent;
use crate::agent::state::AgentEngineState;
use crate::agent::subagent_prompts::{
    self, SubagentRole, truncate_submit_result,
};
use crate::agent::tool_dispatch::DispatchContext;
use crate::agent::tool_groups::{self, parse_allowed_groups, ToolGroup};
use crate::agent::tools::{self, ToolOutcome, WorkspaceRootGuard};
use crate::agent_settings::AgentProviderKind;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

const MAX_AGENTS: usize = 5;
const DEFAULT_CONCURRENCY: usize = 3;
const MAX_SUBAGENT_ROUNDS: u32 = 8;
const MAX_OUTPUT_TOKENS_ESTIMATE: usize = 20_000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunSpec {
    agents: Vec<AgentSpec>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    max_concurrency: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentSpec {
    id: String,
    role: String,
    #[serde(default)]
    title: Option<String>,
    task: String,
    #[serde(default)]
    success_criteria: Vec<String>,
    #[serde(default)]
    allowed_tool_groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletion {
    choices: Vec<ChatChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Deserialize)]
struct ToolCall {
    id: String,
    function: ToolFn,
}

#[derive(Debug, Deserialize)]
struct ToolFn {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[serde(default)]
    completion_tokens: Option<u64>,
}

pub async fn run(
    state: &Arc<AgentEngineState>,
    args: &Value,
    root: Option<&WorkspaceRootGuard>,
    ctx: &DispatchContext,
) -> ToolOutcome {
    let spec: RunSpec = match serde_json::from_value(args.clone()) {
        Ok(s) => s,
        Err(e) => {
            return ToolOutcome {
                ok: false,
                content: format!("invalid subagents.run args: {e}"),
            };
        }
    };
    if spec.agents.is_empty() {
        return ToolOutcome {
            ok: false,
            content: "agents array is empty".into(),
        };
    }
    if spec.agents.len() > MAX_AGENTS {
        return ToolOutcome {
            ok: false,
            content: format!("max {MAX_AGENTS} agents per run"),
        };
    }

    let endpoint = match Endpoint::from_provider(ctx.settings.provider) {
        Some(e) => e,
        None if ctx.settings.provider == AgentProviderKind::Anthropic => {
            return ToolOutcome {
                ok: false,
                content: "subagents.run in v1 requires OpenRouter or OpenAI provider".into(),
            };
        }
        None => {
            return ToolOutcome {
                ok: false,
                content: "unsupported provider for subagents".into(),
            };
        }
    };

    let concurrency = spec
        .max_concurrency
        .unwrap_or(DEFAULT_CONCURRENCY as u32)
        .min(MAX_AGENTS as u32) as usize;
    let sem = Arc::new(Semaphore::new(concurrency.max(1)));
    let web_enabled = crate::agent::web_tools::web_tools_enabled();

    let role_specs: Vec<(SubagentRole, Option<String>)> = spec
        .agents
        .iter()
        .filter_map(|a| {
            SubagentRole::parse(&a.role).map(|r| (r, a.title.clone()))
        })
        .collect();
    let display_names = subagent_prompts::resolve_display_names(&role_specs);

    let mut handles = Vec::new();
    for (idx, agent) in spec.agents.iter().enumerate() {
        let role = match SubagentRole::parse(&agent.role) {
            Some(r) => r,
            None => {
                return ToolOutcome {
                    ok: false,
                    content: format!("unknown role: {}", agent.role),
                };
            }
        };
        let display = display_names
            .get(idx)
            .cloned()
            .unwrap_or_else(|| subagent_prompts::display_name_en(role).to_owned());
        let groups = if agent.allowed_tool_groups.is_empty() {
            role.default_groups()
        } else {
            parse_allowed_groups(&agent.allowed_tool_groups)
        };
        let mut groups = groups;
        groups.push(ToolGroup::SubagentSubmit);
        groups.retain(|g| !matches!(g, ToolGroup::SubagentsRun | ToolGroup::ShellWrite));

        let permit = sem.clone().acquire_owned().await.ok();
        let state_c = Arc::clone(state);
        let ctx_c = ctx.clone();
        let root_s = root.map(|r| r.as_str());
        let agent = agent.clone();
        let display_c = display.clone();
        let handle = tokio::spawn(async move {
            let _permit = permit;
            run_one_subagent(
                &state_c,
                &ctx_c,
                endpoint,
                root_s.as_deref(),
                &agent.id,
                role,
                &display_c,
                &agent.task,
                &agent.success_criteria,
                &groups,
                web_enabled,
            )
            .await
        });
        handles.push(handle);
    }

    let mut results = Vec::new();
    for h in handles {
        match h.await {
            Ok(r) => results.push(r),
            Err(e) => results.push(json!({
                "status": "failed",
                "summary": format!("subagent task join error: {e}"),
            })),
        }
    }

    ToolOutcome {
        ok: true,
        content: serde_json::to_string(&json!({ "agents": results })).unwrap_or_default(),
    }
}

async fn run_one_subagent(
    state: &Arc<AgentEngineState>,
    ctx: &DispatchContext,
    endpoint: Endpoint,
    workspace_root: Option<&str>,
    agent_id: &str,
    role: SubagentRole,
    display_name: &str,
    task: &str,
    success_criteria: &[String],
    groups: &[ToolGroup],
    web_enabled: bool,
) -> Value {
    state.push(AgentEvent::SubagentStarted {
        agent_id: agent_id.to_owned(),
        role: subagent_prompts::role_id(role).to_owned(),
        display_name: display_name.to_owned(),
    });

    let root_guard = workspace_root.and_then(|r| WorkspaceRootGuard::parse(r).ok().flatten());
    let ws = workspace_root.unwrap_or("<no workspace>");
    let system = subagent_prompts::subagent_system_prompt(ws, role, display_name, task, success_criteria);
    let tools = tool_groups::render_for_openai_filtered(groups, web_enabled);

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

    let mut messages = vec![
        json!({ "role": "system", "content": system }),
        json!({ "role": "user", "content": task }),
    ];

    let mut output_estimate = 0usize;
    let mut iterations = 0u32;
    let mut final_submit: Option<Value> = None;

    for round in 0..MAX_SUBAGENT_ROUNDS {
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
            "tools": tools,
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

        let resp = match req.json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                let result = blocked_result(role, display_name, &format!("request failed: {e}"));
                finish_subagent(state, agent_id, &result);
                return result;
            }
        };

        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                let result = blocked_result(role, display_name, &format!("read body: {e}"));
                finish_subagent(state, agent_id, &result);
                return result;
            }
        };

        let parsed: ChatCompletion = match serde_json::from_str(&text) {
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
                choice.message
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments,
                            }
                        })
                    })
                    .collect(),
            );
        }
        messages.push(assistant);

        if choice.message.tool_calls.is_empty() {
            break;
        }

        for tc in choice.message.tool_calls {
            state.push(AgentEvent::SubagentToolCall {
                agent_id: agent_id.to_owned(),
                tool: tc.function.name.clone(),
                call_id: Some(tc.id.clone()),
                args: serde_json::from_str(&tc.function.arguments).ok(),
            });

            if tc.function.name == "submit_result" {
                let args: Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                final_submit = Some(truncate_submit_result(args));
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": "submit_result accepted",
                }));
                break;
            }

            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
            let shell_write = groups.contains(&ToolGroup::ShellWrite);
            let outcome = if tc.function.name == "shell_exec" {
                tools::execute_server_tool(
                    &tc.function.name,
                    &args,
                    root_guard.as_ref(),
                    Some(tools::ToolExecOpts {
                        shell_writes: shell_write,
                    }),
                )
            } else {
                tools::execute_server_tool(&tc.function.name, &args, root_guard.as_ref(), None)
            };

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
