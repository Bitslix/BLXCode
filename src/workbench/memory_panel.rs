//! Workspace-scoped Markdown memory panel — files/editor, backlinks,
//! graph view, search, agent-pointer installer. Mirrors the Phase 1–5
//! design discussed for blxcode's Obsidian-style memory feature.
use crate::agent_wire::{AgentContextItem, AgentContextKind};
use crate::i18n::I18nKey;
use crate::memory_paths::slug_to_filename;
use crate::service::I18nService;
use crate::tauri_bridge::{self, GraphData, NoteContent, NoteMeta, SearchHit};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::memory_graph::MemoryGraphView;
use crate::workbench::state::MemoryCategorySettings;
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
const LEARNINGS_INDEX_PATHS: &[&str] = &["learnings/LEARNINGS.md", "learnings/LEARNIGS.md"];
const MEMORY_INDEX_PATHS: &[&str] = &["README.md", "MEMORY.md"];

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
    pub(crate) editor_content: RwSignal<String>,
    pub(crate) editor_dirty: RwSignal<bool>,
    pub(crate) show_preview: RwSignal<bool>,
    pub(crate) backlinks: RwSignal<Vec<String>>,
    pub(crate) view: RwSignal<MemoryView>,
    pub(crate) error: RwSignal<Option<String>>,
    pub(crate) save_token: RwSignal<u32>,
    pub(crate) graph: RwSignal<Option<GraphData>>,
    pub(crate) search_query: RwSignal<String>,
    pub(crate) search_results: RwSignal<Vec<SearchHit>>,
}

