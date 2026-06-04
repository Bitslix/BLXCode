//! Provider card — Chat / Image / STT / TTS provider pickers + key status.

use leptos::prelude::*;

use crate::i18n::I18nKey;

#[component]
pub(crate) fn ProvidersSection() -> impl IntoView {
    view! {
        <section class="harness-subpane agent-settings-card">
            <super::CardHead icon=icondata::LuPlug label=I18nKey::AgSecProvider />
        </section>
    }
}
