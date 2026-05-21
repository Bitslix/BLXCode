//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
mod ask_user_card;
mod client_tools;
mod context_list;
mod image_context;
mod subagent_debounce;
mod task_list;
mod timeline;
pub(crate) mod turn_metrics_bar;
mod voice_orb;

use crate::agent_wire::{AgentEvent, TaskSnapshot, UserTurn};
use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_abort, agent_clear_conversation, agent_drain_turn_opts, agent_settings_get,
    agent_submit_turn, is_tauri_shell, tasks_list as fetch_tasks_list,
};
use crate::workbench::agent_panel::client_tools::maybe_handle_client_tool;
use crate::workbench::agent_panel::context_list::ContextSection;
use crate::workbench::agent_panel::image_context::{
    clear_drop_state, handle_dom_drag_event, handle_dom_drop, install_agent_image_intake,
    DropZoneState,
};
use crate::workbench::agent_panel::task_list::TaskSection;
use crate::workbench::agent_panel::subagent_debounce::{
    is_subagent_timeline_event, SubagentEventDebounce,
};
use crate::workbench::agent_panel::timeline::{
    apply_agent_event, compact_timeline, ChatLineIndexColumn, TimelineItem, TimelineRow,
};
use std::rc::Rc;
use crate::workbench::agent_panel::voice_orb::{
    handle_voice_event, install_ptt_hotkey, VoiceOrb, VoiceOrbHandle,
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
    let tasks_open = RwSignal::new(false);
    let context_open = RwSignal::new(false);
    let drop_state = RwSignal::new(DropZoneState::Inactive);
    let model_label = RwSignal::new(String::new());
    // Per-session view of the persisted `WorkspaceEntry.agent_image_mode`.
    // Synced on workspace switch and on toggle (writer also persists to the
    // workspace entry so the flag survives reloads).
    let image_mode = RwSignal::new(false);
    let chat_maximized = RwSignal::new(false);
    let chat_scroll_ref = NodeRef::<html::Div>::new();
    let voice_handle = VoiceOrbHandle::new();
    let task_snapshot = RwSignal::new(TaskSnapshot {
        tasks: Vec::new(),
        active_task_id: None,
        active_plan_path: None,
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
            image_mode.set(false);
            return;
        };
        timeline.set(wb.agent_timeline_for_workspace_untracked(id));
        thinking_open.set(HashMap::new());
        draft.set(wb.agent_compose_draft_for_workspace_untracked(id));
        image_mode.set(wb.agent_image_mode_for_workspace_untracked(id));
    });

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                model_label.set(format!("{}/{}", view.provider.as_str(), view.model_id));
            }
        });
        Effect::new(move |_| {
            let _ = wb.active_id().get();
            let handle = voice_handle;
            leptos::task::spawn_local(async move {
                if let Ok(v) = crate::tauri_bridge::voice_settings_get().await {
                    handle.settings.set(Some(v));
                }
            });
        });
    }

    Effect::new(move |_| {
        let active = wb.active_id().get();
        let root = resolve_effective_workspace_root(&wb);
        if !is_tauri_shell() {
            task_snapshot.set(TaskSnapshot {
                tasks: Vec::new(),
                active_task_id: None,
                active_plan_path: None,
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
                            active_plan_path: None,
                        })
                }
                None => TaskSnapshot {
                    tasks: Vec::new(),
                    active_task_id: None,
                    active_plan_path: None,
                },
            };
            // Prime the handoff renderer cache so terminal handoffs after
            // a workspace reload see the restored plan-task state.
            if let Some(ws_id) = active {
                crate::workbench::agent_context_handoff::store_task_snapshot(
                    ws_id,
                    next.clone(),
                );
            }
            task_snapshot_sig.set(next);
        });
    });

    // Collapsed when empty; expanded when at least one task or context item.
    Effect::new(move |_| {
        let count = task_snapshot.get().tasks.len();
        tasks_open.set(count > 0);
    });
    Effect::new(move |_| {
        let active = wb.active_id().get();
        let count = match active {
            Some(id) => wb.workspaces().with(|workspaces| {
                let memory_count = workspaces
                    .iter()
                    .find(|w| w.id == id)
                    .map(|w| w.agent_context_items.len())
                    .unwrap_or(0);
                memory_count + wb.active_agent_image_count()
            }),
            None => 0,
        };
        context_open.set(count > 0);
    });

    Effect::new(move |_| {
        let _ = timeline.get().len();
        if let Some(log) = chat_scroll_ref.get() {
            log.set_scroll_top(log.scroll_height());
        }
    });

    // Window-level PTT hotkey: install once on mount; listeners are removed
    // via on_cleanup inside install_ptt_hotkey.
    if is_tauri_shell() {
        install_agent_image_intake(wb, drop_state, status_line);
        install_ptt_hotkey(voice_handle, i18n, move |text: String, auto_send: bool| {
            if auto_send {
                draft.set(text);
                submit_turn(
                    wb,
                    i18n,
                    draft,
                    busy,
                    status_line,
                    timeline,
                    task_snapshot,
                    thinking_open,
                    voice_handle,
                );
            } else {
                draft.set(text);
                if let Some(id) = wb.active_id().get_untracked() {
                    wb.set_workspace_agent_compose_draft(id, draft.get_untracked());
                }
            }
        });
    }

    view! {
        <section
            class=move || {
                let mut class = "workbench-agent-pane".to_string();
                if chat_maximized.get() {
                    class.push_str(" workbench-agent-pane--chat-maximized");
                }
                match drop_state.get() {
                    DropZoneState::Accept => class.push_str(" workbench-agent-pane--drop-active"),
                    DropZoneState::Reject => class.push_str(" workbench-agent-pane--drop-reject"),
                    DropZoneState::Inactive => {}
                }
                class
            }
            aria-label=move || i18n.tr(I18nKey::AgAriaPane)()
            on:dragenter=move |ev| handle_dom_drag_event(ev, drop_state)
            on:dragover=move |ev| handle_dom_drag_event(ev, drop_state)
            on:dragleave=move |_| clear_drop_state(drop_state)
            on:drop=move |ev| handle_dom_drop(ev, wb, drop_state, status_line)
        >
            <Show when=move || drop_state.get().is_active()>
                <div class="agent-drop-overlay" aria-hidden="true">
                    <span>{move || drop_state.get().message()}</span>
                </div>
            </Show>
            <header class=move || {
                if chat_maximized.get() {
                    "agent-hero agent-hero--compact".to_string()
                } else {
                    "agent-hero".to_string()
                }
            }>
                <VoiceOrb
                    handle=voice_handle
                    on_transcript=move |text: String, auto_send: bool| {
                        if auto_send {
                            draft.set(text);
                            submit_turn(
                                wb,
                                i18n,
                                draft,
                                busy,
                                status_line,
                                timeline,
                                task_snapshot,
                                thinking_open,
                                voice_handle,
                            );
                        } else {
                            draft.set(text);
                            if let Some(id) = wb.active_id().get_untracked() {
                                wb.set_workspace_agent_compose_draft(id, draft.get_untracked());
                            }
                        }
                    }
                />
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
            <ContextSection context_open=context_open />

            <Show when=move || status_line.get().is_some()>
                {move || {
                    let txt = status_line.get().unwrap_or_default();
                    view! {
                        <p class="workbench-agent-status">{txt}</p>
                    }
                }}
            </Show>

            <article
                class="workbench-agent-scroll"
                aria-label=move || i18n.tr(I18nKey::AgChatArticleAria)()
            >
                <div class="agent-section__head agent-chat-head">
                    <h3>{move || i18n.tr(I18nKey::AgChatHeading)()}</h3>
                    <SessionCostChip wb=wb />
                    <div class="agent-chat-head__actions">
                        <span>{move || {
                            if timeline.get().is_empty() {
                                i18n.tr(I18nKey::AgBadgeReady)().to_string()
                            } else {
                                i18n.tr(I18nKey::AgBadgeLive)().to_string()
                            }
                        }}</span>
                        <button
                            type="button"
                            class=move || {
                                let mut c = String::from("agent-chat-head__image-mode");
                                if image_mode.get() {
                                    c.push_str(" agent-chat-head__image-mode--active");
                                }
                                c
                            }
                            prop:disabled=move || busy.get() || !is_tauri_shell()
                            title=move || i18n.tr(I18nKey::ImageModeToggleAria)()
                            aria-label=move || i18n.tr(I18nKey::ImageModeToggleAria)()
                            aria-pressed=move || if image_mode.get() { "true" } else { "false" }
                            on:click=move |_| {
                                let next = !image_mode.get_untracked();
                                image_mode.set(next);
                                if let Some(ws_id) = wb.active_id().get_untracked() {
                                    wb.set_workspace_agent_image_mode(ws_id, next);
                                }
                            }
                        >
                            <LxIcon icon=icondata::LuImagePlus width="0.86rem" height="0.86rem" />
                        </button>
                        <button
                            type="button"
                            class="agent-chat-head__icon-btn"
                            aria-pressed=move || if chat_maximized.get() { "true" } else { "false" }
                            title=move || {
                                if chat_maximized.get() {
                                    i18n.tr(I18nKey::AgChatRestore)().to_string()
                                } else {
                                    i18n.tr(I18nKey::AgChatMaximize)().to_string()
                                }
                            }
                            aria-label=move || {
                                if chat_maximized.get() {
                                    i18n.tr(I18nKey::AgChatRestore)().to_string()
                                } else {
                                    i18n.tr(I18nKey::AgChatMaximize)().to_string()
                                }
                            }
                            on:click=move |_| chat_maximized.update(|v| *v = !*v)
                        >
                            {move || {
                                if chat_maximized.get() {
                                    view! { <LxIcon icon=icondata::LuMinimize2 width="0.86rem" height="0.86rem" /> }.into_any()
                                } else {
                                    view! { <LxIcon icon=icondata::LuMaximize2 width="0.86rem" height="0.86rem" /> }.into_any()
                                }
                            }}
                        </button>
                        <button
                            type="button"
                            class="agent-chat-head__reset"
                            prop:disabled=move || busy.get() || !is_tauri_shell()
                            title=move || i18n.tr(I18nKey::AgResetChat)()
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
                            <LxIcon icon=icondata::LuEraser width="0.86rem" height="0.86rem" />
                        </button>
                    </div>
                </div>
                <Show when=move || image_mode.get()>
                    <p class="agent-chat-head__image-hint">
                        {move || i18n.tr(I18nKey::ImageModeHint)()}
                    </p>
                </Show>
                <div class="workbench-agent-chat-log" node_ref=chat_scroll_ref aria-live="polite">
                    <Show
                        when=move || !timeline.get().is_empty()
                        fallback=move || view! {
                            <div class="agent-chat-line agent-chat-line--agent">
                                <ChatLineIndexColumn
                                    line_no="01".to_string()
                                    tts_text=Some(i18n.tr(I18nKey::AgWelcomeBody)().to_string())
                                    voice_handle=voice_handle
                                />
                                <div class="agent-chat-body">
                                    <strong>"BLXCode"</strong>
                                    <p>{move || i18n.tr(I18nKey::AgWelcomeBody)()}</p>
                                </div>
                            </div>
                        }
                    >
                        <ol class="agent-chat-list" aria-label=move || i18n.tr(I18nKey::AgTimelineAria)()>
                            {move || {
                                let on_redo = Callback::new(move |text: String| {
                                    draft.set(text);
                                    submit_turn(
                                        wb, i18n, draft, busy, status_line,
                                        timeline, task_snapshot, thinking_open, voice_handle,
                                    );
                                });
                                compact_timeline(timeline.get())
                                    .into_iter()
                                    .enumerate()
                                    .map(|(idx, entry)| {
                                        view! { <TimelineRow idx=idx entry=entry i18n=i18n thinking_open=thinking_open voice_handle=voice_handle on_redo=on_redo timeline=timeline /> }
                                    })
                                    .collect_view()
                            }}
                        </ol>
                    </Show>
                </div>
            </article>

            <form
                class="agent-compose"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open, voice_handle);
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
                            submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open, voice_handle);
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
    voice_handle: VoiceOrbHandle,
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
                    wb.clear_chat_usage(ws_id);
                    status_line.set(None);
                }
                Err(msg) => status_line.set(Some(msg)),
            }
        });
        return;
    }

    let workspace_root = resolve_effective_workspace_root(&wb);
    let context_items = wb.agent_context_for_workspace_untracked(ws_id);
    let image_context_items = wb.pending_agent_images_for_workspace_untracked(ws_id);

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

    // Take + reset the voice flag — only this single turn is marked as voice.
    let voice_input = voice_handle.voice_pending.get_untracked()
        && voice_handle
            .settings
            .get_untracked()
            .map(|s| s.tts.enabled)
            .unwrap_or(false);
    voice_handle.voice_pending.set(false);

    // Image mode is workspace-scoped state (persisted in WorkspaceEntry);
    // read it here so every entry point — Enter key, submit button, voice
    // auto-send — honours the toggle without an extra arg.
    let image_generate = wb.agent_image_mode_for_workspace_untracked(ws_id);
    let turn = UserTurn {
        prompt,
        workspace_root,
        voice_input,
        image_generate,
        context_items,
        image_context_items,
    };

    let busy_sig = busy;
    let status_sig = status_line;
    let timeline_sig = timeline;
    let task_snapshot_sig = task_snapshot;
    let ws_capture = ws_id;
    let audio_ref = voice_handle.audio_ref;

    leptos::task::spawn_local(async move {
        if let Err(msg) = agent_submit_turn(turn).await {
            busy_sig.set(false);
            status_sig.set(Some(msg));
            return;
        }

        let i18n_d = i18n;
        let wb_d = wb;
        let subagent_debounce = SubagentEventDebounce::new();
        let flush_subagent = Rc::new(move |debounced: Vec<AgentEvent>| {
            let loc_now = i18n_d.locale().get_untracked();
            for ev in &debounced {
                apply_agent_event(
                    ev,
                    timeline_sig,
                    task_snapshot_sig,
                    loc_now,
                    Some((wb_d, ws_capture)),
                );
            }
        });
        if let Err(msg) = agent_drain_turn_opts(voice_input, move |batch| {
            let loc_now = i18n_d.locale().get_untracked();
            for ev in &batch {
                if matches!(ev, AgentEvent::VoiceReady { .. }) {
                    handle_voice_event(audio_ref, ev);
                    continue;
                }
                if let AgentEvent::ImageContextConsumed { ids } = ev {
                    wb_d.mark_workspace_agent_images_read(ws_capture, ids);
                    continue;
                }
                if is_subagent_timeline_event(ev) {
                    subagent_debounce.push(ev.clone(), flush_subagent.clone());
                    continue;
                }
                apply_agent_event(
                    ev,
                    timeline_sig,
                    task_snapshot_sig,
                    loc_now,
                    Some((wb_d, ws_capture)),
                );
                maybe_handle_client_tool(ev, wb_d);
            }
        })
        .await
        {
            status_sig.set(Some(msg));
        }
        busy_sig.set(false);
    });
}

