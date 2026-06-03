use crate::agent_wire::TaskStatus;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    clipboard_read_text_compat, clipboard_write_text_compat, kanban_board_load,
    kanban_export_layout, kanban_import_layout, kanban_layout_save, kanban_plan_move,
    kanban_task_create, kanban_task_delete, kanban_task_move, kanban_task_update, KanbanBoard,
    KanbanPlanMoveInput, KanbanPlanNode, KanbanPlanState, KanbanTaskCreateInput,
    KanbanTaskMoveInput, KanbanTaskUpdatePatch,
};
use crate::workbench::kanban_dnd::{
    is_kanban_drag, read_drag_payload, start_kanban_drag, KanbanDragKind, KanbanDragMeta,
    KanbanDragPayload, KanbanDragService, KanbanDropTarget,
};
use crate::workbench::toast::{ToastKind, ToastService};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug)]
struct KanbanPlanDrop {
    plan_path: String,
    target_state: KanbanPlanState,
    before_plan_path: Option<String>,
}

#[derive(Clone, Debug)]
struct KanbanTaskDrop {
    plan_path: String,
    task_id: String,
    target_status: TaskStatus,
    before_task_id: Option<String>,
}

#[component]
pub fn WorkspaceKanban(workspace_id: u64) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();
    let kanban_dnd = expect_context::<KanbanDragService>();
    let board = RwSignal::<Option<KanbanBoard>>::new(None);
    let loading = RwSignal::new(false);
    let error = RwSignal::<Option<String>>::new(None);
    let query = RwSignal::new(String::new());
    let expanded_plans = RwSignal::new(Vec::<String>::new());
    let highlighted_plan = RwSignal::<Option<String>>::new(None);
    let open_sections = RwSignal::new(Vec::<KanbanPlanState>::new());
    let open_sections_workspace = RwSignal::<Option<String>>::new(None);
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
                    let focus_request = wb
                        .kanban_plan_focus_request()
                        .get_untracked()
                        .filter(|request| request.workspace_id == workspace_id);
                    let focus_requested = focus_request.is_some();
                    if open_sections_workspace
                        .with_untracked(|current| current.as_deref() != Some(&ws))
                    {
                        expanded_plans.set(Vec::new());
                        open_sections.set(read_open_plan_sections(&ws));
                        open_sections_workspace.set(Some(ws.clone()));
                    }
                    if new_task_plan.with_untracked(|s| s.trim().is_empty()) {
                        if let Some(first) = next.plans.first() {
                            new_task_plan.set(first.meta.path.clone());
                        }
                    }
                    let focused_plan = focus_request.and_then(|request| {
                        next.plans
                            .iter()
                            .find(|plan| plan.meta.path == request.plan_path)
                            .map(|plan| (plan.meta.path.clone(), plan.state.clone()))
                    });
                    if let Some((path, state)) = focused_plan.clone() {
                        query.set(String::new());
                        expanded_plans.update(|items| {
                            if !items.contains(&path) {
                                items.push(path.clone());
                            }
                        });
                        open_sections.update(|items| {
                            if !items.contains(&state) {
                                items.push(state.clone());
                            }
                        });
                    }
                    board.set(Some(next));
                    if focus_requested {
                        wb.kanban_plan_focus_request().set(None);
                    }
                    if let Some((path, _)) = focused_plan {
                        highlighted_plan.set(Some(path.clone()));
                        scroll_kanban_plan_into_view(workspace_id, path.clone());
                        spawn_local(async move {
                            TimeoutFuture::new(1600).await;
                            highlighted_plan.update(|current| {
                                if current.as_deref() == Some(path.as_str()) {
                                    *current = None;
                                }
                            });
                        });
                    }
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

    let move_plan = Callback::new(move |drop: KanbanPlanDrop| {
        let Some(ws) = workspace_cwd.get_untracked() else {
            kanban_dnd.clear();
            return;
        };
        let Some(current_board) = board.get_untracked() else {
            kanban_dnd.clear();
            return;
        };
        let ordered_plan_paths = reordered_plan_paths(
            &current_board,
            &drop.plan_path,
            drop.before_plan_path.as_deref(),
        );
        let input = KanbanPlanMoveInput {
            plan_path: drop.plan_path,
            target_state: drop.target_state,
            ordered_plan_paths,
        };
        spawn_local(async move {
            match kanban_plan_move(&ws, input).await {
                Ok(next) => {
                    board.set(Some(next));
                    wb.bump_plans_epoch();
                }
                Err(err) => toast.error(format!("Plan move failed: {err}")),
            }
            kanban_dnd.clear();
        });
    });

    let move_task = Callback::new(move |drop: KanbanTaskDrop| {
        let Some(ws) = workspace_cwd.get_untracked() else {
            kanban_dnd.clear();
            return;
        };
        let input = KanbanTaskMoveInput {
            plan_path: drop.plan_path,
            task_id: drop.task_id,
            target_status: drop.target_status,
            before_task_id: drop.before_task_id,
        };
        spawn_local(async move {
            match kanban_task_move(&ws, input).await {
                Ok(_) => {
                    wb.bump_plans_epoch();
                    load_board();
                }
                Err(err) => toast.error(format!("Task move failed: {err}")),
            }
            kanban_dnd.clear();
        });
    });

    view! {
        <div
            class="workspace-kanban"
            class:workspace-kanban--drag-active=move || kanban_dnd.active_payload.get().is_some()
            role="region"
            aria-label=move || i18n.tr(I18nKey::KanbanTitle)()
        >
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
                                    workspace_id=workspace_id
                                    expanded_plans=expanded_plans
                                    workspace_cwd=workspace_cwd
                                    on_reload=Callback::new(move |()| load_board())
                                    on_expanded_change=save_expanded_plans
                                    open_sections=open_sections
                                    on_open_sections_change=save_open_sections
                                    on_plan_drop=move_plan
                                    on_task_drop=move_task
                                    highlighted_plan=highlighted_plan
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
    workspace_id: u64,
    expanded_plans: RwSignal<Vec<String>>,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
    on_expanded_change: Callback<Vec<String>>,
    open_sections: RwSignal<Vec<KanbanPlanState>>,
    on_open_sections_change: Callback<Vec<KanbanPlanState>>,
    on_plan_drop: Callback<KanbanPlanDrop>,
    on_task_drop: Callback<KanbanTaskDrop>,
    highlighted_plan: RwSignal<Option<String>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open_state = state.clone();
    let click_state = state.clone();
    let aria_state = state.clone();
    let show_state = state.clone();
    let label_state = state.clone();
    let state_value = StoredValue::new(state.clone());
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
                            children=move |plan| {
                                    let before_path = plan.meta.path.clone();
                                    view! {
                                        <KanbanPlanDropZone
                                            state=state_value.get_value()
                                            before_plan_path=Some(before_path)
                                            on_plan_drop=on_plan_drop
                                        />
                                    <KanbanPlanCard
                                        workspace_id=workspace_id
                                        plan=plan
                                        expanded_plans=expanded_plans
                                        workspace_cwd=workspace_cwd
                                        on_reload=on_reload
                                        on_expanded_change=on_expanded_change
                                        on_task_drop=on_task_drop
                                        highlighted_plan=highlighted_plan
                                    />
                                }
                            }
                        />
                    </Show>
                    <KanbanPlanDropZone
                        state=state_value.get_value()
                        before_plan_path=None
                        on_plan_drop=on_plan_drop
                    />
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
    workspace_id: u64,
    plan: KanbanPlanNode,
    expanded_plans: RwSignal<Vec<String>>,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
    on_expanded_change: Callback<Vec<String>>,
    on_task_drop: Callback<KanbanTaskDrop>,
    highlighted_plan: RwSignal<Option<String>>,
) -> impl IntoView {
    let kanban_dnd = expect_context::<KanbanDragService>();
    let path = plan.meta.path.clone();
    let is_open = Signal::derive(move || expanded_plans.with(|items| items.contains(&path)));
    let drag_path = plan.meta.path.clone();
    let is_drag_source = Signal::derive({
        let path = plan.meta.path.clone();
        move || {
            kanban_dnd.active_payload.get().is_some_and(|payload| {
                payload.kind == KanbanDragKind::Plan && payload.plan_path == path
            })
        }
    });
    let is_drag_potential = Signal::derive(move || {
        kanban_dnd
            .active_payload
            .get()
            .is_some_and(|payload| payload.kind == KanbanDragKind::Plan)
            && !is_drag_source.get()
    });
    let plan_path_for_lanes = StoredValue::new(plan.meta.path.clone());
    let plan_tasks_for_lanes = StoredValue::new(plan.tasks.clone());
    let dom_id = kanban_plan_dom_id(workspace_id, &plan.meta.path);
    let highlight_path = plan.meta.path.clone();
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
        <article
            class="workspace-kanban-plan"
            class:workspace-kanban-plan--drag-source=move || is_drag_source.get()
            class:workspace-kanban-plan--drag-potential=move || is_drag_potential.get()
            class:workspace-kanban-plan--highlight=move || {
                highlighted_plan.with(|current| current.as_deref() == Some(highlight_path.as_str()))
            }
            data-state=plan_state_key(&plan.state)
            id=dom_id
            prop:draggable=true
            on:dragstart={
                let title = plan.meta.title.clone();
                let subtitle = plan.meta.path.clone();
                let payload_path = drag_path.clone();
                move |ev: web_sys::DragEvent| {
                    if drag_started_from_control(&ev) {
                        ev.prevent_default();
                        return;
                    }
                    start_kanban_drag(
                        &ev,
                        kanban_dnd,
                        KanbanDragPayload {
                            workspace_id,
                            kind: KanbanDragKind::Plan,
                            plan_path: payload_path.clone(),
                            task_id: None,
                        },
                        KanbanDragMeta {
                            kind: KanbanDragKind::Plan,
                            title: title.clone(),
                            subtitle: subtitle.clone(),
                            badge: "Plan".into(),
                        },
                    );
                }
            }
            on:drag=move |ev: web_sys::DragEvent| kanban_dnd.set_overlay_pos_from_event(&ev)
            on:dragend=move |_| kanban_dnd.clear()
        >
            <button type="button" class="workspace-kanban-plan__head" on:click=toggle>
                <span
                    class="workspace-kanban-plan__drag"
                    title="Drag plan"
                >
                    <LxIcon icon=icondata::LuGripVertical width="0.86rem" height="0.86rem" />
                </span>
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
                                    workspace_id=workspace_id
                                    workspace_cwd=workspace_cwd
                                    on_reload=on_reload
                                    on_task_drop=on_task_drop
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
    workspace_id: u64,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
    on_task_drop: Callback<KanbanTaskDrop>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let label_status = status.clone();
    let plan_value = StoredValue::new(plan_path.clone());
    let status_value = StoredValue::new(status.clone());

    view! {
        <section
            class="workspace-kanban-lane"
            data-status=task_status_key(&status)
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
                    children=move |task| {
                        let before_task_id = task.id.clone();
                        view! {
                            <KanbanTaskDropZone
                                plan_path=plan_value.get_value()
                                status=status_value.get_value()
                                before_task_id=Some(before_task_id)
                                on_task_drop=on_task_drop
                            />
                            <KanbanTaskCardView
                                workspace_id=workspace_id
                                task=task
                                workspace_cwd=workspace_cwd
                                on_reload=on_reload
                            />
                        }
                    }
                />
                <KanbanTaskDropZone
                    plan_path=plan_value.get_value()
                    status=status_value.get_value()
                    before_task_id=None
                    on_task_drop=on_task_drop
                />
            </div>
        </section>
    }
}

