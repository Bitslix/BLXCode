use crate::tauri_bridge::{
    git_worktree_create, git_worktree_list, git_worktree_open_info, git_worktree_remove,
    GitWorktreeEntry,
};
use crate::workbench::state::WorkspaceWorktreeMeta;
use crate::workbench::WorkbenchService;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveWorktreeScope {
    cwd: String,
    connection_id: Option<String>,
}

#[component]
pub fn WorktreeMenu() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let open = RwSignal::new(false);
    let entries = RwSignal::new(Vec::<GitWorktreeEntry>::new());
    let loading = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let branch_input = RwSignal::new(String::new());
    let start_input = RwSignal::new(String::new());
    let path_input = RwSignal::new(String::new());

    let active_scope = Memo::new(move |_| {
        let active = wb.active_id().get()?;
        wb.workspaces().with(|list| {
            let workspace = list.iter().find(|workspace| workspace.id == active)?;
            let cwd = workspace.cwd.trim().to_string();
            if cwd.is_empty() {
                return None;
            }
            Some(ActiveWorktreeScope {
                cwd,
                connection_id: workspace.remote_connection_id.clone(),
            })
        })
    });

    let close_click = window_event_listener_untyped("click", move |ev| {
        if !open.get_untracked() {
            return;
        }
        let inside = ev
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .and_then(|element| {
                element
                    .closest(".app-titlebar__worktree-wrap")
                    .ok()
                    .flatten()
            })
            .is_some();
        if !inside {
            open.set(false);
        }
    });
    let close_esc = window_event_listener_untyped("keydown", move |ev| {
        let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if ev.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        close_click.remove();
        close_esc.remove();
    });

    let refresh = move || {
        let Some(scope) = active_scope.get_untracked() else {
            entries.set(Vec::new());
            error.set(Some("No active workspace".into()));
            return;
        };
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match git_worktree_list(scope.cwd, scope.connection_id).await {
                Ok(list) => {
                    entries.set(list);
                    error.set(None);
                }
                Err(err) => {
                    entries.set(Vec::new());
                    error.set(Some(err));
                }
            }
            loading.set(false);
        });
    };

    let open_entry = move |entry: GitWorktreeEntry| {
        let Some(scope) = active_scope.get_untracked() else {
            return;
        };
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            let resolved =
                match git_worktree_open_info(entry.path.clone(), scope.connection_id.clone()).await
                {
                    Ok(entry) => entry,
                    Err(_) => entry,
                };
            let meta = entry_to_meta(&resolved);
            if wb
                .open_or_create_worktree_workspace(meta, scope.connection_id)
                .is_ok()
            {
                open.set(false);
            }
            loading.set(false);
        });
    };

    let create_worktree = move |_| {
        let Some(scope) = active_scope.get_untracked() else {
            return;
        };
        let branch = branch_input.get_untracked();
        let branch = branch.trim().to_string();
        if branch.is_empty() {
            error.set(Some("Branch is required".into()));
            return;
        }
        let start_point = start_input
            .get_untracked()
            .trim()
            .to_string()
            .into_non_empty();
        let path = {
            let explicit = path_input.get_untracked().trim().to_string();
            if explicit.is_empty() {
                default_worktree_path(&scope.cwd, &branch)
            } else {
                explicit
            }
        };
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match git_worktree_create(
                scope.cwd.clone(),
                branch,
                start_point,
                path,
                scope.connection_id.clone(),
            )
            .await
            {
                Ok(outcome) => {
                    let meta = entry_to_meta(&outcome.entry);
                    let _ = wb.open_or_create_worktree_workspace(meta, scope.connection_id.clone());
                    branch_input.set(String::new());
                    start_input.set(String::new());
                    path_input.set(String::new());
                    match git_worktree_list(scope.cwd, scope.connection_id).await {
                        Ok(list) => entries.set(list),
                        Err(err) => error.set(Some(err)),
                    }
                    open.set(false);
                }
                Err(err) => error.set(Some(err)),
            }
            loading.set(false);
        });
    };

    let remove_entry = move |entry: GitWorktreeEntry| {
        if entry.is_main {
            return;
        }
        let Some(scope) = active_scope.get_untracked() else {
            return;
        };
        let confirmed = web_sys::window()
            .and_then(|window| {
                window
                    .confirm_with_message(&format!("Remove worktree {}?", entry.path))
                    .ok()
            })
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match git_worktree_remove(
                scope.cwd.clone(),
                entry.path.clone(),
                scope.connection_id.clone(),
            )
            .await
            {
                Ok(_) => match git_worktree_list(scope.cwd, scope.connection_id).await {
                    Ok(list) => entries.set(list),
                    Err(err) => error.set(Some(err)),
                },
                Err(err) => error.set(Some(err)),
            }
            loading.set(false);
        });
    };

    let active_branch = Memo::new(move |_| {
        let active = wb.active_id().get()?;
        wb.workspaces().with(|list| {
            list.iter()
                .find(|workspace| workspace.id == active)
                .and_then(|workspace| {
                    workspace
                        .worktree
                        .as_ref()
                        .and_then(|meta| meta.branch.clone())
                })
        })
    });

    view! {
        <div class="app-titlebar__menu-wrap app-titlebar__worktree-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label="Worktrees"
                title=move || {
                    active_branch
                        .get()
                        .map(|branch| format!("Worktrees: {branch}"))
                        .unwrap_or_else(|| "Worktrees".into())
                }
                disabled=move || active_scope.get().is_none()
                on:click=move |ev| {
                    ev.stop_propagation();
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        refresh();
                    }
                }
            >
                <LxIcon icon=icondata::LuGitBranch width="1rem" height="1rem" />
            </button>
            <Show when=move || open.get()>
                <div class="app-titlebar__popover app-titlebar__popover--worktree" role="menu">
                    <div class="app-titlebar__popover-head-row">
                        <p class="app-titlebar__popover-head">"Worktrees"</p>
                        <button
                            type="button"
                            class="app-titlebar__notif-action"
                            aria-label="Refresh worktrees"
                            title="Refresh worktrees"
                            on:click=move |ev| {
                                ev.stop_propagation();
                                refresh();
                            }
                        >
                            <LxIcon icon=icondata::LuRefreshCw width="0.9rem" height="0.9rem" />
                        </button>
                    </div>

                    <Show when=move || loading.get()>
                        <p class="app-titlebar__menu-note">"Loading..."</p>
                    </Show>
                    <Show when=move || error.get().is_some()>
                        <p class="app-titlebar__menu-note app-titlebar__menu-note--error">
                            {move || error.get().unwrap_or_default()}
                        </p>
                    </Show>

                    <div class="app-titlebar__worktree-list">
                        <For
                            each=move || entries.get()
                            key=|entry| entry.path.clone()
                            children=move |entry| {
                                let open_item = entry.clone();
                                let remove_item = entry.clone();
                                view! {
                                    <div class="app-titlebar__worktree-row" role="none">
                                        <button
                                            type="button"
                                            class="app-titlebar__menu-item app-titlebar__worktree-open"
                                            role="menuitem"
                                            title=entry.path.clone()
                                            on:click=move |_| open_entry(open_item.clone())
                                        >
                                            <LxIcon icon=if entry.is_main { icondata::LuHouse } else { icondata::LuGitBranch } width="0.95rem" height="0.95rem" />
                                            <span class="app-titlebar__menu-item-label app-titlebar__worktree-label">
                                                <span>{entry.branch.clone().unwrap_or_else(|| "detached".into())}</span>
                                                <span class="app-titlebar__menu-item-workspace">{short_path(&entry.path)}</span>
                                            </span>
                                        </button>
                                        <button
                                            type="button"
                                            class="app-titlebar__notif-remove"
                                            aria-label="Remove worktree"
                                            title="Remove worktree"
                                            disabled=entry.is_main
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                remove_entry(remove_item.clone());
                                            }
                                        >
                                            <LxIcon icon=icondata::LuTrash2 width="0.85rem" height="0.85rem" />
                                        </button>
                                    </div>
                                }
                            }
                        />
                    </div>

                    <div class="app-titlebar__menu-sep" role="separator"></div>
                    <div class="app-titlebar__worktree-create" role="group" aria-label="Create worktree">
                        <input
                            class="app-titlebar__worktree-input"
                            type="text"
                            placeholder="branch"
                            prop:value=move || branch_input.get()
                            on:input=move |ev| branch_input.set(event_value(&ev))
                        />
                        <input
                            class="app-titlebar__worktree-input"
                            type="text"
                            placeholder="start point"
                            prop:value=move || start_input.get()
                            on:input=move |ev| start_input.set(event_value(&ev))
                        />
                        <input
                            class="app-titlebar__worktree-input"
                            type="text"
                            placeholder="path"
                            prop:value=move || path_input.get()
                            on:input=move |ev| path_input.set(event_value(&ev))
                        />
                        <button
                            type="button"
                            class="app-titlebar__menu-item app-titlebar__worktree-create-btn"
                            disabled=move || loading.get()
                            on:click=create_worktree
                        >
                            <LxIcon icon=icondata::LuPlus width="0.95rem" height="0.95rem" />
                            <span class="app-titlebar__menu-item-label">"Create worktree"</span>
                        </button>
                    </div>
                </div>
            </Show>
        </div>
    }
}

