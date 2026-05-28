use crate::agent_wire::{AgentEvent, EventEnvelope, TaskSnapshot, TurnMetrics, TurnUsageKind};
use crate::i18n::Locale;
use crate::workbench::agent_panel::timeline::parse_ask_user_args;
use crate::workbench::agent_timeline::{
    is_empty_pending_thinking, tool_label, ActivityStatus, AskUserState, SubagentStatus,
    TimelineDoc, ToolActivity, ToolState, TurnNode, TurnPart,
};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;

#[inline]
fn persist_agent_timeline(
    persist: Option<(WorkbenchService, u64)>,
    timeline: RwSignal<TimelineDoc>,
) {
    if let Some((wb, workspace_id)) = persist {
        wb.set_workspace_agent_timeline(
            workspace_id,
            timeline.get_untracked().sanitize_for_persistence(),
        );
    }
}

pub fn apply_envelope(
    env: &EventEnvelope,
    timeline: RwSignal<TimelineDoc>,
    task_snapshot: RwSignal<TaskSnapshot>,
    loc: Locale,
    persist: Option<(WorkbenchService, u64)>,
) {
    match &env.event {
        AgentEvent::TaskSnapshot { snapshot } => {
            task_snapshot.set(snapshot.clone());
            if let Some((wb, ws_id)) = persist {
                crate::workbench::agent_context_handoff::store_task_snapshot(
                    ws_id,
                    snapshot.clone(),
                );
                let _ = wb;
            }
            return;
        }
        AgentEvent::VoiceReady { .. } | AgentEvent::ImageContextConsumed { .. } => return,
        _ => {}
    }

    timeline.update(|doc| apply_event_to_doc(doc, env, loc, persist));
    persist_agent_timeline(persist, timeline);
}

