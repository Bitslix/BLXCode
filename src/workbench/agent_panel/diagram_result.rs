//! Inline diagram rendering for `mermaid_create` / `mermaid_create_many` tool
//! results in the agent timeline.
//!
//! The backend returns a `{ "diagrams": [...] }` envelope (see
//! `src-tauri/src/agent/mermaid/tool.rs`). Instead of dumping that JSON as raw
//! detail text, this component renders each diagram inline via
//! [`DiagramRender`] and offers per-group actions: open the whole group in the
//! centered gallery, and export the active diagram as `.md` / `.pdf`.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    mermaid_export_markdown, mermaid_export_pdf, parse_timeline_diagrams, TimelineDiagram,
};
use crate::workbench::diagram_render::{mark_diagram_seen, rendered_svg_outer_html, DiagramRender};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// Stable DOM id for one rendered diagram node, derived from the tool row's
/// detail key and the diagram index. Sanitised so it is a valid `id`.
fn diagram_dom_id(detail_key: &str, idx: usize) -> String {
    let safe: String = detail_key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("tl-diag-{safe}-{idx}")
}

/// Renders the diagrams carried in a `mermaid_create*` tool result, or `None`
/// when `detail` is not a diagrams envelope (caller falls back to raw detail).
pub fn diagram_result_view(
    detail: &str,
    detail_key: &str,
    wb: WorkbenchService,
    workspace_id: Option<u64>,
) -> Option<AnyView> {
    let diagrams = parse_timeline_diagrams(detail)?;
    Some(
        view! {
            <DiagramResultCards
                diagrams=diagrams
                detail_key=detail_key.to_string()
                wb=wb
                workspace_id=workspace_id
            />
        }
        .into_any(),
    )
}

#[component]
fn DiagramResultCards(
    diagrams: Vec<TimelineDiagram>,
    detail_key: String,
    wb: WorkbenchService,
    workspace_id: Option<u64>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();

    // Group title for the "open in gallery" tab: the first diagram's title.
    let group_title = diagrams
        .first()
        .map(|d| d.title.clone())
        .unwrap_or_default();
    let group = StoredValue::new(diagrams.clone());

    let cards = diagrams
        .into_iter()
        .enumerate()
        .map(|(idx, d)| {
            // Stamp generation time (first inline render ≈ tool-result arrival)
            // so the centered gallery can show it in the stats panel.
            mark_diagram_seen(&d.id);
            let dom_id = diagram_dom_id(&detail_key, idx);
            let code = RwSignal::new(d.code.clone());
            let title = d.title.clone();
            let kind = d.kind.clone();
            let has_kind = !kind.is_empty();

            // Export .md — straight from the diagram source.
            let toast_md = toast;
            let md_title = d.title.clone();
            let md_kind = d.kind.clone();
            let md_code = d.code.clone();
            let on_export_md = move |_| {
                let (title, kind, code) = (md_title.clone(), md_kind.clone(), md_code.clone());
                let toast = toast_md;
                spawn_local(async move {
                    match mermaid_export_markdown(&title, &kind, &code, false).await {
                        Ok(Some(path)) => toast.success(format!("Saved {path}")),
                        Ok(None) => {}
                        Err(e) => toast.error(format!("Export failed: {e}")),
                    }
                });
            };

            // Export .pdf — needs the rendered SVG read back from this card's node.
            let toast_pdf = toast;
            let pdf_title = d.title.clone();
            let pdf_dom_id = dom_id.clone();
            let on_export_pdf = move |_| {
                let toast = toast_pdf;
                let Some(svg) = rendered_svg_outer_html(&pdf_dom_id) else {
                    toast.error("Diagram not rendered yet".to_string());
                    return;
                };
                let title = pdf_title.clone();
                spawn_local(async move {
                    match mermaid_export_pdf(&title, &svg).await {
                        Ok(Some(path)) => toast.success(format!("Saved {path}")),
                        Ok(None) => {}
                        Err(e) => toast.error(format!("Export failed: {e}")),
                    }
                });
            };

            // Open the whole group in the centered gallery.
            let wb_open = wb;
            let open_title = group_title.clone();
            let on_open = move |_| {
                if let Some(ws_id) = workspace_id {
                    wb_open.open_center_diagram_group(
                        ws_id,
                        open_title.clone(),
                        group.get_value(),
                    );
                }
            };

            view! {
                <div class="diagram-result-card">
                    <div class="diagram-result-card__head">
                        <span class="diagram-result-card__title">{title}</span>
                        <Show when=move || has_kind>
                            <span class="diagram-result-card__kind">{kind.clone()}</span>
                        </Show>
                    </div>
                    <DiagramRender code=code dom_id=dom_id />
                    <div class="diagram-result-card__actions">
                        <Show when=move || workspace_id.is_some()>
                            <button
                                type="button"
                                class="diagram-result-card__btn"
                                on:click=on_open.clone()
                            >
                                {move || i18n.tr(I18nKey::PlansOpenDiagrams)}
                            </button>
                        </Show>
                        <button
                            type="button"
                            class="diagram-result-card__btn"
                            on:click=on_export_md
                        >
                            {move || i18n.tr(I18nKey::DiagramExportMd)}
                        </button>
                        <button
                            type="button"
                            class="diagram-result-card__btn"
                            on:click=on_export_pdf
                        >
                            {move || i18n.tr(I18nKey::DiagramExportPdf)}
                        </button>
                    </div>
                </div>
            }
        })
        .collect_view();

    view! { <div class="diagram-result">{cards}</div> }
}
