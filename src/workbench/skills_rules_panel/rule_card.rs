//! Single Markdown rule rendered with the same expandable card pattern as
//! skills, plus a default-read editor mode for changing the rule body.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::RuleEntry;
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::WorkbenchService;

#[component]
pub fn RuleCard(entry: RuleEntry) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let name = entry.name.clone();
    let name_for_toggle = name.clone();
    let name_for_remove = name.clone();
    let name_for_read = name.clone();
    let name_for_save = name.clone();
    let enabled = entry.enabled;
    let title = entry.title.clone();
    let summary = entry.summary.clone();

    let expanded = RwSignal::new(false);
    let editing = RwSignal::new(false);
    let body = RwSignal::<Option<String>>::new(None);
    let draft = RwSignal::new(String::new());
    let body_loading = RwSignal::new(false);
    let saving = RwSignal::new(false);

    let on_toggle_card = move |_| {
        let next = !expanded.get();
        expanded.set(next);
        if next && body.with(|b| b.is_none()) && !body_loading.get() {
            svc.read_rule_into(wb, name_for_read.clone(), body, body_loading);
        }
    };

    let on_toggle_switch = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        svc.set_rule_enabled(wb, name_for_toggle.clone(), !enabled);
    };

    let on_remove = StoredValue::new(name_for_remove);
    let on_save = StoredValue::new(name_for_save);

    view! {
        <article
            class="blx-sr-card"
            class:blx-sr-card--off=move || !enabled
            class:blx-sr-card--open=move || expanded.get()
        >
            <button
                type="button"
                class="blx-sr-card__row"
                aria-expanded=move || if expanded.get() { "true" } else { "false" }
                on:click=on_toggle_card
            >
                <span class="blx-sr-card__chevron" aria-hidden="true">
                    <LxIcon icon=icondata::LuChevronRight width="14px" height="14px" />
                </span>
                <span class="blx-sr-card__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuShield width="16px" height="16px" />
                </span>
                <span class="blx-sr-card__main">
                    <span class="blx-sr-card__title-row">
                        <span class="blx-sr-card__title">{title}</span>
                        <span class="blx-sr-card__badge" data-kind="rule">rule</span>
                    </span>
                    <span class="blx-sr-card__summary">{summary}</span>
                </span>
                <span
                    class="blx-switch"
                    class:blx-switch--on=move || enabled
                    role="switch"
                    aria-checked=move || if enabled { "true" } else { "false" }
                    aria-label=move || i18n.tr(if enabled { I18nKey::SrDisable } else { I18nKey::SrEnable })()
                    tabindex="0"
                    on:click=on_toggle_switch
                >
                    <span class="blx-switch__thumb" />
                </span>
            </button>

            {move || expanded.get().then(|| {
                let is_loading = body_loading.get();
                let is_saving = saving.get();
                let has_body = body.with(|b| b.is_some());
                let is_editing = editing.get();
                view! {
                    <section class="blx-sr-card__body">
                        {if is_loading && !has_body {
                            view! { <p class="blx-sr-card__hint">{i18n.tr(I18nKey::SrLoading)}</p> }.into_any()
                        } else if is_editing {
                            view! {
                                <textarea
                                    class="blx-sr-card__editor"
                                    spellcheck="false"
                                    prop:value=move || draft.get()
                                    on:input=move |ev| {
                                        if let Some(value) = textarea_value(&ev) {
                                            draft.set(value);
                                        }
                                    }
                                ></textarea>
                            }.into_any()
                        } else {
                            view! {
                                <div
                                    class="blx-sr-card__md"
                                    inner_html=move || body.with(|b| {
                                        b.as_deref().map(render_markdown_to_html).unwrap_or_default()
                                    })
                                />
                            }.into_any()
                        }}
                        <div class="blx-sr-card__actions">
                            {if is_editing {
                                view! {
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--ghost"
                                        disabled=is_saving
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            draft.set(body.with(|b| b.clone().unwrap_or_default()));
                                            editing.set(false);
                                        }
                                    >
                                        <span>{i18n.tr(I18nKey::SrCancel)}</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--primary"
                                        disabled=is_saving
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            svc.write_rule(
                                                wb,
                                                on_save.get_value(),
                                                draft.get_untracked(),
                                                body,
                                                editing,
                                                saving,
                                            );
                                        }
                                    >
                                        <LxIcon icon=icondata::LuSave width="13px" height="13px" />
                                        <span>"Save"</span>
                                    </button>
                                }.into_any()
                            } else {
                                view! {
                                    <button
                                        type="button"
                                        class="blx-sr-btn"
                                        disabled=!has_body
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            draft.set(body.with(|b| b.clone().unwrap_or_default()));
                                            editing.set(true);
                                        }
                                    >
                                        <LxIcon icon=icondata::LuPencil width="13px" height="13px" />
                                        <span>"Edit"</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--danger"
                                        on:click=move |ev: web_sys::MouseEvent| {
                                            ev.stop_propagation();
                                            let msg = i18n.tr(I18nKey::SrConfirmRemove)();
                                            if confirm_window(&msg) {
                                                let n = on_remove.get_value();
                                                svc.remove_rule(wb, n);
                                            }
                                        }
                                    >
                                        <LxIcon icon=icondata::LuTrash2 width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::SrRemove)}</span>
                                    </button>
                                }.into_any()
                            }}
                        </div>
                    </section>
                }
            })}
        </article>
    }
}

fn textarea_value(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlTextAreaElement>()
        .ok()
        .map(|i| i.value())
}

fn confirm_window(msg: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(msg).ok())
        .unwrap_or(true)
}
