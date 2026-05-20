//! Workspace plans panel — list, edit, preview, rename/delete, and load
//! plans from `<workspace>/.agents/plans/` into the BLXCode Agent.
//!
//! Mirrors the design of `memory_panel/MemoryFilesView` (list + selected
//! Markdown editor + preview toggle) but stays focused on plan-specific
//! actions: a single Manage view, plus a "Load into BLXCode Agent" button
//! that calls `plan_load` and attaches the plan to shared context.

use crate::agent_wire::{AgentContextItem, AgentContextKind};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    self, plan_create, plan_delete, plan_list, plan_load, plan_read, plan_rename, plan_write,
    PlanContent, PlanMeta, PlanTaskSummaryWire,
};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

const SAVE_DEBOUNCE_MS: u32 = 600;

#[derive(Clone, Copy)]
struct PlansState {
    workspace_cwd: RwSignal<Option<String>>,
    plans: RwSignal<Vec<PlanMeta>>,
    active_path: RwSignal<Option<String>>,
    editor_content: RwSignal<String>,
    editor_dirty: RwSignal<bool>,
    show_preview: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    save_token: RwSignal<u32>,
    renaming: RwSignal<Option<String>>,
    rename_input: RwSignal<String>,
    new_plan_input: RwSignal<String>,
    is_index: RwSignal<bool>,
}

impl PlansState {
    fn new() -> Self {
        Self {
            workspace_cwd: RwSignal::new(None),
            plans: RwSignal::new(Vec::new()),
            active_path: RwSignal::new(None),
            editor_content: RwSignal::new(String::new()),
            editor_dirty: RwSignal::new(false),
            show_preview: RwSignal::new(true),
            error: RwSignal::new(None),
            save_token: RwSignal::new(0),
            renaming: RwSignal::new(None),
            rename_input: RwSignal::new(String::new()),
            new_plan_input: RwSignal::new(String::new()),
            is_index: RwSignal::new(false),
        }
    }
}

fn current_workspace_cwd(wb: WorkbenchService) -> Option<String> {
    let id = wb.active_id().get()?;
    wb.workspaces().with(|list| {
        list.iter()
            .find(|w| w.id == id)
            .map(|w| w.cwd.clone())
            .filter(|cwd| !cwd.trim().is_empty())
    })
}

