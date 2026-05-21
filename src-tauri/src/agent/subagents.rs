//! Coordinated subagent runs via `subagents.run`.

use crate::agent::subagent_prompts::{self, SubagentRole};
use crate::agent::subagent_runner::{run_one_subagent, SubagentProvider};
use crate::agent::state::AgentEngineState;
use crate::agent::tool_dispatch::DispatchContext;
use crate::agent::tool_groups::{self, parse_allowed_groups, ToolGroup};
use crate::agent::tools::{ToolOutcome, WorkspaceRootGuard};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Semaphore;

const MAX_AGENTS: usize = 5;
const DEFAULT_CONCURRENCY: usize = 3;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunSpec {
    agents: Vec<AgentSpec>,
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

    let provider = match SubagentProvider::from_settings(ctx.settings.provider) {
        Some(p) => p,
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
    let web_enabled = crate::agent::web_settings::web_tools_enabled();

    let role_specs: Vec<(SubagentRole, Option<String>)> = spec
        .agents
        .iter()
        .filter_map(|a| SubagentRole::parse(&a.role).map(|r| (r, a.title.clone())))
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

        let tools_openai =
            tool_groups::render_for_openai_filtered(&groups, web_enabled);
        let mut tools_anthropic =
            tool_groups::render_for_anthropic_filtered(&groups, web_enabled);
        if let Some(arr) = tools_anthropic.as_array_mut() {
            for entry in arr {
                if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                    entry["name"] = Value::String(crate::agent::anthropic::to_anthropic_name(name));
                }
            }
        }

        let permit = sem.clone().acquire_owned().await.ok();
        let state_c = Arc::clone(state);
        let ctx_c = ctx.clone();
        let root_s = root.map(|r| r.as_str());
        let agent = agent.clone();
        let display_c = display.clone();
        let provider_c = provider;
        let handle = tokio::spawn(async move {
            let _permit = permit;
            run_one_subagent(
                &state_c,
                &ctx_c,
                provider_c,
                root_s.as_deref(),
                &agent.id,
                role,
                &display_c,
                &agent.task,
                &agent.success_criteria,
                &groups,
                tools_openai,
                tools_anthropic,
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
