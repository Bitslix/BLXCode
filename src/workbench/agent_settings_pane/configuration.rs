//! Configuration card — Images, Audio, Misc, Web search sub-groups.

use leptos::prelude::*;

use crate::i18n::I18nKey;

#[component]
pub(crate) fn ConfigurationSection() -> impl IntoView {
    view! {
        <section class="harness-subpane agent-settings-card">
            <super::CardHead icon=icondata::LuSlidersHorizontal label=I18nKey::AgSecConfiguration />
        </section>
    }
}
