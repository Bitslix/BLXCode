//! Reusable inline Mermaid renderer.
//!
//! Unlike [`crate::workbench::file_preview::mermaid_view::MermaidView`], which
//! reads a workspace file, this component renders Mermaid source passed
//! directly as a prop. It is shared by the diagram cards in the agent timeline
//! and the centered diagram gallery.
//!
//! The rendered `<svg>` is written into a node carrying the supplied `dom_id`,
//! so callers can read it back out (e.g. for PDF export) via
//! [`rendered_svg_outer_html`].

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::file_preview::mermaid_glue::render_mermaid_to_svg;
use crate::workbench::theme_service::ThemeService;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

// Process-wide counter for unique Mermaid render ids (the library requires a
// unique element id per `render` call).
thread_local! {
    static RENDER_SEQ: Cell<u64> = const { Cell::new(0) };
}

fn next_render_id() -> String {
    RENDER_SEQ.with(|c| {
        let n = c.get().wrapping_add(1);
        c.set(n);
        format!("mmd-render-{n}")
    })
}

thread_local! {
    /// Caches successfully rendered diagram markup keyed by `(theme, code)`.
    ///
    /// The timeline tree is rebuilt on every agent update (no keyed `<For>`),
    /// so an inline `DiagramRender` re-mounts repeatedly. Mermaid renders
    /// asynchronously, so without a cache each re-mount restarts the render and
    /// the just-produced SVG is discarded before it is visible. Re-injecting the
    /// cached SVG synchronously on mount makes inline diagrams appear instantly
    /// and stay put. Keyed by theme so a theme switch forces a re-render.
    static SVG_CACHE: RefCell<HashMap<(String, String), String>> = RefCell::new(HashMap::new());
}

thread_local! {
    /// Records the wall-clock time (epoch ms) a diagram id was first rendered.
    /// Inline timeline cards render as soon as the `mermaid_create` tool result
    /// arrives, so this is a good proxy for the diagram's generation time and is
    /// shown in the centered gallery's stats. Session-scoped (not persisted).
    static FIRST_SEEN: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
}

/// Stamp `id` as seen now if not already recorded; returns the recorded time.
pub fn mark_diagram_seen(id: &str) -> f64 {
    FIRST_SEEN.with(|c| {
        *c.borrow_mut()
            .entry(id.to_owned())
            .or_insert_with(js_sys::Date::now)
    })
}

/// The first-seen time (epoch ms) for `id`, if it has been rendered this session.
pub fn diagram_first_seen(id: &str) -> Option<f64> {
    FIRST_SEEN.with(|c| c.borrow().get(id).copied())
}

fn cache_get(theme: &str, code: &str) -> Option<String> {
    SVG_CACHE.with(|c| c.borrow().get(&(theme.to_owned(), code.to_owned())).cloned())
}

fn cache_put(theme: &str, code: &str, html: String) {
    SVG_CACHE.with(|c| {
        let mut map = c.borrow_mut();
        // Bound the cache so long sessions don't grow it without limit.
        if map.len() > 256 {
            map.clear();
        }
        map.insert((theme.to_owned(), code.to_owned()), html);
    });
}

/// Read back the rendered SVG markup from a diagram node by its `dom_id`.
/// Returns `None` until Mermaid has finished rendering.
pub fn rendered_svg_outer_html(dom_id: &str) -> Option<String> {
    let svg = web_sys::window()?
        .document()?
        .get_element_by_id(dom_id)?
        .query_selector("svg")
        .ok()
        .flatten()?;
    svg.dyn_ref::<web_sys::Element>().map(|e| e.outer_html())
}

#[component]
pub fn DiagramRender(
    /// Raw Mermaid source.
    #[prop(into)]
    code: Signal<String>,
    /// Stable DOM id of the render container (used to read back the SVG).
    dom_id: String,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let theme = expect_context::<ThemeService>();
    let node_ref: NodeRef<html::Div> = NodeRef::new();
    let render_err = RwSignal::new(false);

    Effect::new(move |_| {
        let text = code.get();
        // Subscribe to theme changes so the diagram re-renders with the active
        // theme's tokens (multi-theme support, see rule-theme-tokens.md).
        let theme_id = theme.active_theme_id().get();
        let Some(el) = node_ref.get() else {
            return;
        };
        let element: HtmlElement = el.unchecked_into();
        render_err.set(false);
        if text.trim().is_empty() {
            element.set_inner_html("");
            return;
        }
        // Fast path: a previously rendered SVG for this theme+code is injected
        // synchronously, so a re-mount shows the diagram immediately instead of
        // racing an async render whose result could be discarded.
        if let Some(svg) = cache_get(&theme_id, &text) {
            element.set_inner_html(&svg);
            return;
        }
        element.set_inner_html("");
        let id = next_render_id();
        spawn_local(async move {
            match render_mermaid_to_svg(&id, &text).await {
                Ok(svg) => {
                    // The SVG is fully self-contained (Mermaid measured it in its
                    // own offscreen container), so caching it is always valid —
                    // even if this node was discarded by a timeline rebuild.
                    cache_put(&theme_id, &text, svg.clone());
                    if let Some(el) = node_ref.get_untracked() {
                        let el: HtmlElement = el.unchecked_into();
                        el.set_inner_html(&svg);
                    }
                }
                Err(e) => {
                    web_sys::console::warn_1(&format!("mermaid render: {e}").into());
                    render_err.set(true);
                }
            }
        });
    });

    view! {
        <div class="diagram-render">
            <div node_ref=node_ref id=dom_id class="diagram-render__stage" />
            <Show when=move || render_err.get()>
                <div class="diagram-render__error">
                    {move || i18n.tr(I18nKey::FilePreviewMermaidError)}
                </div>
            </Show>
        </div>
    }
}
