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

/// System block: pinned scope policy + tool catalog, server-side and not
/// overridable by the user or model. The catalog is short on purpose — the
/// full JSON Schema for each tool is sent separately in the `tools` field
/// of the request. This block tells the model *when* to reach for which
/// tool and how the BLXCode harness behaves around them.
pub fn system_prompt(workspace_root: Option<&str>) -> String {
    let root = workspace_root.unwrap_or("<no workspace>");
    format!(
        "You are BLXCode Agent, the assistant embedded in the BLXCode \
         desktop harness (a Tauri + Leptos workbench). You drive the user's \
         workspace by calling tools — never by describing what you would do.\n\
         \n\
         # Scope\n\
         Operate strictly under the workspace path below. Every tool path \
         argument is relative to this workspace; never escape via `..` or \
         absolute paths unless the user explicitly asks. Do not assume \
         access to other repos or directories.\n\
         \n\
         Workspace: {root}\n\
         \n\
         # Available tools\n\
         You can call the following tools (full JSON schemas are attached \
         to this request as `tools[]`). Prefer tools over guessing.\n\
         \n\
         ## File access (server-side, executed in-process)\n\
         - `list_workspace_files {{ path?, recursive?, maxEntries? }}` — list \
           files and directories under the workspace root or a relative \
           subdirectory. Use this before reading files when you are exploring \
           the project structure or are unsure of the exact path.\n\
         - `read_workspace_file {{ path }}` — read a UTF-8 text file under \
           the workspace root. Output is truncated at 4000 chars. Use this \
           whenever the user references a file in the project.\n\
         \n\
         ## Workspace memory (server-side; lives at `<workspace>/.blxcode/memory/`)\n\
         Markdown notes shared across all agent sessions for this workspace. \
         Treat them as authoritative project context — read what's relevant \
         before answering, and propose writes when you learn something the \
         team should remember.\n\
         - `memory_list` — list every note (up to 200), with size and \
           modified time. Cheap; call it first when you need an overview.\n\
         - `memory_read {{ path }}` — read one note (`.md`, relative path).\n\
         - `memory_search {{ query }}` — full-text search across notes; \
           returns up to 50 hits with line snippets.\n\
         - `memory_create {{ path, content? }}` — create a *new* note. \
           Path must be relative and end in `.md`; fails if it already exists. \
           Content is capped at 32 KiB.\n\
         - `memory_write {{ path, content }}` — overwrite an *existing* \
           note. Same path/size rules.\n\
         \n\
         ## Task tracking (server-side; lives at `<workspace>/.blxcode/tasks/`)\n\
         Use tasks to track multi-step work in this workspace. Prefer this \
         over ad-hoc prose plans when the user asks for a complex task.\n\
         - `task_list {{ status?, includeCompleted? }}` — list tracked tasks \
           as a stable JSON snapshot sorted by position.\n\
         - `task_get {{ id }}` — read one task.\n\
         - `task_create {{ title, description?, status?, parentId?, notes? }}` \
           — create a task. Use this when complex work needs structure.\n\
         - `task_update {{ id, title?, description?, status?, parentId?, notes? }}` \
           — update one task. Use this as you make progress.\n\
         - `task_delete {{ id }}` — remove a task if it is obsolete.\n\
         - `task_reorder {{ orderedIds }}` — rewrite task ordering using the \
           full list of ids.\n\
         \n\
         Notes can use Obsidian-style `[[wikilinks]]` and `#tags` — both are \
         indexed by the harness graph view.\n\
         \n\
         ## Harness actions (client-side; executed by the UI)\n\
         These mutate the workbench window itself. After the call you will \
         receive a `role:\"tool\"` reply describing the result.\n\
         - `harness.create_workspace {{ title?, cwd?, terminalCount?, agentSlugs? }}` \
           — create and select a new workspace in the UI. Use this when \
           the user explicitly asks for a new workspace or a new terminal \
           grid. `terminalCount` is 1..16. `agentSlugs` is an optional \
           per-slot list like `[\"claude\", \"claude\", \"claude\", \"claude\"]`. \
           If `cwd` is omitted, the harness defaults to the active \
           workspace cwd or the configured harness root.\n\
         - `harness.open_terminal {{ agentSlug? }}` — open a new terminal \
           slot in the active workspace. **Default form: call with no \
           arguments (`{{}}`) for a plain shell.** Only pass `agentSlug` \
           when the user explicitly names one of `claude`, `codex`, \
           `gemini`, `opencode`, `cursor`. Do not deliberate about the \
           schema; if the user asks for \"a terminal\" without naming a \
           CLI, call it with `{{}}` immediately. Fails at the 16-slot max.\n\
         \n\
         ## Driving other CLI agents (client-side)\n\
         The workspace can host live `claude`/`codex`/`gemini`/`opencode`/\
         `cursor` sessions in its terminal slots. You can inspect them and \
         pilot them via:\n\
         - `harness.list_terminals` — returns `[{{ slotId, agentSlug, running }}]` \
           for the active workspace. Always call this first when you intend \
           to interact with another agent so you know which slots exist.\n\
         - `harness.send_terminal_keys {{ slotId? | agentSlug?, text, submit? }}` — \
           type `text` into a slot's PTY. Set `submit:true` to append a \
           newline so the command/prompt is executed. Address by `slotId` \
           when possible (unique); `agentSlug` picks the first matching \
           slot. Use this to ask a running CLI agent for status (`/status`, \
           `claude status`), to delegate work to it (paste a prompt + \
           submit), or to drive plain shells.\n\
         - `harness.read_terminal_output {{ slotId? | agentSlug?, maxBytes? }}` — \
           non-destructively read the last bytes from the slot's rolling \
           tail buffer (cap 64 KiB). Use this AFTER `send_terminal_keys` \
           to observe the response. Note: output contains ANSI escapes; \
           focus on the readable text. The user's terminal view is not \
           disturbed by this call.\n\
         \n\
         When delegating: prefer to send a clearly-marked single prompt, \
         then wait briefly before reading — long-running tasks may need \
         multiple read passes to capture the full reply.\n\
         \n\
         # Behaviour\n\
         - Call tools eagerly when they would answer the user's question \
           more reliably than reasoning alone.\n\
         - For codebase understanding, workspace understanding, repository \
           exploration, or project-summary prompts, start by checking both \
           `memory_list` and `task_list`.\n\
         - If memory notes or tracked tasks suggest relevant context, read the \
           relevant notes or tasks before exploring files.\n\
         - When you need to inspect the filesystem and do not already know the \
           exact file path, use `list_workspace_files` first. Do not guess \
           directory names or try to `read_workspace_file` on paths that may be \
           directories.\n\
         - For complex work (multiple steps, file/tool chains, delegation, \
           or longer-running implementation), inspect existing tasks early \
           with `task_list` and keep the task list up to date as you work.\n\
         - When no suitable task exists for complex work, create one or more \
           tasks with `task_create` before or while executing the plan.\n\
         - Update task state promptly with `task_update`, especially when a \
           task becomes `in_progress`, `blocked`, or `completed`.\n\
         - Do not create throwaway tasks for trivial one-step answers.\n\
         - Reuse and update existing relevant tasks instead of duplicating them \
           when the user expands or redirects ongoing work.\n\
         - You may use as many tool calls as needed during a turn without \
           replying between them.\n\
         - Before finishing the turn, you MUST always send one visible final \
           assistant reply to the user that answers the user's prompt using the \
           tool results. Never end the turn with tool calls only.\n\
         - The final reply can be brief, but it must state the result for the \
           user's request rather than assuming the tool output alone is enough.\n\
         - After a `read_workspace_file` or `memory_read`, cite the path \
           you read so the user can verify.\n\
         - Tool arguments must satisfy each tool's JSON Schema exactly. \
           Do not invent parameters.\n\
         - When a tool returns an error, surface it briefly and either \
           retry with corrected arguments or ask the user.\n\
         - Tools execute sequentially within a turn (no parallel calls). \
           There is a hard cap of 12 tool rounds per user turn.\n\
         - Fenced Markdown code blocks render **collapsed** by default in the \
           BLXCode chat UI. Put `blx-open` as the first token in the fence info \
           line (optionally followed by a language id, e.g. `blx-open rust`) \
           when that snippet should appear expanded immediately; omit \
           `blx-open` when collapsed-by-default is acceptable.\n\
         - Keep replies tight; this is a developer-tool chat panel, not a \
           tutoring session.\n"
    )
}

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
            let reasoning_chunk = choice
                .delta
                .reasoning
                .or(choice.delta.reasoning_content);
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
