//! Sidebar project file tree (workspace `cwd`).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, list_path_entries, FsEntryBrief};
use crate::workbench::sidebar_view_section::{SidebarSectionIconBtn, SidebarViewSection};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use std::collections::HashSet;
use std::path::Path;

#[component]
pub fn ProjectExplorerSection() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let collapsed = wb.sidebar_collapsed();

    let section_title = Memo::new(move |_| {
        let fallback = i18n.tr(I18nKey::SbExplorerTitle)();
        wb.with_active_workspace_entry()
            .map(|w| {
                let t = w.title.trim();
                if t.is_empty() {
                    Path::new(&w.cwd)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&fallback)
                        .to_string()
                } else {
                    t.to_string()
                }
            })
            .unwrap_or_else(|| fallback.to_string())
    });

    let explorer_open = RwSignal::new(wb.active_sidebar_explorer_open());
    let open_paths = RwSignal::new(HashSet::<String>::new());
    let children_cache = RwSignal::new(HashMap::<String, Vec<FsEntryBrief>>::new());
    let load_gen = RwSignal::new(0u32);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let _ = wb.active_id().get();
        explorer_open.set(wb.active_sidebar_explorer_open());
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
        wb.set_active_sidebar_explorer_open(open);
    });

    Effect::new(move |_| {
        let paths: Vec<String> = open_paths.get().into_iter().collect();
        wb.set_active_sidebar_explorer_expanded_paths(paths);
    });

    let reload = move || {
        children_cache.set(HashMap::new());
        load_gen.update(|g| *g = g.wrapping_add(1));
    };

    let show_section = move || {
        !collapsed.get()
            && wb
                .with_active_workspace_entry()
                .is_some_and(|w| !w.configuring && !w.cwd.trim().is_empty())
    };

    view! {
        <Show when=show_section>
            <SidebarViewSection
                title=Signal::derive(move || section_title.get())
                section_id="sb-explorer"
                open=explorer_open
                toolbar=view! {
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbExplorerRefresh
                        on_click=Callback::new(move |_| reload())
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbExplorerCollapseAll
                        on_click=Callback::new(move |_| {
                            open_paths.set(HashSet::new());
                            children_cache.set(HashMap::new());
                        })
                    >
                        <LxIcon icon=icondata::LuChevronsDownUp width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                }.into_any()
            >
                <ProjectExplorerBody
                    open_paths=open_paths
                    children_cache=children_cache
                    load_gen=load_gen
                    error_msg=error_msg
                />
            </SidebarViewSection>
        </Show>
    }
}

#[component]
fn ProjectExplorerBody(
    open_paths: RwSignal<HashSet<String>>,
    children_cache: RwSignal<HashMap<String, Vec<FsEntryBrief>>>,
    load_gen: RwSignal<u32>,
    error_msg: RwSignal<Option<String>>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    Effect::new(move |_| {
        let _gen = load_gen.get();
        let Some(ws) = wb.with_active_workspace_entry() else {
            return;
        };
        let root = ws.cwd.clone();
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
) -> impl IntoView {
    let parent_rel = rel_path.clone();
    let entries = Memo::new({
        let parent_rel = parent_rel.clone();
        move |_| {
            children_cache
                .get()
                .get(&parent_rel)
                .cloned()
                .unwrap_or_default()
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
                    let mut c = String::from("project-explorer__row");
                    if hidden { c.push_str(" project-explorer__row--hidden"); }
                    if !is_dir { c.push_str(" project-explorer__row--file"); }
                    c
                }
                style=pad
                role="treeitem"
                aria-expanded=move || (is_dir && is_open.get()).to_string()
            >
                <button
                    type="button"
                    class="project-explorer__chev-btn"
                    class:project-explorer__chev-btn--spacer=move || !is_dir
                    disabled=move || !is_dir
                    aria-label=move || if is_dir {
                        if is_open.get() { "Collapse" } else { "Expand" }
                    } else {
                        ""
                    }
                    on:click=move |_| toggle.run(())
                >
                    <span class="project-explorer__chev" aria-hidden="true">
                        {move || {
                            if !is_dir {
                                " "
                            } else if is_open.get() {
                                "▾"
                            } else {
                                "▸"
                            }
                        }}
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

use std::collections::HashMap;