#[component]
fn KanbanTaskCardView(
    workspace_id: u64,
    task: crate::tauri_bridge::KanbanTaskCard,
    workspace_cwd: Signal<Option<String>>,
    on_reload: Callback<()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();
    let kanban_dnd = expect_context::<KanbanDragService>();
    let editing = RwSignal::new(false);
    let draft = RwSignal::new(task.title.clone());
    let task_title = StoredValue::new(task.title.clone());
    let task_for_drag = task.clone();
    let is_drag_source = Signal::derive({
        let task = task.clone();
        move || {
            kanban_dnd.active_payload.get().is_some_and(|payload| {
                payload.kind == KanbanDragKind::Task
                    && payload.plan_path == task.plan_path
                    && payload.task_id.as_deref() == Some(task.id.as_str())
            })
        }
    });
    let is_drag_potential = Signal::derive(move || {
        kanban_dnd.active_payload.get().is_some_and(|payload| {
            payload.kind == KanbanDragKind::Task && payload.plan_path == task_for_drag.plan_path
        }) && !is_drag_source.get()
    });
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
            class:workspace-kanban-task--drag-source=move || is_drag_source.get()
            class:workspace-kanban-task--drag-potential=move || is_drag_potential.get()
            prop:draggable=true
            on:dragstart={
                let task = task.clone();
                move |ev: web_sys::DragEvent| {
                    if drag_started_from_control(&ev) {
                        ev.prevent_default();
                        return;
                    }
                    start_kanban_drag(
                        &ev,
                        kanban_dnd,
                        KanbanDragPayload {
                            workspace_id,
                            kind: KanbanDragKind::Task,
                            plan_path: task.plan_path.clone(),
                            task_id: Some(task.id.clone()),
                        },
                        KanbanDragMeta {
                            kind: KanbanDragKind::Task,
                            title: task.title.clone(),
                            subtitle: task.plan_path.clone(),
                            badge: task.id.clone(),
                        },
                    );
                }
            }
            on:drag=move |ev: web_sys::DragEvent| kanban_dnd.set_overlay_pos_from_event(&ev)
            on:dragend=move |_| kanban_dnd.clear()
        >
            <Show
                when=move || editing.get()
                fallback=move || view! {
                    <button
                        type="button"
                        class="workspace-kanban-task__title"
                        on:dblclick=move |_| editing.set(true)
                    >
                        {task_title.get_value()}
                    </button>
                }
            >
                <input
                    class="workspace-kanban-task__input"
                    data-kanban-no-card-drag="true"
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
                <button
                    type="button"
                    title="Rename"
                    data-kanban-no-card-drag="true"
                    prop:draggable=false
                    on:click=move |_| editing.set(true)
                >
                    <LxIcon icon=icondata::LuPencil width="0.8rem" height="0.8rem" />
                </button>
                <button
                    type="button"
                    title="Delete"
                    data-kanban-no-card-drag="true"
                    prop:draggable=false
                    on:click=delete_task
                >
                    <LxIcon icon=icondata::LuTrash2 width="0.8rem" height="0.8rem" />
                </button>
            </footer>
        </article>
    }
}

