use crate::agent_wire::{AgentEvent, TaskSnapshot};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, voice_settings_get};
use crate::workbench::agent_panel::voice_orb::{
    play_line_tts, tts_line_playback_available, VoiceOrbHandle,
};
pub use crate::workbench::agent_timeline::TimelineItem;
use crate::workbench::agent_timeline::{ActivityStatus, ToolActivity};
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
    Thinking { text: String, done: bool },
}

#[inline]
fn persist_agent_timeline(
    persist: Option<(WorkbenchService, u64)>,
    timeline: RwSignal<Vec<TimelineItem>>,
) {
    if let Some((wb, workspace_id)) = persist {
        wb.set_workspace_agent_timeline(workspace_id, timeline.get_untracked());
    }
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
            let entry = ToolActivity::from_call(tool, args.as_ref());
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
                    let stub = ToolActivity::from_call(tool, None);
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
        "memory_write" => icondata::LuFileEdit,
        "memory_delete" => icondata::LuTrash2,
        "memory_rename" => icondata::LuFileEdit,
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
        "task_create" => icondata::LuPlusCircle,
        "task_update" => icondata::LuListChecks,
        "task_delete" => icondata::LuTrash2,
        "task_reorder" => icondata::LuArrowUpDown,
        "harness.open_terminal" => icondata::LuTerminal,
        "harness.list_terminals" => icondata::LuLayoutGrid,
        "harness.send_terminal_keys" => icondata::LuSendHorizonal,
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
        DisplayTimelineItem::Thinking { text, done } => view! {
            <ThinkingRow idx=idx line_no=line_no text=text done=done thinking_open=thinking_open voice_handle=voice_handle />
        }
        .into_any(),
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

#[component]
fn ToolActivityGroupRow(
    line_no: String,
    entries: Vec<ToolActivity>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    view! {
        <li class="agent-chat-line agent-chat-line--tool">
            <ChatLineIndexColumn line_no=line_no tts_text=None voice_handle=voice_handle />
            <div class="agent-chat-body">
                <strong>"Tool"</strong>
                <div class="agent-tool-group">
                    {entries
                        .into_iter()
                        .enumerate()
                        .map(|(tool_idx, entry)| view! { <ToolActivityRow idx=tool_idx entry=entry /> })
                        .collect_view()}
                </div>
            </div>
        </li>
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
    }
}
