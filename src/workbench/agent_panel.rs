//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
use crate::agent_wire::{AgentEvent, UserTurn};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{agent_abort, agent_drain_turn, agent_submit_turn};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[component]
pub fn AgentPanelDock() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let draft = RwSignal::new(String::new());
    let transcript = RwSignal::new(String::new());
    let activity = RwSignal::new(Vec::<String>::new());
    let busy = RwSignal::new(false);
    let status_line = RwSignal::new(Option::<String>::None);
    let ptt_active = RwSignal::new(false);

    view! {
        <section class="workbench-agent-pane" aria-label=move || i18n.tr(I18nKey::AgAriaPane)()>
            <header class="agent-hero">
                <div class="agent-hero__orb" aria-hidden="true">
                    <span class="agent-hero__logo">"B"</span>
                </div>
                <div class="agent-hero__meta">
                    <p class="agent-hero__eyebrow">"Bridge agent"</p>
                    <h2>"Standby"</h2>
                    <p>{move || if ptt_active.get() { "Listening mock" } else { "Tap or hold to activate" }}</p>
                </div>
            </header>

            <button
                type="button"
                class="agent-ptt"
                class:agent-ptt--active=move || ptt_active.get()
                aria-pressed=move || ptt_active.get().to_string()
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
                <span class="agent-ptt__ring" aria-hidden="true">
                    <LxIcon icon=icondata::LuSparkles width="1.1rem" height="1.1rem" />
                </span>
                <span class="agent-ptt__copy">
                    <strong>{move || if ptt_active.get() { "Listening..." } else { "Push to talk" }}</strong>
                    <small>"Voice capture is mocked for now"</small>
                </span>
            </button>

            <section class="agent-section agent-section--tasks" aria-labelledby="agent-tasks-title">
                <div class="agent-section__head">
                    <h3 id="agent-tasks-title">"Tasks"</h3>
                    <span>{move || if busy.get() { "Running" } else { "Idle" }}</span>
                </div>
                <ol class="agent-task-list">
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
            </section>

            <Show when=move || status_line.get().is_some()>
                {move || {
                    let txt = status_line.get().unwrap_or_default();
                    view! {
                        <p class="workbench-agent-status">{txt}</p>
                    }
                }}
            </Show>

            <article class="workbench-agent-scroll agent-section" aria-live="polite" aria-label="Agent chat log">
                <div class="agent-section__head">
                    <h3>"Chat log"</h3>
                    <span>{move || if activity.get().is_empty() { "Ready" } else { "Tools" }}</span>
                </div>
                <Show
                    when=move || !transcript.get().trim().is_empty()
                    fallback=move || view! {
                        <div class="agent-chat-line agent-chat-line--agent">
                            <strong>"Bridge"</strong>
                            <p>"Ready. Tell me what to plan, run, or investigate in this workspace."</p>
                        </div>
                        <div class="agent-chat-line agent-chat-line--user">
                            <strong>"You"</strong>
                            <p>"Try: help me wire up the auth refactor, list bugs, or prepare a run plan."</p>
                        </div>
                    }
                >
                    <pre class="workbench-agent-transcript">{move || transcript.get()}</pre>
                </Show>
                <Show when=move || !activity.get().is_empty()>
                    {move || {
                        activity.get().into_iter().map(|ln|
                            view! { <div class="workbench-agent-row">{ln}</div> }
                        ).collect_view()
                    }}
                </Show>
            </article>

            <form
                class="agent-compose"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit_turn(wb, i18n, draft, busy, status_line, transcript, activity);
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
                            submit_turn(wb, i18n, draft, busy, status_line, transcript, activity);
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
    transcript: RwSignal<String>,
    activity: RwSignal<Vec<String>>,
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

    let workspace_root_raw = wb.harness_workspace_root().get_untracked();
    let workspace_root = {
        let t = workspace_root_raw.trim().to_owned();
        (!t.is_empty()).then_some(t)
    };

    let you = lookup(loc, I18nKey::AgYou);
    let assistant = lookup(loc, I18nKey::AgAssistant);
    transcript.set(format!("**{you}:** {prompt}\n\n**{assistant}:** "));
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
        if let Err(msg) = agent_drain_turn(move |batch| {
            let loc_now = i18n_d.locale().get_untracked();
            for ev in batch {
                apply_agent_event(&ev, transcript_sig, activity_sig, loc_now);
            }
        })
        .await
        {
            status_sig.set(Some(msg));
        }
        busy_sig.set(false);
    });
}

fn apply_agent_event(
    ev: &AgentEvent,
    transcript: RwSignal<String>,
    activity: RwSignal<Vec<String>>,
    loc: Locale,
) {
    match ev {
        AgentEvent::AssistantDelta { delta } => transcript.update(|t| t.push_str(delta)),
        AgentEvent::ToolCall { tool, args } => activity.update(|lines| {
            let extra = args
                .as_ref()
                .and_then(|a| serde_json::to_string(a).ok())
                .filter(|s| !s.is_empty())
                .map(|s| format!(" [{s}]"))
                .unwrap_or_default();
            lines.push(format!("Tool: {tool}{extra}"));
        }),
        AgentEvent::ToolResult { tool, ok, message } => {
            let hint = message.as_deref().unwrap_or("—");
            let tag = if *ok { "ok" } else { "fail" };
            activity.update(|lines| lines.push(format!("{tag} {tool}: {hint}")));
        }
        AgentEvent::Done => {}
        AgentEvent::Error { message } => {
            let prefix = lookup(loc, I18nKey::AgErrColon);
            transcript.update(|t| t.push_str(&format!("\n{prefix} {message}\n")));
        }
    }
}