fn entry_to_meta(entry: &GitWorktreeEntry) -> WorkspaceWorktreeMeta {
    WorkspaceWorktreeMeta {
        base_cwd: entry
            .main_worktree_cwd
            .clone()
            .unwrap_or_else(|| entry.path.clone()),
        worktree_cwd: entry.path.clone(),
        branch: entry.branch.clone(),
        head: entry.head.clone(),
        git_common_dir: entry.git_common_dir.clone(),
        main_worktree_cwd: entry.main_worktree_cwd.clone(),
        created_by_blxcode: false,
    }
}

fn short_path(path: &str) -> String {
    path.trim()
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn default_worktree_path(base: &str, branch: &str) -> String {
    let clean_branch = branch
        .trim()
        .replace(['/', '\\', ':', ' '], "-")
        .trim_matches('-')
        .to_string();
    let clean_branch = if clean_branch.is_empty() {
        "worktree".into()
    } else {
        clean_branch
    };
    let base = base.trim().trim_end_matches(['/', '\\']);
    let parent = base
        .rsplit_once(['/', '\\'])
        .map(|(parent, _)| parent)
        .filter(|parent| !parent.is_empty())
        .unwrap_or(base);
    format!("{parent}/{clean_branch}")
}

fn event_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

trait IntoNonEmpty {
    fn into_non_empty(self) -> Option<String>;
}

impl IntoNonEmpty for String {
    fn into_non_empty(self) -> Option<String> {
        if self.is_empty() {
            None
        } else {
            Some(self)
        }
    }
}