impl MemoryState {
    fn new() -> Self {
        Self {
            workspace_cwd: RwSignal::new(None),
            notes: RwSignal::new(Vec::new()),
            active_path: RwSignal::new(None),
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

fn load_notes(state: MemoryState, ws: String) {
    spawn_local(async move {
        match tauri_bridge::memory_list(&ws).await {
            Ok(list) => {
                state.notes.set(list);
                state.error.set(None);
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

pub(crate) fn load_note(state: MemoryState, ws: String, path: String) {
    spawn_local(async move {
        match tauri_bridge::memory_read(&ws, &path).await {
            Ok(NoteContent { content, .. }) => {
                state.editor_content.set(content);
                state.editor_dirty.set(false);
                state.show_preview.set(true);
                state.active_path.set(Some(path.clone()));
                state.error.set(None);
                // backlinks for this note
                let ws2 = ws.clone();
                let p2 = path.clone();
                spawn_local(async move {
                    match tauri_bridge::memory_backlinks(&ws2, &p2).await {
                        Ok(v) => state.backlinks.set(v),
                        Err(_) => state.backlinks.set(Vec::new()),
                    }
                });
            }
            Err(e) => state.error.set(Some(e)),
        }
    });
}

fn schedule_save(state: MemoryState, ws: String) {
    let token = state.save_token.get_untracked() + 1;
    state.save_token.set(token);
    let save_token = state.save_token;
    let path = state.active_path.get_untracked();
    let Some(path) = path else { return };
    let content = state.editor_content.get_untracked();
    spawn_local(async move {
        TimeoutFuture::new(SAVE_DEBOUNCE_MS).await;
        if save_token.get_untracked() != token {
            return;
        }
        match tauri_bridge::memory_write(&ws, &path, &content).await {
            Ok(_) => {
                state.editor_dirty.set(false);
                // refresh list (mtime/size update); cheap.
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

    // Track active workspace cwd; reload notes when it changes.
    let eff_state = state.clone();
    Effect::new(move |_| {
        let cwd = current_workspace_cwd(wb);
        let prev = eff_state.workspace_cwd.get_untracked();
        if cwd != prev {
            eff_state.workspace_cwd.set(cwd.clone());
            eff_state.active_path.set(None);
            eff_state.editor_content.set(String::new());
            eff_state.editor_dirty.set(false);
            eff_state.backlinks.set(Vec::new());
            eff_state.graph.set(None);
            eff_state.notes.set(Vec::new());
            if let Some(ws) = cwd {
                load_notes(eff_state.clone(), ws);
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
            load_note(st.clone(), ws, rel);
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

            <div class="workbench-memory__body">
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
        </div>
    }
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
    let new_note_input = RwSignal::new(String::new());
    let renaming = RwSignal::new(None::<String>);
    let rename_input = RwSignal::new(String::new());
    let files_collapsed = RwSignal::new(false);
    let groups_open = RwSignal::new(HashSet::<&'static str>::new());
    let context_menu = RwSignal::new(None::<MemoryContextMenu>);
    let editing_category = RwSignal::new(None::<&'static str>);

    view! {
        <div
            class="workbench-memory-files"
            class:workbench-memory-files--collapsed=move || files_collapsed.get()
            on:click=move |_| context_menu.set(None)
        >
            <aside
                class="workbench-memory-files__tree"
                class:workbench-memory-files__tree--collapsed=move || files_collapsed.get()
            >
                <form
                    class="workbench-memory-files__new"
                    class:workbench-memory-files__new--collapsed=move || files_collapsed.get()
                    on:submit={
                        let s = state.clone();
                        move |ev: web_sys::SubmitEvent| {
                            ev.prevent_default();
                            let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                            let raw = new_note_input.get_untracked();
                            if raw.trim().is_empty() { return; }
                            let fname = slug_to_filename(&raw);
                            let s2 = s.clone();
                            spawn_local(async move {
                                match tauri_bridge::memory_create(&ws, &fname, Some(&format!("# {}\n\n", strip_ext(&fname)))).await {
                                    Ok(meta) => {
                                        new_note_input.set(String::new());
                                        load_notes(s2.clone(), ws.clone());
                                        load_note(s2.clone(), ws, meta.path);
                                    }
                                    Err(e) => s2.error.set(Some(e)),
                                }
                            });
                        }
                    }
                >
                    <Show when=move || !files_collapsed.get()>
                        <input
                            type="text"
                            class="workbench-memory-files__new-input"
                            placeholder=move || i18n.tr(I18nKey::MemNewNotePh)()
                            prop:value=move || new_note_input.get()
                            on:input=move |ev| {
                                if let Some(v) = input_value(ev) {
                                    new_note_input.set(v);
                                }
                            }
                        />
                    </Show>
                    <button type="submit" class="workbench-memory-files__new-btn" title=move || i18n.tr(I18nKey::MemNewNote)()>"+"</button>
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
                </form>
                <ul class="workbench-memory-files__list">
                    <For
                        each={
                            let s = state.clone();
                            move || {
                                let _ = wb.workspaces().get();
                                memory_note_groups(s.notes.get())
                            }
                        }
                        key=|g| g.key
                        children={
                            let state = state.clone();
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
                                    />
                                }
                            }
                        }
                    />
                </ul>
            </aside>
            <section class="workbench-memory-editor">
                <Show
                    when={
                        let s = state.clone();
                        move || s.active_path.get().is_some()
                    }
                    fallback={
                        let i = i18n.clone();
                        move || view! {
                            <div class="workbench-memory-editor__empty">
                                <p>{move || i.tr(I18nKey::MemSelectNote)()}</p>
                            </div>
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
                                    key=|p| p.clone()
                                    children={
                                        let s = state.clone();
                                        move |p: String| {
                                            let s = s.clone();
                                            let p2 = p.clone();
                                            view! {
                                                <li>
                                                    <button
                                                        type="button"
                                                        class="workbench-memory-editor__backlink"
                                                        on:click=move |_| {
                                                            let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                            load_note(s.clone(), ws, p2.clone());
                                                        }
                                                    >{p}</button>
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
    group_key: &'static str,
    groups_open: RwSignal<HashSet<&'static str>>,
    files_collapsed: RwSignal<bool>,
    header_title: String,
    index_path: Option<String>,
    group_paths: Vec<String>,
    context_menu: RwSignal<Option<MemoryContextMenu>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let index_active = index_path.clone();
    let index_open = index_path;
    let context_label = header_title.clone();
    view! {
        <li
            class="workbench-memory-files__group-head"
            class:workbench-memory-files__group-head--sidebar-hidden=move || {
                !memory_category_settings(wb, group_key).show_in_sidebar
            }
            class:workbench-memory-files__group-head--hidden=move || files_collapsed.get()
            class:workbench-memory-files__group-head--active=move || {
                memory_group_index_active(&state, &index_active)
            }
            style=move || format!("--memory-category-color: {}", memory_category_settings(wb, group_key).color)
            on:contextmenu=move |ev: web_sys::MouseEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                context_menu.set(Some(MemoryContextMenu {
                    x: ev.client_x(),
                    y: ev.client_y(),
                    target: MemoryContextTarget::Category {
                        key: group_key,
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
                    groups_open.with(|s| s.contains(group_key))
                }
                aria-expanded=move || groups_open.with(|s| s.contains(group_key)).to_string()
                aria-label=move || {
                    if groups_open.with(|s| s.contains(group_key)) {
                        i18n.tr(I18nKey::MemFilesGroupCollapse)()
                    } else {
                        i18n.tr(I18nKey::MemFilesGroupExpand)()
                    }
                }
                on:click=move |ev: web_sys::MouseEvent| {
                    ev.stop_propagation();
                    groups_open.update(|s| {
                        if s.contains(group_key) {
                            s.remove(group_key);
                        } else {
                            s.insert(group_key);
                        }
                    });
                }
            >
                <LxIcon icon=icondata::LuChevronRight width="0.75rem" height="0.75rem" />
            </button>
            <MemoryFileGroupIndexButton
                state=state
                label=header_title
                index_path=index_open
            />
        </li>
    }
}

#[component]
fn MemoryFileGroupIndexButton(
    state: MemoryState,
    label: String,
    index_path: Option<String>,
) -> impl IntoView {
    let title = label.clone();
    view! {
        <button
            type="button"
            class="workbench-memory-files__group-index"
            title=title
            on:click=move |_| memory_open_group_index(state, index_path.clone())
        >
            {label}
        </button>
    }
}

#[component]
fn MemoryFileGroupSection(
    state: MemoryState,
    group: MemoryNoteGroup,
    groups_open: RwSignal<HashSet<&'static str>>,
    files_collapsed: RwSignal<bool>,
    renaming: RwSignal<Option<String>>,
    rename_input: RwSignal<String>,
    context_menu: RwSignal<Option<MemoryContextMenu>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let group_key = group.key;
    let header_title = memory_group_header_label(&group, &i18n, wb);
    let index_path = group.index.as_ref().map(|n| n.path.clone());
    let index = group.index;
    let group_notes = group.notes;
    let show_sidebar = move || {
        wb.active_id()
            .get()
            .map(|id| {
                wb.memory_category_settings_for_workspace(id, group_key)
                    .show_in_sidebar
            })
            .unwrap_or(true)
    };

    view! {
        <MemoryFileGroupHead
            state=state
            wb=wb
            group_key=group_key
            groups_open=groups_open
            files_collapsed=files_collapsed
            header_title=header_title
            index_path=index_path
            group_paths=memory_group_paths(&index, &group_notes)
            context_menu=context_menu
        />
        <For
            each=move || {
                if !show_sidebar() {
                    Vec::new()
                } else if files_collapsed.get() {
                    memory_group_collapsed_items(&index, &group_notes)
                } else if groups_open.with(|s| s.contains(group_key)) {
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
    let path = note.path.clone();
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
                        path: context_path.clone(),
                        label: context_label.clone(),
                    },
                }));
            }
            on:click=move |_| {
                let Some(ws) = state.workspace_cwd.get_untracked() else {
                    return;
                };
                load_note(state, ws, path.clone());
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
                            spawn_local(async move {
                                match tauri_bridge::memory_rename(&ws, &op, &np, true).await {
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
                            load_note(state, ws, path_for_select.clone());
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
                            spawn_local(async move {
                                match tauri_bridge::memory_delete(&ws, &p).await {
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
    editing_category: RwSignal<Option<&'static str>>,
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
                        let edit_key = key;
                        let send_label = label.clone();
                        view! {
                            <div class="workspace-context-menu memory-context-menu" style=style on:click=move |ev| ev.stop_propagation()>
                                <button
                                    type="button"
                                    class="workspace-context-menu__item"
                                    on:click=move |_| {
                                        menu.set(None);
                                        editing_category.set(Some(edit_key));
                                    }
                                >"Edit"</button>
                                <button
                                    type="button"
                                    class="workspace-context-menu__item"
                                    on:click=move |_| {
                                        add_category_agent_context(wb, edit_key, send_label.clone(), paths.clone());
                                        menu.set(None);
                                    }
                                >"Send to BLXCode Agent"</button>
                            </div>
                        }
                        .into_any()
                    }
                    MemoryContextTarget::Note { path, label } => {
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
                                            load_note(state, ws, open_path.clone());
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
fn MemoryCategoryEditDialog(category: &'static str, on_close: Callback<()>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let initial = wb
        .active_id()
        .get_untracked()
        .map(|id| wb.memory_category_settings_for_workspace_untracked(id, category))
        .unwrap_or_else(|| MemoryCategorySettings::for_category(category));
    let label = RwSignal::new(initial.label);
    let color = RwSignal::new(initial.color);
    let show_sidebar = RwSignal::new(initial.show_in_sidebar);
    let show_graph = RwSignal::new(initial.show_in_graph);
    let title = format!("Edit {}", clean_memory_label(category));

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
                                let fallback = MemoryCategorySettings::for_category(category);
                                wb.set_memory_category_settings(ws_id, category, MemoryCategorySettings {
                                    label: label.get_untracked().trim().to_string(),
                                    color: normalize_memory_color(&color.get_untracked(), &fallback.color),
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
    category: &'static str,
    label: String,
    paths: Vec<String>,
) {
    let Some(ws_id) = wb.active_id().get_untracked() else {
        return;
    };
    let kind = if category == "learnings" {
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
        },
    );
}

fn normalize_memory_color(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
    {
        trimmed.to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

#[derive(Clone)]
struct MemoryNoteGroup {
    key: &'static str,
    index: Option<NoteMeta>,
    notes: Vec<NoteMeta>,
}

#[derive(Clone, PartialEq)]
enum MemoryContextTarget {
    Category {
        key: &'static str,
        label: String,
        paths: Vec<String>,
    },
    Note {
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

fn memory_note_groups(notes: Vec<NoteMeta>) -> Vec<MemoryNoteGroup> {
    let mut memory = Vec::new();
    let mut learnings = Vec::new();
    for n in notes {
        if n.path.starts_with(LEARNINGS_API_PREFIX) {
            learnings.push(n);
        } else {
            memory.push(n);
        }
    }
    let mut groups = Vec::new();
    let (mem_index, mem_notes) = split_group_index(memory, is_memory_index_note);
    if mem_index.is_some() || !mem_notes.is_empty() {
        groups.push(MemoryNoteGroup {
            key: "memory",
            index: mem_index,
            notes: mem_notes,
        });
    }
    let (learn_index, learn_notes) = split_group_index(learnings, is_learnings_index_note);
    if learn_index.is_some() || !learn_notes.is_empty() {
        groups.push(MemoryNoteGroup {
            key: "learnings",
            index: learn_index,
            notes: learn_notes,
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

fn is_memory_index_note(note: &NoteMeta) -> bool {
    MEMORY_INDEX_PATHS
        .iter()
        .any(|p| note.path.eq_ignore_ascii_case(p))
}

fn memory_open_group_index(state: MemoryState, index_path: Option<String>) {
    let Some(path) = index_path else {
        return;
    };
    let Some(ws) = state.workspace_cwd.get_untracked() else {
        return;
    };
    load_note(state, ws, path);
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
    if let Some(ws_id) = wb.active_id().get_untracked() {
        let settings = wb.memory_category_settings_for_workspace_untracked(ws_id, group.key);
        if !settings.label.trim().is_empty() {
            return clean_memory_label(&settings.label);
        }
    }
    if let Some(idx) = &group.index {
        return clean_memory_label(&idx.name);
    }
    match group.key {
        "learnings" => i18n.tr(I18nKey::MemFilesGroupLearnings)().to_string(),
        _ => i18n.tr(I18nKey::MemFilesGroupMemory)().to_string(),
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

fn search_hit_category(path: &str) -> &'static str {
    if path.starts_with(LEARNINGS_API_PREFIX) {
        "learnings"
    } else {
        "memory"
    }
}

fn search_filter_categories(hits: &[SearchHit]) -> Vec<String> {
    let mut memory = false;
    let mut learnings = false;
    for h in hits {
        match search_hit_category(&h.path) {
            "learnings" => learnings = true,
            _ => memory = true,
        }
    }
    let mut out = Vec::new();
    if memory {
        out.push("memory".into());
    }
    if learnings {
        out.push("learnings".into());
    }
    out
}

fn search_category_count(hits: &[SearchHit], category: &str) -> usize {
    hits.iter()
        .filter(|h| search_hit_category(&h.path) == category)
        .count()
}

fn search_category_label(i18n: &I18nService, category: &str) -> String {
    match category {
        "learnings" => i18n.tr(I18nKey::MemFilesGroupLearnings)().to_string(),
        _ => i18n.tr(I18nKey::MemFilesGroupMemory)().to_string(),
    }
}

fn filter_search_hits(hits: Vec<SearchHit>, filter: Option<String>) -> Vec<SearchHit> {
    match filter {
        None => hits,
        Some(cat) => hits
            .into_iter()
            .filter(|h| search_hit_category(&h.path) == cat.as_str())
            .collect(),
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
                    move || search_filter_categories(&s.search_results.get()).len() > 1
                }
            >
                <div class="workbench-memory-search__filters" role="group">
                    <button
                        type="button"
                        class="workbench-memory-search__filter"
                        class:workbench-memory-search__filter--active=move || search_filter.get().is_none()
                        on:click=move |_| search_filter.set(None)
                    >
                        {move || {
                            let s = state.clone();
                            let n = s.search_results.get().len();
                            format!("{} ({n})", i18n.tr(I18nKey::MemSearchFilterAll)())
                        }}
                    </button>
                    <For
                        each={
                            let s = state.clone();
                            move || search_filter_categories(&s.search_results.get())
                        }
                        key=|cat| cat.clone()
                        children={
                            let i18n = i18n.clone();
                            let state = state.clone();
                            move |cat: String| {
                                let i18n = i18n.clone();
                                let state = state.clone();
                                let cat_for_active = cat.clone();
                                let cat_for_click = cat.clone();
                                view! {
                                    <button
                                        type="button"
                                        class="workbench-memory-search__filter"
                                        class:workbench-memory-search__filter--active=move || {
                                            search_filter.get() == Some(cat_for_active.clone())
                                        }
                                        on:click=move |_| search_filter.set(Some(cat_for_click.clone()))
                                    >
                                        {move || {
                                            let n = search_category_count(&state.search_results.get(), &cat);
                                            format!("{} ({n})", search_category_label(&i18n, &cat))
                                        }}
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
                    key=|h| format!("{}:{}", h.path, h.line)
                    children={
                        let state = state.clone();
                        move |h: SearchHit| {
                            let s = state.clone();
                            let p = h.path.clone();
                            view! {
                                <li class="workbench-memory-search__hit">
                                    <button
                                        type="button"
                                        class="workbench-memory-search__hit-btn"
                                        on:click=move |_| {
                                            let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                            load_note(s.clone(), ws, p.clone());
                                            s.view.set(MemoryView::Files);
                                        }
                                    >
                                        <span class="workbench-memory-search__hit-path">{h.path.clone()}":"{h.line}</span>
                                        <span class="workbench-memory-search__hit-snippet">{h.snippet.clone()}</span>
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
