//! Sidebar git commit graph with VS Code-style lanes, compact rows and lazy
//! commit details.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    git_commit_details, git_commit_graph, listen_git_status_dirty, open_external_url,
    GitCommitDetails, GitCommitFileChange, GitCommitNode, GitGraphEntry, GitGraphLayout,
    TauriEventListener, GIT_MISSING_CODE,
};
use crate::workbench::git_sync_controls::{run_sync_op, GitSyncControls, SyncOp};
use crate::workbench::sidebar_view_section::SidebarViewSection;
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use gloo_timers::callback::Timeout;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;

#[component]
pub fn GitGraphSection(git_repo_available: ReadSignal<Option<bool>>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();
    let git_sync = expect_context::<GitSyncControls>();
    let collapsed = wb.sidebar_collapsed();
    let sync = git_sync.status;
    let busy = git_sync.busy;

    let graph_open = RwSignal::new(wb.active_sidebar_graph_open());
    let layout = RwSignal::new(None::<GitGraphLayout>);
    let error_kind = RwSignal::new(None::<GraphErrorKind>);
    let load_gen = RwSignal::new(0u32);

    let title = Signal::derive(move || i18n.tr(I18nKey::SbGraphTitle)().to_uppercase());

    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.workspaces().get();
        let stored = wb.active_sidebar_graph_open();
        if graph_open.get_untracked() != stored {
            graph_open.set(stored);
        }
    });

    Effect::new(move |_| {
        let open = graph_open.get();
        if open != wb.active_sidebar_graph_open() {
            wb.set_active_sidebar_graph_open(open);
        }
    });

    let last_graph_cwd = StoredValue::new(None::<String>);
    let last_load_gen = StoredValue::new(0u32);
    let last_repo_epoch = StoredValue::new(0u32);

    Effect::new(move |_| {
        let gen = load_gen.get();
        let epoch = wb.sidebar_repo_epoch().get();
        let force_reload = gen != last_load_gen.get_value() || epoch != last_repo_epoch.get_value();
        last_load_gen.set_value(gen);
        last_repo_epoch.set_value(epoch);
        match git_repo_available.get() {
            Some(true) => {}
            Some(false) => {
                layout.set(None);
                error_kind.set(None);
                last_graph_cwd.set_value(None);
                return;
            }
            None => return,
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let conn = wb.active_remote_connection_id();
        let cwd_load = cwd.clone();
        let had_layout = layout.get_untracked().is_some();
        let same_cwd = last_graph_cwd.with_value(|prev| prev.as_deref() == Some(cwd.as_str()));
        if same_cwd && had_layout && !force_reload {
            return;
        }
        last_graph_cwd.set_value(Some(cwd));
        if !had_layout {
            layout.set(None);
            error_kind.set(None);
        }
        spawn_local(async move {
            match git_commit_graph(cwd_load, Some(100), conn).await {
                Ok(g) => {
                    layout.set(Some(g));
                    error_kind.set(None);
                }
                Err(e) if e == GIT_MISSING_CODE => {
                    layout.set(None);
                    error_kind.set(Some(GraphErrorKind::GitMissing));
                }
                Err(_) => {
                    layout.set(None);
                    error_kind.set(Some(GraphErrorKind::LoadFailed));
                }
            }
        });
    });

    let pending_timeout: SendWrapper<Rc<RefCell<Option<Timeout>>>> =
        SendWrapper::new(Rc::new(RefCell::new(None)));
    let pending_for_cleanup = pending_timeout.clone();
    let listener_handle: SendWrapper<Rc<RefCell<Option<TauriEventListener>>>> =
        SendWrapper::new(Rc::new(RefCell::new(None)));
    let listener_for_cleanup = listener_handle.clone();

    Effect::new(move |_| {
        if listener_handle.borrow().is_some() {
            return;
        }
        let pending = pending_timeout.clone();
        let listener = listen_git_status_dirty(move |_payload| {
            if let Some(prev) = pending.borrow_mut().take() {
                prev.cancel();
            }
            let timeout = Timeout::new(400, move || {
                load_gen.update(|g| *g = g.wrapping_add(1));
            });
            *pending.borrow_mut() = Some(timeout);
        });
        *listener_handle.borrow_mut() = listener;
    });

    on_cleanup(move || {
        if let Some(prev) = pending_for_cleanup.borrow_mut().take() {
            prev.cancel();
        }
        listener_for_cleanup.borrow_mut().take();
    });

    Effect::new(move |_| {
        let _ = load_gen.get();
        let _ = wb.sidebar_repo_epoch().get();
        if git_repo_available.get() != Some(true) {
            git_sync.clear();
            return;
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        git_sync.refresh(cwd, wb.active_remote_connection_id());
    });

    let can_fetch = move || busy.get().is_none() && sync.get().is_some_and(|s| s.has_remote);
    let can_pull = move || {
        busy.get().is_none()
            && sync
                .get()
                .is_some_and(|s| s.has_remote && !s.detached && s.upstream.is_some())
    };
    let fetch_title = move || i18n.tr(I18nKey::SbDiffFetch)().to_string();
    let pull_title = move || {
        let base = i18n.tr(I18nKey::SbDiffPull)();
        match sync.get() {
            Some(s) if s.behind > 0 => format!("{base} v{}", s.behind),
            _ => base.to_string(),
        }
    };

    let run_sync = move |op: SyncOp| {
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let set_upstream = git_sync.needs_upstream();
        let conn = wb.active_remote_connection_id();
        run_sync_op(
            git_sync,
            op,
            cwd,
            set_upstream,
            conn,
            toast,
            i18n,
            move || wb.sidebar_repo_epoch().update(|n| *n = n.wrapping_add(1)),
        );
    };

    let show = move || !collapsed.get() && git_repo_available.get() == Some(true);

    view! {
        <Show when=show>
            <SidebarViewSection
                title=title
                section_id="sb-graph"
                open=graph_open
                toolbar=view! {
                    <button
                        type="button"
                        class="sidebar-view-section__icon-btn"
                        disabled=move || !can_fetch()
                        aria-label=fetch_title
                        title=fetch_title
                        on:click=move |_| run_sync(SyncOp::Fetch)
                    >
                        <Show
                            when=move || busy.get() == Some(SyncOp::Fetch)
                            fallback=move || view! {
                                <LxIcon icon=icondata::LuDownload width="0.75rem" height="0.75rem" />
                            }
                        >
                            <span class="sidebar-view-section__sync-spin">
                                <LxIcon icon=icondata::LuLoaderCircle width="0.75rem" height="0.75rem" />
                            </span>
                        </Show>
                    </button>
                    <button
                        type="button"
                        class="sidebar-view-section__icon-btn"
                        disabled=move || !can_pull()
                        aria-label=pull_title
                        title=pull_title
                        on:click=move |_| run_sync(SyncOp::Pull)
                    >
                        <Show
                            when=move || busy.get() == Some(SyncOp::Pull)
                            fallback=move || view! {
                                <LxIcon icon=icondata::LuArrowDownToLine width="0.75rem" height="0.75rem" />
                            }
                        >
                            <span class="sidebar-view-section__sync-spin">
                                <LxIcon icon=icondata::LuLoaderCircle width="0.75rem" height="0.75rem" />
                            </span>
                        </Show>
                    </button>
                }.into_any()
            >
                <GitGraphBody layout=layout error_kind=error_kind />
            </SidebarViewSection>
        </Show>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GraphErrorKind {
    GitMissing,
    LoadFailed,
}

#[component]
fn GitGraphBody(
    layout: RwSignal<Option<GitGraphLayout>>,
    error_kind: RwSignal<Option<GraphErrorKind>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let selected_oid = RwSignal::new(None::<String>);
    let hovered_oid = RwSignal::new(None::<String>);
    let details = RwSignal::new(HashMap::<String, GitCommitDetails>::new());
    let loading = RwSignal::new(Vec::<String>::new());

    view! {
        <div class="git-graph">
            <Show
                when=move || error_kind.get().is_some()
                fallback=move || {
                    let Some(g) = layout.get() else {
                        return view! { <p class="sidebar-view-section__empty">"..."</p> }.into_any();
                    };
                    if g.entries.is_empty() {
                        return view! {
                            <p class="sidebar-view-section__empty">{move || i18n.tr(I18nKey::SbGraphLoadError)()}</p>
                        }
                        .into_any();
                    }
                    view! {
                        <GitGraphList
                            layout=g
                            selected_oid=selected_oid
                            hovered_oid=hovered_oid
                            details=details
                            loading=loading
                        />
                    }.into_any()
                }
            >
                <p class="sidebar-view-section__empty">
                    {move || match error_kind.get() {
                        Some(GraphErrorKind::GitMissing) => i18n.tr(I18nKey::SbGraphGitMissing)(),
                        _ => i18n.tr(I18nKey::SbGraphLoadError)(),
                    }}
                </p>
            </Show>
        </div>
    }
}

#[component]
fn GitGraphList(
    layout: GitGraphLayout,
    selected_oid: RwSignal<Option<String>>,
    hovered_oid: RwSignal<Option<String>>,
    details: RwSignal<HashMap<String, GitCommitDetails>>,
    loading: RwSignal<Vec<String>>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let load_details = Callback::new(move |oid: String| {
        if details.with_untracked(|m| m.contains_key(&oid))
            || loading.with_untracked(|v| v.iter().any(|x| x == &oid))
        {
            return;
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let conn = wb.active_remote_connection_id();
        loading.update(|v| v.push(oid.clone()));
        spawn_local(async move {
            if let Ok(detail) = git_commit_details(cwd, oid.clone(), conn).await {
                details.update(|m| {
                    m.insert(oid.clone(), detail);
                });
            }
            loading.update(|v| v.retain(|x| x != &oid));
        });
    });
    let lane_count = layout.lane_count.max(1);
    let gutter_style = format!("--git-graph-lanes: {lane_count}");

    view! {
        <ul class="git-graph__list" role="list" style=gutter_style>
            <For
                each=move || layout.entries.clone()
                key=|e| e.commit.oid.clone()
                children=move |entry: GitGraphEntry| {
                    view! {
                        <GitGraphRow
                            entry=entry
                            selected_oid=selected_oid
                            hovered_oid=hovered_oid
                            details=details
                            loading=loading
                            load_details=load_details
                        />
                    }
                }
            />
        </ul>
    }
}

#[component]
fn GitGraphRow(
    entry: GitGraphEntry,
    selected_oid: RwSignal<Option<String>>,
    hovered_oid: RwSignal<Option<String>>,
    details: RwSignal<HashMap<String, GitCommitDetails>>,
    loading: RwSignal<Vec<String>>,
    load_details: Callback<String>,
) -> impl IntoView {
    let commit = entry.commit.clone();
    let oid = commit.oid.clone();
    let oid_for_class = oid.clone();
    let oid_for_expanded = oid.clone();
    let oid_for_hovered = oid.clone();
    let oid_for_click = oid.clone();
    let oid_for_mouse = oid.clone();
    let oid_for_focus = oid.clone();
    let oid_for_loading_expanded = oid.clone();
    let oid_for_loading_hover = oid.clone();
    let oid_for_detail_expanded = oid.clone();
    let oid_for_detail_hover = oid.clone();
    let commit_for_expanded = commit.clone();
    let commit_for_hover = commit.clone();
    let commit_lane = entry.lane;
    let text_lane = entry.lanes.saturating_sub(1);
    let hover_card_style = RwSignal::new(default_hover_card_style());
    let expanded_signal =
        Signal::derive(move || selected_oid.get().as_deref() == Some(oid_for_expanded.as_str()));
    let expanded_detail = Signal::derive(move || {
        let oid = oid_for_detail_expanded.clone();
        details.with(|m| m.get(&oid).cloned())
    });
    let expanded_loading =
        Signal::derive(move || loading.with(|v| v.iter().any(|x| x == &oid_for_loading_expanded)));
    let hover_detail = Signal::derive(move || {
        let oid = oid_for_detail_hover.clone();
        details.with(|m| m.get(&oid).cloned())
    });
    let hover_loading =
        Signal::derive(move || loading.with(|v| v.iter().any(|x| x == &oid_for_loading_hover)));
    let row_class = move || {
        let mut class = String::from("git-graph__row");
        if selected_oid.get().as_deref() == Some(oid_for_class.as_str()) {
            class.push_str(" git-graph__row--selected");
        }
        if hovered_oid.get().as_deref() == Some(oid_for_class.as_str()) {
            class.push_str(" git-graph__row--hovered");
        }
        class
    };
    let hovered = move || hovered_oid.get().as_deref() == Some(oid_for_hovered.as_str());

    view! {
        <li
            class=row_class
            style=format!("--commit-lane:{commit_lane};--text-lane:{text_lane};")
            on:mouseenter=move |ev: web_sys::MouseEvent| {
                hover_card_style.set(hover_card_style_for_target(ev.current_target()));
                hovered_oid.set(Some(oid_for_mouse.clone()));
                load_details.run(oid_for_mouse.clone());
            }
            on:mouseleave=move |_| hovered_oid.set(None)
        >
            <div class="git-graph__line">
                <GitGraphLaneGutter entry=entry.clone() selected=expanded_signal />
                <button
                    type="button"
                    class="git-graph__commit-btn"
                    aria-expanded=move || expanded_signal.get().to_string()
                    on:click=move |_| {
                        if selected_oid.get_untracked().as_deref() == Some(oid_for_click.as_str()) {
                            selected_oid.set(None);
                        } else {
                            selected_oid.set(Some(oid_for_click.clone()));
                            load_details.run(oid_for_click.clone());
                        }
                    }
                    on:focus=move |ev: web_sys::FocusEvent| {
                        hover_card_style.set(hover_card_style_for_target(ev.current_target()));
                        hovered_oid.set(Some(oid_for_focus.clone()));
                        load_details.run(oid_for_focus.clone());
                    }
                    on:blur=move |_| hovered_oid.set(None)
                >
                    <span class="git-graph__subject">{commit.subject.clone()}</span>
                </button>
            </div>
            <Show when=move || expanded_signal.get()>
                <GitGraphExpandedFiles
                    detail=expanded_detail
                    loading=expanded_loading
                    fallback_commit=commit_for_expanded.clone()
                />
            </Show>
            <Show when=hovered>
                <GitCommitHoverCard
                    detail=hover_detail
                    loading=hover_loading
                    fallback_commit=commit_for_hover.clone()
                    card_style=Signal::derive(move || hover_card_style.get())
                />
            </Show>
        </li>
    }
}

#[component]
fn GitGraphLaneGutter(entry: GitGraphEntry, selected: Signal<bool>) -> impl IntoView {
    let node_lane = entry.lane;
    view! {
        <div class="git-graph__gutter" aria-hidden="true">
            <For
                each=move || entry.active_lanes.clone()
                key=|lane| *lane
                children=move |lane| {
                    let style = format!("--lane:{lane};--lane-color:var(--git-lane-{});", lane % 8);
                    view! { <span class="git-graph__lane" style=style></span> }
                }
            />
            <For
                each=move || entry.edges.clone()
                key=|e| (e.from_lane, e.to_lane, e.color_index)
                children=move |edge| {
                    let left = edge.from_lane.min(edge.to_lane);
                    let width = edge.from_lane.max(edge.to_lane) - left;
                    let style = format!(
                        "--lane:{left};--edge-width:{width};--lane-color:var(--git-lane-{});",
                        edge.color_index % 8
                    );
                    view! { <span class="git-graph__edge" style=style></span> }
                }
            />
            <span
                class="git-graph__node"
                class:git-graph__node--selected=move || selected.get()
                style=format!("--lane:{node_lane};--lane-color:var(--git-lane-{});", node_lane % 8)
            ></span>
        </div>
    }
}

#[component]
fn GitGraphExpandedFiles(
    detail: Signal<Option<GitCommitDetails>>,
    loading: Signal<bool>,
    fallback_commit: GitCommitNode,
) -> impl IntoView {
    view! {
        <div class="git-graph__expanded">
            <div class="git-graph__expanded-meta">
                <span>{fallback_commit.author.clone()}</span>
                <span>{fallback_commit.rel_time.clone()}</span>
            </div>
            {move || match detail.get() {
                Some(d) if d.files.is_empty() => view! {
                    <p class="git-graph__details-empty">"No files changed"</p>
                }.into_any(),
                Some(d) => view! {
                    <ul class="git-graph__files" role="list">
                        <For
                            each=move || d.files.clone()
                            key=|file| (file.path.clone(), file.status.clone())
                            children=move |file| view! { <GitGraphFileRow file=file /> }
                        />
                    </ul>
                }.into_any(),
                None if loading.get() => view! {
                    <p class="git-graph__details-empty">"Loading files..."</p>
                }.into_any(),
                None => view! {
                    <p class="git-graph__details-empty">"Could not load files"</p>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn GitGraphFileRow(file: GitCommitFileChange) -> impl IntoView {
    let marker = status_marker_for(&file.status);
    let status_class = format!(
        "git-graph__file-status git-graph__file-status--{}",
        file.status
    );
    let (dir, name) = split_path(&file.path);
    let has_dir = !dir.is_empty();
    let dir_text = dir.clone();
    let added = file.added.unwrap_or(0);
    let removed = file.removed.unwrap_or(0);
    view! {
        <li class="git-graph__file">
            <span class=status_class>{marker}</span>
            <span class="git-graph__file-path" title=file.path.clone()>
                <Show when=move || has_dir>
                    <span class="git-graph__file-dir">{dir_text.clone()}"/"</span>
                </Show>
                <span class="git-graph__file-name">{name.clone()}</span>
            </span>
            <span class="git-graph__file-stats">
                <Show when=move || { added > 0 }>
                    <span class="git-graph__stat git-graph__stat--add">{format!("+{added}")}</span>
                </Show>
                <Show when=move || { removed > 0 }>
                    <span class="git-graph__stat git-graph__stat--del">{format!("-{removed}")}</span>
                </Show>
            </span>
        </li>
    }
}

#[component]
fn GitCommitHoverCard(
    detail: Signal<Option<GitCommitDetails>>,
    loading: Signal<bool>,
    fallback_commit: GitCommitNode,
    card_style: Signal<String>,
) -> impl IntoView {
    let fallback_decorations = fallback_commit.decorations.clone();
    let github_url = Signal::derive(move || {
        detail
            .get()
            .and_then(|d| github_commit_url(d.remote_url.as_deref(), &d.oid))
    });
    let show_loading = Signal::derive(move || loading.get() && detail.get().is_none());
    view! {
        <aside class="git-graph__hover-card" role="tooltip" style=move || card_style.get()>
            {move || {
                let d = detail.get();
                let subject = d.as_ref().map(|d| d.subject.clone()).unwrap_or_else(|| fallback_commit.subject.clone());
                let body = d.as_ref().map(|d| d.body.clone()).unwrap_or_else(|| fallback_commit.body.clone());
                let author = d.as_ref().map(|d| d.author.clone()).unwrap_or_else(|| fallback_commit.author.clone());
                let rel = d.as_ref().map(|d| d.rel_time.clone()).unwrap_or_else(|| fallback_commit.rel_time.clone());
                let when = d.as_ref().map(|d| d.author_time.clone()).unwrap_or_else(|| fallback_commit.author_time.clone());
                let short = d.as_ref().map(|d| d.short_oid.clone()).unwrap_or_else(|| fallback_commit.short_oid.clone());
                let files = d.as_ref().map(|d| d.files_changed).unwrap_or(0);
                let add = d.as_ref().map(|d| d.insertions).unwrap_or(0);
                let del = d.as_ref().map(|d| d.deletions).unwrap_or(0);
                let body_view = if body.trim().is_empty() {
                    ().into_any()
                } else {
                    view! { <p class="git-graph__hover-body">{body.clone()}</p> }.into_any()
                };
                let refs = fallback_decorations.clone();
                view! {
                    <div>
                        <header class="git-graph__hover-head">
                            <span class="git-graph__avatar">{initials(&author)}</span>
                            <span class="git-graph__hover-author">{author}</span>
                            <span class="git-graph__hover-time" title=when>{rel}</span>
                        </header>
                        <p class="git-graph__hover-subject">{subject}</p>
                        {body_view}
                        <div class="git-graph__hover-stats">
                            <span>{format!("{files} files changed")}</span>
                            <span class="git-graph__stat--add">{format!("{add} insertions(+)")}</span>
                            <span class="git-graph__stat--del">{format!("{del} deletions(-)")}</span>
                        </div>
                        <div class="git-graph__hover-refs">
                            <For
                                each=move || refs.clone()
                                key=|d| (d.kind.clone(), d.label.clone())
                                children=move |d| view! { <span class="git-graph__ref">{d.label.clone()}</span> }
                            />
                        </div>
                        <footer class="git-graph__hover-foot">
                            <span class="git-graph__sha">{short}</span>
                            <Show when=move || show_loading.get()>
                                <span class="git-graph__hover-loading">"Loading..."</span>
                            </Show>
                            <Show when=move || github_url.get().is_some()>
                                <button
                                    type="button"
                                    class="git-graph__github"
                                    on:click=move |ev| {
                                        ev.stop_propagation();
                                        if let Some(url) = github_url.get() {
                                            spawn_local(async move {
                                                let _ = open_external_url(&url).await;
                                            });
                                        }
                                    }
                                >
                                    <LxIcon icon=icondata::LuGithub width="0.78rem" height="0.78rem" />
                                    <span>"Open on GitHub"</span>
                                </button>
                            </Show>
                        </footer>
                    </div>
                }
            }}
        </aside>
    }
}

fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (dir.to_string(), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}

fn status_marker_for(kind: &str) -> &'static str {
    match kind {
        "added" => "A",
        "deleted" => "D",
        "renamed" => "R",
        "copied" => "C",
        _ => "M",
    }
}

fn initials(name: &str) -> String {
    let mut out = String::new();
    for part in name.split_whitespace().take(2) {
        if let Some(ch) = part.chars().next() {
            out.push(ch.to_ascii_uppercase());
        }
    }
    if out.is_empty() {
        "?".into()
    } else {
        out
    }
}

fn github_commit_url(remote: Option<&str>, oid: &str) -> Option<String> {
    let remote = remote?.trim().trim_end_matches(".git");
    let path = if let Some(rest) = remote.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = remote.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = remote.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = remote.strip_prefix("ssh://git@github.com/") {
        rest
    } else {
        return None;
    };
    if path.split('/').count() < 2 {
        return None;
    }
    Some(format!("https://github.com/{path}/commit/{oid}"))
}

fn default_hover_card_style() -> String {
    "left: 0.75rem; top: 0.75rem;".into()
}

fn hover_card_style_for_target(target: Option<web_sys::EventTarget>) -> String {
    let Some(target) = target else {
        return default_hover_card_style();
    };
    let Ok(element) = target.dyn_into::<web_sys::Element>() else {
        return default_hover_card_style();
    };
    let rect = element.get_bounding_client_rect();
    let (viewport_w, viewport_h) = viewport_size();
    let card_w = 420.0_f64.min((viewport_w - 32.0).max(260.0));
    let card_h = 220.0;
    let left = (rect.right() + 8.0).min((viewport_w - card_w - 12.0).max(12.0));
    let top = (rect.top() - 8.0)
        .min((viewport_h - card_h - 12.0).max(12.0))
        .max(12.0);
    format!("left:{left:.0}px;top:{top:.0}px;")
}

fn viewport_size() -> (f64, f64) {
    let Some(window) = web_sys::window() else {
        return (1280.0, 720.0);
    };
    let width = window
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0);
    let height = window
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(720.0);
    (width, height)
}