fn apply_event_to_doc(
    doc: &mut TimelineDoc,
    env: &EventEnvelope,
    loc: Locale,
    persist: Option<(WorkbenchService, u64)>,
) {
    ensure_turn(doc);
    match &env.event {
        AgentEvent::AssistantDelta { delta } => append_text(doc, env, None, delta),
        AgentEvent::ThinkingDelta { delta } => append_thinking(doc, env, None, delta),
        AgentEvent::ThinkingDone => mark_thinking_done(doc, env, None),
        AgentEvent::ToolCall {
            tool,
            call_id,
            args,
        } => {
            if tool == "harness.ask_user" {
                if let Some((call_id, ask)) = call_id
                    .clone()
                    .zip(args.as_ref().and_then(parse_ask_user_args))
                {
                    append_part(
                        doc,
                        env.parent_call_id.as_deref(),
                        None,
                        TurnPart::AskUser {
                            id: call_id.clone(),
                            call_id,
                            question: ask.question,
                            header: ask.header,
                            options: ask.options,
                            multi_select: ask.multi_select,
                            allow_other: ask.allow_other,
                            state: AskUserState::Open,
                        },
                    );
                    return;
                }
            }
            let id = call_id
                .clone()
                .unwrap_or_else(|| format!("tool-{}", env.seq));
            let activity =
                ToolActivity::from_call_with_id(tool, args.as_ref(), loc, Some(id.clone()));
            append_part(
                doc,
                env.parent_call_id.as_deref(),
                None,
                TurnPart::Tool {
                    id,
                    tool: tool.clone(),
                    label: activity.label,
                    args_summary: activity.args_summary,
                    args: args.clone(),
                    state: ToolState::Pending,
                    result: None,
                    metrics: TurnMetrics::default(),
                    children: Vec::new(),
                    paths: activity.paths,
                    merged_count: 1,
                },
            );
        }
        AgentEvent::ToolResult {
            tool,
            call_id,
            ok,
            message,
        } => {
            if tool == "harness.ask_user" {
                return;
            }
            if let Some(cid) = call_id.as_deref() {
                if let Some(part) = find_tool_mut(&mut doc.turns, cid) {
                    set_tool_result(part, *ok, message.clone());
                    if env.parent_call_id.is_none() {
                        append_waiting_placeholder(doc, env.seq);
                    }
                    return;
                }
            }
            let id = call_id
                .clone()
                .unwrap_or_else(|| format!("tool-result-{}", env.seq));
            append_part(
                doc,
                env.parent_call_id.as_deref(),
                None,
                TurnPart::Tool {
                    id,
                    tool: tool.clone(),
                    label: tool_label(tool, loc),
                    args_summary: String::new(),
                    args: None,
                    state: if *ok {
                        ToolState::Success
                    } else {
                        ToolState::Error
                    },
                    result: message.clone().filter(|m| !m.is_empty()),
                    metrics: TurnMetrics::default(),
                    children: Vec::new(),
                    paths: Vec::new(),
                    merged_count: 1,
                },
            );
            if env.parent_call_id.is_none() {
                append_waiting_placeholder(doc, env.seq);
            }
        }
        AgentEvent::SubagentStarted {
            agent_id,
            role,
            display_name,
        } => append_part(
            doc,
            env.parent_call_id.as_deref(),
            None,
            TurnPart::Subagent {
                id: agent_id.clone(),
                role: role.clone(),
                display_name: display_name.clone(),
                status: SubagentStatus::Running,
                parts: Vec::new(),
                metrics: TurnMetrics::default(),
                summary: None,
                steps: Vec::new(),
            },
        ),
        AgentEvent::SubagentStep {
            agent_id,
            step_id,
            title,
            status,
            note,
        } => {
            if let Some(TurnPart::Subagent { steps, .. }) =
                find_subagent_mut(&mut doc.turns, agent_id)
            {
                if let Some(step) = steps.iter_mut().find(|s| s.id == *step_id) {
                    step.title = title.clone();
                    step.status = status.clone();
                    step.note = note.clone();
                } else {
                    steps.push(crate::workbench::agent_timeline::SubagentStepRow {
                        id: step_id.clone(),
                        title: title.clone(),
                        status: status.clone(),
                        note: note.clone(),
                    });
                }
            }
        }
        AgentEvent::SubagentToolCall {
            agent_id,
            tool,
            call_id,
            args,
        } => {
            let id = call_id
                .clone()
                .unwrap_or_else(|| format!("{agent_id}-tool-{}", env.seq));
            let activity =
                ToolActivity::from_call_with_id(tool, args.as_ref(), loc, Some(id.clone()));
            append_part(
                doc,
                env.parent_call_id.as_deref(),
                Some(agent_id),
                TurnPart::Tool {
                    id,
                    tool: tool.clone(),
                    label: activity.label,
                    args_summary: activity.args_summary,
                    args: args.clone(),
                    state: ToolState::Pending,
                    result: None,
                    metrics: TurnMetrics::default(),
                    children: Vec::new(),
                    paths: activity.paths,
                    merged_count: 1,
                },
            );
        }
        AgentEvent::SubagentAssistantDelta { agent_id, delta } => {
            append_text(doc, env, Some(agent_id), delta);
        }
        AgentEvent::SubagentThinkingDelta { agent_id, delta } => {
            append_thinking(doc, env, Some(agent_id), delta);
        }
        AgentEvent::SubagentThinkingDone { agent_id } => {
            mark_thinking_done(doc, env, Some(agent_id))
        }
        AgentEvent::SubagentFinished {
            agent_id,
            status,
            summary,
        } => {
            if let Some(TurnPart::Subagent {
                status: target_status,
                summary: target_summary,
                parts,
                ..
            }) = find_subagent_mut(&mut doc.turns, agent_id)
            {
                *target_status = match status.as_str() {
                    "failed" | "error" => SubagentStatus::Error,
                    _ => SubagentStatus::Done,
                };
                *target_summary = (!summary.is_empty()).then_some(summary.clone());
                complete_pending_tools(parts);
            }
        }
        AgentEvent::TurnUsage {
            kind,
            agent_id,
            call_id,
            round_index,
            turn_generation,
            input_tokens,
            output_tokens,
            ttft_ms,
            elapsed_ms,
            cost_usd,
        } => {
            let metrics = TurnMetrics {
                input_tokens: *input_tokens,
                output_tokens: *output_tokens,
                ttft_ms: *ttft_ms,
                elapsed_ms: *elapsed_ms,
                cost_usd: *cost_usd,
            };
            if let Some((wb, ws_id)) = persist {
                let _ = wb.record_chat_turn_usage(
                    ws_id,
                    *turn_generation,
                    *input_tokens,
                    *output_tokens,
                    *elapsed_ms,
                    *cost_usd,
                );
            }
            match kind {
                TurnUsageKind::ToolExec => {
                    if let Some(cid) = call_id.as_deref() {
                        if let Some(TurnPart::Tool {
                            metrics: m, state, ..
                        }) = find_tool_mut(&mut doc.turns, cid)
                        {
                            *m = metrics;
                            if matches!(state, ToolState::Pending | ToolState::Running) {
                                *state = ToolState::Success;
                            }
                        }
                    }
                }
                TurnUsageKind::ModelRound => {
                    if let Some(agent_id) = agent_id {
                        if let Some(TurnPart::Subagent { metrics: m, .. }) =
                            find_subagent_mut(&mut doc.turns, agent_id)
                        {
                            m.merge(&metrics);
                        }
                    } else if !merge_model_metrics_into_latest_text(
                        doc,
                        env.parent_call_id.as_deref(),
                        metrics,
                    ) {
                        append_part(
                            doc,
                            env.parent_call_id.as_deref(),
                            None,
                            TurnPart::ModelRound {
                                id: format!("round-{}", round_index.unwrap_or(env.seq as u32)),
                                metrics,
                            },
                        );
                    }
                }
            }
        }
        AgentEvent::ImageGenerated {
            prompt,
            mime,
            saved_path,
            filename,
            preview_src,
        } => append_part(
            doc,
            env.parent_call_id.as_deref(),
            None,
            TurnPart::GeneratedImage {
                id: format!("image-{}", env.seq),
                prompt: prompt.clone(),
                mime: mime.clone(),
                preview_src: preview_src.clone(),
                saved_path: saved_path.clone(),
                filename: filename.clone(),
            },
        ),
        AgentEvent::Error { message } => append_part(
            doc,
            env.parent_call_id.as_deref(),
            None,
            TurnPart::Text {
                id: format!("error-{}", env.seq),
                text: format!("Fehler: {message}"),
                metrics: TurnMetrics::default(),
            },
        ),
        AgentEvent::Done
        | AgentEvent::TaskSnapshot { .. }
        | AgentEvent::VoiceReady { .. }
        | AgentEvent::ImageContextConsumed { .. } => {
            remove_empty_pending_top_level_thinking(doc);
        }
    }
}

