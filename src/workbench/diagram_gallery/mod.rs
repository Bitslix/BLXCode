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
    mermaid_delete_diagram, mermaid_export_markdown, mermaid_export_pdf, mermaid_list_diagrams,
    DiagramRecord, TimelineDiagram,
};
use crate::workbench::diagram_render::{rendered_svg_outer_html, DiagramRender};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// What a gallery tab shows.
#[derive(Debug, Clone, PartialEq)]
pub enum GalleryScope {
    /// Load diagrams from the store for this plan slug (deletable).
    Plan { slug: String },
    /// Render an ephemeral diagram group opened from the agent timeline. These
    /// are not backed by the store, so they cannot be deleted from here.
    Ephemeral {
        title: String,
        diagrams: Vec<TimelineDiagram>,
    },
}

/// One diagram normalised for display, independent of its source scope.
#[derive(Clone)]
struct GalleryItem {
    id: String,
    title: String,
    kind: String,
    code: String,
}

impl From<DiagramRecord> for GalleryItem {
    fn from(r: DiagramRecord) -> Self {
        Self {
            id: r.id,
            title: r.title,
            kind: r.kind,
            code: r.code,
        }
    }
}

impl From<TimelineDiagram> for GalleryItem {
    fn from(d: TimelineDiagram) -> Self {
        Self {
            id: d.id,
            title: d.title,
            kind: d.kind,
            code: d.code,
        }
    }
}

const STAGE_DOM_ID: &str = "diagram-gallery-active";

#[component]
pub fn DiagramGallery(scope: GalleryScope, workspace_id: u64) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();

    let diagrams = RwSignal::new(Vec::<GalleryItem>::new());
    let active = RwSignal::new(0usize);
    let loading = RwSignal::new(true);

    // The workspace cwd is captured up front so both the initial load (plan
    // scope) and the per-diagram delete action can reach the store.
    let cwd = wb.workspaces().with_untracked(|list| {
        list.iter()
            .find(|w| w.id == workspace_id)
            .map(|w| w.cwd.clone())
    });

    // Plan diagrams are deletable from the gallery; ephemeral ones are not.
    let plan_slug: Option<String> = match &scope {
        GalleryScope::Plan { slug } => Some(slug.clone()),
        GalleryScope::Ephemeral { .. } => None,
    };
    let allow_delete = plan_slug.is_some();

    // Populate the diagram set.
    match scope {
        GalleryScope::Plan { slug } => {
            if let Some(cwd) = cwd.clone() {
                spawn_local(async move {
                    match mermaid_list_diagrams(&cwd, &slug).await {
                        Ok(list) => diagrams.set(list.into_iter().map(GalleryItem::from).collect()),
                        Err(e) => web_sys::console::warn_1(&format!("load diagrams: {e}").into()),
                    }
                    loading.set(false);
                });
            } else {
                loading.set(false);
            }
        }
        GalleryScope::Ephemeral { diagrams: list, .. } => {
            diagrams.set(list.into_iter().map(GalleryItem::from).collect());
            loading.set(false);
        }
    }

    let active_code = Signal::derive(move || {
        diagrams.with(|d| {
            d.get(active.get())
                .map(|r| r.code.clone())
                .unwrap_or_default()
        })
    });
    let active_title = Signal::derive(move || {
        diagrams.with(|d| {
            d.get(active.get())
                .map(|r| r.title.clone())
                .unwrap_or_default()
        })
    });
    let active_kind = Signal::derive(move || {
        diagrams.with(|d| {
            d.get(active.get())
                .map(|r| r.kind.clone())
                .unwrap_or_default()
        })
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

    // Delete the active diagram from the store, then drop it from the local
    // list and clamp the active index. No-op when the cwd is unavailable.
    // `StoredValue` keeps these `Copy` so the handler stays `Fn` (the `<Show>`
    // children closure that hosts the button must be callable repeatedly).
    let toast_del = toast.clone();
    let del_slug = StoredValue::new(plan_slug.clone());
    let del_cwd = StoredValue::new(cwd.clone());
    let on_delete = move |_| {
        let (Some(cwd), Some(slug)) = (del_cwd.get_value(), del_slug.get_value()) else {
            return;
        };
        let idx = active.get_untracked();
        let Some(id) = diagrams.with_untracked(|d| d.get(idx).map(|r| r.id.clone())) else {
            return;
        };
        let toast = toast_del.clone();
        spawn_local(async move {
            match mermaid_delete_diagram(&cwd, &slug, &id).await {
                Ok(()) => {
                    diagrams.update(|d| {
                        if idx < d.len() {
                            d.remove(idx);
                        }
                    });
                    let len = diagrams.with_untracked(|d| d.len());
                    active.set(if len == 0 { 0 } else { idx.min(len - 1) });
                }
                Err(e) => toast.error(format!("Delete failed: {e}")),
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
                    <Show when=move || allow_delete>
                        <button
                            class="diagram-gallery__export diagram-gallery__delete"
                            on:click=on_delete
                        >
                            {move || i18n.tr(I18nKey::MemDelete)}
                        </button>
                    </Show>
                </div>
                <div class="diagram-gallery__active">
                    <DiagramRender code=active_code dom_id=STAGE_DOM_ID.to_string() />
                </div>
            </Show>
        </div>
    }
}
