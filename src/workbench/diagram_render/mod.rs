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
use crate::workbench::file_preview::mermaid_glue::run_mermaid_on;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

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
    let node_ref: NodeRef<html::Div> = NodeRef::new();
    let render_err = RwSignal::new(false);

    Effect::new(move |_| {
        let text = code.get();
        let Some(el) = node_ref.get() else {
            return;
        };
        let element: HtmlElement = el.unchecked_into();
        element.set_inner_html("");
        render_err.set(false);
        if text.trim().is_empty() {
            return;
        }
        let Some(target) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.create_element("pre").ok())
        else {
            return;
        };
        let _ = target.set_attribute("class", "mermaid diagram-render__node");
        target.set_text_content(Some(&text));
        let _ = element.append_child(&target);
        let target_el: HtmlElement = target.unchecked_into();
        let nodes = vec![target_el];
        spawn_local(async move {
            if let Err(e) = run_mermaid_on(&nodes).await {
                web_sys::console::warn_1(&format!("mermaid render: {e}").into());
                render_err.set(true);
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
