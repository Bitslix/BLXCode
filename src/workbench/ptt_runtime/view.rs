//! Floating push-to-talk indicator: shows the recording state, the live
//! partial transcript, and transient hints. All colours come from theme
//! tokens (see `ptt_runtime.css`).

use leptos::prelude::*;

use crate::i18n::I18nKey;
use crate::service::I18nService;

use super::PttBus;

/// A small overlay rendered at the workbench root. Visible only while
/// recording or when a hint is showing.
#[component]
pub fn PttIndicator() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let bus = expect_context::<PttBus>();

    let visible = move || bus.recording.get() || bus.hint.get().is_some();

    view! {
        <Show when=visible>
            <div class="ptt-indicator" role="status" aria-live="polite">
                <Show when=move || bus.recording.get()>
                    <div class="ptt-indicator__row">
                        <span class="ptt-indicator__dot" aria-hidden="true"></span>
                        <span class="ptt-indicator__label">
                            {move || i18n.tr(I18nKey::VoicePttRecording)()}
                        </span>
                    </div>
                    <Show when=move || !bus.partial.get().is_empty()>
                        <p class="ptt-indicator__partial">{move || bus.partial.get()}</p>
                    </Show>
                </Show>
                <Show when=move || bus.hint.get().is_some()>
                    <p class="ptt-indicator__hint">
                        {move || bus.hint.get().unwrap_or_default()}
                    </p>
                </Show>
            </div>
        </Show>
    }
}
