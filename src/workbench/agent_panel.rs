//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
use crate::agent_wire::{AgentEvent, UserTurn};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_abort, agent_drain_turn, agent_settings_get, agent_submit_tool_result, agent_submit_turn,
    is_tauri_shell, pty_peek_output, pty_write,
};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[component]
pub fn AgentPanelDock() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let draft = RwSignal::new(String::new());
    let timeline = RwSignal::new(Vec::<TimelineItem>::new());
    let busy = RwSignal::new(false);
    let status_line = RwSignal::new(Option::<String>::None);
    let ptt_active = RwSignal::new(false);
    let tasks_open = RwSignal::new(true);
    let model_label = RwSignal::new(String::new());

    // Load the configured provider/model once for the status badge so the
    // user can verify their harness settings are actually being applied.
    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                model_label.set(format!("{}/{}", view.provider.as_str(), view.model_id));
            }
        });
    }

    view! {
        <section class="workbench-agent-pane" aria-label=move || i18n.tr(I18nKey::AgAriaPane)()>
            <header class="agent-hero">
                <button
                    type="button"
                    class="agent-hero__orb"
                    class:agent-hero__orb--active=move || ptt_active.get()
                    aria-pressed=move || ptt_active.get().to_string()
                    aria-label="Talk to BLXCode Agent"
                    on:mousedown=move |_| ptt_active.set(true)
                    on:mouseup=move |_| ptt_active.set(false)
                    on:mouseleave=move |_| ptt_active.set(false)
                    on:keydown=move |ev| {
                        if ev.key() == " " || ev.key() == "Enter" {
                            ev.prevent_default();
                            ptt_active.set(true);
                        }
                    }
                    on:keyup=move |_| ptt_active.set(false)
                >
                    <span class="agent-hero__logo">"B"</span>
                </button>
                <div class="agent-hero__meta">
                    <p class="agent-hero__eyebrow">"BLXCode Agent"</p>
                    <h2>{move || if busy.get() { "Running" } else { "Standby" }}</h2>
                    <p>
                        {move || {
                            let m = model_label.get();
                            if m.is_empty() { "Configure a provider in harness settings".to_string() } else { m }
                        }}
                    </p>
                </div>
            </header>

            <section class="agent-section agent-section--tasks" aria-labelledby="agent-tasks-title">
                <button
                    type="button"
                    class="agent-section__head agent-section__head--toggle"
                    aria-expanded=move || tasks_open.get().to_string()
                    aria-controls="agent-task-list"
                    on:click=move |_| tasks_open.update(|open| *open = !*open)
                >
                    <h3 id="agent-tasks-title">"Tasks"</h3>
                    <span>
                        {move || if busy.get() { "Running" } else { "Idle" }}
                        <span class="agent-section__chev" aria-hidden="true">
                            {move || if tasks_open.get() { "⌃" } else { "⌄" }}
                        </span>
                    </span>
                </button>
                <Show when=move || tasks_open.get()>
                    <ol id="agent-task-list" class="agent-task-list">
                        <li class="agent-task agent-task--active">
                            <span class="agent-task__mark" aria-hidden="true"></span>
                            <div>
                                <strong>"Wait for next instruction"</strong>
                                <small>"The agent will turn new prompts into tracked work."</small>
                            </div>
                        </li>
                        <li class="agent-task">
                            <span class="agent-task__mark" aria-hidden="true"></span>
                            <div>
                                <strong>"Inspect active workspace"</strong>
                                <small>{move || {
                                    let raw = wb.harness_workspace_root().get();
                                    let t = raw.trim();
                                    if t.is_empty() {
                                        lookup(i18n.locale().get(), I18nKey::AgNoPath).to_owned()
                                    } else {
                                        t.to_string()
                                    }
                                }}</small>
                            </div>
                        </li>
                        <li class="agent-task">
                            <span class="agent-task__mark" aria-hidden="true"></span>
                            <div>
                                <strong>"Report execution details"</strong>
                                <small>{move || i18n.tr(I18nKey::AgScopedReadHint)()}</small>
                            </div>
                        </li>
                    </ol>
                </Show>
            </section>

            <Show when=move || status_line.get().is_some()>
                {move || {
                    let txt = status_line.get().unwrap_or_default();
                    view! {
                        <p class="workbench-agent-status">{txt}</p>
                    }
                }}
            </Show>

            <article class="workbench-agent-scroll" aria-live="polite" aria-label="Agent chat log">
                <div class="agent-section__head">
                    <h3>"Chat log"</h3>
                    <span>{move || if timeline.get().is_empty() { "Ready" } else { "Live" }}</span>
                </div>
                <Show
                    when=move || !timeline.get().is_empty()
                    fallback=move || view! {
                        <div class="agent-chat-line agent-chat-line--agent">
                            <span class="agent-chat-index">"01"</span>
                            <div class="agent-chat-body">
                                <strong>"BLXCode"</strong>
                                <p>
                                    {move || {
                                        let m = model_label.get();
                                        if m.is_empty() {
                                            "Hi — I'm the BLXCode agent. Configure a provider and model in the harness settings, then send a prompt to get started.".to_string()
                                        } else {
                                            format!(
                                                "Hi — I'm the BLXCode agent running {m}. I can read files, search workspace memory, and open terminals for you. Send a prompt to get started."
                                            )
                                        }
                                    }}
                                </p>
                            </div>
                        </div>
                    }
                >
                    <ol class="agent-chat-list" aria-label="Agent activity timeline">
                        {move || {
                            timeline.get().into_iter().enumerate().map(|(idx, entry)| {
                                view! { <TimelineRow idx=idx entry=entry i18n=i18n /> }
                            }).collect_view()
                        }}
                    </ol>
                </Show>
            </article>

            <form
                class="agent-compose"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit_turn(wb, i18n, draft, busy, status_line, timeline);
                }
            >
                <input
                    type="text"
                    class="workbench-agent-input workbench-agent-input--single"
                    placeholder=move || i18n.tr(I18nKey::AgPromptPh)()
                    prop:value=move || draft.get()
                    prop:disabled=move || busy.get()
                    on:input=move |ev| {
                        input_value_from(ev, draft);
                    }
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" && !ev.shift_key() && !ev.ctrl_key() && !ev.meta_key() {
                            ev.prevent_default();
                            submit_turn(wb, i18n, draft, busy, status_line, timeline);
                        }
                    }
                />

                <div class="workbench-agent-actions">
                    <button
                        type="submit"
                        class="workbench-mini-btn workbench-mini-btn--primary agent-send-btn"
                        prop:disabled=move || busy.get()
                    >
                        <LxIcon icon=icondata::LuSparkles width="0.9rem" height="0.9rem" />
                        <span>{move || i18n.tr(I18nKey::AgSend)()}</span>
                    </button>

                    <button
                        type="button"
                        class="workbench-mini-btn agent-cancel-btn"
                        prop:disabled=move || !busy.get()
                        on:click=move |_| {
                            leptos::task::spawn_local(async move {
                                let _ = agent_abort().await;
                            });
                        }
                    >
                        <LxIcon icon=icondata::LuX width="0.9rem" height="0.9rem" />
                        <span>{move || i18n.tr(I18nKey::AgCancel)()}</span>
                    </button>
                </div>
            </form>
        </section>
    }
}

