//! Client-side aggregation of chat-session statistics from the timeline.
//!
//! Everything the session-stats panel shows that is *not* already in
//! [`ChatUsageStats`](crate::workbench::state::ChatUsageStats) is derived here
//! by walking the persisted [`TimelineDoc`]: user turns, model rounds, the
//! total tool-call count + per-category buckets (open / read / edit / rm) and
//! the list of currently-running subagents.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::agent_panel::context_meter::{fmt_tokens, occupancy_pct};
use crate::workbench::agent_panel::turn_metrics_bar::fmt_cost;
use crate::workbench::agent_timeline::{SubagentStatus, TimelineDoc, TurnPart};
use crate::workbench::state::{HarnessSettingsCategory, HarnessUiService};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsValue;

/// A subagent that is still running (rendered in the subagent stats row).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveSubagent {
    pub display_name: String,
    pub role: String,
}

/// Aggregated, display-ready session counters derived from the timeline.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionStats {
    /// User submissions (one per `TurnNode`).
    pub user_turns: u32,
    /// Main-agent model rounds (subagent rounds excluded).
    pub model_turns: u32,
    /// Total tool calls (main agent + subagents, counting merged groups).
    pub tool_total: u32,
    pub open: u32,
    pub read: u32,
    pub edit: u32,
    pub rm: u32,
    /// Subagents whose status is still `Running`.
    pub active_subagents: Vec<ActiveSubagent>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToolCategory {
    Open,
    Read,
    Edit,
    Rm,
}

/// Heuristic bucketing of a tool name. Tools without a category still count
/// toward `tool_total`. Keep in sync as new tools are added.
fn tool_category(tool: &str) -> Option<ToolCategory> {
    use ToolCategory::{Edit, Open, Read, Rm};
    match tool {
        "read_workspace_file"
        | "memory_read"
        | "memory_search"
        | "memory_list"
        | "memory_backlinks"
        | "memory_graph"
        | "memory_context_list"
        | "memory_category_list"
        | "rules_read"
        | "skills_read"
        | "task_get"
        | "task_list"
        | "list_tools"
        | "harness.read_terminal_output"
        | "harness.list_terminals" => Some(Read),

        "list_workspace_files"
        | "harness.open_terminal"
        | "harness.create_workspace"
        | "memory_context_attach" => Some(Open),

        "memory_create"
        | "memory_write"
        | "memory_rename"
        | "memory_category_update"
        | "memory_context_detach"
        | "task_create"
        | "task_update"
        | "task_reorder"
        | "harness.send_terminal_keys"
        | "harness.send_agent_context" => Some(Edit),

        "memory_delete" | "task_delete" => Some(Rm),

        _ => None,
    }
}

/// Derive the [`SessionStats`] for a workspace's timeline.
#[must_use]
pub fn compute_session_stats(doc: &TimelineDoc) -> SessionStats {
    let mut stats = SessionStats {
        user_turns: doc.turns.len() as u32,
        ..SessionStats::default()
    };
    for turn in &doc.turns {
        walk_parts(&turn.parts, &mut stats, false);
    }
    stats
}

/// Persisted timestamp of the first user turn in the timeline, when available.
#[must_use]
pub fn session_started_from_timeline(doc: &TimelineDoc) -> Option<f64> {
    doc.turns.first().and_then(|turn| turn.user.created_at)
}

/// Recursively account a list of parts. `in_subagent` suppresses counting
/// subagent model rounds against the main-agent `model_turns`.
fn walk_parts(parts: &[TurnPart], stats: &mut SessionStats, in_subagent: bool) {
    for part in parts {
        match part {
            TurnPart::ModelRound { .. } => {
                if !in_subagent {
                    stats.model_turns = stats.model_turns.saturating_add(1);
                }
            }
            TurnPart::Tool {
                tool,
                children,
                merged_count,
                ..
            } => {
                let n = (*merged_count).max(1) as u32;
                stats.tool_total = stats.tool_total.saturating_add(n);
                match tool_category(tool) {
                    Some(ToolCategory::Open) => stats.open = stats.open.saturating_add(n),
                    Some(ToolCategory::Read) => stats.read = stats.read.saturating_add(n),
                    Some(ToolCategory::Edit) => stats.edit = stats.edit.saturating_add(n),
                    Some(ToolCategory::Rm) => stats.rm = stats.rm.saturating_add(n),
                    None => {}
                }
                walk_parts(children, stats, in_subagent);
            }
            TurnPart::Subagent {
                display_name,
                role,
                status,
                parts,
                ..
            } => {
                if *status == SubagentStatus::Running {
                    stats.active_subagents.push(ActiveSubagent {
                        display_name: display_name.clone(),
                        role: role.clone(),
                    });
                }
                walk_parts(parts, stats, true);
            }
            _ => {}
        }
    }
}

