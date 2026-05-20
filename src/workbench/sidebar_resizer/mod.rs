//! Vertical drag handle that resizes the Project Explorer slot within the sidebar.
//!
//! Sits between two stacked sidebar sections (explorer above, graph below).
//! Dragging it adjusts a percent value, clamped to a project-wide range.

use crate::config::{SIDEBAR_EXPLORER_HEIGHT_PCT_MAX, SIDEBAR_EXPLORER_HEIGHT_PCT_MIN};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent};

#[component]
pub fn SidebarResizer(
    /// Current slot height as percent of the sidebar `__views` container.
    height_pct: RwSignal<f64>,
    /// CSS selector used to look up the container whose height is the 100% basis.
    container_selector: &'static str,
) -> impl IntoView {
    let dragging = RwSignal::new(false);

    let on_pointer_down = move |ev: PointerEvent| {
        ev.prevent_default();
        dragging.set(true);
        // Capture pointer so we keep receiving move events outside the handle.
        if let Some(target) = ev.target().and_then(|t| t.dyn_into::<Element>().ok()) {
            let _ = target.set_pointer_capture(ev.pointer_id());
        }
    };

    // Global move/up listeners run only while dragging.
    let move_listener = window_event_listener_untyped("pointermove", move |ev| {
        if !dragging.get_untracked() {
            return;
        }
        let Some(pe) = ev.dyn_ref::<PointerEvent>() else {
            return;
        };
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let Some(container) = doc.query_selector(container_selector).ok().flatten() else {
            return;
        };
        let rect = container.get_bounding_client_rect();
        let height = rect.height();
        if height <= 0.0 {
            return;
        }
        let offset = f64::from(pe.client_y()) - rect.top();
        let pct = (offset / height) * 100.0;
        let clamped = pct
            .max(SIDEBAR_EXPLORER_HEIGHT_PCT_MIN)
            .min(SIDEBAR_EXPLORER_HEIGHT_PCT_MAX);
        height_pct.set(clamped);
    });

    let up_listener = window_event_listener_untyped("pointerup", move |_| {
        if dragging.get_untracked() {
            dragging.set(false);
        }
    });

    on_cleanup(move || {
        move_listener.remove();
        up_listener.remove();
    });

    view! {
        <div
            class=move || {
                let mut c = String::from("workbench-sidebar__resizer");
                if dragging.get() {
                    c.push_str(" workbench-sidebar__resizer--active");
                }
                c
            }
            role="separator"
            aria-orientation="horizontal"
            aria-label="Resize project files panel"
            on:pointerdown=on_pointer_down
        >
            <span class="workbench-sidebar__resizer-grip" aria-hidden="true"></span>
        </div>
    }
}
