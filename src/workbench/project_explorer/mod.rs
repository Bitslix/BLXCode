//! Sidebar project file tree (workspace `cwd`).

use crate::config::SIDEBAR_EXPLORER_SHOW_HIDDEN_KEY;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, list_path_entries, FsEntryBrief};
use crate::workbench::sidebar_view_section::{SidebarSectionIconBtn, SidebarViewSection};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::HashSet;

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
                        />
                    </ul>
                </Show>
            </Show>
        </div>
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
    let path_base = parent_rel;

    view! {
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
) -> AnyView {
    let wb = expect_context::<WorkbenchService>();
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

    view! {
        <li class="project-explorer__node" role="none">
            <div
                class=move || {
                    let mut c = String::from("project-explorer__row project-explorer__row--dir");
                    if hidden { c.push_str(" project-explorer__row--hidden"); }
                    c
                }
                style=pad
                role="treeitem"
                aria-expanded=move || (is_dir && is_open.get()).to_string()
                on:click=move |ev| {
                    ev.stop_propagation();
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
