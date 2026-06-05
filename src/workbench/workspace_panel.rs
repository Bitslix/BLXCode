use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_session_roles_list, popout_focus, pty_peek_output, pty_write, SessionRoleView,
};
use crate::workbench::app_prefs::AppPrefsService;
use crate::workbench::browser_tab::sync_embedded_browser_layer;
use crate::workbench::create_workspace_wizard::WorkspaceConfigurator;
use crate::workbench::diagram_gallery::{DiagramGallery, GalleryScope};
use crate::workbench::file_diff::FileDiffDock;
use crate::workbench::file_preview::FilePreviewDock;
use crate::workbench::harness_chords::dispatch_shortcut_action;
use crate::workbench::harness_ui::SettingsDock;
use crate::workbench::memory_panel::MemoryPanel;
use crate::workbench::shortcut_config::ShortcutAction;
use crate::workbench::state::{
    workspace_entry_has_folder, BrowserEmbedSurface, CanvasNodeKind, CanvasNodeLayout,
    CanvasPortDirection, CanvasPortRef, CanvasTransferMode, CenterTab, CenterTabKind,
    HarnessUiService, TerminalSplitAxis, WorkspaceEntry, WorkspaceViewMode,
    CENTER_TERMINALS_TAB_ID,
};
use crate::workbench::terminal_cell::WorkspaceTerminalCell;
use crate::workbench::terminal_context_menu::{
    dispatch_terminal_menu_action, install_terminal_clipboard_listeners, TerminalContextMenu,
    TerminalContextMenuState, TerminalMenuAction,
};
use crate::workbench::terminal_glue::{
    terminal_observe_workspace_grid, terminal_unobserve_workspace_grid,
};
use crate::workbench::terminal_slot_dnd::{
    drag_event_data_transfer, is_terminal_drag, read_drag_payload, GhostPos,
    TerminalSlotDragService,
};
use crate::workbench::toast::ToastService;
use crate::workbench::{WorkbenchService, WorkspaceKanban};
use base64::Engine;
use gloo_timers::future::TimeoutFuture;
use leptos::callback::Callback;
use leptos::html;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, HtmlElement, MouseEvent};

/// Whether a terminal-slot drag in flight should be accepted as a drop on the
/// grid slot `(workspace_id, slot_id)`.
///
/// The accept decision is keyed on our own synchronous session flag, not on
/// `DataTransfer` contents: Chromium/WebView2 (Windows) blocks `getData()`
/// during `dragenter`/`dragover`, so the payload is unreadable there and the
/// in-memory drag meta (`slot_dnd.active`) is the reliable source. When the
/// source slot is not yet known (meta not populated, payload protected) we
/// optimistically accept and let the `drop` handler make the final call —
/// without `preventDefault` on dragover, WebView2 never fires `drop` at all.
fn accepts_slot_drop(
    slot_dnd: &TerminalSlotDragService,
    de: &DragEvent,
    workspace_id: u64,
    slot_id: u64,
) -> bool {
    let dt = drag_event_data_transfer(de);
    let is_slot_drag = slot_dnd.session_active() || dt.as_ref().is_some_and(is_terminal_drag);
    if !is_slot_drag {
        return false;
    }
    // Prefer the in-memory meta (works on every platform); fall back to the
    // DataTransfer payload (WebKit only — Chromium protects it here).
    let source = slot_dnd
        .active
        .get_untracked()
        .map(|m| (m.workspace_id, m.slot_id))
        .or_else(|| {
            dt.as_ref()
                .and_then(read_drag_payload)
                .map(|p| (p.workspace_id, p.slot_id))
        });
    match source {
        // Same-workspace, non-source slot → valid grid target. A foreign
        // workspace's terminal is transferred via the sidebar, not the grid.
        Some((src_ws, src_slot)) => src_ws == workspace_id && src_slot != slot_id,
        // Source unknown yet — accept; `drop` re-validates with readable data.
        None => true,
    }
}

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

#[derive(Clone)]
struct CenterSplitDragState {
    start_x: f64,
    start_fraction: f64,
    total_px: f64,
}

#[derive(Clone)]
struct CanvasNodeDragState {
    slot_id: u64,
    start_x: f64,
    start_y: f64,
    layout: CanvasNodeLayout,
    resizing: bool,
}

#[derive(Clone)]
struct SwarmDragState {
    node_id: String,
    start_x: f64,
    start_y: f64,
    origin_x: f64,
    origin_y: f64,
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
    let toast = expect_context::<ToastService>();
    let workspaces = wb.workspaces();
    let active_id = wb.active_id();
    let term_menu_state: RwSignal<Option<TerminalContextMenuState>> = RwSignal::new(None);

    on_cleanup(install_terminal_clipboard_listeners(
        term_menu_state,
        i18n,
        toast,
    ));

    let on_term_menu_action = Callback::new(move |action: TerminalMenuAction| {
        let Some(menu) = term_menu_state.get_untracked() else {
            return;
        };
        term_menu_state.set(None);
        dispatch_terminal_menu_action(action, menu.term_id, i18n, toast);
    });

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
            <Show when=move || !workspaces.get().is_empty()>
                <TerminalContextMenu state=term_menu_state on_action=on_term_menu_action />
            </Show>
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
    let canvas_drag_state = RwSignal::new(None::<CanvasNodeDragState>);
    let canvas_port_start = RwSignal::new(None::<CanvasPortRef>);
    let memory_split_fraction = RwSignal::new(0.5_f64);
    let memory_split_drag = RwSignal::new(None::<CenterSplitDragState>);

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
        let wb = wb;
        move |ev| {
            let Some(ev) = ev.dyn_ref::<MouseEvent>() else {
                return;
            };
            if let Some(drag) = memory_split_drag.try_get_untracked().flatten() {
                ev.prevent_default();
                let delta = (ev.client_x() as f64 - drag.start_x) / drag.total_px.max(1.0);
                memory_split_fraction.set((drag.start_fraction + delta).clamp(0.25, 0.75));
                force_workbench_terminal_layout();
                wb.bump_terminal_layout();
                return;
            }
            if let Some(drag) = canvas_drag_state.try_get_untracked().flatten() {
                ev.prevent_default();
                let mut next = drag.layout.clone();
                let dx = ev.client_x() as f64 - drag.start_x;
                let dy = ev.client_y() as f64 - drag.start_y;
                if drag.resizing {
                    next.width = (drag.layout.width + dx).max(320.0);
                    next.height = (drag.layout.height + dy).max(210.0);
                } else {
                    next.x = (drag.layout.x + dx).max(12.0);
                    next.y = (drag.layout.y + dy).max(12.0);
                }
                wb.set_canvas_terminal_layout(workspace_id, drag.slot_id, next);
                force_workbench_terminal_layout();
                return;
            }
            let Some(drag) = drag_state.try_get_untracked().flatten() else {
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
        let wb = wb;
        move |_| {
            drag_state.try_set(None);
            canvas_drag_state.try_set(None);
            if memory_split_drag.try_get_untracked().flatten().is_some() {
                memory_split_drag.try_set(None);
                force_workbench_terminal_layout();
                wb.bump_terminal_layout();
            }
        }
    });

    on_cleanup(move || {
        drop(move_handle);
        drop(up_handle);
    });

    let is_configuring =
        Memo::new(move |_| workspace.get().map(|w| w.configuring).unwrap_or(false));
    let slot_dnd = expect_context::<TerminalSlotDragService>();

