//! Sidebar project file tree (workspace `cwd`).

use crate::config::SIDEBAR_EXPLORER_SHOW_HIDDEN_KEY;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    create_workspace_dir, create_workspace_file, is_tauri_shell, list_path_entries, FsEntryBrief,
};
use crate::workbench::sidebar_view_section::{SidebarSectionIconBtn, SidebarViewSection};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::HashSet;

/// Pending inline-create row. `parent_rel` is the folder it will be created in
/// (empty string = workspace root); `is_dir` selects folder vs. empty file.
#[derive(Clone)]
struct Draft {
    parent_rel: String,
    is_dir: bool,
}

/// Starts an inline-create draft in `parent_rel` (empty = root). Threaded down
/// to the per-folder hover actions so any folder row can begin a draft.
type StartDraft = Callback<(String, bool)>;

#[component]
pub fn ProjectExplorerSection() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let collapsed = wb.sidebar_collapsed();

    // Narrow projection of the active workspace to just the identity-relevant
    // fields (id, cwd, configuring). Memo equality on this tuple ensures the
    // file browser doesn't re-render or invalidate its cache when unrelated
    // workspace state (e.g. the agent chat timeline) changes — that previously
    // caused the left tree to flicker on every right-side chat update.
    let active_workspace: Memo<Option<(u64, String, bool)>> = Memo::new(move |_| {
        let active_id = wb.active_id().get();
        wb.workspaces()
            .get()
            .into_iter()
            .find(|w| Some(w.id) == active_id)
            .map(|w| (w.id, w.cwd, w.configuring))
    });
    let section_title = Signal::derive(move || i18n.tr(I18nKey::SbExplorerTitle)().to_string());

    let explorer_open = RwSignal::new(wb.active_sidebar_explorer_open());
    let open_paths = RwSignal::new(HashSet::<String>::new());
    let children_cache = RwSignal::new(HashMap::<String, Vec<FsEntryBrief>>::new());
    let load_gen = RwSignal::new(0u32);
    let error_msg = RwSignal::new(None::<String>);
    let show_hidden = RwSignal::new(read_explorer_show_hidden());
    let draft = RwSignal::new(None::<Draft>);
    // Folder the toolbar New File/Folder actions target (None = workspace root).
    let selected_dir = RwSignal::new(None::<String>);

    // Re-sync section open state after hydrate / workspace list updates.
    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.workspaces().get();
        let stored = wb.active_sidebar_explorer_open();
        if explorer_open.get_untracked() != stored {
            explorer_open.set(stored);
        }
    });

    Effect::new(move |_| {
        let _ = active_workspace.get();
        open_paths.set(
            wb.active_sidebar_explorer_expanded_paths()
                .into_iter()
                .collect(),
        );
        children_cache.set(HashMap::new());
        load_gen.update(|g| *g = g.wrapping_add(1));
    });

    Effect::new(move |_| {
        let open = explorer_open.get();
        if open != wb.active_sidebar_explorer_open() {
            wb.set_active_sidebar_explorer_open(open);
        }
    });

    Effect::new(move |_| {
        let paths: Vec<String> = open_paths.get().into_iter().collect();
        wb.set_active_sidebar_explorer_expanded_paths(paths);
    });

    Effect::new(move |_| {
        write_explorer_show_hidden(show_hidden.get());
    });

    let reload = move || {
        children_cache.set(HashMap::new());
        load_gen.update(|g| *g = g.wrapping_add(1));
    };

    // Begin an inline-create draft in `parent_rel`. Expands the target folder
    // (loading its children if needed) so the editable row is visible, then
    // opens the section and records the pending draft.
    let start_draft: StartDraft = Callback::new(move |(parent_rel, is_dir): (String, bool)| {
        if !parent_rel.is_empty() {
            let need_fetch = !children_cache.get_untracked().contains_key(&parent_rel);
            open_paths.update(|s| {
                s.insert(parent_rel.clone());
            });
            if need_fetch {
                if let Some(ws) = wb.with_active_workspace_entry() {
                    let root = ws.cwd.clone();
                    let key = parent_rel.clone();
                    spawn_local(async move {
                        if let Ok(list) = list_path_entries(root, key.clone()).await {
                            children_cache.update(|c| {
                                c.insert(key, list);
                            });
                        }
                    });
                }
            }
        }
        explorer_open.set(true);
        draft.set(Some(Draft { parent_rel, is_dir }));
    });

    let show_section = move || {
        !collapsed.get()
            && active_workspace
                .get()
                .is_some_and(|(_, cwd, configuring)| !configuring && !cwd.trim().is_empty())
    };

    view! {
        <Show when=show_section>
            <SidebarViewSection
                title=section_title
                section_id="sb-explorer"
                open=explorer_open
                toolbar=view! {
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbExplorerNewFile
                        on_click=Callback::new(move |_| {
                            start_draft.run((selected_dir.get_untracked().unwrap_or_default(), false));
                        })
                    >
                        <LxIcon icon=icondata::LuFilePlus width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbExplorerNewFolder
                        on_click=Callback::new(move |_| {
                            start_draft.run((selected_dir.get_untracked().unwrap_or_default(), true));
                        })
                    >
                        <LxIcon icon=icondata::LuFolderPlus width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbExplorerRefresh
                        on_click=Callback::new(move |_| reload())
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                    <button
                        type="button"
                        class="sidebar-view-section__icon-btn"
                        class:sidebar-view-section__icon-btn--pressed=move || show_hidden.get()
                        aria-pressed=move || show_hidden.get().to_string()
                        aria-label=move || {
                            if show_hidden.get() {
                                i18n.tr(I18nKey::SbExplorerHideHidden)()
                            } else {
                                i18n.tr(I18nKey::SbExplorerShowHidden)()
                            }
                        }
                        title=move || {
                            if show_hidden.get() {
                                i18n.tr(I18nKey::SbExplorerHideHidden)()
                            } else {
                                i18n.tr(I18nKey::SbExplorerShowHidden)()
                            }
                        }
                        on:click=move |_| show_hidden.update(|v| *v = !*v)
                    >
                        <Show
                            when=move || show_hidden.get()
                            fallback=move || view! {
                                <LxIcon icon=icondata::LuEye width="0.75rem" height="0.75rem" />
                            }
                        >
                            <LxIcon icon=icondata::LuEyeOff width="0.75rem" height="0.75rem" />
                        </Show>
                    </button>
                }.into_any()
            >
                <ProjectExplorerBody
                    active_workspace=active_workspace
                    open_paths=open_paths
                    children_cache=children_cache
                    load_gen=load_gen
                    error_msg=error_msg
                    show_hidden=show_hidden
                    draft=draft
                    selected_dir=selected_dir
                    start_draft=start_draft
                />
            </SidebarViewSection>
        </Show>
    }
}