#[component]
fn KanbanPlanDropZone(
    state: KanbanPlanState,
    before_plan_path: Option<String>,
    on_plan_drop: Callback<KanbanPlanDrop>,
) -> impl IntoView {
    let kanban_dnd = expect_context::<KanbanDragService>();
    let state_value = StoredValue::new(state);
    let before_value = StoredValue::new(before_plan_path);
    let active = Signal::derive(move || {
        let before_plan_path = before_value.get_value();
        kanban_dnd.active_payload.get().is_some_and(|payload| {
            payload.kind == KanbanDragKind::Plan
                && before_plan_path.as_deref() != Some(payload.plan_path.as_str())
        })
    });
    let over = Signal::derive(move || {
        kanban_dnd.ghost.get().as_ref()
            == Some(&KanbanDropTarget::Plan {
                state: state_value.get_value(),
                before_plan_path: before_value.get_value(),
            })
    });

    view! {
        <div
            class="workspace-kanban-drop-zone workspace-kanban-drop-zone--plan"
            class:workspace-kanban-drop-zone--active=move || active.get()
            class:workspace-kanban-drop-zone--over=move || over.get()
            on:dragenter=move |ev: web_sys::DragEvent| {
                let before_plan_path = before_value.get_value();
                if accepts_plan_drop(kanban_dnd, &ev, before_plan_path.as_deref()) {
                    ev.prevent_default();
                    kanban_dnd.ghost.set(Some(KanbanDropTarget::Plan {
                        state: state_value.get_value(),
                        before_plan_path,
                    }));
                }
            }
            on:dragover=move |ev: web_sys::DragEvent| {
                let before_plan_path = before_value.get_value();
                if accepts_plan_drop(kanban_dnd, &ev, before_plan_path.as_deref()) {
                    ev.prevent_default();
                    if let Some(dt) = ev.data_transfer() {
                        let _ = dt.set_drop_effect("move");
                    }
                    kanban_dnd.set_overlay_pos_from_event(&ev);
                    kanban_dnd.ghost.set(Some(KanbanDropTarget::Plan {
                        state: state_value.get_value(),
                        before_plan_path,
                    }));
                }
            }
            on:dragleave=move |ev: web_sys::DragEvent| {
                ev.prevent_default();
                if kanban_dnd.ghost.get_untracked().as_ref()
                    == Some(&KanbanDropTarget::Plan {
                        state: state_value.get_value(),
                        before_plan_path: before_value.get_value(),
                    })
                {
                    kanban_dnd.ghost.set(None);
                }
            }
            on:drop=move |ev: web_sys::DragEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                let payload = ev
                    .data_transfer()
                    .and_then(|dt| read_drag_payload(&dt))
                    .or_else(|| kanban_dnd.active_payload.get_untracked());
                if let Some(payload) = payload.filter(|p| p.kind == KanbanDragKind::Plan) {
                    let before_plan_path = before_value.get_value();
                    if before_plan_path.as_deref() != Some(payload.plan_path.as_str()) {
                        on_plan_drop.run(KanbanPlanDrop {
                            plan_path: payload.plan_path,
                            target_state: state_value.get_value(),
                            before_plan_path,
                        });
                        return;
                    }
                }
                kanban_dnd.clear();
            }
        >
            <span class="workspace-kanban-drop-zone__line"></span>
            <span class="workspace-kanban-drop-zone__label">"Drop plan"</span>
        </div>
    }
}

