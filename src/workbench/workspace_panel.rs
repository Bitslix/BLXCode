use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, read_workspace_text_file, TextFilePreview};
use crate::workbench::app_prefs::AppPrefsService;
use crate::workbench::browser_tab::sync_embedded_browser_layer;
use crate::workbench::create_workspace_wizard::WorkspaceConfigurator;
use crate::workbench::harness_chords::{
    dispatch_shortcut_action, HarnessShortcutAction, ShortcutKeys,
};
use crate::workbench::harness_ui::SettingsDock;
use crate::workbench::state::{
    workspace_entry_has_folder, BrowserEmbedSurface, CenterTab, CenterTabKind, HarnessUiService,
    RightPanelTab, TerminalSplitAxis, WorkspaceEntry, CENTER_TERMINALS_TAB_ID,
};
use crate::workbench::terminal_cell::WorkspaceTerminalCell;
use crate::workbench::terminal_glue::{
    terminal_observe_workspace_grid, terminal_unobserve_workspace_grid,
};
use crate::workbench::WorkbenchService;
use gloo_timers::future::TimeoutFuture;
use leptos::callback::Callback;
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, MouseEvent};

#[derive(Clone, Copy)]
enum GridResizeAxis {
    Row,
    Col,
}

#[derive(Clone)]
struct GridDragState {
    axis: GridResizeAxis,
    index: usize,
    start_pos: f64,
    start_sizes: Vec<f64>,
    total_px: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TerminalRenderSlot {
    id: u64,
    index: usize,
    agent_slug: String,
}

#[component]
pub fn WorkspacePanel() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let workspaces = wb.workspaces();
    let active_id = wb.active_id();

    view! {
        <section
            class=move || {
                let mut c = String::from("workbench-workspace");
                if active_id.get().is_none() {
                    c.push_str(" workbench-workspace--empty");
                }
                c
            }
            aria-label=move || i18n.tr(I18nKey::WsAria)()
        >
            <div class="workbench-workspace__body">
                <Show
                    when=move || !workspaces.get().is_empty()
                    fallback=move || view! { <WorkspaceEmptyState /> }
                >
                    <For
                        each=move || workspaces.get()
                        key=|workspace| workspace.id
                        children=move |workspace| {
                            view! { <WorkspaceSurface workspace_id=workspace.id /> }
                        }
                    />
                </Show>
            </div>
        </section>
    }
}

