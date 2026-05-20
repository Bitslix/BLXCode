use crate::agent_wire::{AgentTask, TaskSnapshot, TaskStatus};
use leptos::prelude::*;
use std::collections::BTreeMap;

#[component]
pub fn TaskSection(
    snapshot: RwSignal<TaskSnapshot>,
    busy: RwSignal<bool>,
    tasks_open: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <section class="agent-section agent-section--tasks" aria-labelledby="agent-tasks-title">
            <button
                type="button"
                class="agent-section__head agent-section__head--toggle"
                aria-expanded=move || tasks_open.get().to_string()
                aria-controls="agent-task-list"
                on:click=move |_| tasks_open.update(|open| *open = !*open)
            >
                <h3 id="agent-tasks-title">"Tasks"</h3>
                <span>
                    {move || if busy.get() { "Running" } else { "Idle" }}
                    <span class="agent-section__chev" aria-hidden="true">
                        {move || if tasks_open.get() { "⌃" } else { "⌄" }}
                    </span>
                </span>
            </button>
            <Show when=move || tasks_open.get()>
                <div id="agent-task-list" class="agent-task-list">
                    {move || {
                        let snapshot = snapshot.get();
                        if snapshot.tasks.is_empty() {
                            return view! {
                                <ol class="agent-task-list__group">
                                    <li class="agent-task agent-task--empty">
                                        <div>
                                            <strong>"No tracked tasks yet"</strong>
                                            <small>"Complex work will appear here once the agent starts planning and updating tasks."</small>
                                        </div>
                                    </li>
                                </ol>
                            }
                            .into_any();
                        }
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
                                        let title = format!("Plan: {plan_path}");
                                        view! {
                                            <div class="agent-task-list__group-block">
                                                <h4 class="agent-task-list__group-title">{title}</h4>
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
                                            <h4 class="agent-task-list__group-title">"Free Tasks"</h4>
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
    }
}

#[component]
fn TaskRow(task: AgentTask, active: bool) -> impl IntoView {
    let meta = task_meta(&task);
    view! {
        <li class="agent-task" class:agent-task--active=active>
            <span class="agent-task__mark" aria-hidden="true"></span>
            <div class="agent-task__body">
                <div class="agent-task__topline">
                    <strong>{task.title}</strong>
                    <span class=format!("agent-task__status agent-task__status--{}", status_class(&task.status))>
                        {status_label(&task.status)}
                    </span>
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

fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "Pending",
        TaskStatus::InProgress => "In progress",
        TaskStatus::Blocked => "Blocked",
        TaskStatus::Completed => "Completed",
        TaskStatus::Cancelled => "Cancelled",
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