#[component]
fn ProjectExplorerBody(
    active_workspace: Memo<Option<(u64, String, bool)>>,
    open_paths: RwSignal<HashSet<String>>,
    children_cache: RwSignal<HashMap<String, Vec<FsEntryBrief>>>,
    load_gen: RwSignal<u32>,
    error_msg: RwSignal<Option<String>>,
    show_hidden: RwSignal<bool>,
    draft: RwSignal<Option<Draft>>,
    selected_dir: RwSignal<Option<String>>,
    start_draft: StartDraft,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    Effect::new(move |_| {
        let _gen = load_gen.get();
        let Some((_, cwd, configuring)) = active_workspace.get() else {
            return;
        };
        if configuring || cwd.trim().is_empty() {
            return;
        };
        let root = cwd;
        if !is_tauri_shell() {
            error_msg.set(Some(i18n.tr(I18nKey::SbExplorerTauriOnly)().to_string()));
            return;
        }
        error_msg.set(None);
        let root_key = String::new();
        spawn_local(async move {
            match list_path_entries(root.clone(), root.clone()).await {
                Ok(entries) => {
                    children_cache.update(|c| {
                        c.insert(root_key, entries);
                    });
                }
                Err(e) => error_msg.set(Some(e)),
            }
        });
    });

    view! {
        <div class="project-explorer">
            <Show
                when=move || is_tauri_shell()
                fallback=move || {
                    view! {
                        <p class="sidebar-view-section__empty">
                            {move || error_msg.get().unwrap_or_else(|| i18n.tr(I18nKey::SbExplorerTauriOnly)().to_string())}
                        </p>
                    }
                }
            >
                <Show
                    when=move || error_msg.get().is_none()
                    fallback=move || {
                        view! {
                            <p class="sidebar-view-section__empty">{move || error_msg.get().unwrap_or_default()}</p>
                        }
                    }
                >
                    <ul class="project-explorer__tree" role="tree">
                        <ExplorerChildren
                            rel_path=String::new()
                            depth=0
                            open_paths=open_paths
                            children_cache=children_cache
                            load_gen=load_gen
                            error_msg=error_msg
                            show_hidden=show_hidden
                            draft=draft
                            selected_dir=selected_dir
                            start_draft=start_draft
                        />
                    </ul>
                </Show>
            </Show>
        </div>
    }
}