#[component]
fn WorkspaceSurface(workspace_id: u64) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let workspace = Memo::new(move |_| {
        wb.workspaces()
            .get()
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
    });
    let initial = workspace
        .get_untracked()
        .unwrap_or_else(|| WorkspaceEntry::empty_surface(workspace_id));
    let row_fr = RwSignal::new(vec![1.0; initial.grid_rows as usize]);
    let col_fr = RwSignal::new(vec![1.0; initial.grid_cols as usize]);
    let full_size_terminal = RwSignal::new(None::<u64>);
    let drag_state = RwSignal::new(None::<GridDragState>);

    Effect::new({
        move |_| {
            let Some(workspace) = workspace.get() else {
                return;
            };
            let rows = workspace.grid_rows as usize;
            let cols = workspace.grid_cols as usize;
            if full_size_terminal
                .get_untracked()
                .is_some_and(|id| !workspace.slot_ids.contains(&id))
            {
                full_size_terminal.set(None);
            }
            if row_fr.with_untracked(|v| v.len() != rows) {
                row_fr.set(vec![1.0; rows]);
            }
            if col_fr.with_untracked(|v| v.len() != cols) {
                col_fr.set(vec![1.0; cols]);
            }
        }
    });

    // Wizard → grid: flex/grid height may stay 0 until a later reflow; nudge
    // all terminal cells to refit xterm/PTY once layout has settled.
    let was_configuring = StoredValue::new(true);
    Effect::new({
        let wb = wb;
        move |_| {
            let configuring = workspace.get().map(|w| w.configuring).unwrap_or(true);
            if was_configuring.get_value() && !configuring {
                if let Some(ws) = workspace.get() {
                    let rows = ws.grid_rows as usize;
                    let cols = ws.grid_cols as usize;
                    row_fr.set(vec![1.0; rows]);
                    col_fr.set(vec![1.0; cols]);
                }
                force_workbench_terminal_layout();
                wb.bump_terminal_layout();
                leptos::task::spawn_local(async move {
                    for delay_ms in [0_u32, 16, 50, 150, 300, 600, 1000, 1500] {
                        TimeoutFuture::new(delay_ms).await;
                        force_workbench_terminal_layout();
                        wb.bump_terminal_layout();
                    }
                });
            }
            was_configuring.set_value(configuring);
        }
    });

    let move_handle = leptos::leptos_dom::helpers::window_event_listener_untyped("mousemove", {
        move |ev| {
            let Some(ev) = ev.dyn_ref::<MouseEvent>() else {
                return;
            };
            let Some(drag) = drag_state.get_untracked() else {
                return;
            };
            ev.prevent_default();
            let current_pos = match drag.axis {
                GridResizeAxis::Row => ev.client_y() as f64,
                GridResizeAxis::Col => ev.client_x() as f64,
            };
            let delta_fr = (current_pos - drag.start_pos) / drag.total_px.max(1.0)
                * drag.start_sizes.iter().sum::<f64>();
            let mut next = drag.start_sizes.clone();
            if drag.index + 1 >= next.len() {
                return;
            }
            let min = 0.25;
            let left = (drag.start_sizes[drag.index] + delta_fr).max(min);
            let right = (drag.start_sizes[drag.index + 1] - delta_fr).max(min);
            let pair_total = drag.start_sizes[drag.index] + drag.start_sizes[drag.index + 1];
            let adjusted_total = left + right;
            next[drag.index] = left / adjusted_total * pair_total;
            next[drag.index + 1] = right / adjusted_total * pair_total;
            match drag.axis {
                GridResizeAxis::Row => row_fr.set(next),
                GridResizeAxis::Col => col_fr.set(next),
            }
        }
    });

    let up_handle = leptos::leptos_dom::helpers::window_event_listener_untyped("mouseup", {
        move |_| {
            drag_state.set(None);
        }
    });

    on_cleanup(move || {
        drop(move_handle);
        drop(up_handle);
    });

    let is_configuring =
        Memo::new(move |_| workspace.get().map(|w| w.configuring).unwrap_or(false));
    let term_grid_ref = NodeRef::<html::Div>::new();

    Effect::new({
        let term_grid_ref = term_grid_ref;
        let wb = wb;
        move |_| {
            if is_configuring.get() {
                terminal_unobserve_workspace_grid(workspace_id);
                return;
            }
            let Some(el) = term_grid_ref.get() else {
                return;
            };
            if let Ok(grid) = el.dyn_into::<HtmlElement>() {
                terminal_observe_workspace_grid(&grid, workspace_id);
                wb.bump_terminal_layout();
            }
        }
    });

    on_cleanup(move || {
        terminal_unobserve_workspace_grid(workspace_id);
    });

    let active_center_tab_id =
        Memo::new(move |_| wb.active_center_tab_id_for_workspace(workspace_id));

    view! {
        <div
            class=move || {
                let mut class = String::from("workspace-surface");
                if wb.active_id().get() != Some(workspace_id) {
                    class.push_str(" workspace-surface--hidden");
                }
                class
            }
        >
            <Show when=move || is_configuring.get()>
                <WorkspaceConfigurator workspace_id=workspace_id />
            </Show>
            <Show when=move || !is_configuring.get()>
                <CenterTabStrip workspace_id=workspace_id active_tab_id=active_center_tab_id />
                <div class="workspace-center-tab-body">
                    <div
                        class="workspace-center-panel"
                        class:workspace-center-panel--hidden=move || active_center_tab_id.get() != CENTER_TERMINALS_TAB_ID
                    >
                        <div
                            class="ws-term-grid"
                            node_ref=term_grid_ref
                            style=move || {
                                let full = full_size_terminal.get().is_some();
                                // Derive the authoritative track count from the
                                // workspace itself; row_fr/col_fr only carry user-driven
                                // resize fractions. If the cached fractions don't match
                                // the current row/col count (e.g. right after wizard
                                // commit, before the sync Effect fires), fall back to
                                // even 1.0 fractions instead of rendering a stale
                                // template. Without this fallback the grid renders the
                                // old N×M layout, CSS auto-flow pushes children into
                                // implicit rows at `auto` height, terminals collapse to
                                // header-only, and xterm.fit() returns 0×0 — which makes
                                // the spawned agent (claude/codex) see a broken TTY.
                                let ws_rows = workspace.get().map(|w| w.grid_rows as usize).unwrap_or(1);
                                let ws_cols = workspace.get().map(|w| w.grid_cols as usize).unwrap_or(1);
                                let rf = row_fr.get();
                                let cf = col_fr.get();
                                let row_frac = if rf.len() == ws_rows { rf } else { vec![1.0; ws_rows] };
                                let col_frac = if cf.len() == ws_cols { cf } else { vec![1.0; ws_cols] };
                                let rows = if full { "minmax(0,1fr)".to_string() } else { fr_template(&row_frac) };
                                let cols = if full { "minmax(0,1fr)".to_string() } else { fr_template(&col_frac) };
                                format!(
                                    "display:grid;grid-template-rows:{rows};grid-template-columns:{cols};gap:4px;"
                                )
                            }
                        >
                            <For
                                each=move || {
                                    workspace
                                        .get()
                                        .map(|workspace| terminal_slots(&workspace))
                                        .unwrap_or_default()
                                }
                                key=|slot| slot.id
                                children=move |slot| {
                                    let terminal_id = slot.id;
                                    let index = slot.index;
                                    let slug = slot.agent_slug;
                                    let cwd = workspace.get_untracked().map(|w| w.cwd).unwrap_or_default();
                                    let on_full_size = Callback::new(move |()| {
                                        full_size_terminal.update(|current| {
                                            *current = if *current == Some(terminal_id) {
                                                None
                                            } else {
                                                Some(terminal_id)
                                            };
                                        });
                                    });

                                    view! {
                                        <TerminalSlotSurface
                                            workspace_id=workspace_id
                                            slot_id=terminal_id
                                            index=index
                                            cwd=cwd
                                            agent_slug=slug
                                            is_workspace_active=Signal::derive(move || {
                                                wb.active_id().get() == Some(workspace_id)
                                                    && active_center_tab_id.get() == CENTER_TERMINALS_TAB_ID
                                            })
                                            hidden=Signal::derive(move || {
                                                full_size_terminal.get().is_some_and(|active| active != terminal_id)
                                            })
                                            is_full_size=Signal::derive(move || {
                                                full_size_terminal.get() == Some(terminal_id)
                                            })
                                            on_full_size=on_full_size
                                        />
                                    }
                                }
                            />
                            <Show when=move || full_size_terminal.get().is_none()>
                                <For
                                    each=move || {
                                        let cols = workspace.get().map(|w| w.grid_cols).unwrap_or(1);
                                        (0..cols.saturating_sub(1) as usize).collect::<Vec<_>>()
                                    }
                                    key=|i| *i
                                    children=move |i| {
                                        view! {
                                            <button
                                                type="button"
                                                class="ws-term-grid__resize ws-term-grid__resize--col"
                                                style=move || grid_col_handle_style(i, &col_fr.get())
                                                aria-label=move || i18n.tr(I18nKey::WsResizeTermCols)()
                                                on:mousedown=move |ev| {
                                                    ev.prevent_default();
                                                    let total_px = ev
                                                        .current_target()
                                                        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                                                        .and_then(|el| el.parent_element())
                                                        .map(|el| el.client_width() as f64)
                                                        .unwrap_or(1.0);
                                                    drag_state.set(Some(GridDragState {
                                                            axis: GridResizeAxis::Col,
                                                            index: i,
                                                            start_pos: ev.client_x() as f64,
                                                            start_sizes: col_fr.get_untracked(),
                                                            total_px,
                                                    }));
                                                }
                                            ></button>
                                        }
                                    }
                                />
                                <For
                                    each=move || {
                                        let rows = workspace.get().map(|w| w.grid_rows).unwrap_or(1);
                                        (0..rows.saturating_sub(1) as usize).collect::<Vec<_>>()
                                    }
                                    key=|i| *i
                                    children=move |i| {
                                        view! {
                                            <button
                                                type="button"
                                                class="ws-term-grid__resize ws-term-grid__resize--row"
                                                style=move || grid_row_handle_style(i, &row_fr.get())
                                                aria-label=move || i18n.tr(I18nKey::WsResizeTermRows)()
                                                on:mousedown=move |ev| {
                                                    ev.prevent_default();
                                                    let total_px = ev
                                                        .current_target()
                                                        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                                                        .and_then(|el| el.parent_element())
                                                        .map(|el| el.client_height() as f64)
                                                        .unwrap_or(1.0);
                                                    drag_state.set(Some(GridDragState {
                                                            axis: GridResizeAxis::Row,
                                                            index: i,
                                                            start_pos: ev.client_y() as f64,
                                                            start_sizes: row_fr.get_untracked(),
                                                            total_px,
                                                    }));
                                                }
                                            ></button>
                                        }
                                    }
                                />
                            </Show>
                        </div>
                    </div>
                    <DynamicCenterPanels workspace_id=workspace_id active_tab_id=active_center_tab_id />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn CenterTabStrip(workspace_id: u64, active_tab_id: Memo<u64>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();

    view! {
        <header class="workspace-center-tabs">
            <div class="workspace-center-tabs__strip" role="tablist" aria-label="Workspace views">
                <For
                    each=move || wb.center_tabs_for_workspace(workspace_id)
                    key=|tab| tab.id
                    children=move |tab| {
                        view! {
                            <CenterTabButton
                                workspace_id=workspace_id
                                tab=tab
                                active_tab_id=active_tab_id
                            />
                        }
                    }
                />
            </div>
        </header>
    }
}

#[component]
fn CenterTabButton(workspace_id: u64, tab: CenterTab, active_tab_id: Memo<u64>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let id = tab.id;
    let title = tab.title.clone();
    let closeable = tab.closeable();
    let icon = center_tab_icon(&tab.kind);

    view! {
        <button
            type="button"
            role="tab"
            aria-selected=move || active_tab_id.get() == id
            class="workspace-center-tab"
            class:workspace-center-tab--active=move || active_tab_id.get() == id
            title=title.clone()
            on:click=move |_| wb.set_active_center_tab(workspace_id, id)
        >
            <span class="workspace-center-tab__icon" aria-hidden="true">
                <LxIcon icon=icon width="14px" height="14px" />
            </span>
            <span class="workspace-center-tab__label">{title.clone()}</span>
            <Show when=move || closeable>
                <span
                    role="button"
                    tabindex="0"
                    class="workspace-center-tab__close"
                    aria-label="Close tab"
                    title="Close tab"
                    on:click=move |ev: MouseEvent| {
                        ev.prevent_default();
                        ev.stop_propagation();
                        wb.close_center_tab(workspace_id, id);
                    }
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        let key = ev.key();
                        if key == "Enter" || key == " " {
                            ev.prevent_default();
                            ev.stop_propagation();
                            wb.close_center_tab(workspace_id, id);
                        }
                    }
                >
                    <LxIcon icon=icondata::LuX width="12px" height="12px" />
                </span>
            </Show>
        </button>
    }
}

