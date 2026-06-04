//! Mermaid file (`.mmd` / `.mermaid`) preview renderer.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::diagram_render::MermaidPreviewWithInspector;
use crate::workbench::file_preview::editor::policy::Editability;
use crate::workbench::file_preview::editor::{DocStatus, EditMode, EditorSession};
use crate::workbench::file_preview::util::{render_load_error, FilePreviewError};
use crate::workbench::toast::ToastService;
use crate::workbench::{HarnessUiService, WorkbenchService};
use leptos::prelude::*;
use uuid::Uuid;

#[component]
pub fn MermaidView(
    session: EditorSession,
    reload_tick: ReadSignal<u32>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();
    let ui = expect_context::<HarnessUiService>();
    let dom_id = Uuid::new_v4().to_string().replace('-', "");

    Effect::new(move |_| {
        let _ = reload_tick.get();
        session.reload(wb);
    });
    Effect::new(move |_| {
        if matches!(session.status.get(), DocStatus::TooLarge) {
            session.editability.set(Editability::NeverEdit);
            if matches!(session.mode.get_untracked(), EditMode::Edit) {
                session.mode.set(EditMode::View);
            }
        }
    });

    view! {
        <div class="file-preview__stage file-preview__stage--mermaid">
            {move || match session.status.get() {
                DocStatus::Loading => view! {
                    <div class="file-preview__status">{i18n.tr(I18nKey::FilePreviewLoading)}</div>
                }.into_any(),
                DocStatus::Error(e) => {
                    render_load_error(
                        i18n,
                        I18nKey::FilePreviewLoadFailedMermaid,
                        FilePreviewError::Failed(e),
                    )
                }
                _ => {
                    let id_attr = dom_id.clone();
                    view! {
                        <Show when=move || matches!(session.status.get(), DocStatus::TooLarge)>
                            <div class="file-preview__notice">
                                {i18n.tr(I18nKey::FilePreviewEditorTooLargeBanner)}
                            </div>
                        </Show>
                        <MermaidPreviewWithInspector
                            source=session.buffer
                            dom_id=id_attr
                            inspector_open=Signal::derive(move || {
                                matches!(session.mode.get(), EditMode::Edit)
                            })
                            can_save=Signal::derive(move || session.can_save())
                            can_revert=Signal::derive(move || session.dirty.get())
                            on_save=Callback::new(move |()| {
                                session.save(wb, toast, ui, i18n, false);
                            })
                            on_revert=Callback::new(move |()| session.revert())
                            allow_save=true
                            compact=true
                        />
                    }.into_any()
                }
            }}
        </div>
    }
}
