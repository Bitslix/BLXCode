//! Shared confirmation overlay.
//!
//! A single, themed Leptos dialog that replaces the browser-native
//! `window.confirm()` prompt across the app. It is driven entirely by
//! [`HarnessUiService::confirm_request`]: any feature that needs a yes/no
//! confirmation builds a [`ConfirmRequest`] (title, body, button labels, a
//! danger flag, and an `on_confirm` callback) and hands it to
//! [`HarnessUiService::request_confirm`]. The dialog renders the strings it
//! is given — it owns no copy of its own — so it stays reusable.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::state::HarnessUiService;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn ConfirmDialog() -> impl IntoView {
    let ui = expect_context::<HarnessUiService>();
    let i18n = expect_context::<I18nService>();
    let request = ui.confirm_request();

    let title = move || request.get().map(|r| r.title).unwrap_or_default();
    let body = move || request.get().map(|r| r.body).unwrap_or_default();
    let confirm_label = move || request.get().map(|r| r.confirm_label).unwrap_or_default();
    let cancel_label = move || request.get().map(|r| r.cancel_label).unwrap_or_default();
    let is_danger = move || request.get().map(|r| r.danger).unwrap_or(false);

    let accept = move || {
        if let Some(req) = request.get_untracked() {
            req.on_confirm.run(());
        }
        ui.dismiss_confirm();
    };

    view! {
        <Show when=move || request.get().is_some()>
            <div class="harness-overlay harness-overlay--modal" role="presentation">
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=move |_| ui.dismiss_confirm()
                ></button>
                <section
                    class="harness-sheet blx-confirm"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" {
                            ev.prevent_default();
                            ui.dismiss_confirm();
                        }
                    }
                >
                    <header class="blx-confirm__head">
                        <h2 class="harness-settings-title">
                            <span class="harness-settings-title__icon" aria-hidden="true">
                                <LxIcon
                                    icon=icondata::LuTriangleAlert
                                    width="1.05rem"
                                    height="1.05rem"
                                />
                            </span>
                            <span>{title}</span>
                        </h2>
                    </header>

                    <p class="blx-confirm__body">{body}</p>

                    <footer class="blx-confirm__actions">
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| ui.dismiss_confirm()
                        >
                            {cancel_label}
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            class:blx-confirm__accept--danger=is_danger
                            on:click=move |_| accept()
                        >
                            {confirm_label}
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}