#[component]
fn DynamicCenterPanels(workspace_id: u64, active_tab_id: Memo<u64>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let ui = expect_context::<HarnessUiService>();
    let embed = expect_context::<BrowserEmbedSurface>();

    view! {
        <For
            each=move || {
                wb.center_tabs_for_workspace(workspace_id)
                    .into_iter()
                    .filter(|tab| !matches!(tab.kind, CenterTabKind::Terminals))
                    .collect::<Vec<_>>()
            }
            key=|tab| tab.id
            children=move |tab| {
                let tab_id = tab.id;
                match tab.kind {
                    CenterTabKind::Settings => view! {
                        <div
                            class="workspace-center-panel workspace-center-panel--scroll"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <SettingsDock ui=ui wb=wb embed=embed />
                        </div>
                    }.into_any(),
                    CenterTabKind::FilePreview { rel_path } => view! {
                        <div
                            class="workspace-center-panel"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <FilePreviewDock workspace_id=workspace_id rel_path=rel_path />
                        </div>
                    }.into_any(),
                    CenterTabKind::Terminals => view! { <></> }.into_any(),
                }
            }
        />
    }
}

#[component]
fn FilePreviewDock(workspace_id: u64, rel_path: String) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let result = RwSignal::new(None::<Result<TextFilePreview, String>>);
    let load_gen = RwSignal::new(0_u32);
    let rel_for_effect = rel_path.clone();

    Effect::new(move |_| {
        let _ = load_gen.get();
        result.set(None);
        if !is_tauri_shell() {
            result.set(Some(Err(
                "File preview is available in the desktop app.".into()
            )));
            return;
        }
        let Some(workspace) = wb
            .workspaces()
            .get()
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
        else {
            result.set(Some(Err("Workspace not found.".into())));
            return;
        };
        let root = workspace.cwd;
        let path = rel_for_effect.clone();
        leptos::task::spawn_local(async move {
            let next = read_workspace_text_file(root, path).await;
            result.set(Some(next));
        });
    });

    view! {
        <article class="file-preview">
            <header class="file-preview__header">
                <div class="file-preview__title">
                    <span class="file-preview__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuFileText width="1rem" height="1rem" />
                    </span>
                    <span>{rel_path.clone()}</span>
                </div>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    on:click=move |_| load_gen.update(|n| *n = n.wrapping_add(1))
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                        <span>"Refresh"</span>
                    </span>
                </button>
            </header>
            {move || match result.get() {
                None => view! {
                    <div class="file-preview__status">"Loading file..."</div>
                }.into_any(),
                Some(Err(err)) => view! {
                    <div class="file-preview__status file-preview__status--error">{err}</div>
                }.into_any(),
                Some(Ok(preview)) => view! {
                    <Show when=move || preview.truncated>
                        <div class="file-preview__notice">
                            {format!("Preview truncated at 512 KiB of {} bytes.", preview.byte_len)}
                        </div>
                    </Show>
                    <pre class="file-preview__content"><code>{preview.content}</code></pre>
                }.into_any(),
            }}
        </article>
    }
}

