use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, popout_close_current, window_current_is_maximized, window_current_minimize,
    window_current_toggle_maximize, workbench_load_state, PopoutPayload,
};
use crate::workbench::app_prefs::AppPrefsService;
use crate::workbench::diagram_gallery::{DiagramGallery, GalleryScope};
use crate::workbench::file_diff::FileDiffDock;
use crate::workbench::file_preview::FilePreviewDock;
use crate::workbench::memory_panel::MemoryPanel;
use crate::workbench::terminal_cell::WorkspaceTerminalCell;
use crate::workbench::toast::ToastService;
use crate::workbench::state::{WorkbenchService, WorkbenchSnapshot};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::callback::Callback;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn PopoutShell(payload: PopoutPayload) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let prefs = AppPrefsService::new();
    let toast = ToastService::new(prefs);
    provide_context(prefs);
    provide_context(toast);

    let title = payload.fallback_title();
    let title_store = StoredValue::new(title.clone());
    let payload_store = StoredValue::new(payload);
    let hydrated = RwSignal::new(!is_tauri_shell());

    Effect::new(move |_| {
        if !is_tauri_shell() || hydrated.get_untracked() {
            return;
        }
        spawn_local(async move {
            match workbench_load_state().await {
                Ok(Some(json)) => match serde_json::from_str::<WorkbenchSnapshot>(&json) {
                    Ok(mut snap) => {
                        let _ = snap.backfill_storage_keys();
                        hydrated.set(wb.hydrate(snap));
                    }
                    Err(err) => {
                        leptos::logging::warn!("popout workbench state parse: {err}");
                        hydrated.set(true);
                    }
                },
                Ok(None) => hydrated.set(true),
                Err(err) => {
                    leptos::logging::warn!("popout workbench state load: {err}");
                    hydrated.set(true);
                }
            }
        });
    });

    view! {
        <div class="workbench-popout-shell">
            <PopoutTitleBar title=title.clone() />
            <main class="workbench-popout-shell__body">
                <Show
                    when=move || hydrated.get()
                    fallback=move || view! {
                        <PopoutPlaceholder title=title.clone() />
                    }
                >
                    {move || match payload_store.get_value() {
                        PopoutPayload::Terminal {
                            workspace_id,
                            slot_id,
                            pane_id,
                            terminal_key,
                        } => view! {
                            <TerminalPopoutView
                                workspace_id=workspace_id
                                slot_id=slot_id
                                pane_id=pane_id
                                terminal_key=terminal_key
                            />
                        }.into_any(),
                        PopoutPayload::Memory { workspace_id, .. } => view! {
                            <WorkspaceScopedPopout workspace_id=workspace_id>
                                <MemoryPanel centered=true />
                            </WorkspaceScopedPopout>
                        }.into_any(),
                        PopoutPayload::MemoryGraph { workspace_id } => view! {
                            <WorkspaceScopedPopout workspace_id=workspace_id>
                                <MemoryPanel centered=true />
                            </WorkspaceScopedPopout>
                        }.into_any(),
                        PopoutPayload::MermaidFile { workspace_id, rel_path } => view! {
                            <WorkspaceScopedPopout workspace_id=workspace_id>
                                <FilePreviewDock workspace_id=workspace_id rel_path=rel_path />
                            </WorkspaceScopedPopout>
                        }.into_any(),
                        PopoutPayload::DiagramGallery { workspace_id, scope } => {
                            match serde_json::from_value::<GalleryScope>(scope) {
                                Ok(scope) => view! {
                                    <WorkspaceScopedPopout workspace_id=workspace_id>
                                        <DiagramGallery workspace_id=workspace_id scope=scope />
                                    </WorkspaceScopedPopout>
                                }.into_any(),
                                Err(_) => view! { <PopoutPlaceholder title=title_store.get_value() /> }.into_any(),
                            }
                        }
                        PopoutPayload::FileDiff { workspace_id, rel_path, staged } => view! {
                            <WorkspaceScopedPopout workspace_id=workspace_id>
                                <FileDiffDock workspace_id=workspace_id rel_path=rel_path staged=staged />
                            </WorkspaceScopedPopout>
                        }.into_any(),
                    }}
                </Show>
            </main>
        </div>
    }
}

#[component]
fn PopoutPlaceholder(title: String) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="workbench-popout-shell__placeholder">
            <span class="workbench-popout-shell__eyebrow">{i18n.tr(I18nKey::PopoutPlaceholderEyebrow)}</span>
            <h1>{title}</h1>
        </div>
    }
}

#[component]
fn WorkspaceScopedPopout(workspace_id: u64, children: Children) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    Effect::new(move |_| {
        if wb.active_id().get_untracked() != Some(workspace_id) {
            wb.select_workspace(workspace_id);
        }
    });

    view! {
        <div class="workbench-popout-view">
            {children()}
        </div>
    }
}

