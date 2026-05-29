//! Sidebar git commit graph — native `git log --graph` gutter + commit rows.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    git_commit_graph, listen_git_status_dirty, GitGraphEntry, GitGraphLayout, TauriEventListener,
    GIT_MISSING_CODE,
};
use crate::workbench::git_sync_controls::{run_sync_op, GitSyncControls, SyncOp};
use crate::workbench::sidebar_view_section::{SidebarSectionIconBtn, SidebarViewSection};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use gloo_timers::callback::Timeout;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use send_wrapper::SendWrapper;
use std::cell::RefCell;
use std::rc::Rc;

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

    let reload = move || load_gen.update(|g| *g = g.wrapping_add(1));

    let last_graph_cwd = StoredValue::new(None::<String>);
    let last_load_gen = StoredValue::new(0u32);

    Effect::new(move |_| {
        let gen = load_gen.get();
        let force_reload = gen != last_load_gen.get_value();
        last_load_gen.set_value(gen);
        let _ = wb.sidebar_repo_epoch().get();
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
            match git_commit_graph(cwd_load, Some(100)).await {
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

    // Auto-refresh on `git_status_dirty` (shared watcher with the
    // FileDiffSection). 400ms debounce keeps a burst of HEAD/index/refs
    // changes from triggering more than one `git log`.
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

    // Keep the shared sync status fresh for the Fetch/Pull buttons.
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
        git_sync.refresh(cwd);
    });

    let can_fetch = move || busy.get().is_none() && sync.get().is_some_and(|s| s.has_remote);
    let can_pull = move || {
        busy.get().is_none()
            && sync
                .get()
                .is_some_and(|s| s.has_remote && !s.detached && s.upstream.is_some())
    };
    let fetch_title = move || format!("{}", i18n.tr(I18nKey::SbDiffFetch)());
    let pull_title = move || {
        let base = i18n.tr(I18nKey::SbDiffPull)();
        match sync.get() {
            Some(s) if s.behind > 0 => format!("{base} \u{2193}{}", s.behind),
            _ => format!("{base}"),
        }
    };

    let run_sync = move |op: SyncOp| {
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let set_upstream = git_sync.needs_upstream();
        run_sync_op(
            git_sync,
            op,
            cwd,
            set_upstream,
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
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbGraphRefresh
                        on_click=Callback::new(move |_| reload())
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
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

    view! {
        <div class="git-graph">
            <Show
                when=move || error_kind.get().is_some()
                fallback=move || {
                    let Some(g) = layout.get() else {
                        return view! { <p class="sidebar-view-section__empty">"…"</p> }.into_any();
                    };
                    if g.entries.is_empty() {
                        return view! {
                            <p class="sidebar-view-section__empty">{move || i18n.tr(I18nKey::SbGraphLoadError)()}</p>
                        }
                        .into_any();
                    }
                    view! { <GitGraphList layout=g /> }.into_any()
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
fn GitGraphList(layout: GitGraphLayout) -> impl IntoView {
    let gutter_ch = layout.gutter_cols.max(2);
    let gutter_style = format!("--git-graph-cols: {gutter_ch}");

    view! {
        <ul class="git-graph__list" role="list" style=gutter_style>
            <For
                each=move || layout.entries.clone()
                key=|e| e.commit.oid.clone()
                children=move |entry: GitGraphEntry| {
                    view! { <GitGraphEntryView entry=entry /> }
                }
            />
        </ul>
    }
}

#[component]
fn GitGraphEntryView(entry: GitGraphEntry) -> impl IntoView {
    let commit = entry.commit.clone();
    let gutter = entry.gutter.clone();

    view! {
        <li class="git-graph__row">
            <pre class="git-graph__gutter" aria-hidden="true">{gutter}</pre>
            <div class="git-graph__text">
                <div class="git-graph__subject-line">
                    <span class="git-graph__subject" title=commit.subject.clone()>
                        {commit.subject.clone()}
                    </span>
                    <For
                        each=move || commit.decorations.clone()
                        key=|d| (d.kind.clone(), d.label.clone())
                        children=move |d| {
                            view! {
                                <span class="git-graph__ref">{d.label.clone()}</span>
                            }
                        }
                    />
                </div>
                <span class="git-graph__meta">
                    {commit.author.clone()}" · "{commit.rel_time.clone()}
                </span>
            </div>
        </li>
    }
}
