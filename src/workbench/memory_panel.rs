//! Workspace-scoped Markdown memory panel — files/editor, backlinks,
//! graph view, search, agent-pointer installer. Mirrors the Phase 1–5
//! design discussed for blxcode's Obsidian-style memory feature.
use crate::agent_wire::{AgentContextItem, AgentContextKind};
use crate::i18n::I18nKey;
use crate::memory_paths::slug_to_filename;
use crate::service::I18nService;
use crate::tauri_bridge::{
    self, note_key, BacklinkRef, GraphData, MemoryScope, MemoryStatusResponse, NoteContent,
    NoteMeta, PointerResult, SearchHit,
};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::memory_graph::{navigate_to_graph_node, MemoryGraphView};
use crate::workbench::state::{normalize_hex_color, MemoryCategorySettings};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

const SAVE_DEBOUNCE_MS: u32 = 600;
const LEARNINGS_API_PREFIX: &str = "learnings/";
const LEARNINGS_INDEX_PATHS: &[&str] = &["learnings/README.md"];
/// Built-in pseudo categories — top-level memory files and the learnings root.
const CATEGORY_MEMORY: &str = "memory";
const CATEGORY_LEARNINGS: &str = "learnings";
const POINTER_AGENTS: &[PointerAgent] = &[
    PointerAgent {
        id: "claude",
        label: "Claude",
        target: "CLAUDE.md",
        icon: "/public/brand-icons/anthropic.svg",
    },
    PointerAgent {
        id: "codex",
        label: "Codex",
        target: "AGENTS.md",
        icon: "/public/brand-icons/openai.svg",
    },
    PointerAgent {
        id: "gemini",
        label: "Gemini",
        target: "GEMINI.md",
        icon: "/public/brand-icons/gemini.svg",
    },
    PointerAgent {
        id: "cursor",
        label: "Cursor",
        target: ".cursorrules",
        icon: "/public/brand-icons/cursor.svg",
    },
    PointerAgent {
        id: "opencode",
        label: "OpenCode",
        target: "AGENTS.md",
        icon: "/public/brand-icons/opencode.svg",
    },
];

#[derive(Clone, Copy)]
struct PointerAgent {
    id: &'static str,
    label: &'static str,
    target: &'static str,
    icon: &'static str,
}

