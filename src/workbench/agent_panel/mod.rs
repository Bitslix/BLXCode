//! Agent Composer: Prompt → Tauri-Orchestrierung, Drain der Event-Liste in die Ansicht.
mod ask_user_card;
mod changed_files_card;
mod client_tools;
mod composer;
mod context_list;
mod context_meter;
mod diagram_result;
mod image_context;
mod reducer;
mod session_stats;
mod task_list;
mod timeline;
mod tool_group;
pub(crate) mod turn_metrics_bar;
mod voice_orb;

use crate::agent_wire::{
    AgentChatMode, AgentContextKind, AgentEvent, EventEnvelope, TaskSnapshot, TurnMetrics,
    UserTurn, WorkspaceScope, WorkspaceWorktreeMeta,
};
use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_abort, agent_active_context_window, agent_clear_conversation, agent_compact_conversation,
    agent_drain_turn_opts, agent_enhance_prompt, agent_generate_chat_title, agent_settings_get,
    agent_submit_turn, git_is_repository, git_status_changes, is_tauri_shell,
    tasks_list as fetch_tasks_list, workbench_upsert_agent_notification, AgentNotificationInput,
};
use crate::workbench::agent_panel::client_tools::maybe_handle_client_tool;
use crate::workbench::agent_panel::composer::Composer;
use crate::workbench::agent_panel::context_list::ContextSection;
use crate::workbench::agent_panel::context_meter::fmt_tokens;
use crate::workbench::agent_panel::image_context::{
    clear_drop_state, handle_dom_drag_event, handle_dom_drop, install_agent_image_intake,
    DropZoneState,
};
use crate::workbench::agent_panel::reducer::apply_envelope;
use crate::workbench::agent_panel::session_stats::{
    latest_active_thinking_text, AgentSessionStats,
};
use crate::workbench::agent_panel::task_list::TaskSection;
use crate::workbench::agent_panel::timeline::{
    AgentTimelineName, ChatLineIndexColumn, TurnNodeView,
};
use crate::workbench::agent_panel::voice_orb::{handle_voice_event, VoiceOrb, VoiceOrbHandle};
use crate::workbench::agent_timeline::{ChangedFileEntry, TimelineDoc, TurnPart};
use crate::workbench::terminal_slot_dnd::TerminalSlotDragService;
use crate::workbench::{AgentChatSessionStatus, RightPanelTab, WorkbenchService};
use gloo_timers::future::TimeoutFuture;
use leptos::html;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use send_wrapper::SendWrapper;
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

const THINKING_IDLE_MESSAGES: [&str; 10] = [
    "Dangling",
    "Baking",
    "Tracing",
    "Stitching",
    "Weighing",
    "Sketching",
    "Linking",
    "Sorting",
    "Composing",
    "Refining",
];

const DEFAULT_AGENT_TIMELINE_NAME: &str = "BLXCody";

fn resolve_agent_timeline_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        DEFAULT_AGENT_TIMELINE_NAME.to_string()
    } else {
        trimmed.to_string()
    }
}

fn fallback_chat_session_title(prompt: &str) -> String {
    let first_line = prompt
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(prompt);
    let words = first_line
        .split_whitespace()
        .take(7)
        .collect::<Vec<_>>()
        .join(" ");
    let candidate = if words.is_empty() {
        prompt.trim().to_string()
    } else {
        words
    };
    truncate_title(candidate.trim_matches(['.', ':', '-', ' ']).trim(), 44)
}

fn truncate_title(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect::<String>()
}

fn agent_panel_visible_and_focused(wb: WorkbenchService) -> bool {
    if wb.right_collapsed().get_untracked()
        || wb.right_active_tab().get_untracked() != RightPanelTab::Agent
    {
        return false;
    }
    js_sys::eval(
        r#"(() => {
          try {
            return document.visibilityState === "visible" && document.hasFocus();
          } catch (_) {
            return false;
          }
        })()"#,
    )
    .ok()
    .and_then(|v| v.as_bool())
    .unwrap_or(false)
}

fn spawn_agent_notification_fallback(
    wb: WorkbenchService,
    kind: &str,
    title: &str,
    body: Option<String>,
    dedupe_key: &str,
    target: Option<serde_json::Value>,
) {
    if agent_panel_visible_and_focused(wb) || !is_tauri_shell() {
        return;
    }
    let input = AgentNotificationInput {
        id: None,
        title: title.to_string(),
        body,
        kind: kind.to_string(),
        severity: Some(if kind == "error" { "error" } else { "info" }.to_string()),
        source: Some("agent".into()),
        target,
        dedupe_key: Some(dedupe_key.to_string()),
        read: Some(false),
        sent: Some(true),
    };
    let os_title = input.title.clone();
    let os_body = input.body.clone().unwrap_or_default();
    leptos::task::spawn_local(async move {
        if let Ok(item) = workbench_upsert_agent_notification(input).await {
            wb.upsert_agent_notification(item);
            send_native_notification_best_effort(&os_title, &os_body);
            crate::workbench::notification_sound::play_notification_beep();
        }
    });
}

fn send_native_notification_best_effort(title: &str, body: &str) {
    let title = serde_json::to_string(title).unwrap_or_else(|_| "\"BLXCode Agent\"".into());
    let body = serde_json::to_string(body).unwrap_or_else(|_| "\"\"".into());
    let script = format!(
        r#"(() => {{
          try {{
            const n = window.__TAURI__ && window.__TAURI__.notification;
            if (!n) return;
            Promise.resolve(n.isPermissionGranted())
              .then((granted) => granted ? "granted" : n.requestPermission())
              .then((permission) => {{
                if (permission === "granted" || permission === true) {{
                  n.sendNotification({{ title: {title}, body: {body} }});
                }}
              }})
              .catch(() => {{}});
          }} catch (_) {{}}
        }})()"#
    );
    let _ = js_sys::eval(&script);
}

fn refresh_agent_timeline_name(signal: RwSignal<String>) {
    if !is_tauri_shell() {
        return;
    }
    leptos::task::spawn_local(async move {
        if let Ok(view) = agent_settings_get().await {
            signal.set(resolve_agent_timeline_name(&view.agent_nickname));
        }
    });
}

