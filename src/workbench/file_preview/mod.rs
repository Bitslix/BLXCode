//! Rich file-preview dispatcher used by the workspace center tab.
//! Reads file metadata once, then routes to the renderer matching the
//! detected [`crate::tauri_bridge::FileKind`]. Falls back to a monospaced
//! text view for everything else.

mod code_context_menu;
mod code_view;
mod header;
mod hljs_glue;
mod image_view;
mod markdown_view;
mod mermaid_glue;
mod mermaid_view;
mod util;
mod video_view;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, stat_workspace_file, FileKind, FileMeta, PolicyKind};
use crate::workbench::WorkbenchService;
use code_view::CodeView;
use header::FilePreviewHeader;
use image_view::ImageView;
use leptos::prelude::*;
use leptos::task::spawn_local;
use markdown_view::MarkdownView;
use mermaid_view::MermaidView;
use util::{render_load_error, FilePreviewError};
use video_view::VideoView;

/// Top-level file preview component. Owns metadata + reload tick;
/// individual renderers consume both via signals.
#[component]
pub fn FilePreviewDock(workspace_id: u64, rel_path: String) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let meta_sig = RwSignal::new(None::<Result<FileMeta, FilePreviewError>>);
    let (reload_tick, set_reload_tick) = signal(0u32);

    let rel_for_meta = rel_path.clone();
    Effect::new(move |_| {
        // IMPORTANT: only re-run on an explicit reload tick. Reading
        // `wb.workspaces().get()` reactively here would refetch the file on
        // every tab switch, because the workspaces signal also carries
        // `center_active_tab_id`. Use `with_untracked` so the lookup does
        // not subscribe — `workspace_id` and `rel_path` are static props.
        let _ = reload_tick.get();
        meta_sig.set(None);
        if !is_tauri_shell() {
            meta_sig.set(Some(Err(FilePreviewError::NoTauri)));
            return;
        }
        let Some(root) = wb.workspaces().with_untracked(|list| {
            list.iter()
                .find(|w| w.id == workspace_id)
                .map(|w| w.cwd.clone())
        }) else {
            meta_sig.set(Some(Err(FilePreviewError::WorkspaceNotFound)));
            return;
        };
        let rel = rel_for_meta.clone();
        spawn_local(async move {
            match stat_workspace_file(root, rel).await {
                Ok(m) => meta_sig.set(Some(Ok(m))),
                Err(e) => meta_sig.set(Some(Err(FilePreviewError::Failed(e)))),
            }
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
                Some(Err(err)) => render_load_error(i18n, I18nKey::FilePreviewLoadFailedMeta, err),
                Some(Ok(meta)) => render_for_kind(
                    meta.kind,
                    meta.policy_kind,
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
    policy_kind: Option<PolicyKind>,
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
            <MarkdownView
                workspace_id=workspace_id
                rel_path=rel_path
                reload_tick=reload_tick
                policy_kind=policy_kind
            />
        }
        .into_any(),
        FileKind::Mermaid => view! {
            <MermaidView workspace_id=workspace_id rel_path=rel_path reload_tick=reload_tick />
        }
        .into_any(),
        FileKind::Code | FileKind::Text => view! {
            <CodeView
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
fn UnsupportedView() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="file-preview__stage file-preview__stage--unsupported">
            <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewUnsupported)}</div>
        </div>
    }
}