fn ensure_turn(doc: &mut TimelineDoc) {
    if doc.turns.is_empty() {
        doc.push_user_turn(String::new());
    }
}

fn append_text(doc: &mut TimelineDoc, env: &EventEnvelope, agent_id: Option<&String>, delta: &str) {
    let Some(parts) = target_parts_mut(
        doc,
        env.parent_call_id.as_deref(),
        agent_id.map(String::as_str),
    ) else {
        return;
    };
    remove_empty_pending_thinking(parts);
    match parts.last_mut() {
        Some(TurnPart::Text { text, .. }) => text.push_str(delta),
        _ => parts.push(TurnPart::Text {
            id: format!("text-{}", env.seq),
            text: delta.to_owned(),
            metrics: TurnMetrics::default(),
        }),
    }
}

fn append_thinking(
    doc: &mut TimelineDoc,
    env: &EventEnvelope,
    agent_id: Option<&String>,
    delta: &str,
) {
    let Some(parts) = target_parts_mut(
        doc,
        env.parent_call_id.as_deref(),
        agent_id.map(String::as_str),
    ) else {
        return;
    };
    match parts.last_mut() {
        Some(TurnPart::Thinking {
            text, done: false, ..
        }) => text.push_str(delta),
        _ => parts.push(TurnPart::Thinking {
            id: format!("think-{}", env.seq),
            text: delta.to_owned(),
            done: false,
        }),
    }
}

