//! Topbar shown above every file-preview renderer (name, path, size, mtime)
//! plus the editor controls (View/Edit, Save, Revert) driven by the shared
//! [`EditorSession`].

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{self, FileMeta, PopoutPayload};
use crate::workbench::file_preview::editor::policy::Editability;
use crate::workbench::file_preview::editor::{EditMode, EditorSession};
use crate::workbench::file_preview::util::{format_bytes, format_mtime, icon_for_kind};
use crate::workbench::toast::ToastService;
use crate::workbench::{HarnessUiService, WorkbenchService};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn FilePreviewHeader(
    meta: Memo<Option<FileMeta>>,
    rel_path: String,
    on_refresh: Callback<()>,
    session: EditorSession,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let toast = expect_context::<ToastService>();
    let ui = expect_context::<HarnessUiService>();
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
    let mermaid_popout_path = rel_path.clone();
    let is_mermaid_preview = move || {
        let lower = mermaid_popout_path.to_ascii_lowercase();
        lower.ends_with(".mmd") || lower.ends_with(".mermaid")
    };

    // --- Editor state derivations ---
    let editing = move || matches!(session.mode.get(), EditMode::Edit);
    let can_show_edit = move || {
        matches!(session.mode.get(), EditMode::View)
            && matches!(
                session.editability.get(),
                Editability::Editable | Editability::ReadOnlyByDefault
            )
    };
    let is_read_only = move || matches!(session.editability.get(), Editability::NeverEdit);

    let on_edit = move |_| session.enter_edit();
    let on_save = move |_| session.save(wb, toast, ui, i18n, false);
    let on_revert = move |_| {
        if session.dirty.get_untracked() {
            let revert = Callback::new(move |()| session.revert());
            session.confirm_discard(ui, i18n, revert);
        } else {
            session.revert();
        }
    };
    let on_exit = move |_| {
        if session.dirty.get_untracked() {
            let exit = Callback::new(move |()| {
                session.revert();
                session.exit_edit();
            });
            session.confirm_discard(ui, i18n, exit);
        } else {
            session.exit_edit();
        }
    };

    view! {
        <header class="file-preview__header">
            <div class="file-preview__title-block">
                <div class="file-preview__title">
                    <span class="file-preview__icon" aria-hidden="true">
                        <LxIcon icon=kind_icon width="1rem" height="1rem" />
                    </span>
                    <span class="file-preview__name">{display_name}</span>
                    <Show when=move || session.dirty.get()>
                        <span
                            class="file-preview__dirty-dot"
                            aria-hidden="true"
                            title=move || i18n.tr(I18nKey::FilePreviewEditorModified)()
                        >"●"</span>
                    </Show>
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
                    <Show when=editing>
                        <span class="file-preview__state-chip file-preview__state-chip--edit">
                            {i18n.tr(I18nKey::FilePreviewEditorEdit)}
                        </span>
                    </Show>
                    <Show when=move || is_read_only()>
                        <span class="file-preview__state-chip">
                            {i18n.tr(I18nKey::FilePreviewEditorReadOnly)}
                        </span>
                    </Show>
                    <Show when=move || session.dirty.get()>
                        <span class="file-preview__state-chip file-preview__state-chip--dirty">
                            {i18n.tr(I18nKey::FilePreviewEditorModified)}
                        </span>
                    </Show>
                </div>
            </div>
            <div class="file-preview__actions">
                <Show when=is_mermaid_preview>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        title=move || i18n.tr(I18nKey::PopoutMermaid)()
                        aria-label=move || i18n.tr(I18nKey::PopoutMermaid)()
                        on:click={
                            let rel_path = rel_path.clone();
                            move |_| {
                                let Some(workspace_id) = wb.active_id().get_untracked() else {
                                    return;
                                };
                                let payload = PopoutPayload::MermaidFile {
                                    workspace_id,
                                    rel_path: rel_path.clone(),
                                };
                                leptos::task::spawn_local(async move {
                                    let _ = tauri_bridge::popout_open(payload).await;
                                });
                            }
                        }
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuExternalLink width="0.78rem" height="0.78rem" />
                            <span>{i18n.tr(I18nKey::PopoutOpen)}</span>
                        </span>
                    </button>
                </Show>
                <Show when=can_show_edit>
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        title=move || i18n.tr(I18nKey::FilePreviewEditorEdit)()
                        aria-label=move || i18n.tr(I18nKey::FilePreviewEditorEdit)()
                        on:click=on_edit
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuPencil width="0.78rem" height="0.78rem" />
                            <span>{i18n.tr(I18nKey::FilePreviewEditorEdit)}</span>
                        </span>
                    </button>
                </Show>
                <Show when=editing>
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        disabled=move || !session.can_save()
                        title=move || i18n.tr(I18nKey::FilePreviewEditorSave)()
                        aria-label=move || i18n.tr(I18nKey::FilePreviewEditorSave)()
                        on:click=on_save
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                            <span>{i18n.tr(I18nKey::FilePreviewEditorSave)}</span>
                        </span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        disabled=move || !session.dirty.get()
                        title=move || i18n.tr(I18nKey::FilePreviewEditorRevert)()
                        aria-label=move || i18n.tr(I18nKey::FilePreviewEditorRevert)()
                        on:click=on_revert
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuUndo2 width="0.78rem" height="0.78rem" />
                            <span>{i18n.tr(I18nKey::FilePreviewEditorRevert)}</span>
                        </span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        title=move || i18n.tr(I18nKey::FilePreviewEditorViewMode)()
                        aria-label=move || i18n.tr(I18nKey::FilePreviewEditorViewMode)()
                        on:click=on_exit
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuEye width="0.78rem" height="0.78rem" />
                            <span>{i18n.tr(I18nKey::FilePreviewEditorViewMode)}</span>
                        </span>
                    </button>
                </Show>
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
