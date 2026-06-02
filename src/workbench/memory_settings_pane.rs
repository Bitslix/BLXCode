//! Memory settings pane.

use super::app_prefs::AppPrefsService;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::SettingsPaneHeader;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|input| input.checked())
}

#[component]
pub fn MemorySettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();

    view! {
        <article class="harness-pane memory-settings-pane">
            <SettingsPaneHeader
                icon=icondata::LuLayers
                title=I18nKey::TabMemory
                description=I18nKey::MemorySettingsDescription
            />

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuPanelRight width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">
                        {move || i18n.tr(I18nKey::MemorySettingsSidePanelSection)()}
                    </span>
                </h4>
                <label class="app-prefs-toggle memory-settings-toggle">
                    <input
                        class="memory-settings-toggle__input"
                        type="checkbox"
                        prop:checked=move || prefs.memory_right_panel_enabled().get()
                        on:change=move |ev| {
                            if let Some(checked) = checkbox_checked(&ev) {
                                prefs.set_memory_right_panel_enabled(checked);
                            }
                        }
                    />
                    <span
                        class="blx-switch"
                        class:blx-switch--on=move || prefs.memory_right_panel_enabled().get()
                        aria-hidden="true"
                    >
                        <span class="blx-switch__thumb" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::MemorySettingsRightPanelToggle)()}</span>
                </label>
                <p class="app-prefs-hint">
                    {move || i18n.tr(I18nKey::MemorySettingsRightPanelHint)()}
                </p>
            </section>
        </article>
    }
}
