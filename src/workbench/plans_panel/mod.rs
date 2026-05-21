//! Workspace plans panel rendered with the same expandable card flow as the
//! Rules tab: create, view, edit, rename, remove, and load plans from
//! `<workspace>/.agents/plans/` into the BLXCode Agent.

use crate::agent_wire::{AgentContextItem, AgentContextKind};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    self, plan_create, plan_delete, plan_list, plan_load, plan_read, plan_rename, plan_write,
    PlanContent, PlanMeta, PlanTaskSummaryWire,
};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::{RightPanelTab, WorkbenchService};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy)]
struct PlansState {
    workspace_cwd: RwSignal<Option<String>>,
    plans: RwSignal<Vec<PlanMeta>>,
    loading: RwSignal<bool>,
    error: RwSignal<Option<String>>,
}

impl PlansState {
    fn new() -> Self {
        Self {
            workspace_cwd: RwSignal::new(None),
            plans: RwSignal::new(Vec::new()),
            loading: RwSignal::new(false),
            error: RwSignal::new(None),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlanBucket {
    Blocked,
    InProgress,
    Pending,
    Completed,
    Cancelled,
    Empty,
}

impl PlanBucket {
    const ALL: [Self; 6] = [
        Self::Blocked,
        Self::InProgress,
        Self::Pending,
        Self::Completed,
        Self::Cancelled,
        Self::Empty,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::Blocked => "blocked",
            Self::InProgress => "in-progress",
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Empty => "empty",
        }
    }

    fn icon(self) -> icondata::Icon {
        match self {
            Self::Blocked => icondata::LuCircleAlert,
            Self::InProgress => icondata::LuCirclePlay,
            Self::Pending => icondata::LuCircle,
            Self::Completed => icondata::LuCircleCheck,
            Self::Cancelled => icondata::LuCircleMinus,
            Self::Empty => icondata::LuCircleDashed,
        }
    }

    fn label_key(self) -> I18nKey {
        match self {
            Self::Blocked => I18nKey::PlansTaskStatBlocked,
            Self::InProgress => I18nKey::PlansTaskStatInProgress,
            Self::Pending => I18nKey::PlansTaskStatPending,
            Self::Completed => I18nKey::PlansTaskStatCompleted,
            Self::Cancelled => I18nKey::PlansTaskStatCancelled,
            Self::Empty => I18nKey::PlansFilterEmpty,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlanFilter {
    All,
    Active,
    Blocked,
    Pending,
    Completed,
    Cancelled,
    Empty,
}

impl PlanFilter {
    const ALL: [Self; 7] = [
        Self::All,
        Self::Active,
        Self::Blocked,
        Self::Pending,
        Self::Completed,
        Self::Cancelled,
        Self::Empty,
    ];

    fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Active => "active",
            Self::Blocked => "blocked",
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Empty => "empty",
        }
    }

    fn icon(self) -> icondata::Icon {
        match self {
            Self::All => icondata::LuLayers,
            Self::Active => icondata::LuCirclePlay,
            Self::Blocked => icondata::LuCircleAlert,
            Self::Pending => icondata::LuCircle,
            Self::Completed => icondata::LuCircleCheck,
            Self::Cancelled => icondata::LuCircleMinus,
            Self::Empty => icondata::LuCircleDashed,
        }
    }

    fn label_key(self) -> I18nKey {
        match self {
            Self::All => I18nKey::PlansFilterAll,
            Self::Active => I18nKey::PlansFilterActive,
            Self::Blocked => I18nKey::PlansTaskStatBlocked,
            Self::Pending => I18nKey::PlansTaskStatPending,
            Self::Completed => I18nKey::PlansTaskStatCompleted,
            Self::Cancelled => I18nKey::PlansTaskStatCancelled,
            Self::Empty => I18nKey::PlansFilterEmpty,
        }
    }
}

#[derive(Clone)]
struct PlanGroup {
    bucket: PlanBucket,
    plans: Vec<PlanMeta>,
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
    state.loading.set(true);
    spawn_local(async move {
        match plan_list(&ws).await {
            Ok(list) => {
                state.plans.set(list);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
        state.loading.set(false);
    });
}

fn plan_bucket(summary: &PlanTaskSummaryWire) -> PlanBucket {
    if summary.blocked > 0 {
        PlanBucket::Blocked
    } else if summary.in_progress > 0 {
        PlanBucket::InProgress
    } else if summary.pending > 0 {
        PlanBucket::Pending
    } else if summary.completed > 0 {
        PlanBucket::Completed
    } else if summary.cancelled > 0 {
        PlanBucket::Cancelled
    } else {
        PlanBucket::Empty
    }
}

fn filter_matches(filter: PlanFilter, plan: &PlanMeta) -> bool {
    let bucket = plan_bucket(&plan.task_summary);
    match filter {
        PlanFilter::All => true,
        PlanFilter::Active => matches!(bucket, PlanBucket::InProgress | PlanBucket::Pending),
        PlanFilter::Blocked => bucket == PlanBucket::Blocked,
        PlanFilter::Pending => bucket == PlanBucket::Pending,
        PlanFilter::Completed => bucket == PlanBucket::Completed,
        PlanFilter::Cancelled => bucket == PlanBucket::Cancelled,
        PlanFilter::Empty => bucket == PlanBucket::Empty,
    }
}

fn filter_count(filter: PlanFilter, plans: &[PlanMeta]) -> usize {
    plans
        .iter()
        .filter(|plan| filter_matches(filter, plan))
        .count()
}

fn grouped_plans(plans: &[PlanMeta]) -> Vec<PlanGroup> {
    PlanBucket::ALL
        .into_iter()
        .filter_map(|bucket| {
            let grouped = plans
                .iter()
                .filter(|plan| plan_bucket(&plan.task_summary) == bucket)
                .cloned()
                .collect::<Vec<_>>();
            (!grouped.is_empty()).then_some(PlanGroup {
                bucket,
                plans: grouped,
            })
        })
        .collect()
}

fn open_plan_composer(
    composer_open: RwSignal<bool>,
    draft_error: RwSignal<Option<String>>,
) {
    composer_open.set(true);
    draft_error.set(None);
    spawn_local(async move {
        TimeoutFuture::new(0).await;
        scroll_plans_body_to_top();
    });
}

fn scroll_plans_body_to_top() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Ok(Some(node)) = document.query_selector(".blx-plans-pane .blx-sr-pane__body") else {
        return;
    };
    if let Ok(el) = node.dyn_into::<web_sys::HtmlElement>() {
        el.set_scroll_top(0);
    }
}

#[component]
pub fn PlansPanel() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let state = PlansState::new();
    let active_tab = wb.right_active_tab();
    let active_id = wb.active_id();
    let filter = RwSignal::new(PlanFilter::All);
    let composer_open = RwSignal::new(false);
    let draft_title = RwSignal::new(String::new());
    let draft_body = RwSignal::new(String::new());
    let draft_error = RwSignal::<Option<String>>::new(None);
    let saving = RwSignal::new(false);

    Effect::new(move |_| {
        if active_tab.get() != RightPanelTab::Plans {
            return;
        }
        let cwd = current_workspace_cwd(wb);
        let prev = state.workspace_cwd.get_untracked();
        let _ = active_id.get();
        if cwd != prev {
            state.workspace_cwd.set(cwd.clone());
            state.plans.set(Vec::new());
        }
        if let Some(ws) = cwd {
            load_plans_list(state, ws);
        } else {
            state.error.set(None);
            state.loading.set(false);
        }
    });

    let reset_composer = move || {
        draft_title.set(String::new());
        draft_body.set(String::new());
        draft_error.set(None);
        composer_open.set(false);
    };

    let preview_name = Signal::derive(move || {
        let existing = state.plans.get();
        next_plan_name(&draft_title.get(), &existing)
    });

    let submit_new_plan = move |_| {
        let title = draft_title.get().trim().to_owned();
        if title.is_empty() {
            draft_error.set(Some(i18n.tr(I18nKey::PlansTitleRequired)().to_string()));
            return;
        }
        let Some(ws) = state.workspace_cwd.get_untracked() else {
            state
                .error
                .set(Some(i18n.tr(I18nKey::SrNoWorkspace)().to_string()));
            return;
        };
        let body = draft_body.get();
        let content = normalize_plan_content(&title, &body);
        let path = preview_name.get();
        saving.set(true);
        spawn_local(async move {
            match plan_create(&ws, &path, Some(&content)).await {
                Ok(_) => {
                    reset_composer();
                    load_plans_list(state, ws);
                }
                Err(e) => {
                    draft_error.set(Some(e.clone()));
                    state.error.set(Some(e));
                }
            }
            saving.set(false);
        });
    };

    let visible_plans = Signal::derive(move || {
        let mode = filter.get();
        state.plans.with(|plans| {
            plans
                .iter()
                .filter(|p| filter_matches(mode, p))
                .cloned()
                .collect()
        })
    });
    let visible_groups = Signal::derive(move || grouped_plans(&state.plans.get()));

    view! {
        <div class="blx-sr-pane blx-plans-pane" role="region" aria-label=move || i18n.tr(I18nKey::TabPlans)()>
            <header class="blx-sr-pane__header">
                <div class="blx-sr-pane__title-wrap">
                    <span class="blx-sr-pane__title-icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuClipboardList width="14px" height="14px" />
                    </span>
                    <h2 class="blx-sr-pane__title">{i18n.tr(I18nKey::TabPlans)}</h2>
                </div>
                <div class="blx-sr-pane__actions">
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--primary blx-sr-btn--icon"
                        aria-label=move || i18n.tr(I18nKey::PlansNewPlan)()
                        title=move || i18n.tr(I18nKey::PlansNewPlan)()
                        on:click=move |_| {
                            open_plan_composer(composer_open, draft_error);
                        }
                    >
                        <LxIcon icon=icondata::LuPlus width="13px" height="13px" />
                    </button>
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--icon"
                        aria-label=move || i18n.tr(I18nKey::PlansRefresh)()
                        title=move || i18n.tr(I18nKey::PlansRefresh)()
                        on:click=move |_| {
                            if let Some(ws) = state.workspace_cwd.get_untracked() {
                                load_plans_list(state, ws);
                            }
                        }
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="13px" height="13px" />
                    </button>
                </div>
            </header>

            <div class="blx-sr-tabs blx-plans-tabs" role="tablist">
                <For
                    each=move || PlanFilter::ALL
                    key=|mode| mode.key()
                    children=move |mode| view! {
                        <button
                            type="button"
                            role="tab"
                            class="blx-sr-tab"
                            class:blx-sr-tab--active=move || filter.get() == mode
                            aria-selected=move || if filter.get() == mode { "true" } else { "false" }
                            on:click=move |_| filter.set(mode)
                        >
                            <LxIcon icon=mode.icon() width="13px" height="13px" />
                            <span>{i18n.tr(mode.label_key())}</span>
                            <span class="blx-sr-tab__count">
                                {move || state.plans.with(|plans| filter_count(mode, plans))}
                            </span>
                        </button>
                    }
                />
            </div>

            <div class="blx-sr-pane__body">
                {move || state.error.get().map(|e| view! { <p class="blx-sr-pane__err">{e}</p> })}
                <Show
                    when=move || state.workspace_cwd.get().is_some()
                    fallback=move || view! {
                        <div class="blx-sr-empty">
                            <span class="blx-sr-empty__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuClipboardList width="22px" height="22px" />
                            </span>
                            <p class="blx-sr-empty__text">{i18n.tr(I18nKey::PlansEmptyLead)}</p>
                        </div>
                    }
                >
                    {move || composer_open.get().then(|| {
                        let is_saving = saving.get();
                        view! {
                            <article class="blx-sr-card blx-sr-card--open blx-sr-card--composer blx-plans-card">
                                <div class="blx-sr-card__row blx-sr-card__row--static">
                                    <span class="blx-sr-card__chevron" aria-hidden="true">
                                        <LxIcon icon=icondata::LuSparkles width="14px" height="14px" />
                                    </span>
                                    <span class="blx-sr-card__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuClipboardPlus width="16px" height="16px" />
                                    </span>
                                    <span class="blx-sr-card__main">
                                        <span class="blx-sr-card__title-row">
                                            <input
                                                class="blx-sr-card__title-input"
                                                type="text"
                                                placeholder=move || i18n.tr(I18nKey::PlansTitlePh)()
                                                prop:value=move || draft_title.get()
                                                on:input=move |ev| {
                                                    draft_title.set(input_value(&ev));
                                                    draft_error.set(None);
                                                }
                                            />
                                            <span class="blx-sr-card__badge" data-kind="plan">{i18n.tr(I18nKey::SrRuleBadgeNew)}</span>
                                        </span>
                                        <span class="blx-sr-card__summary">{move || preview_name.get()}</span>
                                    </span>
                                </div>

                                <section class="blx-sr-card__body">
                                    <textarea
                                        class="blx-sr-card__editor"
                                        spellcheck="false"
                                        placeholder=move || i18n.tr(I18nKey::PlansBodyPh)()
                                        prop:value=move || draft_body.get()
                                        on:input=move |ev| draft_body.set(textarea_value(&ev))
                                    ></textarea>
                                    {move || draft_error.get().map(|e| view! {
                                        <p class="blx-sr-card__error">{e}</p>
                                    })}
                                    <div class="blx-sr-card__actions">
                                        <button
                                            type="button"
                                            class="blx-sr-btn blx-sr-btn--ghost"
                                            disabled=is_saving
                                            on:click=move |_| reset_composer()
                                        >
                                            <span>{i18n.tr(I18nKey::SrCancel)}</span>
                                        </button>
                                        <button
                                            type="button"
                                            class="blx-sr-btn blx-sr-btn--primary"
                                            disabled=is_saving
                                            on:click=submit_new_plan
                                        >
                                            <LxIcon icon=icondata::LuSave width="13px" height="13px" />
                                            <span>{i18n.tr(I18nKey::BtnSave)}</span>
                                        </button>
                                    </div>
                                </section>
                            </article>
                        }
                    })}

                    {move || {
                        if state.loading.get() && state.plans.with(|plans| plans.is_empty()) {
                            view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrLoading)}</p> }.into_any()
                        } else if state.plans.with(|plans| plans.is_empty()) && !composer_open.get() {
                            view! {
                                <div class="blx-sr-empty">
                                    <span class="blx-sr-empty__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuClipboardPlus width="22px" height="22px" />
                                    </span>
                                    <p class="blx-sr-empty__text">{i18n.tr(I18nKey::PlansEmptyTitle)}</p>
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--primary"
                                        on:click=move |_| open_plan_composer(composer_open, draft_error)
                                    >
                                        <LxIcon icon=icondata::LuPlus width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::PlansNewPlan)}</span>
                                    </button>
                                </div>
                            }.into_any()
                        } else if filter.get() == PlanFilter::All {
                            view! {
                                <For
                                    each=move || visible_groups.get()
                                    key=|group| group.bucket.key()
                                    children=move |group| view! {
                                        <PlanGroupView state=state group=group />
                                    }
                                />
                            }.into_any()
                        } else {
                            let visible: Vec<PlanMeta> = visible_plans.get();
                            if visible.is_empty() {
                                view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::PlansNoFilteredPlans)}</p> }.into_any()
                            } else {
                                view! {
                                    <For
                                        each=move || visible_plans.get()
                                        key=|plan| plan.path.clone()
                                        children=move |plan| view! { <PlanCard state=state plan=plan /> }
                                    />
                                }.into_any()
                            }
                        }
                    }}
                </Show>
            </div>
        </div>
    }
}