fn load_plans_list(state: PlansState, ws: String) {
    spawn_local(async move {
        match plan_list(&ws).await {
            Ok(list) => {
                state.plans.set(list);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

/// Read the persisted task snapshot and, if `activePlanPath` is set,
/// auto-select that plan in the editor. Used on workspace activation
/// so a user closing+reopening the harness lands back on the plan they
/// were working with.
fn restore_active_plan(state: PlansState, ws: String) {
    spawn_local(async move {
        if let Ok(snap) = crate::tauri_bridge::tasks_list(ws.clone()).await {
            if let Some(path) = snap.active_plan_path.clone() {
                if state.active_path.get_untracked().is_none() {
                    load_plan(state, ws, path);
                }
            }
        }
    });
}

fn load_plan(state: PlansState, ws: String, path: String) {
    spawn_local(async move {
        match plan_read(&ws, &path).await {
            Ok(PlanContent {
                content,
                path,
                is_index,
                ..
            }) => {
                state.editor_content.set(content);
                state.editor_dirty.set(false);
                state.active_path.set(Some(path));
                state.is_index.set(is_index);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn schedule_save(state: PlansState, ws: String) {
    let token = state.save_token.get_untracked().wrapping_add(1);
    state.save_token.set(token);
    spawn_local(async move {
        TimeoutFuture::new(SAVE_DEBOUNCE_MS).await;
        if state.save_token.get_untracked() != token {
            return;
        }
        let Some(path) = state.active_path.get_untracked() else {
            return;
        };
        if !state.editor_dirty.get_untracked() {
            return;
        }
        let content = state.editor_content.get_untracked();
        match plan_write(&ws, &path, &content).await {
            Ok(_) => {
                state.editor_dirty.set(false);
                load_plans_list(state, ws);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn input_value(ev: web_sys::Event) -> Option<String> {
    ev.target()
        .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        .map(|el| el.value())
}

#[component]
pub fn PlansPanel() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let state = PlansState::new();

    Effect::new(move |_| {
        let cwd = current_workspace_cwd(wb);
        let prev = state.workspace_cwd.get_untracked();
        if cwd != prev {
            state.workspace_cwd.set(cwd.clone());
            state.active_path.set(None);
            state.editor_content.set(String::new());
            state.editor_dirty.set(false);
            state.plans.set(Vec::new());
            if let Some(ws) = cwd {
                load_plans_list(state, ws.clone());
                restore_active_plan(state, ws);
            }
        }
    });

    view! {
        <div class="workbench-plans" role="region">
            <Show when=move || state.error.get().is_some()>
                {move || view! {
                    <div class="workbench-plans__error" role="alert">
                        <span>{move || state.error.get().unwrap_or_default()}</span>
                        <button
                            type="button"
                            class="workbench-plans__error-dismiss"
                            on:click=move |_| state.error.set(None)
                        >
                            "×"
                        </button>
                    </div>
                }}
            </Show>
            <Show
                when=move || state.workspace_cwd.get().is_some()
                fallback=move || view! {
                    <div class="workbench-plans__placeholder">
                        <p class="workbench-plans__placeholder-title">
                            {move || i18n.tr(I18nKey::PlansEmptyTitle)()}
                        </p>
                        <p class="workbench-plans__placeholder-lead">
                            {move || i18n.tr(I18nKey::PlansEmptyLead)()}
                        </p>
                    </div>
                }
            >
                <PlansManageView state=state />
            </Show>
        </div>
    }
}

#[component]
fn PlansManageView(state: PlansState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();

    let create_plan = move |_| {
        let raw = state.new_plan_input.get_untracked();
        let trimmed = raw.trim().to_owned();
        if trimmed.is_empty() {
            return;
        }
        let path = if trimmed.to_lowercase().ends_with(".md") {
            trimmed
        } else {
            format!("{trimmed}.md")
        };
        let Some(ws) = state.workspace_cwd.get_untracked() else {
            return;
        };
        spawn_local(async move {
            match plan_create(&ws, &path, None).await {
                Ok(meta) => {
                    state.new_plan_input.set(String::new());
                    load_plans_list(state, ws.clone());
                    load_plan(state, ws, meta.path);
                }
                Err(e) => state.error.set(Some(e)),
            }
        });
    };

    let list_collapsed = RwSignal::new(false);

    view! {
        <div
            class="workbench-plans-manage"
            class:workbench-plans-manage--collapsed=move || list_collapsed.get()
        >
            <aside
                class="workbench-plans-manage__tree"
                class:workbench-plans-manage__tree--collapsed=move || list_collapsed.get()
            >
                <form
                    class="workbench-plans-manage__new"
                    class:workbench-plans-manage__new--collapsed=move || list_collapsed.get()
                    on:submit=move |ev: web_sys::SubmitEvent| {
                        ev.prevent_default();
                        create_plan(());
                    }
                >
                    <Show when=move || !list_collapsed.get()>
                        <input
                            type="text"
                            class="workbench-plans-manage__new-input"
                            placeholder=move || i18n.tr(I18nKey::PlansNewPlanPh)()
                            prop:value=move || state.new_plan_input.get()
                            on:input=move |ev| {
                                if let Some(v) = input_value(ev) {
                                    state.new_plan_input.set(v);
                                }
                            }
                        />
                    </Show>
                    <button
                        type="submit"
                        class="workbench-plans-manage__new-btn"
                        title=move || i18n.tr(I18nKey::PlansNewPlan)()
                        aria-label=move || i18n.tr(I18nKey::PlansNewPlan)()
                    >
                        <LxIcon icon=icondata::LuPlus width="0.82rem" height="0.82rem" />
                    </button>
                    <Show when=move || !list_collapsed.get()>
                        <button
                            type="button"
                            class="workbench-plans-manage__refresh"
                            title=move || i18n.tr(I18nKey::PlansRefresh)()
                            aria-label=move || i18n.tr(I18nKey::PlansRefresh)()
                            on:click=move |_| {
                                if let Some(ws) = state.workspace_cwd.get_untracked() {
                                    load_plans_list(state, ws);
                                }
                            }
                        >
                            <LxIcon icon=icondata::LuRefreshCw width="0.82rem" height="0.82rem" />
                        </button>
                    </Show>
                    <button
                        type="button"
                        class="workbench-plans-manage__collapse-btn"
                        aria-label=move || {
                            if list_collapsed.get() {
                                i18n.tr(I18nKey::MemFilesExpand)()
                            } else {
                                i18n.tr(I18nKey::MemFilesCollapse)()
                            }
                        }
                        title=move || {
                            if list_collapsed.get() {
                                i18n.tr(I18nKey::MemFilesExpand)()
                            } else {
                                i18n.tr(I18nKey::MemFilesCollapse)()
                            }
                        }
                        on:click=move |_| {
                            state.renaming.set(None);
                            list_collapsed.update(|value| *value = !*value);
                        }
                    >
                        <Show
                            when=move || list_collapsed.get()
                            fallback=move || view! {
                                <LxIcon icon=icondata::LuPanelLeftClose width="0.82rem" height="0.82rem" />
                            }
                        >
                            <LxIcon icon=icondata::LuPanelLeftOpen width="0.82rem" height="0.82rem" />
                        </Show>
                    </button>
                </form>
                <ul class="workbench-plans-manage__list">
                    <For
                        each=move || state.plans.get()
                        key=|p| p.path.clone()
                        children=move |plan: PlanMeta| {
                            view! {
                                <PlansListRow state=state plan=plan list_collapsed=list_collapsed />
                            }
                        }
                    />
                </ul>
            </aside>
            <section class="workbench-plans-manage__editor">
                <Show
                    when=move || state.active_path.get().is_some()
                    fallback=move || view! {
                        <div class="workbench-plans-manage__editor-empty">
                            <p>{move || i18n.tr(I18nKey::PlansSelectPlan)()}</p>
                        </div>
                    }
                >
                    <PlansEditorToolbar state=state wb=wb />
                    <Show
                        when=move || !state.show_preview.get()
                        fallback=move || view! {
                            <div
                                class="workbench-plans-manage__preview chat-md"
                                inner_html=move || render_markdown_to_html(
                                    &state.editor_content.get(),
                                )
                            ></div>
                        }
                    >
                        <textarea
                            class="workbench-plans-manage__textarea"
                            prop:value=move || state.editor_content.get()
                            on:input=move |ev| {
                                let target = ev
                                    .target()
                                    .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok());
                                if let Some(el) = target {
                                    state.editor_content.set(el.value());
                                    state.editor_dirty.set(true);
                                    if let Some(ws) = state.workspace_cwd.get_untracked() {
                                        schedule_save(state, ws);
                                    }
                                }
                            }
                        ></textarea>
                    </Show>
                </Show>
            </section>
        </div>
    }
}

#[component]
fn PlansEditorToolbar(state: PlansState, wb: WorkbenchService) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    let on_load = move |_| {
        let Some(path) = state.active_path.get_untracked() else {
            return;
        };
        let Some(ws) = state.workspace_cwd.get_untracked() else {
            return;
        };
        let Some(ws_id) = wb.active_id().get_untracked() else {
            return;
        };
        spawn_local(async move {
            match plan_load(&ws, &path).await {
                Ok(report) => {
                    // Attach plan to shared context.
                    let label = state
                        .plans
                        .get_untracked()
                        .into_iter()
                        .find(|m| m.path == report.path)
                        .map(|m| m.title)
                        .unwrap_or_else(|| report.path.clone());
                    let summary = format!(
                        "{} task(s) — kept {} free task(s)",
                        report.tasks_added, report.free_tasks_kept
                    );
                    let item = AgentContextItem {
                        id: format!("plan-file:{}", report.path),
                        kind: AgentContextKind::PlanFile,
                        label,
                        source: summary,
                        paths: vec![report.path.clone()],
                        added_at: Date::now() as i64,
                    };
                    wb.upsert_workspace_agent_context(ws_id, item);
                    // Refresh task snapshot for downstream rendering.
                    if let Ok(snap) = tauri_bridge::tasks_list(ws.clone()).await {
                        crate::workbench::agent_context_handoff::store_task_snapshot(ws_id, snap);
                    }
                    state.error.set(None);
                }
                Err(e) => state.error.set(Some(e)),
            }
        });
    };

    view! {
        <header class="workbench-plans-manage__toolbar">
            <span class="workbench-plans-manage__path">
                {move || state.active_path.get().unwrap_or_default()}
            </span>
            <Show when=move || state.is_index.get()>
                <span class="workbench-plans-manage__protected">
                    {move || i18n.tr(I18nKey::PlansProtectedIndex)()}
                </span>
            </Show>
            <span class="workbench-plans-manage__spacer"></span>
            <button
                type="button"
                class="workbench-plans-manage__btn"
                aria-label=move || if state.show_preview.get() {
                    i18n.tr(I18nKey::PlansEdit)()
                } else {
                    i18n.tr(I18nKey::PlansPreview)()
                }
                title=move || if state.show_preview.get() {
                    i18n.tr(I18nKey::PlansEdit)()
                } else {
                    i18n.tr(I18nKey::PlansPreview)()
                }
                on:click=move |_| state.show_preview.update(|v| *v = !*v)
            >
                <Show
                    when=move || state.show_preview.get()
                    fallback=move || view! {
                        <LxIcon icon=icondata::LuEye width="0.82rem" height="0.82rem" />
                    }
                >
                    <LxIcon icon=icondata::LuPencil width="0.82rem" height="0.82rem" />
                </Show>
            </button>
            <button
                type="button"
                class="workbench-plans-manage__btn workbench-plans-manage__btn--primary"
                on:click=on_load
            >
                {move || i18n.tr(I18nKey::PlansLoadIntoAgent)()}
            </button>
        </header>
    }
}

#[component]
fn PlanTaskStatChip(
    icon: icondata::Icon,
    count: u32,
    modifier: &'static str,
    label_key: I18nKey,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <span
            class=format!(
                "workbench-plans-manage__task-stat workbench-plans-manage__task-stat--{modifier}"
            )
            class:workbench-plans-manage__task-stat--zero=move || count == 0
            title=move || format!("{}: {count}", i18n.tr(label_key)())
        >
            <LxIcon icon=icon width="0.68rem" height="0.68rem" />
            <span class="workbench-plans-manage__task-stat-count">{count}</span>
        </span>
    }
}

#[component]
fn PlanTaskSummaryIcons(summary: PlanTaskSummaryWire) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <span
            class="workbench-plans-manage__task-summary"
            aria-label=move || i18n.tr(I18nKey::PlansTaskSummary)()
        >
            <PlanTaskStatChip
                icon=icondata::LuListTodo
                count=summary.total
                modifier="total"
                label_key=I18nKey::PlansTaskStatTotal
            />
            <PlanTaskStatChip
                icon=icondata::LuCircle
                count=summary.pending
                modifier="pending"
                label_key=I18nKey::PlansTaskStatPending
            />
            <PlanTaskStatChip
                icon=icondata::LuPlayCircle
                count=summary.in_progress
                modifier="in-progress"
                label_key=I18nKey::PlansTaskStatInProgress
            />
            <PlanTaskStatChip
                icon=icondata::LuAlertCircle
                count=summary.blocked
                modifier="blocked"
                label_key=I18nKey::PlansTaskStatBlocked
            />
            <PlanTaskStatChip
                icon=icondata::LuCheckCircle
                count=summary.completed
                modifier="completed"
                label_key=I18nKey::PlansTaskStatCompleted
            />
            <PlanTaskStatChip
                icon=icondata::LuMinusCircle
                count=summary.cancelled
                modifier="cancelled"
                label_key=I18nKey::PlansTaskStatCancelled
            />
        </span>
    }
}

#[component]
fn PlansListRow(
    state: PlansState,
    plan: PlanMeta,
    list_collapsed: RwSignal<bool>,
) -> impl IntoView {
    let plan_path_active = plan.path.clone();
    let plan_expanded = plan.clone();
    let plan_collapsed = plan.clone();

    view! {
        <li
            class="workbench-plans-manage__row"
            class:workbench-plans-manage__row--collapsed=move || list_collapsed.get()
            class:workbench-plans-manage__row--active=move || {
                state.active_path.get().as_deref() == Some(plan_path_active.as_str())
            }
            class:workbench-plans-manage__row--index=plan.is_index
        >
            <Show
                when=move || list_collapsed.get()
                fallback=move || view! {
                    <PlansListRowExpanded state=state plan=plan_expanded.clone() />
                }
            >
                <PlansListRowCollapsed state=state plan=plan_collapsed.clone() />
            </Show>
        </li>
    }
}

#[component]
fn PlansListRowCollapsed(state: PlansState, plan: PlanMeta) -> impl IntoView {
    let badge = plan_badge_text(&plan.title);
    view! {
        <button
            type="button"
            class="workbench-plans-manage__badge"
            title=plan.title.clone()
            aria-label=plan.title.clone()
            on:click=move |_| {
                if let Some(ws) = state.workspace_cwd.get_untracked() {
                    load_plan(state, ws, plan.path.clone());
                }
            }
        >
            {badge}
        </button>
    }
}

#[component]
fn PlansListRowExpanded(state: PlansState, plan: PlanMeta) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let plan_path_select = plan.path.clone();
    let plan_path_rename = plan.path.clone();
    let plan_path_delete = plan.path.clone();
    let plan_path_rename_match = plan.path.clone();
    let plan_path_for_rename_row = plan.path.clone();
    let title = plan.title.clone();
    let is_index = plan.is_index;
    let task_summary = plan.task_summary;

    view! {
        <button
            type="button"
            class="workbench-plans-manage__row-main"
            on:click=move |_| {
                if let Some(ws) = state.workspace_cwd.get_untracked() {
                    load_plan(state, ws, plan_path_select.clone());
                }
            }
        >
            <span class="workbench-plans-manage__row-title">{title}</span>
            <span class="workbench-plans-manage__row-summary">
                <PlanTaskSummaryIcons summary=task_summary />
            </span>
        </button>
        <Show when=move || !is_index>
            <button
                type="button"
                class="workbench-plans-manage__row-action"
                title=move || i18n.tr(I18nKey::PlansRename)()
                aria-label=move || i18n.tr(I18nKey::PlansRename)()
                on:click={
                    let path = plan_path_rename.clone();
                    move |_| {
                        state.renaming.set(Some(path.clone()));
                        state.rename_input.set(path.clone());
                    }
                }
            >
                <LxIcon icon=icondata::LuPencil width="0.78rem" height="0.78rem" />
            </button>
            <button
                type="button"
                class="workbench-plans-manage__row-action"
                title=move || i18n.tr(I18nKey::PlansDelete)()
                aria-label=move || i18n.tr(I18nKey::PlansDelete)()
                on:click={
                    let path = plan_path_delete.clone();
                    move |_| {
                        let Some(ws) = state.workspace_cwd.get_untracked() else { return };
                        let path = path.clone();
                        spawn_local(async move {
                            match plan_delete(&ws, &path).await {
                                Ok(()) => {
                                    if state.active_path.get_untracked().as_deref()
                                        == Some(path.as_str())
                                    {
                                        state.active_path.set(None);
                                        state.editor_content.set(String::new());
                                    }
                                    load_plans_list(state, ws);
                                }
                                Err(e) => state.error.set(Some(e)),
                            }
                        });
                    }
                }
            >
                <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
            </button>
        </Show>
        <Show when=move || {
            state.renaming.get().as_deref() == Some(plan_path_rename_match.as_str())
        }>
            <PlansRenameRow state=state old_path=plan_path_for_rename_row.clone() />
        </Show>
    }
}

fn plan_badge_text(title: &str) -> String {
    let mut out = String::new();
    for part in title
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        if let Some(ch) = part.chars().next() {
            out.extend(ch.to_uppercase());
        }
        if out.chars().count() >= 2 {
            break;
        }
    }
    if out.is_empty() {
        title
            .chars()
            .next()
            .map(|ch| ch.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into())
    } else {
        out
    }
}

#[component]
fn PlansRenameRow(state: PlansState, old_path: String) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let cancel = move |_| {
        state.renaming.set(None);
    };
    let submit = {
        let old_path = old_path.clone();
        move |ev: web_sys::SubmitEvent| {
            ev.prevent_default();
            let new_raw = state.rename_input.get_untracked();
            let new_path = if new_raw.to_lowercase().ends_with(".md") {
                new_raw
            } else {
                format!("{new_raw}.md")
            };
            if new_path.trim().is_empty() || new_path == old_path {
                state.renaming.set(None);
                return;
            }
            let Some(ws) = state.workspace_cwd.get_untracked() else {
                return;
            };
            let old_path = old_path.clone();
            spawn_local(async move {
                match plan_rename(&ws, &old_path, &new_path).await {
                    Ok(meta) => {
                        state.renaming.set(None);
                        load_plans_list(state, ws.clone());
                        load_plan(state, ws, meta.path);
                    }
                    Err(e) => state.error.set(Some(e)),
                }
            });
        }
    };
    view! {
        <form class="workbench-plans-manage__rename" on:submit=submit>
            <input
                type="text"
                class="workbench-plans-manage__rename-input"
                prop:value=move || state.rename_input.get()
                on:input=move |ev| {
                    if let Some(v) = input_value(ev) {
                        state.rename_input.set(v);
                    }
                }
            />
            <button type="submit" class="workbench-plans-manage__rename-btn">{move || i18n.tr(I18nKey::PlansRename)()}</button>
            <button type="button" class="workbench-plans-manage__rename-btn" on:click=cancel>"×"</button>
        </form>
    }
}
