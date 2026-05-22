//! Video preview renderer (base64 data URL into a native `<video>`).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, read_workspace_video_file, BinaryFilePreview};
use crate::workbench::file_preview::util::{render_load_error, FilePreviewError};
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[derive(Clone, PartialEq)]
struct VideoSrc {
    data_url: String,
    mime: String,
}

#[component]
pub fn VideoView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let video = RwSignal::new(None::<Result<VideoSrc, FilePreviewError>>);

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        // Only refetch on explicit reload. See FilePreviewDock for context.
        let _ = reload_tick.get();
        video.set(None);
        if !is_tauri_shell() {
            video.set(Some(Err(FilePreviewError::NoTauri)));
            return;
        }
        let Some(root) = wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.cwd.clone())
        }) else {
            video.set(Some(Err(FilePreviewError::WorkspaceNotFound)));
            return;
        };
        let rel = rel_for_effect.clone();
        spawn_local(async move {
            match read_workspace_video_file(root, rel).await {
                Ok(BinaryFilePreview {
                    base64,
                    mime,
                    byte_len,
                    truncated,
                }) => {
                    if truncated {
                        video.set(Some(Err(FilePreviewError::TooLarge(byte_len))));
                    } else {
                        let data_url = format!("data:{mime};base64,{base64}");
                        video.set(Some(Ok(VideoSrc { data_url, mime })));
                    }
                }
                Err(e) => video.set(Some(Err(FilePreviewError::Failed(e)))),
            }
        });
    });

    view! {
        <div class="file-preview__stage file-preview__stage--video">
            {move || match video.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => render_load_error(i18n, I18nKey::FilePreviewLoadFailedVideo, err),
                Some(Ok(src)) => view! {
                    <video
                        class="file-preview__video"
                        controls=true
                        preload="metadata"
                    >
                        <source src=src.data_url.clone() type=src.mime.clone() />
                    </video>
                }.into_any(),
            }}
        </div>
    }
}
