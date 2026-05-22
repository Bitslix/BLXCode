//! Topbar shown above every file-preview renderer (name, path, size, mtime).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::FileMeta;
use crate::workbench::file_preview::util::{format_bytes, format_mtime, icon_for_kind};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn FilePreviewHeader(
    meta: Memo<Option<FileMeta>>,
    rel_path: String,
    on_refresh: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let copied = RwSignal::new(false);

    let copy_path = {
        let rel_path = rel_path.clone();
        move |_| {
            let text = rel_path.clone();
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = clipboard.write_text(&text);
                copied.set(true);
                leptos::task::spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(1400).await;
                    copied.set(false);
                });
            }
        }
    };

    let display_name_path = rel_path.clone();
    let display_name = move || {
        meta.get()
            .map(|m| m.name)
            .unwrap_or_else(|| display_name_path.clone())
    };
    let size_label = move || meta.get().map(|m| format_bytes(m.byte_len));
    let mtime_label = move || meta.get().and_then(|m| format_mtime(m.modified_ms));
    let kind_icon = Signal::derive(move || {
        meta.get()
            .map(|m| icon_for_kind(m.kind))
            .unwrap_or(icondata::LuFile)
    });

    let path_title = rel_path.clone();
    let path_text = rel_path.clone();

    view! {
        <header class="file-preview__header">
            <div class="file-preview__title-block">
                <div class="file-preview__title">
                    <span class="file-preview__icon" aria-hidden="true">
                        <LxIcon icon=kind_icon width="1rem" height="1rem" />
                    </span>
                    <span class="file-preview__name">{display_name}</span>
                </div>
                <div class="file-preview__meta">
                    <span class="file-preview__path" title=path_title>{path_text}</span>
                    {move || size_label().map(|s| view! {
                        <span class="file-preview__meta-chip">
                            <span class="file-preview__meta-label">{i18n.tr(I18nKey::FilePreviewSize)}":"</span>
                            <span class="file-preview__meta-value">{s}</span>
                        </span>
                    })}
                    {move || mtime_label().map(|m| view! {
                        <span class="file-preview__meta-chip">
                            <span class="file-preview__meta-label">{i18n.tr(I18nKey::FilePreviewModified)}":"</span>
                            <span class="file-preview__meta-value">{m}</span>
                        </span>
                    })}
                </div>
            </div>
            <div class="file-preview__actions">
                <button
                    type="button"
                    class="workbench-mini-btn"
                    title=move || i18n.tr(I18nKey::FilePreviewCopyPath)()
                    aria-label=move || i18n.tr(I18nKey::FilePreviewCopyPath)()
                    on:click=copy_path
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuClipboard width="0.78rem" height="0.78rem" />
                        <span>
                            {move || if copied.get() {
                                i18n.tr(I18nKey::FilePreviewPathCopied)().to_string()
                            } else {
                                i18n.tr(I18nKey::FilePreviewCopyPath)().to_string()
                            }}
                        </span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    title=move || i18n.tr(I18nKey::FilePreviewRefresh)()
                    aria-label=move || i18n.tr(I18nKey::FilePreviewRefresh)()
                    on:click=move |_| on_refresh.run(())
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                        <span>{i18n.tr(I18nKey::FilePreviewRefresh)}</span>
                    </span>
                </button>
            </div>
        </header>
    }
}
