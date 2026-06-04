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
    mermaid_update_diagram, DiagramRecord, TimelineDiagram,
};
use crate::workbench::diagram_render::{
    diagram_first_seen, rendered_svg_outer_html, MermaidPreviewWithInspector,
};
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
    saved_code: String,
    /// Generation time (epoch ms). Stored diagrams carry it; ephemeral ones fall
    /// back to the session first-seen registry (resolved at display time).
    created_ms: Option<f64>,
    /// Provider/model that generated the diagram (when recorded).
    provider: Option<String>,
    model: Option<String>,
}

impl GalleryItem {
    /// `provider · model` label for the stats panel, or `None` when neither was
    /// recorded (e.g. diagrams created before this was tracked).
    fn model_label(&self) -> Option<String> {
        match (self.provider.as_deref(), self.model.as_deref()) {
            (Some(p), Some(m)) => Some(format!("{p} · {m}")),
            (Some(p), None) => Some(p.to_string()),
            (None, Some(m)) => Some(m.to_string()),
            (None, None) => None,
        }
    }
}

impl From<DiagramRecord> for GalleryItem {
    fn from(r: DiagramRecord) -> Self {
        Self {
            id: r.id,
            title: r.title,
            kind: r.kind,
            saved_code: r.code.clone(),
            code: r.code,
            created_ms: Some(r.created_ms as f64),
            provider: r.provider,
            model: r.model,
        }
    }
}

impl From<TimelineDiagram> for GalleryItem {
    fn from(d: TimelineDiagram) -> Self {
        Self {
            id: d.id,
            title: d.title,
            kind: d.kind,
            saved_code: d.code.clone(),
            code: d.code,
            created_ms: None,
            provider: d.provider,
            model: d.model,
        }
    }
}

