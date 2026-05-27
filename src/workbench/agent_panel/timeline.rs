use crate::agent_wire::{AgentEvent, TaskSnapshot, TurnMetrics, TurnUsageKind};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, voice_settings_get};
use crate::workbench::agent_panel::ask_user_card::AskUserCard;
use crate::workbench::agent_panel::turn_metrics_bar::{BarContext, TurnMetricsBar};
use crate::workbench::agent_panel::voice_orb::{
    play_line_tts, tts_line_playback_available, VoiceOrbHandle,
};
pub use crate::workbench::agent_timeline::TimelineItem;
use crate::workbench::agent_timeline::{
    subagent_role_label, subagent_status_label, ActivityStatus, AskUserOption, AskUserState,
    SubagentCard, SubagentGroup, SubagentStepRow, ToolActivity,
};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::WorkbenchService;
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum DisplayTimelineItem {
    User {
        text: String,
    },
    Assistant {
        text: String,
        /// Latest user-message text preceding this assistant block — used as
        /// the "Redo" button's payload. `None` for the welcome/system bubble
        /// and for any assistant block that has no preceding user turn.
        prev_user: Option<String>,
        metrics: TurnMetrics,
    },
    Tool(ToolActivity),
    /// A tool-only model round: LLM inference metrics + the tool calls the
    /// model issued in that round, grouped into one timeline block.
    ModelRound {
        metrics: TurnMetrics,
        tools: Vec<ToolActivity>,
    },
    SubagentGroup(SubagentGroup),
    Thinking {
        text: String,
        done: bool,
    },
    GeneratedImage {
        prompt: String,
        mime: String,
        preview_src: String,
        saved_path: Option<String>,
        filename: Option<String>,
    },
    AskUser {
        call_id: String,
        question: String,
        header: Option<String>,
        options: Vec<AskUserOption>,
        multi_select: bool,
        allow_other: bool,
        state: AskUserState,
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
            // Open ask-user bubbles cannot be resumed after a reload — the
            // backend agent loop awaiting the response is already dead. Drop
            // them from the persisted snapshot so they don't linger as
            // permanently disabled cards.
            .filter(|item| {
                !matches!(
                    item,
                    TimelineItem::AskUser {
                        state: AskUserState::Open,
                        ..
                    }
                )
            })
            .map(|item| match item {
                TimelineItem::GeneratedImage {
                    prompt,
                    mime,
                    preview_src,
                    saved_path,
                    filename,
                } => {
                    let drop_preview = saved_path.as_deref().is_some_and(|p| !p.trim().is_empty());
                    TimelineItem::GeneratedImage {
                        prompt,
                        mime,
                        preview_src: if drop_preview {
                            String::new()
                        } else {
                            preview_src
                        },
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

pub(super) struct ParsedAskUserArgs {
    pub question: String,
    pub header: Option<String>,
    pub options: Vec<AskUserOption>,
    pub multi_select: bool,
    pub allow_other: bool,
}

pub(super) fn parse_ask_user_args(value: &serde_json::Value) -> Option<ParsedAskUserArgs> {
    let obj = value.as_object()?;
    let question = obj.get("question")?.as_str()?.trim().to_string();
    if question.is_empty() {
        return None;
    }
    let header = obj
        .get("header")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let options_raw = obj.get("options")?.as_array()?;
    if options_raw.len() < 2 || options_raw.len() > 4 {
        return None;
    }
    let mut options = Vec::with_capacity(options_raw.len());
    for raw in options_raw {
        let o = raw.as_object()?;
        let label = o.get("label")?.as_str()?.trim().to_string();
        if label.is_empty() {
            return None;
        }
        let description = o
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        options.push(AskUserOption { label, description });
    }
    let multi_select = obj
        .get("multiSelect")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let allow_other = obj
        .get("allowOther")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    Some(ParsedAskUserArgs {
        question,
        header,
        options,
        multi_select,
        allow_other,
    })
}

fn find_subagent_card_mut<'a>(
    rows: &'a mut [TimelineItem],
    agent_id: &str,
) -> Option<&'a mut SubagentCard> {
    rows.iter_mut().rev().find_map(|entry| match entry {
        TimelineItem::SubagentGroup(group) => {
            group.agents.iter_mut().find(|c| c.agent_id == agent_id)
        }
        _ => None,
    })
}

/// Attach `ModelRound` metrics from the main agent to the right timeline
/// row. Walks backwards from the tail:
///
/// - If the last row is `Assistant` → merge metrics there (the model
///   produced visible text this round).
/// - Else if the tail is a contiguous run of `Tool` rows (a tool-only
///   round) → insert a synthetic `ModelDecision { metrics }` just before
///   the first tool of that run.
/// - Else (nothing visible at all) → append a `ModelDecision` row at the
///   end so the metric still surfaces.
fn attach_main_model_round(rows: &mut Vec<TimelineItem>, metrics: TurnMetrics) {
    if metrics.is_empty() {
        return;
    }
    // Last row is the assistant block this round produced — merge there.
    if let Some(TimelineItem::Assistant { metrics: m, .. }) = rows.last_mut() {
        m.merge(&metrics);
        return;
    }
    // Tool-only round: the backend pushes TurnUsage(ModelRound) *before*
    // dispatching tools, so ToolCall events for this round always arrive
    // *after* this event. Appending here lets those pushes land naturally
    // after the ModelDecision row — no walk-back needed.
    rows.push(TimelineItem::ModelDecision { metrics });
}

/// Attach `ToolExec` metrics for a main-agent tool. Walks backwards for
/// the matching `call_id` (the typical match is the most recent row).
fn attach_main_tool_exec(rows: &mut [TimelineItem], call_id: &str, metrics: TurnMetrics) {
    if metrics.is_empty() {
        return;
    }
    for row in rows.iter_mut().rev() {
        if let TimelineItem::Tool(tool) = row {
            if tool.call_id.as_deref() == Some(call_id) {
                tool.metrics.merge(&metrics);
                return;
            }
        }
    }
}

/// Attach `ToolExec` metrics for a subagent tool, scoped to the matching
/// `(agent_id, call_id)`.
fn attach_subagent_tool_exec(
    rows: &mut [TimelineItem],
    agent_id: &str,
    call_id: &str,
    metrics: TurnMetrics,
) {
    if metrics.is_empty() {
        return;
    }
    if let Some(card) = find_subagent_card_mut(rows, agent_id) {
        for tool in card.tools.iter_mut().rev() {
            if tool.call_id.as_deref() == Some(call_id) {
                tool.metrics.merge(&metrics);
                return;
            }
        }
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
                Some(TimelineItem::Assistant { text, .. }) => text.push_str(delta),
                _ => rows.push(TimelineItem::Assistant {
                    text: delta.clone(),
                    metrics: TurnMetrics::default(),
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
        AgentEvent::ToolCall {
            tool,
            args,
            call_id,
        } => {
            if tool == "harness.ask_user" {
                if let Some((call_id, ask)) = call_id
                    .clone()
                    .zip(args.as_ref().and_then(parse_ask_user_args))
                {
                    timeline.update(|rows| {
                        rows.push(TimelineItem::AskUser {
                            call_id,
                            question: ask.question,
                            header: ask.header,
                            options: ask.options,
                            multi_select: ask.multi_select,
                            allow_other: ask.allow_other,
                            state: AskUserState::Open,
                        });
                    });
                    persist_agent_timeline(persist, timeline);
                    return;
                }
                // Malformed payload — fall through to normal Tool row so the
                // user at least sees something landed. The client_tools.rs
                // dispatcher will short-circuit the result with ok=false.
            }
            let entry = ToolActivity::from_call_with_id(tool, args.as_ref(), loc, call_id.clone());
            timeline.update(|rows| rows.push(TimelineItem::Tool(entry)));
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::ToolResult { tool, ok, message } => {
            if tool == "harness.ask_user" {
                // The AskUser bubble already reflects the user's action; do
                // not append a phantom Tool row for the result event.
                persist_agent_timeline(persist, timeline);
                return;
            }
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
                        call_id: None,
                        metrics: TurnMetrics::default(),
                        paths: Vec::new(),
                        merged_count: 1,
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
                    metrics: TurnMetrics::default(),
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
                    if let Some(step) = card.steps.iter_mut().find(|s| s.id == *step_id) {
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
        AgentEvent::SubagentToolCall {
            agent_id,
            tool,
            args,
            ..
        } => {
            let entry = ToolActivity::from_call(tool, args.as_ref(), loc);
            timeline.update(|rows| {
                if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                    card.tools.push(entry);
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::TurnUsage {
            kind,
            agent_id,
            call_id,
            round_index: _,
            turn_generation,
            input_tokens,
            output_tokens,
            ttft_ms,
            elapsed_ms,
            cost_usd,
        } => {
            // 1) Update the workspace-level session aggregate first. If
            //    the event is stale (lower generation than the workspace's
            //    high-water-mark) `record_chat_turn_usage` drops it and we
            //    also skip the per-row routing below.
            let applied = match persist.as_ref() {
                Some((wb, workspace_id)) => wb.record_chat_turn_usage(
                    *workspace_id,
                    *turn_generation,
                    *input_tokens,
                    *output_tokens,
                    *elapsed_ms,
                    *cost_usd,
                ),
                None => true,
            };
            if !applied {
                return;
            }

            let metrics = TurnMetrics {
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                ttft_ms: *ttft_ms,
                elapsed_ms: *elapsed_ms,
                cost_usd: *cost_usd,
            };

            // 2) Per-row routing — the 4 cases from the plan.
            timeline.update(
                |rows| match (*kind, agent_id.as_deref(), call_id.as_deref()) {
                    (TurnUsageKind::ToolExec, None, Some(call_id)) => {
                        attach_main_tool_exec(rows, call_id, metrics);
                    }
                    (TurnUsageKind::ToolExec, Some(agent_id), Some(call_id)) => {
                        attach_subagent_tool_exec(rows, agent_id, call_id, metrics);
                    }
                    (TurnUsageKind::ModelRound, None, _) => {
                        attach_main_model_round(rows, metrics);
                    }
                    (TurnUsageKind::ModelRound, Some(agent_id), _) => {
                        if let Some(card) = find_subagent_card_mut(rows, agent_id) {
                            card.metrics.merge(&metrics);
                        }
                    }
                    // ToolExec without a call_id is malformed — nothing to
                    // route to, the session aggregate above still counted it.
                    (TurnUsageKind::ToolExec, _, None) => {}
                },
            );
            persist_agent_timeline(persist, timeline);
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
                        metrics: TurnMetrics::default(),
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
                        metrics: TurnMetrics::default(),
                    });
                }
            });
            persist_agent_timeline(persist, timeline);
        }
        AgentEvent::Error { message } => {
            let prefix = lookup(loc, I18nKey::AgErrColon);
            let line = format!("{prefix} {message}");
            timeline.update(|rows| match rows.last_mut() {
                Some(TimelineItem::Assistant { text, .. }) => {
                    if !text.is_empty() && !text.ends_with('\n') {
                        text.push('\n');
                    }
                    text.push_str(&line);
                    text.push('\n');
                }
                _ => rows.push(TimelineItem::Assistant {
                    text: format!("{line}\n"),
                    metrics: TurnMetrics::default(),
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

    let has_assistant_after_user = rows[last_user_idx + 1..].iter().any(
        |entry| matches!(entry, TimelineItem::Assistant { text, .. } if !text.trim().is_empty()),
    );
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

/// Merge consecutive `ModelRound` display items when they each contain only a
/// single (already-grouped) tool of the same type. This collapses e.g. three
/// separate `rules_read` rounds into one entry the user can expand to see all
/// files read, instead of forcing them to scroll past N nearly-identical rows.
fn merge_consecutive_model_rounds(items: Vec<DisplayTimelineItem>) -> Vec<DisplayTimelineItem> {
    let mut out: Vec<DisplayTimelineItem> = Vec::new();
    for item in items {
        if let DisplayTimelineItem::ModelRound { metrics, mut tools } = item {
            // Only collapse single-tool rounds with the same tool name as the
            // previous round.
            if tools.len() == 1 {
                let incoming_tool = tools.remove(0);
                if let Some(DisplayTimelineItem::ModelRound {
                    metrics: prev_metrics,
                    tools: prev_tools,
                }) = out.last_mut()
                {
                    if prev_tools.len() == 1 && prev_tools[0].tool == incoming_tool.tool {
                        let prev = &mut prev_tools[0];
                        prev.paths.extend(incoming_tool.paths);
                        prev.metrics.merge(&incoming_tool.metrics);
                        prev.merged_count += incoming_tool.merged_count;
                        if incoming_tool.status == ActivityStatus::Fail {
                            prev.status = ActivityStatus::Fail;
                        } else if prev.status == ActivityStatus::Ok
                            && incoming_tool.status == ActivityStatus::Pending
                        {
                            prev.status = ActivityStatus::Pending;
                        }
                        prev_metrics.merge(&metrics);
                        continue;
                    }
                }
                // Not merged — restore the tool and push as new entry.
                tools.push(incoming_tool);
            }
            out.push(DisplayTimelineItem::ModelRound { metrics, tools });
        } else {
            out.push(item);
        }
    }
    out
}

/// Merge consecutive tool rows with the same tool name into a single entry.
/// Paths are accumulated; metrics are summed; status is the worst seen.
fn group_consecutive_tools(tools: Vec<ToolActivity>) -> Vec<ToolActivity> {
    let mut out: Vec<ToolActivity> = Vec::new();
    for t in tools {
        if let Some(last) = out.last_mut() {
            if last.tool == t.tool {
                last.paths.extend(t.paths);
                last.metrics.merge(&t.metrics);
                last.merged_count += t.merged_count;
                if t.status == ActivityStatus::Fail {
                    last.status = ActivityStatus::Fail;
                } else if last.status == ActivityStatus::Ok && t.status == ActivityStatus::Pending {
                    last.status = ActivityStatus::Pending;
                }
                continue;
            }
        }
        out.push(t);
    }
    out
}

/// One rendered chat-log row (index in [`compact_timeline`] output).
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineDisplayRow {
    pub idx: usize,
    pub entry: DisplayTimelineItem,
}

pub fn timeline_display_rows(items: Vec<TimelineItem>) -> Vec<TimelineDisplayRow> {
    compact_timeline(items)
        .into_iter()
        .enumerate()
        .map(|(idx, entry)| TimelineDisplayRow { idx, entry })
        .collect()
}

/// Stable key for tool-detail expand state across streaming rerenders.
pub fn tool_detail_key(
    line_idx: usize,
    tool: &str,
    call_id: Option<&str>,
    sub_idx: Option<usize>,
) -> String {
    if let Some(si) = sub_idx {
        format!("{line_idx}-s{si}-{tool}")
    } else {
        format!("{line_idx}-{tool}-{}", call_id.unwrap_or(""))
    }
}

pub fn compact_timeline(items: Vec<TimelineItem>) -> Vec<DisplayTimelineItem> {
    let mut out = Vec::with_capacity(items.len());
    let mut last_user_text: Option<String> = None;
    let mut iter = items.into_iter().peekable();

    while let Some(item) = iter.next() {
        match item {
            TimelineItem::User { text } => {
                last_user_text = Some(text.clone());
                out.push(DisplayTimelineItem::User { text });
            }
            TimelineItem::Assistant { text, metrics } => {
                out.push(DisplayTimelineItem::Assistant {
                    text,
                    prev_user: last_user_text.clone(),
                    metrics,
                });
            }
            TimelineItem::ModelDecision { metrics } => {
                // Collect the tool calls that follow this model round into a
                // single grouped block instead of emitting separate Tool rows.
                let mut tools = Vec::new();
                while iter
                    .peek()
                    .is_some_and(|x| matches!(x, TimelineItem::Tool(_)))
                {
                    if let Some(TimelineItem::Tool(t)) = iter.next() {
                        tools.push(t);
                    }
                }
                out.push(DisplayTimelineItem::ModelRound {
                    metrics,
                    tools: group_consecutive_tools(tools),
                });
            }
            TimelineItem::Tool(tool) => {
                // Standalone tool not preceded by a ModelDecision (e.g. a
                // text+tool round where metrics merged into the Assistant row).
                out.push(DisplayTimelineItem::Tool(tool));
            }
            TimelineItem::Thinking { text, done } => {
                out.push(DisplayTimelineItem::Thinking { text, done });
            }
            TimelineItem::GeneratedImage {
                prompt,
                mime,
                preview_src,
                saved_path,
                filename,
            } => {
                out.push(DisplayTimelineItem::GeneratedImage {
                    prompt,
                    mime,
                    preview_src,
                    saved_path,
                    filename,
                });
            }
            TimelineItem::SubagentGroup(group) => {
                out.push(DisplayTimelineItem::SubagentGroup(group));
            }
            TimelineItem::AskUser {
                call_id,
                question,
                header,
                options,
                multi_select,
                allow_other,
                state,
            } => {
                out.push(DisplayTimelineItem::AskUser {
                    call_id,
                    question,
                    header,
                    options,
                    multi_select,
                    allow_other,
                    state,
                });
            }
        }
    }

    merge_consecutive_model_rounds(out)
}

/// Returns the last path component of a workspace-relative path for display.
fn path_tail(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_owned()
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
    tool_detail_open: RwSignal<HashMap<String, bool>>,
    voice_handle: VoiceOrbHandle,
    on_redo: Callback<String>,
    timeline: RwSignal<Vec<TimelineItem>>,
    wb: WorkbenchService,
    workspace_id: Option<u64>,
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
        DisplayTimelineItem::Assistant { text, prev_user, metrics } => {
            let tts_text = text.clone();
            let copy_text = text.clone();
            let redo_text = prev_user.clone();
            let copied = RwSignal::new(false);
            view! {
            <li class="agent-chat-line agent-chat-line--agent">
                <ChatLineIndexColumn line_no=line_no.clone() tts_text=Some(tts_text) voice_handle=voice_handle />
                <div class="agent-chat-body">
                    <strong>{move || i18n.tr(I18nKey::AgAssistant)()}</strong>
                    <div class="workbench-agent-markdown" inner_html=render_markdown_to_html(&text)></div>
                    <div class="agent-chat-actions">
                        <button
                            type="button"
                            class="agent-chat-action"
                            title="Copy answer to clipboard"
                            aria-label="Copy answer"
                            on:click=move |_| {
                                let text = copy_text.clone();
                                copied.set(true);
                                leptos::task::spawn_local(async move {
                                    if let Some(win) = web_sys::window() {
                                        let clipboard = win.navigator().clipboard();
                                        let promise = clipboard.write_text(&text);
                                        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
                                    }
                                    gloo_timers::future::TimeoutFuture::new(1400).await;
                                    copied.set(false);
                                });
                            }
                        >
                            {move || if copied.get() {
                                view! { <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" /> }
                            } else {
                                view! { <LxIcon icon=icondata::LuCopy width="0.78rem" height="0.78rem" /> }
                            }}
                        </button>
                        <Show when={
                            let r = redo_text.clone();
                            move || r.as_ref().is_some_and(|s| !s.trim().is_empty())
                        }>
                            <button
                                type="button"
                                class="agent-chat-action"
                                title="Redo this turn (resubmit the same prompt)"
                                aria-label="Redo"
                                on:click={
                                    let r = redo_text.clone();
                                    move |_| {
                                        if let Some(text) = r.clone() {
                                            on_redo.run(text);
                                        }
                                    }
                                }
                            >
                                <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                            </button>
                        </Show>
                    </div>
                    <TurnMetricsBar metrics=metrics context=BarContext::Main />
                </div>
            </li>
        }
            .into_any()
        }
        DisplayTimelineItem::Tool(tool) => {
            let detail_key = tool_detail_key(idx, &tool.tool, tool.call_id.as_deref(), None);
            view! {
                <ToolActivityRow
                    line_no=line_no
                    tool=tool
                    detail_key=detail_key
                    tool_detail_open=tool_detail_open
                    voice_handle=voice_handle
                />
            }
            .into_any()
        }
        DisplayTimelineItem::ModelRound { metrics, tools } => {
            let loc = i18n.locale().get_untracked();
            let label = lookup(loc, I18nKey::AgMetricsModelRound).to_string();
            view! {
                <li class="agent-chat-line agent-chat-line--model-round">
                    <ChatLineIndexColumn line_no=line_no.clone() tts_text=None voice_handle=voice_handle />
                    <div class="agent-chat-body">
                        <span class="agent-chat-decision-label">{label}</span>
                        <ul class="model-round-tools">
                            {tools.into_iter().enumerate().map(|(ti, tool)| {
                                let status_class = match tool.status {
                                    ActivityStatus::Ok => "agent-tool-row--ok",
                                    ActivityStatus::Fail => "agent-tool-row--fail",
                                    ActivityStatus::Pending => "agent-tool-row--pending",
                                };
                                let status_icon = match tool.status {
                                    ActivityStatus::Ok => icondata::LuCheck,
                                    ActivityStatus::Fail => icondata::LuTriangleAlert,
                                    ActivityStatus::Pending => icondata::LuLoader,
                                };
                                let tool_icon_val = tool_icon(&tool.tool);
                                let label = tool.label.clone();
                                // For grouped calls: show "×N" count instead of single-arg summary
                                let merged_count = tool.merged_count;
                                let summary = if merged_count > 1 {
                                    String::new()
                                } else {
                                    tool.args_summary.clone()
                                };
                                let count_badge = if merged_count > 1 {
                                    format!("×{merged_count}")
                                } else {
                                    String::new()
                                };
                                let tool_name = tool.tool.clone();
                                let has_paths = !tool.paths.is_empty();
                                let has_detail = has_paths
                                    || tool.detail.as_ref().is_some_and(|s| !s.is_empty());
                                let detail_text = tool.detail.clone().unwrap_or_default();
                                let paths_sv = StoredValue::new(tool.paths.clone());
                                let detail_key =
                                    tool_detail_key(idx, &tool_name, tool.call_id.as_deref(), Some(ti));
                                let detail_key_memo = detail_key.clone();
                                let detail_open = Memo::new(move |_| {
                                    tool_detail_open
                                        .with(|m| m.get(&detail_key_memo).copied().unwrap_or(false))
                                });
                                view! {
                                    <li class="model-round-tool-item">
                                        <div class=format!("agent-tool-row {status_class}") title=tool_name>
                                            <button
                                                type="button"
                                                class="agent-tool-row__head"
                                                aria-expanded=move || detail_open.get().to_string()
                                                prop:disabled=move || !has_detail
                                                on:click=move |_| {
                                                    if has_detail {
                                                        tool_detail_open.update(|m| {
                                                            let cur =
                                                                m.get(&detail_key).copied().unwrap_or(false);
                                                            m.insert(detail_key.clone(), !cur);
                                                        });
                                                    }
                                                }
                                            >
                                                <span class="agent-tool-row__icon" aria-hidden="true">
                                                    <LxIcon icon=tool_icon_val width="0.82rem" height="0.82rem" />
                                                </span>
                                                <span class="agent-tool-row__label">{label}</span>
                                                <Show when={let s = summary.clone(); move || !s.is_empty()}>
                                                    <span class="agent-tool-row__arg">{summary.clone()}</span>
                                                </Show>
                                                <Show when={let b = count_badge.clone(); move || !b.is_empty()}>
                                                    <span class="agent-tool-row__count">{count_badge.clone()}</span>
                                                </Show>
                                                <span class="agent-tool-row__status" aria-hidden="true">
                                                    <LxIcon icon=status_icon width="0.78rem" height="0.78rem" />
                                                </span>
                                            </button>
                                            {move || {
                                                if !has_detail || !detail_open.get() {
                                                    return view! { <></> }.into_any();
                                                }
                                                if has_paths {
                                                    view! {
                                                        <ul class="tool-row-paths">
                                                            {paths_sv.get_value().into_iter().map(|p| {
                                                                let display = path_tail(&p);
                                                                let p_open = p.clone();
                                                                view! {
                                                                    <li>
                                                                        <button
                                                                            type="button"
                                                                            class="tool-row-path-btn"
                                                                            title=p.clone()
                                                                            on:click=move |_| {
                                                                                if let Some(ws_id) = workspace_id {
                                                                                    wb.open_center_file_tab(ws_id, p_open.clone());
                                                                                }
                                                                            }
                                                                        >{display}</button>
                                                                    </li>
                                                                }
                                                            }).collect_view()}
                                                        </ul>
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <pre class="agent-tool-row__detail">{detail_text.clone()}</pre>
                                                    }.into_any()
                                                }
                                            }}
                                        </div>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                        <TurnMetricsBar metrics=metrics context=BarContext::Main />
                    </div>
                </li>
            }
            .into_any()
        }
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
                            let card_metrics = card.metrics;
                            view! {
                                <details class="agent-subagent-card subagent-card" open>
                                    <summary class="agent-subagent-card__summary">
                                        <span class="agent-subagent-card__name">{name}</span>
                                        <span class="agent-subagent-card__status">{status}</span>
                                        <Show when=move || is_running>
                                            <span class="agent-subagent-card__pulse" aria-hidden="true">
                                                <span></span><span></span><span></span>
                                            </span>
                                        </Show>
                                        <TurnMetricsBar metrics=card_metrics context=BarContext::Subagent />
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
                                        {card.tools.into_iter().map(|tool| {
                                            let label = tool.label.clone();
                                            let metrics = tool.metrics;
                                            view! {
                                                <li>
                                                    <span>{label}</span>
                                                    <TurnMetricsBar metrics=metrics context=BarContext::Subagent />
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
        DisplayTimelineItem::AskUser {
            call_id,
            question,
            header,
            options,
            multi_select,
            allow_other,
            state,
        } => view! {
            <li class="agent-chat-line agent-chat-line--ask-user">
                <ChatLineIndexColumn line_no=line_no.clone() tts_text=None voice_handle=voice_handle />
                <div class="agent-chat-body">
                    <AskUserCard
                        call_id=call_id
                        question=question
                        header=header
                        options=options
                        multi_select=multi_select
                        allow_other=allow_other
                        state=state
                        timeline=timeline
                    />
                </div>
            </li>
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
            if let Ok(resp) = crate::tauri_bridge::generated_image_preview(path).await {
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
    let body_ref = NodeRef::<html::Pre>::new();
    let body_scroll_top = StoredValue::new(0i32);
    Effect::new(move |_| {
        let _ = text.len();
        let Some(pre) = body_ref.get() else {
            return;
        };
        let sh = pre.scroll_height();
        let ch = pre.client_height();
        let st = body_scroll_top.get_value();
        let at_bottom = sh - st - ch < 8;
        pre.set_scroll_top(if at_bottom { sh } else { st });
    });
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
                    <pre
                        class="agent-thinking-card__body"
                        node_ref=body_ref
                        on:scroll=move |_| {
                            if let Some(pre) = body_ref.get() {
                                body_scroll_top.set_value(pre.scroll_top());
                            }
                        }
                    >
                        {body.clone()}
                    </pre>
                </Show>
            </div>
        </li>
    }
}

#[component]
fn ToolActivityRow(
    line_no: String,
    tool: ToolActivity,
    detail_key: String,
    tool_detail_open: RwSignal<HashMap<String, bool>>,
    voice_handle: VoiceOrbHandle,
) -> impl IntoView {
    let status_class = match tool.status {
        ActivityStatus::Pending => "agent-tool-row--pending",
        ActivityStatus::Ok => "agent-tool-row--ok",
        ActivityStatus::Fail => "agent-tool-row--fail",
    };
    let status_icon = match tool.status {
        ActivityStatus::Pending => icondata::LuLoader,
        ActivityStatus::Ok => icondata::LuCheck,
        ActivityStatus::Fail => icondata::LuTriangleAlert,
    };

    let detail_key_memo = detail_key.clone();
    let detail_open = Memo::new(move |_| {
        tool_detail_open
            .with(|m| m.get(&detail_key_memo).copied().unwrap_or(false))
    });
    let has_detail = tool.detail.as_ref().is_some_and(|s| !s.is_empty());
    let detail_text = tool.detail.clone().unwrap_or_default();
    let label = tool.label.clone();
    let summary = tool.args_summary.clone();
    let tool_name_for_title = tool.tool.clone();
    let metrics = tool.metrics;

    view! {
        <li class="agent-chat-line agent-chat-line--tool">
            <ChatLineIndexColumn line_no=line_no tts_text=None voice_handle=voice_handle />
            <div class="agent-chat-body">
                <div class=format!("agent-tool-row {status_class}") title=tool_name_for_title>
                    <button
                        type="button"
                        class="agent-tool-row__head"
                        aria-expanded=move || detail_open.get().to_string()
                        prop:disabled=move || !has_detail
                        on:click=move |_| {
                            if has_detail {
                                let key = detail_key.clone();
                                tool_detail_open.update(|m| {
                                    let cur = m.get(&key).copied().unwrap_or(false);
                                    m.insert(key, !cur);
                                });
                            }
                        }
                    >
                        <span class="agent-tool-row__icon" aria-hidden="true">
                            <LxIcon icon=tool_icon(&tool.tool) width="0.82rem" height="0.82rem" />
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
                <TurnMetricsBar metrics=metrics context=BarContext::Main />
            </div>
        </li>
    }
}
