//! Shared header for settings panes.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;

#[component]
pub fn SettingsPaneHeader(
    icon: icondata::Icon,
    title: I18nKey,
    description: I18nKey,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <header class="harness-pane-header">
            <span class="harness-pane-title__icon" aria-hidden="true">
                <LxIcon icon=icon width="1.02rem" height="1.02rem" />
            </span>
            <div class="harness-pane-header__copy">
                <h3 class="harness-pane-title">
                    <span class="harness-pane-title__text">{move || i18n.tr(title)()}</span>
                </h3>
                <p class="harness-pane-description">{move || i18n.tr(description)()}</p>
            </div>
        </header>
    }
}