#[component]
fn PlanGroupView(state: PlansState, group: PlanGroup) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let bucket = group.bucket;
    let plans = group.plans;
    view! {
        <section class="blx-plans-group" data-state=bucket.key()>
            <header class="blx-plans-group__header">
                <span class="blx-plans-group__icon" aria-hidden="true">
                    <LxIcon icon=bucket.icon() width="13px" height="13px" />
                </span>
                <span class="blx-plans-group__title">{i18n.tr(bucket.label_key())}</span>
                <span class="blx-plans-group__count">{plans.len()}</span>
            </header>
            <div class="blx-plans-group__cards">
                <For
                    each=move || plans.clone()
                    key=|plan| plan.path.clone()
                    children=move |plan| view! { <PlanCard state=state plan=plan /> }
                />
            </div>
        </section>
    }
}

#[component]
fn PlanCard(state: PlansState, plan: PlanMeta) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let path = plan.path.clone();
    let path_for_read = path.clone();
    let path_for_save = path.clone();
    let path_for_delete = path.clone();
    let path_for_load = path.clone();
    let title = plan.title.clone();
    let summary = plan.task_summary.clone();
    let is_index = plan.is_index;
    let bucket = plan_bucket(&summary);
    let card_path = StoredValue::new(path.clone());

    let expanded = RwSignal::new(false);
    let editing = RwSignal::new(false);
    let renaming = RwSignal::new(false);
    let rename_input = RwSignal::new(path.clone());
    let body = RwSignal::<Option<String>>::new(None);
    let draft = RwSignal::new(String::new());
    let body_loading = RwSignal::new(false);
    let saving = RwSignal::new(false);

    let on_toggle_card = move |_| {
        let next = !expanded.get();
        expanded.set(next);
        if next && body.with(|b| b.is_none()) && !body_loading.get() {
            read_plan_into(state, path_for_read.clone(), body, body_loading);
        }
    };

    let on_save = StoredValue::new(path_for_save);
    let on_delete = StoredValue::new(path_for_delete);
    let on_load = StoredValue::new(path_for_load);

    view! {
        <article
            class="blx-sr-card blx-plans-card"
            class:blx-sr-card--open=move || expanded.get()
            data-state=bucket.key()
        >
            <div class="blx-sr-card__row blx-plans-card__row">
                <button
                    type="button"
                    class="blx-plans-card__row-main"
                    aria-expanded=move || if expanded.get() { "true" } else { "false" }
                    on:click=on_toggle_card
                >
                    <span class="blx-sr-card__chevron" aria-hidden="true">
                        <LxIcon icon=icondata::LuChevronRight width="14px" height="14px" />
                    </span>
                    <span class="blx-sr-card__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuClipboardList width="16px" height="16px" />
                    </span>
                    <span class="blx-sr-card__main">
                        <span class="blx-sr-card__title-row">
                            <span class="blx-sr-card__title">{title}</span>
                            <span class="blx-sr-card__badge" data-kind="plan">"plan"</span>
                            {is_index.then(|| view! {
                                <span class="blx-plans-card__protected">{i18n.tr(I18nKey::PlansProtectedIndex)}</span>
                            })}
                        </span>
                        <span class="blx-sr-card__summary blx-plans-card__summary">
                            <span class="blx-plans-card__path">{path.clone()}</span>
                            <PlanTaskSummaryIcons summary=summary.clone() />
                        </span>
                    </span>
                </button>
                <button
                    type="button"
                    class="blx-sr-btn blx-sr-btn--icon blx-plans-card__edit-toggle"
                    disabled=move || body_loading.get() || saving.get()
                    aria-label=move || {
                        if editing.get() {
                            i18n.tr(I18nKey::PlansPreview)()
                        } else {
                            i18n.tr(I18nKey::PlansEdit)()
                        }
                    }
                    title=move || {
                        if editing.get() {
                            i18n.tr(I18nKey::PlansPreview)()
                        } else {
                            i18n.tr(I18nKey::PlansEdit)()
                        }
                    }
                    on:click=move |ev: web_sys::MouseEvent| {
                        ev.stop_propagation();
                        toggle_plan_edit_from_header(state, card_path.get_value(), expanded, editing, body, draft, body_loading);
                    }
                >
                    {move || {
                        if editing.get() {
                            view! { <LxIcon icon=icondata::LuEye width="13px" height="13px" /> }.into_any()
                        } else {
                            view! { <LxIcon icon=icondata::LuPencil width="13px" height="13px" /> }.into_any()
                        }
                    }}
                </button>
            </div>

            {move || expanded.get().then(|| {
                let is_loading = body_loading.get();
                let is_saving = saving.get();
                let has_body = body.with(|b| b.is_some());
                let is_editing = editing.get();
                let is_renaming = renaming.get();
                view! {
                    <section class="blx-sr-card__body">
                        {if is_loading && !has_body {
                            view! { <p class="blx-sr-card__hint">{i18n.tr(I18nKey::SrLoading)}</p> }.into_any()
                        } else if is_editing {
                            view! {
                                <textarea
                                    class="blx-sr-card__editor"
                                    spellcheck="false"
                                    prop:value=move || draft.get()
                                    on:input=move |ev| draft.set(textarea_value(&ev))
                                ></textarea>
                            }.into_any()
                        } else {
                            view! {
                                <div
                                    class="blx-sr-card__md"
                                    inner_html=move || body.with(|b| {
                                        b.as_deref().map(render_markdown_to_html).unwrap_or_default()
                                    })
                                />
                            }.into_any()
                        }}

                        {is_renaming.then(|| view! {
                            <form
                                class="blx-plans-card__rename"
                                on:submit=move |ev: web_sys::SubmitEvent| {
                                    ev.prevent_default();
                                    submit_rename(state, card_path.get_value(), rename_input.get_untracked(), renaming);
                                }
                            >
                                <input
                                    type="text"
                                    class="blx-plans-card__rename-input"
                                    prop:value=move || rename_input.get()
                                    on:input=move |ev| rename_input.set(input_value(&ev))
                                />
                                <button type="submit" class="blx-sr-btn blx-sr-btn--primary">
                                    <span>{i18n.tr(I18nKey::PlansRename)}</span>
                                </button>
                                <button
                                    type="button"
                                    class="blx-sr-btn blx-sr-btn--ghost"
                                    on:click=move |_| renaming.set(false)
                                >
                                    <span>{i18n.tr(I18nKey::SrCancel)}</span>
                                </button>
                            </form>
                        })}

                        <div class="blx-sr-card__actions">
                            {if is_editing {
                                view! {
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--ghost"
                                        disabled=is_saving
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            draft.set(body.with(|b| b.clone().unwrap_or_default()));
                                            editing.set(false);
                                        }
                                    >
                                        <span>{i18n.tr(I18nKey::SrCancel)}</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--primary"
                                        disabled=is_saving
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            write_plan_body(
                                                state,
                                                on_save.get_value(),
                                                draft.get_untracked(),
                                                body,
                                                editing,
                                                saving,
                                            );
                                        }
                                    >
                                        <LxIcon icon=icondata::LuSave width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::BtnSave)}</span>
                                    </button>
                                }.into_any()
                            } else {
                                view! {
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--primary"
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            load_plan_into_agent(state, wb, on_load.get_value());
                                        }
                                    >
                                        <LxIcon icon=icondata::LuBot width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::PlansLoadIntoAgent)}</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="blx-sr-btn"
                                        disabled=is_index
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            rename_input.set(card_path.get_value());
                                            renaming.update(|open| *open = !*open);
                                        }
                                    >
                                        <LxIcon icon=icondata::LuFilePenLine width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::PlansRename)}</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--danger"
                                        disabled=is_index
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            let msg = i18n.tr(I18nKey::SrConfirmRemove)();
                                            if confirm_window(&msg) {
                                                remove_plan(state, on_delete.get_value());
                                            }
                                        }
                                    >
                                        <LxIcon icon=icondata::LuTrash2 width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::SrRemove)}</span>
                                    </button>
                                }.into_any()
                            }}
                        </div>
                    </section>
                }
            })}
            <PlanStateLine summary=summary />
        </article>
    }
}