fn center_tab_icon(kind: &CenterTabKind) -> icondata::Icon {
    match kind {
        CenterTabKind::Terminals => icondata::LuTerminal,
        CenterTabKind::Settings => icondata::LuSettings2,
        CenterTabKind::FilePreview { .. } => icondata::LuFileText,
    }
}

#[component]
fn WorkspaceEmptyState() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let ui = expect_context::<HarnessUiService>();
    let embed = expect_context::<BrowserEmbedSurface>();
    let prefs = expect_context::<AppPrefsService>();
    let i18n = expect_context::<I18nService>();

    let shortcut_mode = move || prefs.shortcut_mode().get();

    view! {
        <div class="workbench-empty-editor">
            <p class="workbench-empty-editor__lead">{move || i18n.tr(I18nKey::WsEmptyLead)()}</p>
            <div class="workbench-empty-editor__logo-wrap" aria-hidden="true">
                <img
                    class="workbench-empty-editor__logo"
                    src="/public/blxcode.png"
                    alt=""
                    width="192"
                    height="192"
                    decoding="async"
                />
            </div>
            <p class="workbench-empty-editor__note">{move || i18n.tr(I18nKey::WsEmptyNote)()}</p>
            <p class="harness-quickopen-section workbench-empty-recent-heading">
                {move || i18n.tr(I18nKey::QkRecentHeading)()}
            </p>
            <ul class="harness-cmd-list workbench-empty-recent-list" role="list">
                <Show
                    when=move || {
                        wb.recent_workspaces()
                            .get()
                            .iter()
                            .any(|it| workspace_entry_has_folder(&it.workspace))
                    }
                    fallback=move || {
                        view! {
                            <li class="workbench-empty-recent-empty" role="status">
                                {move || i18n.tr(I18nKey::QkEmptyRecent)()}
                            </li>
                        }
                    }
                >
                    <For
                        each=move || {
                            wb.recent_workspaces()
                                .get()
                                .into_iter()
                                .enumerate()
                                .filter(|(_, it)| workspace_entry_has_folder(&it.workspace))
                                .collect::<Vec<_>>()
                        }
                        key=|(_, it)| it.workspace.cwd.clone()
                        children=move |(orig_idx, item)| {
                            let title = item.workspace.title.clone();
                            let cwd = item.workspace.cwd.clone();
                            view! {
                                <li class="harness-cmd-li workbench-recent-row">
                                    <button
                                        type="button"
                                        class="harness-cmd-btn"
                                        on:click=move |_| {
                                            wb.reopen_recent_workspace(orig_idx);
                                            let wb_c = wb;
                                            let embed_c = embed;
                                            leptos::task::spawn_local(async move {
                                                TimeoutFuture::new(48).await;
                                                let _ =
                                                    sync_embedded_browser_layer(wb_c, embed_c).await;
                                            });
                                        }
                                    >
                                        <span class="harness-cmd-btn__icon" aria-hidden="true">
                                            <LxIcon icon=icondata::LuFolder width="1rem" height="1rem" />
                                        </span>
                                        <span class="harness-cmd-btn__text">
                                            <span class="harness-cmd-title">{title}</span>
                                            <span class="harness-cmd-sub">{cwd}</span>
                                        </span>
                                    </button>
                                    <button
                                        type="button"
                                        class="workbench-recent-remove"
                                        aria-label=move || i18n.tr(I18nKey::QkRecentRemoveAria)()
                                        on:click=move |ev: MouseEvent| {
                                            ev.stop_propagation();
                                            ev.prevent_default();
                                            wb.remove_recent_workspace(orig_idx);
                                        }
                                    >
                                        <span aria-hidden="true">
                                            <LxIcon icon=icondata::LuX width="0.85rem" height="0.85rem" />
                                        </span>
                                    </button>
                                </li>
                            }
                        }
                    />
                </Show>
            </ul>
            <ul class="workbench-shortcut-list">
                <ShortcutActionRow
                    icon=icondata::LuFolderSearch
                    label=I18nKey::WsKwQuickOpen
                    keys=move || ShortcutKeys::quick_open(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::OpenQuickOpen,
                        ui,
                        wb,
                        embed,
                    )
                />
                <ShortcutActionRow
                    icon=icondata::LuPanelRight
                    label=I18nKey::WsKwSidePanel
                    keys=move || ShortcutKeys::side_panel(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::ToggleRightPanel,
                        ui,
                        wb,
                        embed,
                    )
                />
                <li class="workbench-shortcut-row workbench-shortcut-row--spacer" aria-hidden="true"></li>
                <ShortcutActionRow
                    icon=icondata::LuSparkles
                    label=I18nKey::WsKwAgent
                    keys=move || ShortcutKeys::agent(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::RightTab(RightPanelTab::Agent),
                        ui,
                        wb,
                        embed,
                    )
                />
                <ShortcutActionRow
                    icon=icondata::LuGlobe
                    label=I18nKey::WsKwBrowser
                    keys=move || ShortcutKeys::browser(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::RightTab(RightPanelTab::Browser),
                        ui,
                        wb,
                        embed,
                    )
                />
                <ShortcutActionRow
                    icon=icondata::LuLayers
                    label=I18nKey::WsKwMemory
                    keys=move || ShortcutKeys::memory(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::RightTab(RightPanelTab::Memory),
                        ui,
                        wb,
                        embed,
                    )
                />
                <li class="workbench-shortcut-row workbench-shortcut-row--spacer" aria-hidden="true"></li>
                <ShortcutActionRow
                    icon=icondata::LuTerminal
                    label=I18nKey::WsKwTerminal
                    keys=move || ShortcutKeys::terminal(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::OpenNewTerminal,
                        ui,
                        wb,
                        embed,
                    )
                />
                <ShortcutActionRow
                    icon=icondata::LuCommand
                    label=I18nKey::WsKwCmdPalette
                    keys=move || ShortcutKeys::command_palette(shortcut_mode())
                    on_activate=shortcut_cb(
                        HarnessShortcutAction::ToggleCommandPalette,
                        ui,
                        wb,
                        embed,
                    )
                />
            </ul>
        </div>
    }
}