fn agent_session_status_label(i18n: I18nService, status: AgentChatSessionStatus) -> String {
    let key = match status {
        AgentChatSessionStatus::Idle => I18nKey::AgSessionIdle,
        AgentChatSessionStatus::Running => I18nKey::AgSessionRunning,
        AgentChatSessionStatus::NeedsInput => I18nKey::AgSessionNeedsInput,
        AgentChatSessionStatus::Error => I18nKey::AgSessionErrored,
        AgentChatSessionStatus::Restored => I18nKey::AgSessionRestored,
    };
    i18n.tr(key)().to_string()
}

fn agent_session_status_class(status: AgentChatSessionStatus) -> &'static str {
    match status {
        AgentChatSessionStatus::Idle => "agent-chat-session-tab__status--idle",
        AgentChatSessionStatus::Running => "agent-chat-session-tab__status--running",
        AgentChatSessionStatus::NeedsInput => "agent-chat-session-tab__status--needs-input",
        AgentChatSessionStatus::Error => "agent-chat-session-tab__status--error",
        AgentChatSessionStatus::Restored => "agent-chat-session-tab__status--restored",
    }
}

#[component]
pub fn AgentPanelDock() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let slot_dnd = expect_context::<TerminalSlotDragService>();
    let context_dnd = expect_context::<crate::workbench::context_drag::ContextDragService>();
    let kanban_dnd = expect_context::<crate::workbench::kanban_dnd::KanbanDragService>();

    let draft = RwSignal::new(String::new());
    let timeline = RwSignal::new(TimelineDoc::default());
    let active_session_id = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let status_line = RwSignal::new(Option::<String>::None);
    let tasks_open = RwSignal::new(false);
    let context_open = RwSignal::new(false);
    let drop_state = RwSignal::new(DropZoneState::Inactive);
    let model_label = RwSignal::new(String::new());
    let agent_timeline_name = RwSignal::new(DEFAULT_AGENT_TIMELINE_NAME.to_string());
    provide_context(AgentTimelineName(agent_timeline_name));
    // Per-session view of the persisted `WorkspaceEntry.agent_image_mode`.
    // Synced on workspace switch and on toggle (writer also persists to the
    // workspace entry so the flag survives reloads).
    let image_mode = RwSignal::new(false);
    let chat_mode = RwSignal::new(AgentChatMode::AskEdits);
    let enhance_prompt = RwSignal::new(false);
    let chat_maximized = RwSignal::new(false);
    let head_tooltips_suppressed = RwSignal::new(false);
    // Context-window meter + compaction state.
    let context_length = RwSignal::new(Option::<u64>::None);
    let auto_compact_enabled = RwSignal::new(true);
    let auto_compact_threshold = RwSignal::new(85u8);
    let compacting = RwSignal::new(false);
    // Re-armed once occupancy drops back below the threshold so auto-compact
    // fires at most once per crossing (no compaction storm).
    let auto_compact_armed = RwSignal::new(true);
    let chat_scroll_ref = NodeRef::<html::Div>::new();
    let compose_input_ref = NodeRef::<html::Textarea>::new();
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
            active_session_id.set(String::new());
            thinking_open.set(HashMap::new());
            tool_detail_open.set(HashMap::new());
            draft.set(String::new());
            image_mode.set(false);
            chat_mode.set(AgentChatMode::AskEdits);
            enhance_prompt.set(false);
            return;
        };
        active_session_id.set(wb.active_agent_chat_session_id_for_workspace(id));
        timeline.set(wb.agent_timeline_for_workspace_untracked(id));
        thinking_open.set(HashMap::new());
        tool_detail_open.set(HashMap::new());
        draft.set(wb.agent_compose_draft_for_workspace_untracked(id));
        image_mode.set(wb.agent_image_mode_for_workspace_untracked(id));
        chat_mode.set(wb.agent_chat_mode_for_workspace_untracked(id));
        enhance_prompt.set(wb.agent_enhance_prompt_for_workspace_untracked(id));
    });

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                model_label.set(format!("{}/{}", view.provider.as_str(), view.model_id));
                agent_timeline_name.set(resolve_agent_timeline_name(&view.agent_nickname));
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
                    agent_timeline_name.set(resolve_agent_timeline_name(&view.agent_nickname));
                    auto_compact_enabled.set(view.auto_compact_enabled);
                    auto_compact_threshold.set(view.auto_compact_threshold_pct);
                }
            });
        });
        let settings_change_handle =
            window_event_listener_untyped("blxcode-agent-settings-changed", move |_| {
                refresh_agent_timeline_name(agent_timeline_name);
            });
        on_cleanup(move || drop(settings_change_handle));
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

    // The compact tasks bar stays collapsed by default; the user toggles the
    // full list open via its chevron.
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
        install_agent_image_intake(wb, i18n, drop_state, status_line);
    }

    // Push-to-talk transcripts targeting the agent composer arrive via the
    // shared `PttBus` (the window-level handler lives in `ptt_runtime`). The
    // mic-button click/hold path on the `VoiceOrb` is unaffected.
    if let Some(bus) = use_context::<crate::workbench::ptt_runtime::PttBus>() {
        Effect::new(move |_| {
            let Some((text, auto_submit)) = bus.agent_transcript.get() else {
                return;
            };
            bus.agent_transcript.set(None);
            if text.trim().is_empty() {
                return;
            }
            draft.set(text);
            if auto_submit {
                submit_turn(
                    wb,
                    i18n,
                    draft,
                    chat_mode,
                    enhance_prompt,
                    busy,
                    status_line,
                    timeline,
                    task_snapshot,
                    thinking_open,
                    tool_detail_open,
                    voice_handle,
                    true,
                );
            } else if let Some(id) = wb.active_id().get_untracked() {
                wb.set_workspace_agent_compose_draft(id, draft.get_untracked());
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
        let session_id = wb.active_agent_chat_session_id_for_workspace(ws_id);
        let used = wb.chat_usage_for_workspace(ws_id).last_round_input_tokens;
        compacting.set(true);
        status_line.set(Some(i18n.tr(I18nKey::AgCompactRunning)().to_string()));
        crate::app_log::info(
            "agent",
            "compaction_started",
            serde_json::json!({ "manual": manual, "tokens": used }),
        );
        leptos::task::spawn_local(async move {
            let current = (used > 0).then_some(used);
            match agent_compact_conversation(Some(session_id.clone()), current).await {
                Ok(result) => {
                    crate::app_log::info(
                        "agent",
                        "compaction_finished",
                        serde_json::json!({
                            "manual": manual,
                            "beforeTokens": result.before_tokens,
                            "afterTokensEstimate": result.after_tokens_estimate,
                        }),
                    );
                    // Start fresh: the backend now holds only the compacted
                    // summary, so reset the visible timeline to match.
                    timeline.set(TimelineDoc::default());
                    thinking_open.set(HashMap::new());
                    tool_detail_open.set(HashMap::new());
                    wb.set_workspace_agent_session_timeline(
                        ws_id,
                        &session_id,
                        TimelineDoc::default(),
                    );
                    wb.set_session_last_round_input_tokens(
                        ws_id,
                        &session_id,
                        result.after_tokens_estimate,
                    );
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
                        status_line.set(Some(i18n.tr(I18nKey::AgAutoCompactStatus)().to_string()));
                    }
                }
                Err(e) if e == "nothing-to-compact" => {
                    crate::app_log::info(
                        "agent",
                        "compaction_skipped",
                        serde_json::json!({ "manual": manual, "reason": "nothing-to-compact" }),
                    );
                    status_line.set(if manual {
                        Some(i18n.tr(I18nKey::AgCompactNothing)().to_string())
                    } else {
                        None
                    });
                }
                Err(e) => {
                    crate::app_log::error(
                        "agent",
                        "compaction_failed",
                        serde_json::json!({ "manual": manual, "error": e.clone() }),
                    );
                    status_line.set(Some(e));
                }
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
            if let (Some(max), Some(ws_id)) = (
                context_length.get_untracked(),
                wb.active_id().get_untracked(),
            ) {
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
                    DropZoneState::AcceptImage
                    | DropZoneState::AcceptTerminal
                    | DropZoneState::AcceptFile
                    | DropZoneState::AcceptFolder
                    | DropZoneState::AcceptDiff
                    | DropZoneState::AcceptCommit
                    | DropZoneState::AcceptPlan
                    | DropZoneState::AcceptTask => {
                        class.push_str(" workbench-agent-pane--drop-active")
                    }
                    DropZoneState::Reject => class.push_str(" workbench-agent-pane--drop-reject"),
                    DropZoneState::Inactive => {}
                }
                class
            }
            aria-label=move || i18n.tr(I18nKey::AgAriaPane)()
            on:dragenter=move |ev| handle_dom_drag_event(ev, wb, i18n, drop_state, slot_dnd, context_dnd, kanban_dnd)
            on:dragover=move |ev| handle_dom_drag_event(ev, wb, i18n, drop_state, slot_dnd, context_dnd, kanban_dnd)
            on:dragleave=move |_| clear_drop_state(drop_state)
            on:drop=move |ev| handle_dom_drop(ev, wb, i18n, drop_state, status_line, slot_dnd, context_dnd, kanban_dnd)
        >
            <Show when=move || drop_state.get().is_active()>
                <div class="agent-drop-overlay" aria-hidden="true">
                    <span>{move || drop_state.get().message(i18n)}</span>
                </div>
            </Show>
            <header class=move || {
                if chat_maximized.get() {
                    "agent-hero agent-hero--compact".to_string()
                } else {
                    "agent-hero".to_string()
                }
            }>
                <AgentSessionStats
                    timeline=timeline
                    wb=wb
                    context_length=context_length
                    model_label=model_label
                    busy=busy
                />
                <VoiceOrb
                    handle=voice_handle
                    compact=Signal::derive(move || chat_maximized.get())
                    thinking=busy
                    on_transcript=move |text: String, auto_send: bool| {
                        if auto_send {
                            draft.set(text);
                            submit_turn(
                                wb,
                                i18n,
                                draft,
                                chat_mode,
                                enhance_prompt,
                                busy,
                                status_line,
                                timeline,
                                task_snapshot,
                                thinking_open,
                                tool_detail_open,
                                voice_handle,
                                true,
                            );
                        } else {
                            draft.set(text);
                            if let Some(id) = wb.active_id().get_untracked() {
                                wb.set_workspace_agent_compose_draft(id, draft.get_untracked());
                            }
                        }
                    }
                />
                <AgentThinkingStream timeline=timeline />
            </header>

            <TaskSection
                snapshot=task_snapshot
                busy=busy
                tasks_open=tasks_open
                timeline=timeline
                wb=wb
            />
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
                    <div
                        class="agent-chat-head__actions"
                        class:agent-chat-head__actions--tooltips-suppressed=move || {
                            head_tooltips_suppressed.get()
                        }
                        on:pointerdown=move |_| head_tooltips_suppressed.set(true)
                        on:mouseleave=move |_| head_tooltips_suppressed.set(false)
                    >
                        <span class="blx-tip-anchor blx-tip-anchor--left agent-chat-head__tip">
                            <button
                                type="button"
                                class="agent-chat-head__icon-btn"
                                aria-describedby="agent-chat-new-session-tooltip"
                                aria-label=move || i18n.tr(I18nKey::AgNewSession)()
                                on:click=move |_| {
                                    let Some(ws_id) = wb.active_id().get_untracked() else {
                                        return;
                                    };
                                    if let Some(session_id) = wb.create_agent_chat_session(ws_id) {
                                        active_session_id.set(session_id);
                                        timeline.set(wb.agent_timeline_for_workspace_untracked(ws_id));
                                        thinking_open.set(HashMap::new());
                                        tool_detail_open.set(HashMap::new());
                                        draft.set(wb.agent_compose_draft_for_workspace_untracked(ws_id));
                                        image_mode.set(wb.agent_image_mode_for_workspace_untracked(ws_id));
                                        chat_mode.set(wb.agent_chat_mode_for_workspace_untracked(ws_id));
                                        enhance_prompt.set(wb.agent_enhance_prompt_for_workspace_untracked(ws_id));
                                        busy.set(false);
                                        status_line.set(None);
                                    }
                                }
                            >
                                <LxIcon icon=icondata::LuPlus width="0.86rem" height="0.86rem" />
                            </button>
                            <span id="agent-chat-new-session-tooltip" class="blx-tooltip agent-chat-head__tooltip" role="tooltip">
                                <span class="blx-tooltip__eyebrow">
                                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                                    {move || i18n.tr(I18nKey::AgChatHeading)()}
                                </span>
                                <span class="blx-tooltip__main">{move || i18n.tr(I18nKey::AgNewSession)()}</span>
                            </span>
                        </span>
                        <span class="blx-tip-anchor blx-tip-anchor--left agent-chat-head__tip">
                            <button
                                type="button"
                                class="agent-chat-head__icon-btn"
                                prop:disabled=move || busy.get() || compacting.get() || !is_tauri_shell()
                                aria-describedby="agent-chat-compact-tooltip"
                                aria-label=move || i18n.tr(I18nKey::AgCompactSessionAria)()
                                on:click=move |_| run_compaction(true)
                            >
                                <LxIcon icon=icondata::LuShrink width="0.86rem" height="0.86rem" />
                            </button>
                            <span id="agent-chat-compact-tooltip" class="blx-tooltip agent-chat-head__tooltip" role="tooltip">
                                <span class="blx-tooltip__eyebrow">
                                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                                    "Chat log"
                                </span>
                                <span class="blx-tooltip__main">{move || i18n.tr(I18nKey::AgCompactSession)()}</span>
                                <span class="blx-tooltip__hint">{move || i18n.tr(I18nKey::AgCompactSessionAria)()}</span>
                            </span>
                        </span>
                        <span class="blx-tip-anchor blx-tip-anchor--left agent-chat-head__tip">
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
                                aria-describedby="agent-chat-image-mode-tooltip"
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
                            <span id="agent-chat-image-mode-tooltip" class="blx-tooltip agent-chat-head__tooltip" role="tooltip">
                                <span class="blx-tooltip__eyebrow">
                                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                                    "Image"
                                </span>
                                <span class="blx-tooltip__main">{move || i18n.tr(I18nKey::ImageModeToggleAria)()}</span>
                                <span class="blx-tooltip__hint">"Attach images or generate visual output"</span>
                            </span>
                        </span>
                        <span class="blx-tip-anchor blx-tip-anchor--left agent-chat-head__tip">
                            <button
                                type="button"
                                class="agent-chat-head__icon-btn"
                                aria-describedby="agent-chat-jump-bottom-tooltip"
                                aria-label=move || i18n.tr(I18nKey::AgentPanelJumpToBottom)()
                                on:click=move |_| {
                                    if let Some(log) = chat_scroll_ref.get_untracked() {
                                        smooth_scroll_chat_to_bottom(log);
                                    }
                                }
                            >
                                <LxIcon icon=icondata::LuArrowDownToLine width="0.86rem" height="0.86rem" />
                            </button>
                            <span id="agent-chat-jump-bottom-tooltip" class="blx-tooltip agent-chat-head__tooltip" role="tooltip">
                                <span class="blx-tooltip__eyebrow">
                                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                                    "Timeline"
                                </span>
                                <span class="blx-tooltip__main">{move || i18n.tr(I18nKey::AgentPanelJumpToBottom)()}</span>
                                <span class="blx-tooltip__hint">"Slide to the latest output"</span>
                            </span>
                        </span>
                        <span class="blx-tip-anchor blx-tip-anchor--left agent-chat-head__tip">
                            <button
                                type="button"
                                class="agent-chat-head__icon-btn"
                                aria-pressed=move || if chat_maximized.get() { "true" } else { "false" }
                                aria-describedby="agent-chat-maximize-tooltip"
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
                            <span id="agent-chat-maximize-tooltip" class="blx-tooltip agent-chat-head__tooltip" role="tooltip">
                                <span class="blx-tooltip__eyebrow">
                                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                                    "Layout"
                                </span>
                                <span class="blx-tooltip__main">
                                    {move || {
                                        if chat_maximized.get() {
                                            i18n.tr(I18nKey::AgChatRestore)().to_string()
                                        } else {
                                            i18n.tr(I18nKey::AgChatMaximize)().to_string()
                                        }
                                    }}
                                </span>
                                <span class="blx-tooltip__hint">"Resize the Agent workspace"</span>
                            </span>
                        </span>
                        <span class="blx-tip-anchor blx-tip-anchor--left agent-chat-head__tip">
                            <button
                                type="button"
                                class="agent-chat-head__reset"
                                prop:disabled=move || busy.get() || !is_tauri_shell()
                                aria-describedby="agent-chat-reset-tooltip"
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
                                            status_line.set(Some(
                                                i18n
                                                    .tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)()
                                                    .to_string(),
                                            ));
                                            return;
                                        };
                                        let session_id = wb.active_agent_chat_session_id_for_workspace(ws_id);
                                        match agent_clear_conversation(Some(session_id.clone())).await {
                                            Ok(()) => {
                                                timeline.set(TimelineDoc::default());
                                                thinking_open.set(HashMap::new());
                                                tool_detail_open.set(HashMap::new());
                                                draft.set(String::new());
                                                wb.set_workspace_agent_session_timeline(ws_id, &session_id, TimelineDoc::default());
                                                wb.set_workspace_agent_compose_draft(ws_id, String::new());
                                                wb.clear_chat_usage_for_session(ws_id, &session_id);
                                                wb.reset_workspace_agent_chat_mode(ws_id);
                                                chat_mode.set(AgentChatMode::AskEdits);
                                                status_line.set(None);
                                            }
                                            Err(msg) => status_line.set(Some(msg)),
                                        }
                                    });
                                }
                            >
                                <LxIcon icon=icondata::LuEraser width="0.86rem" height="0.86rem" />
                            </button>
                            <span id="agent-chat-reset-tooltip" class="blx-tooltip agent-chat-head__tooltip" role="tooltip">
                                <span class="blx-tooltip__eyebrow">
                                    <span class="blx-tooltip__spark" aria-hidden="true"></span>
                                    "History"
                                </span>
                                <span class="blx-tooltip__main">{move || i18n.tr(I18nKey::AgResetChat)()}</span>
                                <span class="blx-tooltip__hint">{move || i18n.tr(I18nKey::AgResetChatAria)()}</span>
                            </span>
                        </span>
                    </div>
                </div>
                <div class="agent-chat-session-tabs" role="tablist" aria-label=move || i18n.tr(I18nKey::AgSessionTabsAria)()>
                    {move || {
                        let Some(ws_id) = wb.active_id().get() else {
                            return Vec::<AnyView>::new();
                        };
                        let sessions = wb.agent_chat_sessions_for_workspace(ws_id);
                        let active_id = wb.active_agent_chat_session_id_for_workspace(ws_id);
                        let total = sessions.len();
                        sessions
                            .into_iter()
                            .map(|session| {
                                let session_id = session.id.clone();
                                let session_id_for_select = session_id.clone();
                                let session_id_for_close = StoredValue::new(session_id.clone());
                                let is_active = session.id == active_id;
                                let can_close = total > 1
                                    && !matches!(
                                        session.status,
                                        AgentChatSessionStatus::Running
                                            | AgentChatSessionStatus::NeedsInput
                                    );
                                let status_label = agent_session_status_label(i18n, session.status);
                                let status_class = agent_session_status_class(session.status);
                                view! {
                                    <div
                                        role="tab"
                                        tabindex="0"
                                        class=move || {
                                            if is_active {
                                                "agent-chat-session-tab agent-chat-session-tab--active".to_string()
                                            } else {
                                                "agent-chat-session-tab".to_string()
                                            }
                                        }
                                        aria-selected=if is_active { "true" } else { "false" }
                                        on:click=move |_| {
                                            if wb.select_agent_chat_session(ws_id, &session_id_for_select) {
                                                active_session_id.set(session_id_for_select.clone());
                                                timeline.set(wb.agent_timeline_for_workspace_untracked(ws_id));
                                                thinking_open.set(HashMap::new());
                                                tool_detail_open.set(HashMap::new());
                                                draft.set(wb.agent_compose_draft_for_workspace_untracked(ws_id));
                                                image_mode.set(wb.agent_image_mode_for_workspace_untracked(ws_id));
                                                chat_mode.set(wb.agent_chat_mode_for_workspace_untracked(ws_id));
                                                enhance_prompt.set(wb.agent_enhance_prompt_for_workspace_untracked(ws_id));
                                                busy.set(matches!(
                                                    wb.agent_chat_sessions_for_workspace(ws_id)
                                                        .into_iter()
                                                        .find(|s| s.id == session_id_for_select)
                                                        .map(|s| s.status),
                                                    Some(AgentChatSessionStatus::Running | AgentChatSessionStatus::NeedsInput)
                                                ));
                                                status_line.set(None);
                                            }
                                        }
                                    >
                                        <span class=format!("agent-chat-session-tab__status {status_class}") title=status_label.clone() aria-label=status_label></span>
                                        <span class="agent-chat-session-tab__title">{session.title.clone()}</span>
                                        <Show when=move || { session.unread_count > 0 }>
                                            <span class="agent-chat-session-tab__unread" aria-label=move || i18n.tr(I18nKey::AgSessionUnread)()>
                                                {session.unread_count}
                                            </span>
                                        </Show>
                                        <Show when=move || can_close>
                                            <button
                                                type="button"
                                                class="agent-chat-session-tab__close"
                                                aria-label=move || i18n.tr(I18nKey::AgCloseSession)()
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    let sid = session_id_for_close.get_value();
                                                    let _ = wb.close_agent_chat_session(ws_id, &sid);
                                                    active_session_id.set(wb.active_agent_chat_session_id_for_workspace(ws_id));
                                                    timeline.set(wb.agent_timeline_for_workspace_untracked(ws_id));
                                                    draft.set(wb.agent_compose_draft_for_workspace_untracked(ws_id));
                                                }
                                            >
                                                <LxIcon icon=icondata::LuX width="0.68rem" height="0.68rem" />
                                            </button>
                                        </Show>
                                    </div>
                                }
                                .into_any()
                            })
                            .collect::<Vec<_>>()
                    }}
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
                                        wb, i18n, draft, chat_mode, enhance_prompt, busy, status_line,
                                        timeline, task_snapshot, thinking_open, tool_detail_open, voice_handle, true,
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
                                                    chat_mode=chat_mode
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

            <Composer
                draft=draft
                chat_mode=chat_mode
                enhance_prompt=enhance_prompt
                busy=busy
                model_label=model_label
                input_ref=compose_input_ref
                wb=wb
                i18n=i18n
                on_submit=Callback::new(move |()| {
                    submit_turn(wb, i18n, draft, chat_mode, enhance_prompt, busy, status_line, timeline, task_snapshot, thinking_open, tool_detail_open, voice_handle, true);
                })
                on_cancel=Callback::new(move |()| {
                    crate::app_log::info("agent", "abort_requested", serde_json::json!({}));
                    leptos::task::spawn_local(async move {
                        let session_id = wb
                            .active_id()
                            .get_untracked()
                            .map(|ws_id| wb.active_agent_chat_session_id_for_workspace(ws_id));
                        if let Err(error) = agent_abort(session_id).await {
                            crate::app_log::error(
                                "agent",
                                "abort_failed",
                                serde_json::json!({ "error": error }),
                            );
                        }
                    });
                })
            />
        </section>
    }
}