fn read_plan_into(
    state: PlansState,
    path: String,
    body: RwSignal<Option<String>>,
    loading: RwSignal<bool>,
) {
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    loading.set(true);
    spawn_local(async move {
        match plan_read(&ws, &path).await {
            Ok(PlanContent { content, .. }) => {
                body.set(Some(content));
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
        loading.set(false);
    });
}

fn toggle_plan_edit_from_header(
    state: PlansState,
    path: String,
    expanded: RwSignal<bool>,
    editing: RwSignal<bool>,
    body: RwSignal<Option<String>>,
    draft: RwSignal<String>,
    loading: RwSignal<bool>,
) {
    expanded.set(true);
    if editing.get_untracked() {
        draft.set(body.with_untracked(|b| b.clone().unwrap_or_default()));
        editing.set(false);
        return;
    }
    if let Some(content) = body.get_untracked() {
        draft.set(content);
        editing.set(true);
        return;
    }
    if loading.get_untracked() {
        return;
    }
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    loading.set(true);
    spawn_local(async move {
        match plan_read(&ws, &path).await {
            Ok(PlanContent { content, .. }) => {
                draft.set(content.clone());
                body.set(Some(content));
                editing.set(true);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
        loading.set(false);
    });
}

fn write_plan_body(
    state: PlansState,
    path: String,
    content: String,
    body: RwSignal<Option<String>>,
    editing: RwSignal<bool>,
    saving: RwSignal<bool>,
) {
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    saving.set(true);
    spawn_local(async move {
        match plan_write(&ws, &path, &content).await {
            Ok(_) => {
                body.set(Some(content));
                editing.set(false);
                state.error.set(None);
                load_plans_list(state, ws);
            }
            Err(e) => state.error.set(Some(e)),
        }
        saving.set(false);
    });
}

fn submit_rename(state: PlansState, old_path: String, raw: String, renaming: RwSignal<bool>) {
    let new_path = normalize_plan_path(&raw);
    if new_path.trim().is_empty() || new_path == old_path {
        renaming.set(false);
        return;
    }
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    spawn_local(async move {
        match plan_rename(&ws, &old_path, &new_path).await {
            Ok(_) => {
                renaming.set(false);
                state.error.set(None);
                load_plans_list(state, ws);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn remove_plan(state: PlansState, path: String) {
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    spawn_local(async move {
        match plan_delete(&ws, &path).await {
            Ok(()) => {
                state.error.set(None);
                load_plans_list(state, ws);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn load_plan_into_agent(state: PlansState, wb: WorkbenchService, path: String) {
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    let Some(ws_id) = wb.active_id().get_untracked() else {
        return;
    };
    spawn_local(async move {
        match plan_load(&ws, &path).await {
            Ok(report) => {
                let label = state
                    .plans
                    .get_untracked()
                    .into_iter()
                    .find(|m| m.path == report.path)
                    .map(|m| m.title)
                    .unwrap_or_else(|| report.path.clone());
                let summary = format!(
                    "{} task(s) - kept {} free task(s)",
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
                if let Ok(snap) = tauri_bridge::tasks_list(ws.clone()).await {
                    crate::workbench::agent_context_handoff::store_task_snapshot(ws_id, snap);
                }
                state.error.set(None);
                load_plans_list(state, ws);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
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
            class=format!("blx-plans-task-stat blx-plans-task-stat--{modifier}")
            class:blx-plans-task-stat--zero=move || count == 0
            title=move || format!("{}: {count}", i18n.tr(label_key)())
        >
            <LxIcon icon=icon width="12px" height="12px" />
            <span>{count}</span>
        </span>
    }
}

#[component]
fn PlanTaskSummaryIcons(summary: PlanTaskSummaryWire) -> impl IntoView {
    view! {
        <span class="blx-plans-task-summary">
            <PlanTaskStatChip icon=icondata::LuListTodo count=summary.total modifier="total" label_key=I18nKey::PlansTaskStatTotal />
            <PlanTaskStatChip icon=icondata::LuCircle count=summary.pending modifier="pending" label_key=I18nKey::PlansTaskStatPending />
            <PlanTaskStatChip icon=icondata::LuCirclePlay count=summary.in_progress modifier="in-progress" label_key=I18nKey::PlansTaskStatInProgress />
            <PlanTaskStatChip icon=icondata::LuCircleAlert count=summary.blocked modifier="blocked" label_key=I18nKey::PlansTaskStatBlocked />
            <PlanTaskStatChip icon=icondata::LuCircleCheck count=summary.completed modifier="completed" label_key=I18nKey::PlansTaskStatCompleted />
            <PlanTaskStatChip icon=icondata::LuCircleMinus count=summary.cancelled modifier="cancelled" label_key=I18nKey::PlansTaskStatCancelled />
        </span>
    }
}

#[component]
fn PlanStateLine(summary: PlanTaskSummaryWire) -> impl IntoView {
    let pending = state_line_class("pending", summary.pending > 0);
    let in_progress = state_line_class("in-progress", summary.in_progress > 0);
    let blocked = state_line_class("blocked", summary.blocked > 0);
    let completed = state_line_class("completed", summary.completed > 0);
    let cancelled = state_line_class("cancelled", summary.cancelled > 0);
    view! {
        <div class="blx-plans-state-line" aria-hidden="true">
            <span class=pending></span>
            <span class=in_progress></span>
            <span class=blocked></span>
            <span class=completed></span>
            <span class=cancelled></span>
        </div>
    }
}

fn state_line_class(kind: &str, active: bool) -> String {
    let mut class = format!("blx-plans-state-line__seg blx-plans-state-line__seg--{kind}");
    if active {
        class.push_str(" blx-plans-state-line__seg--active");
    }
    class
}

fn normalize_plan_content(title: &str, body: &str) -> String {
    if body.trim().is_empty() {
        format!("# {title}\n\n## Tasks\n\n")
    } else if body.trim_start().starts_with('#') {
        body.to_owned()
    } else {
        format!("# {title}\n\n{body}")
    }
}

fn normalize_plan_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.to_ascii_lowercase().ends_with(".md") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}.md")
    }
}

fn next_plan_name(title: &str, existing: &[PlanMeta]) -> String {
    let base = slugify_title(title);
    let mut name = format!("{base}.md");
    let mut i = 2;
    while existing.iter().any(|plan| plan.path == name) {
        name = format!("{base}-{i}.md");
        i += 1;
    }
    name
}

fn slugify_title(title: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in title.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || lower == '_' {
            slug.push(lower);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "new-plan".into()
    } else {
        slug.chars().take(80).collect()
    }
}

fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

fn textarea_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

fn confirm_window(msg: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(msg).ok())
        .unwrap_or(true)
}