/// Inline editable row for creating a new file or folder inside a folder —
/// VS Code style. Enter commits in `draft.parent_rel`, Escape/blur cancels.
/// Errors (e.g. a duplicate name) keep the row open with an inline message.
#[component]
fn ExplorerDraftRow(
    draft: RwSignal<Option<Draft>>,
    depth: u8,
    children_cache: RwSignal<HashMap<String, Vec<FsEntryBrief>>>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();
    let error = RwSignal::new(None::<String>);

    let is_dir = move || draft.get().map(|d| d.is_dir).unwrap_or(false);
    let pad = format!("padding-left: {}rem", 0.65 + f64::from(depth) * 0.85);

    // Focus the input as soon as a draft begins.
    Effect::new(move |_| {
        let _ = draft.get();
        if let Some(el) = input_ref.get() {
            let _ = el.focus();
        }
    });

    let commit = move || {
        let Some(el) = input_ref.get_untracked() else {
            return;
        };
        let name = el.value().trim().to_string();
        let Some(d) = draft.get_untracked() else {
            return;
        };
        if name.is_empty() {
            draft.set(None);
            return;
        }
        let Some(ws) = wb.with_active_workspace_entry() else {
            draft.set(None);
            return;
        };
        let root = ws.cwd.clone();
        let ws_id = ws.id;
        let dir = d.is_dir;
        let parent_rel = d.parent_rel.clone();
        let rel = if parent_rel.is_empty() {
            name.clone()
        } else {
            format!("{parent_rel}/{name}")
        };
        error.set(None);
        spawn_local(async move {
            let res = if dir {
                create_workspace_dir(root.clone(), rel.clone()).await
            } else {
                create_workspace_file(root.clone(), rel.clone()).await
            };
            match res {
                Ok(()) => {
                    draft.set(None);
                    // Targeted refresh: re-list just the parent folder so the
                    // new entry shows without collapsing the rest of the tree.
                    if let Ok(list) = list_path_entries(root, parent_rel.clone()).await {
                        children_cache.update(|c| {
                            c.insert(parent_rel, list);
                        });
                    }
                    if !dir {
                        wb.open_center_file_tab(ws_id, rel);
                    }
                }
                Err(_) => {
                    error.set(Some(i18n.tr(I18nKey::SbExplorerCreateError)().to_string()));
                }
            }
        });
    };

    let cancel = move || draft.set(None);

    view! {
        <li class="project-explorer__node" role="none">
            <div class="project-explorer__row project-explorer__draft-row" style=pad>
                <span class="project-explorer__chev project-explorer__chev--spacer" aria-hidden="true"></span>
                <span class="project-explorer__icon" aria-hidden="true">
                    <Show
                        when=is_dir
                        fallback=move || view! {
                            <LxIcon icon=icondata::LuFile width="0.8rem" height="0.8rem" />
                        }
                    >
                        <LxIcon icon=icondata::LuFolder width="0.8rem" height="0.8rem" />
                    </Show>
                </span>
                <input
                    node_ref=input_ref
                    class="project-explorer__draft-input"
                    type="text"
                    spellcheck="false"
                    autocomplete="off"
                    placeholder=move || {
                        if is_dir() {
                            i18n.tr(I18nKey::SbExplorerNewFolderPlaceholder)()
                        } else {
                            i18n.tr(I18nKey::SbExplorerNewFilePlaceholder)()
                        }
                    }
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        match ev.key().as_str() {
                            "Enter" => {
                                ev.prevent_default();
                                commit();
                            }
                            "Escape" => {
                                ev.prevent_default();
                                cancel();
                            }
                            _ => {}
                        }
                    }
                    on:blur=move |_| cancel()
                />
            </div>
            <Show when=move || error.get().is_some()>
                <p class="project-explorer__draft-error">
                    {move || error.get().unwrap_or_default()}
                </p>
            </Show>
        </li>
    }
}

