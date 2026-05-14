//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
use crate::agent_wire::{AgentEvent, UserTurn};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_abort, agent_drain_turn, agent_settings_get, agent_submit_tool_result, agent_submit_turn,
    is_tauri_shell,
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
    let user_prompt = RwSignal::new(String::new());
    let transcript = RwSignal::new(String::new());
    let activity = RwSignal::new(Vec::<ToolActivity>::new());
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
                    <span>{move || if activity.get().is_empty() { "Ready" } else { "Tools" }}</span>
                </div>
                <Show
                    when=move || !user_prompt.get().trim().is_empty() || !transcript.get().trim().is_empty()
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
                    <Show when=move || !user_prompt.get().trim().is_empty()>
                        <div class="agent-chat-line agent-chat-line--user">
                            <span class="agent-chat-index">"01"</span>
                            <div class="agent-chat-body">
                                <strong>{move || i18n.tr(I18nKey::AgYou)()}</strong>
                                <pre class="workbench-agent-transcript">{move || user_prompt.get()}</pre>
                            </div>
                        </div>
                    </Show>
                    <Show when=move || !transcript.get().trim().is_empty()>
                        <div class="agent-chat-line agent-chat-line--agent">
                            <span class="agent-chat-index">"02"</span>
                            <div class="agent-chat-body">
                                <strong>{move || i18n.tr(I18nKey::AgAssistant)()}</strong>
                                <pre class="workbench-agent-transcript">{move || transcript.get()}</pre>
                            </div>
                        </div>
                    </Show>
                </Show>
                <Show when=move || !activity.get().is_empty()>
                    <ul class="agent-tool-list" aria-label="Tool activity">
                        {move || {
                            activity.get().into_iter().enumerate().map(|(idx, a)| {
                                view! { <ToolActivityRow idx=idx entry=a /> }
                            }).collect_view()
                        }}
                    </ul>
                </Show>
            </article>

            <form
                class="agent-compose"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit_turn(wb, i18n, draft, busy, status_line, user_prompt, transcript, activity);
                }
            >
                <textarea
                    class="workbench-agent-input"
                    placeholder=move || i18n.tr(I18nKey::AgPromptPh)()
                    rows="2"
                    prop:value=move || draft.get()
                    prop:disabled=move || busy.get()
                    on:input=move |ev| {
                        textarea_value_from(ev, draft);
                    }
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                            ev.prevent_default();
                            submit_turn(wb, i18n, draft, busy, status_line, user_prompt, transcript, activity);
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

fn textarea_value_from(ev: web_sys::Event, draft: RwSignal<String>) {
    if let Some(t) = ev.target() {
        if let Ok(ta) = t.dyn_into::<web_sys::HtmlTextAreaElement>() {
            draft.set(ta.value());
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
    user_prompt: RwSignal<String>,
    transcript: RwSignal<String>,
    activity: RwSignal<Vec<ToolActivity>>,
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

    user_prompt.set(prompt.clone());
    transcript.set(String::new());
    activity.set(Vec::new());
    status_line.set(None);
    busy.set(true);
    draft.set(String::new());

    let turn = UserTurn {
        prompt,
        workspace_root,
    };

    let busy_sig = busy;
    let status_sig = status_line;
    let transcript_sig = transcript;
    let activity_sig = activity;

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
                apply_agent_event(ev, transcript_sig, activity_sig, loc_now);
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
    if tool != "harness.open_terminal" {
        return;
    }
    let call_id = call_id.clone();
    let agent_slug = args
        .as_ref()
        .and_then(|v| v.get("agentSlug"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    let active = wb.active_id().get_untracked();
    let outcome = match active {
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

    leptos::task::spawn_local(async move {
        let _ = agent_submit_tool_result(call_id, outcome.0, Some(outcome.1), None).await;
    });
}

fn apply_agent_event(
    ev: &AgentEvent,
    transcript: RwSignal<String>,
    activity: RwSignal<Vec<ToolActivity>>,
    loc: Locale,
) {
    match ev {
        AgentEvent::AssistantDelta { delta } => transcript.update(|t| t.push_str(delta)),
        AgentEvent::ToolCall { tool, args, .. } => {
            let entry = ToolActivity::from_call(tool, args.as_ref());
            activity.update(|rows| rows.push(entry));
        }
        AgentEvent::ToolResult { tool, ok, message } => {
            activity.update(|rows| {
                // Match the most recent pending entry for this tool name.
                let slot = rows
                    .iter_mut()
                    .rev()
                    .find(|r| r.tool == *tool && r.status == ActivityStatus::Pending);
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
                    rows.push(ToolActivity {
                        tool: tool.clone(),
                        label: friendly_label(tool).to_owned(),
                        args_summary: String::new(),
                        status: if *ok {
                            ActivityStatus::Ok
                        } else {
                            ActivityStatus::Fail
                        },
                        detail: message.clone().filter(|m| !m.is_empty()),
                    });
                }
            });
        }
        AgentEvent::Done => {}
        AgentEvent::Error { message } => {
            let prefix = lookup(loc, I18nKey::AgErrColon);
            transcript.update(|t| t.push_str(&format!("\n{prefix} {message}\n")));
        }
    }
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
        _ => icondata::LuWrench,
    }
}

#[component]
fn ToolActivityRow(idx: usize, entry: ToolActivity) -> impl IntoView {
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
        <li class=format!("agent-tool-row {status_class}") title=tool_name_for_title>
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
                    <LxIcon icon=tool_icon(&entry.tool) width="0.95rem" height="0.95rem" />
                </span>
                <span class="agent-tool-row__label">{label}</span>
                <Show when={
                    let s = summary.clone();
                    move || !s.is_empty()
                }>
                    <span class="agent-tool-row__arg">{summary.clone()}</span>
                </Show>
                <span class="agent-tool-row__status" aria-hidden="true">
                    <LxIcon icon=status_icon width="0.85rem" height="0.85rem" />
                </span>
            </button>
            <Show when=move || has_detail && detail_open.get()>
                <pre class="agent-tool-row__detail">{detail_text.clone()}</pre>
            </Show>
        </li>
    }
}