fn render_shortcut_keys(keys: ShortcutKeys, i18n: I18nService) -> impl IntoView {
    match keys {
        ShortcutKeys::Combo(parts) => view! {
            {parts
                .iter()
                .enumerate()
                .map(|(i, key)| view! {
                    <Show when=move || i != 0>
                        <span class="workbench-kbd-gap">"+"</span>
                    </Show>
                    <kbd class="workbench-kbd">{*key}</kbd>
                })
                .collect_view()}
        }
        .into_any(),
        ShortcutKeys::Chord { prefix, second } => view! {
            {prefix
                .iter()
                .enumerate()
                .map(|(i, key)| view! {
                    <Show when=move || i != 0>
                        <span class="workbench-kbd-gap">"+"</span>
                    </Show>
                    <kbd class="workbench-kbd">{*key}</kbd>
                })
                .collect_view()}
            <span class="workbench-kbd-chord-gap">{move || i18n.tr(I18nKey::WsKwThen)()}</span>
            <kbd class="workbench-kbd">{second}</kbd>
        }
        .into_any(),
    }
}

fn shortcut_cb(
    action: HarnessShortcutAction,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> Callback<(), ()> {
    Callback::new(move |()| dispatch_shortcut_action(action, ui, wb, embed))
}

#[component]
fn ShortcutActionRow(
    icon: icondata::Icon,
    label: I18nKey,
    keys: impl Fn() -> ShortcutKeys + Send + Sync + 'static,
    on_activate: Callback<(), ()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <li class="workbench-shortcut-li">
            <button
                type="button"
                class="workbench-shortcut-row workbench-shortcut-row--action"
                on:click=move |_| on_activate.run(())
            >
                <span class="workbench-shortcut-row__lead">
                    <span class="workbench-shortcut-row__icon-wrap" aria-hidden="true">
                        <LxIcon icon=icon width="0.9rem" height="0.9rem" />
                    </span>
                    <span class="workbench-shortcut-row__label">{move || i18n.tr(label)()}</span>
                </span>
                <span class="workbench-shortcut-row__keys">
                    {move || render_shortcut_keys(keys(), i18n).into_any()}
                </span>
            </button>
        </li>
    }
}

