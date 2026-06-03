use crate::agent_wire::TaskStatus;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    clipboard_read_text_compat, clipboard_write_text_compat, kanban_board_load,
    kanban_export_layout, kanban_import_layout, kanban_layout_save, kanban_task_create,
    kanban_task_delete, kanban_task_update, KanbanBoard, KanbanPlanNode, KanbanPlanState,
    KanbanTaskCreateInput, KanbanTaskUpdatePatch,
};
use crate::workbench::toast::{ToastKind, ToastService};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn WorkspaceKanban(workspace_id: u64) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();
    let board = RwSignal::<Option<KanbanBoard>>::new(None);
    let loading = RwSignal::new(false);
    let error = RwSignal::<Option<String>>::new(None);
    let query = RwSignal::new(String::new());
    let expanded_plans = RwSignal::new(Vec::<String>::new());
    let open_sections = RwSignal::new(Vec::<KanbanPlanState>::new());
    let open_sections_workspace = RwSignal::<Option<String>>::new(None);
    let dragged_task = RwSignal::<Option<(String, String)>>::new(None);
    let new_task_plan = RwSignal::new(String::new());
    let new_task_title = RwSignal::new(String::new());
    let new_task_status = RwSignal::new(TaskStatus::Pending);

    let workspace_cwd = Signal::derive(move || {
        wb.workspaces().with(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.cwd.clone())
                .filter(|cwd| !cwd.trim().is_empty())
        })
    });

    let load_board = move || {
        let Some(ws) = workspace_cwd.get_untracked() else {
            board.set(None);
            return;
        };
        loading.set(true);
        spawn_local(async move {
            match kanban_board_load(&ws).await {
                Ok(next) => {
                    if open_sections_workspace.with_untracked(|current| current.as_deref() != Some(&ws)) {
                        expanded_plans.set(Vec::new());
                        open_sections.set(read_open_plan_sections(&ws));
                        open_sections_workspace.set(Some(ws.clone()));
                    }
                    if new_task_plan.with_untracked(|s| s.trim().is_empty()) {
                        if let Some(first) = next.plans.first() {
                            new_task_plan.set(first.meta.path.clone());
                        }
                    }
                    board.set(Some(next));
                    error.set(None);
                }
                Err(err) => error.set(Some(err)),
            }
            loading.set(false);
        });
    };

    Effect::new(move |_| {
        let _ = workspace_cwd.get();
        let _ = wb.plans_epoch().get();
        load_board();
    });

    let filtered_plans = Signal::derive(move || {
        let q = query.get().trim().to_lowercase();
        board
            .get()
            .map(|b| {
                b.plans
                    .into_iter()
                    .filter(|plan| {
                        q.is_empty()
                            || plan.meta.title.to_lowercase().contains(&q)
                            || plan.meta.path.to_lowercase().contains(&q)
                            || plan
                                .tasks
                                .iter()
                                .any(|task| task.title.to_lowercase().contains(&q))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

    let create_task = Callback::new(move |()| {
        let Some(ws) = workspace_cwd.get_untracked() else {
            return;
        };
        let plan_path = new_task_plan.get_untracked();
        let title = new_task_title.get_untracked();
        if plan_path.trim().is_empty() || title.trim().is_empty() {
            toast.error(i18n.tr(I18nKey::KanbanPickPlanAndTitle)());
            return;
        }
        let status = new_task_status.get_untracked();
        let progress = toast.loading(i18n.tr(I18nKey::KanbanAddingTask)());
        spawn_local(async move {
            let result = kanban_task_create(
                &ws,
                KanbanTaskCreateInput {
                    plan_path,
                    title,
                    status: Some(status),
                },
            )
            .await;
            match result {
                Ok(_) => {
                    new_task_title.set(String::new());
                    toast.resolve(
                        progress,
                        ToastKind::Success,
                        i18n.tr(I18nKey::KanbanTaskAdded)(),
                    );
                    wb.bump_plans_epoch();
                }
                Err(err) => toast.resolve(progress, ToastKind::Error, err),
            }
        });
    });

    let export_layout = move |_| {
        let Some(ws) = workspace_cwd.get_untracked() else {
            return;
        };
        spawn_local(async move {
            match kanban_export_layout(&ws).await {
                Ok(json) => match clipboard_write_text_compat(json).await {
                    Ok(()) => toast.success(i18n.tr(I18nKey::KanbanLayoutCopied)()),
                    Err(err) => toast.error(format!("Export failed: {err}")),
                },
                Err(err) => toast.error(format!("Export failed: {err}")),
            }
        });
    };

    let import_layout = move |_| {
        let Some(ws) = workspace_cwd.get_untracked() else {
            return;
        };
        spawn_local(async move {
            match clipboard_read_text_compat().await {
                Ok(json) => match kanban_import_layout(&ws, &json).await {
                    Ok(_) => {
                        toast.success(i18n.tr(I18nKey::KanbanLayoutImported)());
                        load_board();
                    }
                    Err(err) => toast.error(format!("Import failed: {err}")),
                },
                Err(err) => toast.error(format!("Import failed: {err}")),
            }
        });
    };

    let save_expanded_plans = Callback::new(move |expanded: Vec<String>| {
        let Some(ws) = workspace_cwd.get_untracked() else {
            return;
        };
        let Some(mut current) = board.get_untracked().map(|b| b.layout) else {
            return;
        };
        current.expanded_plans = expanded;
        spawn_local(async move {
            if let Err(err) = kanban_layout_save(&ws, current).await {
                toast.error(format!(
                    "{}: {err}",
                    i18n.tr(I18nKey::KanbanLayoutSaveFailed)()
                ));
            }
        });
    });

    let save_open_sections = Callback::new(move |sections: Vec<KanbanPlanState>| {
        if let Some(ws) = workspace_cwd.get_untracked() {
            write_open_plan_sections(&ws, &sections);
        }
    });

    view! {
        <div class="workspace-kanban" role="region" aria-label=move || i18n.tr(I18nKey::KanbanTitle)()>
            <header class="workspace-kanban__toolbar">
                <div class="workspace-kanban__title">
                    <LxIcon icon=icondata::LuKanban width="1rem" height="1rem" />
                    <span>{move || i18n.tr(I18nKey::KanbanTitle)()}</span>
                </div>
                <label class="workspace-kanban__search">
                    <LxIcon icon=icondata::LuSearch width="0.9rem" height="0.9rem" />
                    <input
                        type="search"
                        placeholder=move || i18n.tr(I18nKey::KanbanSearchPh)()
                        prop:value=move || query.get()
                        on:input=move |ev| query.set(input_value(&ev))
                    />
                </label>
                <button type="button" class="workspace-kanban__btn" on:click=move |_| load_board()>
                    <LxIcon icon=icondata::LuRefreshCw width="0.9rem" height="0.9rem" />
                    <span>{move || i18n.tr(I18nKey::SrRefresh)()}</span>
                </button>
                <button type="button" class="workspace-kanban__btn" on:click=export_layout>
                    <LxIcon icon=icondata::LuUpload width="0.9rem" height="0.9rem" />
                    <span>{move || i18n.tr(I18nKey::KanbanExport)()}</span>
                </button>
                <button type="button" class="workspace-kanban__btn" on:click=import_layout>
                    <LxIcon icon=icondata::LuDownload width="0.9rem" height="0.9rem" />
                    <span>{move || i18n.tr(I18nKey::KanbanImport)()}</span>
                </button>
            </header>

            {move || error.get().map(|err| view! {
                <p class="workspace-kanban__error">{err}</p>
            })}

            <Show
                when=move || workspace_cwd.get().is_some()
                fallback=move || view! {
                    <div class="workspace-kanban__empty">
                        <LxIcon icon=icondata::LuKanban width="1.4rem" height="1.4rem" />
                        <span>{move || i18n.tr(I18nKey::SrNoWorkspace)()}</span>
                    </div>
                }
            >
                <section class="workspace-kanban__quickadd">
                    <div class="workspace-kanban-field workspace-kanban-field--plan">
                        <span class="workspace-kanban-field__label">
                            <LxIcon icon=icondata::LuClipboardList width="0.72rem" height="0.72rem" />
                            <span>{move || i18n.tr(I18nKey::KanbanPlanLabel)()}</span>
                        </span>
                        <KanbanPlanPicker
                            plans=Signal::derive(move || board.get().map(|b| b.plans).unwrap_or_default())
                            selected_path=new_task_plan
                            on_select=Callback::new(move |path| new_task_plan.set(path))
                        />
                    </div>
                    <div class="workspace-kanban-field workspace-kanban-field--status">
                        <span class="workspace-kanban-field__label">
                            <LxIcon icon=icondata::LuCircle width="0.72rem" height="0.72rem" />
                            <span>{move || i18n.tr(I18nKey::KanbanStatusLabel)()}</span>
                        </span>
                        <KanbanTaskStatusPicker
                            selected_status=new_task_status
                            on_select=Callback::new(move |status| new_task_status.set(status))
                        />
                    </div>
                    <div class="workspace-kanban-field workspace-kanban-field--task">
                        <span class="workspace-kanban-field__label">
                            <LxIcon icon=icondata::LuTextCursor width="0.72rem" height="0.72rem" />
                            <span>{move || i18n.tr(I18nKey::KanbanTaskTitleLabel)()}</span>
                        </span>
                        <input
                            type="text"
                            placeholder=move || i18n.tr(I18nKey::KanbanNewTask)()
                            prop:value=move || new_task_title.get()
                            on:input=move |ev| new_task_title.set(input_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    create_task.run(());
                                }
                            }
                        />
                    </div>
                    <button type="button" class="workspace-kanban__btn workspace-kanban__btn--primary workspace-kanban__quickadd-submit" on:click=move |_| create_task.run(())>
                        <LxIcon icon=icondata::LuPlus width="0.9rem" height="0.9rem" />
                        <span>{move || i18n.tr(I18nKey::KanbanNewTask)()}</span>
                    </button>
                </section>

                <Show when=move || loading.get() && board.get().is_none()>
                    <p class="workspace-kanban__hint">{move || i18n.tr(I18nKey::SrLoading)()}</p>
                </Show>

                <div class="workspace-kanban__sections">
                    <For
                        each=plan_states
                        key=plan_state_key
                        children=move |state| {
                            let state_for_count = state.clone();
                            let state_for_list = state.clone();
                            view! {
                                <KanbanStateSection
                                    state=state
                                    plans=Signal::derive(move || {
                                        filtered_plans
                                            .get()
                                            .into_iter()
                                            .filter(|plan| plan.state == state_for_list)
                                            .collect::<Vec<_>>()
                                    })
                                    count=Signal::derive(move || {
                                        filtered_plans
                                            .get()
                                            .into_iter()
                                            .filter(|plan| plan.state == state_for_count)
                                            .count()
                                    })
                                    expanded_plans=expanded_plans
                                    dragged_task=dragged_task
                                    workspace_cwd=workspace_cwd
                                    on_reload=Callback::new(move |()| load_board())
                                    on_expanded_change=save_expanded_plans
                                    open_sections=open_sections
                                    on_open_sections_change=save_open_sections
                                />
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn KanbanStateSection(
    state: KanbanPlanState,
    plans: Signal<Vec<KanbanPlanNode>>,
    count: Signal<usize>,
    expanded_plans: RwSignal<Vec<String>>,
    dragged_task: RwSignal<Option<(String, String)>>,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
    on_expanded_change: Callback<Vec<String>>,
    open_sections: RwSignal<Vec<KanbanPlanState>>,
    on_open_sections_change: Callback<Vec<KanbanPlanState>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open_state = state.clone();
    let click_state = state.clone();
    let aria_state = state.clone();
    let show_state = state.clone();
    let label_state = state.clone();
    view! {
        <section
            class="workspace-kanban-section"
            class:workspace-kanban-section--open=move || open_sections.with(|items| items.contains(&open_state))
            data-state=plan_state_key(&state)
        >
            <button
                type="button"
                class="workspace-kanban-section__head"
                aria-expanded=move || open_sections.with(|items| items.contains(&aria_state)).to_string()
                on:click=move |_| {
                    let mut next = Vec::new();
                    open_sections.update(|items| {
                        if let Some(pos) = items.iter().position(|item| item == &click_state) {
                            items.remove(pos);
                        } else {
                            items.push(click_state.clone());
                        }
                        next = items.clone();
                    });
                    on_open_sections_change.run(next);
                }
            >
                <LxIcon icon=plan_state_icon(&state) width="1rem" height="1rem" />
                <span>{move || i18n.tr(plan_state_label_key(&label_state))()}</span>
                <span class="workspace-kanban-section__count">{move || count.get()}</span>
                <LxIcon
                    icon=icondata::LuChevronDown
                    width="0.9rem"
                    height="0.9rem"
                />
            </button>
            <Show when=move || open_sections.with(|items| items.contains(&show_state))>
                <div class="workspace-kanban-section__body">
                    <Show
                        when=move || !plans.get().is_empty()
                        fallback=move || view! {
                            <p class="workspace-kanban__hint">{move || i18n.tr(I18nKey::KanbanNoPlansInState)()}</p>
                        }
                    >
                        <For
                            each=move || plans.get()
                            key=|plan| plan.meta.path.clone()
                            children=move |plan| view! {
                                <KanbanPlanCard
                                    plan=plan
                                    expanded_plans=expanded_plans
                                    dragged_task=dragged_task
                                    workspace_cwd=workspace_cwd
                                    on_reload=on_reload
                                    on_expanded_change=on_expanded_change
                                />
                            }
                        />
                    </Show>
                </div>
            </Show>
        </section>
    }
}

#[component]
fn KanbanTaskStatusPicker(
    selected_status: RwSignal<TaskStatus>,
    on_select: Callback<TaskStatus>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    view! {
        <div class="workspace-kanban-status-picker">
            <button
                type="button"
                class="workspace-kanban-status-picker__button"
                data-status=move || task_status_key(&selected_status.get()).to_string()
                aria-label=move || i18n.tr(I18nKey::KanbanStatusLabel)()
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|value| *value = !*value)
                on:keydown=move |ev| {
                    if ev.key() == "Escape" {
                        open.set(false);
                    }
                }
            >
                <LxIcon icon=move || task_status_icon(&selected_status.get()) width="0.9rem" height="0.9rem" />
                <span class="workspace-kanban-status-picker__label">
                    {move || i18n.tr(task_status_label_key(&selected_status.get()))()}
                </span>
                <LxIcon icon=icondata::LuChevronDown width="0.86rem" height="0.86rem" />
            </button>
            <Show when=move || open.get()>
                <div class="workspace-kanban-status-picker__menu" role="listbox">
                    <For
                        each=task_statuses
                        key=task_status_key
                        children=move |status| {
                            let option_status = status.clone();
                            let aria_status = status.clone();
                            let label_status = status.clone();
                            let icon_status = status.clone();
                            let active_status = status.clone();
                            view! {
                                <button
                                    type="button"
                                    class="workspace-kanban-status-picker__option"
                                    class:workspace-kanban-status-picker__option--active=move || {
                                        selected_status.get() == active_status
                                    }
                                    data-status=task_status_key(&status)
                                    role="option"
                                    aria-selected=move || (selected_status.get() == aria_status).to_string()
                                    on:click=move |_| {
                                        on_select.run(option_status.clone());
                                        open.set(false);
                                    }
                                >
                                    <LxIcon icon=task_status_icon(&icon_status) width="0.9rem" height="0.9rem" />
                                    <span>{move || i18n.tr(task_status_label_key(&label_status))()}</span>
                                </button>
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn KanbanPlanPicker(
    plans: Signal<Vec<KanbanPlanNode>>,
    selected_path: RwSignal<String>,
    on_select: Callback<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);
    let selected = Signal::derive(move || {
        let selected_path = selected_path.get();
        plans
            .get()
            .into_iter()
            .find(|plan| plan.meta.path == selected_path)
    });

    view! {
        <div class="workspace-kanban-plan-picker">
            <button
                type="button"
                class="workspace-kanban-plan-picker__button"
                aria-label=move || i18n.tr(I18nKey::KanbanPlanLabel)()
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|value| *value = !*value)
                on:keydown=move |ev| {
                    if ev.key() == "Escape" {
                        open.set(false);
                    }
                }
            >
                <LxIcon icon=icondata::LuClipboardList width="0.9rem" height="0.9rem" />
                <span class="workspace-kanban-plan-picker__text">
                    <span class="workspace-kanban-plan-picker__title">
                        {move || {
                            selected
                                .get()
                                .map(|plan| plan.meta.title)
                                .unwrap_or_else(|| i18n.tr(I18nKey::KanbanSelectPlan)().to_string())
                        }}
                    </span>
                    <span class="workspace-kanban-plan-picker__description">
                        {move || {
                            selected
                                .get()
                                .map(|plan| plan_picker_description(&plan))
                                .unwrap_or_default()
                        }}
                    </span>
                </span>
                <LxIcon icon=icondata::LuChevronDown width="0.86rem" height="0.86rem" />
            </button>
            <Show when=move || open.get()>
                <div class="workspace-kanban-plan-picker__menu" role="listbox">
                    <For
                        each=move || plans.get()
                        key=|plan| plan.meta.path.clone()
                        children=move |plan| {
                            let path = plan.meta.path.clone();
                            let title = plan.meta.title.clone();
                            let description = plan_picker_description(&plan);
                            let active_path = path.clone();
                            view! {
                                <button
                                    type="button"
                                    class="workspace-kanban-plan-picker__option"
                                    class:workspace-kanban-plan-picker__option--active=move || {
                                        selected_path.get() == active_path
                                    }
                                    on:click=move |_| {
                                        on_select.run(path.clone());
                                        open.set(false);
                                    }
                                >
                                    <span class="workspace-kanban-plan-picker__option-title">{title}</span>
                                    <span class="workspace-kanban-plan-picker__option-description">{description}</span>
                                </button>
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn KanbanPlanCard(
    plan: KanbanPlanNode,
    expanded_plans: RwSignal<Vec<String>>,
    dragged_task: RwSignal<Option<(String, String)>>,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
    on_expanded_change: Callback<Vec<String>>,
) -> impl IntoView {
    let path = plan.meta.path.clone();
    let is_open = Signal::derive(move || expanded_plans.with(|items| items.contains(&path)));
    let plan_path_for_lanes = StoredValue::new(plan.meta.path.clone());
    let plan_tasks_for_lanes = StoredValue::new(plan.tasks.clone());
    let toggle = {
        let path = plan.meta.path.clone();
        move |_| {
            let mut next = Vec::new();
            expanded_plans.update(|items| {
                if let Some(pos) = items.iter().position(|item| item == &path) {
                    items.remove(pos);
                } else {
                    items.push(path.clone());
                }
                next = items.clone();
            });
            on_expanded_change.run(next);
        }
    };

    view! {
        <article class="workspace-kanban-plan" data-state=plan_state_key(&plan.state)>
            <button type="button" class="workspace-kanban-plan__head" on:click=toggle>
                <LxIcon icon=icondata::LuFolderKanban width="1rem" height="1rem" />
                <span class="workspace-kanban-plan__main">
                    <span class="workspace-kanban-plan__title">{plan.meta.title.clone()}</span>
                    <span class="workspace-kanban-plan__path">{plan.meta.path.clone()}</span>
                </span>
                <span class="workspace-kanban-plan__stats">
                    {format!(
                        "{} / {}",
                        plan.meta.task_summary.completed,
                        plan.meta.task_summary.total
                    )}
                </span>
                <LxIcon icon=icondata::LuChevronDown width="0.9rem" height="0.9rem" />
            </button>
            <Show when=move || is_open.get()>
                <div class="workspace-kanban-plan__lanes">
                    <For
                        each=task_statuses
                        key=task_status_key
                        children=move |status| {
                            let tasks = plan_tasks_for_lanes.get_value();
                            let plan_path = plan_path_for_lanes.get_value();
                            let lane_tasks = tasks
                                .iter()
                                .filter(|task| task.status == status)
                                .cloned()
                                .collect::<Vec<_>>();
                            view! {
                                <KanbanTaskLane
                                    status=status
                                    plan_path=plan_path
                                    tasks=lane_tasks
                                    dragged_task=dragged_task
                                    workspace_cwd=workspace_cwd
                                    on_reload=on_reload
                                />
                            }
                        }
                    />
                </div>
            </Show>
        </article>
    }
}

#[component]
fn KanbanTaskLane(
    status: TaskStatus,
    plan_path: String,
    tasks: Vec<crate::tauri_bridge::KanbanTaskCard>,
    dragged_task: RwSignal<Option<(String, String)>>,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();
    let i18n = expect_context::<I18nService>();
    let status_for_drop = status.clone();
    let label_status = status.clone();
    let plan_for_drop = plan_path.clone();
    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        let Some((drag_plan, task_id)) = dragged_task.get_untracked() else {
            return;
        };
        if drag_plan != plan_for_drop {
            return;
        }
        let Some(ws) = workspace_cwd.get_untracked() else {
            return;
        };
        let patch = KanbanTaskUpdatePatch {
            title: None,
            status: Some(status_for_drop.clone()),
        };
        spawn_local(async move {
            match kanban_task_update(&ws, &drag_plan, &task_id, patch).await {
                Ok(_) => {
                    wb.bump_plans_epoch();
                    on_reload.run(());
                }
                Err(err) => toast.error(format!("Task update failed: {err}")),
            }
        });
        dragged_task.set(None);
    };

    view! {
        <section
            class="workspace-kanban-lane"
            data-status=task_status_key(&status)
            on:dragover=move |ev| ev.prevent_default()
            on:drop=on_drop
        >
            <header class="workspace-kanban-lane__head">
                <LxIcon icon=task_status_icon(&status) width="0.9rem" height="0.9rem" />
                <span>{move || i18n.tr(task_status_label_key(&label_status))()}</span>
                <span>{tasks.len()}</span>
            </header>
            <div class="workspace-kanban-lane__cards">
                <For
                    each=move || tasks.clone()
                    key=|task| task.id.clone()
                    children=move |task| view! {
                        <KanbanTaskCardView
                            task=task
                            dragged_task=dragged_task
                            workspace_cwd=workspace_cwd
                            on_reload=on_reload
                        />
                    }
                />
            </div>
        </section>
    }
}

#[component]
fn KanbanTaskCardView(
    task: crate::tauri_bridge::KanbanTaskCard,
    dragged_task: RwSignal<Option<(String, String)>>,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();
    let editing = RwSignal::new(false);
    let draft = RwSignal::new(task.title.clone());
    let task_for_drag = task.clone();
    let delete_task = {
        let task = task.clone();
        move |_| {
            let Some(ws) = workspace_cwd.get_untracked() else {
                return;
            };
            let plan_path = task.plan_path.clone();
            let id = task.id.clone();
            spawn_local(async move {
                match kanban_task_delete(&ws, &plan_path, &id).await {
                    Ok(()) => {
                        wb.bump_plans_epoch();
                        on_reload.run(());
                    }
                    Err(err) => toast.error(format!("Delete failed: {err}")),
                }
            });
        }
    };
    let save_title = Callback::new({
        let task = task.clone();
        move |()| {
            let Some(ws) = workspace_cwd.get_untracked() else {
                return;
            };
            let title = draft.get_untracked();
            if title.trim().is_empty() {
                return;
            }
            let plan_path = task.plan_path.clone();
            let id = task.id.clone();
            spawn_local(async move {
                match kanban_task_update(
                    &ws,
                    &plan_path,
                    &id,
                    KanbanTaskUpdatePatch {
                        title: Some(title),
                        status: None,
                    },
                )
                .await
                {
                    Ok(_) => {
                        editing.set(false);
                        wb.bump_plans_epoch();
                        on_reload.run(());
                    }
                    Err(err) => toast.error(format!("Rename failed: {err}")),
                }
            });
        }
    });

    view! {
        <article
            class="workspace-kanban-task"
            draggable="true"
            on:dragstart=move |_| {
                dragged_task.set(Some((task_for_drag.plan_path.clone(), task_for_drag.id.clone())));
            }
            on:dragend=move |_| dragged_task.set(None)
        >
            <Show
                when=move || editing.get()
                fallback=move || view! {
                    <button
                        type="button"
                        class="workspace-kanban-task__title"
                        on:dblclick=move |_| editing.set(true)
                    >
                        {task.title.clone()}
                    </button>
                }
            >
                <input
                    class="workspace-kanban-task__input"
                    prop:value=move || draft.get()
                    on:input=move |ev| draft.set(input_value(&ev))
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" {
                            save_title.run(());
                        } else if ev.key() == "Escape" {
                            editing.set(false);
                        }
                    }
                />
            </Show>
            <footer class="workspace-kanban-task__foot">
                <span>{task.id.clone()}</span>
                {task.runtime_task_id.as_ref().map(|runtime_id| view! {
                    <span title=runtime_id.clone()>{runtime_id.clone()}</span>
                })}
                <button type="button" title="Rename" on:click=move |_| editing.set(true)>
                    <LxIcon icon=icondata::LuPencil width="0.8rem" height="0.8rem" />
                </button>
                <button type="button" title="Delete" on:click=delete_task>
                    <LxIcon icon=icondata::LuTrash2 width="0.8rem" height="0.8rem" />
                </button>
            </footer>
        </article>
    }
}

fn plan_states() -> Vec<KanbanPlanState> {
    vec![
        KanbanPlanState::Blocked,
        KanbanPlanState::InProgress,
        KanbanPlanState::Pending,
        KanbanPlanState::Completed,
        KanbanPlanState::Cancelled,
        KanbanPlanState::Empty,
    ]
}

fn open_plan_sections_storage_key(workspace_cwd: &str) -> String {
    format!("blxcode.workspace-kanban.open-sections.v1:{workspace_cwd}")
}

fn read_open_plan_sections(workspace_cwd: &str) -> Vec<KanbanPlanState> {
    let key = open_plan_sections_storage_key(workspace_cwd);
    let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
    else {
        return vec![KanbanPlanState::InProgress];
    };
    match storage.get_item(&key).ok().flatten() {
        Some(raw) => raw
            .split(',')
            .filter_map(plan_state_from_key)
            .collect::<Vec<_>>(),
        None => vec![KanbanPlanState::InProgress],
    }
}

fn write_open_plan_sections(workspace_cwd: &str, sections: &[KanbanPlanState]) {
    let Some(storage) = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
    else {
        return;
    };
    let raw = sections
        .iter()
        .map(plan_state_key)
        .collect::<Vec<_>>()
        .join(",");
    let _ = storage.set_item(&open_plan_sections_storage_key(workspace_cwd), &raw);
}

fn plan_state_from_key(raw: &str) -> Option<KanbanPlanState> {
    match raw {
        "blocked" => Some(KanbanPlanState::Blocked),
        "in-progress" => Some(KanbanPlanState::InProgress),
        "pending" => Some(KanbanPlanState::Pending),
        "completed" => Some(KanbanPlanState::Completed),
        "cancelled" => Some(KanbanPlanState::Cancelled),
        "empty" => Some(KanbanPlanState::Empty),
        _ => None,
    }
}

fn plan_picker_description(plan: &KanbanPlanNode) -> String {
    let summary = &plan.meta.task_summary;
    let active = summary.pending + summary.in_progress + summary.blocked;
    format!(
        "{} active / {} total - {}",
        active, summary.total, plan.meta.path
    )
}

fn task_statuses() -> Vec<TaskStatus> {
    vec![
        TaskStatus::Pending,
        TaskStatus::InProgress,
        TaskStatus::Blocked,
        TaskStatus::Completed,
        TaskStatus::Cancelled,
    ]
}

fn plan_state_key(state: &KanbanPlanState) -> &'static str {
    match state {
        KanbanPlanState::Blocked => "blocked",
        KanbanPlanState::InProgress => "in-progress",
        KanbanPlanState::Pending => "pending",
        KanbanPlanState::Completed => "completed",
        KanbanPlanState::Cancelled => "cancelled",
        KanbanPlanState::Empty => "empty",
    }
}

fn plan_state_label_key(state: &KanbanPlanState) -> I18nKey {
    match state {
        KanbanPlanState::Blocked => I18nKey::PlansTaskStatBlocked,
        KanbanPlanState::InProgress => I18nKey::PlansTaskStatInProgress,
        KanbanPlanState::Pending => I18nKey::PlansTaskStatPending,
        KanbanPlanState::Completed => I18nKey::PlansTaskStatCompleted,
        KanbanPlanState::Cancelled => I18nKey::PlansTaskStatCancelled,
        KanbanPlanState::Empty => I18nKey::PlansFilterEmpty,
    }
}

fn plan_state_icon(state: &KanbanPlanState) -> icondata::Icon {
    match state {
        KanbanPlanState::Blocked => icondata::LuCircleAlert,
        KanbanPlanState::InProgress => icondata::LuCirclePlay,
        KanbanPlanState::Pending => icondata::LuCircle,
        KanbanPlanState::Completed => icondata::LuCircleCheck,
        KanbanPlanState::Cancelled => icondata::LuCircleMinus,
        KanbanPlanState::Empty => icondata::LuCircleDashed,
    }
}

fn task_status_key(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Completed => "completed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn task_status_label_key(status: &TaskStatus) -> I18nKey {
    match status {
        TaskStatus::Pending => I18nKey::PlansTaskStatPending,
        TaskStatus::InProgress => I18nKey::PlansTaskStatInProgress,
        TaskStatus::Blocked => I18nKey::PlansTaskStatBlocked,
        TaskStatus::Completed => I18nKey::PlansTaskStatCompleted,
        TaskStatus::Cancelled => I18nKey::PlansTaskStatCancelled,
    }
}

fn task_status_icon(status: &TaskStatus) -> icondata::Icon {
    match status {
        TaskStatus::Pending => icondata::LuCircle,
        TaskStatus::InProgress => icondata::LuCirclePlay,
        TaskStatus::Blocked => icondata::LuCircleAlert,
        TaskStatus::Completed => icondata::LuCircleCheck,
        TaskStatus::Cancelled => icondata::LuCircleMinus,
    }
}

fn input_value(ev: &web_sys::Event) -> String {
    event_target_value(ev)
}
