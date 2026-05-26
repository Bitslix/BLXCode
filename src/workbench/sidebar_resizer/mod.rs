//! Vertical drag handle that resizes stacked sidebar sections.
//!
//! Sits between two stacked blocks (workspaces ↔ panels, or explorer ↔ graph).
//! Dragging adjusts a percent value, clamped to a project-wide range.

use crate::config::{
    SIDEBAR_DIFF_HEIGHT_PCT_MAX, SIDEBAR_DIFF_HEIGHT_PCT_MIN, SIDEBAR_EXPLORER_HEIGHT_PCT_MAX,
    SIDEBAR_EXPLORER_HEIGHT_PCT_MIN, SIDEBAR_PANELS_HEIGHT_PCT_MAX, SIDEBAR_PANELS_HEIGHT_PCT_MIN,
};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent};

#[derive(Clone, Copy, Default)]
pub enum SidebarResizerClamp {
    #[default]
    ExplorerInPanels,
    DiffInPanels,
    PanelsInSidebar,
}

impl SidebarResizerClamp {
    fn min_max(self) -> (f64, f64) {
        match self {
            Self::ExplorerInPanels => (
                SIDEBAR_EXPLORER_HEIGHT_PCT_MIN,
                SIDEBAR_EXPLORER_HEIGHT_PCT_MAX,
            ),
            Self::DiffInPanels => (SIDEBAR_DIFF_HEIGHT_PCT_MIN, SIDEBAR_DIFF_HEIGHT_PCT_MAX),
            Self::PanelsInSidebar => (SIDEBAR_PANELS_HEIGHT_PCT_MIN, SIDEBAR_PANELS_HEIGHT_PCT_MAX),
        }
    }
}

#[component]
pub fn SidebarResizer(
    /// Current slot height as percent of the container given by `container_selector`.
    height_pct: RwSignal<f64>,
    /// CSS selector used to look up the container whose height is the 100% basis.
    container_selector: &'static str,
    /// When true, `height_pct` is the portion from the pointer down to the container bottom.
    #[prop(default = false)]
    measure_from_bottom: bool,
    #[prop(default = SidebarResizerClamp::ExplorerInPanels)] clamp: SidebarResizerClamp,
    aria_key: I18nKey,
    #[prop(default = "")] extra_class: &'static str,
    /// Subtract this percentage from the raw Y-from-top value before storing.
    /// Used when the resizer sits below another fixed-size slot (e.g. diff resizer
    /// must subtract explorer_pct so `height_pct` stores the section height, not
    /// the absolute boundary position).
    #[prop(default = None)]
    subtract_pct: Option<Signal<f64>>,
    /// Dynamic ceiling that overrides the static clamp max — used to reserve space
    /// for sections below (e.g. ensure the graph slot never collapses to zero).
    #[prop(default = None)]
    clamp_max: Option<Signal<f64>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let dragging = RwSignal::new(false);

    let on_pointer_down = move |ev: PointerEvent| {
        ev.prevent_default();
        dragging.set(true);
        if let Some(target) = ev.target().and_then(|t| t.dyn_into::<Element>().ok()) {
            let _ = target.set_pointer_capture(ev.pointer_id());
        }
    };

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
        let raw_pct = if measure_from_bottom {
            ((height - offset) / height) * 100.0
        } else {
            (offset / height) * 100.0
        };
        let pct = match subtract_pct {
            Some(sub) => raw_pct - sub.get_untracked(),
            None => raw_pct,
        };
        let (min_pct, static_max) = clamp.min_max();
        let max_pct = match clamp_max {
            Some(s) => s.get_untracked().min(static_max),
            None => static_max,
        };
        height_pct.set(pct.max(min_pct).min(max_pct));
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
                if !extra_class.is_empty() {
                    c.push(' ');
                    c.push_str(extra_class);
                }
                if dragging.get() {
                    c.push_str(" workbench-sidebar__resizer--active");
                }
                c
            }
            role="separator"
            aria-orientation="horizontal"
            aria-label=move || i18n.tr(aria_key)()
            on:pointerdown=on_pointer_down
        >
            <span class="workbench-sidebar__resizer-grip" aria-hidden="true"></span>
        </div>
    }
}
