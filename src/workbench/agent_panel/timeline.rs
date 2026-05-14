use crate::agent_wire::{AgentEvent, TaskSnapshot};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[derive(Clone, Debug, PartialEq)]
pub enum TimelineItem {
    User { text: String },
    Assistant { text: String },
    Tool(ToolActivity),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DisplayTimelineItem {
    User { text: String },
    Assistant { text: String },
    ToolGroup(Vec<ToolActivity>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ActivityStatus {
    Pending,
    Ok,
    Fail,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolActivity {
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

pub fn apply_agent_event(
    ev: &AgentEvent,
    timeline: RwSignal<Vec<TimelineItem>>,
    task_snapshot: RwSignal<TaskSnapshot>,
    loc: Locale,
) {
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
        AgentEvent::TaskSnapshot { snapshot } => {
            task_snapshot.set(snapshot.clone());
        }
        AgentEvent::Done => timeline.update(|rows| {
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
        }),
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
        }
    }

    flush_tools(&mut out, &mut pending_tools);
    out
}

fn friendly_label(tool: &str) -> &str {
    match tool {
        "harness.create_workspace" => "Create workspace",
        "read_workspace_file" => "Read file",
        "memory_list" => "List memory notes",
        "memory_read" => "Read memory note",
        "memory_search" => "Search memory",
        "memory_create" => "Create memory note",
        "memory_write" => "Update memory note",
        "task_list" => "List tasks",
        "task_get" => "Read task",
        "task_create" => "Create task",
        "task_update" => "Update task",
        "task_delete" => "Delete task",
        "task_reorder" => "Reorder tasks",
        "harness.open_terminal" => "Open terminal",
        "harness.list_terminals" => "List terminals",
        "harness.send_terminal_keys" => "Send keys to terminal",
        "harness.read_terminal_output" => "Read terminal output",
        other => other,
    }
}

fn summarize_args(tool: &str, args: Option<&serde_json::Value>) -> String {
    let Some(args) = args else {
        return String::new();
    };
    let pick = match tool {
        "harness.create_workspace" => Some("title"),
        "read_workspace_file" | "memory_read" | "memory_create" | "memory_write" => Some("path"),
        "memory_search" => Some("query"),
        "task_get" | "task_update" | "task_delete" => Some("id"),
        "task_create" => Some("title"),
        "harness.open_terminal" => Some("agentSlug"),
        "harness.send_terminal_keys" => Some("text"),
        _ => None,
    };
    if let Some(key) = pick {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            return v.to_owned();
        }
    }
    if tool == "task_reorder" {
        if let Some(ids) = args.get("orderedIds").and_then(|v| v.as_array()) {
            return format!("{} ids", ids.len());
        }
    }
    String::new()
}

fn tool_icon(tool: &str) -> icondata::Icon {
    match tool {
        "harness.create_workspace" => icondata::LuLayoutGrid,
        "read_workspace_file" => icondata::LuFileText,
        "memory_list" => icondata::LuList,
        "memory_read" => icondata::LuBookOpen,
        "memory_search" => icondata::LuSearch,
        "memory_create" => icondata::LuFilePlus,
        "memory_write" => icondata::LuFileEdit,
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
pub fn TimelineRow(idx: usize, entry: DisplayTimelineItem, i18n: I18nService) -> impl IntoView {
    let line_no = format!("{:02}", idx + 1);
    match entry {
        DisplayTimelineItem::User { text } => view! {
            <li class="agent-chat-line agent-chat-line--user">
                <span class="agent-chat-index">{line_no.clone()}</span>
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgYou)()}</strong>
                    <pre class="workbench-agent-transcript">{text}</pre>
                </div>
            </li>
        }
        .into_any(),
        DisplayTimelineItem::Assistant { text } => view! {
            <li class="agent-chat-line agent-chat-line--agent">
                <span class="agent-chat-index">{line_no.clone()}</span>
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgAssistant)()}</strong>
                    <pre class="workbench-agent-transcript">{text}</pre>
                </div>
            </li>
        }
        .into_any(),
        DisplayTimelineItem::ToolGroup(entries) => view! {
            <ToolActivityGroupRow line_no=line_no entries=entries />
        }
        .into_any(),
    }
}

#[component]
fn ToolActivityGroupRow(line_no: String, entries: Vec<ToolActivity>) -> impl IntoView {
    view! {
        <li class="agent-chat-line agent-chat-line--tool">
            <span class="agent-chat-index">{line_no}</span>
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