#[component]
fn TerminalSlotSurface(
    workspace_id: u64,
    slot_id: u64,
    index: usize,
    cwd: String,
    agent_slug: String,
    is_workspace_active: Signal<bool>,
    hidden: Signal<bool>,
    is_full_size: Signal<bool>,
    on_full_size: Callback<(), ()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    // Hydrate split layout from persisted workspace state so a restart
    // preserves the user's exact pane grid.
    let persisted = wb.slot_panes(workspace_id, slot_id);
    let pane_ids = RwSignal::new(persisted.pane_ids);
    let next_pane_id = RwSignal::new(persisted.next_pane_id);
    let split_axis = RwSignal::new(persisted.axis);

    // Push every change back into the workspace so the workbench
    // auto-save effect can persist it. set_slot_panes deduplicates so
    // unchanged ticks don't trigger spurious saves.
    Effect::new(move |_| {
        let snapshot = crate::workbench::state::SlotPaneState {
            axis: split_axis.get(),
            pane_ids: pane_ids.get(),
            next_pane_id: next_pane_id.get(),
        };
        wb.set_slot_panes(workspace_id, slot_id, snapshot);
    });

    view! {
        <div
            class=move || {
                let mut class = String::from("ws-term-slot");
                if hidden.get() {
                    class.push_str(" ws-term-slot--hidden");
                }
                class
            }
        >
            <div
                class="ws-term-pane-grid"
                style=move || pane_grid_style(split_axis.get(), pane_ids.get().len())
            >
                <For
                    each=move || {
                        let loc = i18n.locale().get();
                        pane_ids
                            .get()
                            .into_iter()
                            .map(move |id| (loc, id))
                            .collect::<Vec<_>>()
                    }
                    key=|(loc, pane_id)| format!("{}-{pane_id}", loc.as_str())
                    children=move |(loc, pane_id)| {
                        let slug = agent_slug.clone();
                        let pane_index = pane_ids
                            .get_untracked()
                            .iter()
                            .position(|id| *id == pane_id)
                            .unwrap_or_default();
                        let term_word = lookup(loc, I18nKey::WsTermSlot);
                        let slug_trim = slug.trim();
                        let role = match slug_trim {
                            "" => term_word.to_string(),
                            "claude" => lookup(loc, I18nKey::WzAgentClaude).to_string(),
                            "codex" => lookup(loc, I18nKey::WzAgentCodex).to_string(),
                            "gemini" => lookup(loc, I18nKey::WzAgentGemini).to_string(),
                            "opencode" => lookup(loc, I18nKey::WzAgentOpencode).to_string(),
                            "cursor" => lookup(loc, I18nKey::WzAgentCursor).to_string(),
                            other => other.to_string(),
                        };
                        let title = if pane_ids.get_untracked().len() <= 1 {
                            lookup(loc, I18nKey::WsTermPaneTitleSingle)
                                .replace("{role}", &role)
                                .replace("{term}", term_word)
                                .replace("{n}", &(index + 1).to_string())
                        } else {
                            lookup(loc, I18nKey::WsTermPaneTitleMulti)
                                .replace("{role}", &role)
                                .replace("{term}", term_word)
                                .replace("{slot}", &(index + 1).to_string())
                                .replace("{pane}", &(pane_index + 1).to_string())
                        };

                        let on_split_vertical = Callback::new(move |()| {
                            split_axis.set(TerminalSplitAxis::Vertical);
                            insert_pane_after(pane_ids, next_pane_id, pane_id);
                        });
                        let on_split_horizontal = Callback::new(move |()| {
                            split_axis.set(TerminalSplitAxis::Horizontal);
                            insert_pane_after(pane_ids, next_pane_id, pane_id);
                        });
                        let on_close = Callback::new(move |()| {
                            if pane_ids.with_untracked(|ids| ids.len() > 1) {
                                pane_ids.update(|ids| ids.retain(|id| *id != pane_id));
                            } else {
                                wb.close_terminal(workspace_id, slot_id);
                            }
                        });
                        // Hide the X when this cell cannot actually be removed:
                        // single pane in the only remaining terminal slot.
                        let workspaces_sig = wb.workspaces();
                        let can_close = Signal::derive(move || {
                            if pane_ids.with(|ids| ids.len() > 1) {
                                return true;
                            }
                            workspaces_sig.with(|ws| {
                                ws.iter()
                                    .find(|w| w.id == workspace_id)
                                    .map(|w| w.terminal_count > 1)
                                    .unwrap_or(false)
                            })
                        });

                        let storage_key = workspaces_sig.with_untracked(|ws| {
                            ws.iter()
                                .find(|w| w.id == workspace_id)
                                .map(|w| w.storage_key.clone())
                                .unwrap_or_default()
                        });
                        let terminal_key = format!("{storage_key}:{slot_id}:{pane_id}");
                        view! {
                            <WorkspaceTerminalCell
                                workspace_id=workspace_id
                                slot_id=slot_id
                                pane_id=pane_id
                                cwd=cwd.clone()
                                grid_index=index
                                agent_slug=agent_slug.clone()
                                title=title
                                terminal_key=terminal_key
                                is_workspace_active=is_workspace_active
                                is_slot_hidden=hidden
                                is_full_size=is_full_size
                                on_full_size=on_full_size
                                on_split_vertical=on_split_vertical
                                on_split_horizontal=on_split_horizontal
                                on_close=on_close
                                can_close=can_close
                            />
                        }
                    }
                />
            </div>
        </div>
    }
}

