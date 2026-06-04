//! Mermaid file (`.mmd` / `.mermaid`) preview renderer.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, read_workspace_text_file};
use crate::workbench::file_preview::util::{render_load_error, FilePreviewError};
use crate::workbench::diagram_render::InteractiveDiagramViewport;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;

#[component]
pub fn MermaidView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let source = RwSignal::new(None::<Result<String, FilePreviewError>>);
    let dom_id = Uuid::new_v4().to_string().replace('-', "");

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        // Only refetch on explicit reload. See FilePreviewDock for context.
        let _ = reload_tick.get();
        source.set(None);
        if !is_tauri_shell() {
            source.set(Some(Err(FilePreviewError::NoTauri)));
            return;
        }
        let Some((root, conn)) = wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| (w.cwd.clone(), w.remote_connection_id.clone()))
        }) else {
            source.set(Some(Err(FilePreviewError::WorkspaceNotFound)));
            return;
        };
        let rel = rel_for_effect.clone();
        spawn_local(async move {
            match read_workspace_text_file(root, rel, conn).await {
                Ok(t) => source.set(Some(Ok(t.content))),
                Err(e) => source.set(Some(Err(FilePreviewError::Failed(e)))),
            }
        });
    });

    let render_code = Signal::derive(move || {
        source.with(|s| match s {
            Some(Ok(text)) => text.clone(),
            _ => String::new(),
        })
    });

    view! {
        <div class="file-preview__stage file-preview__stage--mermaid">
            {move || match source.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => render_load_error(i18n, I18nKey::FilePreviewLoadFailedMermaid, err),
                Some(Ok(_)) => {
                    let id_attr = dom_id.clone();
                    view! {
                        <InteractiveDiagramViewport code=render_code dom_id=id_attr compact=true />
                    }.into_any()
                }
            }}
        </div>
    }
}