#[component]
fn KanbanTaskDropZone(
    plan_path: String,
    status: TaskStatus,
    before_task_id: Option<String>,
    on_task_drop: Callback<KanbanTaskDrop>,
) -> impl IntoView {
    let kanban_dnd = expect_context::<KanbanDragService>();
    let plan_value = StoredValue::new(plan_path);
    let status_value = StoredValue::new(status);
    let before_value = StoredValue::new(before_task_id);
    let active = Signal::derive(move || {
        let plan_path = plan_value.get_value();
        let before_task_id = before_value.get_value();
        kanban_dnd.active_payload.get().is_some_and(|payload| {
            payload.kind == KanbanDragKind::Task
                && payload.plan_path == plan_path
                && before_task_id.as_deref() != payload.task_id.as_deref()
        })
    });
    let over = Signal::derive(move || {
        kanban_dnd.ghost.get().as_ref()
            == Some(&KanbanDropTarget::Task {
                plan_path: plan_value.get_value(),
                status: status_value.get_value(),
                before_task_id: before_value.get_value(),
            })
    });

    view! {
        <div
            class="workspace-kanban-drop-zone workspace-kanban-drop-zone--task"
            class:workspace-kanban-drop-zone--active=move || active.get()
            class:workspace-kanban-drop-zone--over=move || over.get()
            on:dragenter=move |ev: web_sys::DragEvent| {
                let plan_path = plan_value.get_value();
                let before_task_id = before_value.get_value();
                if accepts_task_drop(kanban_dnd, &ev, &plan_path, before_task_id.as_deref()) {
                    ev.prevent_default();
                    kanban_dnd.ghost.set(Some(KanbanDropTarget::Task {
                        plan_path,
                        status: status_value.get_value(),
                        before_task_id,
                    }));
                }
            }
            on:dragover=move |ev: web_sys::DragEvent| {
                let plan_path = plan_value.get_value();
                let before_task_id = before_value.get_value();
                if accepts_task_drop(kanban_dnd, &ev, &plan_path, before_task_id.as_deref()) {
                    ev.prevent_default();
                    if let Some(dt) = ev.data_transfer() {
                        let _ = dt.set_drop_effect("move");
                    }
                    kanban_dnd.set_overlay_pos_from_event(&ev);
                    kanban_dnd.ghost.set(Some(KanbanDropTarget::Task {
                        plan_path,
                        status: status_value.get_value(),
                        before_task_id,
                    }));
                }
            }
            on:dragleave=move |ev: web_sys::DragEvent| {
                ev.prevent_default();
                if kanban_dnd.ghost.get_untracked().as_ref()
                    == Some(&KanbanDropTarget::Task {
                        plan_path: plan_value.get_value(),
                        status: status_value.get_value(),
                        before_task_id: before_value.get_value(),
                    })
                {
                    kanban_dnd.ghost.set(None);
                }
            }
            on:drop=move |ev: web_sys::DragEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                let payload = ev
                    .data_transfer()
                    .and_then(|dt| read_drag_payload(&dt))
                    .or_else(|| kanban_dnd.active_payload.get_untracked());
                let plan_path = plan_value.get_value();
                let before_task_id = before_value.get_value();
                if let Some(payload) = payload.filter(|p| {
                    p.kind == KanbanDragKind::Task
                        && p.plan_path == plan_path
                        && before_task_id.as_deref() != p.task_id.as_deref()
                }) {
                    if let Some(task_id) = payload.task_id {
                        on_task_drop.run(KanbanTaskDrop {
                            plan_path: payload.plan_path,
                            task_id,
                            target_status: status_value.get_value(),
                            before_task_id,
                        });
                        return;
                    }
                }
                kanban_dnd.clear();
            }
        >
            <span class="workspace-kanban-drop-zone__line"></span>
            <span class="workspace-kanban-drop-zone__label">"Drop task"</span>
        </div>
    }
}

