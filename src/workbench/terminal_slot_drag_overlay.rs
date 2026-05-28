//! Cursor-following preview card shown while a terminal slot is being
//! dragged. Mirrors EB's `TerminalSlotPreview` rendered inside `<DragOverlay>`:
//! a fixed-position, pointer-events: none card with the slot title +
//! agent badge that floats under the cursor independently of the grid.
//!
//! We deliberately do not use HTML5 [`DataTransfer::set_drag_image`]
//! because Tauri / WebKitGTK aborts `dragstart` if the image isn't ready
//! synchronously. A normal DOM element positioned from the `drag`
//! event's `client_x` / `client_y` works on every platform with no
//! native-ghost API.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::terminal_slot_dnd::TerminalSlotDragService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

/// Width of the floating preview card in pixels — used to center it
/// under the cursor without a follow-up layout measurement.
const PREVIEW_WIDTH_PX: f64 = 256.0;
const PREVIEW_HEIGHT_PX: f64 = 96.0;

#[component]
pub fn TerminalSlotDragOverlay() -> impl IntoView {
    let slot_dnd = expect_context::<TerminalSlotDragService>();
    let i18n = expect_context::<I18nService>();

    let visible = Memo::new(move |_| slot_dnd.active.get().is_some());

    view! {
        <Show when=move || visible.get()>
            {move || {
                let Some(meta) = slot_dnd.active.get() else {
                    return view! { <></> }.into_any();
                };
                let title = meta.title.clone();
                let agent_label = meta.agent_label.clone();
                view! {
                    <div
                        class="ws-term-drag-preview"
                        aria-hidden="true"
                        style=move || {
                            let (x, y) = slot_dnd.overlay_pos.get().unwrap_or((-9999.0, -9999.0));
                            // Anchor below-and-right of the cursor so the
                            // preview never sits on top of the source slot.
                            format!(
                                "left:{lx:.1}px;top:{ly:.1}px;width:{w:.0}px;height:{h:.0}px;",
                                lx = x - PREVIEW_WIDTH_PX / 2.0,
                                ly = y - PREVIEW_HEIGHT_PX / 2.0,
                                w = PREVIEW_WIDTH_PX,
                                h = PREVIEW_HEIGHT_PX,
                            )
                        }
                    >
                        <header class="ws-term-drag-preview__head">
                            <span class="ws-term-drag-preview__grip" aria-hidden="true">
                                <LxIcon icon=icondata::LuGripHorizontal width="0.9rem" height="0.9rem" />
                            </span>
                            <span class="ws-term-drag-preview__title">{title}</span>
                            <Show when={
                                let agent = agent_label.clone();
                                move || !agent.is_empty()
                            }>
                                <span class="ws-term-drag-preview__badge">{agent_label.clone()}</span>
                            </Show>
                        </header>
                        <div class="ws-term-drag-preview__body">
                            <LxIcon icon=icondata::LuTerminal width="1rem" height="1rem" />
                            <span>{move || i18n.tr(I18nKey::WsTermDragging)()}</span>
                        </div>
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}