    let slot_drag_enabled =
        Memo::new(move |_| !is_configuring.get() && full_size_terminal.get().is_none());
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
    let view_mode = Memo::new(move |_| wb.view_mode_for_workspace(workspace_id));
    let memory_split_view = RwSignal::new(false);
    let memory_split_active = Memo::new(move |_| {
        if !memory_split_view.get() {
            return false;
        }
        let active_tab_id = active_center_tab_id.get();
        wb.center_tabs_for_workspace(workspace_id)
            .into_iter()
            .any(|tab| tab.id == active_tab_id && matches!(tab.kind, CenterTabKind::Memory))
    });
    let is_mode_tab_active =
        Memo::new(move |_| active_center_tab_id.get() == CENTER_TERMINALS_TAB_ID);
    let canvas_active = Memo::new(move |_| {
        is_mode_tab_active.get() && view_mode.get() == WorkspaceViewMode::Canvas
    });
    let grid_active =
        Memo::new(move |_| is_mode_tab_active.get() && view_mode.get() == WorkspaceViewMode::Grid);
    let swarm_active =
        Memo::new(move |_| is_mode_tab_active.get() && view_mode.get() == WorkspaceViewMode::Swarm);
    let terminal_split_active =
        Memo::new(move |_| memory_split_active.get() && !swarm_active.get());
    let on_canvas_port = Callback::new(move |port: CanvasPortRef| {
        let can_start = matches!(
            port.direction,
            CanvasPortDirection::Stdout | CanvasPortDirection::AgentCommand
        );
        if let Some(start) = canvas_port_start.get_untracked() {
            if wb
                .connect_canvas_ports(workspace_id, start.clone(), port.clone())
                .is_some()
            {
                canvas_port_start.set(None);
            } else if can_start {
                canvas_port_start.set(Some(port));
            } else {
                canvas_port_start.set(None);
            }
        } else if can_start {
            canvas_port_start.set(Some(port));
        }
    });
    Effect::new({
        let wb = wb;
        move |_| {
            if terminal_split_active.get() {
                let _ = memory_split_fraction.get();
                force_workbench_terminal_layout();
                wb.bump_terminal_layout();
            }
        }
    });
    let on_memory_split_down = move |ev: MouseEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        let total_px = ev
            .current_target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.parent_element())
            .map(|parent| parent.client_width() as f64)
            .unwrap_or(1.0);
        memory_split_drag.set(Some(CenterSplitDragState {
            start_x: ev.client_x() as f64,
            start_fraction: memory_split_fraction.get_untracked(),
            total_px,
        }));
    };

    // The terminal grid stays mounted for the entire life of the workspace
    // as soon as the inline configurator is done; it is hidden via
    // `--hidden` (display: none) when a different center tab is active.
    // We deliberately do NOT unmount on tab switch: every TerminalSlotSurface
    // has an `on_cleanup` that calls `pty_kill` + `unregister_pty_session`,
    // so unmounting would terminate running agents (claude/codex) and wipe
    // the PTY registry — `pty_sessions_for_workspace` would then return an
    // empty list and the file-preview right-click menu would show
    // "No open terminals in any workspace". xterm.js inside the cells is
    // paused via `is_workspace_active` while hidden and refits on tab
    // re-activation, so a hidden 0x0 layout is harmless.
    let grid_mounted = Memo::new(move |_| !is_configuring.get());

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
            // Tab strip is always present so the user can switch to
            // Settings (or close the Terminals tab) even while the
            // configurator is up.
            <CenterTabStrip workspace_id=workspace_id active_tab_id=active_center_tab_id />
            <div
                class="workspace-center-tab-body"
                class:workspace-center-tab-body--split=move || terminal_split_active.get()
            >
                <Show when=move || is_configuring.get() && active_center_tab_id.get() == CENTER_TERMINALS_TAB_ID>
                    <div class="workspace-center-panel">
                        <WorkspaceConfigurator workspace_id=workspace_id />
                    </div>
                </Show>
                <Show when=move || grid_mounted.get()>
                    <div
                        class=move || {
                            let mut class = String::from("workspace-center-panel workspace-terminal-board");
                            match view_mode.get() {
                                WorkspaceViewMode::Grid => class.push_str(" workspace-terminal-board--grid"),
                                WorkspaceViewMode::Canvas => class.push_str(" workspace-terminal-board--canvas"),
                                WorkspaceViewMode::Swarm => class.push_str(" workspace-terminal-board--swarm"),
                            }
                            class
                        }
                        class:workspace-center-panel--hidden=move || {
                            !grid_active.get() && !canvas_active.get() && !terminal_split_active.get()
                        }
                        style=move || {
                            if terminal_split_active.get() {
                                format!(
                                    "flex:0 1 calc({:.3}% - 2px);",
                                    memory_split_fraction.get() * 100.0
                                )
                            } else {
                                String::new()
                            }
                        }
                    >
                        <div
                            class=move || {
                                let mut class = String::from("ws-term-grid");
                                if canvas_active.get() {
                                    class.push_str(" ws-term-grid--canvas");
                                }
                                if slot_dnd.active.get().is_some() {
                                    class.push_str(" ws-term-grid--drag-active");
                                }
                                class
                            }
                            node_ref=term_grid_ref
                            style=move || {
                                if canvas_active.get() {
                                    return "display:block;position:relative;width:100%;height:100%;".to_string();
                                }
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
                            <Show when=move || canvas_active.get()>
                                <CanvasEdgesOverlay workspace_id=workspace_id />
                                <CanvasAgentHub
                                    workspace_id=workspace_id
                                    active_port=Signal::derive(move || {
                                        canvas_port_start.get().map(|p| canvas_port_identity(&p))
                                    })
                                    on_port=on_canvas_port
                                />
                            </Show>
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
                                            slot_drag_enabled=slot_drag_enabled
                                            is_workspace_active=Signal::derive(move || {
                                                wb.active_id().get() == Some(workspace_id)
                                                    && (grid_active.get()
                                                        || canvas_active.get()
                                                        || terminal_split_active.get())
                                            })
                                            hidden=Signal::derive(move || {
                                                full_size_terminal.get().is_some_and(|active| active != terminal_id)
                                            })
                                            is_full_size=Signal::derive(move || {
                                                full_size_terminal.get() == Some(terminal_id)
                                            })
                                            on_full_size=on_full_size
                                            canvas_mode=Signal::derive(move || canvas_active.get())
                                            canvas_style=Signal::derive(move || {
                                                canvas_terminal_style(
                                                    &wb.workspaces()
                                                        .get()
                                                        .into_iter()
                                                        .find(|w| w.id == workspace_id),
                                                    terminal_id,
                                                    index,
                                                )
                                            })
                                            canvas_port_active=Signal::derive(move || {
                                                canvas_port_start.get().map(|p| canvas_port_identity(&p))
                                            })
                                            on_canvas_port=on_canvas_port
                                            on_canvas_drag_start=Callback::new(move |ev: MouseEvent| {
                                                ev.prevent_default();
                                                ev.stop_propagation();
                                                let layout = wb
                                                    .workspaces()
                                                    .with_untracked(|list| {
                                                        list.iter()
                                                            .find(|w| w.id == workspace_id)
                                                            .and_then(|w| w.canvas_view_state.terminal_nodes.get(&terminal_id).cloned())
                                                    })
                                                    .unwrap_or_else(|| CanvasNodeLayout::for_index(index));
                                                canvas_drag_state.set(Some(CanvasNodeDragState {
                                                    slot_id: terminal_id,
                                                    start_x: ev.client_x() as f64,
                                                    start_y: ev.client_y() as f64,
                                                    layout,
                                                    resizing: false,
                                                }));
                                            })
                                            on_canvas_resize_start=Callback::new(move |ev: MouseEvent| {
                                                ev.prevent_default();
                                                ev.stop_propagation();
                                                let layout = wb
                                                    .workspaces()
                                                    .with_untracked(|list| {
                                                        list.iter()
                                                            .find(|w| w.id == workspace_id)
                                                            .and_then(|w| w.canvas_view_state.terminal_nodes.get(&terminal_id).cloned())
                                                    })
                                                    .unwrap_or_else(|| CanvasNodeLayout::for_index(index));
                                                canvas_drag_state.set(Some(CanvasNodeDragState {
                                                    slot_id: terminal_id,
                                                    start_x: ev.client_x() as f64,
                                                    start_y: ev.client_y() as f64,
                                                    layout,
                                                    resizing: true,
                                                }));
                                            })
                                        />
                                    }
                                }
                            />
                            <Show when=move || full_size_terminal.get().is_none() && grid_active.get()>
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
                </Show>
                <Show when=move || swarm_active.get()>
                    <div class="workspace-center-panel workspace-swarm-panel">
                        <WorkspaceSwarmView workspace_id=workspace_id />
                    </div>
                </Show>
                <Show when=move || terminal_split_active.get()>
                    <button
                        type="button"
                        class="workspace-center-split-resizer"
                        class:workspace-center-split-resizer--active=move || {
                            memory_split_drag.get().is_some()
                        }
                        aria-label="Resize terminal and memory split"
                        on:mousedown=on_memory_split_down
                    >
                        <span aria-hidden="true"></span>
                    </button>
                </Show>
                <DynamicCenterPanels
                    workspace_id=workspace_id
                    active_tab_id=active_center_tab_id
                    memory_split_view=memory_split_view
                    memory_split_active=terminal_split_active
                    memory_split_fraction=memory_split_fraction
                />
                <Show when=move || memory_split_drag.get().is_some()>
                    <div class="workspace-center-split-shield" aria-hidden="true"></div>
                </Show>
            </div>
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
    let ui = expect_context::<HarnessUiService>();
    let prefs = expect_context::<AppPrefsService>();
    let i18n = expect_context::<I18nService>();
    let id = tab.id;
    let title = tab.title.clone();
    let icon = center_tab_icon(&tab.kind);

    // Closing the Terminals tab closes the whole workspace; gate it behind
    // the confirmation dialog when the user has it enabled, otherwise close
    // straight away.
    let close_terminals = move || {
        if wb.workspace_is_configuring(workspace_id)
            || !prefs.confirm_close_workspace_enabled().get_untracked()
        {
            wb.close_workspace(workspace_id);
        } else {
            ui.request_close_terminals_tab(workspace_id);
        }
    };
    // Terminals tabs route their close action through the confirmation
    // dialog; every other tab type closes immediately. The Terminals close
    // button is always visible — closing it is what triggers the "close
    // workspace" flow.
    let is_terminals = matches!(
        tab.kind,
        CenterTabKind::Terminals | CenterTabKind::Canvas | CenterTabKind::Swarm
    );
    let is_kanban = matches!(tab.kind, CenterTabKind::Kanban);

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
            <Show when=move || !is_kanban>
                <span
                    role="button"
                    tabindex="0"
                    class="workspace-center-tab__close"
                    aria-label=move || i18n.tr(I18nKey::CenterTabCloseAria)()
                    title=move || i18n.tr(I18nKey::CenterTabCloseAria)()
                    on:click=move |ev: MouseEvent| {
                        ev.prevent_default();
                        ev.stop_propagation();
                        if is_terminals {
                            close_terminals();
                        } else {
                            wb.close_center_tab(workspace_id, id);
                        }
                    }
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        let key = ev.key();
                        if key == "Enter" || key == " " {
                            ev.prevent_default();
                            ev.stop_propagation();
                            if is_terminals {
                                close_terminals();
                            } else {
                                wb.close_center_tab(workspace_id, id);
                            }
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
fn DynamicCenterPanels(
    workspace_id: u64,
    active_tab_id: Memo<u64>,
    memory_split_view: RwSignal<bool>,
    memory_split_active: Memo<bool>,
    memory_split_fraction: RwSignal<f64>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let ui = expect_context::<HarnessUiService>();
    let embed = expect_context::<BrowserEmbedSurface>();

    view! {
        <For
            each=move || {
                wb.center_tabs_for_workspace(workspace_id)
                    .into_iter()
                    .filter(|tab| {
                        !matches!(
                            tab.kind,
                            CenterTabKind::Terminals | CenterTabKind::Canvas | CenterTabKind::Swarm
                        )
                    })
                    .collect::<Vec<_>>()
            }
            key=|tab| tab.id
            children=move |tab| {
                let tab_id = tab.id;
                match tab.kind {
                    CenterTabKind::Kanban => view! {
                        <div
                            class="workspace-center-panel workspace-center-panel--kanban"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <WorkspaceKanban workspace_id=workspace_id />
                        </div>
                    }.into_any(),
                    CenterTabKind::Settings => view! {
                        <div
                            class="workspace-center-panel workspace-center-panel--scroll"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <SettingsDock ui=ui wb=wb embed=embed />
                        </div>
                    }.into_any(),
                    CenterTabKind::Memory => view! {
                        <div
                            class="workspace-center-panel workspace-center-panel--memory"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                            style=move || {
                                if memory_split_active.get() {
                                    format!(
                                        "flex:0 1 calc({:.3}% - 2px);",
                                        (1.0 - memory_split_fraction.get()) * 100.0
                                    )
                                } else {
                                    String::new()
                                }
                            }
                        >
                            <MemoryPanel centered=true split_view=memory_split_view />
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
                    CenterTabKind::FileDiff { rel_path, staged } => view! {
                        <div
                            class="workspace-center-panel workspace-center-panel--scroll"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <FileDiffDock
                                workspace_id=workspace_id
                                rel_path=rel_path
                                staged=staged
                            />
                        </div>
                    }.into_any(),
                    CenterTabKind::DiagramGallery { slug } => view! {
                        <div
                            class="workspace-center-panel"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <DiagramGallery
                                scope=GalleryScope::Plan { slug }
                                workspace_id=workspace_id
                            />
                        </div>
                    }.into_any(),
                    CenterTabKind::DiagramGroup { title, diagrams } => view! {
                        <div
                            class="workspace-center-panel"
                            class:workspace-center-panel--hidden=move || active_tab_id.get() != tab_id
                        >
                            <DiagramGallery
                                scope=GalleryScope::Ephemeral { title, diagrams }
                                workspace_id=workspace_id
                            />
                        </div>
                    }.into_any(),
                    CenterTabKind::Terminals | CenterTabKind::Canvas | CenterTabKind::Swarm => {
                        view! { <></> }.into_any()
                    }
                }
            }
        />
    }
}

fn center_tab_icon(kind: &CenterTabKind) -> icondata::Icon {
    match kind {
        CenterTabKind::Kanban => icondata::LuKanban,
        CenterTabKind::Terminals => icondata::LuTerminal,
        CenterTabKind::Canvas => icondata::LuWorkflow,
        CenterTabKind::Swarm => icondata::LuNetwork,
        CenterTabKind::Settings => icondata::LuSettings2,
        CenterTabKind::Memory => icondata::LuLayers,
        CenterTabKind::FilePreview { .. } => icondata::LuFileText,
        CenterTabKind::FileDiff { .. } => icondata::LuFileDiff,
        CenterTabKind::DiagramGallery { .. } => icondata::LuWorkflow,
        CenterTabKind::DiagramGroup { .. } => icondata::LuWorkflow,
    }
}

#[component]
fn WorkspaceEmptyState() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let embed = expect_context::<BrowserEmbedSurface>();
    let i18n = expect_context::<I18nService>();

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
            <div class="ws-config__recent workbench-empty-recent">
                <div class="ws-config__recent-head">
                    <span class="ws-config__recent-label">
                        <LxIcon icon=icondata::LuClock width="0.82rem" height="0.82rem" />
                        <span>{move || i18n.tr(I18nKey::QkRecentHeading)()}</span>
                        <span class="ws-config__recent-count">
                            {move || {
                                wb.recent_workspaces()
                                    .get()
                                    .iter()
                                    .filter(|it| workspace_entry_has_folder(&it.workspace))
                                    .count()
                            }}
                        </span>
                    </span>
                    <span class="ws-config__recent-note">"Last opened workspaces"</span>
                </div>
                <ul class="ws-config__recent-list workbench-empty-recent-list" role="list">
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
                            let terminal_count = item.workspace.terminal_count;
                            let base = cwd
                                .trim_end_matches(['/', '\\'])
                                .rsplit(['/', '\\'])
                                .next()
                                .unwrap_or(cwd.as_str())
                                .to_string();
                            let label = if title.trim().is_empty() {
                                base
                            } else {
                                title
                            };
                            let card_title = cwd.clone();
                            let path_label = cwd.clone();
                            view! {
                                <li class="ws-config__recent-item workbench-recent-row">
                                    <button
                                        type="button"
                                        class="ws-config__recent-card workbench-empty-recent-card"
                                        title=card_title
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
                                        <span class="ws-config__recent-icon" aria-hidden="true">
                                            <LxIcon icon=icondata::LuFolderClock width="0.9rem" height="0.9rem" />
                                        </span>
                                        <span class="ws-config__recent-text">
                                            <span class="ws-config__recent-title">{label}</span>
                                            <span class="ws-config__recent-path">{path_label}</span>
                                        </span>
                                        <span class="ws-config__recent-terminals">{terminal_count}</span>
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
                                            <LxIcon icon=icondata::LuX width="0.82rem" height="0.82rem" />
                                        </span>
                                    </button>
                                </li>
                            }
                        }
                    />
                </Show>
                </ul>
            </div>
            <div class="workbench-shortcut-wrap">
                <CreateWorkspaceHeroCard />
                <ul class="workbench-shortcut-list" aria-label="Main destinations">
                    <ShortcutActionRow icon=icondata::LuSparkles action=ShortcutAction::Agent />
                    <ShortcutActionRow icon=icondata::LuLayers action=ShortcutAction::Memory />
                    <ShortcutActionRow icon=icondata::LuGlobe action=ShortcutAction::Browser />
                    <ComingSoonShortcutCard icon=icondata::LuClipboardList label="Kanban" />
                </ul>
                <ul class="workbench-shortcut-utility-list" aria-label="Utility shortcuts">
                    <ShortcutUtilityRow icon=icondata::LuFolderSearch action=ShortcutAction::QuickOpen />
                    <ShortcutUtilityRow icon=icondata::LuFileSearch action=ShortcutAction::FindFile />
                    <ShortcutUtilityRow icon=icondata::LuPanelRight action=ShortcutAction::SidePanel />
                    <ShortcutUtilityRow icon=icondata::LuTerminal action=ShortcutAction::Terminal />
                    <ShortcutUtilityRow icon=icondata::LuCommand action=ShortcutAction::CommandPalette />
                </ul>
            </div>
        </div>
    }
}

