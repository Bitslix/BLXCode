//! Code editor settings pane.
//!
//! Configures the in-app file editor / preview. First control is the Vim
//! key-bindings switch (default on); the section layout is built to host
//! further code-editor settings later. Styled with theme tokens only.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::{EditorSettingsService, SettingsPaneHeader};

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|input| input.checked())
}

#[component]
pub fn CodeEditorSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let editor_settings = expect_context::<EditorSettingsService>();
    let vim_enabled = editor_settings.vim_enabled();

    view! {
        <article class="harness-pane code-editor-pane">
            <SettingsPaneHeader
                icon=icondata::LuCode
                title=I18nKey::CodeEditorHeading
                description=I18nKey::CodeEditorDescription
            />

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuKeyboard width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">
                        {move || i18n.tr(I18nKey::CodeEditorVimTitle)()}
                    </span>
                </h4>
                <label class="app-prefs-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || vim_enabled.get()
                        on:change=move |ev| {
                            if let Some(checked) = checkbox_checked(&ev) {
                                editor_settings.set_vim_enabled(checked);
                            }
                        }
                    />
                    <span
                        class="blx-switch"
                        class:blx-switch--on=move || vim_enabled.get()
                        aria-hidden="true"
                    >
                        <span class="blx-switch__thumb" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::CodeEditorVimTitle)()}</span>
                </label>
                <p class="app-prefs-hint">
                    {move || i18n.tr(I18nKey::CodeEditorVimDesc)()}
                </p>
            </section>
        </article>
    }
}
