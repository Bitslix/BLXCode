//! Rich file-preview dispatcher used by the workspace center tab.
//! Reads file metadata once, then routes to the renderer matching the
//! detected [`crate::tauri_bridge::FileKind`]. Falls back to a monospaced
//! text view for everything else.

mod header;
mod image_view;
mod markdown_view;
mod mermaid_glue;
mod mermaid_view;
mod util;
mod video_view;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, read_workspace_text_file, stat_workspace_file, FileKind, FileMeta,
};
use crate::workbench::WorkbenchService;
use header::FilePreviewHeader;
use image_view::ImageView;
use leptos::prelude::*;
use leptos::task::spawn_local;
use markdown_view::MarkdownView;
use mermaid_view::MermaidView;
use video_view::VideoView;

/// Top-level file preview component. Owns metadata + reload tick;
/// individual renderers consume both via signals.
#[component]
pub fn FilePreviewDock(workspace_id: u64, rel_path: String) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let meta_sig = RwSignal::new(None::<Result<FileMeta, String>>);
    let (reload_tick, set_reload_tick) = signal(0u32);

    let rel_for_meta = rel_path.clone();
    Effect::new(move |_| {
        let _ = reload_tick.get();
        meta_sig.set(None);
        if !is_tauri_shell() {
            meta_sig.set(Some(Err(
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
            meta_sig.set(Some(Err("Workspace not found.".into())));
            return;
        };
        let root = ws.cwd;
        let rel = rel_for_meta.clone();
        spawn_local(async move {
            let next = stat_workspace_file(root, rel).await;
            meta_sig.set(Some(next));
        });
    });

    let meta_memo: Memo<Option<FileMeta>> = Memo::new(move |_| match meta_sig.get() {
        Some(Ok(m)) => Some(m),
        _ => None,
    });

    let on_refresh = Callback::new(move |()| {
        set_reload_tick.update(|n| *n = n.wrapping_add(1));
    });

    let dispatcher_workspace_id = workspace_id;
    let dispatcher_rel_path = rel_path.clone();

    view! {
        <article class="file-preview">
            <FilePreviewHeader
                meta=meta_memo
                rel_path=rel_path.clone()
                on_refresh=on_refresh
            />
            {move || match meta_sig.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => view! {
                    <div class="file-preview__status file-preview__status--error">{err}</div>
                }.into_any(),
                Some(Ok(meta)) => render_for_kind(
                    meta.kind,
                    dispatcher_workspace_id,
                    dispatcher_rel_path.clone(),
                    reload_tick,
                ),
            }}
        </article>
    }
}

fn render_for_kind(
    kind: FileKind,
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> AnyView {
    match kind {
        FileKind::Image => view! {
            <ImageView workspace_id=workspace_id rel_path=rel_path reload_tick=reload_tick />
        }
        .into_any(),
        FileKind::Video => view! {
            <VideoView workspace_id=workspace_id rel_path=rel_path reload_tick=reload_tick />
        }
        .into_any(),
        FileKind::Markdown => view! {
            <MarkdownView workspace_id=workspace_id rel_path=rel_path reload_tick=reload_tick />
        }
        .into_any(),
        FileKind::Mermaid => view! {
            <MermaidView workspace_id=workspace_id rel_path=rel_path reload_tick=reload_tick />
        }
        .into_any(),
        FileKind::Text => view! {
            <TextFallbackView
                workspace_id=workspace_id
                rel_path=rel_path
                reload_tick=reload_tick
            />
        }
        .into_any(),
        FileKind::Binary => view! {
            <UnsupportedView />
        }
        .into_any(),
    }
}

#[component]
fn TextFallbackView(
    workspace_id: u64,
    rel_path: String,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let result = RwSignal::new(None::<Result<(String, bool, u64), String>>);

    let rel_for_effect = rel_path.clone();
    Effect::new(move |_| {
        let _ = reload_tick.get();
        result.set(None);
        if !is_tauri_shell() {
            result.set(Some(Err(
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
            result.set(Some(Err("Workspace not found.".into())));
            return;
        };
        let root = ws.cwd;
        let rel = rel_for_effect.clone();
        spawn_local(async move {
            match read_workspace_text_file(root, rel).await {
                Ok(t) => result.set(Some(Ok((t.content, t.truncated, t.byte_len)))),
                Err(e) => result.set(Some(Err(e))),
            }
        });
    });

    view! {
        <div class="file-preview__stage file-preview__stage--text">
            {move || match result.get() {
                None => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                Some(Err(err)) => view! {
                    <div class="file-preview__status file-preview__status--error">{err}</div>
                }.into_any(),
                Some(Ok((content, truncated, byte_len))) => view! {
                    <Show when=move || truncated>
                        <div class="file-preview__notice">
                            {format!("Preview truncated at 512 KiB of {byte_len} bytes.")}
                        </div>
                    </Show>
                    <pre class="file-preview__content"><code>{content}</code></pre>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn UnsupportedView() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="file-preview__stage file-preview__stage--unsupported">
            <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewUnsupported)}</div>
        </div>
    }
}
