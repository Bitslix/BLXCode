use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::create_workspace_wizard::WorkspaceConfigurator;
use crate::workbench::state::{TerminalSplitAxis, WorkspaceEntry};
use crate::workbench::terminal_cell::WorkspaceTerminalCell;
use crate::workbench::WorkbenchService;
use leptos::callback::Callback;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

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
            <div
                class="ws-term-grid"
                style=move || {
                    let full = full_size_terminal.get().is_some();
                    let rows = if full { "minmax(0,1fr)".to_string() } else { fr_template(&row_fr.get()) };
                    let cols = if full { "minmax(0,1fr)".to_string() } else { fr_template(&col_fr.get()) };
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
                                    aria-label="Resize terminal columns"
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
                                    aria-label="Resize terminal rows"
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
            </Show>
        </div>
    }
}

#[component]
fn WorkspaceEmptyState() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="workbench-empty-editor">
            <p class="workbench-empty-editor__lead">{move || i18n.tr(I18nKey::WsEmptyLead)()}</p>
            <p class="workbench-empty-editor__note">{move || i18n.tr(I18nKey::WsEmptyNote)()}</p>
            <ul class="workbench-shortcut-list">
                <ShortcutRow icon=icondata::LuFolderSearch label=I18nKey::WsKwQuickOpen keys=vec!["Ctrl", "O"] />
                <ShortcutRow icon=icondata::LuPanelRight label=I18nKey::WsKwSidePanel keys=vec!["Ctrl", "P"] />
                <li class="workbench-shortcut-row workbench-shortcut-row--spacer" aria-hidden="true"></li>
                <ShortcutRow icon=icondata::LuSparkles label=I18nKey::WsKwAgent keys=vec!["Ctrl", "Shift", "A"] />
                <ShortcutRow icon=icondata::LuGlobe label=I18nKey::WsKwBrowser keys=vec!["Ctrl", "Shift", "B"] />
                <ShortcutRow icon=icondata::LuLayers label=I18nKey::WsKwMemory keys=vec!["Ctrl", "Shift", "M"] />
                <li class="workbench-shortcut-row workbench-shortcut-row--spacer" aria-hidden="true"></li>
                <ShortcutRow icon=icondata::LuTerminal label=I18nKey::WsKwTerminal keys=vec!["Ctrl", "`"] />
                <ShortcutRow icon=icondata::LuCommand label=I18nKey::WsKwCmdPalette keys=vec!["Ctrl", "Shift", "P"] />
            </ul>
        </div>
    }
}

#[component]
fn TerminalSlotSurface(
    workspace_id: u64,
    slot_id: u64,
    index: usize,
    cwd: String,
    agent_slug: String,
    hidden: Signal<bool>,
    is_full_size: Signal<bool>,
    on_full_size: Callback<(), ()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
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
                    each=move || pane_ids.get()
                    key=|pane_id| *pane_id
                    children=move |pane_id| {
                        let pane_index = pane_ids
                            .get_untracked()
                            .iter()
                            .position(|id| *id == pane_id)
                            .unwrap_or_default();
                        // Initial title shown until the agent's title hook
                        // (UserPromptSubmit / BeforeAgent / beforeSubmitPrompt
                        // / chat.message — depending on the agent) emits an
                        // OSC-2 sequence that overrides it. We label by the
                        // selected agent slug so the slot is identifiable
                        // *before* hooks fire (or when hooks aren't installed
                        // yet for that agent).
                        let agent_label = match agent_slug.trim() {
                            "" => "Terminal",
                            "claude" => "Claude",
                            "codex" => "Codex",
                            "gemini" => "Gemini",
                            "opencode" => "OpenCode",
                            "cursor" => "Cursor",
                            other => other,
                        };
                        let title = if pane_ids.get_untracked().len() <= 1 {
                            format!("{agent_label} · Terminal {}", index + 1)
                        } else {
                            format!(
                                "{agent_label} · Terminal {}.{}",
                                index + 1,
                                pane_index + 1
                            )
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

                        let terminal_key = format!("{workspace_id}:{slot_id}:{pane_id}");
                        view! {
                            <WorkspaceTerminalCell
                                cwd=cwd.clone()
                                grid_index=index
                                agent_slug=agent_slug.clone()
                                title=title
                                terminal_key=terminal_key
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

#[component]
fn ShortcutRow(icon: icondata::Icon, label: I18nKey, keys: Vec<&'static str>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <li class="workbench-shortcut-row">
            <span class="workbench-shortcut-row__lead">
                <span class="workbench-shortcut-row__icon-wrap" aria-hidden="true">
                    <LxIcon icon=icon width="0.9rem" height="0.9rem" />
                </span>
                <span class="workbench-shortcut-row__label">{move || i18n.tr(label)()}</span>
            </span>
            <span class="workbench-shortcut-row__keys">
                {keys.into_iter()
                    .enumerate()
                    .map(|(i, key)| view! {
                        <Show when=move || { i > 0 }>
                            <span class="workbench-kbd-gap">"+"</span>
                        </Show>
                        <kbd class="workbench-kbd">{key}</kbd>
                    })
                    .collect_view()}
            </span>
        </li>
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

fn grid_col_handle_style(index: usize, values: &[f64]) -> String {
    format!("left:{:.4}%;", grid_handle_offset(index, values))
}

fn grid_row_handle_style(index: usize, values: &[f64]) -> String {
    format!("top:{:.4}%;", grid_handle_offset(index, values))
}