/// Prominent, highlighted "Create Workspace" call-to-action above the
/// destinations row. Larger than the regular shortcut cards.
#[component]
fn CreateWorkspaceHeroCard() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let ui = expect_context::<HarnessUiService>();
    let wb = expect_context::<WorkbenchService>();
    let embed = expect_context::<BrowserEmbedSurface>();
    let action = ShortcutAction::CreateWorkspace;
    view! {
        <button
            type="button"
            class="workbench-create-hero"
            on:click=move |_| {
                if let Some(harness) = action.to_harness_action() {
                    dispatch_shortcut_action(harness, ui, wb, embed);
                }
            }
        >
            <span class="workbench-create-hero__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuFolderPlus width="1.35rem" height="1.35rem" />
            </span>
            <span class="workbench-create-hero__copy">
                <span class="workbench-create-hero__title">
                    {move || i18n.tr(I18nKey::WsKwCreateWorkspace)()}
                </span>
                <span class="workbench-create-hero__hint">
                    {move || i18n.tr(I18nKey::WsCreateWorkspaceHint)()}
                </span>
            </span>
            <span class="workbench-create-hero__keys">
                <kbd class="workbench-kbd">
                    {move || {
                        let cfg = prefs.shortcut_config().get();
                        let then = lookup(i18n.locale().get(), I18nKey::WsKwThen);
                        cfg.binding(action).display(&cfg.prefix, then)
                    }}
                </kbd>
            </span>
        </button>
    }
}

