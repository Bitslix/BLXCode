//! Personality card — Name, 2D|3D orb, Role, Intelligence, Gender, reply Voice.

use leptos::prelude::*;

use crate::i18n::I18nKey;

#[component]
pub(crate) fn PersonalitySection() -> impl IntoView {
    view! {
        <section class="harness-subpane agent-settings-card">
            <super::CardHead icon=icondata::LuUser label=I18nKey::AgSecPersonality />
        </section>
    }
}
