//! Single installed skill rendered as an expandable card with a shadcn-style
//! switch, a source-kind icon badge, and an inline reader for the skill's
//! `SKILL.md` body (loaded lazily on first expand).

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::{SkillEntry, SkillSourceKind};
use crate::workbench::chat_markdown::render_markdown_to_html;
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::WorkbenchService;

#[component]
pub fn SkillCard(entry: SkillEntry) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let name = entry.name.clone();
    let name_for_toggle = name.clone();
    let name_for_remove = name.clone();
    let name_for_read = name.clone();
    let enabled = entry.enabled;
    let title = entry.title.clone();
    let summary = entry.summary.clone();
    let missing_skill_md = entry.missing_skill_md;
    let is_core = entry.source.kind == SkillSourceKind::Core;

    let (source_label, source_icon) = match entry.source.kind {
        SkillSourceKind::Core => ("core", icondata::LuBox),
        SkillSourceKind::Git => ("git", icondata::LuGitBranch),
        SkillSourceKind::Npm => ("npm", icondata::LuPackage),
        SkillSourceKind::Local => ("local", icondata::LuFolder),
        SkillSourceKind::AgentCreated => ("agent", icondata::LuBot),
    };

    let expanded = RwSignal::new(false);
    let body = RwSignal::<Option<String>>::new(None);
    let body_loading = RwSignal::new(false);

    let on_toggle_card = move |_| {
        let next = !expanded.get();
        expanded.set(next);
        if next && body.with(|b| b.is_none()) && !body_loading.get() {
            svc.read_skill_into(wb, name_for_read.clone(), body, body_loading);
        }
    };

    let on_toggle_switch = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        svc.set_skill_enabled(wb, name_for_toggle.clone(), !enabled);
    };

    let on_remove = StoredValue::new(name_for_remove);

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
                    <LxIcon icon=source_icon width="16px" height="16px" />
                </span>
                <span class="blx-sr-card__main">
                    <span class="blx-sr-card__title-row">
                        <span class="blx-sr-card__title">{title}</span>
                        <span class="blx-sr-card__badge" data-kind=source_label>{source_label}</span>
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

            {move || missing_skill_md.then(|| view! {
                <p class="blx-sr-card__warn">
                    <LxIcon icon=icondata::LuTriangleAlert width="13px" height="13px" />
                    <span>{i18n.tr(I18nKey::SrMissingSkillMd)}</span>
                </p>
            })}

            {move || expanded.get().then(|| {
                let i18n = i18n;
                let is_loading = body_loading.get();
                let has_body = body.with(|b| b.is_some());
                view! {
                    <section class="blx-sr-card__body">
                        {if is_loading && !has_body {
                            view! { <p class="blx-sr-card__hint">{i18n.tr(I18nKey::SrLoading)}</p> }.into_any()
                        } else if !has_body {
                            view! { <p class="blx-sr-card__hint">{i18n.tr(I18nKey::SrMissingSkillMd)}</p> }.into_any()
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
                            {(!is_core).then(|| view! {
                                <button
                                    type="button"
                                    class="blx-sr-btn blx-sr-btn--danger"
                                    on:click=move |ev: web_sys::MouseEvent| {
                                        ev.stop_propagation();
                                        let msg = i18n.tr(I18nKey::SrConfirmRemove)();
                                        if confirm_window(&msg) {
                                            let n = on_remove.get_value();
                                            svc.remove_skill(wb, n);
                                        }
                                    }
                                >
                                    <LxIcon icon=icondata::LuTrash2 width="13px" height="13px" />
                                    <span>{i18n.tr(I18nKey::SrRemove)}</span>
                                </button>
                            })}
                        </div>
                    </section>
                }
            })}
        </article>
    }
}

fn confirm_window(msg: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(msg).ok())
        .unwrap_or(true)
}