#[component]
fn ShortcutActionRow(icon: icondata::Icon, action: ShortcutAction) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let ui = expect_context::<HarnessUiService>();
    let wb = expect_context::<WorkbenchService>();
    let embed = expect_context::<BrowserEmbedSurface>();
    let label = action.label_key();
    view! {
        <li class="workbench-shortcut-li">
            <button
                type="button"
                class="workbench-shortcut-row workbench-shortcut-row--action"
                on:click=move |_| {
                    if let Some(harness) = action.to_harness_action() {
                        dispatch_shortcut_action(harness, ui, wb, embed);
                    }
                }
            >
                <span class="workbench-shortcut-row__lead">
                    <span class="workbench-shortcut-row__icon-wrap" aria-hidden="true">
                        <LxIcon icon=icon width="0.92rem" height="0.92rem" />
                    </span>
                    <span class="workbench-shortcut-row__label">{move || i18n.tr(label)()}</span>
                </span>
                <span class="workbench-shortcut-row__keys">
                    <kbd class="workbench-kbd">
                        {move || {
                            let cfg = prefs.shortcut_config().get();
                            let then = lookup(i18n.locale().get(), I18nKey::WsKwThen);
                            cfg.binding(action).display(&cfg.prefix, then)
                        }}
                    </kbd>
                </span>
            </button>
        </li>
    }
}

#[component]
fn ComingSoonShortcutCard(icon: icondata::Icon, label: &'static str) -> impl IntoView {
    view! {
        <li class="workbench-shortcut-li">
            <button
                type="button"
                class="workbench-shortcut-row workbench-shortcut-row--action workbench-shortcut-row--disabled"
                disabled
            >
                <span class="workbench-shortcut-row__lead">
                    <span class="workbench-shortcut-row__icon-wrap" aria-hidden="true">
                        <LxIcon icon=icon width="0.92rem" height="0.92rem" />
                    </span>
                    <span class="workbench-shortcut-row__label">{label}</span>
                </span>
                <span class="workbench-shortcut-row__soon">"coming soon"</span>
            </button>
        </li>
    }
}

