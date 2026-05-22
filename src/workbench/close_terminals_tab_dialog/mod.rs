//! Confirmation overlay shown when the user clicks the close button on a
//! workspace's Terminals tab.
//!
//! The Terminals tab represents the whole workspace's PTY surface, so
//! closing it acts like a sidebar close: every running terminal is
//! terminated and the workspace is saved + moved to the recent list.
//! A 10s countdown gates the confirm button so accidental double-clicks
//! can't destroy a workspace.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::state::{HarnessUiService, WorkbenchService};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn CloseTerminalsTabDialog() -> impl IntoView {
    let ui = expect_context::<HarnessUiService>();
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let state = ui.close_terminals_confirm();
    let seconds_left = Memo::new(move |_| state.get().map(|s| s.seconds_left).unwrap_or(0));
    let can_confirm = Memo::new(move |_| seconds_left.get() == 0);

    let confirm_label = move || {
        let secs = seconds_left.get();
        if secs == 0 {
            // Drop the counter once enabled.
            let raw = i18n.tr(I18nKey::CenterTabCloseTerminalsConfirm)();
            raw.replace(" ({seconds}s)", "")
                .replace("({seconds}s)", "")
                .replace("（{seconds}秒）", "")
                .replace(" ({seconds} 秒)", "")
                .replace(" ({seconds}초)", "")
                .replace(" ({seconds} с)", "")
                .replace("({seconds})", "")
        } else {
            i18n.tr(I18nKey::CenterTabCloseTerminalsConfirm)()
                .replace("{seconds}", &secs.to_string())
        }
    };

    view! {
        <Show when=move || state.get().is_some()>
            <div class="harness-overlay harness-overlay--modal" role="presentation">
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=move |_| ui.dismiss_close_terminals_confirm()
                ></button>
                <section
                    class="harness-sheet harness-sheet--close-terminals"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" {
                            ev.prevent_default();
                            ui.dismiss_close_terminals_confirm();
                        }
                    }
                >
                    <header class="blx-close-terminals__head">
                        <h2 class="harness-settings-title">
                            <span class="harness-settings-title__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuTriangleAlert width="1.05rem" height="1.05rem" />
                            </span>
                            <span>{move || i18n.tr(I18nKey::CenterTabCloseTerminalsTitle)()}</span>
                        </h2>
                    </header>

                    <p class="blx-close-terminals__body">
                        {move || i18n.tr(I18nKey::CenterTabCloseTerminalsBody)()}
                    </p>

                    <footer class="blx-close-terminals__actions">
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            prop:disabled=move || !can_confirm.get()
                            on:click=move |_| {
                                let Some(state) = ui.close_terminals_confirm().get_untracked() else {
                                    return;
                                };
                                let ws_id = state.workspace_id;
                                ui.dismiss_close_terminals_confirm();
                                wb.close_center_terminals_tab(ws_id);
                            }
                        >
                            {confirm_label}
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| ui.dismiss_close_terminals_confirm()
                        >
                            {move || i18n.tr(I18nKey::CenterTabCloseTerminalsCancel)()}
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}
