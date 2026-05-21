use crate::agent_wire::{AgentEvent, TaskSnapshot};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, voice_settings_get};
use crate::workbench::agent_panel::voice_orb::{
    play_line_tts, tts_line_playback_available, VoiceOrbHandle,
};
pub use crate::workbench::agent_timeline::TimelineItem;
use crate::workbench::agent_timeline::{
    subagent_role_label, subagent_status_label, ActivityStatus, SubagentCard, SubagentGroup,
    SubagentStepRow, ToolActivity,
};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum DisplayTimelineItem {
    User { text: String },
    Assistant { text: String },
    ToolGroup(Vec<ToolActivity>),
    SubagentGroup(SubagentGroup),
    Thinking { text: String, done: bool },
    GeneratedImage {
        prompt: String,
        mime: String,
        preview_src: String,
        saved_path: Option<String>,
        filename: Option<String>,
    },
}

#[inline]
fn persist_agent_timeline(
    persist: Option<(WorkbenchService, u64)>,
    timeline: RwSignal<Vec<TimelineItem>>,
) {
    if let Some((wb, workspace_id)) = persist {
        // Drop large base64 previews when a `saved_path` is available — the
        // image lives on disk and is rehydrated via `generated_image_preview`
        // on next render. Keeps `sessions.json` small.
        let sanitized = timeline
            .get_untracked()
            .into_iter()
            .map(|item| match item {
                TimelineItem::GeneratedImage {
                    prompt,
                    mime,
                    preview_src,
                    saved_path,
                    filename,
                } => {
                    let drop_preview = saved_path
                        .as_deref()
                        .is_some_and(|p| !p.trim().is_empty());
                    TimelineItem::GeneratedImage {
                        prompt,
                        mime,
                        preview_src: if drop_preview { String::new() } else { preview_src },
                        saved_path,
                        filename,
                    }
                }
                other => other,
            })
            .collect::<Vec<_>>();
        wb.set_workspace_agent_timeline(workspace_id, sanitized);
    }
}

fn find_subagent_card_mut<'a>(
    rows: &'a mut [TimelineItem],
    agent_id: &str,
) -> Option<&'a mut SubagentCard> {
    rows.iter_mut().rev().find_map(|entry| match entry {
        TimelineItem::SubagentGroup(group) => group
            .agents
            .iter_mut()
            .find(|c| c.agent_id == agent_id),
        _ => None,
    })
}