#[component]
fn ShortcutUtilityRow(icon: icondata::Icon, action: ShortcutAction) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let ui = expect_context::<HarnessUiService>();
    let wb = expect_context::<WorkbenchService>();
    let embed = expect_context::<BrowserEmbedSurface>();
    let label = action.label_key();
    view! {
        <li class="workbench-shortcut-utility-li">
            <button
                type="button"
                class="workbench-shortcut-utility"
                on:click=move |_| {
                    if let Some(harness) = action.to_harness_action() {
                        dispatch_shortcut_action(harness, ui, wb, embed);
                    }
                }
            >
                <span class="workbench-shortcut-utility__lead">
                    <LxIcon icon=icon width="0.72rem" height="0.72rem" />
                    <span>{move || i18n.tr(label)()}</span>
                </span>
                <kbd class="workbench-kbd workbench-kbd--utility">
                    {move || {
                        let cfg = prefs.shortcut_config().get();
                        let then = lookup(i18n.locale().get(), I18nKey::WsKwThen);
                        cfg.binding(action).display(&cfg.prefix, then)
                    }}
                </kbd>
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
    slot_drag_enabled: Memo<bool>,
    is_workspace_active: Signal<bool>,
    hidden: Signal<bool>,
    is_full_size: Signal<bool>,
    on_full_size: Callback<(), ()>,
    canvas_mode: Signal<bool>,
    canvas_style: Signal<String>,
    canvas_port_active: Signal<Option<String>>,
    on_canvas_port: Callback<CanvasPortRef>,
    on_canvas_drag_start: Callback<MouseEvent>,
    on_canvas_resize_start: Callback<MouseEvent>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let slot_dnd = expect_context::<TerminalSlotDragService>();
    // Hydrate split layout from persisted workspace state so a restart
    // preserves the user's exact pane grid.
    let persisted = wb.slot_panes(workspace_id, slot_id);
    let pane_ids = RwSignal::new(persisted.pane_ids);
    let next_pane_id = RwSignal::new(persisted.next_pane_id);
    let split_axis = RwSignal::new(persisted.axis);

    // Per-slot drag-eligibility gate. Split panes can't be reordered
    // piecewise, but the slot chrome should still expose a grab handle for
    // every leaf terminal. The drop handlers decide whether a concrete target
    // is valid and surface the existing transfer error as a toast.
    let can_drag_slot = Memo::new(move |_| {
        if !slot_drag_enabled.get() || is_full_size.get() {
            return false;
        }
        pane_ids.with(|ids| ids.len() == 1)
    });
    // Memo: this slot is a *potential* drop target during a drag (any
    // other slot in the same workspace). Used for the dashed-accent
    // chrome on idle peers.
    let is_potential_target = Memo::new(move |_| {
        slot_dnd
            .active
            .get()
            .is_some_and(|m| m.workspace_id == workspace_id && m.slot_id != slot_id)
    });
    let is_drag_source = Memo::new(move |_| {
        slot_dnd
            .active
            .get()
            .is_some_and(|m| m.workspace_id == workspace_id && m.slot_id == slot_id)
    });
    let is_drop_over = Memo::new(move |_| {
        is_potential_target.get()
            && slot_dnd
                .ghost
                .get()
                .is_some_and(|g| g.target_slot_id == slot_id)
    });

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
                if is_drag_source.get() {
                    class.push_str(" ws-term-slot--drag-source");
                } else if is_potential_target.get() {
                    class.push_str(" ws-term-slot--drag-potential");
                }
                if is_drop_over.get() {
                    class.push_str(" ws-term-slot--drag-over");
                }
                if canvas_mode.get() {
                    class.push_str(" ws-term-slot--canvas-node");
                }
                class
            }
            style=move || {
                if canvas_mode.get() {
                    canvas_style.get()
                } else {
                    String::new()
                }
            }
            on:dragenter=move |ev| {
                if !slot_drag_enabled.get_untracked() {
                    return;
                }
                let Some(de) = ev.dyn_ref::<DragEvent>() else {
                    return;
                };
                // Register this slot as a drop target as early as possible.
                // Chromium/WebView2 (Windows) protects the DataTransfer during
                // dragenter/dragover — `getData` returns "" — so we key off our
                // own synchronous session flag rather than the payload. Without
                // an accepted dragenter, WebView2 never fires `drop`.
                if accepts_slot_drop(&slot_dnd, de, workspace_id, slot_id) {
                    de.prevent_default();
                }
            }
            on:dragover=move |ev| {
                if !slot_drag_enabled.get_untracked() {
                    return;
                }
                let Some(de) = ev.dyn_ref::<DragEvent>() else {
                    return;
                };
                if !accepts_slot_drop(&slot_dnd, de, workspace_id, slot_id) {
                    return;
                }
                de.prevent_default();
                if let Some(dt) = drag_event_data_transfer(de) {
                    let _ = dt.set_drop_effect("move");
                }
                slot_dnd.set_overlay_pos_from_event(de);
                let (rows, cols) = wb.workspaces().with_untracked(|list| {
                    list.iter()
                        .find(|w| w.id == workspace_id)
                        .map(|w| (w.grid_rows, w.grid_cols))
                        .unwrap_or((1, 1))
                });
                slot_dnd.ghost.set(Some(GhostPos {
                    target_slot_id: slot_id,
                    rows,
                    cols,
                }));
            }
            on:dragleave=move |ev| {
                if let Some(de) = ev.dyn_ref::<DragEvent>() {
                    if slot_dnd
                        .ghost
                        .get_untracked()
                        .is_some_and(|g| g.target_slot_id == slot_id)
                    {
                        slot_dnd.ghost.set(None);
                    }
                    de.prevent_default();
                }
            }
            on:drop=move |ev| {
                ev.prevent_default();
                ev.stop_propagation();
                if !slot_drag_enabled.get_untracked() {
                    slot_dnd.clear();
                    return;
                }
                let payload = ev
                    .dyn_ref::<DragEvent>()
                    .and_then(|de| de.data_transfer())
                    .and_then(|dt| read_drag_payload(&dt))
                    .or_else(|| {
                        slot_dnd.active.get().map(|meta| {
                            crate::workbench::terminal_slot_dnd::TerminalSlotDragPayload {
                                workspace_id: meta.workspace_id,
                                slot_id: meta.slot_id,
                            }
                        })
                    });
                if let Some(payload) = payload {
                    if payload.workspace_id == workspace_id && payload.slot_id != slot_id {
                        wb.swap_terminal_slots(workspace_id, payload.slot_id, slot_id);
                    }
                }
                slot_dnd.clear();
            }
        >
            <Show when=move || canvas_mode.get()>
                <div class="ws-canvas-node__drag" on:mousedown=move |ev| on_canvas_drag_start.run(ev)>
                    <LxIcon icon=icondata::LuGrip width="0.82rem" height="0.82rem" />
                    <span>{move || i18n.tr(I18nKey::CanvasSlotLabel)().replace("{id}", &slot_id.to_string())}</span>
                </div>
                    <CanvasPortButton
                    class_name="ws-canvas-port ws-canvas-port--stdin ws-canvas-port--slot"
                    label_key=I18nKey::CanvasPortStdin
                    active=canvas_port_active
                    port=CanvasPortRef {
                        node_kind: CanvasNodeKind::Terminal,
                        slot_id: Some(slot_id),
                        pane_id: None,
                        direction: CanvasPortDirection::Stdin,
                    }
                    on_port=on_canvas_port
                />
                <CanvasPortButton
                    class_name="ws-canvas-port ws-canvas-port--stdout ws-canvas-port--slot"
                    label_key=I18nKey::CanvasPortStdout
                    active=canvas_port_active
                    port=CanvasPortRef {
                        node_kind: CanvasNodeKind::Terminal,
                        slot_id: Some(slot_id),
                        pane_id: None,
                        direction: CanvasPortDirection::Stdout,
                    }
                    on_port=on_canvas_port
                />
                <button
                    type="button"
                    class="ws-canvas-node__resize"
                    aria-label=move || i18n.tr(I18nKey::CanvasResizeNode)()
                    title=move || i18n.tr(I18nKey::CanvasResizeNode)()
                    on:mousedown=move |ev| on_canvas_resize_start.run(ev)
                >
                    <LxIcon icon=icondata::LuGrip width="0.72rem" height="0.72rem" />
                </button>
            </Show>
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
                        let popout_key = terminal_key.clone();
                        let popout_label = Signal::derive(move || wb.terminal_popout_label(&popout_key));
                        let cwd_store = StoredValue::new(cwd.clone());
                        let agent_slug_store = StoredValue::new(agent_slug.clone());
                        let title_store = StoredValue::new(title.clone());
                        let terminal_key_store = StoredValue::new(terminal_key.clone());
                        view! {
                            <div class="ws-canvas-pane-wrap">
                                <Show when=move || canvas_mode.get() && pane_ids.with(|ids| ids.len() > 1)>
                                    <CanvasPortButton
                                        class_name="ws-canvas-port ws-canvas-port--stdin ws-canvas-port--pane"
                                        label_key=I18nKey::CanvasPortStdin
                                        active=canvas_port_active
                                        port=CanvasPortRef {
                                            node_kind: CanvasNodeKind::Terminal,
                                            slot_id: Some(slot_id),
                                            pane_id: Some(pane_id),
                                            direction: CanvasPortDirection::Stdin,
                                        }
                                        on_port=on_canvas_port
                                    />
                                    <CanvasPortButton
                                        class_name="ws-canvas-port ws-canvas-port--stdout ws-canvas-port--pane"
                                        label_key=I18nKey::CanvasPortStdout
                                        active=canvas_port_active
                                        port=CanvasPortRef {
                                            node_kind: CanvasNodeKind::Terminal,
                                            slot_id: Some(slot_id),
                                            pane_id: Some(pane_id),
                                            direction: CanvasPortDirection::Stdout,
                                        }
                                        on_port=on_canvas_port
                                    />
                                </Show>
                                <Show
                                    when=move || popout_label.get().is_none()
                                    fallback=move || {
                                        let label = popout_label.get().unwrap_or_default();
                                        view! {
                                            <div class="ws-term-cell ws-term-cell--popout-placeholder">
                                                <div class="ws-term-cell__head">
                                                    <span class="ws-term-cell__slot">{format!("#{slot_id}")}</span>
                                                    <span class="ws-term-cell__title">"Terminal popped out"</span>
                                                    <button
                                                        type="button"
                                                        class="ws-term-cell__tool"
                                                        on:click=move |_| {
                                                            let label = label.clone();
                                                            spawn_local(async move {
                                                                let _ = popout_focus(label).await;
                                                            });
                                                        }
                                                    >
                                                        <LxIcon icon=icondata::LuExternalLink width="0.82rem" height="0.82rem" />
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    }
                                >
                                    <WorkspaceTerminalCell
                                        workspace_id=workspace_id
                                        slot_id=slot_id
                                        pane_id=pane_id
                                        cwd=cwd_store.get_value()
                                        grid_index=index
                                        agent_slug=agent_slug_store.get_value()
                                        title=title_store.get_value()
                                        terminal_key=terminal_key_store.get_value()
                                        is_workspace_active=is_workspace_active
                                        is_slot_hidden=hidden
                                        is_full_size=is_full_size
                                        on_full_size=on_full_size
                                        on_split_vertical=on_split_vertical
                                        on_split_horizontal=on_split_horizontal
                                        on_close=on_close
                                        can_close=can_close
                                        slot_drag_enabled=Signal::derive(move || can_drag_slot.get())
                                    />
                                </Show>
                            </div>
                        }
                    }
                />
            </div>
            <Show when=move || is_drop_over.get()>
                <div class="ws-term-slot__drop-hint" aria-hidden="true">
                    <LxIcon icon=icondata::LuArrowLeftRight width="0.9rem" height="0.9rem" />
                    <span>{move || i18n.tr(I18nKey::WsTermDropHere)()}</span>
                </div>
            </Show>
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

