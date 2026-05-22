//! Mermaid file (`.mmd` / `.mermaid`) preview renderer.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, read_workspace_text_file};
use crate::workbench::file_preview::mermaid_glue::run_mermaid_on;
use crate::workbench::WorkbenchService;
use leptos::html;
use leptos::prelude::*;
use leptos::task::spawn_local;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

#[component]
pub fn MermaidView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let source = RwSignal::new(None::<Result<String, String>>);
    let render_err = RwSignal::new(None::<String>);
    let node_ref: NodeRef<html::Div> = NodeRef::new();
    let dom_id = Uuid::new_v4().to_string().replace('-', "");

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        let _ = reload_tick.get();
        source.set(None);
        render_err.set(None);
        if !is_tauri_shell() {
            source.set(Some(Err(
                "File preview is available in the desktop app.".into()
            )));
            return;
        }
        let Some(ws) = wb
            .workspaces()
            .get()
            .into_iter()
            .find(|w| w.id == workspace_id)
        else {
            source.set(Some(Err("Workspace not found.".into())));
            return;
        };
        let root = ws.cwd;
        let rel = rel_for_effect.clone();
        spawn_local(async move {
            match read_workspace_text_file(root, rel).await {
                Ok(t) => source.set(Some(Ok(t.content))),
                Err(e) => source.set(Some(Err(e))),
            }
        });
    });

    // Reactively (re-)render Mermaid whenever source content changes.
    Effect::new(move |_| {
        let Some(Ok(text)) = source.get() else {
            return;
        };
        let Some(el) = node_ref.get() else {
            return;
        };
        let element: HtmlElement = el.unchecked_into();
        element.set_inner_html("");
        let target = match web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.create_element("pre").ok())
        {
            Some(e) => e,
            None => return,
        };
        let _ = target.set_attribute("class", "mermaid file-preview__mermaid-node");
        target.set_text_content(Some(&text));
        let _ = element.append_child(&target);
        let target_el: HtmlElement = target.unchecked_into();
        let nodes = vec![target_el];
        spawn_local(async move {
            if let Err(e) = run_mermaid_on(&nodes).await {
                render_err.set(Some(e));
            }
        });
    });

    view! {
        <div class="file-preview__stage file-preview__stage--mermaid">
            {move || match source.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => view! {
                    <div class="file-preview__status file-preview__status--error">{err}</div>
                }.into_any(),
                Some(Ok(_)) => {
                    let id_attr = dom_id.clone();
                    view! {
                        <div node_ref=node_ref id=id_attr class="file-preview__mermaid-stage" />
                        {move || render_err.get().map(|e| view! {
                            <div class="file-preview__notice file-preview__notice--error">
                                {i18n.tr(I18nKey::FilePreviewMermaidError)}" "{e}
                            </div>
                        })}
                    }.into_any()
                }
            }}
        </div>
    }
}
