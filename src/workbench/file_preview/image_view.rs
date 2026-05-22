//! Image preview renderer (raster + inline SVG).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, read_workspace_image_file, read_workspace_text_file, BinaryFilePreview,
};
use crate::workbench::file_preview::util::{format_bytes, sanitize_svg};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, PartialEq)]
enum ImagePayload {
    Raster { data_url: String },
    Svg { sanitized: String },
}

#[component]
pub fn ImageView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let payload = RwSignal::new(None::<Result<ImagePayload, String>>);
    let too_large_size = RwSignal::new(None::<u64>);

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        let _ = reload_tick.get();
        payload.set(None);
        too_large_size.set(None);
        if !is_tauri_shell() {
            payload.set(Some(Err(
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
            payload.set(Some(Err("Workspace not found.".into())));
            return;
        };
        let root = ws.cwd;
        let rel = rel_for_effect.clone();
        let is_svg = rel.to_ascii_lowercase().ends_with(".svg");
        spawn_local(async move {
            if is_svg {
                match read_workspace_text_file(root, rel).await {
                    Ok(text) => payload.set(Some(Ok(ImagePayload::Svg {
                        sanitized: sanitize_svg(&text.content),
                    }))),
                    Err(e) => payload.set(Some(Err(e))),
                }
            } else {
                match read_workspace_image_file(root, rel).await {
                    Ok(BinaryFilePreview {
                        base64,
                        mime,
                        byte_len,
                        truncated,
                    }) => {
                        if truncated {
                            too_large_size.set(Some(byte_len));
                            payload.set(Some(Err(String::new())));
                        } else {
                            payload.set(Some(Ok(ImagePayload::Raster {
                                data_url: format!("data:{mime};base64,{base64}"),
                            })));
                        }
                    }
                    Err(e) => payload.set(Some(Err(e))),
                }
            }
        });
    });

    view! {
        <div class="file-preview__stage file-preview__stage--image">
            {move || match payload.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => {
                    if let Some(size) = too_large_size.get() {
                        let msg = i18n
                            .tr(I18nKey::FilePreviewTooLarge)()
                            .replace("{size}", &format_bytes(size));
                        view! {
                            <div class="file-preview__status file-preview__status--error">{msg}</div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="file-preview__status file-preview__status--error">{err}</div>
                        }.into_any()
                    }
                }
                Some(Ok(ImagePayload::Raster { data_url })) => view! {
                    <img class="file-preview__image" src=data_url alt="" />
                }.into_any(),
                Some(Ok(ImagePayload::Svg { sanitized })) => view! {
                    <div class="file-preview__image-svg" inner_html=sanitized />
                }.into_any(),
            }}
        </div>
    }
}