#[component]
fn AgentThinkingStream(timeline: RwSignal<TimelineDoc>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let stream_ref = NodeRef::<html::Div>::new();
    let thinking_text = Memo::new(move |_| timeline.with(latest_active_thinking_text));
    let speed_samples = Memo::new(move |_| timeline.with(turn_speed_samples));
    let idle_idx = RwSignal::new(0usize);
    let idle_alive: SendWrapper<Rc<Cell<bool>>> = SendWrapper::new(Rc::new(Cell::new(true)));
    let idle_alive_loop = idle_alive.clone();
    leptos::task::spawn_local(async move {
        while idle_alive_loop.get() {
            TimeoutFuture::new(random_thinking_idle_delay_ms()).await;
            if !idle_alive_loop.get() {
                break;
            }
            idle_idx.set(random_thinking_idle_index(idle_idx.get_untracked()));
        }
    });
    on_cleanup(move || {
        idle_alive.set(false);
    });

    Effect::new(move |_| {
        let _ = thinking_text.get();
        if let Some(node) = stream_ref.get() {
            node.set_scroll_top(node.scroll_height());
        }
    });

    view! {
        <aside
            class=move || {
                if thinking_text.with(Option::is_some) {
                    "agent-thinking-stream"
                } else {
                    "agent-thinking-stream agent-thinking-stream--idle"
                }
            }
            aria-live="polite"
        >
            <div class="agent-thinking-stream__head">
                <span class="agent-thinking-stream__pulse" aria-hidden="true"></span>
                <span>{move || {
                    if thinking_text.with(Option::is_some) {
                        i18n.tr(I18nKey::AgComposerThinking)().to_string()
                    } else {
                        i18n.tr(I18nKey::CommonIdle)().to_string()
                    }
                }}</span>
            </div>
            <div class="agent-thinking-stream__body" node_ref=stream_ref>
                {move || {
                    if let Some(text) = thinking_text.get() {
                        if text.trim().is_empty() {
                            let phrase = THINKING_IDLE_MESSAGES[idle_idx.get()].to_string();
                            view! {
                                <span class="agent-thinking-stream__idle">
                                    <span class="agent-thinking-stream__idle-icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuSparkles width="0.72rem" height="0.72rem" />
                                    </span>
                                    <span>{phrase}</span>
                                </span>
                            }
                            .into_any()
                        } else {
                            view! { <>{text}</> }.into_any()
                        }
                    } else {
                        let samples = speed_samples.get();
                        view! {
                            <div class="agent-thinking-stream__speed-card">
                                <div class="agent-thinking-stream__speed-chart" aria-label="Model speed by turn">
                                    {render_speed_bars(samples.clone())}
                                </div>
                                <div class="agent-thinking-stream__speed-meta">
                                    <span>"tok/s"</span>
                                    <strong>{speed_summary(&samples)}</strong>
                                </div>
                            </div>
                        }
                        .into_any()
                    }
                }}
            </div>
        </aside>
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TurnSpeedSample {
    tok_per_sec: f64,
}

fn turn_speed_samples(doc: &TimelineDoc) -> Vec<TurnSpeedSample> {
    let mut samples = doc
        .turns
        .iter()
        .filter_map(|turn| {
            let mut metrics = TurnMetrics::default();
            merge_provider_metrics(&turn.parts, &mut metrics);
            speed_from_metrics(metrics)
        })
        .collect::<Vec<_>>();
    let keep_from = samples.len().saturating_sub(12);
    samples.drain(0..keep_from);
    samples
}

fn merge_provider_metrics(parts: &[TurnPart], out: &mut TurnMetrics) {
    for part in parts {
        match part {
            TurnPart::Text { metrics, .. } | TurnPart::ModelRound { metrics, .. } => {
                out.merge(metrics);
            }
            TurnPart::Subagent { metrics, parts, .. } => {
                if metrics.is_empty() {
                    merge_provider_metrics(parts, out);
                } else {
                    out.merge(metrics);
                }
            }
            TurnPart::Tool { children, .. } => merge_provider_metrics(children, out),
            _ => {}
        }
    }
}

fn speed_from_metrics(metrics: TurnMetrics) -> Option<TurnSpeedSample> {
    let output = metrics.output_tokens?;
    if output == 0 || metrics.elapsed_ms == 0 {
        return None;
    }
    Some(TurnSpeedSample {
        tok_per_sec: output as f64 / (metrics.elapsed_ms as f64 / 1_000.0),
    })
}

fn render_speed_bars(samples: Vec<TurnSpeedSample>) -> impl IntoView {
    if samples.is_empty() {
        return view! {
            <span class="agent-thinking-stream__speed-empty">"No speed data"</span>
        }
        .into_any();
    }
    let max = samples
        .iter()
        .map(|sample| sample.tok_per_sec)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    samples
        .into_iter()
        .map(|sample| {
            let height = ((sample.tok_per_sec / max) * 100.0).clamp(12.0, 100.0);
            let label = format!("{:.1} tok/s", sample.tok_per_sec);
            view! {
                <span
                    class="agent-thinking-stream__speed-bar"
                    title=label.clone()
                    aria-label=label
                    style=format!("--speed-height: {height:.1}%")
                ></span>
            }
        })
        .collect_view()
        .into_any()
}

fn speed_summary(samples: &[TurnSpeedSample]) -> String {
    match samples.last() {
        Some(sample) => format!("{:.1}", sample.tok_per_sec),
        None => "-".to_string(),
    }
}

fn smooth_scroll_chat_to_bottom(log: web_sys::HtmlDivElement) {
    let start = log.scroll_top();
    let target = (log.scroll_height() - log.client_height()).max(0);
    leptos::task::spawn_local(async move {
        const STEPS: i32 = 14;
        for step in 1..=STEPS {
            let t = step as f64 / STEPS as f64;
            let eased = 1.0 - (1.0 - t).powi(3);
            let next = start as f64 + (target - start) as f64 * eased;
            log.set_scroll_top(next.round() as i32);
            TimeoutFuture::new(16).await;
        }
        log.set_scroll_top(log.scroll_height());
    });
}

fn random_thinking_idle_delay_ms() -> u32 {
    5_000 + (js_sys::Math::random() * 5_000.0).floor() as u32
}

fn random_thinking_idle_index(current: usize) -> usize {
    let len = THINKING_IDLE_MESSAGES.len();
    let mut next = (js_sys::Math::random() * len as f64).floor() as usize;
    if next == current {
        next = (next + 1) % len;
    }
    next
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

fn resolve_workspace_scope(wb: &WorkbenchService, ws_id: u64) -> Option<WorkspaceScope> {
    wb.workspaces().with_untracked(|list| {
        let workspace = list.iter().find(|workspace| workspace.id == ws_id)?;
        let root = workspace.cwd.trim().to_string().into_non_empty();
        Some(WorkspaceScope {
            root,
            connection_id: workspace.remote_connection_id.clone(),
            worktree: workspace
                .worktree
                .as_ref()
                .map(|meta| WorkspaceWorktreeMeta {
                    base_cwd: meta.base_cwd.clone(),
                    worktree_cwd: meta.worktree_cwd.clone(),
                    branch: meta.branch.clone(),
                    head: meta.head.clone(),
                    git_common_dir: meta.git_common_dir.clone(),
                    main_worktree_cwd: meta.main_worktree_cwd.clone(),
                    created_by_blxcode: meta.created_by_blxcode,
                }),
        })
    })
}

trait IntoNonEmptyString {
    fn into_non_empty(self) -> Option<String>;
}

impl IntoNonEmptyString for String {
    fn into_non_empty(self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn submit_turn(
    wb: WorkbenchService,
    i18n: I18nService,
    draft: RwSignal<String>,
    chat_mode: RwSignal<AgentChatMode>,
    enhance_prompt: RwSignal<bool>,
    busy: RwSignal<bool>,
    status_line: RwSignal<Option<String>>,
    timeline: RwSignal<TimelineDoc>,
    task_snapshot: RwSignal<TaskSnapshot>,
    thinking_open: RwSignal<HashMap<usize, bool>>,
    tool_detail_open: RwSignal<HashMap<String, bool>>,
    voice_handle: VoiceOrbHandle,
    allow_enhance: bool,
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
    let prompt_chars = prompt.chars().count();

    let Some(ws_id) = wb.active_id().get_untracked() else {
        status_line.set(Some(
            i18n.tr(I18nKey::AgentImageSelectAWorkspaceTabFirst)().to_string(),
        ));
        return;
    };
    let session_id = wb.active_agent_chat_session_id_for_workspace(ws_id);

    if is_reset_command(&prompt) {
        draft.set(String::new());
        wb.set_workspace_agent_compose_draft(ws_id, String::new());
        status_line.set(None);
        leptos::task::spawn_local(async move {
            match agent_clear_conversation(Some(session_id.clone())).await {
                Ok(()) => {
                    timeline.set(TimelineDoc::default());
                    thinking_open.set(HashMap::new());
                    tool_detail_open.set(HashMap::new());
                    wb.set_workspace_agent_session_timeline(
                        ws_id,
                        &session_id,
                        TimelineDoc::default(),
                    );
                    wb.clear_chat_usage_for_session(ws_id, &session_id);
                    wb.reset_workspace_agent_chat_mode(ws_id);
                    chat_mode.set(AgentChatMode::AskEdits);
                    status_line.set(None);
                }
                Err(msg) => status_line.set(Some(msg)),
            }
        });
        return;
    }

    if allow_enhance && wb.agent_enhance_prompt_for_workspace_untracked(ws_id) {
        busy.set(true);
        status_line.set(Some("Enhancing prompt…".into()));
        let original_prompt = prompt.clone();
        leptos::task::spawn_local(async move {
            match agent_enhance_prompt(original_prompt.clone()).await {
                Ok(enhanced) => {
                    let enhanced_prompt = enhanced.prompt.trim().to_string();
                    if enhanced_prompt.is_empty() {
                        busy.set(false);
                        status_line.set(Some(
                            i18n.tr(I18nKey::AgentPanelPromptEnhancementReturnedAnEmptyPrompt)()
                                .to_string(),
                        ));
                        draft.set(original_prompt.clone());
                        wb.set_workspace_agent_compose_draft(ws_id, original_prompt);
                        return;
                    }
                    draft.set(enhanced_prompt.clone());
                    wb.set_workspace_agent_compose_draft(ws_id, enhanced_prompt);
                    busy.set(false);
                    status_line.set(None);
                    submit_turn(
                        wb,
                        i18n,
                        draft,
                        chat_mode,
                        enhance_prompt,
                        busy,
                        status_line,
                        timeline,
                        task_snapshot,
                        thinking_open,
                        tool_detail_open,
                        voice_handle,
                        false,
                    );
                }
                Err(msg) => {
                    busy.set(false);
                    status_line.set(Some(format!("Prompt enhancement failed: {msg}")));
                    draft.set(original_prompt.clone());
                    wb.set_workspace_agent_compose_draft(ws_id, original_prompt);
                }
            }
        });
        return;
    }

    let workspace_root = resolve_effective_workspace_root(&wb);
    let workspace_scope = resolve_workspace_scope(&wb, ws_id);
    let context_items = wb.agent_context_for_workspace_untracked(ws_id);
    let transient_context_ids = transient_agent_context_ids(&context_items);
    let image_context_items = wb.pending_agent_images_for_workspace_untracked(ws_id);
    let workspace_root_present = workspace_root.is_some();
    let context_count = context_items.len();
    let image_context_count = image_context_items.len();

    let starts_new_chat_session = timeline.with_untracked(|doc| doc.turns.is_empty());
    let session_turn_started_at = js_sys::Date::now();
    timeline.update(|doc| {
        doc.push_user_turn_with_pending(prompt.clone(), Some(session_turn_started_at))
    });
    wb.set_workspace_agent_session_timeline(ws_id, &session_id, timeline.get_untracked());
    if starts_new_chat_session {
        wb.ensure_chat_session_started_for_session(ws_id, &session_id, session_turn_started_at);
        let fallback_title = fallback_chat_session_title(&prompt);
        if !fallback_title.is_empty() {
            wb.set_agent_chat_session_title_if_auto(
                ws_id,
                &session_id,
                fallback_title.clone(),
                None,
            );
            let wb_title = wb;
            let session_id_title = session_id.clone();
            let prompt_for_title = prompt.clone();
            leptos::task::spawn_local(async move {
                if let Ok(generated) = agent_generate_chat_title(prompt_for_title).await {
                    wb_title.set_agent_chat_session_title_if_auto(
                        ws_id,
                        &session_id_title,
                        generated.title,
                        Some(&fallback_title),
                    );
                }
            });
        }
    }

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
    let session_role = wb.agent_session_role_for_workspace_untracked(ws_id);
    let chat_mode_value = chat_mode.get_untracked();
    let turn = UserTurn {
        prompt,
        workspace_root,
        workspace_scope,
        chat_mode: chat_mode_value,
        session_role,
        voice_input,
        image_generate,
        context_items,
        image_context_items,
    };
    crate::app_log::info(
        "agent",
        "turn_submitted",
        serde_json::json!({
            "chatMode": format!("{chat_mode_value:?}"),
            "promptChars": prompt_chars,
            "workspaceRootPresent": workspace_root_present,
            "contextItems": context_count,
            "imageContextItems": image_context_count,
            "voiceInput": voice_input,
            "imageGenerate": image_generate,
        }),
    );

    let busy_sig = busy;
    let status_sig = status_line;
    let timeline_sig = timeline;
    let session_timeline = RwSignal::new(timeline.get_untracked());
    let task_snapshot_sig = task_snapshot;
    let ws_capture = ws_id;
    let session_capture = session_id.clone();
    let audio_ref = voice_handle.audio_ref;
    let turn_had_error = RwSignal::new(false);
    // Whether this turn ran a file-mutating tool — gates the turn-end
    // "Changed files" summary so it never shows for read-only turns.
    let turn_touched_files = RwSignal::new(false);
    // Workspace cwd for the turn-end `git_status_changes` lookup.
    let changed_files_cwd = resolve_effective_workspace_root(&wb);

    leptos::task::spawn_local(async move {
        wb.set_agent_chat_session_status(
            ws_capture,
            &session_capture,
            AgentChatSessionStatus::Running,
        );
        if let Err(msg) = agent_submit_turn(Some(session_id.clone()), turn).await {
            crate::app_log::error(
                "agent",
                "turn_submit_failed",
                serde_json::json!({ "error": msg.clone() }),
            );
            busy_sig.set(false);
            wb.set_agent_chat_session_status(
                ws_capture,
                &session_capture,
                AgentChatSessionStatus::Error,
            );
            status_sig.set(Some(msg));
            return;
        }

        let i18n_d = i18n;
        let wb_d = wb;
        let wb_after_drain = wb;
        let session_for_drain = session_capture.clone();
        if let Err(msg) = agent_drain_turn_opts(
            Some(session_id.clone()),
            voice_input,
            move |batch: Vec<EventEnvelope>| {
                let loc_now = i18n_d.locale().get_untracked();
                let notification_target = serde_json::json!({
                    "view": "agent",
                    "workspaceId": ws_capture,
                    "sessionId": session_for_drain.clone(),
                });
                for env in &batch {
                    let ev = &env.event;
                    if let AgentEvent::Error { message } = ev {
                        turn_had_error.set(true);
                        wb_d.set_agent_chat_session_status(
                            ws_capture,
                            &session_for_drain,
                            AgentChatSessionStatus::Error,
                        );
                        spawn_agent_notification_fallback(
                            wb_d,
                            "error",
                            i18n_d.tr(I18nKey::AgentPanelAgentError)(),
                            Some(message.clone()),
                            "agent:error",
                            Some(notification_target.clone()),
                        );
                    }
                    if matches!(ev, AgentEvent::Done)
                        && wb_d.active_agent_chat_session_id_for_workspace(ws_capture)
                            != session_for_drain
                    {
                        spawn_agent_notification_fallback(
                            wb_d,
                            "cli_agent_response",
                            i18n_d.tr(I18nKey::AgAssistant)(),
                            None,
                            "agent:done",
                            Some(notification_target.clone()),
                        );
                    }
                    if let AgentEvent::ToolCall { tool, .. } = ev {
                        if is_file_mutating_tool(tool) {
                            turn_touched_files.set(true);
                        }
                    }
                    if let AgentEvent::ToolCall { tool, args, .. } = ev {
                        if tool == "harness.ask_user" {
                            wb_d.set_agent_chat_session_status(
                                ws_capture,
                                &session_for_drain,
                                AgentChatSessionStatus::NeedsInput,
                            );
                            let question = args
                                .as_ref()
                                .and_then(|v| v.get("question"))
                                .and_then(|v| v.as_str())
                                .unwrap_or_else(|| {
                                    i18n_d.tr(I18nKey::AgentPanelTheAgentNeedsYourInput)()
                                })
                                .to_string();
                            spawn_agent_notification_fallback(
                                wb_d,
                                "question",
                                i18n_d.tr(I18nKey::AgentPanelAgentNeedsInput)(),
                                Some(question),
                                "agent:question",
                                Some(notification_target.clone()),
                            );
                        }
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
                        session_timeline,
                        task_snapshot_sig,
                        loc_now,
                        Some((wb_d, ws_capture, Some(session_for_drain.clone()))),
                    );
                    wb_d.set_workspace_agent_session_timeline(
                        ws_capture,
                        &session_for_drain,
                        session_timeline.get_untracked(),
                    );
                    if wb_d.active_agent_chat_session_id_for_workspace(ws_capture)
                        == session_for_drain
                    {
                        timeline_sig.set(session_timeline.get_untracked());
                    }
                    maybe_handle_client_tool(ev, wb_d);
                }
            },
        )
        .await
        {
            crate::app_log::error(
                "agent",
                "turn_drain_failed",
                serde_json::json!({ "error": msg.clone() }),
            );
            status_sig.set(Some(msg));
        } else if !turn_had_error.get_untracked() && !transient_context_ids.is_empty() {
            wb_after_drain.remove_workspace_agent_context_items(ws_capture, &transient_context_ids);
        }
        crate::app_log::info(
            "agent",
            "turn_finished",
            serde_json::json!({
                "hadError": turn_had_error.get_untracked(),
                "touchedFiles": turn_touched_files.get_untracked(),
            }),
        );
        busy_sig.set(false);
        if !turn_had_error.get_untracked() {
            wb_after_drain.set_agent_chat_session_status(
                ws_capture,
                &session_capture,
                AgentChatSessionStatus::Idle,
            );
        }

        // Turn-end "Changed files" summary: only after a turn that ran a
        // file-mutating tool, in a Git repo. Snapshots the working tree once
        // (no live polling) and attaches it to the just-finished turn.
        if turn_touched_files.get_untracked() {
            if let Some(cwd) = changed_files_cwd.clone() {
                maybe_attach_changed_files(timeline_sig, wb_after_drain, ws_capture, cwd).await;
            }
        }
    });
}

/// Tools that can change files on disk in the workspace. Used to gate the
/// turn-end changed-files summary.
fn is_file_mutating_tool(tool: &str) -> bool {
    matches!(
        tool,
        "workspace_file_write"
            | "workspace_file_delete"
            | "workspace_dir_create"
            | "workspace_entry_rename"
            | "git_apply_patch"
            | "git_add"
            | "git_commit"
            | "shell_exec"
    )
}

/// Fetch the working-tree changes via `git_status_changes` and attach a
/// [`TurnPart::ChangedFiles`] summary to the most recent turn. No-op outside
/// the Tauri shell, in a non-repo, or when nothing changed.
async fn maybe_attach_changed_files(
    timeline: RwSignal<TimelineDoc>,
    wb: WorkbenchService,
    workspace_id: u64,
    cwd: String,
) {
    if !is_tauri_shell() {
        return;
    }
    if !git_is_repository(cwd.clone(), None).await.unwrap_or(false) {
        return;
    }
    let Ok(changes) = git_status_changes(cwd, None).await else {
        return;
    };
    let entries: Vec<ChangedFileEntry> = changes
        .into_iter()
        .map(|c| {
            let added = c.staged_stats.as_ref().map(|s| s.added).unwrap_or(0)
                + c.unstaged_stats.as_ref().map(|s| s.added).unwrap_or(0);
            let removed = c.staged_stats.as_ref().map(|s| s.removed).unwrap_or(0)
                + c.unstaged_stats.as_ref().map(|s| s.removed).unwrap_or(0);
            ChangedFileEntry {
                rel_path: c.rel_path,
                status: c.status,
                added,
                removed,
            }
        })
        .collect();
    timeline.update(|doc| doc.set_last_turn_changed_files(entries));
    wb.set_workspace_agent_timeline(workspace_id, timeline.get_untracked());
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
