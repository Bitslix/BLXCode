//! Cursor-following preview for Workspace Kanban plan/task drags.

use crate::workbench::kanban_dnd::{KanbanDragKind, KanbanDragService};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

const PREVIEW_WIDTH_PX: f64 = 268.0;
const PREVIEW_HEIGHT_PX: f64 = 86.0;

#[component]
pub fn KanbanDragOverlay() -> impl IntoView {
    let kanban_dnd = expect_context::<KanbanDragService>();
    let visible = Memo::new(move |_| kanban_dnd.active.get().is_some());

    view! {
        <Show when=move || visible.get()>
            {move || {
                let Some(meta) = kanban_dnd.active.get() else {
                    return view! { <></> }.into_any();
                };
                let variant = match meta.kind {
                    KanbanDragKind::Plan => "workspace-kanban-drag-preview--plan",
                    KanbanDragKind::Task => "workspace-kanban-drag-preview--task",
                };
                let icon = match meta.kind {
                    KanbanDragKind::Plan => icondata::LuFolderKanban,
                    KanbanDragKind::Task => icondata::LuListTodo,
                };
                let title = meta.title.clone();
                let subtitle = meta.subtitle.clone();
                let badge = meta.badge.clone();
                view! {
                    <div
                        class=format!("workspace-kanban-drag-preview {variant}")
                        aria-hidden="true"
                        style=move || {
                            let (x, y) = kanban_dnd
                                .overlay_pos
                                .get()
                                .unwrap_or((-9999.0, -9999.0));
                            format!(
                                "left:{lx:.1}px;top:{ly:.1}px;width:{w:.0}px;min-height:{h:.0}px;",
                                lx = x - PREVIEW_WIDTH_PX / 2.0,
                                ly = y - PREVIEW_HEIGHT_PX / 2.0,
                                w = PREVIEW_WIDTH_PX,
                                h = PREVIEW_HEIGHT_PX,
                            )
                        }
                    >
                        <header class="workspace-kanban-drag-preview__head">
                            <span class="workspace-kanban-drag-preview__grip" aria-hidden="true">
                                <LxIcon icon=icondata::LuGripHorizontal width="0.9rem" height="0.9rem" />
                            </span>
                            <span class="workspace-kanban-drag-preview__title">{title}</span>
                            <span class="workspace-kanban-drag-preview__badge">{badge}</span>
                        </header>
                        <div class="workspace-kanban-drag-preview__body">
                            <LxIcon icon=icon width="1rem" height="1rem" />
                            <span>{subtitle}</span>
                        </div>
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}