#[component]
pub fn AgentSessionStats(
    timeline: RwSignal<TimelineDoc>,
    wb: WorkbenchService,
    context_length: RwSignal<Option<u64>>,
    model_label: RwSignal<String>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ui = expect_context::<HarnessUiService>();
    let stats = Memo::new(move |_| compute_session_stats(&timeline.get()));
    let usage = Memo::new(move |_| {
        wb.active_id()
            .get()
            .map(|id| wb.chat_usage_for_workspace(id))
            .unwrap_or_default()
    });

    let model_text = Signal::derive(move || {
        let label = model_label.get();
        if label.trim().is_empty() {
            i18n.tr(I18nKey::AgStatsModelUnknown)().to_string()
        } else {
            label
        }
    });
    let started_text = Signal::derive(move || {
        let timeline_started_at = timeline.with(session_started_from_timeline);
        usage
            .get()
            .session_started_at
            .or(timeline_started_at)
            .map(format_session_time)
            .unwrap_or_else(|| i18n.tr(I18nKey::AgStatsEmpty)().to_string())
    });
    let context_text = Signal::derive(move || {
        let used = usage.get().last_round_input_tokens;
        format_context_value(used, context_length.get())
    });
    let cost_text = Signal::derive(move || fmt_cost(usage.get().total_cost_usd));
    let state_label = Signal::derive(move || {
        if busy.get() {
            i18n.tr(I18nKey::AgStateRunning)().to_string()
        } else {
            i18n.tr(I18nKey::AgStateStandby)().to_string()
        }
    });

    view! {
        <section class="agent-session-stats" aria-label=move || i18n.tr(I18nKey::AgStatsAria)()>
            <button
                type="button"
                class="agent-session-stats__model"
                aria-label=move || i18n.tr(I18nKey::AgStatsModelTip)()
                on:click=move |_| {
                    ui.settings_category().set(HarnessSettingsCategory::AgentProvider);
                    wb.open_center_settings_tab(HarnessSettingsCategory::AgentProvider);
                }
            >
                <span class="agent-session-stats__model-icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuCpu width="0.88rem" height="0.88rem" />
                </span>
                <span class="agent-session-stats__model-text">{move || model_text.get()}</span>
                <span class=move || {
                    if busy.get() {
                        "agent-session-stats__state agent-session-stats__state--live"
                    } else {
                        "agent-session-stats__state"
                    }
                }>
                    {move || state_label.get()}
                </span>
            </button>

            <div class="agent-session-stats__rows">
                <StatsRow
                    icon=icondata::LuClock
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsStarted)().to_string())
                    value=started_text
                />
                <ContextStatsRow
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsContext)().to_string())
                    value=context_text
                    used=Signal::derive(move || usage.get().last_round_input_tokens)
                    max=Signal::derive(move || context_length.get())
                />
                <StatsRow
                    icon=icondata::LuUser
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsUserTurns)().to_string())
                    value=Signal::derive(move || stats.get().user_turns.to_string())
                />
                <StatsRow
                    icon=icondata::LuBot
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsModelTurns)().to_string())
                    value=Signal::derive(move || stats.get().model_turns.to_string())
                />
                <ToolStatsRow
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsToolCalls)().to_string())
                    value=Signal::derive(move || stats.get().tool_total.to_string())
                    stats=stats
                    i18n=i18n
                />
                <SubagentsStatsRow
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsSubagents)().to_string())
                    count=Signal::derive(move || stats.get().active_subagents.len())
                    agents=stats
                />
                <StatsRow
                    icon=icondata::LuCircleDollarSign
                    label=Signal::derive(move || i18n.tr(I18nKey::AgStatsCosts)().to_string())
                    value=cost_text
                />
            </div>
        </section>
    }
}

#[component]
fn StatsRow(icon: icondata::Icon, label: Signal<String>, value: Signal<String>) -> impl IntoView {
    view! {
        <div class="agent-session-stats__row">
            <span class="agent-session-stats__icon" aria-hidden="true">
                <LxIcon icon=icon width="0.78rem" height="0.78rem" />
            </span>
            <span class="agent-session-stats__label">{move || label.get()}</span>
            <span class="agent-session-stats__value">{move || value.get()}</span>
        </div>
    }
}

