//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
mod ask_user_card;
mod client_tools;
mod context_list;
mod context_meter;
mod image_context;
mod reducer;
mod task_list;
mod timeline;
pub(crate) mod turn_metrics_bar;
mod voice_orb;

use crate::agent_wire::{AgentContextKind, AgentEvent, EventEnvelope, TaskSnapshot, UserTurn};
use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_abort, agent_active_context_window, agent_clear_conversation, agent_compact_conversation,
    agent_drain_turn_opts, agent_settings_get, agent_submit_turn, is_tauri_shell,
    tasks_list as fetch_tasks_list,
};
use crate::workbench::agent_panel::context_meter::{fmt_tokens, ContextMeter};
use crate::workbench::agent_panel::client_tools::maybe_handle_client_tool;
use crate::workbench::agent_panel::context_list::ContextSection;
use crate::workbench::agent_panel::image_context::{
    clear_drop_state, handle_dom_drag_event, handle_dom_drop, install_agent_image_intake,
    DropZoneState,
};
use crate::workbench::agent_panel::reducer::apply_envelope;
use crate::workbench::agent_panel::task_list::TaskSection;
use crate::workbench::agent_panel::timeline::{ChatLineIndexColumn, TurnNodeView};
use crate::workbench::agent_panel::voice_orb::{
    handle_voice_event, install_ptt_hotkey, VoiceOrb, VoiceOrbHandle,
};
use crate::workbench::agent_timeline::TimelineDoc;
use crate::workbench::terminal_slot_dnd::TerminalSlotDragService;
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
    let slot_dnd = expect_context::<TerminalSlotDragService>();

    let draft = RwSignal::new(String::new());
    let timeline = RwSignal::new(TimelineDoc::default());
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
    // Context-window meter + compaction state.
    let context_length = RwSignal::new(Option::<u64>::None);
    let auto_compact_enabled = RwSignal::new(true);
    let auto_compact_threshold = RwSignal::new(85u8);
    let compacting = RwSignal::new(false);
    // Re-armed once occupancy drops back below the threshold so auto-compact
    // fires at most once per crossing (no compaction storm).
    let auto_compact_armed = RwSignal::new(true);
    let chat_scroll_ref = NodeRef::<html::Div>::new();
    let compose_input_ref = NodeRef::<html::Input>::new();
    // Refocus the compose input whenever the agent finishes (busy → false).
    Effect::new(move |_| {
        if !busy.get() {
            if let Some(el) = compose_input_ref.get() {
                let _ = el.focus();
            }
        }
    });
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
    let tool_detail_open = RwSignal::new(HashMap::<String, bool>::new());

    // Load authoritative timeline + compose draft when the active workspace
    // changes only (do not subscribe to `workspaces` — timeline writes would
    // reset thinking UI and fight streaming).
    Effect::new(move |_| {
        let active = wb.active_id().get();
        let Some(id) = active else {
            timeline.set(TimelineDoc::default());
            thinking_open.set(HashMap::new());
            tool_detail_open.set(HashMap::new());
            draft.set(String::new());
            image_mode.set(false);
            return;
        };
        timeline.set(wb.agent_timeline_for_workspace_untracked(id));
        thinking_open.set(HashMap::new());
        tool_detail_open.set(HashMap::new());
        draft.set(wb.agent_compose_draft_for_workspace_untracked(id));
        image_mode.set(wb.agent_image_mode_for_workspace_untracked(id));
    });

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                model_label.set(format!("{}/{}", view.provider.as_str(), view.model_id));
                auto_compact_enabled.set(view.auto_compact_enabled);
                auto_compact_threshold.set(view.auto_compact_threshold_pct);
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
        // Resolve the active model's context-window size. Re-runs on
        // workspace switch and whenever a turn finishes (busy → false), so a
        // model change in Settings is picked up and fresh settings (auto-
        // compact toggle/threshold) stay in sync.
        Effect::new(move |_| {
            let _ = wb.active_id().get();
            let _ = busy.get();
            leptos::task::spawn_local(async move {
                if let Ok(info) = agent_active_context_window().await {
                    context_length.set(info.context_length);
                }
                if let Ok(view) = agent_settings_get().await {
                    auto_compact_enabled.set(view.auto_compact_enabled);
                    auto_compact_threshold.set(view.auto_compact_threshold_pct);
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
                crate::workbench::agent_context_handoff::store_task_snapshot(ws_id, next.clone());
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

    // Autoscroll while following the stream (near bottom or new row). Skip when
    // the user scrolled up to read/expand older tool or thinking blocks.
    const AUTOSCROLL_PX: i32 = 80;
    let last_timeline_len = StoredValue::new(0usize);
    Effect::new(move |_| {
        let len = timeline.with(|t| t.display_len());
        let prev_len = last_timeline_len.get_value();
        last_timeline_len.set_value(len);
        let Some(log) = chat_scroll_ref.get() else {
            return;
        };
        let scroll_top = log.scroll_top();
        let scroll_height = log.scroll_height();
        let client_height = log.client_height();
        let near_bottom = scroll_height - scroll_top - client_height < AUTOSCROLL_PX;
        if len > prev_len || near_bottom {
            log.set_scroll_top(scroll_height);
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
                    tool_detail_open,
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

    // Summarize the running session and start fresh from the compacted
    // briefing. Shared by the header Compact button (`manual = true`) and the
    // auto-compact watcher (`manual = false`).
    let run_compaction = move |manual: bool| {
        if compacting.get_untracked() || busy.get_untracked() {
            return;
        }
        let Some(ws_id) = wb.active_id().get_untracked() else {
            return;
        };
        let used = wb.chat_usage_for_workspace(ws_id).last_round_input_tokens;
        compacting.set(true);
        status_line.set(Some(i18n.tr(I18nKey::AgCompactRunning)().to_string()));
        leptos::task::spawn_local(async move {
            let current = (used > 0).then_some(used);
            match agent_compact_conversation(current).await {
                Ok(result) => {
                    // Start fresh: the backend now holds only the compacted
                    // summary, so reset the visible timeline to match.
                    timeline.set(TimelineDoc::default());
                    thinking_open.set(HashMap::new());
                    tool_detail_open.set(HashMap::new());
                    wb.set_workspace_agent_timeline(ws_id, TimelineDoc::default());
                    wb.set_last_round_input_tokens(ws_id, result.after_tokens_estimate);
                    // Re-arming is handled by the auto-compact watcher once
                    // occupancy is observed below the threshold again — avoids
                    // a compaction storm if a summary is still large.
                    if manual {
                        let done = i18n.tr(I18nKey::AgCompactDone)().to_string();
                        status_line.set(Some(format!(
                            "{done} · {} → {} tok",
                            fmt_tokens(result.before_tokens),
                            fmt_tokens(result.after_tokens_estimate)
                        )));
                    } else {
                        status_line
                            .set(Some(i18n.tr(I18nKey::AgAutoCompactStatus)().to_string()));
                    }
                }
                Err(e) if e == "nothing-to-compact" => {
                    status_line.set(if manual {
                        Some(i18n.tr(I18nKey::AgCompactNothing)().to_string())
                    } else {
                        None
                    });
                }
                Err(e) => status_line.set(Some(e)),
            }
            compacting.set(false);
        });
    };

    // Auto-compact watcher: on the busy true→false edge (a turn just
    // finished), compact once if occupancy crossed the threshold. Re-arms
    // only after occupancy drops back below it, so it fires at most once per
    // crossing and never mid-turn.
    Effect::new(move |prev: Option<bool>| {
        let now_busy = busy.get();
        let finished = prev == Some(true) && !now_busy;
        if finished && is_tauri_shell() && auto_compact_enabled.get_untracked() {
            if let (Some(max), Some(ws_id)) =
                (context_length.get_untracked(), wb.active_id().get_untracked())
            {
                if max > 0 && !compacting.get_untracked() {
                    let used = wb.chat_usage_for_workspace(ws_id).last_round_input_tokens;
                    let pct = (used as f64 / max as f64) * 100.0;
                    let threshold = auto_compact_threshold.get_untracked() as f64;
                    if pct < threshold {
                        auto_compact_armed.set(true);
                    } else if auto_compact_armed.get_untracked() {
                        auto_compact_armed.set(false);
                        run_compaction(false);
                    }
                }
            }
        }
        now_busy
    });

    view! {
        <section
            class=move || {
                let mut class = "workbench-agent-pane".to_string();
                if chat_maximized.get() {
                    class.push_str(" workbench-agent-pane--chat-maximized");
                }
                match drop_state.get() {
                    DropZoneState::AcceptImage | DropZoneState::AcceptTerminal => {
                        class.push_str(" workbench-agent-pane--drop-active")
                    }
                    DropZoneState::Reject => class.push_str(" workbench-agent-pane--drop-reject"),
                    DropZoneState::Inactive => {}
                }
                class
            }
            aria-label=move || i18n.tr(I18nKey::AgAriaPane)()
            on:dragenter=move |ev| handle_dom_drag_event(ev, drop_state, slot_dnd)
            on:dragover=move |ev| handle_dom_drag_event(ev, drop_state, slot_dnd)
            on:dragleave=move |_| clear_drop_state(drop_state)
            on:drop=move |ev| handle_dom_drop(ev, wb, drop_state, status_line, slot_dnd)
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
                                tool_detail_open,
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
                    <ContextMeter wb=wb context_length=context_length />
                    <div class="agent-chat-head__actions">
                        <button
                            type="button"
                            class="agent-chat-head__icon-btn"
                            prop:disabled=move || busy.get() || compacting.get() || !is_tauri_shell()
                            title=move || i18n.tr(I18nKey::AgCompactSession)()
                            aria-label=move || i18n.tr(I18nKey::AgCompactSessionAria)()
                            on:click=move |_| run_compaction(true)
                        >
                            <LxIcon icon=icondata::LuShrink width="0.86rem" height="0.86rem" />
                        </button>
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
                                let tool_detail_open = tool_detail_open;
                                leptos::task::spawn_local(async move {
                                    let Some(ws_id) = wb.active_id().get_untracked() else {
                                        status_line.set(Some("Select a workspace tab first.".into()));
                                        return;
                                    };
                                    match agent_clear_conversation().await {
                                        Ok(()) => {
                                            timeline.set(TimelineDoc::default());
                                            thinking_open.set(HashMap::new());
                                            tool_detail_open.set(HashMap::new());
                                            draft.set(String::new());
                                            wb.set_workspace_agent_timeline(ws_id, TimelineDoc::default());
                                            wb.set_workspace_agent_compose_draft(ws_id, String::new());
                                            wb.clear_chat_usage(ws_id);
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
                            {
                                let on_redo = Callback::new(move |text: String| {
                                    draft.set(text);
                                    submit_turn(
                                        wb, i18n, draft, busy, status_line,
                                        timeline, task_snapshot, thinking_open, tool_detail_open, voice_handle,
                                    );
                                });
                                view! {
                                    {move || {
                                        timeline.get()
                                            .turns
                                            .into_iter()
                                            .enumerate()
                                            .map(|(idx, turn)| {
                                            view! {
                                                <TurnNodeView
                                                    idx=idx
                                                    turn=turn
                                                    i18n=i18n
                                                    thinking_open=thinking_open
                                                    tool_detail_open=tool_detail_open
                                                    voice_handle=voice_handle
                                                    timeline=timeline
                                                    wb=wb
                                                    workspace_id=wb.active_id().get_untracked()
                                                    on_redo=on_redo
                                                />
                                            }
                                            })
                                            .collect_view()
                                    }}
                                }
                            }
                        </ol>
                    </Show>
                </div>
            </article>

            <form
                class="agent-compose"
                on:submit=move |ev| {
                    ev.prevent_default();
                    submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open, tool_detail_open, voice_handle);
                }
            >
                <input
                    type="text"
                    node_ref=compose_input_ref
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
                            submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open, tool_detail_open, voice_handle);
                        }
                    }
                />

                <div class="workbench-agent-actions">
                    <button
                        type="button"
                        class=move || {
                            if busy.get() {
                                "workbench-mini-btn agent-cancel-btn"
                            } else {
                                "workbench-mini-btn workbench-mini-btn--primary agent-send-btn"
                            }
                        }
                        on:mousedown=|ev| ev.prevent_default()
                        on:click=move |_| {
                            if busy.get_untracked() {
                                leptos::task::spawn_local(async move {
                                    let _ = agent_abort().await;
                                });
                            } else {
                                submit_turn(wb, i18n, draft, busy, status_line, timeline, task_snapshot, thinking_open, tool_detail_open, voice_handle);
                            }
                        }
                    >
                        {move || if busy.get() {
                            view! { <LxIcon icon=icondata::LuSquare width="0.9rem" height="0.9rem" /> }.into_any()
                        } else {
                            view! { <LxIcon icon=icondata::LuSparkles width="0.9rem" height="0.9rem" /> }.into_any()
                        }}
                        <span>{move || if busy.get() {
                            i18n.tr(I18nKey::AgCancel)()
                        } else {
                            i18n.tr(I18nKey::AgSend)()
                        }}</span>
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
    timeline: RwSignal<TimelineDoc>,
    task_snapshot: RwSignal<TaskSnapshot>,
    thinking_open: RwSignal<HashMap<usize, bool>>,
    tool_detail_open: RwSignal<HashMap<String, bool>>,
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
                    timeline.set(TimelineDoc::default());
                    thinking_open.set(HashMap::new());
                    tool_detail_open.set(HashMap::new());
                    wb.set_workspace_agent_timeline(ws_id, TimelineDoc::default());
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
    let transient_context_ids = transient_agent_context_ids(&context_items);
    let image_context_items = wb.pending_agent_images_for_workspace_untracked(ws_id);

    timeline.update(|doc| doc.push_user_turn_with_pending(prompt.clone()));
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
    let turn_had_error = RwSignal::new(false);

    leptos::task::spawn_local(async move {
        if let Err(msg) = agent_submit_turn(turn).await {
            busy_sig.set(false);
            status_sig.set(Some(msg));
            return;
        }

        let i18n_d = i18n;
        let wb_d = wb;
        let wb_after_drain = wb;
        if let Err(msg) = agent_drain_turn_opts(voice_input, move |batch: Vec<EventEnvelope>| {
            let loc_now = i18n_d.locale().get_untracked();
            for env in &batch {
                let ev = &env.event;
                if matches!(ev, AgentEvent::Error { .. }) {
                    turn_had_error.set(true);
                }
                if matches!(ev, AgentEvent::VoiceReady { .. }) {
                    handle_voice_event(audio_ref, ev);
                    continue;
                }
                if let AgentEvent::ImageContextConsumed { ids } = ev {
                    wb_d.mark_workspace_agent_images_read(ws_capture, ids);
                    continue;
                }
                apply_envelope(
                    env,
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
        } else if !turn_had_error.get_untracked() && !transient_context_ids.is_empty() {
            wb_after_drain.remove_workspace_agent_context_items(ws_capture, &transient_context_ids);
        }
        busy_sig.set(false);
    });
}

fn transient_agent_context_ids(items: &[crate::agent_wire::AgentContextItem]) -> Vec<String> {
    items
        .iter()
        .filter(|item| {
            matches!(
                item.kind,
                AgentContextKind::TerminalSession | AgentContextKind::FileSnippet
            )
        })
        .map(|item| item.id.clone())
        .collect()
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