#[component]
fn WorkspaceSwarmView(workspace_id: u64) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let roles = RwSignal::new(Vec::<SessionRoleView>::new());
    let selected_slot = RwSignal::new(None::<u64>);
    let drag_state = RwSignal::new(None::<SwarmDragState>);
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(list) = agent_session_roles_list().await {
                roles.set(list);
            }
        });
    });

    let workspace = Signal::derive(move || {
        wb.workspaces()
            .get()
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
    });
    let hub_role = Signal::derive(move || {
        let ws = workspace.get()?;
        let slug = ws.agent_session_role?;
        roles
            .get()
            .into_iter()
            .find(|role| role.slug == slug && role.terminal_agent_swarm)
    });

    let move_handle = window_event_listener_untyped("mousemove", {
        let wb = wb;
        move |ev| {
            let Some(drag) = drag_state.get_untracked() else {
                return;
            };
            let Some(ev) = ev.dyn_ref::<MouseEvent>() else {
                return;
            };
            ev.prevent_default();
            let x = drag.origin_x + ev.client_x() as f64 - drag.start_x;
            let y = drag.origin_y + ev.client_y() as f64 - drag.start_y;
            wb.set_swarm_node_position(workspace_id, drag.node_id, x, y);
        }
    });
    let up_handle = window_event_listener_untyped("mouseup", move |_| {
        drag_state.set(None);
    });
    on_cleanup(move || {
        move_handle.remove();
        up_handle.remove();
    });

    view! {
        <section class="workspace-swarm">
            <svg
                class="workspace-swarm__edges"
                viewBox="-640 -360 1280 720"
                preserveAspectRatio="xMidYMid meet"
                aria-hidden="true"
            >
                {move || {
                    let Some(ws) = workspace.get() else {
                        return Vec::<AnyView>::new();
                    };
                    let hub = swarm_node_position(&ws, "hub", (0.0, 0.0));
                    ws.slot_ids
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(idx, slot_id)| {
                            let fallback = swarm_node_point(idx, ws.slot_ids.len());
                            let node_id = swarm_slot_node_id(slot_id);
                            let (tx, ty) = swarm_node_position(&ws, &node_id, fallback);
                            let d = curved_path(hub.0, hub.1, tx, ty);
                            view! { <path class="workspace-swarm__edge" d=d></path> }.into_any()
                        })
                        .collect::<Vec<_>>()
                }}
            </svg>
            <div
                class="workspace-swarm__hub"
                style=move || {
                    let (x, y) = workspace
                        .get()
                        .map(|ws| swarm_node_position(&ws, "hub", (0.0, 0.0)))
                        .unwrap_or((0.0, 0.0));
                    let color = hub_role
                        .get()
                        .map(|role| role.color)
                        .filter(|color| !color.trim().is_empty())
                        .unwrap_or_else(|| "var(--accent)".into());
                    format!("--role-accent:{color};left:calc(50% + {x:.1}px);top:calc(43% + {y:.1}px);")
                }
                on:mousedown=move |ev| {
                    ev.prevent_default();
                    let (x, y) = workspace
                        .get_untracked()
                        .map(|ws| swarm_node_position(&ws, "hub", (0.0, 0.0)))
                        .unwrap_or((0.0, 0.0));
                    drag_state.set(Some(SwarmDragState {
                        node_id: "hub".into(),
                        start_x: ev.client_x() as f64,
                        start_y: ev.client_y() as f64,
                        origin_x: x,
                        origin_y: y,
                    }));
                }
            >
                <div class="workspace-swarm__hub-core">
                    <LxIcon icon=icondata::LuCrown width="1.55rem" height="1.55rem" />
                    <span class="workspace-swarm__led"></span>
                </div>
                <strong>{move || {
                    hub_role
                        .get()
                        .map(|r| r.title)
                        .unwrap_or_else(|| i18n.tr(I18nKey::SwarmAgentFallbackTitle)().into())
                }}</strong>
                <span>{move || {
                    if hub_role.get().is_some() {
                        i18n.tr(I18nKey::SwarmAgentEnabled)()
                    } else {
                        i18n.tr(I18nKey::SwarmDefaultRole)()
                    }
                }}</span>
            </div>
            <div class="workspace-swarm__nodes">
                {move || {
                    let Some(ws) = workspace.get() else {
                        return Vec::<AnyView>::new();
                    };
                    let running = wb.pty_sessions_for_workspace(workspace_id);
                    ws.slot_ids
                        .iter()
                        .copied()
                        .enumerate()
                        .map(|(idx, slot_id)| {
                            let agent = ws.slot_agent_labels.get(idx).cloned().unwrap_or_default();
                            let running_now = running.iter().any(|(slot, _, _)| *slot == slot_id);
                            let fallback = swarm_node_point(idx, ws.slot_ids.len());
                            let node_id = swarm_slot_node_id(slot_id);
                            let (x, y) = swarm_node_position(&ws, &node_id, fallback);
                            let label = if agent.trim().is_empty() {
                                i18n.tr(I18nKey::SwarmTerminalLabel)().replace("{id}", &slot_id.to_string())
                            } else {
                                format!("{} {slot_id}", title_case_ascii(&agent))
                            };
                            let status_key = if running_now {
                                I18nKey::SwarmStatusRunning
                            } else {
                                I18nKey::SwarmStatusIdle
                            };
                            view! {
                                <button
                                    type="button"
                                    class="workspace-swarm__node"
                                    class:workspace-swarm__node--running=move || running_now
                                    class:workspace-swarm__node--selected=move || selected_slot.get() == Some(slot_id)
                                    style=format!("left:calc(50% + {x:.1}px);top:calc(50% + {y:.1}px);")
                                    on:mousedown=move |ev| {
                                        ev.prevent_default();
                                        selected_slot.set(Some(slot_id));
                                        drag_state.set(Some(SwarmDragState {
                                            node_id: node_id.clone(),
                                            start_x: ev.client_x() as f64,
                                            start_y: ev.client_y() as f64,
                                            origin_x: x,
                                            origin_y: y,
                                        }));
                                    }
                                    on:click=move |_| selected_slot.set(Some(slot_id))
                                >
                                    <span class="workspace-swarm__node-icon">
                                        <LxIcon icon=icondata::LuHammer width="1.2rem" height="1.2rem" />
                                        <span class="workspace-swarm__led"></span>
                                    </span>
                                    <strong>{label}</strong>
                                    <span>{move || i18n.tr(status_key)()}</span>
                                </button>
                            }.into_any()
                        })
                        .collect::<Vec<_>>()
                }}
            </div>
            <SwarmNodePanel workspace_id=workspace_id selected_slot=selected_slot />
        </section>
    }
}

#[component]
fn SwarmNodePanel(workspace_id: u64, selected_slot: RwSignal<Option<u64>>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let preview = RwSignal::new(String::new());
    Effect::new(move |_| {
        let Some(slot_id) = selected_slot.get() else {
            preview.set(String::new());
            return;
        };
        let session = terminal_session_for_port(wb, workspace_id, slot_id, None);
        spawn_local(async move {
            let text = match session {
                Some(session) => pty_peek_output(session, 2048).await.unwrap_or_default(),
                None => String::new(),
            };
            preview.set(strip_ansi_for_preview(&text));
        });
    });

    view! {
        <aside class="workspace-swarm__inspector">
            <Show
                when=move || selected_slot.get().is_some()
                fallback=move || view! {
                    <span class="workspace-swarm__empty">{move || i18n.tr(I18nKey::SwarmSelectTerminalAgent)()}</span>
                }
            >
                {move || {
                    let slot_id = selected_slot.get().unwrap_or_default();
                    view! {
                        <header>
                            <span class="workspace-swarm__badge">{move || i18n.tr(I18nKey::SwarmAgentBadge)()}</span>
                            <strong>{move || i18n.tr(I18nKey::SwarmTerminalLabel)().replace("{id}", &slot_id.to_string())}</strong>
                        </header>
                        <pre>{move || preview.get()}</pre>
                        <footer>
                            <button type="button" on:click=move |_| wb.open_center_terminals_tab(workspace_id)>
                                <LxIcon icon=icondata::LuTerminal width="0.8rem" height="0.8rem" />
                                <span>{move || i18n.tr(I18nKey::SwarmOpenTerminal)()}</span>
                            </button>
                            <button
                                type="button"
                                class="workspace-swarm__danger"
                                on:click=move |_| {
                                    if let Some(session) = terminal_session_for_port(wb, workspace_id, slot_id, None) {
                                        spawn_local(async move {
                                            let b64 = base64::engine::general_purpose::STANDARD.encode([3_u8]);
                                            let _ = pty_write(session, b64).await;
                                        });
                                    }
                                }
                            >
                                <LxIcon icon=icondata::LuOctagonX width="0.8rem" height="0.8rem" />
                                <span>{move || i18n.tr(I18nKey::SwarmStop)()}</span>
                            </button>
                        </footer>
                    }
                }}
            </Show>
        </aside>
    }
}

