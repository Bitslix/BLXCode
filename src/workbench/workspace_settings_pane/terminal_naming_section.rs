//! Workspace-settings section for the terminal naming mode + editable name
//! pool. Mode and pool persist immediately via [`AppPrefsService`]; there is
//! no save/discard cycle (same as the other app-pref toggles).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::app_prefs::AppPrefsService;
use crate::workbench::terminal_naming::TerminalNamingMode;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.checked())
}

fn input_value(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

#[component]
pub fn TerminalNamingSection() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let mode = prefs.terminal_naming_mode();
    let names_on = Memo::new(move |_| mode.get() == TerminalNamingMode::Names);
    // Local editable copy: allows transient blanks while typing; the cleaned
    // result is pushed to prefs on every commit (blur / add / remove / reset).
    let draft = RwSignal::new(prefs.terminal_name_pool().get_untracked());

    let commit = move || prefs.set_terminal_name_pool(draft.get_untracked());

    let on_toggle = move |ev: web_sys::Event| {
        if let Some(checked) = checkbox_checked(&ev) {
            prefs.set_terminal_naming_mode(if checked {
                TerminalNamingMode::Names
            } else {
                TerminalNamingMode::SlotNumbers
            });
        }
    };

    view! {
        <section class="harness-subpane">
            <h4 class="harness-pane-subhead">
                <span class="harness-pane-subhead__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuTerminal width="0.82rem" height="0.82rem" />
                </span>
                <span class="harness-pane-subhead__text">
                    {move || i18n.tr(I18nKey::WsSectionTerminalNaming)()}
                </span>
            </h4>
            <label class="app-prefs-toggle">
                <input type="checkbox" prop:checked=move || names_on.get() on:change=on_toggle />
                <span>{move || i18n.tr(I18nKey::WsTerminalNamingToggleLabel)()}</span>
            </label>
            <p class="app-prefs-hint">{move || i18n.tr(I18nKey::WsTerminalNamingHint)()}</p>

            <Show when=move || names_on.get()>
                <div class="terminal-naming-pool">
                    <span class="terminal-naming-pool__label">
                        {move || i18n.tr(I18nKey::WsTerminalNamingPoolLabel)()}
                    </span>
                    <div class="terminal-naming-pool__items">
                        {move || {
                            draft
                                .get()
                                .into_iter()
                                .enumerate()
                                .map(|(idx, name)| {
                                    view! {
                                        <div class="terminal-naming-pool__row">
                                            <input
                                                class="workbench-plain-input terminal-naming-pool__input"
                                                type="text"
                                                prop:value=name
                                                on:input=move |ev| {
                                                    if let Some(v) = input_value(&ev) {
                                                        draft.update(|list| {
                                                            if let Some(slot) = list.get_mut(idx) {
                                                                *slot = v;
                                                            }
                                                        });
                                                    }
                                                }
                                                on:blur=move |_| commit()
                                            />
                                            <button
                                                type="button"
                                                class="workbench-mini-btn workbench-mini-btn--danger"
                                                title=move || i18n.tr(I18nKey::WsTerminalNamingRemoveAria)()
                                                aria-label=move || i18n.tr(I18nKey::WsTerminalNamingRemoveAria)()
                                                on:click=move |_| {
                                                    draft.update(|list| {
                                                        if idx < list.len() {
                                                            list.remove(idx);
                                                        }
                                                    });
                                                    commit();
                                                }
                                            >
                                                <LxIcon icon=icondata::LuX width="0.74rem" height="0.74rem" />
                                            </button>
                                        </div>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                    <div class="terminal-naming-pool__actions harness-row-gap">
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| {
                                draft.update(|list| list.push(String::new()));
                            }
                        >
                            <span class="harness-btn-inline">
                                <LxIcon icon=icondata::LuPlus width="0.74rem" height="0.74rem" />
                                <span>{move || i18n.tr(I18nKey::WsTerminalNamingAdd)()}</span>
                            </span>
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            on:click=move |_| {
                                prefs.reset_terminal_name_pool();
                                draft.set(prefs.terminal_name_pool().get_untracked());
                            }
                        >
                            <span class="harness-btn-inline">
                                <LxIcon icon=icondata::LuUndo2 width="0.74rem" height="0.74rem" />
                                <span>{move || i18n.tr(I18nKey::WsTerminalNamingReset)()}</span>
                            </span>
                        </button>
                    </div>
                    <p class="app-prefs-hint">{move || i18n.tr(I18nKey::WsTerminalNamingPoolHint)()}</p>
                </div>
            </Show>
        </section>
    }
}