#[component]
fn ContextStatsRow(
    label: Signal<String>,
    value: Signal<String>,
    used: Signal<u64>,
    max: Signal<Option<u64>>,
) -> impl IntoView {
    let fill_pct = move || {
        max.get()
            .filter(|m| *m > 0)
            .map(|m| occupancy_pct(used.get(), m).min(100))
            .unwrap_or(0)
    };
    let level_class = move || {
        let pct = max
            .get()
            .filter(|m| *m > 0)
            .map(|m| occupancy_pct(used.get(), m))
            .unwrap_or(0);
        if pct >= 85 {
            "agent-session-stats__meter-fill agent-session-stats__meter-fill--danger"
        } else if pct >= 70 {
            "agent-session-stats__meter-fill agent-session-stats__meter-fill--warn"
        } else {
            "agent-session-stats__meter-fill"
        }
    };

    view! {
        <div class="agent-session-stats__row agent-session-stats__row--context">
            <span class="agent-session-stats__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuGauge width="0.78rem" height="0.78rem" />
            </span>
            <span class="agent-session-stats__label">{move || label.get()}</span>
            <span class="agent-session-stats__value agent-session-stats__value--context">
                <span>{move || value.get()}</span>
                <span class="agent-session-stats__meter" aria-hidden="true">
                    <span class=level_class style=move || format!("width:{}%", fill_pct())></span>
                </span>
            </span>
        </div>
    }
}

#[component]
fn ToolStatsRow(
    label: Signal<String>,
    value: Signal<String>,
    stats: Memo<SessionStats>,
    i18n: I18nService,
) -> impl IntoView {
    view! {
        <div class="agent-session-stats__row agent-session-stats__row--tools">
            <span class="agent-session-stats__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuWrench width="0.78rem" height="0.78rem" />
            </span>
            <span class="agent-session-stats__label">{move || label.get()}</span>
            <span class="agent-session-stats__value agent-session-stats__value--tools">
                <strong>{move || value.get()}</strong>
                <span class="agent-session-stats__badges" aria-hidden="true">
                    {move || {
                        let s = stats.get();
                        view! {
                            <span>{s.open} <em>{i18n.tr(I18nKey::AgStatsOpen)()}</em></span>
                            <span>{s.read} <em>{i18n.tr(I18nKey::AgStatsRead)()}</em></span>
                            <span>{s.edit} <em>{i18n.tr(I18nKey::AgStatsEdit)()}</em></span>
                            <span>{s.rm} <em>{i18n.tr(I18nKey::AgStatsRm)()}</em></span>
                        }
                    }}
                </span>
            </span>
        </div>
    }
}

#[component]
fn SubagentsStatsRow(
    label: Signal<String>,
    count: Signal<usize>,
    agents: Memo<SessionStats>,
) -> impl IntoView {
    view! {
        <div class="agent-session-stats__row agent-session-stats__row--subagents">
            <span class="agent-session-stats__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuUsers width="0.78rem" height="0.78rem" />
            </span>
            <span class="agent-session-stats__label">{move || label.get()}</span>
            <span class="agent-session-stats__value agent-session-stats__value--subagents">
                <strong>{move || count.get().to_string()}</strong>
                <Show when=move || { count.get() > 0 }>
                    <span class="agent-session-stats__subagent-chips" aria-hidden="true">
                        {move || {
                            agents.get().active_subagents.into_iter().map(|agent| {
                                view! {
                                    <span class="agent-session-stats__subagent-chip">
                                        {agent.display_name}
                                    </span>
                                }
                            }).collect_view()
                        }}
                    </span>
                </Show>
            </span>
        </div>
    }
}

fn format_context_value(used: u64, max: Option<u64>) -> String {
    match max.filter(|m| *m > 0) {
        Some(max) => format!(
            "{} / {} · {}%",
            fmt_tokens(used),
            fmt_tokens(max),
            occupancy_pct(used, max)
        ),
        None => format!("{} tok", fmt_tokens(used)),
    }
}