fn swarm_node_point(index: usize, count: usize) -> (f64, f64) {
    let count = count.max(1);
    let angle =
        -std::f64::consts::FRAC_PI_2 + (index as f64 / count as f64) * std::f64::consts::TAU;
    let rx = 320.0;
    let ry = 185.0;
    (angle.cos() * rx, angle.sin() * ry)
}

fn swarm_slot_node_id(slot_id: u64) -> String {
    format!("slot:{slot_id}")
}

fn swarm_node_position(
    workspace: &WorkspaceEntry,
    node_id: &str,
    fallback: (f64, f64),
) -> (f64, f64) {
    workspace
        .swarm_view_state
        .node_positions
        .get(node_id)
        .map(|layout| (layout.x, layout.y))
        .unwrap_or(fallback)
}

fn title_case_ascii(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn strip_ansi_for_preview(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            if ch == '\n' || ch == '\r' || ch == '\t' || !ch.is_control() {
                output.push(ch);
            }
            continue;
        }

        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                let mut prev_was_escape = false;
                for next in chars.by_ref() {
                    if next == '\u{7}' || (prev_was_escape && next == '\\') {
                        break;
                    }
                    prev_was_escape = next == '\u{1b}';
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }

    output
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[component]
fn CanvasEdgesOverlay(workspace_id: u64) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let roles = RwSignal::new(Vec::<SessionRoleView>::new());
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(list) = agent_session_roles_list().await {
                roles.set(list);
            }
        });
    });

    let workspace = Signal::derive(move || {
        wb.workspaces()
            .get()
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
    });

    view! {
        <svg class="ws-canvas-edges" aria-hidden="true">
            {move || {
                let Some(ws) = workspace.get() else {
                    return Vec::<AnyView>::new();
                };
                let mut paths = Vec::<AnyView>::new();
                if canvas_role_allows_swarm(&roles.get(), ws.agent_session_role.as_deref())
                    && ws.canvas_view_state.show_agent_links
                {
                    for (idx, slot_id) in ws.slot_ids.iter().copied().enumerate() {
                        let layout = ws.canvas_view_state.terminal_nodes
                            .get(&slot_id)
                            .cloned()
                            .unwrap_or_else(|| CanvasNodeLayout::for_index(idx));
                        let (sx, sy) = (610.0, 136.0);
                        let (tx, ty) = (layout.x, layout.y + layout.height * 0.48);
                        let d = curved_path(sx, sy, tx, ty);
                        paths.push(view! {
                            <path class="ws-canvas-edge ws-canvas-edge--auto" d=d></path>
                        }.into_any());
                    }
                }
                for edge in ws.canvas_edges.iter() {
                    let Some((sx, sy)) = canvas_port_point(&ws, &edge.source) else {
                        continue;
                    };
                    let Some((tx, ty)) = canvas_port_point(&ws, &edge.target) else {
                        continue;
                    };
                    let class = match edge.transfer_mode {
                        CanvasTransferMode::Raw => "ws-canvas-edge ws-canvas-edge--raw",
                        CanvasTransferMode::Structured => "ws-canvas-edge ws-canvas-edge--structured",
                    };
                    let d = curved_path(sx, sy, tx, ty);
                    paths.push(view! { <path class=class d=d></path> }.into_any());
                }
                paths
            }}
        </svg>
        <div class="ws-canvas-edge-actions">
            {move || {
                workspace
                    .get()
                    .map(|ws| {
                        let edges = ws.canvas_edges.clone();
                        edges
                            .into_iter()
                            .filter_map(move |edge| {
                                let (sx, sy) = canvas_port_point(&ws, &edge.source)?;
                                let (tx, ty) = canvas_port_point(&ws, &edge.target)?;
                                let left = (sx + tx) / 2.0;
                                let top = (sy + ty) / 2.0;
                                let edge_send = edge.clone();
                                let edge_toggle = edge.id.clone();
                                let edge_remove = edge.id.clone();
                                let mode = match edge.transfer_mode {
                                    CanvasTransferMode::Raw => I18nKey::CanvasTransferRawShort,
                                    CanvasTransferMode::Structured => I18nKey::CanvasTransferStructuredShort,
                                };
                                Some(view! {
                                    <div class="ws-canvas-edge-chip" style=format!("left:{left:.1}px;top:{top:.1}px;")>
                                        <button
                                            type="button"
                                            title=move || i18n.tr(I18nKey::CanvasEdgeSend)()
                                            on:click=move |_| send_canvas_edge(wb, workspace_id, edge_send.clone())
                                        >
                                            <LxIcon icon=icondata::LuSendHorizontal width="0.72rem" height="0.72rem" />
                                        </button>
                                        <button
                                            type="button"
                                            title=move || i18n.tr(I18nKey::CanvasEdgeToggleTransfer)()
                                            on:click=move |_| wb.toggle_canvas_edge_transfer_mode(workspace_id, edge_toggle.clone())
                                        >
                                            {move || i18n.tr(mode)()}
                                        </button>
                                        <button
                                            type="button"
                                            title=move || i18n.tr(I18nKey::CanvasEdgeRemove)()
                                            on:click=move |_| wb.remove_canvas_edge(workspace_id, edge_remove.clone())
                                        >
                                            <LxIcon icon=icondata::LuX width="0.72rem" height="0.72rem" />
                                        </button>
                                    </div>
                                }.into_any())
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            }}
        </div>
    }
}

#[component]
fn CanvasAgentHub(
    workspace_id: u64,
    active_port: Signal<Option<String>>,
    on_port: Callback<CanvasPortRef>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let roles = RwSignal::new(Vec::<SessionRoleView>::new());
    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(list) = agent_session_roles_list().await {
                roles.set(list);
            }
        });
    });

    let role = Signal::derive(move || {
        let slug = wb
            .workspaces()
            .get()
            .into_iter()
            .find(|workspace| workspace.id == workspace_id)
            .and_then(|workspace| workspace.agent_session_role);
        let slug = slug?;
        roles
            .get()
            .into_iter()
            .find(|role| role.slug == slug && role.terminal_agent_swarm)
    });

    view! {
        <Show when=move || role.get().is_some()>
            {move || {
                let role = role.get().unwrap_or_else(|| SessionRoleView {
                    slug: "agent".into(),
                    title: i18n.tr(I18nKey::SwarmAgentFallbackTitle)().into(),
                    description: String::new(),
                    tools: Vec::new(),
                    skills: Vec::new(),
                    color: String::new(),
                    provider: String::new(),
                    models: Vec::new(),
                    terminal_agent_swarm: true,
                    enabled: true,
                });
                let color = if role.color.trim().is_empty() {
                    "var(--accent)".to_string()
                } else {
                    role.color.clone()
                };
                view! {
                    <section class="ws-canvas-agent-hub" style=format!("--role-accent:{color};")>
                        <CanvasPortButton
                            class_name="ws-canvas-port ws-canvas-port--agent-command"
                            label_key=I18nKey::CanvasPortCommand
                            active=active_port
                            port=CanvasPortRef {
                                node_kind: CanvasNodeKind::AgentHub,
                                slot_id: None,
                                pane_id: None,
                                direction: CanvasPortDirection::AgentCommand,
                            }
                            on_port=on_port
                        />
                        <CanvasPortButton
                            class_name="ws-canvas-port ws-canvas-port--agent-observe"
                            label_key=I18nKey::CanvasPortObserve
                            active=active_port
                            port=CanvasPortRef {
                                node_kind: CanvasNodeKind::AgentHub,
                                slot_id: None,
                                pane_id: None,
                                direction: CanvasPortDirection::AgentObserve,
                            }
                            on_port=on_port
                        />
                        <div class="ws-canvas-agent-hub__icon">
                            <LxIcon icon=icondata::LuCrown width="1.35rem" height="1.35rem" />
                        </div>
                        <div class="ws-canvas-agent-hub__text">
                            <strong>{role.title}</strong>
                            <span>{move || i18n.tr(I18nKey::CanvasAgentHubSubtitle)()}</span>
                        </div>
                        <div class="ws-canvas-agent-hub__actions">
                            <button
                                type="button"
                                title=move || i18n.tr(I18nKey::CanvasAgentHubList)()
                                on:click=move |_| wb.open_center_terminals_tab(workspace_id)
                            >
                                <LxIcon icon=icondata::LuList width="0.72rem" height="0.72rem" />
                                <span>{move || i18n.tr(I18nKey::CanvasAgentHubList)()}</span>
                            </button>
                            <button
                                type="button"
                                title=move || i18n.tr(I18nKey::CanvasAgentHubOpen)()
                                on:click=move |_| {
                                    let _ = wb.append_terminal_slot(workspace_id, None);
                                }
                            >
                                <LxIcon icon=icondata::LuPlus width="0.72rem" height="0.72rem" />
                                <span>{move || i18n.tr(I18nKey::CanvasAgentHubOpen)()}</span>
                            </button>
                            <button
                                type="button"
                                title=move || i18n.tr(I18nKey::CanvasAgentHubObserve)()
                                on:click=move |_| wb.open_center_swarm_tab(workspace_id)
                            >
                                <LxIcon icon=icondata::LuActivity width="0.72rem" height="0.72rem" />
                                <span>{move || i18n.tr(I18nKey::CanvasAgentHubObserve)()}</span>
                            </button>
                        </div>
                    </section>
                }
            }}
        </Show>
    }
}