fn mark_thinking_done(doc: &mut TimelineDoc, env: &EventEnvelope, agent_id: Option<&String>) {
    if let Some(parts) = target_parts_mut(
        doc,
        env.parent_call_id.as_deref(),
        agent_id.map(String::as_str),
    ) {
        if parts.last().is_some_and(is_empty_pending_thinking) {
            parts.pop();
            return;
        }
        if let Some(TurnPart::Thinking { done, .. }) = parts
            .iter_mut()
            .rev()
            .find(|part| matches!(part, TurnPart::Thinking { done: false, .. }))
        {
            *done = true;
        }
    }
}

fn append_part(
    doc: &mut TimelineDoc,
    parent_call_id: Option<&str>,
    agent_id: Option<&String>,
    part: TurnPart,
) {
    if let Some(parts) = target_parts_mut(doc, parent_call_id, agent_id.map(String::as_str)) {
        remove_empty_pending_thinking(parts);
        parts.push(part);
    }
}

fn append_waiting_placeholder(doc: &mut TimelineDoc, seq: u64) {
    let Some(turn) = doc.turns.last_mut() else {
        return;
    };
    if turn.parts.last().is_some_and(is_empty_pending_thinking) {
        return;
    }
    turn.parts.push(TurnPart::Thinking {
        id: format!("think-pending-after-tool-{seq}"),
        text: String::new(),
        done: false,
    });
}

fn remove_empty_pending_top_level_thinking(doc: &mut TimelineDoc) {
    if let Some(turn) = doc.turns.last_mut() {
        remove_empty_pending_thinking(&mut turn.parts);
    }
}

fn remove_empty_pending_thinking(parts: &mut Vec<TurnPart>) {
    parts.retain(|part| !is_empty_pending_thinking(part));
}

fn merge_model_metrics_into_latest_text(
    doc: &mut TimelineDoc,
    parent_call_id: Option<&str>,
    metrics: TurnMetrics,
) -> bool {
    let Some(parts) = target_parts_mut(doc, parent_call_id, None) else {
        return false;
    };
    remove_empty_pending_thinking(parts);
    for part in parts.iter_mut().rev() {
        match part {
            TurnPart::Text { metrics: m, .. } => {
                m.merge(&metrics);
                return true;
            }
            TurnPart::Thinking { .. } => continue,
            _ => break,
        }
    }
    false
}

fn target_parts_mut<'a>(
    doc: &'a mut TimelineDoc,
    parent_call_id: Option<&str>,
    agent_id: Option<&str>,
) -> Option<&'a mut Vec<TurnPart>> {
    let turn = doc.turns.last_mut()?;
    let top_parts = &mut turn.parts as *mut Vec<TurnPart>;
    if let Some(agent_id) = agent_id {
        if let Some(parts) = find_subagent_parts_mut(&mut turn.parts, agent_id) {
            return Some(parts);
        }
        // SAFETY: `top_parts` points at the current turn's parts. No borrow
        // from the failed search escapes this branch.
        return Some(unsafe { &mut *top_parts });
    }
    if let Some(parent) = parent_call_id {
        if let Some(parts) = find_parent_container_parts_mut(&mut turn.parts, parent) {
            return Some(parts);
        }
        // SAFETY: same as above; the recursive search returned `None`.
        return Some(unsafe { &mut *top_parts });
    }
    Some(unsafe { &mut *top_parts })
}