fn format_session_time(epoch_ms: f64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(epoch_ms));
    format!("{:02}:{:02}", date.get_hours(), date.get_minutes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_wire::TurnMetrics;
    use crate::workbench::agent_timeline::{ToolState, TurnNode, UserPart};

    fn user_turn(parts: Vec<TurnPart>) -> TurnNode {
        user_turn_started_at(parts, Some(42.0))
    }

    fn user_turn_started_at(parts: Vec<TurnPart>, created_at: Option<f64>) -> TurnNode {
        TurnNode {
            id: "t".into(),
            user: UserPart {
                id: "u".into(),
                text: "hi".into(),
                created_at,
            },
            parts,
        }
    }

    fn tool(name: &str) -> TurnPart {
        tool_n(name, 1)
    }

    fn tool_n(name: &str, merged_count: usize) -> TurnPart {
        TurnPart::Tool {
            id: format!("tool-{name}"),
            tool: name.into(),
            label: name.into(),
            args_summary: String::new(),
            args: None,
            state: ToolState::Success,
            result: None,
            metrics: TurnMetrics::default(),
            children: Vec::new(),
            paths: Vec::new(),
            merged_count,
        }
    }

    fn model_round() -> TurnPart {
        TurnPart::ModelRound {
            id: "mr".into(),
            metrics: TurnMetrics::default(),
        }
    }

    fn doc(turns: Vec<TurnNode>) -> TimelineDoc {
        TimelineDoc {
            turns,
            ..TimelineDoc::default()
        }
    }

    #[test]
    fn counts_user_and_model_turns() {
        let d = doc(vec![
            user_turn(vec![model_round(), model_round()]),
            user_turn(vec![model_round()]),
        ]);
        let s = compute_session_stats(&d);
        assert_eq!(s.user_turns, 2);
        assert_eq!(s.model_turns, 3);
    }

    #[test]
    fn session_start_uses_first_user_turn_timestamp() {
        let d = doc(vec![
            user_turn_started_at(Vec::new(), Some(100.0)),
            user_turn_started_at(Vec::new(), Some(200.0)),
        ]);

        assert_eq!(session_started_from_timeline(&d), Some(100.0));
    }

    #[test]
    fn session_start_does_not_skip_untimed_first_turn() {
        let d = doc(vec![
            user_turn_started_at(Vec::new(), None),
            user_turn_started_at(Vec::new(), Some(200.0)),
        ]);

        assert_eq!(session_started_from_timeline(&d), None);
    }

    #[test]
    fn buckets_tool_calls_by_category() {
        let d = doc(vec![user_turn(vec![
            model_round(),
            tool("read_workspace_file"),
            tool("list_workspace_files"),
            tool("memory_write"),
            tool("memory_delete"),
            tool("some_unknown_tool"),
        ])]);
        let s = compute_session_stats(&d);
        assert_eq!(s.tool_total, 5);
        assert_eq!(s.read, 1);
        assert_eq!(s.open, 1);
        assert_eq!(s.edit, 1);
        assert_eq!(s.rm, 1);
    }

    #[test]
    fn merged_count_inflates_totals() {
        let d = doc(vec![user_turn(vec![tool_n("read_workspace_file", 4)])]);
        let s = compute_session_stats(&d);
        assert_eq!(s.tool_total, 4);
        assert_eq!(s.read, 4);
    }

    #[test]
    fn subagent_rounds_excluded_tools_and_active_collected() {
        let sub = TurnPart::Subagent {
            id: "s1".into(),
            role: "explorer".into(),
            display_name: "Scout".into(),
            status: SubagentStatus::Running,
            parts: vec![model_round(), tool("read_workspace_file")],
            metrics: TurnMetrics::default(),
            summary: None,
            steps: Vec::new(),
        };
        let d = doc(vec![user_turn(vec![model_round(), sub])]);
        let s = compute_session_stats(&d);
        // main model round counts, subagent's does not
        assert_eq!(s.model_turns, 1);
        // subagent tool still counts toward totals
        assert_eq!(s.tool_total, 1);
        assert_eq!(s.read, 1);
        assert_eq!(s.active_subagents.len(), 1);
        assert_eq!(s.active_subagents[0].display_name, "Scout");
    }

    #[test]
    fn finished_subagent_not_active() {
        let sub = TurnPart::Subagent {
            id: "s1".into(),
            role: "explorer".into(),
            display_name: "Scout".into(),
            status: SubagentStatus::Done,
            parts: Vec::new(),
            metrics: TurnMetrics::default(),
            summary: None,
            steps: Vec::new(),
        };
        let d = doc(vec![user_turn(vec![sub])]);
        let s = compute_session_stats(&d);
        assert!(s.active_subagents.is_empty());
    }
}