fn insert_pane_after(pane_ids: RwSignal<Vec<u64>>, next_pane_id: RwSignal<u64>, after_id: u64) {
    let new_id = next_pane_id.get_untracked();
    next_pane_id.set(new_id.saturating_add(1));
    pane_ids.update(|ids| {
        let insert_at = ids
            .iter()
            .position(|id| *id == after_id)
            .map(|i| i + 1)
            .unwrap_or(ids.len());
        ids.insert(insert_at, new_id);
    });
}

fn pane_grid_style(axis: TerminalSplitAxis, count: usize) -> String {
    let count = count.max(1);
    match axis {
        TerminalSplitAxis::Vertical => format!(
            "display:grid;grid-template-columns:repeat({count},minmax(0,1fr));grid-template-rows:minmax(0,1fr);gap:4px;flex:1;min-width:0;min-height:0;"
        ),
        TerminalSplitAxis::Horizontal => format!(
            "display:grid;grid-template-columns:minmax(0,1fr);grid-template-rows:repeat({count},minmax(0,1fr));gap:4px;flex:1;min-width:0;min-height:0;"
        ),
    }
}

fn terminal_slots(workspace: &WorkspaceEntry) -> Vec<TerminalRenderSlot> {
    workspace
        .slot_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(index, id)| TerminalRenderSlot {
            id,
            index,
            agent_slug: workspace
                .slot_agent_labels
                .get(index)
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

fn fr_template(values: &[f64]) -> String {
    if values.is_empty() {
        return "minmax(0,1fr)".to_string();
    }
    values
        .iter()
        .map(|v| format!("minmax(0,{:.3}fr)", v.max(0.25)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn grid_handle_offset(index: usize, values: &[f64]) -> f64 {
    let total = values.iter().sum::<f64>().max(1.0);
    values.iter().take(index + 1).sum::<f64>() / total * 100.0
}

/// Nudge the browser to resolve flex/grid sizes for terminal panes.
fn force_workbench_terminal_layout() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(grids) = doc.query_selector_all(".ws-term-grid") else {
        return;
    };
    for i in 0..grids.length() {
        let Some(node) = grids.item(i) else {
            continue;
        };
        let Some(el) = node.dyn_ref::<HtmlElement>() else {
            continue;
        };
        let _ = el.offset_height();
        let _ = el.get_bounding_client_rect();
        if let Ok(cells) = el.query_selector_all(".ws-term-cell__xterm") {
            for j in 0..cells.length() {
                if let Some(c) = cells.item(j).and_then(|n| n.dyn_into::<HtmlElement>().ok()) {
                    let _ = c.offset_height();
                    let _ = c.get_bounding_client_rect();
                }
            }
        }
    }
}

fn grid_col_handle_style(index: usize, values: &[f64]) -> String {
    format!("left:{:.4}%;", grid_handle_offset(index, values))
}

fn grid_row_handle_style(index: usize, values: &[f64]) -> String {
    format!("top:{:.4}%;", grid_handle_offset(index, values))
}