fn accepts_plan_drop(
    kanban_dnd: KanbanDragService,
    ev: &web_sys::DragEvent,
    before_plan_path: Option<&str>,
) -> bool {
    let is_drag =
        kanban_dnd.session_active() || ev.data_transfer().as_ref().is_some_and(is_kanban_drag);
    if !is_drag {
        return false;
    }
    let payload = kanban_dnd
        .active_payload
        .get_untracked()
        .or_else(|| ev.data_transfer().and_then(|dt| read_drag_payload(&dt)));
    match payload {
        Some(payload) => {
            payload.kind == KanbanDragKind::Plan
                && before_plan_path != Some(payload.plan_path.as_str())
        }
        None => true,
    }
}

fn drag_started_from_control(ev: &web_sys::DragEvent) -> bool {
    ev.target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .and_then(|element| {
            element
                .closest("input, textarea, select, [data-kanban-no-card-drag='true']")
                .ok()
                .flatten()
        })
        .is_some()
}

fn accepts_task_drop(
    kanban_dnd: KanbanDragService,
    ev: &web_sys::DragEvent,
    plan_path: &str,
    before_task_id: Option<&str>,
) -> bool {
    let is_drag =
        kanban_dnd.session_active() || ev.data_transfer().as_ref().is_some_and(is_kanban_drag);
    if !is_drag {
        return false;
    }
    let payload = kanban_dnd
        .active_payload
        .get_untracked()
        .or_else(|| ev.data_transfer().and_then(|dt| read_drag_payload(&dt)));
    match payload {
        Some(payload) => {
            payload.kind == KanbanDragKind::Task
                && payload.plan_path == plan_path
                && before_task_id != payload.task_id.as_deref()
        }
        None => true,
    }
}