pub fn apply_agent_event(
    ev: &AgentEvent,
    timeline: RwSignal<Vec<TimelineItem>>,
    task_snapshot: RwSignal<TaskSnapshot>,
    loc: Locale,
    persist: Option<(WorkbenchService, u64)>,
) {
    match ev {
        AgentEvent::AssistantDelta { delta } => {
            timeline.update(|rows| match rows.last_mut() {
                Some(TimelineItem::Assistant { text }) => text.push_str(delta),
                _ => rows.push(TimelineItem::Assistant {
                    text: delta.clone(),
                }),
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::ThinkingDelta { delta } => {
            timeline.update(|rows| {
                let append = matches!(
                    rows.last(),
                    Some(TimelineItem::Thinking { done: false, .. })
                );
                if append {
                    if let Some(TimelineItem::Thinking { text, .. }) = rows.last_mut() {
                        text.push_str(delta);
                    }
                } else {
                    rows.push(TimelineItem::Thinking {
                        text: delta.clone(),
                        done: false,
                    });
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::ThinkingDone => {
            timeline.update(|rows| {
                for row in rows.iter_mut().rev() {
                    if let TimelineItem::Thinking { done, .. } = row {
                        *done = true;
                        break;
                    }
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::ToolCall { tool, args, .. } => {
            let entry = ToolActivity::from_call(tool, args.as_ref(), loc);
            timeline.update(|rows| rows.push(TimelineItem::Tool(entry)));
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::ToolResult { tool, ok, message } => {
            timeline.update(|rows| {
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
                    let stub = ToolActivity::from_call(tool, None, loc);
                    rows.push(TimelineItem::Tool(ToolActivity {
                        tool: tool.clone(),
                        label: stub.label,
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
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::SubagentStarted {
            agent_id,
            role,
            display_name,
        } => {
            timeline.update(|rows| {
                let card = SubagentCard {
                    agent_id: agent_id.clone(),
                    role: role.clone(),
                    display_name: display_name.clone(),
                    status: "running".into(),
                    summary: String::new(),
                    steps: Vec::new(),
                    tools: Vec::new(),
                    live_text: String::new(),
                    live_thinking: String::new(),
                    thinking_done: false,
                };
                if let Some(TimelineItem::SubagentGroup(group)) = rows.last_mut() {
                    group.agents.push(card);
                } else {
                    rows.push(TimelineItem::SubagentGroup(SubagentGroup {
                        agents: vec![card],
                    }));
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::SubagentStep {
            agent_id,
            step_id,
            title,
            status,
            note,
        } => {
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    if let Some(step) = card
                        .steps
                        .iter_mut()
                        .find(|s| s.id == *step_id)
                    {
                        step.title = title.clone();
                        step.status = status.clone();
                        step.note = note.clone();
                    } else {
                        card.steps.push(SubagentStepRow {
                            id: step_id.clone(),
                            title: title.clone(),
                            status: status.clone(),
                            note: note.clone(),
                        });
                    }
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::SubagentToolCall { agent_id, tool, args, .. } => {
            let entry = ToolActivity::from_call(tool, args.as_ref(), loc);
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    card.tools.push(entry);
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::TurnUsage {
            input_tokens,
            output_tokens,
            ttft_ms,
            elapsed_ms,
        } => {
            if let Some((wb, workspace_id)) = persist.clone() {
                wb.record_chat_turn_usage(
                    workspace_id,
                    *input_tokens,
                    *output_tokens,
                    *ttft_ms,
                    *elapsed_ms,
                );
            }
        }
        AgentEvent::SubagentAssistantDelta { agent_id, delta } => {
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    card.live_text.push_str(delta);
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::SubagentThinkingDelta { agent_id, delta } => {
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    card.live_thinking.push_str(delta);
                    card.thinking_done = false;
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::SubagentThinkingDone { agent_id } => {
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    card.thinking_done = true;
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::SubagentFinished {
            agent_id,
            status,
            summary,
        } => {
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    card.status = status.clone();
                    card.summary = summary.clone();
                    // Drop live buffers once the subagent finishes — `summary`
                    // is now the authoritative output. Keeping the buffers
                    // would double-render the same text in the card.
                    card.live_text.clear();
                    card.live_thinking.clear();
                    card.thinking_done = true;
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::TaskSnapshot { snapshot } => {
            task_snapshot.set(snapshot.clone());
        }
        AgentEvent::Done => {
            timeline.update(|rows| {
                if let Some(message) = synthesize_completion_message(rows) {
                    rows.push(TimelineItem::Assistant {
                        text: format!("{message}\n"),
                    });
                    return;
                }
                let fallback = match rows.last() {
                    Some(TimelineItem::Tool(tool)) if tool.status == ActivityStatus::Fail => tool
                        .detail
                        .as_deref()
                        .filter(|detail| !detail.is_empty())
                        .map(|detail| format!("`{}` failed: {detail}", tool.label))
                        .or_else(|| Some(format!("`{}` failed.", tool.label))),
                    _ => None,
                };
                if let Some(message) = fallback {
                    rows.push(TimelineItem::Assistant {
                        text: format!("{message}\n"),
                    });
                }
            });
            persist_agent_timeline(persist, timeline);
        }
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
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::VoiceReady { .. } => {
            // Voice playback is handled in the agent panel; no timeline mutation.
        }
        AgentEvent::ImageContextConsumed { .. } => {
            // Image context status is handled in the agent panel; no timeline mutation.
        }
        AgentEvent::ImageGenerated {
            prompt,
            mime,
            saved_path,
            filename,
            preview_src,
        } => {
            timeline.update(|rows| {
                rows.push(TimelineItem::GeneratedImage {
                    prompt: prompt.clone(),
                    mime: mime.clone(),
                    preview_src: preview_src.clone(),
                    saved_path: saved_path.clone(),
                    filename: filename.clone(),
                });
            });
            persist_agent_timeline(persist, timeline);
        }
    }
}

fn synthesize_completion_message(rows: &[TimelineItem]) -> Option<String> {
    let last_user_idx = rows
        .iter()
        .rposition(|entry| matches!(entry, TimelineItem::User { .. }))?;

    let has_assistant_after_user = rows[last_user_idx + 1..]
        .iter()
        .any(|entry| matches!(entry, TimelineItem::Assistant { text } if !text.trim().is_empty()));
    if has_assistant_after_user {
        return None;
    }

    // Image-mode turns end with a `GeneratedImage` row instead of assistant
    // text — skip the synthetic completion blurb so the chat stays clean.
    let has_image_after_user = rows[last_user_idx + 1..]
        .iter()
        .any(|entry| matches!(entry, TimelineItem::GeneratedImage { .. }));
    if has_image_after_user {
        return None;
    }

    let tool_rows: Vec<&ToolActivity> = rows[last_user_idx + 1..]
        .iter()
        .filter_map(|entry| match entry {
            TimelineItem::Tool(tool) => Some(tool),
            _ => None,
        })
        .collect();
    if tool_rows.is_empty() {
        return Some("The model completed the turn without emitting visible text.".to_string());
    }

    let failed = tool_rows
        .iter()
        .filter(|tool| matches!(tool.status, ActivityStatus::Fail))
        .count();
    let succeeded = tool_rows
        .iter()
        .filter(|tool| matches!(tool.status, ActivityStatus::Ok))
        .count();

    let mut message = format!(
        "The model finished after {count} tool call(s) but did not emit a visible reply.",
        count = tool_rows.len()
    );
    if succeeded > 0 || failed > 0 {
        message.push_str(&format!(" {succeeded} succeeded, {failed} failed.",));
    }
    message.push_str(" Expand the tool group above for the raw results.");
    Some(message)
}

pub fn compact_timeline(items: Vec<TimelineItem>) -> Vec<DisplayTimelineItem> {
    let mut out = Vec::new();
    let mut pending_tools = Vec::new();

    let flush_tools = |out: &mut Vec<DisplayTimelineItem>,
                       pending_tools: &mut Vec<ToolActivity>| {
        if !pending_tools.is_empty() {
            out.push(DisplayTimelineItem::ToolGroup(std::mem::take(
                pending_tools,
            )));
        }
    };

    for item in items {
        match item {
            TimelineItem::User { text } => {
                flush_tools(&mut out, &mut pending_tools);
                out.push(DisplayTimelineItem::User { text });
            }
            TimelineItem::Assistant { text } => {
                flush_tools(&mut out, &mut pending_tools);
                out.push(DisplayTimelineItem::Assistant { text });
            }
            TimelineItem::Tool(tool) => pending_tools.push(tool),
            TimelineItem::Thinking { text, done } => {
                flush_tools(&mut out, &mut pending_tools);
                out.push(DisplayTimelineItem::Thinking { text, done });
            }
            TimelineItem::GeneratedImage {
                prompt,
                mime,
                preview_src,
                saved_path,
                filename,
            } => {
                flush_tools(&mut out, &mut pending_tools);
                out.push(DisplayTimelineItem::GeneratedImage {
                    prompt,
                    mime,
                    preview_src,
                    saved_path,
                    filename,
                });
            }
            TimelineItem::SubagentGroup(group) => {
                flush_tools(&mut out, &mut pending_tools);
                out.push(DisplayTimelineItem::SubagentGroup(group));
            }
        }
    }

    flush_tools(&mut out, &mut pending_tools);
    out
}

fn tool_icon(tool: &str) -> icondata::Icon {
    match tool {
        "harness.create_workspace" => icondata::LuLayoutGrid,
        "list_workspace_files" => icondata::LuFolderTree,
        "read_workspace_file" => icondata::LuFileText,
        "memory_list" => icondata::LuList,
        "memory_read" => icondata::LuBookOpen,
        "memory_search" => icondata::LuSearch,
        "memory_create" => icondata::LuFilePlus,
        "memory_write" => icondata::LuFilePenLine,
        "memory_delete" => icondata::LuTrash2,
        "memory_rename" => icondata::LuFilePenLine,
        "memory_graph" => icondata::LuShare2,
        "memory_backlinks" => icondata::LuLink,
        "memory_category_list" => icondata::LuPalette,
        "memory_category_update" => icondata::LuPalette,
        "memory_context_list" => icondata::LuList,
        "memory_context_attach" => icondata::LuPaperclip,
        "memory_context_detach" => icondata::LuUnlink,
        "list_tools" => icondata::LuWrench,
        "task_list" => icondata::LuListTodo,
        "task_get" => icondata::LuClipboardList,
        "task_create" => icondata::LuCirclePlus,
        "task_update" => icondata::LuListChecks,
        "task_delete" => icondata::LuTrash2,
        "task_reorder" => icondata::LuArrowUpDown,
        "harness.open_terminal" => icondata::LuTerminal,
        "harness.list_terminals" => icondata::LuLayoutGrid,
        "harness.send_terminal_keys" => icondata::LuSendHorizontal,
        "harness.send_agent_context" => icondata::LuShare2,
        "harness.read_terminal_output" => icondata::LuWrapText,
        _ => icondata::LuWrench,
    }
}

#[component]
pub fn ChatLineIndexColumn(
    line_no: String,
    tts_text: Option<String>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    let play_text = StoredValue::new(tts_text.clone().unwrap_or_default());
    let show_play = move || {
        is_tauri_shell()
            && play_text.with_value(|t| !t.trim().is_empty())
            && tts_line_playback_available(voice_handle.settings.get().as_ref())
    };
    let voice_handle = voice_handle;
    view! {
        <div class="agent-chat-index-col">
            <span class="agent-chat-index">{line_no}</span>
            <Show when=show_play>
                <button
                    type="button"
                    class="agent-chat-tts-btn"
                    title="Play"
                    aria-label="Play message audio"
                    on:click=move |_| {
                        let text = play_text.get_value();
                        if text.trim().is_empty() {
                            return;
                        }
                        let audio_ref = voice_handle.audio_ref;
                        if let Some(settings) = voice_handle.settings.get_untracked() {
                            play_line_tts(audio_ref, settings, text.clone());
                            return;
                        }
                        leptos::task::spawn_local(async move {
                            if let Ok(settings) = voice_settings_get().await {
                                voice_handle.settings.set(Some(settings.clone()));
                                play_line_tts(audio_ref, settings, text);
                            }
                        });
                    }
                >
                    <LxIcon icon=icondata::LuPlay width="0.62rem" height="0.62rem" />
                </button>
            </Show>
        </div>
    }
}

#[component]
pub fn TimelineRow(
    idx: usize,
    entry: DisplayTimelineItem,
    i18n: I18nService,
    thinking_open: RwSignal<HashMap<usize, bool>>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    let line_no = format!("{:02}", idx + 1);
    match entry {
        DisplayTimelineItem::User { text } => view! {
            <li class="agent-chat-line agent-chat-line--user">
                <ChatLineIndexColumn line_no=line_no.clone() tts_text=None voice_handle=voice_handle />
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgYou)()}</strong>
                    <pre class="workbench-agent-transcript">{text}</pre>
                </div>
            </li>
        }
        .into_any(),
        DisplayTimelineItem::Assistant { text } => {
            let tts_text = text.clone();
            view! {
            <li class="agent-chat-line agent-chat-line--agent">
                <ChatLineIndexColumn line_no=line_no.clone() tts_text=Some(tts_text) voice_handle=voice_handle />
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgAssistant)()}</strong>
                    <div class="workbench-agent-markdown" inner_html=render_markdown_to_html(&text)></div>
                </div>
            </li>
        }
            .into_any()
        }
        DisplayTimelineItem::ToolGroup(entries) => view! {
            <ToolActivityGroupRow line_no=line_no entries=entries voice_handle=voice_handle />
        }
        .into_any(),
        DisplayTimelineItem::SubagentGroup(group) => {
            let agents = group.agents.clone();
            let loc = i18n.locale().get_untracked();
            view! {
                <li class="agent-chat-line agent-chat-line--subagents">
                    <ChatLineIndexColumn line_no=line_no.clone() tts_text=None voice_handle=voice_handle />
                    <div class="agent-chat-body agent-subagent-group">
                        <strong>{lookup(loc, I18nKey::AgSubagentGroupTitle)}</strong>
                        {agents.into_iter().map(|card| {
                            let status_raw = card.status.clone();
                            let status = subagent_status_label(loc, &status_raw);
                            let role_hint = subagent_role_label(loc, &card.role);
                            let name = if card.display_name.is_empty() {
                                role_hint
                            } else {
                                card.display_name.clone()
                            };
                            let summary = card.summary.clone();
                            let live_text = card.live_text.clone();
                            let live_thinking = card.live_thinking.clone();
                            let thinking_done = card.thinking_done;
                            let is_running = status_raw == "running";
                            view! {
                                <details class="agent-subagent-card" open>
                                    <summary class="agent-subagent-card__summary">
                                        <span class="agent-subagent-card__name">{name}</span>
                                        <span class="agent-subagent-card__status">{status}</span>
                                        <Show when=move || is_running>
                                            <span class="agent-subagent-card__pulse" aria-hidden="true">
                                                <span></span><span></span><span></span>
                                            </span>
                                        </Show>
                                    </summary>
                                    {(!live_thinking.is_empty()).then(|| {
                                        let class = if thinking_done {
                                            "agent-subagent-card__thinking agent-subagent-card__thinking--done"
                                        } else {
                                            "agent-subagent-card__thinking"
                                        };
                                        view! {
                                            <details class=class>
                                                <summary>"Thinking"{(!thinking_done).then(|| "…")}</summary>
                                                <pre class="agent-subagent-card__thinking-body">{live_thinking}</pre>
                                            </details>
                                        }
                                    })}
                                    {(!live_text.is_empty()).then(|| view! {
                                        <pre class="agent-subagent-card__live-text">{live_text}</pre>
                                    })}
                                    {(!summary.is_empty()).then(|| view! {
                                        <p class="agent-subagent-card__summary-text">{summary}</p>
                                    })}
                                    <ul class="agent-subagent-card__tools">
                                        {merge_consecutive_tools(card.tools).into_iter().map(|run| {
                                            let label = run.first()
                                                .map(|t| t.label.clone())
                                                .unwrap_or_default();
                                            let count = run.len();
                                            let multiple = count > 1;
                                            view! {
                                                <li>
                                                    <span>{label}</span>
                                                    <Show when=move || multiple>
                                                        <span class="agent-subagent-card__tools-count">
                                                            {format!("\u{00d7}{count}")}
                                                        </span>
                                                    </Show>
                                                </li>
                                            }
                                        }).collect_view()}
                                    </ul>
                                </details>
                            }
                        }).collect_view()}
                    </div>
                </li>
            }
            .into_any()
        }
        DisplayTimelineItem::Thinking { text, done } => view! {
            <ThinkingRow idx=idx line_no=line_no text=text done=done thinking_open=thinking_open voice_handle=voice_handle />
        }
        .into_any(),
        DisplayTimelineItem::GeneratedImage {
            prompt,
            mime,
            preview_src,
            saved_path,
            filename,
        } => view! {
            <GeneratedImageRow
                line_no=line_no
                prompt=prompt
                mime=mime
                preview_src=preview_src
                saved_path=saved_path
                filename=filename
                voice_handle=voice_handle
            />
        }
        .into_any(),
    }
}

#[component]
fn GeneratedImageRow(
    line_no: String,
    prompt: String,
    mime: String,
    preview_src: String,
    saved_path: Option<String>,
    filename: Option<String>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    // Lazily re-hydrate the preview from disk when restored timelines only
    // carry a `saved_path` (we never persist the data URL in the snapshot).
    let preview = RwSignal::new(preview_src.clone());
    let path_for_effect = saved_path.clone();
    Effect::new(move |_| {
        let needs_hydrate = preview.with(|v| v.is_empty());
        if !needs_hydrate {
            return;
        }
        let Some(path) = path_for_effect.clone() else {
            return;
        };
        if !is_tauri_shell() || path.is_empty() {
            return;
        }
        leptos::task::spawn_local(async move {
            if let Ok(resp) =
                crate::tauri_bridge::generated_image_preview(path).await
            {
                preview.set(format!("data:{};base64,{}", resp.mime, resp.bytes_b64));
            }
        });
    });

    let download_name = filename
        .clone()
        .unwrap_or_else(|| format!("generated.{}", ext_for_mime(&mime)));
    let saved_path_attr = saved_path.clone().unwrap_or_default();
    let saved_path_has = !saved_path_attr.is_empty();
    let alt_text = prompt.clone();
    let prompt_for_label = prompt.clone();

    view! {
        <li class="agent-chat-line agent-chat-line--image">
            <ChatLineIndexColumn line_no=line_no tts_text=None voice_handle=voice_handle />
            <div class="agent-chat-body">
                <strong>{move || i18n.tr(I18nKey::ImageModeBadge)()}</strong>
                <p class="agent-chat-image-prompt">
                    <span class="agent-chat-image-prompt__label">
                        {move || i18n.tr(I18nKey::ImageGenerateUserPromptPrefix)()}":"
                    </span>
                    <span>{prompt_for_label.clone()}</span>
                </p>
                <Show
                    when=move || !preview.with(|s| s.is_empty())
                    fallback=move || view! {
                        <p class="agent-chat-image-missing">"…"</p>
                    }
                >
                    <img
                        class="agent-chat-image"
                        src=move || preview.get()
                        alt=alt_text.clone()
                    />
                </Show>
                <div class="agent-chat-image-actions">
                    <a
                        class="workbench-mini-btn"
                        prop:href=move || preview.get()
                        prop:download=download_name.clone()
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuDownload width="0.82rem" height="0.82rem" />
                            <span>{move || i18n.tr(I18nKey::ImageGenerateDownload)()}</span>
                        </span>
                    </a>
                    <Show when=move || saved_path_has>
                        <small class="harness-muted agent-chat-image-path">
                            {saved_path_attr.clone()}
                        </small>
                    </Show>
                </div>
            </div>
        </li>
    }
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "bin",
    }
}

#[component]
fn ThinkingRow(
    idx: usize,
    line_no: String,
    text: String,
    done: bool,
    thinking_open: RwSignal<HashMap<usize, bool>>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    let open = Memo::new(move |_| thinking_open.with(|m| m.get(&idx).copied().unwrap_or(false)));
    let has_content = !text.trim().is_empty();
    let label = if done { "Thinking" } else { "Thinking…" };
    let body = text.clone();
    view! {
        <li class="agent-chat-line agent-chat-line--thinking">
            <ChatLineIndexColumn line_no=line_no tts_text=None voice_handle=voice_handle />
            <div class="agent-chat-body">
                <button
                    type="button"
                    class="agent-thinking-title"
                    class:agent-thinking-title--active=move || !done
                    aria-expanded=move || open.get().to_string()
                    prop:disabled=move || !has_content
                    on:click=move |_| {
                        if has_content {
                            thinking_open.update(|m| {
                                let cur = m.get(&idx).copied().unwrap_or(false);
                                m.insert(idx, !cur);
                            });
                        }
                    }
                >
                    <Show when=move || !done>
                        <span class="agent-thinking__dots" aria-hidden="true">
                            <span></span><span></span><span></span>
                        </span>
                    </Show>
                    <strong class="agent-thinking-title__label">{label}</strong>
                    <Show when=move || has_content>
                        <span class="agent-thinking-title__chevron" aria-hidden="true">
                            {move || if open.get() {
                                view! { <LxIcon icon=icondata::LuChevronUp width="0.85rem" height="0.85rem" /> }
                            } else {
                                view! { <LxIcon icon=icondata::LuChevronDown width="0.85rem" height="0.85rem" /> }
                            }}
                        </span>
                    </Show>
                </button>
                <Show when=move || open.get() && has_content>
                    <pre class="agent-thinking-card__body">{body.clone()}</pre>
                </Show>
            </div>
        </li>
    }
}

/// Collapse consecutive `ToolActivity` entries with the same tool name into a
/// single visual row carrying a `×N` badge. Each merged run keeps the original
/// invocations so the expanded view can show per-call args + details.
fn merge_consecutive_tools(entries: Vec<ToolActivity>) -> Vec<Vec<ToolActivity>> {
    let mut groups: Vec<Vec<ToolActivity>> = Vec::new();
    for entry in entries {
        match groups.last_mut() {
            Some(group) if group.last().map(|t| t.tool.as_str()) == Some(entry.tool.as_str()) => {
                group.push(entry);
            }
            _ => groups.push(vec![entry]),
        }
    }
    groups
}

fn aggregate_status(entries: &[ToolActivity]) -> ActivityStatus {
    if entries.iter().any(|e| e.status == ActivityStatus::Pending) {
        ActivityStatus::Pending
    } else if entries.iter().any(|e| e.status == ActivityStatus::Fail) {
        ActivityStatus::Fail
    } else {
        ActivityStatus::Ok
    }
}

#[component]
fn ToolActivityGroupRow(
    line_no: String,
    entries: Vec<ToolActivity>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    let runs = merge_consecutive_tools(entries);
    view! {
        <li class="agent-chat-line agent-chat-line--tool">
            <ChatLineIndexColumn line_no=line_no tts_text=None voice_handle=voice_handle />
            <div class="agent-chat-body">
                <strong>"Tool"</strong>
                <div class="agent-tool-group">
                    {runs
                        .into_iter()
                        .enumerate()
                        .map(|(idx, run)| view! { <ToolActivityRow idx=idx run=run /> })
                        .collect_view()}
                </div>
            </div>
        </li>
    }
}

#[component]
fn ToolActivityRow(idx: usize, run: Vec<ToolActivity>) -> impl IntoView {
    let _ = idx;
    let count = run.len();
    let head = run.first().cloned().expect("merge_consecutive_tools yields non-empty runs");
    let agg = aggregate_status(&run);
    let status_class = match agg {
        ActivityStatus::Pending => "agent-tool-row--pending",
        ActivityStatus::Ok => "agent-tool-row--ok",
        ActivityStatus::Fail => "agent-tool-row--fail",
    };
    let status_icon = match agg {
        ActivityStatus::Pending => icondata::LuLoader,
        ActivityStatus::Ok => icondata::LuCheck,
        ActivityStatus::Fail => icondata::LuTriangleAlert,
    };

    // Single invocation: keep the previous compact layout (no badge, no list).
    if count <= 1 {
        let detail_open = RwSignal::new(false);
        let has_detail = head.detail.as_ref().is_some_and(|s| !s.is_empty());
        let detail_text = head.detail.clone().unwrap_or_default();
        let label = head.label.clone();
        let summary = head.args_summary.clone();
        let tool_name_for_title = head.tool.clone();
        return view! {
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
                        <LxIcon icon=tool_icon(&head.tool) width="0.82rem" height="0.82rem" />
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
        }
        .into_any();
    }

    // Merged run: counter badge + expandable per-invocation list.
    let any_detail = run
        .iter()
        .any(|e| e.detail.as_ref().is_some_and(|s| !s.is_empty()));
    let any_arg = run.iter().any(|e| !e.args_summary.is_empty());
    let expandable = any_detail || any_arg;
    let open = RwSignal::new(false);
    let label = head.label.clone();
    let tool_for_icon = head.tool.clone();
    let tool_name_for_title = head.tool.clone();
    let count_badge = format!("×{count}");
    let invocations = run.clone();

    view! {
        <div class=format!("agent-tool-row agent-tool-row--merged {status_class}") title=tool_name_for_title>
            <button
                type="button"
                class="agent-tool-row__head"
                aria-expanded=move || open.get().to_string()
                prop:disabled=move || !expandable
                on:click=move |_| {
                    if expandable {
                        open.update(|o| *o = !*o);
                    }
                }
            >
                <span class="agent-tool-row__icon" aria-hidden="true">
                    <LxIcon icon=tool_icon(&tool_for_icon) width="0.82rem" height="0.82rem" />
                </span>
                <span class="agent-tool-row__label">{label}</span>
                <span class="agent-tool-row__count" aria-label=format!("{count} calls")>{count_badge}</span>
                <Show when=move || expandable>
                    <span class="agent-tool-row__chevron" aria-hidden="true">
                        {move || if open.get() {
                            view! { <LxIcon icon=icondata::LuChevronUp width="0.72rem" height="0.72rem" /> }
                        } else {
                            view! { <LxIcon icon=icondata::LuChevronDown width="0.72rem" height="0.72rem" /> }
                        }}
                    </span>
                </Show>
                <span class="agent-tool-row__status" aria-hidden="true">
                    <LxIcon icon=status_icon width="0.78rem" height="0.78rem" />
                </span>
            </button>
            <Show when=move || expandable && open.get()>
                <ul class="agent-tool-row__sublist">
                    {invocations
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(i, entry)| {
                            let sub_status = match entry.status {
                                ActivityStatus::Pending => "agent-tool-row__sub--pending",
                                ActivityStatus::Ok => "agent-tool-row__sub--ok",
                                ActivityStatus::Fail => "agent-tool-row__sub--fail",
                            };
                            let sub_icon = match entry.status {
                                ActivityStatus::Pending => icondata::LuLoader,
                                ActivityStatus::Ok => icondata::LuCheck,
                                ActivityStatus::Fail => icondata::LuTriangleAlert,
                            };
                            let arg = entry.args_summary.clone();
                            let detail = entry.detail.clone().unwrap_or_default();
                            let has_detail_i = !detail.is_empty();
                            view! {
                                <li class=format!("agent-tool-row__sub {sub_status}")>
                                    <span class="agent-tool-row__sub-index">{format!("{:02}", i + 1)}</span>
                                    <span class="agent-tool-row__sub-status" aria-hidden="true">
                                        <LxIcon icon=sub_icon width="0.7rem" height="0.7rem" />
                                    </span>
                                    <Show when={
                                        let a = arg.clone();
                                        move || !a.is_empty()
                                    }>
                                        <span class="agent-tool-row__sub-arg">{arg.clone()}</span>
                                    </Show>
                                    <Show when=move || has_detail_i>
                                        <pre class="agent-tool-row__sub-detail">{detail.clone()}</pre>
                                    </Show>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </Show>
        </div>
    }
    .into_any()
}
