use crate::agent_wire::{AgentTask, TaskSnapshot, TaskStatus};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::agent_panel::session_stats::session_started_from_timeline;
use crate::workbench::agent_timeline::TimelineDoc;
use crate::workbench::WorkbenchService;
use icondata::Icon;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::BTreeMap;
use std::time::Duration;

/// Circumference of the donut circle (`r = 6` → `2·π·6`). Used for the
/// `stroke-dasharray`/`stroke-dashoffset` progress trick.
const DONUT_CIRCUMFERENCE: f64 = 37.699;

#[component]
pub fn TaskSection(
    snapshot: RwSignal<TaskSnapshot>,
    busy: RwSignal<bool>,
    tasks_open: RwSignal<bool>,
    timeline: RwSignal<TimelineDoc>,
    wb: WorkbenchService,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    // (total, done) — drives the count chips and the donut fill.
    let counts = Memo::new(move |_| {
        let snap = snapshot.get();
        let total = snap.tasks.len();
        let done = snap
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        (total, done)
    });

    // Title of the task the agent is currently focused on, if any.
    let active_desc = Memo::new(move |_| {
        let snap = snapshot.get();
        let active = snap.active_task_id.clone();
        snap.tasks
            .iter()
            .find(|t| Some(t.id.as_str()) == active.as_deref())
            .or_else(|| {
                snap.tasks
                    .iter()
                    .find(|t| matches!(t.status, TaskStatus::InProgress))
            })
            .or_else(|| {
                snap.tasks
                    .iter()
                    .find(|t| matches!(t.status, TaskStatus::Pending))
            })
            .map(|t| t.title.clone())
    });

    // Session runtime: tick once per second and diff against the session start.
    let now_ms = RwSignal::new(js_sys::Date::now());
    if let Ok(handle) = set_interval_with_handle(
        move || now_ms.set(js_sys::Date::now()),
        Duration::from_secs(1),
    ) {
        on_cleanup(move || handle.clear());
    }
    let usage = Memo::new(move |_| {
        wb.active_id()
            .get()
            .map(|id| wb.chat_usage_for_workspace(id))
            .unwrap_or_default()
    });
    let started = Signal::derive(move || {
        usage
            .get()
            .session_started_at
            .or_else(|| timeline.with(session_started_from_timeline))
    });
    let runtime_label = Signal::derive(move || match started.get() {
        Some(start) => fmt_runtime((now_ms.get() - start).max(0.0)),
        None => "00:00".to_string(),
    });

    let donut_offset = move || {
        let (total, done) = counts.get();
        let pct = if total == 0 {
            0.0
        } else {
            (done as f64 / total as f64).clamp(0.0, 1.0)
        };
        (DONUT_CIRCUMFERENCE * (1.0 - pct)).to_string()
    };
    let count_text = move || {
        let (total, done) = counts.get();
        format!("{done}/{total}")
    };

    view! {
        <Show when=move || { counts.get().0 > 0 }>
            <section class="agent-section agent-section--tasks" aria-labelledby="agent-tasks-title">
            <div class="agent-tasks-bar">
                <button
                    type="button"
                    class="agent-tasks-bar__summary"
                    id="agent-tasks-title"
                    aria-expanded=move || tasks_open.get().to_string()
                    aria-controls="agent-task-list"
                    aria-label=move || {
                        if tasks_open.get() {
                            i18n.tr(I18nKey::AgTasksCollapse)()
                        } else {
                            i18n.tr(I18nKey::AgTasksExpand)()
                        }
                    }
                    on:click=move |_| tasks_open.update(|open| *open = !*open)
                >
                    <span class="agent-tasks-bar__count">{count_text}</span>
                    <span class="agent-tasks-bar__desc">
                        {move || {
                            active_desc
                                .get()
                                .unwrap_or_else(|| i18n.tr(I18nKey::AgTasksEmpty)().to_string())
                        }}
                    </span>
                    <span class="agent-tasks-bar__chev" aria-hidden="true">
                        {move || if tasks_open.get() { "⌃" } else { "⌄" }}
                    </span>
                </button>

                <div class="agent-tasks-bar__progress">
                    <svg
                        class="agent-tasks-bar__donut"
                        viewBox="0 0 16 16"
                        width="16"
                        height="16"
                        aria-hidden="true"
                    >
                        <circle class="agent-tasks-bar__donut-track" cx="8" cy="8" r="6" />
                        <circle
                            class="agent-tasks-bar__donut-fill"
                            cx="8"
                            cy="8"
                            r="6"
                            transform="rotate(-90 8 8)"
                            stroke-dasharray=DONUT_CIRCUMFERENCE.to_string()
                            stroke-dashoffset=donut_offset
                        />
                    </svg>
                    <span class="agent-tasks-bar__progress-label">
                        {move || i18n.tr(I18nKey::AgTasksTitle)()}
                        " "
                        {count_text}
                    </span>
                    <span class=move || {
                        if busy.get() {
                            "agent-tasks-bar__runtime agent-tasks-bar__runtime--live"
                        } else {
                            "agent-tasks-bar__runtime"
                        }
                    }>
                        {move || i18n.tr(I18nKey::AgTasksRuntime)()}
                        " "
                        {move || runtime_label.get()}
                    </span>
                </div>
            </div>

            <Show when=move || tasks_open.get()>
                <div id="agent-task-list" class="agent-task-list">
                    {move || {
                        let snapshot = snapshot.get();
                        let mut by_plan: BTreeMap<String, Vec<AgentTask>> = BTreeMap::new();
                        let mut free_tasks: Vec<AgentTask> = Vec::new();
                        for task in snapshot.tasks {
                            if let Some(path) = task.plan_path.clone() {
                                by_plan.entry(path).or_default().push(task);
                            } else {
                                free_tasks.push(task);
                            }
                        }
                        let active = snapshot.active_task_id.clone();
                        view! {
                            <>
                                {by_plan
                                    .into_iter()
                                    .map(|(plan_path, tasks)| {
                                        let title = format!(
                                            "{}: {plan_path}",
                                            i18n.tr(I18nKey::AgTasksGroupPlan)(),
                                        );
                                        view! {
                                            <div class="agent-task-list__group-block">
                                                <TaskGroupTitle icon=icondata::LuClipboardList label=title />
                                                <ol class="agent-task-list__group">
                                                    {tasks
                                                        .into_iter()
                                                        .map(|task| {
                                                            let is_active = active.as_deref() == Some(task.id.as_str())
                                                                || matches!(task.status, TaskStatus::InProgress);
                                                            view! { <TaskRow task=task active=is_active /> }
                                                        })
                                                        .collect_view()}
                                                </ol>
                                            </div>
                                        }
                                    })
                                    .collect_view()}
                                {if free_tasks.is_empty() {
                                    ().into_any()
                                } else {
                                    let active = active.clone();
                                    view! {
                                        <div class="agent-task-list__group-block">
                                            <TaskGroupTitle
                                                icon=icondata::LuListTodo
                                                label=i18n.tr(I18nKey::AgTasksGroupFree)().to_string()
                                            />
                                            <ol class="agent-task-list__group">
                                                {free_tasks
                                                    .into_iter()
                                                    .map(|task| {
                                                        let is_active = active.as_deref() == Some(task.id.as_str())
                                                            || matches!(task.status, TaskStatus::InProgress);
                                                        view! { <TaskRow task=task active=is_active /> }
                                                    })
                                                    .collect_view()}
                                            </ol>
                                        </div>
                                    }
                                    .into_any()
                                }}
                            </>
                        }
                        .into_any()
                    }}
                </div>
            </Show>
            </section>
        </Show>
    }
}