/// Resolves the effective sandbox root for an agent turn:
/// 1. cwd of the active workspace (real repo), if any and non-empty,
/// 2. otherwise the persisted harness workspace root.
///
/// In Phase A the harness root is itself bootstrapped to `{app_data}/sandbox`
/// at shell mount, so this returns `None` only in pathological cases.
fn resolve_effective_workspace_root(wb: &WorkbenchService) -> Option<String> {
    let active = wb.active_id().get_untracked();
    if let Some(id) = active {
        let cwd = wb
            .workspaces()
            .with_untracked(|list| list.iter().find(|w| w.id == id).map(|w| w.cwd.clone()));
        if let Some(cwd) = cwd {
            let t = cwd.trim();
            if !t.is_empty() {
                return Some(t.to_owned());
            }
        }
    }
    let fallback = wb.harness_workspace_root().get_untracked();
    let t = fallback.trim();
    (!t.is_empty()).then(|| t.to_owned())
}

fn input_value_from(ev: web_sys::Event, draft: RwSignal<String>) {
    if let Some(t) = ev.target() {
        if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
            draft.set(inp.value());
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn submit_turn(
    wb: WorkbenchService,
    i18n: I18nService,
    draft: RwSignal<String>,
    busy: RwSignal<bool>,
    status_line: RwSignal<Option<String>>,
    timeline: RwSignal<Vec<TimelineItem>>,
) {
    if busy.get_untracked() {
        return;
    }

    let loc = i18n.locale().get_untracked();

    let prompt = draft.get_untracked().trim().to_owned();
    if prompt.is_empty() {
        status_line.set(Some(lookup(loc, I18nKey::AgErrNeedPrompt).into()));
        return;
    }

    let workspace_root = resolve_effective_workspace_root(&wb);

    timeline.set(vec![TimelineItem::User {
        text: prompt.clone(),
    }]);
    status_line.set(None);
    busy.set(true);
    draft.set(String::new());

    let turn = UserTurn {
        prompt,
        workspace_root,
    };

    let busy_sig = busy;
    let status_sig = status_line;
    let timeline_sig = timeline;

    leptos::task::spawn_local(async move {
        if let Err(msg) = agent_submit_turn(turn).await {
            busy_sig.set(false);
            status_sig.set(Some(msg));
            return;
        }

        let i18n_d = i18n;
        let wb_d = wb;
        if let Err(msg) = agent_drain_turn(move |batch| {
            let loc_now = i18n_d.locale().get_untracked();
            for ev in &batch {
                apply_agent_event(ev, timeline_sig, loc_now);
                maybe_handle_client_tool(ev, wb_d);
            }
            // The borrow of `batch` ends here; the closure returns ().
            let _ = batch;
        })
        .await
        {
            status_sig.set(Some(msg));
        }
        busy_sig.set(false);
    });
}

/// Routes UI-side (`runs_on: Client`) tool calls to the workbench and posts
/// their result back into the active turn. Server-side calls are ignored
/// here because the orchestrator handles them in-process.
fn maybe_handle_client_tool(ev: &AgentEvent, wb: WorkbenchService) {
    let AgentEvent::ToolCall {
        tool,
        call_id: Some(call_id),
        args,
    } = ev
    else {
        return;
    };
    let call_id = call_id.clone();
    match tool.as_str() {
        "harness.open_terminal" => handle_open_terminal(call_id, args.clone(), wb),
        "harness.list_terminals" => handle_list_terminals(call_id, wb),
        "harness.send_terminal_keys" => handle_send_keys(call_id, args.clone(), wb),
        "harness.read_terminal_output" => handle_read_output(call_id, args.clone(), wb),
        _ => {}
    }
}

fn submit_async(call_id: String, ok: bool, message: String, data: Option<serde_json::Value>) {
    leptos::task::spawn_local(async move {
        let _ = agent_submit_tool_result(call_id, ok, Some(message), data).await;
    });
}

fn handle_open_terminal(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let agent_slug = args
        .as_ref()
        .and_then(|v| v.get("agentSlug"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let active = wb.active_id().get_untracked();
    let (ok, msg) = match active {
        Some(workspace_id) => match wb.append_terminal_slot(workspace_id, agent_slug.clone()) {
            Ok(slot_id) => (
                true,
                format!(
                    "opened terminal slot {slot_id}{}",
                    agent_slug
                        .as_ref()
                        .map(|s| format!(" with agent={s}"))
                        .unwrap_or_default()
                ),
            ),
            Err(e) => (false, e),
        },
        None => (false, "no active workspace".to_owned()),
    };
    submit_async(call_id, ok, msg, None);
}

/// Resolves a terminal-targeting arg-blob to one specific PTY session id.
/// Prefers `slotId`, falls back to `agentSlug` (first match), then to the
/// first running PTY for the workspace.
fn resolve_target_session(
    wb: &WorkbenchService,
    workspace_id: u64,
    args: &Option<serde_json::Value>,
) -> Result<(u64, u64), String> {
    let slot_filter = args
        .as_ref()
        .and_then(|v| v.get("slotId"))
        .and_then(|v| v.as_u64());
    let agent_slug = args
        .as_ref()
        .and_then(|v| v.get("agentSlug"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let entries = wb.pty_sessions_for_workspace(workspace_id);
    if entries.is_empty() {
        return Err("no running terminal sessions in this workspace".into());
    }

    // Find slot_id → agent_slug map from workspace.
    let label_for_slot = |slot_id: u64| -> Option<String> {
        wb.workspaces().with_untracked(|ws| {
            ws.iter().find(|w| w.id == workspace_id).and_then(|w| {
                w.slot_ids
                    .iter()
                    .position(|id| *id == slot_id)
                    .and_then(|idx| w.slot_agent_labels.get(idx).cloned())
            })
        })
    };

    if let Some(slot) = slot_filter {
        if let Some((sid, pane)) = entries
            .iter()
            .find(|(s, _, _)| *s == slot)
            .map(|(_, p, sid)| (*sid, *p))
        {
            return Ok((sid, pane));
        }
        return Err(format!("slot {slot} not running"));
    }
    if let Some(slug) = agent_slug {
        for (slot, pane, sid) in &entries {
            if label_for_slot(*slot).as_deref() == Some(slug.as_str()) {
                return Ok((*sid, *pane));
            }
        }
        return Err(format!("no running slot with agent={slug}"));
    }
    let (_, pane, sid) = entries[0];
    Ok((sid, pane))
}

fn handle_list_terminals(call_id: String, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let running = wb.pty_sessions_for_workspace(workspace_id);
    let entries = wb.workspaces().with_untracked(|ws| {
        let Some(w) = ws.iter().find(|w| w.id == workspace_id) else {
            return Vec::new();
        };
        w.slot_ids
            .iter()
            .enumerate()
            .map(|(idx, slot_id)| {
                let agent = w.slot_agent_labels.get(idx).cloned().unwrap_or_default();
                let running = running.iter().any(|(s, _, _)| *s == *slot_id);
                serde_json::json!({
                    "slotId": slot_id,
                    "agentSlug": agent,
                    "running": running,
                })
            })
            .collect::<Vec<_>>()
    });
    let body = serde_json::Value::Array(entries.clone());
    let summary = format!("{} slot(s) listed", entries.len());
    submit_async(call_id, true, summary, Some(body));
}

fn handle_send_keys(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let text = args
        .as_ref()
        .and_then(|v| v.get("text"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let Some(text) = text else {
        submit_async(call_id, false, "missing text".into(), None);
        return;
    };
    let submit = args
        .as_ref()
        .and_then(|v| v.get("submit"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let (sid, _pane) = match resolve_target_session(&wb, workspace_id, &args) {
        Ok(t) => t,
        Err(e) => {
            submit_async(call_id, false, e, None);
            return;
        }
    };
    let mut payload = text.clone();
    if submit {
        payload.push('\r');
    }
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
    leptos::task::spawn_local(async move {
        match pty_write(sid, b64).await {
            Ok(()) => {
                let msg = format!(
                    "wrote {} byte(s) to session {sid}{}",
                    text.len(),
                    if submit { " (submitted)" } else { "" }
                );
                let _ = agent_submit_tool_result(call_id, true, Some(msg), None).await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn handle_read_output(call_id: String, args: Option<serde_json::Value>, wb: WorkbenchService) {
    let Some(workspace_id) = wb.active_id().get_untracked() else {
        submit_async(call_id, false, "no active workspace".into(), None);
        return;
    };
    let max_bytes = args
        .as_ref()
        .and_then(|v| v.get("maxBytes"))
        .and_then(|v| v.as_u64())
        .unwrap_or(4096)
        .min(65536) as usize;
    let (sid, _pane) = match resolve_target_session(&wb, workspace_id, &args) {
        Ok(t) => t,
        Err(e) => {
            submit_async(call_id, false, e, None);
            return;
        }
    };
    leptos::task::spawn_local(async move {
        match pty_peek_output(sid, max_bytes).await {
            Ok(text) => {
                let len = text.len();
                let _ = agent_submit_tool_result(
                    call_id,
                    true,
                    Some(text),
                    Some(serde_json::json!({ "bytes": len, "sessionId": sid })),
                )
                .await;
            }
            Err(e) => {
                let _ = agent_submit_tool_result(call_id, false, Some(e), None).await;
            }
        }
    });
}

fn apply_agent_event(ev: &AgentEvent, timeline: RwSignal<Vec<TimelineItem>>, loc: Locale) {
    match ev {
        AgentEvent::AssistantDelta { delta } => timeline.update(|rows| match rows.last_mut() {
            Some(TimelineItem::Assistant { text }) => text.push_str(delta),
            _ => rows.push(TimelineItem::Assistant {
                text: delta.clone(),
            }),
        }),
        AgentEvent::ToolCall { tool, args, .. } => {
            let entry = ToolActivity::from_call(tool, args.as_ref());
            timeline.update(|rows| rows.push(TimelineItem::Tool(entry)));
        }
        AgentEvent::ToolResult { tool, ok, message } => {
            timeline.update(|rows| {
                // Match the most recent pending entry for this tool name.
                let slot = rows.iter_mut().rev().find_map(|entry| match entry {
                    TimelineItem::Tool(row)
                        if row.tool == *tool && row.status == ActivityStatus::Pending =>
                    {
                        Some(row)
                    }
                    _ => None,
                });
                if let Some(row) = slot {
                    row.status = if *ok {
                        ActivityStatus::Ok
                    } else {
                        ActivityStatus::Fail
                    };
                    row.detail = message.clone().filter(|m| !m.is_empty());
                } else {
                    // No matching pending row — synthesise one so the result
                    // isn't silently dropped (e.g. mock orchestrator path).
                    rows.push(TimelineItem::Tool(ToolActivity {
                        tool: tool.clone(),
                        label: friendly_label(tool).to_owned(),
                        args_summary: String::new(),
                        status: if *ok {
                            ActivityStatus::Ok
                        } else {
                            ActivityStatus::Fail
                        },
                        detail: message.clone().filter(|m| !m.is_empty()),
                    }));
                }
            });
        }
        AgentEvent::Done => {}
        AgentEvent::Error { message } => {
            let prefix = lookup(loc, I18nKey::AgErrColon);
            let line = format!("{prefix} {message}");
            timeline.update(|rows| match rows.last_mut() {
                Some(TimelineItem::Assistant { text }) => {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str(&line);
                    text.push('\n');
                }
                _ => rows.push(TimelineItem::Assistant {
                    text: format!("{line}\n"),
                }),
            });
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TimelineItem {
    User { text: String },
    Assistant { text: String },
    Tool(ToolActivity),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ActivityStatus {
    Pending,
    Ok,
    Fail,
}

#[derive(Clone, Debug, PartialEq)]
struct ToolActivity {
    tool: String,
    label: String,
    args_summary: String,
    status: ActivityStatus,
    detail: Option<String>,
}

impl ToolActivity {
    fn from_call(tool: &str, args: Option<&serde_json::Value>) -> Self {
        Self {
            tool: tool.to_owned(),
            label: friendly_label(tool).to_owned(),
            args_summary: summarize_args(tool, args),
            status: ActivityStatus::Pending,
            detail: None,
        }
    }
}

/// Maps raw tool ids onto short, user-readable labels. Unknown tools fall
/// back to the bare id so we never lose information.
fn friendly_label(tool: &str) -> &str {
    match tool {
        "read_workspace_file" => "Read file",
        "memory_list" => "List memory notes",
        "memory_read" => "Read memory note",
        "memory_search" => "Search memory",
        "memory_create" => "Create memory note",
        "memory_write" => "Update memory note",
        "harness.open_terminal" => "Open terminal",
        "harness.list_terminals" => "List terminals",
        "harness.send_terminal_keys" => "Send keys to terminal",
        "harness.read_terminal_output" => "Read terminal output",
        other => other,
    }
}

/// Pulls the *interesting* field out of a tool-call args blob — path,
/// query, agent slug — so the row stays a one-liner.
fn summarize_args(tool: &str, args: Option<&serde_json::Value>) -> String {
    let Some(args) = args else {
        return String::new();
    };
    let pick = match tool {
        "read_workspace_file" | "memory_read" | "memory_create" | "memory_write" => Some("path"),
        "memory_search" => Some("query"),
        "harness.open_terminal" => Some("agentSlug"),
        "harness.send_terminal_keys" => Some("text"),
        _ => None,
    };
    if let Some(key) = pick {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            return v.to_owned();
        }
    }
    String::new()
}

fn tool_icon(tool: &str) -> icondata::Icon {
    match tool {
        "read_workspace_file" => icondata::LuFileText,
        "memory_list" => icondata::LuList,
        "memory_read" => icondata::LuBookOpen,
        "memory_search" => icondata::LuSearch,
        "memory_create" => icondata::LuFilePlus,
        "memory_write" => icondata::LuFileEdit,
        "harness.open_terminal" => icondata::LuTerminal,
        "harness.list_terminals" => icondata::LuLayoutGrid,
        "harness.send_terminal_keys" => icondata::LuSendHorizonal,
        "harness.read_terminal_output" => icondata::LuWrapText,
        _ => icondata::LuWrench,
    }
}

#[component]
fn TimelineRow(idx: usize, entry: TimelineItem, i18n: I18nService) -> impl IntoView {
    let line_no = format!("{:02}", idx + 1);
    match entry {
        TimelineItem::User { text } => view! {
            <li class="agent-chat-line agent-chat-line--user">
                <span class="agent-chat-index">{line_no.clone()}</span>
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgYou)()}</strong>
                    <pre class="workbench-agent-transcript">{text}</pre>
                </div>
            </li>
        }
        .into_any(),
        TimelineItem::Assistant { text } => view! {
            <li class="agent-chat-line agent-chat-line--agent">
                <span class="agent-chat-index">{line_no.clone()}</span>
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgAssistant)()}</strong>
                    <pre class="workbench-agent-transcript">{text}</pre>
                </div>
            </li>
        }
        .into_any(),
        TimelineItem::Tool(entry) => view! {
            <ToolActivityRow idx=idx line_no=line_no entry=entry />
        }
        .into_any(),
    }
}

#[component]
fn ToolActivityRow(idx: usize, line_no: String, entry: ToolActivity) -> impl IntoView {
    let status_class = match entry.status {
        ActivityStatus::Pending => "agent-tool-row--pending",
        ActivityStatus::Ok => "agent-tool-row--ok",
        ActivityStatus::Fail => "agent-tool-row--fail",
    };
    let status_icon = match entry.status {
        ActivityStatus::Pending => icondata::LuLoader,
        ActivityStatus::Ok => icondata::LuCheck,
        ActivityStatus::Fail => icondata::LuAlertTriangle,
    };
    let detail_open = RwSignal::new(false);
    let has_detail = entry.detail.as_ref().is_some_and(|s| !s.is_empty());
    let detail_text = entry.detail.clone().unwrap_or_default();
    let label = entry.label.clone();
    let summary = entry.args_summary.clone();
    let tool_name_for_title = entry.tool.clone();
    let _ = idx;

    view! {
        <li class="agent-chat-line agent-chat-line--tool">
            <span class="agent-chat-index">{line_no}</span>
            <div class="agent-chat-body">
                <strong>"Tool"</strong>
                <div class=format!("agent-tool-row {status_class}") title=tool_name_for_title>
                    <button
                        type="button"
                        class="agent-tool-row__head"
                        aria-expanded=move || detail_open.get().to_string()
                        prop:disabled=move || !has_detail
                        on:click=move |_| {
                            if has_detail {
                                detail_open.update(|o| *o = !*o);
                            }
                        }
                    >
                        <span class="agent-tool-row__icon" aria-hidden="true">
                            <LxIcon icon=tool_icon(&entry.tool) width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="agent-tool-row__label">{label}</span>
                        <Show when={
                            let s = summary.clone();
                            move || !s.is_empty()
                        }>
                            <span class="agent-tool-row__arg">{summary.clone()}</span>
                        </Show>
                        <span class="agent-tool-row__status" aria-hidden="true">
                            <LxIcon icon=status_icon width="0.78rem" height="0.78rem" />
                        </span>
                    </button>
                    <Show when=move || has_detail && detail_open.get()>
                        <pre class="agent-tool-row__detail">{detail_text.clone()}</pre>
                    </Show>
                </div>
            </div>
        </li>
    }
}