fn set_tool_result(part: &mut TurnPart, ok: bool, message: Option<String>) {
    if let TurnPart::Tool { state, result, .. } = part {
        *state = if ok {
            ToolState::Success
        } else {
            ToolState::Error
        };
        *result = message.filter(|m| !m.is_empty());
    }
}

fn complete_pending_tools(parts: &mut [TurnPart]) {
    for part in parts {
        match part {
            TurnPart::Tool {
                state, children, ..
            } => {
                if matches!(state, ToolState::Pending | ToolState::Running) {
                    *state = ToolState::Success;
                }
                complete_pending_tools(children);
            }
            TurnPart::Subagent { parts, .. } => complete_pending_tools(parts),
            _ => {}
        }
    }
}

fn find_subagent_parts_mut<'a>(
    parts: &'a mut [TurnPart],
    id: &str,
) -> Option<&'a mut Vec<TurnPart>> {
    for part in parts {
        match part {
            TurnPart::Subagent {
                id: part_id,
                parts: child_parts,
                ..
            } => {
                if part_id == id {
                    return Some(child_parts);
                }
                if let Some(found) = find_subagent_parts_mut(child_parts, id) {
                    return Some(found);
                }
            }
            TurnPart::Tool { children, .. } => {
                if let Some(found) = find_subagent_parts_mut(children, id) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_parent_container_parts_mut<'a>(
    parts: &'a mut [TurnPart],
    id: &str,
) -> Option<&'a mut Vec<TurnPart>> {
    for part in parts {
        match part {
            TurnPart::Tool {
                id: part_id,
                children,
                ..
            } => {
                if part_id == id {
                    return Some(children);
                }
                if let Some(found) = find_parent_container_parts_mut(children, id) {
                    return Some(found);
                }
            }
            TurnPart::Subagent {
                id: part_id,
                parts: child_parts,
                ..
            } => {
                if part_id == id {
                    return Some(child_parts);
                }
                if let Some(found) = find_parent_container_parts_mut(child_parts, id) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_tool_mut<'a>(turns: &'a mut [TurnNode], id: &str) -> Option<&'a mut TurnPart> {
    for turn in turns.iter_mut().rev() {
        if let Some(found) = find_tool_in_parts_mut(&mut turn.parts, id) {
            return Some(found);
        }
    }
    None
}

fn find_tool_in_parts_mut<'a>(parts: &'a mut [TurnPart], id: &str) -> Option<&'a mut TurnPart> {
    for part in parts {
        if matches!(part, TurnPart::Tool { id: part_id, .. } if part_id == id) {
            return Some(part);
        }
        match part {
            TurnPart::Tool { children, .. } => {
                if let Some(found) = find_tool_in_parts_mut(children, id) {
                    return Some(found);
                }
            }
            TurnPart::Subagent { parts, .. } => {
                if let Some(found) = find_tool_in_parts_mut(parts, id) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn find_subagent_mut<'a>(turns: &'a mut [TurnNode], id: &str) -> Option<&'a mut TurnPart> {
    for turn in turns.iter_mut().rev() {
        if let Some(found) = find_subagent_in_parts_mut(&mut turn.parts, id) {
            return Some(found);
        }
    }
    None
}

fn find_subagent_in_parts_mut<'a>(parts: &'a mut [TurnPart], id: &str) -> Option<&'a mut TurnPart> {
    for part in parts {
        if matches!(part, TurnPart::Subagent { id: part_id, .. } if part_id == id) {
            return Some(part);
        }
        match part {
            TurnPart::Subagent {
                parts: child_parts, ..
            } => {
                if let Some(found) = find_subagent_in_parts_mut(child_parts, id) {
                    return Some(found);
                }
            }
            TurnPart::Tool { children, .. } => {
                if let Some(found) = find_subagent_in_parts_mut(children, id) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

#[allow(dead_code)]
fn _activity_status_from_tool_state(state: &ToolState) -> ActivityStatus {
    match state {
        ToolState::Pending | ToolState::Running => ActivityStatus::Pending,
        ToolState::Success => ActivityStatus::Ok,
        ToolState::Error => ActivityStatus::Fail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Locale;
    use serde_json::json;

    fn env(seq: u64, parent_call_id: Option<&str>, event: AgentEvent) -> EventEnvelope {
        EventEnvelope {
            seq,
            parent_call_id: parent_call_id.map(str::to_owned),
            event,
        }
    }

    #[test]
    fn reducer_streams_into_existing_text_part() {
        let mut doc = TimelineDoc::default();
        doc.push_user_turn("hi".to_owned());
        apply_event_to_doc(
            &mut doc,
            &env(
                1,
                None,
                AgentEvent::AssistantDelta {
                    delta: "hel".to_owned(),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                2,
                None,
                AgentEvent::AssistantDelta {
                    delta: "lo".to_owned(),
                },
            ),
            Locale::EnUs,
            None,
        );

        assert_eq!(doc.turns[0].parts.len(), 1);
        match &doc.turns[0].parts[0] {
            TurnPart::Text { id, text, .. } => {
                assert_eq!(id, "text-1");
                assert_eq!(text, "hello");
            }
            other => panic!("expected text part, got {other:?}"),
        }
    }

    #[test]
    fn reducer_new_text_part_after_tool() {
        let mut doc = TimelineDoc::default();
        doc.push_user_turn("hi".to_owned());
        apply_event_to_doc(
            &mut doc,
            &env(1, None, AgentEvent::AssistantDelta { delta: "a".into() }),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                2,
                None,
                AgentEvent::ToolCall {
                    tool: "read_workspace_file".into(),
                    call_id: Some("cid-t".into()),
                    args: Some(json!({"path": "src/main.rs"})),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                3,
                None,
                AgentEvent::ToolResult {
                    tool: "read_workspace_file".into(),
                    call_id: Some("cid-t".into()),
                    ok: true,
                    message: Some("ok".into()),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(4, None, AgentEvent::AssistantDelta { delta: "b".into() }),
            Locale::EnUs,
            None,
        );

        let text_ids = doc.turns[0]
            .parts
            .iter()
            .filter_map(|part| match part {
                TurnPart::Text { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(text_ids, vec!["text-1", "text-4"]);
    }

    #[test]
    fn reducer_model_round_metrics_merge_into_latest_text_part() {
        let mut doc = TimelineDoc::default();
        doc.push_user_turn("hi".to_owned());
        apply_event_to_doc(
            &mut doc,
            &env(1, None, AgentEvent::AssistantDelta { delta: "hi".into() }),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                2,
                None,
                AgentEvent::TurnUsage {
                    kind: TurnUsageKind::ModelRound,
                    agent_id: None,
                    call_id: None,
                    round_index: Some(0),
                    turn_generation: 0,
                    input_tokens: Some(90),
                    output_tokens: Some(4),
                    ttft_ms: Some(100),
                    elapsed_ms: 200,
                    cost_usd: Some(0.01),
                },
            ),
            Locale::EnUs,
            None,
        );

        assert_eq!(doc.turns[0].parts.len(), 1);
        match &doc.turns[0].parts[0] {
            TurnPart::Text { metrics, .. } => {
                assert_eq!(metrics.input_tokens, Some(90));
                assert_eq!(metrics.output_tokens, Some(4));
                assert_eq!(metrics.cost_usd, Some(0.01));
            }
            other => panic!("expected text part, got {other:?}"),
        }
    }

    #[test]
    fn reducer_nests_subagent_under_tool() {
        let mut doc = TimelineDoc::default();
        doc.push_user_turn("run".to_owned());
        apply_event_to_doc(
            &mut doc,
            &env(
                1,
                None,
                AgentEvent::ToolCall {
                    tool: "subagents.run".into(),
                    call_id: Some("cid-run".into()),
                    args: Some(json!({})),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                2,
                Some("cid-run"),
                AgentEvent::SubagentStarted {
                    agent_id: "sa1".into(),
                    role: "scout".into(),
                    display_name: "Scout".into(),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                3,
                Some("cid-run"),
                AgentEvent::SubagentToolCall {
                    agent_id: "sa1".into(),
                    tool: "read_workspace_file".into(),
                    call_id: Some("cid-t1".into()),
                    args: Some(json!({"path": "Cargo.toml"})),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                4,
                Some("cid-run"),
                AgentEvent::TurnUsage {
                    kind: TurnUsageKind::ToolExec,
                    agent_id: Some("sa1".into()),
                    call_id: Some("cid-t1".into()),
                    round_index: Some(0),
                    turn_generation: 0,
                    input_tokens: None,
                    output_tokens: None,
                    ttft_ms: None,
                    elapsed_ms: 12,
                    cost_usd: None,
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                5,
                Some("cid-run"),
                AgentEvent::SubagentFinished {
                    agent_id: "sa1".into(),
                    status: "completed".into(),
                    summary: "done".into(),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                6,
                None,
                AgentEvent::ToolResult {
                    tool: "subagents.run".into(),
                    call_id: Some("cid-run".into()),
                    ok: true,
                    message: Some("ok".into()),
                },
            ),
            Locale::EnUs,
            None,
        );

        assert_eq!(doc.turns[0].parts.len(), 2);
        assert!(is_empty_pending_thinking(&doc.turns[0].parts[1]));
        let TurnPart::Tool {
            id,
            state,
            children,
            ..
        } = &doc.turns[0].parts[0]
        else {
            panic!("expected top-level tool");
        };
        assert_eq!(id, "cid-run");
        assert_eq!(*state, ToolState::Success);
        assert_eq!(children.len(), 1);
        let TurnPart::Subagent {
            id,
            status,
            parts,
            summary,
            ..
        } = &children[0]
        else {
            panic!("expected nested subagent");
        };
        assert_eq!(id, "sa1");
        assert_eq!(*status, SubagentStatus::Done);
        assert_eq!(summary.as_deref(), Some("done"));
        assert!(matches!(
            &parts[0],
            TurnPart::Tool { id, state, .. } if id == "cid-t1" && *state == ToolState::Success
        ));
    }

    #[test]
    fn reducer_subagent_finish_completes_submit_result_tool() {
        let mut doc = TimelineDoc::default();
        doc.push_user_turn("run".to_owned());
        apply_event_to_doc(
            &mut doc,
            &env(
                1,
                None,
                AgentEvent::ToolCall {
                    tool: "subagents.run".into(),
                    call_id: Some("cid-run".into()),
                    args: Some(json!({})),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                2,
                Some("cid-run"),
                AgentEvent::SubagentStarted {
                    agent_id: "sa1".into(),
                    role: "scout".into(),
                    display_name: "Scout".into(),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                3,
                Some("cid-run"),
                AgentEvent::SubagentToolCall {
                    agent_id: "sa1".into(),
                    tool: "submit_result".into(),
                    call_id: Some("cid-submit".into()),
                    args: Some(json!({"status": "completed"})),
                },
            ),
            Locale::EnUs,
            None,
        );
        apply_event_to_doc(
            &mut doc,
            &env(
                4,
                Some("cid-run"),
                AgentEvent::SubagentFinished {
                    agent_id: "sa1".into(),
                    status: "completed".into(),
                    summary: "done".into(),
                },
            ),
            Locale::EnUs,
            None,
        );

        let TurnPart::Tool { children, .. } = &doc.turns[0].parts[0] else {
            panic!("expected parent tool");
        };
        let TurnPart::Subagent { parts, .. } = &children[0] else {
            panic!("expected subagent");
        };
        assert!(matches!(
            &parts[0],
            TurnPart::Tool { id, state, .. } if id == "cid-submit" && *state == ToolState::Success
        ));
    }
}