#[component]
fn TaskGroupTitle(icon: Icon, label: String) -> impl IntoView {
    view! {
        <h4 class="agent-task-list__group-title">
            <span class="agent-task-list__group-icon" aria-hidden="true">
                <LxIcon icon=icon width="13px" height="13px" />
            </span>
            {label}
        </h4>
    }
}

#[component]
fn TaskRow(task: AgentTask, active: bool) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let meta = task_meta(&task);
    let status_cls = format!(
        "agent-task__status agent-task__status--{}",
        status_class(&task.status)
    );
    let status_text = i18n.tr(status_key(&task.status))().to_string();
    view! {
        <li class="agent-task" class:agent-task--active=active>
            <span class="agent-task__mark" aria-hidden="true"></span>
            <div class="agent-task__body">
                <div class="agent-task__topline">
                    <strong>{task.title}</strong>
                    <span class=status_cls>{status_text}</span>
                </div>
                {if meta.is_empty() {
                    ().into_any()
                } else {
                    view! { <small>{meta.clone()}</small> }.into_any()
                }}
            </div>
        </li>
    }
}

fn status_key(status: &TaskStatus) -> I18nKey {
    match status {
        TaskStatus::Pending => I18nKey::PlansTaskStatPending,
        TaskStatus::InProgress => I18nKey::PlansTaskStatInProgress,
        TaskStatus::Blocked => I18nKey::PlansTaskStatBlocked,
        TaskStatus::Completed => I18nKey::PlansTaskStatCompleted,
        TaskStatus::Cancelled => I18nKey::PlansTaskStatCancelled,
    }
}

fn status_class(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::InProgress => "in-progress",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Completed => "completed",
        TaskStatus::Cancelled => "cancelled",
    }
}

/// Format a runtime in milliseconds as `MM:SS`, or `HH:MM:SS` past one hour.
fn fmt_runtime(ms: f64) -> String {
    let total_secs = (ms / 1000.0) as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    if hours > 0 {
        format!("{hours:02}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    }
}

fn task_meta(task: &AgentTask) -> String {
    let mut parts = Vec::new();
    let description = task.description.trim();
    if !description.is_empty() {
        parts.push(description.to_owned());
    }
    if let Some(notes) = task.notes.as_deref() {
        let notes = notes.trim();
        if !notes.is_empty() {
            parts.push(notes.to_owned());
        }
    }
    if let Some(parent_id) = task.parent_id.as_deref() {
        parts.push(format!("Parent: {parent_id}"));
    }
    parts.join(" · ")
}
