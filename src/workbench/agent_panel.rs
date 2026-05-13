//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
use crate::agent_wire::{AgentEvent, UserTurn};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{agent_abort, agent_drain_turn, agent_submit_turn};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
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

    view! {
        <section class="workbench-agent-pane" aria-label=move || i18n.tr(I18nKey::AgAriaPane)()>
            <div class="workbench-agent-scope-hint">
                <span>{move || i18n.tr(I18nKey::AgSandbox)()}</span>
                <code>{move || {
                    let raw = wb.harness_workspace_root().get();
                    let t = raw.trim();
                    if t.is_empty() {
                        lookup(i18n.locale().get(), I18nKey::AgNoPath).to_owned()
                    } else {
                        t.to_string()
                    }
                }}</code>
                <small class="workbench-muted">
                    {move || i18n.tr(I18nKey::AgScopedReadHint)()}
                </small>
            </div>

            <textarea
                class="workbench-agent-input"
                placeholder=move || i18n.tr(I18nKey::AgPromptPh)()
                rows="4"
                prop:value=move || draft.get()
                prop:disabled=move || busy.get()
                on:input=move |ev| {
                    textarea_value_from(ev, draft);
                }
            />

            <div class="workbench-agent-actions">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    prop:disabled=move || busy.get()
                    on:click=move |_| {
                        submit_turn(wb, i18n, draft, busy, status_line, transcript, activity);
                    }
                >
                    {move || i18n.tr(I18nKey::AgSend)()}
                </button>

                <button
                    type="button"
                    class="workbench-mini-btn"
                    prop:disabled=move || !busy.get()
                    on:click=move |_| {
                        leptos::task::spawn_local(async move {
                            let _ = agent_abort().await;
                        });
                    }
                >
                    {move || i18n.tr(I18nKey::AgCancel)()}
                </button>
            </div>

            <Show when=move || status_line.get().is_some()>
                {move || {
                    let txt = status_line.get().unwrap_or_default();
                    view! {
                        <p class="workbench-agent-status">{txt}</p>
                    }
                }}
            </Show>

            <article class="workbench-agent-scroll" aria-live="polite">
                <pre class="workbench-agent-transcript">{move || transcript.get()}</pre>
                <Show when=move || !activity.get().is_empty()>
                    {move || {
                        activity.get().into_iter().map(|ln|
                            view! { <div class="workbench-agent-row">{ln}</div> }
                        ).collect_view()
                    }}
                </Show>
            </article>
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