#[component]
fn ExplorerChildren(
    rel_path: String,
    depth: u8,
    open_paths: RwSignal<HashSet<String>>,
    children_cache: RwSignal<HashMap<String, Vec<FsEntryBrief>>>,
    load_gen: RwSignal<u32>,
    error_msg: RwSignal<Option<String>>,
    show_hidden: RwSignal<bool>,
    draft: RwSignal<Option<Draft>>,
    selected_dir: RwSignal<Option<String>>,
    start_draft: StartDraft,
) -> impl IntoView {
    let parent_rel = rel_path.clone();
    let entries = Memo::new({
        let parent_rel = parent_rel.clone();
        move |_| {
            let show_h = show_hidden.get();
            children_cache
                .get()
                .get(&parent_rel)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|e| show_h || !e.hidden)
                .collect::<Vec<_>>()
        }
    });

    let key_base = parent_rel.clone();
    let path_base = parent_rel.clone();
    let draft_group = parent_rel;

    view! {
        {move || {
            draft
                .get()
                .filter(|d| d.parent_rel == draft_group)
                .map(|_| view! {
                    <ExplorerDraftRow draft=draft depth=depth children_cache=children_cache />
                })
        }}
        <For
            each=move || entries.get()
            key={
                let key_base = key_base.clone();
                move |e| format!("{key_base}/{}", e.name)
            }
            children={
                let path_base = path_base.clone();
                move |entry: FsEntryBrief| {
                let name = entry.name.clone();
                let child_rel = if path_base.is_empty() {
                    name.clone()
                } else {
                    format!("{}/{}", path_base, name)
                };
                let is_open = Memo::new({
                    let child_rel = child_rel.clone();
                    move |_| open_paths.get().contains(&child_rel)
                });

                view! {
                    <ExplorerNode
                        entry=entry
                        rel_path=child_rel
                        depth=depth
                        is_open=is_open
                        open_paths=open_paths
                        children_cache=children_cache
                        load_gen=load_gen
                        error_msg=error_msg
                        show_hidden=show_hidden
                        draft=draft
                        selected_dir=selected_dir
                        start_draft=start_draft
                    />
                }
            }
            }
        />
    }
}

