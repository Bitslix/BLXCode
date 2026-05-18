//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
mod client_tools;
mod task_list;
mod timeline;

use crate::agent_wire::{TaskSnapshot, UserTurn};
use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_abort, agent_clear_conversation, agent_drain_turn, agent_settings_get, agent_submit_turn,
    is_tauri_shell,
    tasks_list as fetch_tasks_list,
};
use crate::workbench::agent_panel::client_tools::maybe_handle_client_tool;
use crate::workbench::agent_panel::task_list::TaskSection;
use crate::workbench::agent_panel::timeline::{
    apply_agent_event, compact_timeline, TimelineItem, TimelineRow,
};
use crate::workbench::WorkbenchService;
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::HashMap;
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
    let chat_scroll_ref = NodeRef::<html::Article>::new();
    let task_snapshot = RwSignal::new(TaskSnapshot {
        tasks: Vec::new(),
        active_task_id: None,
    });
    // Open/closed state per thinking item, keyed by its position in the
    // display timeline. Lives on the parent so streaming rerenders do not
    // remount the row and reset the local open flag.
    let thinking_open = RwSignal::new(HashMap::<usize, bool>::new());

    // Load authoritative timeline + compose draft when the active workspace
    // changes only (do not subscribe to `workspaces` — timeline writes would
    // reset thinking UI and fight streaming).
    Effect::new(move |_| {
        let active = wb.active_id().get();
        let Some(id) = active else {
            timeline.set(Vec::new());
            thinking_open.set(HashMap::new());
            draft.set(String::new());
            return;
        };
        timeline.set(wb.agent_timeline_for_workspace_untracked(id));
        thinking_open.set(HashMap::new());
        draft.set(wb.agent_compose_draft_for_workspace_untracked(id));
    });

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                model_label.set(format!("{}/{}", view.provider.as_str(), view.model_id));
            }
        });
    }

    Effect::new(move |_| {
        let active = wb.active_id().get();
        let root = resolve_effective_workspace_root(&wb);
        if !is_tauri_shell() {
            task_snapshot.set(TaskSnapshot {
                tasks: Vec::new(),
                active_task_id: None,
            });
            return;
        }
        let task_snapshot_sig = task_snapshot;
        leptos::task::spawn_local(async move {
            let next = match root {
                Some(workspace_cwd) => {
                    fetch_tasks_list(workspace_cwd)
                        .await
                        .unwrap_or(TaskSnapshot {
                            tasks: Vec::new(),
                            active_task_id: None,
                        })
                }
                None => TaskSnapshot {
                    tasks: Vec::new(),
                    active_task_id: None,
                },
            };
            let _ = active;
            task_snapshot_sig.set(next);
        });
    });

    Effect::new(move |_| {
        let _ = timeline.get().len();
        if let Some(article) = chat_scroll_ref.get() {
            article.set_scroll_top(article.scroll_height());
        }
    });

    view! {
        <section class="workbench-agent-pane" aria-label=move || i18n.tr(I18nKey::AgAriaPane)()>
            <header class="agent-hero">
                <button
                    type="button"
                    class="agent-hero__orb"
                    class:agent-hero__orb--active=move || ptt_active.get()
                    aria-pressed=move || ptt_active.get().to_string()
                    aria-label=move || i18n.tr(I18nKey::AgOrbAria)()
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
                    <p class="agent-hero__eyebrow">{move || i18n.tr(I18nKey::AgBrandTitle)()}</p>
                    <h2>{move || {
                        if busy.get() {
                            i18n.tr(I18nKey::AgStateRunning)().to_string()
                        } else {
                            i18n.tr(I18nKey::AgStateStandby)().to_string()
                        }
                    }}</h2>
                    <p>{move || i18n.tr(I18nKey::AgTagline)()}</p>
                </div>
            </header>

            <TaskSection snapshot=task_snapshot busy=busy tasks_open=tasks_open />

            <Show when=move || status_line.get().is_some()>
                {move || {
                    let txt = status_line.get().unwrap_or_default();
                    view! {
                        <p class="workbench-agent-status">{txt}</p>
                    }
                }}
            </Show>

            <article
                node_ref=chat_scroll_ref
                class="workbench-agent-scroll"
                aria-live="polite"
                aria-label=move || i18n.tr(I18nKey::AgChatArticleAria)()
            >
                <div class="agent-section__head">
                    <h3>{move || i18n.tr(I18nKey::AgChatHeading)()}</h3>
                    <span>{move || {
                        if timeline.get().is_empty() {
                            i18n.tr(I18nKey::AgBadgeReady)().to_string()
                        } else {
                            i18n.tr(I18nKey::AgBadgeLive)().to_string()
                        }
                    }}</span>
                </div>
                <Show
                    when=move || !timeline.get().is_empty()
                    fallback=move || view! {
                        <div class="agent-chat-line agent-chat-line--agent">
                            <span class="agent-chat-index">"01"</span>
                            <div class="agent-chat-body">
                                <strong>"BLXCode"</strong>
                                <p>{move || i18n.tr(I18nKey::AgWelcomeBody)()}</p>
                            </div>
                        </div>
                    }
                >
                    <ol class="agent-chat-list" aria-label=move || i18n.tr(I18nKey::AgTimelineAria)()>
                        {move || {
                            compact_timeline(timeline.get())
                                .into_iter()
                                .enumerate()
                                .map(|(idx, entry)| {
                                    view! { <TimelineRow idx=idx entry=entry i18n=i18n thinking_open=thinking_open /> }
                                })
                                .collect_view()
                        }}
                    </ol>
                </Show>
            </article>

            <form
                class="agent-compose"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open);
                }
            >
                <input
                    type="text"
                    class="workbench-agent-input workbench-agent-input--single"
                    placeholder=move || i18n.tr(I18nKey::AgPromptPh)()
                    prop:value=move || draft.get()
                    prop:disabled=move || busy.get()
                    on:input=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                let v = inp.value();
                                draft.set(v.clone());
                                if let Some(id) = wb.active_id().get_untracked() {
                                    wb.set_workspace_agent_compose_draft(id, v);
                                }
                            }
                        }
                    }
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" && !ev.shift_key() && !ev.ctrl_key() && !ev.meta_key() {
                            ev.prevent_default();
                            submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open);
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

                    <button
                        type="button"
                        class="workbench-mini-btn agent-reset-chat-btn"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        aria-label=move || i18n.tr(I18nKey::AgResetChatAria)()
                        on:click=move |_| {
                            let wb = wb;
                            let status_line = status_line;
                            let timeline = timeline;
                            let draft = draft;
                            let thinking_open = thinking_open;
                            leptos::task::spawn_local(async move {
                                let Some(ws_id) = wb.active_id().get_untracked() else {
                                    status_line.set(Some("Select a workspace tab first.".into()));
                                    return;
                                };
                                match agent_clear_conversation().await {
                                    Ok(()) => {
                                        timeline.set(Vec::new());
                                        thinking_open.set(HashMap::new());
                                        draft.set(String::new());
                                        wb.set_workspace_agent_timeline(ws_id, Vec::new());
                                        wb.set_workspace_agent_compose_draft(ws_id, String::new());
                                        status_line.set(None);
                                    }
                                    Err(msg) => status_line.set(Some(msg)),
                                }
                            });
                        }
                    >
                        <LxIcon icon=icondata::LuEraser width="0.9rem" height="0.9rem" />
                        <span>{move || i18n.tr(I18nKey::AgResetChat)()}</span>
                    </button>
                </div>
            </form>
        </section>
    }
}