#[component]
fn CanvasPortButton(
    class_name: &'static str,
    label_key: I18nKey,
    active: Signal<Option<String>>,
    port: CanvasPortRef,
    on_port: Callback<CanvasPortRef>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let active_label = canvas_port_identity(&port);
    view! {
        <button
            type="button"
            class=class_name
            class:ws-canvas-port--active=move || active.get().as_deref() == Some(active_label.as_str())
            title=move || i18n.tr(label_key)()
            aria-label=move || i18n.tr(label_key)()
            on:mousedown=|ev: MouseEvent| ev.stop_propagation()
            on:click=move |ev| {
                ev.prevent_default();
                ev.stop_propagation();
                on_port.run(port.clone());
            }
        >
            <span>{move || i18n.tr(label_key)()}</span>
        </button>
    }
}

fn canvas_terminal_style(workspace: &Option<WorkspaceEntry>, slot_id: u64, index: usize) -> String {
    let layout = workspace
        .as_ref()
        .and_then(|w| w.canvas_view_state.terminal_nodes.get(&slot_id).cloned())
        .unwrap_or_else(|| CanvasNodeLayout::for_index(index));
    format!(
        "position:absolute;left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;",
        layout.x, layout.y, layout.width, layout.height
    )
}

fn canvas_port_identity(port: &CanvasPortRef) -> String {
    let node = match port.node_kind {
        CanvasNodeKind::Terminal => "terminal",
        CanvasNodeKind::AgentHub => "agent",
    };
    let direction = match port.direction {
        CanvasPortDirection::Stdin => "stdin",
        CanvasPortDirection::Stdout => "stdout",
        CanvasPortDirection::AgentCommand => "command",
        CanvasPortDirection::AgentObserve => "observe",
    };
    match (port.slot_id, port.pane_id) {
        (Some(slot), Some(pane)) => format!("{node}:{slot}:{pane}:{direction}"),
        (Some(slot), None) => format!("{node}:{slot}:{direction}"),
        _ => format!("{node}:{direction}"),
    }
}

fn canvas_role_allows_swarm(roles: &[SessionRoleView], slug: Option<&str>) -> bool {
    let Some(slug) = slug else {
        return false;
    };
    roles
        .iter()
        .any(|role| role.slug == slug && role.terminal_agent_swarm)
}

fn canvas_port_point(workspace: &WorkspaceEntry, port: &CanvasPortRef) -> Option<(f64, f64)> {
    match (port.node_kind, port.direction) {
        (CanvasNodeKind::AgentHub, CanvasPortDirection::AgentCommand) => Some((650.0, 118.0)),
        (CanvasNodeKind::AgentHub, CanvasPortDirection::AgentObserve) => Some((522.0, 118.0)),
        (CanvasNodeKind::AgentHub, CanvasPortDirection::Stdin | CanvasPortDirection::Stdout) => {
            None
        }
        (CanvasNodeKind::Terminal, direction) => {
            let slot_id = port.slot_id?;
            let index = workspace
                .slot_ids
                .iter()
                .position(|id| *id == slot_id)
                .unwrap_or_default();
            let layout = workspace
                .canvas_view_state
                .terminal_nodes
                .get(&slot_id)
                .cloned()
                .unwrap_or_else(|| CanvasNodeLayout::for_index(index));
            match direction {
                CanvasPortDirection::Stdin => Some((layout.x, layout.y + layout.height * 0.5)),
                CanvasPortDirection::Stdout => {
                    Some((layout.x + layout.width, layout.y + layout.height * 0.5))
                }
                CanvasPortDirection::AgentCommand | CanvasPortDirection::AgentObserve => None,
            }
        }
    }
}

fn curved_path(sx: f64, sy: f64, tx: f64, ty: f64) -> String {
    let dx = (tx - sx).abs().max(120.0) * 0.45;
    let c1x = if tx >= sx { sx + dx } else { sx - dx };
    let c2x = if tx >= sx { tx - dx } else { tx + dx };
    format!("M {sx:.1} {sy:.1} C {c1x:.1} {sy:.1}, {c2x:.1} {ty:.1}, {tx:.1} {ty:.1}")
}

fn send_canvas_edge(
    wb: WorkbenchService,
    workspace_id: u64,
    edge: crate::workbench::state::CanvasEdge,
) {
    let Some(source_slot) = edge.source.slot_id else {
        return;
    };
    let Some(target_slot) = edge.target.slot_id else {
        return;
    };
    let source_pane = edge.source.pane_id;
    let target_pane = edge.target.pane_id;
    let source_session = terminal_session_for_port(wb, workspace_id, source_slot, source_pane);
    let target_session = terminal_session_for_port(wb, workspace_id, target_slot, target_pane);
    let Some(source_session) = source_session else {
        return;
    };
    let Some(target_session) = target_session else {
        return;
    };
    spawn_local(async move {
        let Ok(output) = pty_peek_output(source_session, 16 * 1024).await else {
            return;
        };
        let payload = match edge.transfer_mode {
            CanvasTransferMode::Raw => output,
            CanvasTransferMode::Structured => structured_canvas_payload(
                source_slot,
                source_pane,
                target_slot,
                target_pane,
                &output,
            ),
        };
        let b64 = base64::engine::general_purpose::STANDARD.encode(payload.as_bytes());
        let _ = pty_write(target_session, b64).await;
    });
}

fn terminal_session_for_port(
    wb: WorkbenchService,
    workspace_id: u64,
    slot_id: u64,
    pane_id: Option<u64>,
) -> Option<u64> {
    wb.pty_sessions_for_workspace(workspace_id)
        .into_iter()
        .find(|(slot, pane, _)| *slot == slot_id && pane_id.map_or(true, |id| *pane == id))
        .map(|(_, _, session)| session)
}

fn structured_canvas_payload(
    source_slot: u64,
    source_pane: Option<u64>,
    target_slot: u64,
    target_pane: Option<u64>,
    output: &str,
) -> String {
    let timestamp = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    format!(
        "⟪ BLXCode Canvas routed terminal context ⟫\n\n## Route\n- Source: slot {source_slot}{source_pane_label}\n- Target: slot {target_slot}{target_pane_label}\n- Transfer: structured stdout -> stdin\n- Timestamp: {timestamp}\n\n## Source output\n```text\n{output}\n```\n",
        source_pane_label = source_pane
            .map(|pane| format!(", pane {pane}"))
            .unwrap_or_default(),
        target_pane_label = target_pane
            .map(|pane| format!(", pane {pane}"))
            .unwrap_or_default(),
    )
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

#[cfg(test)]
mod swarm_preview_tests {
    use super::strip_ansi_for_preview;

    #[test]
    fn strips_terminal_escape_sequences_for_preview() {
        let raw = "\u{1b}[1mClaude\u{1b}[0m\r\n\u{1b}]0;title\u{7}ready\u{1b}[38;2;1;2;3m!";

        assert_eq!(strip_ansi_for_preview(raw), "Claude\nready!");
    }
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