/// Derive the category key for a given note API path.
fn category_for_path(path: &str) -> String {
    if path.starts_with(LEARNINGS_API_PREFIX) {
        return CATEGORY_LEARNINGS.to_string();
    }
    if let Some((head, _)) = path.split_once('/') {
        if !head.is_empty() {
            return head.to_string();
        }
    }
    CATEGORY_MEMORY.to_string()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemoryView {
    Files,
    Graph,
    Search,
}

#[derive(Clone, Copy)]
pub(crate) struct MemoryState {
    pub(crate) workspace_cwd: RwSignal<Option<String>>,
    pub(crate) notes: RwSignal<Vec<NoteMeta>>,
    pub(crate) active_path: RwSignal<Option<String>>,
    pub(crate) active_scope: RwSignal<MemoryScope>,
    pub(crate) editor_content: RwSignal<String>,
    pub(crate) editor_dirty: RwSignal<bool>,
    pub(crate) show_preview: RwSignal<bool>,
    pub(crate) backlinks: RwSignal<Vec<BacklinkRef>>,
    pub(crate) view: RwSignal<MemoryView>,
    pub(crate) error: RwSignal<Option<String>>,
    pub(crate) save_token: RwSignal<u32>,
    pub(crate) graph: RwSignal<Option<GraphData>>,
    pub(crate) search_query: RwSignal<String>,
    pub(crate) search_results: RwSignal<Vec<SearchHit>>,
    pub(crate) memory_status: RwSignal<Option<MemoryStatusResponse>>,
    pub(crate) pointer_status: RwSignal<Option<Vec<PointerResult>>>,
    pub(crate) pointers_open: RwSignal<bool>,
    pub(crate) pointers_notice_dismissed: RwSignal<bool>,
    pub(crate) pointers_busy: RwSignal<bool>,
    pub(crate) selected_pointer_agents: RwSignal<HashSet<String>>,
    pub(crate) global_readme_preview: RwSignal<Option<String>>,
    /// Expanded category groups in the Files sidebar (keys: plain for workspace, "global:cat" for global).
    pub(crate) groups_open: RwSignal<HashSet<String>>,
    /// User-created workspace categories that have no notes yet.
    pub(crate) empty_categories: RwSignal<Vec<String>>,
    /// User-created global categories that have no notes yet.
    pub(crate) global_subcategories: RwSignal<Vec<String>>,
    /// True once global memory folders are known to exist.
    pub(crate) global_bootstrapped: RwSignal<bool>,
    /// Selected graph node (API path); shared across Graph / Search / Files.
    pub(crate) graph_selected_node: RwSignal<Option<String>>,
    /// Bumped to re-run fly-to animation for the current selection.
    pub(crate) graph_focus_generation: RwSignal<u32>,
    /// When true, Graph tab should prefer 3D mode (e.g. jump from Search).
    pub(crate) graph_prefer_3d: RwSignal<bool>,
}

impl MemoryState {
    fn new() -> Self {
        Self {
            workspace_cwd: RwSignal::new(None),
            notes: RwSignal::new(Vec::new()),
            active_path: RwSignal::new(None),
            active_scope: RwSignal::new(MemoryScope::Workspace),
            editor_content: RwSignal::new(String::new()),
            editor_dirty: RwSignal::new(false),
            show_preview: RwSignal::new(false),
            backlinks: RwSignal::new(Vec::new()),
            view: RwSignal::new(MemoryView::Files),
            error: RwSignal::new(None),
            save_token: RwSignal::new(0),
            graph: RwSignal::new(None),
            search_query: RwSignal::new(String::new()),
            search_results: RwSignal::new(Vec::new()),
            memory_status: RwSignal::new(None),
            pointer_status: RwSignal::new(None),
            pointers_open: RwSignal::new(false),
            pointers_notice_dismissed: RwSignal::new(false),
            pointers_busy: RwSignal::new(false),
            selected_pointer_agents: RwSignal::new(HashSet::new()),
            global_readme_preview: RwSignal::new(None),
            groups_open: RwSignal::new(HashSet::new()),
            empty_categories: RwSignal::new(Vec::new()),
            global_subcategories: RwSignal::new(Vec::new()),
            global_bootstrapped: RwSignal::new(false),
            graph_selected_node: RwSignal::new(None),
            graph_focus_generation: RwSignal::new(0),
            graph_prefer_3d: RwSignal::new(false),
        }
    }
}

/// Ensure the Files sidebar group containing `path` is expanded.
pub(crate) fn expand_files_group_for_path(state: MemoryState, scope: &MemoryScope, path: &str) {
    let cat = category_for_path(path);
    let key = match scope {
        MemoryScope::Global => format!("global:{cat}"),
        MemoryScope::Workspace => cat,
    };
    state.groups_open.update(|open| {
        open.insert(key);
    });
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

fn load_notes(state: MemoryState, ws: String) {
    spawn_local(async move {
        match tauri_bridge::memory_list(&ws).await {
            Ok(resp) => {
                state
                    .empty_categories
                    .set(resp.memory_subcategories.workspace);
                state
                    .global_subcategories
                    .set(resp.memory_subcategories.global);
                state.notes.set(resp.notes);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
        if let Ok(status) = tauri_bridge::memory_status(&ws).await {
            state.memory_status.set(Some(status.clone()));
            state
                .global_bootstrapped
                .set(status.global.memory && status.global.learnings);
        }
        load_global_readme_preview(state, ws);
    });
}

fn load_pointer_status(state: MemoryState, ws: String) {
    spawn_local(async move {
        match tauri_bridge::memory_pointer_status(&ws).await {
            Ok(results) => state.pointer_status.set(Some(results)),
            Err(e) => {
                state.pointer_status.set(Some(Vec::new()));
                state.error.set(Some(e));
            }
        }
    });
}

fn load_global_readme_preview(state: MemoryState, ws: String) {
    spawn_local(async move {
        match tauri_bridge::memory_read(&ws, &MemoryScope::Global, "README.md").await {
            Ok(NoteContent { content, .. }) => state.global_readme_preview.set(Some(content)),
            Err(_) => state.global_readme_preview.set(None),
        }
    });
}

pub(crate) fn load_note(state: MemoryState, ws: String, scope: MemoryScope, path: String) {
    spawn_local(async move {
        match tauri_bridge::memory_read(&ws, &scope, &path).await {
            Ok(NoteContent { content, .. }) => {
                state.editor_content.set(content);
                state.editor_dirty.set(false);
                state.show_preview.set(true);
                state.active_path.set(Some(path.clone()));
                state.active_scope.set(scope.clone());
                state.error.set(None);
                let ws2 = ws.clone();
                let p2 = path.clone();
                spawn_local(async move {
                    match tauri_bridge::memory_backlinks(&ws2, &scope, &p2).await {
                        Ok(v) => state.backlinks.set(v),
                        Err(_) => state.backlinks.set(Vec::new()),
                    }
                });
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn clear_memory_selection(state: MemoryState) {
    if state.active_path.get_untracked().is_none() {
        return;
    }
    state.active_path.set(None);
    state.editor_content.set(String::new());
    state.editor_dirty.set(false);
    state.show_preview.set(false);
    state.backlinks.set(Vec::new());
    state.graph_selected_node.set(None);
}

fn schedule_save(state: MemoryState, ws: String) {
    let token = state.save_token.get_untracked() + 1;
    state.save_token.set(token);
    let save_token = state.save_token;
    let path = state.active_path.get_untracked();
    let Some(path) = path else { return };
    let scope = state.active_scope.get_untracked();
    let content = state.editor_content.get_untracked();
    spawn_local(async move {
        TimeoutFuture::new(SAVE_DEBOUNCE_MS).await;
        if save_token.get_untracked() != token {
            return;
        }
        match tauri_bridge::memory_write(&ws, &scope, &path, &content).await {
            Ok(_) => {
                state.editor_dirty.set(false);
                load_notes(state.clone(), ws.clone());
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

pub(crate) fn refresh_graph(state: MemoryState, ws: String) {
    spawn_local(async move {
        match tauri_bridge::memory_graph(&ws).await {
            Ok(g) => state.graph.set(Some(g)),
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn input_value(ev: web_sys::Event) -> Option<String> {
    let t = ev.target()?;
    let el = t.dyn_into::<HtmlInputElement>().ok()?;
    Some(el.value())
}

#[component]
pub fn MemoryPanel() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let state = MemoryState::new();

    // Track active workspace cwd and reload memory state when it changes.
    let eff_state = state.clone();
    Effect::new(move |_| {
        let cwd = current_workspace_cwd(wb);
        let prev = eff_state.workspace_cwd.get_untracked();
        if cwd != prev {
            eff_state.workspace_cwd.set(cwd.clone());
            eff_state.active_path.set(None);
            eff_state.active_scope.set(MemoryScope::Workspace);
            eff_state.editor_content.set(String::new());
            eff_state.editor_dirty.set(false);
            eff_state.backlinks.set(Vec::new());
            eff_state.graph.set(None);
            eff_state.notes.set(Vec::new());
            eff_state.memory_status.set(None);
            eff_state.global_bootstrapped.set(false);
            eff_state.pointer_status.set(None);
            eff_state.pointers_notice_dismissed.set(false);
            eff_state.pointers_open.set(false);
            eff_state.pointers_busy.set(false);
            eff_state.selected_pointer_agents.set(HashSet::new());
            eff_state.global_readme_preview.set(None);
            if let Some(ws) = cwd {
                let st = eff_state.clone();
                let ws2 = ws.clone();
                spawn_local(async move {
                    load_notes(st.clone(), ws2.clone());
                    load_global_readme_preview(st.clone(), ws2.clone());
                    load_pointer_status(st, ws2);
                });
            }
        }
    });

    Effect::new({
        let st = state.clone();
        move |_| {
            if !st.pointers_open.get() {
                return;
            }
            st.selected_pointer_agents.set(HashSet::new());
            if let Some(ws) = st.workspace_cwd.get() {
                load_pointer_status(st.clone(), ws);
            }
        }
    });

    Effect::new({
        let wb = wb;
        let st = state.clone();
        move |_| {
            let pending = wb.pending_memory_note().get();
            let cwd = st.workspace_cwd.get();
            let Some(rel) = pending else {
                return;
            };
            let Some(ws) = cwd else {
                return;
            };
            wb.pending_memory_note().set(None);
            st.view.set(MemoryView::Files);
            load_note(st.clone(), ws, MemoryScope::Workspace, rel);
        }
    });

    view! {
        <div class="workbench-memory" role="region">
            <header class="workbench-memory__tabs" role="tablist">
                <MemoryTabBtn label=I18nKey::MemTabFiles state=state.clone() target=MemoryView::Files icon=icondata::LuFiles />
                <MemoryTabBtn label=I18nKey::MemTabGraph state=state.clone() target=MemoryView::Graph icon=icondata::LuNetwork />
                <MemoryTabBtn label=I18nKey::MemTabSearch state=state.clone() target=MemoryView::Search icon=icondata::LuSearch />
            </header>

            <Show when={
                let s = state.clone();
                move || s.error.get().is_some()
            }>
                {
                    let s = state.clone();
                    move || view! {
                        <div class="workbench-memory__error" role="alert">
                            <span>{move || s.error.get().unwrap_or_default()}</span>
                            <button
                                type="button"
                                class="workbench-memory__error-dismiss"
                                on:click={
                                    let s = s.clone();
                                    move |_| s.error.set(None)
                                }
                            >
                                "×"
                            </button>
                        </div>
                    }
                }
            </Show>

            <Show when={
                let s = state.clone();
                move || {
                    s.workspace_cwd.get().is_some()
                        && !s.pointers_notice_dismissed.get()
                        && s.pointer_status
                            .get()
                            .is_some_and(|status| !status.iter().any(|entry| entry.installed))
                }
            }>
                <MemoryPointersNotice state=state.clone() />
            </Show>

            <div class="workbench-memory__body">
                <Show when={
                    let s = state.clone();
                    move || memory_show_bootstrap_prompt(s)
                } fallback=move || view! {
                    <Show when={
                        let s = state.clone();
                        move || s.view.get() == MemoryView::Files
                    }>
                        <MemoryFilesView state=state.clone() />
                    </Show>
                    <Show when={
                        let s = state.clone();
                        move || s.view.get() == MemoryView::Graph
                    }>
                        <MemoryGraphView state=state.clone() />
                    </Show>
                    <Show when={
                        let s = state.clone();
                        move || s.view.get() == MemoryView::Search
                    }>
                        <MemorySearchView state=state.clone() />
                    </Show>
                }>
                    <MemoryBootstrapView state=state.clone() />
                </Show>
            </div>
            <Show when={
                let s = state.clone();
                move || s.workspace_cwd.get().is_none()
            }>
                <div class="workbench-memory__placeholder">
                    <p class="workbench-memory__placeholder-title">
                        {move || i18n.tr(I18nKey::MemEmptyTitle)()}
                    </p>
                    <p class="workbench-memory__placeholder-lead">
                        {move || i18n.tr(I18nKey::MemEmptyLead)()}
                    </p>
                </div>
            </Show>
            <Show when={
                let s = state.clone();
                move || s.pointers_open.get()
            }>
                <MemoryPointersDialog state=state.clone() />
            </Show>
        </div>
    }
}

fn memory_needs_workspace_bootstrap(status: &MemoryStatusResponse) -> bool {
    !status.workspace.memory || !status.workspace.learnings
}

fn memory_needs_global_bootstrap(status: &MemoryStatusResponse) -> bool {
    !status.global.memory || !status.global.learnings
}

fn memory_show_bootstrap_prompt(state: MemoryState) -> bool {
    if state.workspace_cwd.get().is_none() || !state.notes.get().is_empty() {
        return false;
    }
    state.memory_status.get().is_some_and(|status| {
        memory_needs_workspace_bootstrap(&status) || memory_needs_global_bootstrap(&status)
    })
}

#[component]
fn MemoryBootstrapView(state: MemoryState) -> impl IntoView {
    view! {
        {move || {
            let Some(status) = state.memory_status.get() else {
                return ().into_any();
            };
            let show_workspace = memory_needs_workspace_bootstrap(&status);
            let show_global = memory_needs_global_bootstrap(&status);
            view! {
                <div class="workbench-memory-bootstrap">
                    <Show when=move || show_workspace>
                        <MemoryBootstrapCard
                            state=state
                            scope=MemoryScope::Workspace
                            title="Create workspace memory"
                            path=".agents/memory + .agents/learnings"
                            description="Create the workspace memory and learnings folders with matching README.md overview files."
                        />
                    </Show>
                    <Show when=move || show_workspace && show_global>
                        <div class="workbench-memory-bootstrap__divider"></div>
                    </Show>
                    <Show when=move || show_global>
                        <MemoryBootstrapCard
                            state=state
                            scope=MemoryScope::Global
                            title="Create global memory"
                            path="~/.blxcode/memory + ~/.blxcode/learnings"
                            description="Create the global memory and learnings folders with matching README.md overview files."
                        />
                    </Show>
                </div>
            }
            .into_any()
        }}
    }
}

#[component]
fn MemoryBootstrapCard(
    state: MemoryState,
    scope: MemoryScope,
    title: &'static str,
    path: &'static str,
    description: &'static str,
) -> impl IntoView {
    let target = match scope {
        MemoryScope::Workspace => "workspace",
        MemoryScope::Global => "global",
    };
    view! {
        <section class="workbench-memory-bootstrap__card">
            <div class="workbench-memory-bootstrap__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuFolderOpen width="1.3rem" height="1.3rem" />
            </div>
            <h3>{title}</h3>
            <p>{description}</p>
            <code>{path}</code>
            <button
                type="button"
                class="workbench-memory-bootstrap__button"
                on:click=move |_| bootstrap_memory_scope(state, target)
            >
                <LxIcon icon=icondata::LuFolderPlus width="0.85rem" height="0.85rem" />
                <span>"Create folders"</span>
            </button>
        </section>
    }
}

fn bootstrap_memory_scope(state: MemoryState, target: &'static str) {
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    spawn_local(async move {
        match tauri_bridge::memory_bootstrap(&ws, target).await {
            Ok(()) => {
                load_notes(state.clone(), ws.clone());
                load_global_readme_preview(state, ws);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

#[component]
fn MemoryPointersNotice(state: MemoryState) -> impl IntoView {
    view! {
        <div class="workbench-memory-pointers-notice" role="status">
            <span class="workbench-memory-pointers-notice__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuInfo width="0.9rem" height="0.9rem" />
            </span>
            <p>
                "No agent memory pointers installed — external agents won't know where memory lives. "
                <button
                    type="button"
                    class="workbench-memory-pointers-notice__setup"
                    on:click=move |_| state.pointers_open.set(true)
                >
                    "Set up pointers"
                </button>
            </p>
            <button
                type="button"
                class="workbench-memory-pointers-notice__close"
                aria-label="Close"
                on:click=move |_| state.pointers_notice_dismissed.set(true)
            >
                <LxIcon icon=icondata::LuX width="0.75rem" height="0.75rem" />
            </button>
        </div>
    }
}

#[component]
fn MemoryPointersDialog(state: MemoryState) -> impl IntoView {
    view! {
        <div
            class="workspace-rename-backdrop"
            on:click=move |_| {
                if !state.pointers_busy.get_untracked() {
                    state.pointers_open.set(false);
                }
            }
        >
            <section
                class="workspace-rename-dialog memory-pointers-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="memory-pointers-title"
                on:click=move |ev| ev.stop_propagation()
            >
                <header class="memory-pointers-dialog__head">
                    <div class="memory-pointers-dialog__title-icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuLink2 width="1.1rem" height="1.1rem" />
                    </div>
                    <div>
                        <h2 id="memory-pointers-title">"Agent memory pointers"</h2>
                        <p>
                            "Install a marked block in CLAUDE.md, AGENTS.md, or similar so external agents know where memory lives. Target files must already exist."
                        </p>
                    </div>
                    <button
                        type="button"
                        class="workspace-rename-dialog__close"
                        disabled=move || state.pointers_busy.get()
                        on:click=move |_| state.pointers_open.set(false)
                    >
                        "×"
                    </button>
                </header>
                <ul class="memory-pointers-dialog__agents">
                    <For
                        each=move || POINTER_AGENTS.to_vec()
                        key=|agent| agent.id
                        children=move |agent: PointerAgent| view! {
                            <MemoryPointerAgentRow state=state agent=agent />
                        }
                    />
                </ul>
                <footer class="workspace-rename-dialog__actions memory-pointers-dialog__actions">
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--ghost"
                        disabled=move || {
                            state.pointers_busy.get()
                                || state.selected_pointer_agents.with(HashSet::is_empty)
                        }
                        on:click=move |_| run_pointer_action(state, false)
                    >
                        "Uninstall"
                    </button>
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                        disabled=move || {
                            state.pointers_busy.get()
                                || state.selected_pointer_agents.with(HashSet::is_empty)
                        }
                        on:click=move |_| run_pointer_action(state, true)
                    >
                        "Install"
                    </button>
                </footer>
            </section>
        </div>
    }
}

#[component]
fn MemoryPointerAgentRow(state: MemoryState, agent: PointerAgent) -> impl IntoView {
    let input_id = format!("memory-pointer-agent-{}", agent.id);
    let id_for_status = agent.id;
    let id_for_checked = agent.id;
    let id_for_change = agent.id.to_owned();
    view! {
        <li>
            <label class="memory-pointers-dialog__agent" for=input_id.clone()>
                <input
                    id=input_id.clone()
                    type="checkbox"
                    class="memory-pointers-dialog__checkbox"
                    prop:checked=move || {
                        state
                            .selected_pointer_agents
                            .with(|selected| selected.contains(id_for_checked))
                    }
                    disabled=move || state.pointers_busy.get()
                    on:change=move |ev| {
                        let checked = ev
                            .target()
                            .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
                            .is_some_and(|input| input.checked());
                        state.selected_pointer_agents.update(|selected| {
                            if checked {
                                selected.insert(id_for_change.clone());
                            } else {
                                selected.remove(&id_for_change);
                            }
                        });
                    }
                />
                <span class="memory-pointers-dialog__brand">
                    <img src=agent.icon alt="" prop:draggable=false />
                </span>
                <span class="memory-pointers-dialog__meta">
                    <span class="memory-pointers-dialog__label">{agent.label}</span>
                    <span class="memory-pointers-dialog__target">{agent.target}</span>
                </span>
                <PointerStatusBadge state=state agent_id=id_for_status />
            </label>
        </li>
    }
}

#[component]
fn PointerStatusBadge(state: MemoryState, agent_id: &'static str) -> impl IntoView {
    view! {
        {move || {
            let entry = state
                .pointer_status
                .get()
                .and_then(|status| status.into_iter().find(|item| item.agent == agent_id));
            let (class, icon, label) = pointer_status_view(entry.as_ref());
            view! {
                <span class=class>
                    {icon.map(|icon| view! {
                        <LxIcon icon=icon width="0.7rem" height="0.7rem" />
                    })}
                    <span>{label}</span>
                </span>
            }
        }}
    }
}

fn pointer_status_view(
    entry: Option<&PointerResult>,
) -> (&'static str, Option<icondata::Icon>, &'static str) {
    if entry.is_some_and(|result| result.installed) {
        (
            "memory-pointers-dialog__status memory-pointers-dialog__status--installed",
            Some(icondata::LuCircleCheck),
            "Installed",
        )
    } else if entry
        .and_then(|result| result.note.as_deref())
        .is_some_and(|note| note == "file missing")
    {
        (
            "memory-pointers-dialog__status memory-pointers-dialog__status--missing",
            Some(icondata::LuFileWarning),
            "File missing",
        )
    } else {
        (
            "memory-pointers-dialog__status memory-pointers-dialog__status--pending",
            None,
            "Not installed",
        )
    }
}

fn run_pointer_action(state: MemoryState, install: bool) {
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    let selected = state.selected_pointer_agents.get_untracked();
    let agents: Vec<String> = POINTER_AGENTS
        .iter()
        .filter(|agent| selected.contains(agent.id))
        .map(|agent| agent.id.to_owned())
        .collect();
    if agents.is_empty() || state.pointers_busy.get_untracked() {
        return;
    }
    state.pointers_busy.set(true);
    spawn_local(async move {
        let result = if install {
            tauri_bridge::memory_install_pointers(&ws, agents).await
        } else {
            tauri_bridge::memory_uninstall_pointers(&ws, agents).await
        };
        match result {
            Ok(results) => {
                merge_pointer_results(state, results);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
        state.pointers_busy.set(false);
    });
}

fn merge_pointer_results(state: MemoryState, results: Vec<PointerResult>) {
    state.pointer_status.update(|status| {
        let mut merged = status.take().unwrap_or_default();
        for result in results {
            if let Some(existing) = merged.iter_mut().find(|entry| entry.agent == result.agent) {
                *existing = result;
            } else {
                merged.push(result);
            }
        }
        *status = Some(merged);
    });
}

#[component]
fn MemoryTabBtn(
    label: I18nKey,
    state: MemoryState,
    target: MemoryView,
    icon: icondata::Icon,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let s = state.clone();
    let s2 = state.clone();
    view! {
        <button
            type="button"
            role="tab"
            class="workbench-memory__tab"
            class:workbench-memory__tab--active=move || s.view.get() == target
            aria-selected=move || (s2.view.get() == target).to_string()
            on:click=move |_| state.view.set(target)
        >
            <span class="workbench-memory__tab-icon" aria-hidden="true">
                <LxIcon icon=icon width="14px" height="14px" />
            </span>
            <span>{move || i18n.tr(label)()}</span>
        </button>
    }
}

#[component]
fn MemoryFilesView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let renaming = RwSignal::new(None::<String>);
    let rename_input = RwSignal::new(String::new());
    let files_collapsed = RwSignal::new(false);
    let groups_open = state.groups_open;
    let context_menu = RwSignal::new(None::<MemoryContextMenu>);
    let editing_category = RwSignal::new(None::<String>);
    // Some(scope) = dialog open for that scope; None = closed.
    let new_category_scope: RwSignal<Option<MemoryScope>> = RwSignal::new(None);
    let new_note_category: RwSignal<Option<(MemoryScope, String)>> = RwSignal::new(None);

    let make_groups = {
        let s = state.clone();
        move |scope: MemoryScope| {
            let _ = wb.workspaces().get();
            let empty = match scope {
                MemoryScope::Workspace => s.empty_categories.get(),
                MemoryScope::Global => s.global_subcategories.get(),
            };
            memory_note_groups_for_scope(s.notes.get(), empty, scope)
        }
    };

    let section = {
        let state = state.clone();
        move |scope: MemoryScope| {
            let s = state.clone();
            let make = make_groups.clone();
            let scope2 = scope.clone();
            let scope_for_roots = scope.clone();
            view! {
                <ul class="workbench-memory-files__list">
                    <For
                        each={
                            let s = s.clone();
                            move || root_memory_notes_for_scope(&s.notes.get(), &scope_for_roots)
                        }
                        key=|n| note_key(&n.scope, &n.path)
                        children={
                            let state = s.clone();
                            move |n: NoteMeta| {
                                let path = n.path.clone();
                                let expanded_note = n.clone();
                                let collapsed_note = n.clone();
                                let s_active = state.clone();
                                let note_scope = n.scope.clone();
                                let path_for_active = path.clone();
                                view! {
                                    <li
                                        class="workbench-memory-files__item"
                                        class:workbench-memory-files__item--collapsed=move || files_collapsed.get()
                                        class:workbench-memory-files__item--active=move || {
                                            s_active.active_scope.get() == note_scope
                                                && s_active.active_path.get().as_deref() == Some(path_for_active.as_str())
                                        }
                                    >
                                        <Show
                                            when=move || files_collapsed.get()
                                            fallback=move || view! {
                                                <MemoryFileExpandedRow
                                                    state=state
                                                    note=expanded_note.clone()
                                                    renaming=renaming
                                                    rename_input=rename_input
                                                    context_menu=context_menu
                                                />
                                            }
                                        >
                                            <MemoryFileCollapsedRow state=state note=collapsed_note.clone() context_menu=context_menu />
                                        </Show>
                                    </li>
                                }
                            }
                        }
                    />
                    <For
                        each=move || make(scope2.clone())
                        key=|g| g.key.clone()
                        children={
                            let state = s.clone();
                            move |group: MemoryNoteGroup| {
                                view! {
                                    <MemoryFileGroupSection
                                        state=state.clone()
                                        group=group
                                        groups_open=groups_open
                                        files_collapsed=files_collapsed
                                        renaming=renaming
                                        rename_input=rename_input
                                        context_menu=context_menu
                                        new_note_category=new_note_category
                                    />
                                }
                            }
                        }
                    />
                </ul>
            }
        }
    };

    view! {
        <div
            class="workbench-memory-files"
            class:workbench-memory-files--collapsed=move || files_collapsed.get()
            on:click=move |_| context_menu.set(None)
        >
            <aside
                class="workbench-memory-files__tree"
                class:workbench-memory-files__tree--collapsed=move || files_collapsed.get()
                on:click={
                    let state = state.clone();
                    move |ev: web_sys::MouseEvent| {
                        let same = ev
                            .target()
                            .zip(ev.current_target())
                            .is_some_and(|(t, c)| t == c);
                        if same {
                            clear_memory_selection(state.clone());
                        }
                    }
                }
            >
                <div
                    class="workbench-memory-files__new"
                    class:workbench-memory-files__new--collapsed=move || files_collapsed.get()
                >
                    <button
                        type="button"
                        class="workbench-memory-files__collapse-btn"
                        aria-label=move || {
                            if files_collapsed.get() {
                                i18n.tr(I18nKey::MemFilesExpand)()
                            } else {
                                i18n.tr(I18nKey::MemFilesCollapse)()
                            }
                        }
                        title=move || {
                            if files_collapsed.get() {
                                i18n.tr(I18nKey::MemFilesExpand)()
                            } else {
                                i18n.tr(I18nKey::MemFilesCollapse)()
                            }
                        }
                        on:click=move |_| {
                            renaming.set(None);
                            files_collapsed.update(|value| *value = !*value);
                        }
                    >
                        <Show
                            when=move || files_collapsed.get()
                            fallback=move || view! {
                                <LxIcon icon=icondata::LuPanelLeftClose width="0.82rem" height="0.82rem" />
                            }
                        >
                            <LxIcon icon=icondata::LuPanelLeftOpen width="0.82rem" height="0.82rem" />
                        </Show>
                    </button>
                </div>

                // ── Workspace (Projekt) section ──────────────────────────────
                <div class="workbench-memory-files__scope-section">
                    <div class="workbench-memory-files__scope-head">
                        <span>"Projekt"</span>
                        <button
                            type="button"
                            class="workbench-memory-files__scope-add"
                            title=move || i18n.tr(I18nKey::MemNewCategory)()
                            aria-label=move || i18n.tr(I18nKey::MemNewCategory)()
                            on:click=move |_| new_category_scope.set(Some(MemoryScope::Workspace))
                        >
                            <LxIcon icon=icondata::LuPlus width="0.75rem" height="0.75rem" />
                        </button>
                    </div>
                    {section(MemoryScope::Workspace)}
                </div>

                // ── Global section ───────────────────────────────────────────
                <div class="workbench-memory-files__scope-section">
                    <div class="workbench-memory-files__scope-head">
                        <span>"Global"</span>
                        <Show when=move || state.global_bootstrapped.get()>
                            <button
                                type="button"
                                class="workbench-memory-files__scope-add"
                                title=move || i18n.tr(I18nKey::MemNewCategory)()
                                aria-label=move || i18n.tr(I18nKey::MemNewCategory)()
                                on:click=move |_| new_category_scope.set(Some(MemoryScope::Global))
                            >
                                <LxIcon icon=icondata::LuPlus width="0.75rem" height="0.75rem" />
                            </button>
                        </Show>
                    </div>
                    <Show
                        when=move || state.global_bootstrapped.get()
                        fallback={
                            let st = state.clone();
                            let i = i18n.clone();
                            move || {
                                let st2 = st.clone();
                                view! {
                                    <div class="workbench-memory-files__global-init">
                                        <button
                                            type="button"
                                            class="workbench-memory-files__global-init-btn"
                                            on:click=move |_| {
                                                let Some(ws) = st2.workspace_cwd.get_untracked() else { return };
                                                let st3 = st2.clone();
                                                spawn_local(async move {
                                                    if tauri_bridge::memory_bootstrap(&ws, "global").await.is_ok() {
                                                        load_notes(st3, ws);
                                                    }
                                                });
                                            }
                                        >
                                            <LxIcon icon=icondata::LuGlobe width="0.82rem" height="0.82rem" />
                                            " "
                                            {move || i.tr(I18nKey::MemGlobalCreate)()}
                                        </button>
                                    </div>
                                }
                            }
                        }
                    >
                        {section(MemoryScope::Global)}
                    </Show>
                </div>
                <div
                    class="workbench-memory-files__tree-spacer"
                    aria-hidden="true"
                    on:click={
                        let state = state.clone();
                        move |_| clear_memory_selection(state.clone())
                    }
                ></div>
            </aside>
            <section class="workbench-memory-editor">
                <Show
                    when={
                        let s = state.clone();
                        move || s.active_path.get().is_some()
                    }
                    fallback={
                        let s = state.clone();
                        let i = i18n.clone();
                        move || view! {
                            <MemoryEditorFallback state=s i18n=i />
                        }
                    }
                >
                    <header class="workbench-memory-editor__toolbar">
                        <span class="workbench-memory-editor__flag" aria-live="polite">
                            {
                                let s = state.clone();
                                let i = i18n.clone();
                                move || if s.editor_dirty.get() { i.tr(I18nKey::MemDirty)().to_string() } else { String::new() }
                            }
                        </span>
                        <button
                            type="button"
                            class="workbench-memory-editor__preview-btn"
                            title=move || i18n.tr(I18nKey::MemShowInGraph)()
                            aria-label=move || i18n.tr(I18nKey::MemShowInGraph)()
                            on:click={
                                let s = state.clone();
                                move |_| {
                                    if let Some(path) = s.active_path.get_untracked() {
                                        navigate_to_graph_node(
                                            s.clone(),
                                            s.active_scope.get_untracked(),
                                            path,
                                        );
                                    }
                                }
                            }
                        >
                            <LxIcon icon=icondata::LuNetwork width="0.8rem" height="0.8rem" />
                        </button>
                        <button
                            type="button"
                            class="workbench-memory-editor__preview-btn"
                            aria-label={
                                let s = state.clone();
                                let i = i18n.clone();
                                move || if s.show_preview.get() { i.tr(I18nKey::MemEdit)() } else { i.tr(I18nKey::MemPreview)() }
                            }
                            title={
                                let s = state.clone();
                                let i = i18n.clone();
                                move || if s.show_preview.get() { i.tr(I18nKey::MemEdit)() } else { i.tr(I18nKey::MemPreview)() }
                            }
                            on:click={
                                let s = state.clone();
                                move |_| s.show_preview.update(|v| *v = !*v)
                            }
                        >
                            <Show
                                when={
                                    let s = state.clone();
                                    move || s.show_preview.get()
                                }
                                fallback=move || view! {
                                    <LxIcon icon=icondata::LuEye width="0.8rem" height="0.8rem" />
                                }
                            >
                                <LxIcon icon=icondata::LuPencil width="0.8rem" height="0.8rem" />
                            </Show>
                        </button>
                    </header>
                    <Show
                        when={
                            let s = state.clone();
                            move || !s.show_preview.get()
                        }
                        fallback={
                            let s = state.clone();
                            move || view! {
                                <div class="workbench-memory-editor__preview"
                                    inner_html=move || render_markdown_to_html(&s.editor_content.get())
                                />
                            }
                        }
                    >
                        <textarea
                            class="workbench-memory-editor__textarea"
                            prop:value={
                                let s = state.clone();
                                move || s.editor_content.get()
                            }
                            on:input={
                                let s = state.clone();
                                move |ev: web_sys::Event| {
                                    let Some(t) = ev.target() else { return };
                                    let Ok(el) = t.dyn_into::<web_sys::HtmlTextAreaElement>() else { return };
                                    s.editor_content.set(el.value());
                                    s.editor_dirty.set(true);
                                    if let Some(ws) = s.workspace_cwd.get_untracked() {
                                        schedule_save(s.clone(), ws);
                                    }
                                }
                            }
                        ></textarea>
                    </Show>
                    <footer class="workbench-memory-editor__backlinks">
                        <Show when={
                            let s = state.clone();
                            move || !s.backlinks.get().is_empty()
                        }>
                            <p class="workbench-memory-editor__backlinks-title">
                                {move || i18n.tr(I18nKey::MemBacklinks)()}
                            </p>
                            <ul class="workbench-memory-editor__backlinks-list">
                                <For
                                    each={
                                        let s = state.clone();
                                        move || s.backlinks.get()
                                    }
                                    key=|b| note_key(&b.scope, &b.path)
                                    children={
                                        let s = state.clone();
                                        move |b: BacklinkRef| {
                                            let s = s.clone();
                                            let scope = b.scope.clone();
                                            let path = b.path.clone();
                                            let label = path.clone();
                                            view! {
                                                <li>
                                                    <button
                                                        type="button"
                                                        class="workbench-memory-editor__backlink"
                                                        on:click=move |_| {
                                                            let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                            load_note(s.clone(), ws, scope.clone(), path.clone());
                                                        }
                                                    >{label}</button>
                                                </li>
                                            }
                                        }
                                    }
                                />
                            </ul>
                        </Show>
                    </footer>
                </Show>
            </section>
            <MemoryContextMenuView
                state=state.clone()
                menu=context_menu
                editing_category=editing_category
            />
            <Show when=move || editing_category.get().is_some()>
                {move || {
                    editing_category
                        .get()
                        .map(|category| view! {
                            <MemoryCategoryEditDialog
                                category=category
                                on_close=Callback::new(move |_| editing_category.set(None))
                            />
                        })
                }}
            </Show>
            <Show when=move || new_category_scope.get().is_some()>
                {
                    let state_for_cat = state.clone();
                    move || {
                        new_category_scope.get().map(|scope| view! {
                            <NewCategoryDialog
                                state=state_for_cat.clone()
                                scope=scope
                                on_close=Callback::new(move |_| new_category_scope.set(None))
                            />
                        })
                    }
                }
            </Show>
            <Show when=move || new_note_category.get().is_some()>
                {
                    let state_for_dialog = state.clone();
                    move || {
                        new_note_category.get().map(|(scope, category)| view! {
                            <NewNoteDialog
                                state=state_for_dialog.clone()
                                scope=scope
                                category=category
                                on_close=Callback::new(move |_| new_note_category.set(None))
                            />
                        })
                    }
                }
            </Show>
        </div>
    }
}

#[component]
fn MemoryEditorFallback(state: MemoryState, i18n: I18nService) -> impl IntoView {
    view! {
        {move || {
            if let Some(content) = state.global_readme_preview.get() {
                view! {
                    <header class="workbench-memory-editor__toolbar workbench-memory-editor__toolbar--readonly">
                        <span class="workbench-memory-editor__path">"Global / README.md"</span>
                        <span class="workbench-memory-editor__readonly">"Read-only preview"</span>
                    </header>
                    <div
                        class="workbench-memory-editor__preview"
                        inner_html=render_markdown_to_html(&content)
                    />
                }
                .into_any()
            } else {
                view! {
                    <div class="workbench-memory-editor__empty">
                        <p>{i18n.tr(I18nKey::MemSelectNote)()}</p>
                    </div>
                }
                .into_any()
            }
        }}
    }
}

#[component]
fn NewCategoryDialog(
    state: MemoryState,
    scope: MemoryScope,
    on_close: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let name = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);

    let submit = Callback::new(move |()| {
        let raw = name.get_untracked();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let Some(ws) = state.workspace_cwd.get_untracked() else {
            return;
        };
        let cat = trimmed.to_string();
        let sc = scope.clone();
        let state2 = state.clone();
        spawn_local(async move {
            match tauri_bridge::memory_create_category(&ws, &sc, &cat).await {
                Ok(created) => {
                    let key = match sc {
                        MemoryScope::Global => format!("global:{created}"),
                        MemoryScope::Workspace => created,
                    };
                    state2.groups_open.update(|s| {
                        s.insert(key);
                    });
                    load_notes(state2.clone(), ws);
                    on_close.run(());
                }
                Err(e) => error.set(Some(e)),
            }
        });
    });

    view! {
        <div class="workspace-rename-backdrop" on:click=move |_| on_close.run(())>
            <section
                class="workspace-rename-dialog"
                role="dialog"
                aria-modal="true"
                on:click=move |ev| ev.stop_propagation()
            >
                <header class="workspace-rename-dialog__head">
                    <h2>{move || i18n.tr(I18nKey::MemNewCategoryTitle)()}</h2>
                    <button
                        type="button"
                        class="workspace-rename-dialog__close"
                        on:click=move |_| on_close.run(())
                    >"×"</button>
                </header>
                <div class="workspace-rename-dialog__body">
                    <label class="workspace-rename-dialog__label" for="memory-new-category">
                        {move || i18n.tr(I18nKey::MemNewCategoryLabel)()}
                    </label>
                    <input
                        id="memory-new-category"
                        type="text"
                        class="workspace-rename-dialog__input"
                        placeholder=move || i18n.tr(I18nKey::MemNewCategoryPh)()
                        prop:value=move || name.get()
                        on:input=move |ev| {
                            if let Some(v) = input_value(ev) {
                                name.set(v);
                            }
                        }
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                submit.run(());
                            }
                        }
                    />
                    <Show when=move || error.get().is_some()>
                        <p class="workspace-rename-dialog__error">{move || error.get().unwrap_or_default()}</p>
                    </Show>
                </div>
                <footer class="workspace-rename-dialog__actions">
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--ghost"
                        on:click=move |_| on_close.run(())
                    >{move || i18n.tr(I18nKey::MemCancel)()}</button>
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                        disabled=move || name.get().trim().is_empty()
                        on:click=move |_| submit.run(())
                    >{move || i18n.tr(I18nKey::MemCreate)()}</button>
                </footer>
            </section>
        </div>
    }
}

#[component]
fn NewNoteDialog(
    state: MemoryState,
    scope: MemoryScope,
    category: String,
    on_close: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let title = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let header = {
        let label = clean_memory_label(&category);
        format!("{} – {label}", i18n.tr(I18nKey::MemNewNoteTitle)())
    };
    let category_for_submit = category.clone();

    let submit = Callback::new(move |()| {
        let raw = title.get_untracked();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let Some(ws) = state.workspace_cwd.get_untracked() else {
            return;
        };
        let fname = slug_to_filename(trimmed);
        let api_path = match category_for_submit.as_str() {
            CATEGORY_MEMORY => fname.clone(),
            CATEGORY_LEARNINGS => format!("{LEARNINGS_API_PREFIX}{fname}"),
            other => format!("{other}/{fname}"),
        };
        let body = format!("# {}\n\n", strip_ext(&fname));
        let state2 = state.clone();
        let sc = scope.clone();
        let cat = category_for_submit.clone();
        // Group key uses scope prefix for global notes.
        let group_key = match sc {
            MemoryScope::Global => format!("global:{cat}"),
            MemoryScope::Workspace => cat.clone(),
        };
        spawn_local(async move {
            match tauri_bridge::memory_create(&ws, &sc, &api_path, Some(&body)).await {
                Ok(meta) => {
                    state2.groups_open.update(|s| {
                        s.insert(group_key.clone());
                    });
                    load_notes(state2.clone(), ws.clone());
                    load_note(state2.clone(), ws, meta.scope, meta.path);
                    on_close.run(());
                }
                Err(e) => error.set(Some(e)),
            }
        });
    });

    view! {
        <div class="workspace-rename-backdrop" on:click=move |_| on_close.run(())>
            <section
                class="workspace-rename-dialog"
                role="dialog"
                aria-modal="true"
                on:click=move |ev| ev.stop_propagation()
            >
                <header class="workspace-rename-dialog__head">
                    <h2>{header}</h2>
                    <button
                        type="button"
                        class="workspace-rename-dialog__close"
                        on:click=move |_| on_close.run(())
                    >"×"</button>
                </header>
                <div class="workspace-rename-dialog__body">
                    <label class="workspace-rename-dialog__label" for="memory-new-note">
                        {move || i18n.tr(I18nKey::MemNewNoteLabel)()}
                    </label>
                    <input
                        id="memory-new-note"
                        type="text"
                        class="workspace-rename-dialog__input"
                        placeholder=move || i18n.tr(I18nKey::MemNewNotePh)()
                        prop:value=move || title.get()
                        on:input=move |ev| {
                            if let Some(v) = input_value(ev) {
                                title.set(v);
                            }
                        }
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                submit.run(());
                            }
                        }
                    />
                    <Show when=move || error.get().is_some()>
                        <p class="workspace-rename-dialog__error">{move || error.get().unwrap_or_default()}</p>
                    </Show>
                </div>
                <footer class="workspace-rename-dialog__actions">
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--ghost"
                        on:click=move |_| on_close.run(())
                    >{move || i18n.tr(I18nKey::MemCancel)()}</button>
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                        disabled=move || title.get().trim().is_empty()
                        on:click=move |_| submit.run(())
                    >{move || i18n.tr(I18nKey::MemCreate)()}</button>
                </footer>
            </section>
        </div>
    }
}

fn strip_ext(s: &str) -> String {
    if let Some(idx) = s.rfind('.') {
        if s[idx + 1..].eq_ignore_ascii_case("md") {
            return s[..idx].to_owned();
        }
    }
    s.to_owned()
}

#[component]
fn MemoryFileGroupHead(
    state: MemoryState,
    wb: WorkbenchService,
    group_key: String,
    group_scope: MemoryScope,
    groups_open: RwSignal<HashSet<String>>,
    files_collapsed: RwSignal<bool>,
    header_title: String,
    index_path: Option<String>,
    group_paths: Vec<String>,
    context_menu: RwSignal<Option<MemoryContextMenu>>,
    new_note_category: RwSignal<Option<(MemoryScope, String)>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let index_active = index_path.clone();
    let index_open = index_path;
    let context_label = header_title.clone();
    let plain_key = group_key
        .strip_prefix("global:")
        .unwrap_or(&group_key)
        .to_string();
    let key_for_settings = plain_key.clone();
    let key_for_color = plain_key.clone();
    let key_for_ctx = group_key.clone();
    let key_for_chev_class = group_key.clone();
    let key_for_aria_state = group_key.clone();
    let key_for_aria_label = group_key.clone();
    let key_for_click = group_key.clone();
    // Strip "global:" prefix so NewNoteDialog receives a plain category name.
    let cat_for_new = group_key
        .strip_prefix("global:")
        .unwrap_or(&group_key)
        .to_string();
    let scope_for_new = group_scope.clone();
    view! {
        <li
            class="workbench-memory-files__group-head"
            class:workbench-memory-files__group-head--sidebar-hidden=move || {
                !memory_category_settings(wb, &key_for_settings).show_in_sidebar
            }
            class:workbench-memory-files__group-head--hidden=move || files_collapsed.get()
            class:workbench-memory-files__group-head--active=move || {
                memory_group_index_active(&state, &index_active)
            }
            style=move || format!("--memory-category-color: {}", memory_category_settings(wb, &key_for_color).color)
            on:contextmenu=move |ev: web_sys::MouseEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                context_menu.set(Some(MemoryContextMenu {
                    x: ev.client_x(),
                    y: ev.client_y(),
                    target: MemoryContextTarget::Category {
                        key: key_for_ctx.clone(),
                        label: context_label.clone(),
                        paths: group_paths.clone(),
                    },
                }));
            }
        >
            <button
                type="button"
                class="workbench-memory-files__group-chevron"
                class:workbench-memory-files__group-chevron--open=move || {
                    groups_open.with(|s| s.contains(&key_for_chev_class))
                }
                aria-expanded=move || groups_open.with(|s| s.contains(&key_for_aria_state)).to_string()
                aria-label=move || {
                    if groups_open.with(|s| s.contains(&key_for_aria_label)) {
                        i18n.tr(I18nKey::MemFilesGroupCollapse)()
                    } else {
                        i18n.tr(I18nKey::MemFilesGroupExpand)()
                    }
                }
                on:click=move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    groups_open.update(|s| {
                        if s.contains(&key_for_click) {
                            s.remove(&key_for_click);
                        } else {
                            s.insert(key_for_click.clone());
                        }
                    });
                }
            >
                <LxIcon icon=icondata::LuChevronRight width="0.75rem" height="0.75rem" />
            </button>
            <MemoryFileGroupIndexButton
                state=state
                scope=group_scope.clone()
                label=header_title
                index_path=index_open
            />
            <button
                type="button"
                class="workbench-memory-files__group-add"
                title=move || i18n.tr(I18nKey::MemNewNoteInGroup)()
                aria-label=move || i18n.tr(I18nKey::MemNewNoteInGroup)()
                on:click=move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    new_note_category.set(Some((scope_for_new.clone(), cat_for_new.clone())));
                }
            >
                <LxIcon icon=icondata::LuPlus width="0.75rem" height="0.75rem" />
            </button>
        </li>
    }
}

#[component]
fn MemoryFileGroupIndexButton(
    state: MemoryState,
    scope: MemoryScope,
    label: String,
    index_path: Option<String>,
) -> impl IntoView {
    let title = label.clone();
    view! {
        <button
            type="button"
            class="workbench-memory-files__group-index"
            title=title
            on:click=move |_| memory_open_group_index(state, scope.clone(), index_path.clone())
        >
            {label}
        </button>
    }
}

#[component]
fn MemoryFileGroupSection(
    state: MemoryState,
    group: MemoryNoteGroup,
    groups_open: RwSignal<HashSet<String>>,
    files_collapsed: RwSignal<bool>,
    renaming: RwSignal<Option<String>>,
    rename_input: RwSignal<String>,
    context_menu: RwSignal<Option<MemoryContextMenu>>,
    new_note_category: RwSignal<Option<(MemoryScope, String)>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let group_key = group.key.clone();
    let group_scope = group.scope.clone();
    let header_title = memory_group_header_label(&group, &i18n, wb);
    let index_path = group.index.as_ref().map(|n| n.path.clone());
    let index = group.index;
    let group_notes = group.notes;
    let plain_key_for_show = group_key
        .strip_prefix("global:")
        .unwrap_or(&group_key)
        .to_string();
    let key_for_open_check = group_key.clone();
    let show_sidebar = move || {
        wb.active_id()
            .get()
            .map(|id| {
                wb.memory_category_settings_for_workspace(id, &plain_key_for_show)
                    .show_in_sidebar
            })
            .unwrap_or(true)
    };

    view! {
        <MemoryFileGroupHead
            state=state
            wb=wb
            group_key=group_key.clone()
            group_scope=group_scope
            groups_open=groups_open
            files_collapsed=files_collapsed
            header_title=header_title
            index_path=index_path
            group_paths=memory_group_paths(&index, &group_notes)
            context_menu=context_menu
            new_note_category=new_note_category
        />
        <For
            each=move || {
                if !show_sidebar() {
                    Vec::new()
                } else if files_collapsed.get() {
                    memory_group_collapsed_items(&index, &group_notes)
                } else if groups_open.with(|s| s.contains(&key_for_open_check)) {
                    group_notes.clone()
                } else {
                    Vec::new()
                }
            }
            key=|n| n.path.clone()
            children={
                let state = state.clone();
                move |n: NoteMeta| {
                    let path = n.path.clone();
                    let expanded_note = n.clone();
                    let collapsed_note = n.clone();
                    let s_active = state.clone();
                    let path_for_active = path.clone();
                    view! {
                        <li
                            class="workbench-memory-files__item"
                            class:workbench-memory-files__item--collapsed=move || files_collapsed.get()
                            class:workbench-memory-files__item--active=move || {
                                s_active.active_path.get().as_deref() == Some(path_for_active.as_str())
                            }
                        >
                            <Show
                                when=move || files_collapsed.get()
                                fallback=move || view! {
                                    <MemoryFileExpandedRow
                                        state=state
                                        note=expanded_note.clone()
                                        renaming=renaming
                                        rename_input=rename_input
                                        context_menu=context_menu
                                    />
                                }
                            >
                                <MemoryFileCollapsedRow state=state note=collapsed_note.clone() context_menu=context_menu />
                            </Show>
                        </li>
                    }
                }
            }
        />
    }
}

#[component]
fn MemoryFileCollapsedRow(
    state: MemoryState,
    note: NoteMeta,
    context_menu: RwSignal<Option<MemoryContextMenu>>,
) -> impl IntoView {
    let label = clean_memory_label(&note.name);
    let badge = note_badge_text(&label);
    let scope = note.scope.clone();
    let path = note.path.clone();
    let context_scope = scope.clone();
    let context_path = path.clone();
    let context_label = label.clone();

    view! {
        <button
            type="button"
            class="workbench-memory-files__badge"
            title=label.clone()
            aria-label=label
            on:contextmenu=move |ev: web_sys::MouseEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                context_menu.set(Some(MemoryContextMenu {
                    x: ev.client_x(),
                    y: ev.client_y(),
                    target: MemoryContextTarget::Note {
                        scope: context_scope.clone(),
                        path: context_path.clone(),
                        label: context_label.clone(),
                    },
                }));
            }
            on:click=move |_| {
                let Some(ws) = state.workspace_cwd.get_untracked() else {
                    return;
                };
                load_note(state, ws, scope.clone(), path.clone());
            }
        >
            {badge}
        </button>
    }
}

#[component]
fn MemoryFileExpandedRow(
    state: MemoryState,
    note: NoteMeta,
    renaming: RwSignal<Option<String>>,
    rename_input: RwSignal<String>,
    context_menu: RwSignal<Option<MemoryContextMenu>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let label = clean_memory_label(&note.name);
    let folder = memory_display_folder(&note.path);
    let note_scope = note.scope.clone();
    let note_path = note.path.clone();
    let path_for_select = note_path.clone();
    let path_for_del = note_path.clone();
    let path_for_ren = note_path.clone();
    let label_for_ren = label.clone();

    view! {
        {move || {
            let note_path = note_path.clone();
            let path_for_select = path_for_select.clone();
            let path_for_del = path_for_del.clone();
            let path_for_ren = path_for_ren.clone();
            let label = label.clone();
            let folder = folder.clone();
            let label_for_ren = label_for_ren.clone();
            if renaming.get().as_deref() == Some(note_path.as_str()) {
                let old_path = note_path.clone();
                let scope_for_rename = note_scope.clone();
                view! {
                    <form
                        class="workbench-memory-files__rename"
                        on:submit=move |ev: web_sys::SubmitEvent| {
                            ev.prevent_default();
                            let Some(ws) = state.workspace_cwd.get_untracked() else {
                                return;
                            };
                            let v = rename_input.get_untracked();
                            let new_name = slug_to_filename(&v);
                            let new_path = if let Some(parent) = old_path.rsplit_once('/').map(|(d, _)| d) {
                                format!("{parent}/{new_name}")
                            } else {
                                new_name
                            };
                            let op = old_path.clone();
                            let np = new_path.clone();
                            let sc = scope_for_rename.clone();
                            spawn_local(async move {
                                match tauri_bridge::memory_rename(&ws, &sc, &op, &np, true).await {
                                    Ok(_) => {
                                        renaming.set(None);
                                        if state.active_path.get_untracked().as_deref() == Some(op.as_str()) {
                                            state.active_path.set(Some(np.clone()));
                                        }
                                        load_notes(state, ws);
                                    }
                                    Err(e) => state.error.set(Some(e)),
                                }
                            });
                        }
                    >
                        <input
                            type="text"
                            class="workbench-memory-files__rename-input"
                            prop:value=move || rename_input.get()
                            on:input=move |ev| {
                                if let Some(v) = input_value(ev) {
                                    rename_input.set(v);
                                }
                            }
                        />
                        <button type="submit" class="workbench-memory-files__action" title=move || i18n.tr(I18nKey::MemSave)()>"✔"</button>
                        <button
                            type="button"
                            class="workbench-memory-files__action"
                            title=move || i18n.tr(I18nKey::MemCancel)()
                            on:click=move |_| renaming.set(None)
                        >"✗"</button>
                    </form>
                }
                .into_any()
            } else {
                let scope_for_ctx = note_scope.clone();
                let scope_for_open = note_scope.clone();
                let scope_for_del = note_scope.clone();
                view! {
                    <button
                        type="button"
                        class="workbench-memory-files__name"
                        on:contextmenu={
                            let p = path_for_select.clone();
                            let l = label.clone();
                            move |ev: web_sys::MouseEvent| {
                                ev.prevent_default();
                                ev.stop_propagation();
                                context_menu.set(Some(MemoryContextMenu {
                                    x: ev.client_x(),
                                    y: ev.client_y(),
                                    target: MemoryContextTarget::Note {
                                        scope: scope_for_ctx.clone(),
                                        path: p.clone(),
                                        label: l.clone(),
                                    },
                                }));
                            }
                        }
                        on:click=move |_| {
                            let Some(ws) = state.workspace_cwd.get_untracked() else {
                                return;
                            };
                            load_note(state, ws, scope_for_open.clone(), path_for_select.clone());
                        }
                    >
                        <span class="workbench-memory-files__name-text">{label.clone()}</span>
                        {folder.clone().map(|f| view! { <small class="workbench-memory-files__folder">{f}</small> })}
                    </button>
                    <button
                        type="button"
                        class="workbench-memory-files__action"
                        title=move || i18n.tr(I18nKey::MemRename)()
                        on:click=move |_| {
                            rename_input.set(label_for_ren.clone());
                            renaming.set(Some(path_for_ren.clone()));
                        }
                    >"✎"</button>
                    <button
                        type="button"
                        class="workbench-memory-files__action workbench-memory-files__action--danger"
                        title=move || i18n.tr(I18nKey::MemDelete)()
                        on:click=move |_| {
                            let Some(ws) = state.workspace_cwd.get_untracked() else {
                                return;
                            };
                            let p = path_for_del.clone();
                            let sc = scope_for_del.clone();
                            spawn_local(async move {
                                match tauri_bridge::memory_delete(&ws, &sc, &p).await {
                                    Ok(()) => {
                                        if state.active_path.get_untracked().as_deref() == Some(p.as_str()) {
                                            state.active_path.set(None);
                                            state.editor_content.set(String::new());
                                            state.backlinks.set(Vec::new());
                                        }
                                        load_notes(state, ws);
                                    }
                                    Err(e) => state.error.set(Some(e)),
                                }
                            });
                        }
                    >"🗑"</button>
                }
                .into_any()
            }
        }}
    }
}

#[component]
fn MemoryContextMenuView(
    state: MemoryState,
    menu: RwSignal<Option<MemoryContextMenu>>,
    editing_category: RwSignal<Option<String>>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    view! {
        <Show when=move || menu.get().is_some()>
            {move || {
                let Some(current) = menu.get() else {
                    return ().into_any();
                };
                let style = format!("left: {}px; top: {}px", current.x, current.y);
                match current.target {
                    MemoryContextTarget::Category { key, label, paths } => {
                        let edit_key = key.clone();
                        let send_key = key.clone();
                        let send_label = label.clone();
                        view! {
                            <div class="workspace-context-menu memory-context-menu" style=style on:click=move |ev| ev.stop_propagation()>
                                <button
                                    type="button"
                                    class="workspace-context-menu__item"
                                    on:click=move |_| {
                                        menu.set(None);
                                        editing_category.set(Some(edit_key.clone()));
                                    }
                                >"Edit"</button>
                                <button
                                    type="button"
                                    class="workspace-context-menu__item"
                                    on:click=move |_| {
                                        add_category_agent_context(wb, &send_key, send_label.clone(), paths.clone());
                                        menu.set(None);
                                    }
                                >"Send to BLXCode Agent"</button>
                            </div>
                        }
                        .into_any()
                    }
                    MemoryContextTarget::Note { scope, path, label } => {
                        let open_scope = scope.clone();
                        let open_path = path.clone();
                        let send_path = path.clone();
                        let send_label = label.clone();
                        view! {
                            <div class="workspace-context-menu memory-context-menu" style=style on:click=move |ev| ev.stop_propagation()>
                                <button
                                    type="button"
                                    class="workspace-context-menu__item"
                                    on:click=move |_| {
                                        menu.set(None);
                                        if let Some(ws) = current_workspace_cwd(wb) {
                                            load_note(state, ws, open_scope.clone(), open_path.clone());
                                        }
                                    }
                                >"Open"</button>
                                <button
                                    type="button"
                                    class="workspace-context-menu__item"
                                    on:click=move |_| {
                                        add_note_agent_context(wb, send_path.clone(), send_label.clone());
                                        menu.set(None);
                                    }
                                >"Send to BLXCode Agent"</button>
                            </div>
                        }
                        .into_any()
                    }
                }
            }}
        </Show>
    }
}

#[component]
fn MemoryCategoryEditDialog(category: String, on_close: Callback<()>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let initial = wb
        .active_id()
        .get_untracked()
        .map(|id| wb.memory_category_settings_for_workspace_untracked(id, &category))
        .unwrap_or_else(|| MemoryCategorySettings::for_category(&category));
    let label = RwSignal::new(initial.label);
    let color = RwSignal::new(initial.color);
    let show_sidebar = RwSignal::new(initial.show_in_sidebar);
    let show_graph = RwSignal::new(initial.show_in_graph);
    let title = format!("Edit {}", clean_memory_label(&category));
    let category_for_save = category.clone();

    view! {
        <div class="workspace-rename-backdrop" on:click=move |_| on_close.run(())>
            <section class="workspace-rename-dialog memory-category-dialog" on:click=move |ev| ev.stop_propagation()>
                <header class="workspace-rename-dialog__head">
                    <h2>{title}</h2>
                    <button type="button" class="workspace-rename-dialog__close" on:click=move |_| on_close.run(())>"×"</button>
                </header>
                <div class="workspace-rename-dialog__body memory-category-dialog__body">
                    <label class="workspace-rename-dialog__label" for="memory-category-label">"Display name"</label>
                    <input
                        id="memory-category-label"
                        type="text"
                        class="workspace-rename-dialog__input"
                        prop:value=move || label.get()
                        on:input=move |ev| {
                            if let Some(v) = input_value(ev) {
                                label.set(v);
                            }
                        }
                    />
                    <label class="workspace-rename-dialog__label" for="memory-category-color">"Color"</label>
                    <div class="memory-category-dialog__color-row">
                        <input
                            id="memory-category-color"
                            type="color"
                            class="memory-category-dialog__color-input"
                            prop:value=move || color.get()
                            on:input=move |ev| {
                                if let Some(v) = input_value(ev) {
                                    color.set(v);
                                }
                            }
                        />
                        <input
                            type="text"
                            class="workspace-rename-dialog__input"
                            prop:value=move || color.get()
                            on:input=move |ev| {
                                if let Some(v) = input_value(ev) {
                                    color.set(v);
                                }
                            }
                        />
                    </div>
                    <div class="memory-color-swatches" aria-label="Memory color presets">
                        <For
                            each=move || wb.memory_color_presets().get()
                            key=|preset| preset.id.clone()
                            children=move |preset| {
                                let preset_color = preset.color.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="memory-color-swatch"
                                        title=preset.label
                                        style=format!("--memory-swatch: {}", preset_color)
                                        on:click=move |_| color.set(preset_color.clone())
                                    ></button>
                                }
                            }
                        />
                    </div>
                    <label class="memory-category-dialog__toggle">
                        <input
                            type="checkbox"
                            prop:checked=move || show_sidebar.get()
                            on:change=move |ev| {
                                let checked = ev
                                    .target()
                                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                    .is_some_and(|input| input.checked());
                                show_sidebar.set(checked);
                            }
                        />
                        <span>"Show in sidebar"</span>
                    </label>
                    <label class="memory-category-dialog__toggle">
                        <input
                            type="checkbox"
                            prop:checked=move || show_graph.get()
                            on:change=move |ev| {
                                let checked = ev
                                    .target()
                                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                    .is_some_and(|input| input.checked());
                                show_graph.set(checked);
                            }
                        />
                        <span>"Show in graph"</span>
                    </label>
                </div>
                <footer class="workspace-rename-dialog__actions">
                    <button type="button" class="workspace-rename-dialog__btn workspace-rename-dialog__btn--ghost" on:click=move |_| on_close.run(())>"Cancel"</button>
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                        on:click=move |_| {
                            if let Some(ws_id) = wb.active_id().get_untracked() {
                                let fallback = MemoryCategorySettings::for_category(&category_for_save);
                                wb.set_memory_category_settings(ws_id, &category_for_save, MemoryCategorySettings {
                                    label: label.get_untracked().trim().to_string(),
                                    color: normalize_hex_color(&color.get_untracked(), &fallback.color),
                                    show_in_sidebar: show_sidebar.get_untracked(),
                                    show_in_graph: show_graph.get_untracked(),
                                });
                            }
                            on_close.run(());
                        }
                    >"Save"</button>
                </footer>
            </section>
        </div>
    }
}

fn memory_category_settings(wb: WorkbenchService, category: &str) -> MemoryCategorySettings {
    wb.active_id()
        .get()
        .map(|id| wb.memory_category_settings_for_workspace(id, category))
        .unwrap_or_else(|| MemoryCategorySettings::for_category(category))
}

fn memory_group_paths(index: &Option<NoteMeta>, notes: &[NoteMeta]) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(index) = index {
        paths.push(index.path.clone());
    }
    paths.extend(notes.iter().map(|note| note.path.clone()));
    paths
}

fn add_category_agent_context(
    wb: WorkbenchService,
    category: &str,
    label: String,
    paths: Vec<String>,
) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        return;
    };
    let plain = category.strip_prefix("global:").unwrap_or(category);
    let kind = if plain == CATEGORY_LEARNINGS {
        AgentContextKind::LearningCategory
    } else {
        AgentContextKind::MemoryCategory
    };
    let count = paths.len();
    wb.upsert_workspace_agent_context(
        ws_id,
        AgentContextItem {
            id: format!("memory-category:{category}"),
            kind,
            label,
            source: format!("{count} memory paths"),
            paths,
            added_at: Date::now() as i64,
            content: None,
        },
    );
}

fn add_note_agent_context(wb: WorkbenchService, path: String, label: String) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        return;
    };
    let kind = if path.starts_with(LEARNINGS_API_PREFIX) {
        AgentContextKind::LearningNote
    } else {
        AgentContextKind::MemoryNote
    };
    wb.upsert_workspace_agent_context(
        ws_id,
        AgentContextItem {
            id: format!("memory-note:{path}"),
            kind,
            label,
            source: path.clone(),
            paths: vec![path],
            added_at: Date::now() as i64,
            content: None,
        },
    );
}

#[derive(Clone)]
struct MemoryNoteGroup {
    /// Group key; global groups are prefixed `"global:"`.
    key: String,
    scope: MemoryScope,
    index: Option<NoteMeta>,
    notes: Vec<NoteMeta>,
}

#[derive(Clone, PartialEq)]
enum MemoryContextTarget {
    Category {
        key: String,
        label: String,
        paths: Vec<String>,
    },
    Note {
        scope: MemoryScope,
        path: String,
        label: String,
    },
}

#[derive(Clone, PartialEq)]
struct MemoryContextMenu {
    x: i32,
    y: i32,
    target: MemoryContextTarget,
}

fn memory_note_groups_for_scope(
    notes: Vec<NoteMeta>,
    empty_categories: Vec<String>,
    scope: MemoryScope,
) -> Vec<MemoryNoteGroup> {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<String, Vec<NoteMeta>> = BTreeMap::new();
    buckets.entry(CATEGORY_LEARNINGS.to_string()).or_default();
    for n in notes
        .into_iter()
        .filter(|n| n.scope == scope && n.enabled && !n.is_template)
    {
        let cat = category_for_path(&n.path);
        if cat == CATEGORY_MEMORY {
            continue;
        }
        buckets.entry(cat).or_default().push(n);
    }
    for cat in empty_categories {
        if cat == CATEGORY_MEMORY || cat == CATEGORY_LEARNINGS {
            continue;
        }
        buckets.entry(cat).or_default();
    }

    let mut keys: Vec<String> = buckets.keys().cloned().collect();
    keys.sort_by(|a, b| match (a.as_str(), b.as_str()) {
        (CATEGORY_LEARNINGS, _) => std::cmp::Ordering::Less,
        (_, CATEGORY_LEARNINGS) => std::cmp::Ordering::Greater,
        _ => a.to_lowercase().cmp(&b.to_lowercase()),
    });

    let mut groups = Vec::new();
    for cat in keys {
        let bucket = buckets.remove(&cat).unwrap_or_default();
        let (index, notes) = match cat.as_str() {
            CATEGORY_LEARNINGS => split_group_index(bucket, is_learnings_index_note),
            _ => split_group_index(bucket, is_category_index_note),
        };
        // Global groups use a "global:" prefix on the key so they don't clash with workspace keys.
        let key = match scope {
            MemoryScope::Global => format!("global:{cat}"),
            MemoryScope::Workspace => cat,
        };
        groups.push(MemoryNoteGroup {
            key,
            scope: scope.clone(),
            index,
            notes,
        });
    }
    groups
}

fn split_group_index(
    notes: Vec<NoteMeta>,
    is_index: impl Fn(&NoteMeta) -> bool,
) -> (Option<NoteMeta>, Vec<NoteMeta>) {
    let mut index = None;
    let mut rest = Vec::new();
    for n in notes {
        if index.is_none() && is_index(&n) {
            index = Some(n);
        } else {
            rest.push(n);
        }
    }
    (index, rest)
}

fn is_learnings_index_note(note: &NoteMeta) -> bool {
    LEARNINGS_INDEX_PATHS
        .iter()
        .any(|p| note.path.eq_ignore_ascii_case(p))
}

fn is_category_index_note(note: &NoteMeta) -> bool {
    std::path::Path::new(&note.path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("README.md"))
}

fn root_memory_notes_for_scope(notes: &[NoteMeta], scope: &MemoryScope) -> Vec<NoteMeta> {
    let mut out: Vec<NoteMeta> = notes
        .iter()
        .filter(|n| {
            &n.scope == scope
                && n.enabled
                && !n.is_template
                && !n.is_overview
                && n.category == CATEGORY_MEMORY
        })
        .cloned()
        .collect();
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

fn memory_open_group_index(state: MemoryState, scope: MemoryScope, index_path: Option<String>) {
    let Some(path) = index_path else {
        return;
    };
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    load_note(state, ws, scope, path);
}

fn memory_group_index_active(state: &MemoryState, index_path: &Option<String>) -> bool {
    index_path
        .as_deref()
        .is_some_and(|p| state.active_path.get().as_deref() == Some(p))
}

fn memory_group_header_label(
    group: &MemoryNoteGroup,
    i18n: &I18nService,
    wb: WorkbenchService,
) -> String {
    // Use plain category (strip "global:" prefix) for settings + i18n keys.
    let plain_key = group.key.strip_prefix("global:").unwrap_or(&group.key);
    if let Some(ws_id) = wb.active_id().get_untracked() {
        let settings = wb.memory_category_settings_for_workspace_untracked(ws_id, plain_key);
        if !settings.label.trim().is_empty() {
            return clean_memory_label(&settings.label);
        }
    }
    match plain_key {
        CATEGORY_LEARNINGS => return i18n.tr(I18nKey::MemFilesGroupLearnings)().to_string(),
        CATEGORY_MEMORY => return i18n.tr(I18nKey::MemFilesGroupMemory)().to_string(),
        _ => {}
    }
    if let Some(idx) = &group.index {
        return clean_memory_label(&idx.name);
    }
    match plain_key {
        CATEGORY_LEARNINGS => i18n.tr(I18nKey::MemFilesGroupLearnings)().to_string(),
        CATEGORY_MEMORY => i18n.tr(I18nKey::MemFilesGroupMemory)().to_string(),
        other => clean_memory_label(other),
    }
}

fn clean_memory_label(raw: &str) -> String {
    let tail = raw
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .last()
        .unwrap_or(raw)
        .trim_end_matches(".md")
        .trim_end_matches(".MD")
        .to_string();
    let words: Vec<String> = tail
        .split(|ch: char| matches!(ch, '-' | '_' | '.' | ' ') || ch.is_whitespace())
        .filter(|word| !word.trim().is_empty())
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "api"
                    | "ui"
                    | "ux"
                    | "url"
                    | "http"
                    | "https"
                    | "json"
                    | "css"
                    | "html"
                    | "js"
                    | "ts"
                    | "2d"
                    | "3d"
            ) {
                return lower.to_ascii_uppercase();
            }
            let mut chars = lower.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
        })
        .collect();
    if words.is_empty() {
        raw.to_string()
    } else {
        words.join(" ")
    }
}

fn memory_group_collapsed_items(index: &Option<NoteMeta>, notes: &[NoteMeta]) -> Vec<NoteMeta> {
    let mut out = Vec::new();
    if let Some(idx) = index {
        out.push(idx.clone());
    }
    out.extend(notes.iter().cloned());
    out
}

fn memory_display_folder(path: &str) -> Option<String> {
    let tail = path.strip_prefix(LEARNINGS_API_PREFIX).unwrap_or(path);
    tail.rsplit_once('/').map(|(d, _)| d.to_string())
}

fn note_badge_text(name: &str) -> String {
    let mut out = String::new();
    for part in name
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
        name.chars()
            .next()
            .map(|ch| ch.to_uppercase().collect())
            .unwrap_or_else(|| "?".to_string())
    } else {
        out
    }
}

fn filter_search_hits(hits: Vec<SearchHit>, filter: Option<String>) -> Vec<SearchHit> {
    match filter.as_deref() {
        None => hits,
        Some("workspace") => hits
            .into_iter()
            .filter(|h| h.scope == MemoryScope::Workspace)
            .collect(),
        Some("global") => hits
            .into_iter()
            .filter(|h| h.scope == MemoryScope::Global)
            .collect(),
        Some(key) => {
            if let Some((scope_str, category)) = key.split_once(':') {
                let target_scope = match scope_str {
                    "workspace" => MemoryScope::Workspace,
                    "global" => MemoryScope::Global,
                    _ => return hits,
                };
                hits.into_iter()
                    .filter(|h| h.scope == target_scope && h.category == category)
                    .collect()
            } else {
                hits
            }
        }
    }
}

fn search_scope_categories(hits: &[SearchHit]) -> Vec<(Option<String>, usize)> {
    let mut workspace_count = 0_usize;
    let mut global_count = 0_usize;
    let mut category_counts: Vec<(String, usize)> = Vec::new();

    for hit in hits {
        let scope_key = match hit.scope {
            MemoryScope::Workspace => {
                workspace_count += 1;
                "workspace"
            }
            MemoryScope::Global => {
                global_count += 1;
                "global"
            }
        };
        let key = format!("{scope_key}:{}", hit.category);
        if let Some((_, count)) = category_counts
            .iter_mut()
            .find(|(existing, _)| existing == &key)
        {
            *count += 1;
        } else {
            category_counts.push((key, 1));
        }
    }

    category_counts.sort_by(|(a, _), (b, _)| a.cmp(b));
    let mut chips = Vec::with_capacity(category_counts.len() + 3);
    chips.push((None, hits.len()));
    if workspace_count > 0 {
        chips.push((Some("workspace".to_string()), workspace_count));
    }
    if global_count > 0 {
        chips.push((Some("global".to_string()), global_count));
    }
    chips.extend(
        category_counts
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(key, count)| (Some(key), count)),
    );
    chips
}

fn search_filter_label(i18n: &I18nService, key: Option<&str>) -> String {
    match key {
        None => i18n.tr(I18nKey::MemSearchFilterAll)().to_string(),
        Some("workspace") => i18n.tr(I18nKey::MemSearchFilterWorkspace)().to_string(),
        Some("global") => i18n.tr(I18nKey::MemSearchFilterGlobal)().to_string(),
        Some(key) => {
            if let Some((scope, category)) = key.split_once(':') {
                let scope_label = match scope {
                    "workspace" => i18n.tr(I18nKey::MemSearchFilterWorkspace)().to_string(),
                    "global" => i18n.tr(I18nKey::MemSearchFilterGlobal)().to_string(),
                    _ => scope.to_string(),
                };
                format!("{scope_label}:{}", clean_memory_label(category))
            } else {
                clean_memory_label(key)
            }
        }
    }
}

#[component]
fn MemorySearchView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let debounce_token: RwSignal<u32> = RwSignal::new(0);
    let search_filter: RwSignal<Option<String>> = RwSignal::new(None);

    let on_input = {
        let search_filter = search_filter;
        move |ev: web_sys::Event| {
            let v = input_value(ev).unwrap_or_default();
            state.search_query.set(v.clone());
            search_filter.set(None);
            let token = debounce_token.get_untracked() + 1;
            debounce_token.set(token);
            let s = state;
            spawn_local(async move {
                TimeoutFuture::new(200).await;
                if debounce_token.get_untracked() != token {
                    return;
                }
                let Some(ws) = s.workspace_cwd.get_untracked() else {
                    return;
                };
                if v.trim().is_empty() {
                    s.search_results.set(Vec::new());
                    return;
                }
                match tauri_bridge::memory_search(&ws, &v).await {
                    Ok(r) => s.search_results.set(r),
                    Err(e) => s.error.set(Some(e)),
                }
            });
        }
    };

    view! {
        <div class="workbench-memory-search">
            <input
                type="text"
                class="workbench-memory-search__input"
                placeholder=move || i18n.tr(I18nKey::MemSearchPh)()
                prop:value={
                    let s = state.clone();
                    move || s.search_query.get()
                }
                on:input=on_input
            />
            <Show
                when={
                    let s = state.clone();
                    move || !s.search_results.get().is_empty()
                }
            >
                <div class="workbench-memory-search__filters" role="group">
                    <For
                        each={
                            let s = state.clone();
                            move || search_scope_categories(&s.search_results.get())
                        }
                        key=|(key, _)| key.clone().unwrap_or_else(|| "__all".to_string())
                        children={
                            let i18n = i18n.clone();
                            move |(key, count): (Option<String>, usize)| {
                                let key_for_active = key.clone();
                                let key_for_click = key.clone();
                                let key_for_label = key.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="workbench-memory-search__filter"
                                        class:workbench-memory-search__filter--active=move || {
                                            search_filter.get() == key_for_active
                                        }
                                        on:click=move |_| search_filter.set(key_for_click.clone())
                                    >
                                        {format!(
                                            "{} ({count})",
                                            search_filter_label(&i18n, key_for_label.as_deref())
                                        )}
                                    </button>
                                }
                            }
                        }
                    />
                </div>
            </Show>
            <ul class="workbench-memory-search__results">
                <For
                    each={
                        let s = state.clone();
                        move || {
                            let hits = s.search_results.get();
                            filter_search_hits(hits, search_filter.get())
                        }
                    }
                    key=|h| format!("{:?}:{}:{}", h.scope, h.path, h.line)
                    children={
                        let state = state.clone();
                        move |h: SearchHit| {
                            let s = state.clone();
                            let scope = h.scope.clone();
                            let p = h.path.clone();
                            let p_graph = p.clone();
                            let scope_graph = scope.clone();
                            view! {
                                <li class="workbench-memory-search__hit">
                                    <button
                                        type="button"
                                        class="workbench-memory-search__hit-btn"
                                        on:click={
                                            let p = p.clone();
                                            let sc = scope.clone();
                                            move |_| {
                                                let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                expand_files_group_for_path(s.clone(), &sc, &p);
                                                load_note(s.clone(), ws, sc.clone(), p.clone());
                                                s.view.set(MemoryView::Files);
                                            }
                                        }
                                    >
                                        <span class="workbench-memory-search__hit-path">{h.path.clone()}":"{h.line}</span>
                                        <span class="workbench-memory-search__hit-snippet">{h.snippet.clone()}</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="workbench-memory-search__hit-graph"
                                        title=move || i18n.tr(I18nKey::MemShowInGraph)()
                                        aria-label=move || i18n.tr(I18nKey::MemShowInGraph)()
                                        on:click={
                                            let p_graph = p_graph.clone();
                                            let scope_graph = scope_graph.clone();
                                            move |_| {
                                                navigate_to_graph_node(
                                                    s.clone(),
                                                    scope_graph.clone(),
                                                    p_graph.clone(),
                                                )
                                            }
                                        }
                                    >
                                        <LxIcon icon=icondata::LuNetwork width="0.82rem" height="0.82rem" />
                                    </button>
                                </li>
                            }
                        }
                    }
                />
            </ul>
        </div>
    }
}