#[component]
fn TerminalPopoutView(
    workspace_id: u64,
    slot_id: u64,
    pane_id: u64,
    terminal_key: String,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let terminal = Memo::new(move |_| {
        wb.workspaces().with(|workspaces| {
            let ws = workspaces.iter().find(|ws| ws.id == workspace_id)?;
            let index = ws.slot_ids.iter().position(|id| *id == slot_id)?;
            Some((
                ws.cwd.clone(),
                ws.slot_agent_labels
                    .get(index)
                    .cloned()
                    .unwrap_or_default(),
                format!("Terminal {slot_id}"),
                index,
            ))
        })
    });

    let active = Signal::derive(move || true);
    let hidden = Signal::derive(move || false);
    let full_size = Signal::derive(move || false);
    let can_close = Signal::derive(move || false);
    let drag_enabled = Signal::derive(move || false);
    let noop = Callback::new(|()| {});

    view! {
        <div class="workbench-popout-terminal">
            {move || {
                let Some((cwd, agent_slug, title, index)) = terminal.get() else {
                    return view! {
                        <PopoutPlaceholder title=terminal_key.clone() />
                    }.into_any();
                };
                view! {
                    <WorkspaceTerminalCell
                        workspace_id=workspace_id
                        slot_id=slot_id
                        pane_id=pane_id
                        cwd=cwd
                        grid_index=index
                        agent_slug=agent_slug
                        title=title
                        terminal_key=terminal_key.clone()
                        is_workspace_active=active
                        is_slot_hidden=hidden
                        is_full_size=full_size
                        on_full_size=noop
                        on_split_vertical=noop
                        on_split_horizontal=noop
                        on_close=noop
                        can_close=can_close
                        slot_drag_enabled=drag_enabled
                        popout_surface=true
                    />
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn PopoutTitleBar(title: String) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let maximized = RwSignal::new(false);

    let refresh_maximized = move || {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(state) = window_current_is_maximized().await {
                maximized.set(state);
            }
        });
    };

    Effect::new(move |_| refresh_maximized());
    let resize_handle = window_event_listener_untyped("resize", move |_| refresh_maximized());
    on_cleanup(move || resize_handle.remove());

    let on_minimize = move |_| {
        if is_tauri_shell() {
            spawn_local(async move {
                let _ = window_current_minimize().await;
            });
        }
    };
    let on_toggle_maximize = move |_| {
        if is_tauri_shell() {
            spawn_local(async move {
                if let Ok(state) = window_current_toggle_maximize().await {
                    maximized.set(state);
                }
            });
        }
    };
    let on_close = move |_| {
        if is_tauri_shell() {
            spawn_local(async move {
                let _ = popout_close_current().await;
            });
        }
    };

    view! {
        <header class="workbench-popout-titlebar" data-tauri-drag-region="">
            <div class="workbench-popout-titlebar__brand" data-tauri-drag-region="">
                <img
                    class="workbench-popout-titlebar__logo"
                    src="/blxcode.png"
                    alt=""
                    draggable="false"
                />
                <span class="workbench-popout-titlebar__app">"BLXCode"</span>
                <span class="workbench-popout-titlebar__sep" aria-hidden="true"></span>
                <span class="workbench-popout-titlebar__title" title=title.clone()>{title.clone()}</span>
            </div>
            <div class="workbench-popout-titlebar__controls" role="group">
                <button
                    type="button"
                    class="workbench-popout-titlebar__btn"
                    aria-label=move || i18n.tr(I18nKey::TbWinMinimize)()
                    title=move || i18n.tr(I18nKey::TbWinMinimize)()
                    on:click=on_minimize
                >
                    <span class="workbench-popout-titlebar__min" aria-hidden="true"></span>
                </button>
                <button
                    type="button"
                    class="workbench-popout-titlebar__btn"
                    aria-label=move || {
                        if maximized.get() {
                            i18n.tr(I18nKey::TbWinRestore)()
                        } else {
                            i18n.tr(I18nKey::TbWinMaximize)()
                        }
                    }
                    title=move || {
                        if maximized.get() {
                            i18n.tr(I18nKey::TbWinRestore)()
                        } else {
                            i18n.tr(I18nKey::TbWinMaximize)()
                        }
                    }
                    on:click=on_toggle_maximize
                >
                    {move || {
                        if maximized.get() {
                            view! { <LxIcon icon=icondata::LuCopy width="0.72rem" height="0.72rem" /> }.into_any()
                        } else {
                            view! { <LxIcon icon=icondata::LuSquare width="0.72rem" height="0.72rem" /> }.into_any()
                        }
                    }}
                </button>
                <button
                    type="button"
                    class="workbench-popout-titlebar__btn workbench-popout-titlebar__btn--close"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    title=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=on_close
                >
                    <LxIcon icon=icondata::LuX width="0.84rem" height="0.84rem" />
                </button>
            </div>
        </header>
    }
}