/// Format an epoch-ms timestamp as a compact local `YYYY-MM-DD HH:MM`.
fn fmt_epoch_ms(ms: f64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date(),
        d.get_hours(),
        d.get_minutes(),
    )
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
    let source = RwSignal::new(String::new());
    let inspector_open = RwSignal::new(false);

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
    let allow_save = plan_slug.is_some();

    // Populate the diagram set.
    match scope {
        GalleryScope::Plan { slug } => {
            if let Some(cwd) = cwd.clone() {
                spawn_local(async move {
                    match mermaid_list_diagrams(&cwd, &slug).await {
                        Ok(list) => {
                            let items: Vec<GalleryItem> =
                                list.into_iter().map(GalleryItem::from).collect();
                            source.set(items.first().map(|d| d.code.clone()).unwrap_or_default());
                            diagrams.set(items);
                        }
                        Err(e) => web_sys::console::warn_1(&format!("load diagrams: {e}").into()),
                    }
                    loading.set(false);
                });
            } else {
                loading.set(false);
            }
        }
        GalleryScope::Ephemeral { diagrams: list, .. } => {
            let items: Vec<GalleryItem> = list.into_iter().map(GalleryItem::from).collect();
            source.set(items.first().map(|d| d.code.clone()).unwrap_or_default());
            diagrams.set(items);
            loading.set(false);
        }
    }

    Effect::new(move |_| {
        let text = source.get();
        let idx = active.get_untracked();
        diagrams.update(|d| {
            if let Some(item) = d.get_mut(idx) {
                item.code = text;
            }
        });
    });
    let active_code = Signal::derive(move || source.get());
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

    // --- Stats panel data -------------------------------------------------
    // Active diagram's generation time: stored timestamp, else the session
    // first-seen registry (inline timeline render), else unknown.
    let active_gen_time = Signal::derive(move || {
        diagrams.with(|d| {
            d.get(active.get()).and_then(|r| {
                r.created_ms
                    .or_else(|| diagram_first_seen(&r.id))
                    .map(fmt_epoch_ms)
            })
        })
    });
    let diagram_count = Signal::derive(move || diagrams.with(Vec::len));
    let active_pos = Signal::derive(move || active.get() + 1);

    // Provider/model recorded on the active diagram (None for ones generated
    // before this was tracked).
    let active_model_label = Signal::derive(move || {
        diagrams.with(|d| d.get(active.get()).and_then(GalleryItem::model_label))
    });
    let can_revert = Signal::derive(move || {
        diagrams.with(|d| {
            d.get(active.get())
                .map(|r| r.code != r.saved_code)
                .unwrap_or(false)
        })
    });
    let can_save = Signal::derive(move || allow_save && can_revert.get());

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
                Err(e) => toast.error(format!(
                    "{} {e}",
                    i18n.tr(I18nKey::AgentDiagramExportFailed)()
                )),
            }
        });
    };

    let toast_pdf = toast.clone();
    let on_export_pdf = move |_| {
        let title = active_title.get_untracked();
        let toast = toast_pdf.clone();
        let Some(svg) = rendered_svg_outer_html(STAGE_DOM_ID) else {
            toast.error(i18n.tr(I18nKey::AgentDiagramNotRenderedYet)().to_string());
            return;
        };
        spawn_local(async move {
            match mermaid_export_pdf(&title, &svg).await {
                Ok(Some(path)) => toast.success(format!("Saved {path}")),
                Ok(None) => {}
                Err(e) => toast.error(format!(
                    "{} {e}",
                    i18n.tr(I18nKey::AgentDiagramExportFailed)()
                )),
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
                    let next_idx = if len == 0 { 0 } else { idx.min(len - 1) };
                    active.set(next_idx);
                    source.set(
                        diagrams
                            .with_untracked(|d| d.get(next_idx).map(|r| r.code.clone()))
                            .unwrap_or_default(),
                    );
                }
                Err(e) => toast.error(format!(
                    "{} {e}",
                    i18n.tr(I18nKey::DiagramGalleryDeleteFailed)()
                )),
            }
        });
    };
    let toast_save = toast.clone();
    let on_save = Callback::new(move |()| {
        let (Some(cwd), Some(slug)) = (del_cwd.get_value(), del_slug.get_value()) else {
            return;
        };
        let idx = active.get_untracked();
        let Some((id, code)) =
            diagrams.with_untracked(|d| d.get(idx).map(|r| (r.id.clone(), r.code.clone())))
        else {
            return;
        };
        let toast = toast_save.clone();
        spawn_local(async move {
            match mermaid_update_diagram(&cwd, &slug, &id, &code).await {
                Ok(updated) => {
                    diagrams.update(|items| {
                        if let Some(item) = items.get_mut(idx) {
                            item.title = updated.title;
                            item.kind = updated.kind;
                            item.code = updated.code.clone();
                            item.saved_code = updated.code;
                            item.created_ms = Some(updated.created_ms as f64);
                            item.provider = updated.provider;
                            item.model = updated.model;
                        }
                    });
                    toast.success(i18n.tr(I18nKey::FilePreviewEditorSaved)().to_string());
                }
                Err(e) => toast.error(
                    i18n.tr(I18nKey::FilePreviewEditorSaveError)()
                        .replace("{detail}", &e),
                ),
            }
        });
    });
    let on_revert = Callback::new(move |()| {
        let idx = active.get_untracked();
        let text = diagrams
            .with_untracked(|d| d.get(idx).map(|r| r.saved_code.clone()))
            .unwrap_or_default();
        source.set(text);
    });

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
                                        on:click=move |_| {
                                            active.set(i);
                                            source.set(
                                                diagrams
                                                    .with_untracked(|items| {
                                                        items.get(i).map(|r| r.code.clone())
                                                    })
                                                    .unwrap_or_default(),
                                            );
                                        }
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
                    <button
                        class="diagram-gallery__export"
                        on:click=move |_| inspector_open.update(|open| *open = !*open)
                    >
                        {move || i18n.tr(I18nKey::FilePreviewEditorEdit)}
                    </button>
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
                    <MermaidPreviewWithInspector
                        source=source
                        dom_id=STAGE_DOM_ID.to_string()
                        inspector_open=inspector_open
                        can_save=can_save
                        can_revert=can_revert
                        on_save=on_save
                        on_revert=on_revert
                        allow_save=allow_save
                    >
                        <div class="diagram-gallery__stats">
                            <div class="diagram-gallery__stat">
                                <span class="diagram-gallery__stat-key">
                                    {move || i18n.tr(I18nKey::PlansOpenDiagrams)}
                                </span>
                                <span class="diagram-gallery__stat-val">
                                    {move || format!("{} / {}", active_pos.get(), diagram_count.get())}
                                </span>
                            </div>
                            <Show when=move || !active_kind.get().is_empty()>
                                <div class="diagram-gallery__stat">
                                    <span class="diagram-gallery__stat-key">
                                        {move || i18n.tr(I18nKey::DiagramStatType)}
                                    </span>
                                    <span class="diagram-gallery__stat-val">{move || active_kind.get()}</span>
                                </div>
                            </Show>
                            <Show when=move || active_gen_time.get().is_some()>
                                <div class="diagram-gallery__stat">
                                    <span class="diagram-gallery__stat-key">
                                        {move || i18n.tr(I18nKey::DiagramStatGenerated)}
                                    </span>
                                    <span class="diagram-gallery__stat-val">
                                        {move || active_gen_time.get().unwrap_or_default()}
                                    </span>
                                </div>
                            </Show>
                            <Show when=move || active_model_label.get().is_some()>
                                <div class="diagram-gallery__stat">
                                    <span class="diagram-gallery__stat-key">
                                        {move || i18n.tr(I18nKey::DiagramStatModel)}
                                    </span>
                                    <span class="diagram-gallery__stat-val">
                                        {move || active_model_label.get().unwrap_or_default()}
                                    </span>
                                </div>
                            </Show>
                        </div>
                    </MermaidPreviewWithInspector>
                </div>
            </Show>
        </div>
    }
}
