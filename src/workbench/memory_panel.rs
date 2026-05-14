//! Workspace-scoped Markdown memory panel — files/editor, backlinks,
//! graph view, search, agent-pointer installer. Mirrors the Phase 1–5
//! design discussed for blxcode's Obsidian-style memory feature.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    self, GraphData, NoteContent, NoteMeta, PointerResult, SearchHit,
};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use pulldown_cmark::{html, Options, Parser};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

const SAVE_DEBOUNCE_MS: u32 = 600;

const AGENT_KEYS: [(&str, &str); 5] = [
    ("claude", "Claude"),
    ("codex", "Codex"),
    ("gemini", "Gemini"),
    ("cursor", "Cursor"),
    ("opencode", "OpenCode"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryView {
    Files,
    Graph,
    Search,
    Agents,
}

#[derive(Clone)]
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
    save_token: Arc<AtomicU32>,
    graph: RwSignal<Option<GraphData>>,
    search_query: RwSignal<String>,
    search_results: RwSignal<Vec<SearchHit>>,
    pointer_status: RwSignal<Vec<PointerResult>>,
    busy: RwSignal<bool>,
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
            save_token: Arc::new(AtomicU32::new(0)),
            graph: RwSignal::new(None),
            search_query: RwSignal::new(String::new()),
            search_results: RwSignal::new(Vec::new()),
            pointer_status: RwSignal::new(Vec::new()),
            busy: RwSignal::new(false),
        }
    }
}