/// Compact session-cost chip rendered in the chat header. Replaces the
/// retired `ChatUsageFooter`. Shows the resolved USD total + turn count;
/// hidden until the first `TurnUsage` event lands.
#[component]
fn SessionCostChip(wb: WorkbenchService) -> impl IntoView {
    use crate::workbench::agent_panel::turn_metrics_bar::fmt_cost;
    let i18n = expect_context::<I18nService>();
    let stats = Memo::new(move |_| {
        let id = wb.active_id().get()?;
        let s = wb.chat_usage_for_workspace(id);
        if s.turn_count == 0 {
            None
        } else {
            Some(s)
        }
    });

    let aria = move || lookup(i18n.locale().get(), I18nKey::AgSessionCostAria).to_string();
    view! {
        <Show when=move || stats.with(|s| s.is_some())>
            <div class="agent-chat-head__cost" aria-label=aria>
                {move || {
                    let s = stats.get().expect("Show gate");
                    let cost = fmt_cost(s.total_cost_usd);
                    let turns = s.turn_count;
                    let loc = i18n.locale().get();
                    let turn_label = lookup(
                        loc,
                        if turns == 1 { I18nKey::AgMetricsTurnsOne } else { I18nKey::AgMetricsTurnsMany },
                    );
                    view! {
                        <strong>{cost}</strong>
                        <span class="agent-chat-head__cost-sep">"·"</span>
                        <span>{format!("{turns} {turn_label}")}</span>
                    }
                }}
            </div>
        </Show>
    }
}
