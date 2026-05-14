//! Workspace-scoped Markdown memory panel — files/editor, backlinks,
//! graph view, search, agent-pointer installer. Mirrors the Phase 1–5
//! design discussed for blxcode's Obsidian-style memory feature.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{self, GraphData, NoteContent, NoteMeta, SearchHit};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use pulldown_cmark::{html, Options, Parser};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

const SAVE_DEBOUNCE_MS: u32 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryView {
    Files,
    Graph,
    Search,
}

#[derive(Clone, Copy)]
struct MemoryState {
    workspace_cwd: RwSignal<Option<String>>,
    notes: RwSignal<Vec<NoteMeta>>,
    active_path: RwSignal<Option<String>>,
    editor_content: RwSignal<String>,
    editor_dirty: RwSignal<bool>,
    show_preview: RwSignal<bool>,
    backlinks: RwSignal<Vec<String>>,
    view: RwSignal<MemoryView>,
    error: RwSignal<Option<String>>,
    save_token: RwSignal<u32>,
    graph: RwSignal<Option<GraphData>>,
    search_query: RwSignal<String>,
    search_results: RwSignal<Vec<SearchHit>>,
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

fn slug_to_filename(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return "untitled.md".into();
    }
    let lower = trimmed.contains('/') || trimmed.contains('\\');
    let base = if lower {
        trimmed.replace('\\', "/")
    } else {
        trimmed.to_owned()
    };
    if base.to_ascii_lowercase().ends_with(".md") {
        base
    } else {
        format!("{base}.md")
    }
}