fn is_reset_command(prompt: &str) -> bool {
    let p = prompt.trim().to_ascii_lowercase();
    matches!(p.as_str(), "/reset" | "/new")
}

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

#[allow(clippy::too_many_arguments)]
fn submit_turn(
    wb: WorkbenchService,
    i18n: I18nService,
    draft: RwSignal<String>,
    busy: RwSignal<bool>,
    status_line: RwSignal<Option<String>>,
    timeline: RwSignal<Vec<TimelineItem>>,
    task_snapshot: RwSignal<TaskSnapshot>,
    thinking_open: RwSignal<HashMap<usize, bool>>,
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

    let Some(ws_id) = wb.active_id().get_untracked() else {
        status_line.set(Some("Select a workspace tab first.".into()));
        return;
    };

    if is_reset_command(&prompt) {
        draft.set(String::new());
        wb.set_workspace_agent_compose_draft(ws_id, String::new());
        status_line.set(None);
        leptos::task::spawn_local(async move {
            match agent_clear_conversation().await {
                Ok(()) => {
                    timeline.set(Vec::new());
                    thinking_open.set(HashMap::new());
                    wb.set_workspace_agent_timeline(ws_id, Vec::new());
                    status_line.set(None);
                }
                Err(msg) => status_line.set(Some(msg)),
            }
        });
        return;
    }

    let workspace_root = resolve_effective_workspace_root(&wb);

    timeline.update(|items| {
        items.push(TimelineItem::User {
            text: prompt.clone(),
        });
    });
    wb.set_workspace_agent_timeline(ws_id, timeline.get_untracked());

    status_line.set(None);
    busy.set(true);
    draft.set(String::new());
    wb.set_workspace_agent_compose_draft(ws_id, String::new());

    let turn = UserTurn {
        prompt,
        workspace_root,
    };

    let busy_sig = busy;
    let status_sig = status_line;
    let timeline_sig = timeline;
    let task_snapshot_sig = task_snapshot;
    let ws_capture = ws_id;

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
                apply_agent_event(
                    ev,
                    timeline_sig,
                    task_snapshot_sig,
                    loc_now,
                    Some((wb_d, ws_capture)),
                );
                maybe_handle_client_tool(ev, wb_d);
            }
            let _ = batch;
        })
        .await
        {
            status_sig.set(Some(msg));
        }
        busy_sig.set(false);
    });
}
