//! Sidebar section listing the changed files in the active workspace's
//! repository (`git status` + `git diff --numstat`). Mirrors the lifecycle
//! of [`crate::workbench::git_graph::GitGraphSection`] but additionally
//! starts a backend filesystem watcher to refresh on every change.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    git_stage_file, git_status_changes, git_status_watch_start, git_status_watch_stop,
    git_unstage_file, listen_git_status_dirty, ChangedFile, LineStats, TauriEventListener,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffErrorKind {
    GitMissing,
    LoadFailed,
}

#[component]
pub fn FileDiffSection(git_repo_available: ReadSignal<Option<bool>>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();
    let git_sync = expect_context::<GitSyncControls>();
    let collapsed = wb.sidebar_collapsed();

    let diff_open = RwSignal::new(wb.active_sidebar_diff_open());
    let entries = RwSignal::new(None::<Vec<ChangedFile>>);
    let error_kind = RwSignal::new(None::<DiffErrorKind>);
    let load_gen = RwSignal::new(0u32);
    // Shared remote-sync state (Push lives here; Fetch/Pull in the graph
    // section). `sync` mirrors `git_sync.status` for terse use below.
    let sync = git_sync.status;
    let busy = git_sync.busy;

    let title = Signal::derive(move || i18n.tr(I18nKey::SbDiffTitle)().to_uppercase());

    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.workspaces().get();
        let stored = wb.active_sidebar_diff_open();
        if diff_open.get_untracked() != stored {
            diff_open.set(stored);
        }
    });

    Effect::new(move |_| {
        let open = diff_open.get();
        if open != wb.active_sidebar_diff_open() {
            wb.set_active_sidebar_diff_open(open);
        }
    });

    let reload = move || load_gen.update(|g| *g = g.wrapping_add(1));

    let last_cwd = StoredValue::new(None::<String>);
    let last_load_gen = StoredValue::new(0u32);

    Effect::new(move |_| {
        let gen = load_gen.get();
        let force_reload = gen != last_load_gen.get_value();
        last_load_gen.set_value(gen);
        let _ = wb.sidebar_repo_epoch().get();
        match git_repo_available.get() {
            Some(true) => {}
            Some(false) => {
                entries.set(None);
                error_kind.set(None);
                last_cwd.set_value(None);
                return;
            }
            None => return,
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let cwd_load = cwd.clone();
        let same_cwd = last_cwd.with_value(|prev| prev.as_deref() == Some(cwd.as_str()));
        let had_data = entries.get_untracked().is_some();
        if same_cwd && had_data && !force_reload {
            return;
        }
        last_cwd.set_value(Some(cwd));
        if !had_data {
            entries.set(None);
            error_kind.set(None);
        }
        spawn_local(async move {
            match git_status_changes(cwd_load).await {
                Ok(list) => {
                    entries.set(Some(list));
                    error_kind.set(None);
                }
                Err(e) if e == GIT_MISSING_CODE => {
                    entries.set(None);
                    error_kind.set(Some(DiffErrorKind::GitMissing));
                }
                Err(_) => {
                    entries.set(None);
                    error_kind.set(Some(DiffErrorKind::LoadFailed));
                }
            }
        });
    });

    // Watcher lifecycle: keep one token per repo cwd. Drop the previous one
    // on cwd change, the event listener on unmount.
    let watch_token = StoredValue::new(None::<u64>);
    let last_watch_cwd = StoredValue::new(None::<String>);

    Effect::new(move |_| {
        let _ = wb.sidebar_repo_epoch().get();
        let cwd = match git_repo_available.get() {
            Some(true) => wb.default_workspace_cwd(),
            _ => None,
        };
        let same = last_watch_cwd.with_value(|prev| prev.as_deref() == cwd.as_deref());
        if same {
            return;
        }
        if let Some(token) = watch_token.get_value() {
            spawn_local(async move {
                let _ = git_status_watch_stop(token).await;
            });
            watch_token.set_value(None);
        }
        last_watch_cwd.set_value(cwd.clone());
        let Some(cwd) = cwd else {
            return;
        };
        spawn_local(async move {
            if let Ok(token) = git_status_watch_start(cwd).await {
                watch_token.set_value(Some(token));
            }
        });
    });

    on_cleanup(move || {
        if let Some(token) = watch_token.get_value() {
            spawn_local(async move {
                let _ = git_status_watch_stop(token).await;
            });
            watch_token.set_value(None);
        }
    });

    // Listener for the dirty event with a 200ms debounce on top of the
    // backend's 300ms aggregator. Two-tier debounce so a single fast `git
    // checkout` that fires staged + unstaged + index events still runs
    // exactly one `git status` reload.
    //
    // `SendWrapper` is needed because Leptos requires `Send + Sync` cleanup
    // closures even though the target is single-threaded WASM.
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
            let timeout = Timeout::new(200, move || {
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

    // Refresh shared branch / upstream / ahead-behind state whenever the repo
    // changes (reload bump, repo epoch, or the dirty watcher via load_gen).
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

    // Push lives in this section; Fetch/Pull are in the Git Commits section.
    // Push is only offered once every change is staged (no unstaged/untracked
    // entries remain) and a remote branch is reachable.
    let all_staged = move || {
        entries
            .get()
            .map(|list| list.iter().all(|e| !e.unstaged))
            .unwrap_or(false)
    };
    let can_push = move || {
        busy.get().is_none()
            && all_staged()
            && sync.get().is_some_and(|s| s.has_remote && !s.detached)
    };
    let push_title = move || {
        let base = i18n.tr(I18nKey::SbDiffPush)();
        match sync.get() {
            Some(s) if s.ahead > 0 => format!("{base} \u{2191}{}", s.ahead),
            _ => format!("{base}"),
        }
    };

    let run_push = move |_| {
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        let set_upstream = git_sync.needs_upstream();
        run_sync_op(
            git_sync,
            SyncOp::Push,
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
                section_id="sb-diff"
                open=diff_open
                toolbar=view! {
                    <button
                        type="button"
                        class="sidebar-view-section__icon-btn"
                        disabled=move || !can_push()
                        aria-label=push_title
                        title=push_title
                        on:click=run_push
                    >
                        <Show
                            when=move || busy.get() == Some(SyncOp::Push)
                            fallback=move || view! {
                                <LxIcon icon=icondata::LuArrowUpFromLine width="0.75rem" height="0.75rem" />
                            }
                        >
                            <span class="sidebar-view-section__sync-spin">
                                <LxIcon icon=icondata::LuLoaderCircle width="0.75rem" height="0.75rem" />
                            </span>
                        </Show>
                    </button>
                    <SidebarSectionIconBtn
                        aria_key=I18nKey::SbDiffRefresh
                        on_click=Callback::new(move |_| reload())
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="0.75rem" height="0.75rem" />
                    </SidebarSectionIconBtn>
                }.into_any()
            >
                <FileDiffBody entries=entries error_kind=error_kind reload=Callback::new(move |_| reload()) />
            </SidebarViewSection>
        </Show>
    }
}

#[component]
fn FileDiffBody(
    entries: RwSignal<Option<Vec<ChangedFile>>>,
    error_kind: RwSignal<Option<DiffErrorKind>>,
    reload: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let staged_open = RwSignal::new(true);
    let unstaged_open = RwSignal::new(true);

    view! {
        <div class="file-diff-section">
            <Show when=move || error_kind.get().is_some()>
                <p class="sidebar-view-section__empty">
                    {move || match error_kind.get() {
                        Some(DiffErrorKind::GitMissing) => i18n.tr(I18nKey::SbDiffGitMissing)(),
                        _ => i18n.tr(I18nKey::SbDiffLoadError)(),
                    }}
                </p>
            </Show>
            <Show when=move || error_kind.get().is_none()>
                <FileDiffList
                    entries=entries
                    reload=reload
                    staged_open=staged_open
                    unstaged_open=unstaged_open
                />
            </Show>
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffGroupVariant {
    Staged,
    Unstaged,
}

fn partition_entries(entries: &[ChangedFile]) -> (Vec<ChangedFile>, Vec<ChangedFile>) {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    for entry in entries {
        if entry.staged {
            staged.push(entry.clone());
        }
        if entry.unstaged {
            unstaged.push(entry.clone());
        }
    }
    (staged, unstaged)
}

#[component]
fn FileDiffList(
    entries: RwSignal<Option<Vec<ChangedFile>>>,
    reload: Callback<()>,
    staged_open: RwSignal<bool>,
    unstaged_open: RwSignal<bool>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        {move || match entries.get() {
            None => {
                view! { <p class="sidebar-view-section__empty">"…"</p> }.into_any()
            }
            Some(list) if list.is_empty() => {
                view! {
                    <p class="sidebar-view-section__empty">
                        {i18n.tr(I18nKey::SbDiffEmpty)()}
                    </p>
                }
                .into_any()
            }
            Some(list) => {
                let (staged, unstaged) = partition_entries(&list);
                view! {
                    <ul
                        class="file-diff-section__list"
                        role="list"
                        aria-label=i18n.tr(I18nKey::SbDiffListAria)()
                    >
                        <FileDiffGroup
                            variant=DiffGroupVariant::Staged
                            entries=staged
                            open=staged_open
                            reload=reload
                        />
                        <FileDiffGroup
                            variant=DiffGroupVariant::Unstaged
                            entries=unstaged
                            open=unstaged_open
                            reload=reload
                        />
                    </ul>
                }
                .into_any()
            }
        }}
    }
}

#[component]
fn FileDiffGroup(
    variant: DiffGroupVariant,
    entries: Vec<ChangedFile>,
    open: RwSignal<bool>,
    reload: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    if entries.is_empty() {
        return ().into_any();
    }

    let count = entries.len();
    let panel_id = match variant {
        DiffGroupVariant::Staged => "file-diff-staged",
        DiffGroupVariant::Unstaged => "file-diff-unstaged",
    };
    let title_base = move || match variant {
        DiffGroupVariant::Staged => i18n.tr(I18nKey::SbDiffGroupStaged)(),
        DiffGroupVariant::Unstaged => i18n.tr(I18nKey::SbDiffGroupUnstaged)(),
    };
    let title = move || format!("{} ({count})", title_base());

    view! {
        <li class="file-diff-section__group">
            <button
                type="button"
                class="file-diff-section__group-toggle"
                id=format!("{panel_id}-header")
                aria-expanded=move || open.get()
                aria-controls=panel_id
                aria-label=move || {
                    let prefix = if open.get() {
                        i18n.tr(I18nKey::SbDiffGroupCollapse)()
                    } else {
                        i18n.tr(I18nKey::SbDiffGroupExpand)()
                    };
                    format!("{prefix} {}", title())
                }
                on:click=move |_| open.update(|v| *v = !*v)
            >
                <span class="file-diff-section__group-title">{title}</span>
                <span class="file-diff-section__group-chev" aria-hidden="true">
                    {move || if open.get() { "▾" } else { "▸" }}
                </span>
            </button>
            <Show when=move || open.get()>
                <FileDiffGroupList
                    entries=entries.clone()
                    variant=variant
                    reload=reload
                    panel_id=panel_id
                />
            </Show>
        </li>
    }
    .into_any()
}

#[component]
fn FileDiffGroupList(
    entries: Vec<ChangedFile>,
    variant: DiffGroupVariant,
    reload: Callback<()>,
    panel_id: &'static str,
) -> impl IntoView {
    view! {
        <ul
            id=panel_id
            role="list"
            aria-labelledby=format!("{panel_id}-header")
            class="file-diff-section__group-list"
        >
            <For
                each=move || entries.clone()
                key=move |e| (e.rel_path.clone(), variant as u8)
                children=move |entry: ChangedFile| {
                    view! {
                        <FileDiffRow entry=entry variant=variant reload=reload />
                    }
                }
            />
        </ul>
    }
}

#[component]
fn FileDiffRow(
    entry: ChangedFile,
    variant: DiffGroupVariant,
    reload: Callback<()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let rel = entry.rel_path.clone();
    let rel_for_open = rel.clone();
    let rel_for_stage = rel.clone();
    let rel_for_unstage = rel.clone();
    let staged = entry.staged;
    let unstaged = entry.unstaged;
    let status_kind = entry.status.clone();
    let stats: Option<LineStats> = match variant {
        DiffGroupVariant::Staged => entry.staged_stats,
        DiffGroupVariant::Unstaged => entry.unstaged_stats,
    };
    let added = stats.as_ref().map(|s| s.added).unwrap_or(0);
    let removed = stats.as_ref().map(|s| s.removed).unwrap_or(0);
    let open_staged = variant == DiffGroupVariant::Staged;
    let status_label_key = match status_kind.as_str() {
        "added" => I18nKey::SbDiffStatusAdded,
        "deleted" => I18nKey::SbDiffStatusDeleted,
        "renamed" => I18nKey::SbDiffStatusRenamed,
        "untracked" => I18nKey::SbDiffStatusUntracked,
        "conflicted" => I18nKey::SbDiffStatusConflicted,
        _ => I18nKey::SbDiffStatusModified,
    };
    let status_marker = status_marker_for(&status_kind);
    let row_class = format!("file-diff-section__row file-diff-section__row--{status_kind}");
    let marker_class =
        format!("file-diff-section__status file-diff-section__status--{status_kind}");
    let on_open = move |_| {
        let workspace_id = wb.active_id().get_untracked();
        let Some(ws_id) = workspace_id else {
            return;
        };
        wb.open_center_diff_tab(ws_id, rel_for_open.clone(), open_staged);
    };
    let on_stage = {
        let rel = rel_for_stage.clone();
        move |ev: web_sys::MouseEvent| {
            ev.stop_propagation();
            let Some(cwd) = wb.default_workspace_cwd() else {
                return;
            };
            let rel = rel.clone();
            let reload = reload;
            spawn_local(async move {
                let _ = git_stage_file(cwd, rel).await;
                reload.run(());
            });
        }
    };
    let on_unstage = {
        let rel = rel_for_unstage.clone();
        move |ev: web_sys::MouseEvent| {
            ev.stop_propagation();
            let Some(cwd) = wb.default_workspace_cwd() else {
                return;
            };
            let rel = rel.clone();
            let reload = reload;
            spawn_local(async move {
                let _ = git_unstage_file(cwd, rel).await;
                reload.run(());
            });
        }
    };
    let stage_aria_label = {
        let prefix = i18n.tr(I18nKey::SbDiffStageAriaPrefix)();
        let path = rel.clone();
        move || format!("{prefix} {path}")
    };
    let unstage_aria_label = {
        let prefix = i18n.tr(I18nKey::SbDiffUnstageAriaPrefix)();
        let path = rel.clone();
        move || format!("{prefix} {path}")
    };
    let status_title_fn = i18n.tr(status_label_key);
    let status_title = StoredValue::new(status_title_fn());

    view! {
        <li class=row_class>
            <button
                type="button"
                class="file-diff-section__row-btn"
                title=rel.clone()
                on:click=on_open
            >
                <span
                    class=marker_class
                    aria-label=move || status_title.get_value()
                    title=move || status_title.get_value()
                >
                    {status_marker}
                </span>
                <span class="file-diff-section__path">{rel.clone()}</span>
                <span class="file-diff-section__counts">
                    <Show when=move || { added > 0 }>
                        <span class="file-diff-section__count file-diff-section__count--add">
                            {format!("+{added}")}
                        </span>
                    </Show>
                    <Show when=move || { removed > 0 }>
                        <span class="file-diff-section__count file-diff-section__count--del">
                            {format!("-{removed}")}
                        </span>
                    </Show>
                </span>
            </button>
            <div class="file-diff-section__actions">
                <Show when=move || variant == DiffGroupVariant::Unstaged && unstaged && status_kind != "conflicted">
                    <button
                        type="button"
                        class="file-diff-section__action file-diff-section__action--stage"
                        title=stage_aria_label.clone()
                        aria-label=stage_aria_label.clone()
                        on:click=on_stage.clone()
                    >
                        "+"
                    </button>
                </Show>
                <Show when=move || variant == DiffGroupVariant::Staged && staged>
                    <button
                        type="button"
                        class="file-diff-section__action file-diff-section__action--unstage"
                        title=unstage_aria_label.clone()
                        aria-label=unstage_aria_label.clone()
                        on:click=on_unstage.clone()
                    >
                        "−"
                    </button>
                </Show>
            </div>
        </li>
    }
}

fn status_marker_for(kind: &str) -> &'static str {
    match kind {
        "modified" => "M",
        "added" => "A",
        "deleted" => "D",
        "renamed" => "R",
        "untracked" => "?",
        "conflicted" => "C",
        _ => "•",
    }
}