fn render_markdown(src: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    // Expand `[[wikilinks]]` to plain emphasised text before passing
    // to pulldown so users see them clearly in preview.
    let mut prepped = String::with_capacity(src.len());
    let mut i = 0;
    let b = src.as_bytes();
    while i < b.len() {
        if i + 1 < b.len() && b[i] == b'[' && b[i + 1] == b'[' {
            if let Some(end) = src[i + 2..].find("]]") {
                let inner = &src[i + 2..i + 2 + end];
                let label = inner.split('|').next().unwrap_or(inner).trim();
                prepped.push_str("[[");
                prepped.push_str(label);
                prepped.push_str("]]");
                i += 2 + end + 2;
                continue;
            }
        }
        let ch = src[i..].chars().next().unwrap();
        prepped.push(ch);
        i += ch.len_utf8();
    }
    let parser = Parser::new_ext(&prepped, opts);
    let mut html_out = String::with_capacity(prepped.len() * 2);
    html::push_html(&mut html_out, parser);
    html_out
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

fn load_note(state: MemoryState, ws: String, path: String) {
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

fn refresh_graph(state: MemoryState, ws: String) {
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
                        aria-label=move || if files_collapsed.get() { "Expand file list" } else { "Collapse file list" }
                        title=move || if files_collapsed.get() { "Expand file list" } else { "Collapse file list" }
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
                            move || s.notes.get()
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
                                    inner_html=move || render_markdown(&s.editor_content.get())
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
    let folder = note.path.rsplit_once('/').map(|(d, _)| d.to_string());
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

#[component]
fn MemoryGraphView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let layout = RwSignal::new(HashMap::<String, (f32, f32)>::new());
    let viewbox = RwSignal::new((0.0_f32, 0.0_f32, 400.0_f32, 320.0_f32));
    let panning = RwSignal::new(false);
    let last_pos = RwSignal::new((0.0_f32, 0.0_f32));
    let user_interacted = RwSignal::new(false);
    let last_node_set: RwSignal<Vec<String>> = RwSignal::new(Vec::new());
    let hovered: RwSignal<Option<String>> = RwSignal::new(None);

    Effect::new({
        let state = state.clone();
        move |_| {
            if state.view.get() != MemoryView::Graph {
                return;
            }
            let Some(ws) = state.workspace_cwd.get() else {
                return;
            };
            refresh_graph(state.clone(), ws);
        }
    });

    let fit_viewbox = move |pos: &HashMap<String, (f32, f32)>| {
        if pos.is_empty() {
            viewbox.set((0.0, 0.0, 400.0, 320.0));
            return;
        }
        let (mut minx, mut miny, mut maxx, mut maxy) =
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pos.values() {
            minx = minx.min(x);
            maxx = maxx.max(x);
            miny = miny.min(y);
            maxy = maxy.max(y);
        }
        let pad = 60.0;
        let vw = (maxx - minx + pad * 2.0).max(120.0);
        let vh = (maxy - miny + pad * 2.0).max(120.0);
        viewbox.set((minx - pad, miny - pad, vw, vh));
    };

    Effect::new({
        let state = state.clone();
        move |_| {
            let Some(g) = state.graph.get() else { return };
            let pos = force_layout(&g, 400.0, 320.0, 180);

            let mut ids: Vec<String> = g.nodes.iter().map(|n| n.id.clone()).collect();
            ids.sort();
            let prev = last_node_set.get_untracked();
            let node_set_changed = prev != ids;
            if node_set_changed {
                last_node_set.set(ids);
            }

            if !user_interacted.get_untracked() || node_set_changed {
                fit_viewbox(&pos);
                if node_set_changed {
                    user_interacted.set(false);
                }
            }
            layout.set(pos);
        }
    });

    let reset_view = move |_| {
        let pos = layout.get_untracked();
        fit_viewbox(&pos);
        user_interacted.set(false);
    };

    let on_wheel = move |ev: web_sys::WheelEvent| {
        ev.prevent_default();
        let Some(t) = ev.current_target() else { return };
        let Ok(svg) = t.dyn_into::<web_sys::Element>() else { return };
        let rect = svg.get_bounding_client_rect();
        let w = rect.width() as f32;
        let h = rect.height() as f32;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let mx = ev.client_x() as f32 - rect.left() as f32;
        let my = ev.client_y() as f32 - rect.top() as f32;
        let (vx, vy, vw, vh) = viewbox.get_untracked();
        let sx = vx + (mx / w) * vw;
        let sy = vy + (my / h) * vh;
        let factor = if ev.delta_y() > 0.0 { 1.15 } else { 1.0 / 1.15 };
        let new_vw = (vw * factor).clamp(20.0, 8000.0);
        let new_vh = (vh * factor).clamp(20.0, 8000.0);
        let new_vx = sx - (mx / w) * new_vw;
        let new_vy = sy - (my / h) * new_vh;
        viewbox.set((new_vx, new_vy, new_vw, new_vh));
        user_interacted.set(true);
    };

    let on_mousedown = move |ev: web_sys::MouseEvent| {
        panning.set(true);
        last_pos.set((ev.client_x() as f32, ev.client_y() as f32));
    };
    let on_mousemove = move |ev: web_sys::MouseEvent| {
        if !panning.get_untracked() {
            return;
        }
        let (lx, ly) = last_pos.get_untracked();
        let dx = ev.client_x() as f32 - lx;
        let dy = ev.client_y() as f32 - ly;
        last_pos.set((ev.client_x() as f32, ev.client_y() as f32));
        let Some(t) = ev.current_target() else { return };
        let Ok(svg) = t.dyn_into::<web_sys::Element>() else { return };
        let rect = svg.get_bounding_client_rect();
        let w = rect.width() as f32;
        let h = rect.height() as f32;
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (vx, vy, vw, vh) = viewbox.get_untracked();
        viewbox.set((vx - dx * (vw / w), vy - dy * (vh / h), vw, vh));
        if dx.abs() + dy.abs() > 0.0 {
            user_interacted.set(true);
        }
    };
    let on_mouseup = move |_: web_sys::MouseEvent| panning.set(false);
    let on_mouseleave = move |_: web_sys::MouseEvent| panning.set(false);

    let viewbox_str = move || {
        let (x, y, w, h) = viewbox.get();
        format!("{} {} {} {}", x, y, w, h)
    };

    view! {
        <div class="workbench-memory-graph">
            <Show
                when={
                    let s = state.clone();
                    move || s.graph.get().as_ref().map(|g| !g.nodes.is_empty()).unwrap_or(false)
                }
                fallback={
                    let i = i18n.clone();
                    move || view! {
                        <p class="workbench-memory-graph__empty">{move || i.tr(I18nKey::MemGraphEmpty)()}</p>
                    }
                }
            >
                <div class="workbench-memory-graph__toolbar">
                    <button
                        class="workbench-memory-graph__btn"
                        on:click=reset_view
                        title="Reset view"
                    >"Reset"</button>
                </div>
                <svg
                    class="workbench-memory-graph__svg"
                    viewBox=viewbox_str
                    xmlns="http://www.w3.org/2000/svg"
                    on:wheel=on_wheel
                    on:mousedown=on_mousedown
                    on:mousemove=on_mousemove
                    on:mouseup=on_mouseup
                    on:mouseleave=on_mouseleave
                >
                    // edges
                    <g class="workbench-memory-graph__edges">
                        {
                            let s = state.clone();
                            move || {
                                let pos = layout.get();
                                let edges = s.graph.get().map(|g| g.edges).unwrap_or_default();
                                let hov = hovered.get();
                                edges.into_iter().filter_map(|e| {
                                    let (x1, y1) = *pos.get(&e.source)?;
                                    let (x2, y2) = *pos.get(&e.target)?;
                                    let incident = match hov.as_deref() {
                                        Some(h) => e.source == h || e.target == h,
                                        None => true,
                                    };
                                    let (stroke, width) = if hov.is_none() {
                                        ("rgba(255,255,255,0.18)", "1")
                                    } else if incident {
                                        ("rgba(180,210,255,0.85)", "1.6")
                                    } else {
                                        ("rgba(255,255,255,0.04)", "1")
                                    };
                                    Some(view! {
                                        <line
                                            x1=x1.to_string()
                                            y1=y1.to_string()
                                            x2=x2.to_string()
                                            y2=y2.to_string()
                                            stroke=stroke
                                            stroke-width=width
                                        />
                                    })
                                }).collect::<Vec<_>>()
                            }
                        }
                    </g>
                    // nodes
                    <g class="workbench-memory-graph__nodes">
                        {
                            let s = state.clone();
                            move || {
                                let pos = layout.get();
                                let graph = s.graph.get();
                                let nodes = graph.as_ref().map(|g| g.nodes.clone()).unwrap_or_default();
                                let edges = graph.as_ref().map(|g| g.edges.clone()).unwrap_or_default();
                                let degrees = compute_degrees(&nodes, &edges);
                                let neighbors = compute_neighbors(&edges);
                                let hov = hovered.get();
                                let (_, _, vw, vh) = viewbox.get();
                                // Hide labels when zoomed far out (each pixel covers many viewBox units).
                                let zoom_scale = (vw * vh).sqrt();
                                let show_labels = zoom_scale < 900.0;
                                nodes.into_iter().filter_map(|n| {
                                    let (x, y) = *pos.get(&n.id)?;
                                    let deg = degrees.get(&n.id).copied().unwrap_or(0);
                                    let radius = 4.0_f32 + (deg as f32).sqrt() * 2.5;
                                    let (focus_state, is_hovered) = match hov.as_deref() {
                                        Some(h) if h == n.id => (NodeFocus::Hovered, true),
                                        Some(h) => {
                                            let near = neighbors.get(h).map(|set| set.contains(&n.id)).unwrap_or(false);
                                            (if near { NodeFocus::Neighbor } else { NodeFocus::Dim }, false)
                                        }
                                        None => (NodeFocus::Normal, false),
                                    };
                                    let base_fill = cluster_color(&n.tags, n.orphan);
                                    let fill = match focus_state {
                                        NodeFocus::Dim => fade_color(&base_fill, 0.18),
                                        _ => base_fill,
                                    };
                                    let stroke = match focus_state {
                                        NodeFocus::Hovered => "rgba(255,255,255,0.95)",
                                        NodeFocus::Neighbor => "rgba(255,255,255,0.6)",
                                        NodeFocus::Dim => "rgba(255,255,255,0.08)",
                                        NodeFocus::Normal => "rgba(255,255,255,0.4)",
                                    };
                                    let stroke_width = if matches!(focus_state, NodeFocus::Hovered) { "1.4" } else { "0.5" };
                                    let label_opacity = match focus_state {
                                        NodeFocus::Hovered => 1.0_f32,
                                        NodeFocus::Neighbor => 0.95,
                                        NodeFocus::Dim => 0.0,
                                        NodeFocus::Normal => if show_labels { 0.9 } else { 0.0 },
                                    };
                                    let label_force_visible = is_hovered;
                                    let s_click = s.clone();
                                    let id_for_click = n.id.clone();
                                    let id_for_enter = n.id.clone();
                                    let label = n.label.clone();
                                    Some(view! {
                                        <g class="workbench-memory-graph__node"
                                            on:click=move |_| {
                                                let Some(ws) = s_click.workspace_cwd.get_untracked() else { return };
                                                load_note(s_click.clone(), ws, id_for_click.clone());
                                                s_click.view.set(MemoryView::Files);
                                            }
                                            on:mouseenter=move |_| hovered.set(Some(id_for_enter.clone()))
                                            on:mouseleave=move |_| hovered.set(None)
                                        >
                                            <circle
                                                cx=x.to_string()
                                                cy=y.to_string()
                                                r=radius.to_string()
                                                fill=fill
                                                stroke=stroke
                                                stroke-width=stroke_width
                                            />
                                            {(label_opacity > 0.0 || label_force_visible).then(|| view! {
                                                <text
                                                    x=(x + radius + 3.0).to_string()
                                                    y=(y + 3.0).to_string()
                                                    font-size="9"
                                                    fill="rgba(238,239,245,0.95)"
                                                    opacity=label_opacity.to_string()
                                                >{label}</text>
                                            })}
                                        </g>
                                    })
                                }).collect::<Vec<_>>()
                            }
                        }
                    </g>
                </svg>
                <p class="workbench-memory-graph__legend">
                    {move || i18n.tr(I18nKey::MemGraphLegend)()}
                </p>
            </Show>
        </div>
    }
}

#[derive(Clone, Copy)]
enum NodeFocus { Normal, Hovered, Neighbor, Dim }

fn compute_degrees(
    nodes: &[crate::tauri_bridge::GraphNode],
    edges: &[crate::tauri_bridge::GraphEdge],
) -> HashMap<String, u32> {
    let mut d: HashMap<String, u32> = nodes.iter().map(|n| (n.id.clone(), 0)).collect();
    for e in edges {
        if let Some(v) = d.get_mut(&e.source) { *v += 1; }
        if let Some(v) = d.get_mut(&e.target) { *v += 1; }
    }
    d
}

fn compute_neighbors(
    edges: &[crate::tauri_bridge::GraphEdge],
) -> HashMap<String, std::collections::HashSet<String>> {
    let mut m: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for e in edges {
        m.entry(e.source.clone()).or_default().insert(e.target.clone());
        m.entry(e.target.clone()).or_default().insert(e.source.clone());
    }
    m
}

/// Stable hash → hue color for cluster grouping by first tag.
/// Orphans get a neutral grey. Notes without tags fall back to a hash of the id (group by basename prefix).
fn cluster_color(tags: &[String], orphan: bool) -> String {
    if orphan {
        return "rgba(170,170,185,0.55)".to_string();
    }
    let key = tags.first().cloned();
    let hue = match key {
        Some(t) => stable_hue(&t),
        None => 215.0, // default blue when untagged
    };
    format!("hsla({:.0}, 70%, 64%, 0.9)", hue)
}

fn stable_hue(s: &str) -> f32 {
    // FNV-1a 32-bit
    let mut h: u32 = 0x811c9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    (h % 360) as f32
}

fn fade_color(css: &str, alpha: f32) -> String {
    // Replace last alpha component in hsla(...) / rgba(...). Falls back to wrapping in <g opacity> via re-emit.
    if let Some(open) = css.find('(') {
        if let Some(close) = css.rfind(')') {
            let inner = &css[open + 1..close];
            let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
            if parts.len() == 4 {
                let prefix = &css[..open + 1];
                return format!("{}{}, {}, {}, {:.3})", prefix, parts[0], parts[1], parts[2], alpha);
            }
        }
    }
    css.to_string()
}

/// Simple force-directed layout. O(n²) per iteration; fine for <500 nodes.
fn force_layout(g: &GraphData, w: f32, h: f32, iters: u32) -> HashMap<String, (f32, f32)> {
    let n = g.nodes.len();
    if n == 0 {
        return HashMap::new();
    }
    // initial: circle
    let mut pos: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = (i as f32) / (n.max(1) as f32) * std::f32::consts::TAU;
            let r = (w.min(h) * 0.35).max(20.0);
            (w / 2.0 + r * a.cos(), h / 2.0 + r * a.sin())
        })
        .collect();
    let idx: HashMap<&str, usize> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let edges: Vec<(usize, usize)> = g
        .edges
        .iter()
        .filter_map(|e| Some((*idx.get(e.source.as_str())?, *idx.get(e.target.as_str())?)))
        .collect();
    let k = (w * h / (n as f32 + 1.0)).sqrt().max(20.0);
    let mut t = w.min(h) * 0.1;
    for _ in 0..iters {
        let mut disp = vec![(0.0_f32, 0.0_f32); n];
        // repulsion
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let dist = (dx * dx + dy * dy).sqrt().max(0.5);
                let force = k * k / dist;
                disp[i].0 += dx / dist * force;
                disp[i].1 += dy / dist * force;
            }
        }
        // attraction along edges
        for &(a, b) in &edges {
            let dx = pos[a].0 - pos[b].0;
            let dy = pos[a].1 - pos[b].1;
            let dist = (dx * dx + dy * dy).sqrt().max(0.5);
            let force = dist * dist / k;
            let ux = dx / dist * force;
            let uy = dy / dist * force;
            disp[a].0 -= ux;
            disp[a].1 -= uy;
            disp[b].0 += ux;
            disp[b].1 += uy;
        }
        // apply with cooling and bounds
        for i in 0..n {
            let d = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1)
                .sqrt()
                .max(0.001);
            let limit = t.min(d);
            pos[i].0 += disp[i].0 / d * limit;
            pos[i].1 += disp[i].1 / d * limit;
            pos[i].0 = pos[i].0.clamp(10.0, w - 10.0);
            pos[i].1 = pos[i].1.clamp(10.0, h - 10.0);
        }
        t *= 0.96;
    }
    g.nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), pos[i]))
        .collect()
}

#[component]
fn MemorySearchView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let debounce_token: RwSignal<u32> = RwSignal::new(0);

    let on_input = {
        move |ev: web_sys::Event| {
            let v = input_value(ev).unwrap_or_default();
            state.search_query.set(v.clone());
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
            <ul class="workbench-memory-search__results">
                <For
                    each={
                        let s = state.clone();
                        move || s.search_results.get()
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
