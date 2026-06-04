//! Models card — Chat / Image / STT / TTS model pickers + refresh.

use leptos::prelude::*;

use crate::i18n::I18nKey;

#[component]
pub(crate) fn ModelsSection() -> impl IntoView {
    view! {
        <section class="harness-subpane agent-settings-card">
            <super::CardHead icon=icondata::LuBoxes label=I18nKey::AgSecModels />
        </section>
    }
}