#[component]
fn ExplorerNode(
    entry: FsEntryBrief,
    rel_path: String,
    depth: u8,
    is_open: Memo<bool>,
    open_paths: RwSignal<HashSet<String>>,
    children_cache: RwSignal<HashMap<String, Vec<FsEntryBrief>>>,
    load_gen: RwSignal<u32>,
    error_msg: RwSignal<Option<String>>,
    show_hidden: RwSignal<bool>,
    draft: RwSignal<Option<Draft>>,
    selected_dir: RwSignal<Option<String>>,
    start_draft: StartDraft,
) -> AnyView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let name = entry.name.clone();
    let is_dir = entry.is_dir;
    let hidden = entry.hidden;

    let rel_for_toggle = rel_path.clone();
    let toggle = Callback::new(move |_| {
        if !is_dir {
            return;
        }
        let rel_path = rel_for_toggle.clone();
        open_paths.update(|set| {
            if set.contains(&rel_path) {
                set.remove(&rel_path);
            } else {
                set.insert(rel_path.clone());
                let Some(ws) = wb.with_active_workspace_entry() else {
                    return;
                };
                if children_cache.get_untracked().contains_key(&rel_path) {
                    return;
                }
                let root = ws.cwd.clone();
                let path = rel_path.clone();
                let cache_key = rel_path.clone();
                spawn_local(async move {
                    match list_path_entries(root, path).await {
                        Ok(list) => {
                            children_cache.update(|c| {
                                c.insert(cache_key, list);
                            });
                        }
                        Err(e) => error_msg.set(Some(e)),
                    }
                });
            }
        });
    });

    let pad = format!("padding-left: {}rem", 0.65 + f64::from(depth) * 0.85);

    if !is_dir {
        return view! {
            <li class="project-explorer__node" role="none">
                <div
                    class=move || {
                        let mut c = String::from("project-explorer__row project-explorer__row--file");
                        if hidden { c.push_str(" project-explorer__row--hidden"); }
                        c
                    }
                    style=pad.clone()
                    role="treeitem"
                    on:click={
                        let rel_path = rel_path.clone();
                        move |ev: web_sys::MouseEvent| {
                            ev.stop_propagation();
                            // New file/folder default to this file's folder.
                            let parent = rel_path
                                .rsplit_once('/')
                                .map(|(p, _)| p.to_string())
                                .unwrap_or_default();
                            selected_dir.set((!parent.is_empty()).then_some(parent));
                            if let Some(ws) = wb.with_active_workspace_entry() {
                                wb.open_center_file_tab(ws.id, rel_path.clone());
                            }
                        }
                    }
                >
                    <span class="project-explorer__chev project-explorer__chev--spacer" aria-hidden="true"></span>
                    <span class="project-explorer__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuFile width="0.8rem" height="0.8rem" />
                    </span>
                    <span class="project-explorer__name">{name}</span>
                </div>
            </li>
        }
        .into_any();
    }

    let rel_for_sel = rel_path.clone();
    let rel_for_click = rel_path.clone();
    let rel_for_newfile = rel_path.clone();
    let rel_for_newfolder = rel_path.clone();

    view! {
        <li class="project-explorer__node" role="none">
            <div
                class=move || {
                    let mut c = String::from("project-explorer__row project-explorer__row--dir");
                    if hidden { c.push_str(" project-explorer__row--hidden"); }
                    if selected_dir.get().as_deref() == Some(rel_for_sel.as_str()) {
                        c.push_str(" project-explorer__row--selected");
                    }
                    c
                }
                style=pad
                role="treeitem"
                aria-expanded=move || (is_dir && is_open.get()).to_string()
                on:click=move |ev| {
                    ev.stop_propagation();
                    selected_dir.set(Some(rel_for_click.clone()));
                    toggle.run(());
                }
            >
                <button
                    type="button"
                    class="project-explorer__chev-btn"
                    aria-label=move || if is_open.get() { "Collapse" } else { "Expand" }
                    on:click=move |ev| {
                        ev.stop_propagation();
                        toggle.run(());
                    }
                >
                    <span class="project-explorer__chev" aria-hidden="true">
                        {move || if is_open.get() { "▾" } else { "▸" }}
                    </span>
                </button>
                <span class="project-explorer__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuFolder width="0.8rem" height="0.8rem" />
                </span>
                <span class="project-explorer__name">{name}</span>
                <span class="project-explorer__actions">
                    <button
                        type="button"
                        class="project-explorer__action"
                        title=move || i18n.tr(I18nKey::SbExplorerNewFile)()
                        aria-label=move || i18n.tr(I18nKey::SbExplorerNewFile)()
                        on:click=move |ev: web_sys::MouseEvent| {
                            ev.stop_propagation();
                            start_draft.run((rel_for_newfile.clone(), false));
                        }
                    >
                        <LxIcon icon=icondata::LuFilePlus width="0.7rem" height="0.7rem" />
                    </button>
                    <button
                        type="button"
                        class="project-explorer__action"
                        title=move || i18n.tr(I18nKey::SbExplorerNewFolder)()
                        aria-label=move || i18n.tr(I18nKey::SbExplorerNewFolder)()
                        on:click=move |ev: web_sys::MouseEvent| {
                            ev.stop_propagation();
                            start_draft.run((rel_for_newfolder.clone(), true));
                        }
                    >
                        <LxIcon icon=icondata::LuFolderPlus width="0.7rem" height="0.7rem" />
                    </button>
                </span>
            </div>
            {move || {
                if is_dir && is_open.get() {
                    Some(view! {
                        <ul class="project-explorer__tree" role="group">
                            <ExplorerChildren
                                rel_path=rel_path.clone()
                                depth=depth.saturating_add(1)
                                open_paths=open_paths
                                children_cache=children_cache
                                load_gen=load_gen
                                error_msg=error_msg
                                show_hidden=show_hidden
                                draft=draft
                                selected_dir=selected_dir
                                start_draft=start_draft
                            />
                        </ul>
                    })
                } else {
                    None
                }
            }}
        </li>
    }
    .into_any()
}

fn read_explorer_show_hidden() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(SIDEBAR_EXPLORER_SHOW_HIDDEN_KEY).ok().flatten())
        .is_some_and(|v| v == "1")
}

fn write_explorer_show_hidden(show: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(
            SIDEBAR_EXPLORER_SHOW_HIDDEN_KEY,
            if show { "1" } else { "0" },
        );
    }
}

use std::collections::HashMap;