fn current_workspace_cwd(wb: WorkbenchService) -> Option<String> {
    let id = wb.active_id().get()?;
    wb.workspaces().with(|list| {
        list.iter()
            .find(|w| w.id == id && !w.configuring)
            .map(|w| w.cwd.clone())
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
    let token = state.save_token.fetch_add(1, Ordering::Relaxed) + 1;
    let save_token = state.save_token.clone();
    let path = state.active_path.get_untracked();
    let Some(path) = path else { return };
    let content = state.editor_content.get_untracked();
    spawn_local(async move {
        TimeoutFuture::new(SAVE_DEBOUNCE_MS).await;
        if save_token.load(Ordering::Relaxed) != token {
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

fn refresh_pointer_status(state: MemoryState, ws: String) {
    spawn_local(async move {
        if let Ok(s) = tauri_bridge::memory_pointer_status(&ws).await {
            state.pointer_status.set(s);
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
    Effect::new(move |_| {
        let cwd = current_workspace_cwd(wb);
        let prev = state.workspace_cwd.get_untracked();
        if cwd != prev {
            state.workspace_cwd.set(cwd.clone());
            state.active_path.set(None);
            state.editor_content.set(String::new());
            state.editor_dirty.set(false);
            state.backlinks.set(Vec::new());
            state.graph.set(None);
            state.notes.set(Vec::new());
            state.pointer_status.set(Vec::new());
            if let Some(ws) = cwd {
                load_notes(state.clone(), ws.clone());
                refresh_pointer_status(state.clone(), ws);
            }
        }
    });

    view! {
        <div class="workbench-memory" role="region">
            <header class="workbench-memory__tabs" role="tablist">
                <MemoryTabBtn label=I18nKey::MemTabFiles state=state.clone() target=MemoryView::Files icon=icondata::LuFiles />
                <MemoryTabBtn label=I18nKey::MemTabGraph state=state.clone() target=MemoryView::Graph icon=icondata::LuNetwork />
                <MemoryTabBtn label=I18nKey::MemTabSearch state=state.clone() target=MemoryView::Search icon=icondata::LuSearch />
                <MemoryTabBtn label=I18nKey::MemTabAgents state=state.clone() target=MemoryView::Agents icon=icondata::LuPlug />
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
                <Show when={
                    let s = state.clone();
                    move || s.view.get() == MemoryView::Agents
                }>
                    <MemoryAgentsView state=state.clone() />
                </Show>
            </div>
            // hidden i18n consumer so unused-import warnings don't trip on stub locales
            <span class="workbench-memory__hidden-i18n" aria-hidden="true">
                {move || i18n.tr(I18nKey::MemEmptyTitle)()}
            </span>
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

    view! {
        <div class="workbench-memory-files">
            <aside class="workbench-memory-files__tree">
                <form
                    class="workbench-memory-files__new"
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
                    <button type="submit" class="workbench-memory-files__new-btn" title=move || i18n.tr(I18nKey::MemNewNote)()>"+"</button>
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
                                let label = n.name.clone();
                                let folder = n.path.rsplit_once('/').map(|(d, _)| d.to_string());
                                let s = state.clone();
                                let s_active = state.clone();
                                let path_for_active = path.clone();
                                let path_for_select = path.clone();
                                let path_for_del = path.clone();
                                let path_for_ren = path.clone();
                                let label_for_ren = label.clone();
                                view! {
                                    <li
                                        class="workbench-memory-files__item"
                                        class:workbench-memory-files__item--active=move || {
                                            s_active.active_path.get().as_deref() == Some(path_for_active.as_str())
                                        }
                                    >
                                        <Show
                                            when=move || renaming.get().as_deref() == Some(path_for_ren.as_str())
                                            fallback={
                                                let s = s.clone();
                                                let label = label.clone();
                                                let folder = folder.clone();
                                                let path_for_select = path_for_select.clone();
                                                let path_for_del = path_for_del.clone();
                                                let path_for_ren = path_for_ren.clone();
                                                let label_for_ren = label_for_ren.clone();
                                                move || view! {
                                                    <button
                                                        type="button"
                                                        class="workbench-memory-files__name"
                                                        on:click={
                                                            let s = s.clone();
                                                            let p = path_for_select.clone();
                                                            move |_| {
                                                                let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                                load_note(s.clone(), ws, p.clone());
                                                            }
                                                        }
                                                    >
                                                        <span class="workbench-memory-files__name-text">{label.clone()}</span>
                                                        {folder.clone().map(|f| view! { <small class="workbench-memory-files__folder">{f}</small> })}
                                                    </button>
                                                    <button
                                                        type="button"
                                                        class="workbench-memory-files__action"
                                                        title=move || i18n.tr(I18nKey::MemRename)()
                                                        on:click={
                                                            let label_for_ren = label_for_ren.clone();
                                                            let path_for_ren = path_for_ren.clone();
                                                            move |_| {
                                                                rename_input.set(label_for_ren.clone());
                                                                renaming.set(Some(path_for_ren.clone()));
                                                            }
                                                        }
                                                    >"✎"</button>
                                                    <button
                                                        type="button"
                                                        class="workbench-memory-files__action workbench-memory-files__action--danger"
                                                        title=move || i18n.tr(I18nKey::MemDelete)()
                                                        on:click={
                                                            let s = s.clone();
                                                            let p = path_for_del.clone();
                                                            move |_| {
                                                                let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                                let s2 = s.clone();
                                                                let p2 = p.clone();
                                                                spawn_local(async move {
                                                                    match tauri_bridge::memory_delete(&ws, &p2).await {
                                                                        Ok(()) => {
                                                                            if s2.active_path.get_untracked().as_deref() == Some(p2.as_str()) {
                                                                                s2.active_path.set(None);
                                                                                s2.editor_content.set(String::new());
                                                                                s2.backlinks.set(Vec::new());
                                                                            }
                                                                            load_notes(s2, ws);
                                                                        }
                                                                        Err(e) => s2.error.set(Some(e)),
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    >"🗑"</button>
                                                }
                                            }
                                        >
                                            {
                                                let s = s.clone();
                                                let path_for_ren = path_for_ren.clone();
                                                view! {
                                                    <form
                                                        class="workbench-memory-files__rename"
                                                        on:submit={
                                                            let s = s.clone();
                                                            let old_path = path_for_ren.clone();
                                                            move |ev: web_sys::SubmitEvent| {
                                                                ev.prevent_default();
                                                                let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                                let v = rename_input.get_untracked();
                                                                let new_name = slug_to_filename(&v);
                                                                // preserve folder
                                                                let new_path = if let Some(parent) = old_path.rsplit_once('/').map(|(d, _)| d) {
                                                                    format!("{parent}/{new_name}")
                                                                } else {
                                                                    new_name
                                                                };
                                                                let s2 = s.clone();
                                                                let op = old_path.clone();
                                                                let np = new_path.clone();
                                                                spawn_local(async move {
                                                                    match tauri_bridge::memory_rename(&ws, &op, &np, true).await {
                                                                        Ok(_) => {
                                                                            renaming.set(None);
                                                                            if s2.active_path.get_untracked().as_deref() == Some(op.as_str()) {
                                                                                s2.active_path.set(Some(np.clone()));
                                                                            }
                                                                            load_notes(s2, ws);
                                                                        }
                                                                        Err(e) => s2.error.set(Some(e)),
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    >
                                                        <input
                                                            type="text"
                                                            class="workbench-memory-files__rename-input"
                                                            prop:value=move || rename_input.get()
                                                            on:input=move |ev| {
                                                                if let Some(v) = input_value(ev) { rename_input.set(v); }
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
                                            }
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
                        <span class="workbench-memory-editor__path">
                            {
                                let s = state.clone();
                                move || s.active_path.get().unwrap_or_default()
                            }
                        </span>
                        <span class="workbench-memory-editor__flag" aria-live="polite">
                            {
                                let s = state.clone();
                                let i = i18n.clone();
                                move || if s.editor_dirty.get() { i.tr(I18nKey::MemDirty)() } else { String::new() }
                            }
                        </span>
                        <button
                            type="button"
                            class="workbench-memory-editor__preview-btn"
                            on:click={
                                let s = state.clone();
                                move |_| s.show_preview.update(|v| *v = !*v)
                            }
                        >
                            {
                                let s = state.clone();
                                let i = i18n.clone();
                                move || if s.show_preview.get() { i.tr(I18nKey::MemEdit)() } else { i.tr(I18nKey::MemPreview)() }
                            }
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
fn MemoryGraphView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let layout = RwSignal::new(HashMap::<String, (f32, f32)>::new());

    Effect::new({
        let state = state.clone();
        move |_| {
            if state.view.get() != MemoryView::Graph {
                return;
            }
            let Some(ws) = state.workspace_cwd.get() else { return };
            refresh_graph(state.clone(), ws);
        }
    });

    Effect::new({
        let state = state.clone();
        move |_| {
            let Some(g) = state.graph.get() else { return };
            layout.set(force_layout(&g, 400.0, 320.0, 180));
        }
    });

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
                <svg class="workbench-memory-graph__svg" viewBox="0 0 400 320" xmlns="http://www.w3.org/2000/svg">
                    // edges
                    <g class="workbench-memory-graph__edges">
                        <For
                            each={
                                let s = state.clone();
                                move || s.graph.get().map(|g| g.edges).unwrap_or_default()
                            }
                            key=|e| format!("{}->{}", e.source, e.target)
                            children=move |e| {
                                let pos = layout.get();
                                let src = pos.get(&e.source).copied();
                                let tgt = pos.get(&e.target).copied();
                                match (src, tgt) {
                                    (Some((x1, y1)), Some((x2, y2))) => view! {
                                        <line
                                            x1=x1.to_string()
                                            y1=y1.to_string()
                                            x2=x2.to_string()
                                            y2=y2.to_string()
                                            stroke="rgba(255,255,255,0.18)"
                                            stroke-width="1"
                                        />
                                    }.into_any(),
                                    _ => view! { <g /> }.into_any(),
                                }
                            }
                        />
                    </g>
                    // nodes
                    <g class="workbench-memory-graph__nodes">
                        <For
                            each={
                                let s = state.clone();
                                move || s.graph.get().map(|g| g.nodes).unwrap_or_default()
                            }
                            key=|n| n.id.clone()
                            children={
                                let state = state.clone();
                                move |n| {
                                    let pos = layout.get();
                                    let (x, y) = pos.get(&n.id).copied().unwrap_or((200.0, 160.0));
                                    let s = state.clone();
                                    let id_for_click = n.id.clone();
                                    let fill = if n.orphan { "rgba(180,180,200,0.35)" } else { "rgba(120,170,255,0.85)" };
                                    view! {
                                        <g class="workbench-memory-graph__node"
                                            on:click=move |_| {
                                                let Some(ws) = s.workspace_cwd.get_untracked() else { return };
                                                load_note(s.clone(), ws, id_for_click.clone());
                                                s.view.set(MemoryView::Files);
                                            }
                                        >
                                            <circle cx=x.to_string() cy=y.to_string() r="6" fill=fill stroke="rgba(255,255,255,0.4)" stroke-width="0.5" />
                                            <text x=(x + 8.0).to_string() y=(y + 3.0).to_string() font-size="9" fill="rgba(238,239,245,0.9)">{n.label}</text>
                                        </g>
                                    }
                                }
                            }
                        />
                    </g>
                </svg>
                <p class="workbench-memory-graph__legend">
                    {move || i18n.tr(I18nKey::MemGraphLegend)()}
                </p>
            </Show>
        </div>
    }
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
            let d = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1).sqrt().max(0.001);
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
    let debounce_token: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));

    let on_input = {
        let state = state.clone();
        let debounce_token = debounce_token.clone();
        move |ev: web_sys::Event| {
            let v = input_value(ev).unwrap_or_default();
            state.search_query.set(v.clone());
            let token = debounce_token.fetch_add(1, Ordering::Relaxed) + 1;
            let s = state.clone();
            let dt = debounce_token.clone();
            spawn_local(async move {
                TimeoutFuture::new(200).await;
                if dt.load(Ordering::Relaxed) != token {
                    return;
                }
                let Some(ws) = s.workspace_cwd.get_untracked() else { return };
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

#[component]
fn MemoryAgentsView(state: MemoryState) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <div class="workbench-memory-agents">
            <p class="workbench-memory-agents__lead">
                {move || i18n.tr(I18nKey::MemAgentsLead)()}
            </p>
            <ul class="workbench-memory-agents__list">
                {AGENT_KEYS.iter().map(|(key, label)| {
                    let key = (*key).to_string();
                    let label = (*label).to_string();
                    let state = state.clone();
                    let key_for_status = key.clone();
                    let key_for_install = key.clone();
                    let key_for_uninstall = key.clone();
                    let i18n_install = i18n.clone();
                    let i18n_uninstall = i18n.clone();
                    let s_install = state.clone();
                    let s_uninstall = state.clone();
                    let s_status = state.clone();
                    view! {
                        <li class="workbench-memory-agents__item">
                            <span class="workbench-memory-agents__name">{label}</span>
                            <span class="workbench-memory-agents__status">
                                {move || {
                                    let installed = s_status.pointer_status.get().iter()
                                        .find(|p| p.agent == key_for_status)
                                        .map(|p| p.installed)
                                        .unwrap_or(false);
                                    if installed { i18n.tr(I18nKey::MemAgentsInstalled)() } else { i18n.tr(I18nKey::MemAgentsMissing)() }
                                }}
                            </span>
                            <button
                                type="button"
                                class="workbench-memory-agents__btn"
                                on:click=move |_| {
                                    let Some(ws) = s_install.workspace_cwd.get_untracked() else { return };
                                    let s2 = s_install.clone();
                                    let agents = vec![key_for_install.clone()];
                                    s_install.busy.set(true);
                                    spawn_local(async move {
                                        match tauri_bridge::memory_install_pointers(&ws, agents).await {
                                            Ok(_) => refresh_pointer_status(s2.clone(), ws),
                                            Err(e) => s2.error.set(Some(e)),
                                        }
                                        s2.busy.set(false);
                                    });
                                }
                            >
                                {move || i18n_install.tr(I18nKey::MemAgentsInstall)()}
                            </button>
                            <button
                                type="button"
                                class="workbench-memory-agents__btn workbench-memory-agents__btn--ghost"
                                on:click=move |_| {
                                    let Some(ws) = s_uninstall.workspace_cwd.get_untracked() else { return };
                                    let s2 = s_uninstall.clone();
                                    let agents = vec![key_for_uninstall.clone()];
                                    spawn_local(async move {
                                        match tauri_bridge::memory_uninstall_pointers(&ws, agents).await {
                                            Ok(_) => refresh_pointer_status(s2.clone(), ws),
                                            Err(e) => s2.error.set(Some(e)),
                                        }
                                    });
                                }
                            >
                                {move || i18n_uninstall.tr(I18nKey::MemAgentsUninstall)()}
                            </button>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
}
