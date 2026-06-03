//! Cursor-following preview card shown while a file / diff / commit is being
//! dragged from the sidebar toward the Agent panel's context drop zone.
//!
//! Mirrors [`crate::workbench::terminal_slot_drag_overlay`]: a fixed-position,
//! `pointer-events: none` card positioned from the `drag` event's
//! `client_x` / `client_y`. The icon, accent class and labels are derived from
//! the in-flight [`ContextDragKind`] so each type gets its own look — File,
//! Diff and Commit — matching the "cool" static overlay the terminals use.

use crate::workbench::context_drag::{ContextDragKind, ContextDragService};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

/// Width/height of the floating preview card in pixels — used to center it
/// under the cursor without a follow-up layout measurement.
const PREVIEW_WIDTH_PX: f64 = 256.0;
const PREVIEW_HEIGHT_PX: f64 = 84.0;

#[component]
pub fn ContextDragOverlay() -> impl IntoView {
    let context_dnd = expect_context::<ContextDragService>();

    let visible = Memo::new(move |_| context_dnd.active.get().is_some());

    view! {
        <Show when=move || visible.get()>
            {move || {
                let Some(meta) = context_dnd.active.get() else {
                    return view! { <></> }.into_any();
                };
                let (variant, label) = match meta.kind {
                    ContextDragKind::File => ("context-drag-preview--file", "File"),
                    ContextDragKind::Folder => ("context-drag-preview--folder", "Folder"),
                    ContextDragKind::Diff => ("context-drag-preview--diff", "Diff"),
                    ContextDragKind::Commit => ("context-drag-preview--commit", "Commit"),
                };
                let kind = meta.kind;
                let title = meta.title.clone();
                let subtitle = meta.subtitle.clone();
                view! {
                    <div
                        class=format!("context-drag-preview {variant}")
                        aria-hidden="true"
                        style=move || {
                            let (x, y) = context_dnd
                                .overlay_pos
                                .get()
                                .unwrap_or((-9999.0, -9999.0));
                            format!(
                                "left:{lx:.1}px;top:{ly:.1}px;width:{w:.0}px;",
                                lx = x - PREVIEW_WIDTH_PX / 2.0,
                                ly = y - PREVIEW_HEIGHT_PX / 2.0,
                                w = PREVIEW_WIDTH_PX,
                            )
                        }
                    >
                        <header class="context-drag-preview__head">
                            <span class="context-drag-preview__icon" aria-hidden="true">
                                <ContextDragIcon kind=kind />
                            </span>
                            <span class="context-drag-preview__title">{title}</span>
                            <span class="context-drag-preview__badge">{label}</span>
                        </header>
                        <div class="context-drag-preview__body">
                            <span class="context-drag-preview__subtitle">{subtitle}</span>
                        </div>
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}

#[component]
fn ContextDragIcon(kind: ContextDragKind) -> impl IntoView {
    match kind {
        ContextDragKind::File => view! {
            <LxIcon icon=icondata::LuFile width="0.95rem" height="0.95rem" />
        }
        .into_any(),
        ContextDragKind::Folder => view! {
            <LxIcon icon=icondata::LuFolder width="0.95rem" height="0.95rem" />
        }
        .into_any(),
        ContextDragKind::Diff => view! {
            <LxIcon icon=icondata::LuFileDiff width="0.95rem" height="0.95rem" />
        }
        .into_any(),
        ContextDragKind::Commit => view! {
            <LxIcon icon=icondata::LuGitCommitHorizontal width="0.95rem" height="0.95rem" />
        }
        .into_any(),
    }
}