fn reordered_plan_paths(
    board: &KanbanBoard,
    dragged_path: &str,
    before_plan_path: Option<&str>,
) -> Vec<String> {
    let mut paths = board
        .plans
        .iter()
        .map(|plan| plan.meta.path.clone())
        .filter(|path| path != dragged_path)
        .collect::<Vec<_>>();
    let insert_idx = before_plan_path
        .and_then(|before| paths.iter().position(|path| path == before))
        .unwrap_or(paths.len());
    paths.insert(insert_idx, dragged_path.to_owned());
    paths
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
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
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
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
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

fn kanban_plan_dom_id(workspace_id: u64, plan_path: &str) -> String {
    let mut id = format!("workspace-kanban-plan-{workspace_id}-");
    for byte in plan_path.as_bytes() {
        id.push_str(&format!("{byte:02x}"));
    }
    id
}

fn scroll_kanban_plan_into_view(workspace_id: u64, plan_path: String) {
    spawn_local(async move {
        TimeoutFuture::new(0).await;
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        if let Some(node) =
            document.get_element_by_id(&kanban_plan_dom_id(workspace_id, &plan_path))
        {
            node.scroll_into_view();
        }
    });
}

fn input_value(ev: &web_sys::Event) -> String {
    event_target_value(ev)
}
