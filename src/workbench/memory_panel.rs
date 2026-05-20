//! Workspace-scoped Markdown memory panel — files/editor, backlinks,
//! graph view, search, agent-pointer installer. Mirrors the Phase 1–5
//! design discussed for blxcode's Obsidian-style memory feature.
use crate::i18n::I18nKey;
use crate::memory_paths::slug_to_filename;
use crate::service::I18nService;
use crate::tauri_bridge::{self, GraphData, NoteContent, NoteMeta, SearchHit};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::memory_graph::MemoryGraphView;
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
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
    let new_note_input = RwSignal::new(String::new());
    let renaming = RwSignal::new(None::<String>);
    let rename_input = RwSignal::new(String::new());
    let files_collapsed = RwSignal::new(false);
    let groups_open = RwSignal::new(HashSet::<&'static str>::new());

    view! {
        <div
            class="workbench-memory-files"
            class:workbench-memory-files--collapsed=move || files_collapsed.get()
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
                            move || memory_note_groups(s.notes.get())
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
    group_key: &'static str,
    groups_open: RwSignal<HashSet<&'static str>>,
    files_collapsed: RwSignal<bool>,
    header_title: String,
    index_path: Option<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let index_active = index_path.clone();
    let index_open = index_path;

    view! {
        <li
            class="workbench-memory-files__group-head"
            class:workbench-memory-files__group-head--hidden=move || files_collapsed.get()
            class:workbench-memory-files__group-head--active=move || {
                memory_group_index_active(&state, &index_active)
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
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let group_key = group.key;
    let header_title = memory_group_header_label(&group, &i18n);
    let index_path = group.index.as_ref().map(|n| n.path.clone());
    let index = group.index;
    let group_notes = group.notes;

    view! {
        <MemoryFileGroupHead
            state=state
            group_key=group_key
            groups_open=groups_open
            files_collapsed=files_collapsed
            header_title=header_title
            index_path=index_path
        />
        <For
            each=move || {
                if files_collapsed.get() {
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
                                    />
                                }
                            >
                                <MemoryFileCollapsedRow state=state note=collapsed_note.clone() />
                            </Show>
                        </li>
                    }
                }
            }
        />
    }
}

#[component]
fn MemoryFileCollapsedRow(state: MemoryState, note: NoteMeta) -> impl IntoView {
    let label = note.name.clone();
    let badge = note_badge_text(&label);
    let path = note.path.clone();

    view! {
        <button
            type="button"
            class="workbench-memory-files__badge"
            title=label.clone()
            aria-label=label
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
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let label = note.name.clone();
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

#[derive(Clone)]
struct MemoryNoteGroup {
    key: &'static str,
    index: Option<NoteMeta>,
    notes: Vec<NoteMeta>,
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

fn memory_group_header_label(group: &MemoryNoteGroup, i18n: &I18nService) -> String {
    if let Some(idx) = &group.index {
        return idx.name.clone();
    }
    match group.key {
        "learnings" => i18n.tr(I18nKey::MemFilesGroupLearnings)().to_string(),
        _ => i18n.tr(I18nKey::MemFilesGroupMemory)().to_string(),
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
