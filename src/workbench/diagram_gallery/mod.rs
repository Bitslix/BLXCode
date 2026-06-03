//! Centered diagram gallery tab.
//!
//! Layout: a horizontal thumbnail slider on top, the active diagram rendered
//! large below. Used for plan-linked diagram sets (loaded from the store by
//! `plan_slug`) and for multi-diagram groups opened from the agent timeline.
//!
//! Export buttons (`.md` / `.pdf`) act on the active diagram via the
//! `mermaid_export_*` Tauri commands (native Save dialog). The PDF path reads
//! the rendered SVG back out of the active render node.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    mermaid_export_markdown, mermaid_export_pdf, mermaid_list_diagrams, DiagramRecord,
};
use crate::workbench::diagram_render::{rendered_svg_outer_html, DiagramRender};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// What a gallery tab shows: diagrams persisted under a plan, or an ad-hoc set
/// passed inline (e.g. from a timeline tool result).
#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScope {
    /// Load diagrams from the store for this plan slug.
    Plan { slug: String },
    /// Render the supplied diagrams directly (not necessarily persisted).
    #[allow(dead_code)] // alternate scope; construction site pending
    Inline { diagrams: Vec<DiagramRecord> },
}

const STAGE_DOM_ID: &str = "diagram-gallery-active";

#[component]
pub fn DiagramGallery(scope: GalleryScope, workspace_id: u64) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();

    let diagrams = RwSignal::new(Vec::<DiagramRecord>::new());
    let active = RwSignal::new(0usize);
    let loading = RwSignal::new(matches!(scope, GalleryScope::Plan { .. }));

    // Populate the diagram set.
    match scope.clone() {
        GalleryScope::Inline { diagrams: list } => diagrams.set(list),
        GalleryScope::Plan { slug } => {
            let cwd = wb.workspaces().with_untracked(|list| {
                list.iter().find(|w| w.id == workspace_id).map(|w| w.cwd.clone())
            });
            if let Some(cwd) = cwd {
                spawn_local(async move {
                    match mermaid_list_diagrams(&cwd, &slug).await {
                        Ok(list) => diagrams.set(list),
                        Err(e) => web_sys::console::warn_1(&format!("load diagrams: {e}").into()),
                    }
                    loading.set(false);
                });
            } else {
                loading.set(false);
            }
        }
    }

    let active_code = Signal::derive(move || {
        diagrams.with(|d| d.get(active.get()).map(|r| r.code.clone()).unwrap_or_default())
    });
    let active_title = Signal::derive(move || {
        diagrams.with(|d| d.get(active.get()).map(|r| r.title.clone()).unwrap_or_default())
    });
    let active_kind = Signal::derive(move || {
        diagrams.with(|d| d.get(active.get()).map(|r| r.kind.clone()).unwrap_or_default())
    });

    let toast_md = toast.clone();
    let on_export_md = move |_| {
        let title = active_title.get_untracked();
        let kind = active_kind.get_untracked();
        let code = active_code.get_untracked();
        let toast = toast_md.clone();
        spawn_local(async move {
            // Orientation: portrait by default; SVG-derived flip happens for PDF.
            match mermaid_export_markdown(&title, &kind, &code, false).await {
                Ok(Some(path)) => toast.success(format!("Saved {path}")),
                Ok(None) => {}
                Err(e) => toast.error(format!("Export failed: {e}")),
            }
        });
    };

    let toast_pdf = toast.clone();
    let on_export_pdf = move |_| {
        let title = active_title.get_untracked();
        let toast = toast_pdf.clone();
        let Some(svg) = rendered_svg_outer_html(STAGE_DOM_ID) else {
            toast.error("Diagram not rendered yet".to_string());
            return;
        };
        spawn_local(async move {
            match mermaid_export_pdf(&title, &svg).await {
                Ok(Some(path)) => toast.success(format!("Saved {path}")),
                Ok(None) => {}
                Err(e) => toast.error(format!("Export failed: {e}")),
            }
        });
    };

    view! {
        <div class="diagram-gallery">
            <Show
                when=move || !diagrams.with(|d| d.is_empty())
                fallback=move || view! {
                    <div class="diagram-gallery__empty">
                        {move || if loading.get() {
                            i18n.tr(I18nKey::FilePreviewLoading)
                        } else {
                            i18n.tr(I18nKey::DiagramGalleryEmpty)
                        }}
                    </div>
                }
            >
                <div class="diagram-gallery__slider" role="tablist">
                    {move || {
                        diagrams
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(i, d)| {
                                let is_active = move || active.get() == i;
                                let title = d.title.clone();
                                let kind = d.kind.clone();
                                view! {
                                    <button
                                        class="diagram-gallery__thumb"
                                        class:diagram-gallery__thumb--active=is_active
                                        on:click=move |_| active.set(i)
                                        title=d.title.clone()
                                    >
                                        <span class="diagram-gallery__thumb-title">{title}</span>
                                        <span class="diagram-gallery__thumb-kind">{kind}</span>
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </div>
                <div class="diagram-gallery__toolbar">
                    <span class="diagram-gallery__active-title">{move || active_title.get()}</span>
                    <span class="diagram-gallery__spacer" />
                    <button class="diagram-gallery__export" on:click=on_export_md>
                        {move || i18n.tr(I18nKey::DiagramExportMd)}
                    </button>
                    <button class="diagram-gallery__export" on:click=on_export_pdf>
                        {move || i18n.tr(I18nKey::DiagramExportPdf)}
                    </button>
                </div>
                <div class="diagram-gallery__active">
                    <DiagramRender code=active_code dom_id=STAGE_DOM_ID.to_string() />
                </div>
            </Show>
        </div>
    }
}
