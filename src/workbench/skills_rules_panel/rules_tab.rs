//! Right-panel tab body for `.agents/rules/*.md`.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::RuleEntry;
use crate::workbench::skills_rules_panel::rule_card::RuleCard;
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::{RightPanelTab, WorkbenchService};

#[component]
pub fn RulesTabDock() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let rules = svc.rules();
    let loading = svc.rules_loading();
    let error = svc.rules_error();
    let active_tab = wb.right_active_tab();
    let active_id = wb.active_id();
    let composer_open = RwSignal::new(false);
    let draft_title = RwSignal::new(String::new());
    let draft_body = RwSignal::new(String::new());
    let draft_error = RwSignal::<Option<String>>::new(None);
    let saving = RwSignal::new(false);

    let reset_composer = move || {
        draft_title.set(String::new());
        draft_body.set(String::new());
        draft_error.set(None);
        composer_open.set(false);
    };

    let preview_name = Signal::derive(move || {
        let existing = rules.get();
        next_rule_name(&draft_title.get(), &existing)
    });

    let submit_new_rule = move |_| {
        let title = draft_title.get().trim().to_owned();
        if title.is_empty() {
            draft_error.set(Some(i18n.tr(I18nKey::SrRuleTitleRequired)().to_string()));
            return;
        }
        let body = draft_body.get();
        let content = if body.trim().is_empty() {
            format!("# {title}\n\n")
        } else if body.trim_start().starts_with('#') {
            body
        } else {
            format!("# {title}\n\n{body}")
        };
        let name = preview_name.get();
        svc.create_rule(wb, name, content, saving, move |result| {
            if result.is_ok() {
                reset_composer();
            }
        });
    };

    // Auto-load when the tab becomes active or the workspace changes.
    Effect::new(move |_| {
        if active_tab.get() == RightPanelTab::Rules {
            // Touch active_id so we react to workspace switches too.
            let _ = active_id.get();
            svc.refresh_rules(wb);
        }
    });

    view! {
        <div class="blx-sr-pane" role="region" aria-label=move || i18n.tr(I18nKey::TabRules)()>
            <header class="blx-sr-pane__header">
                <div class="blx-sr-pane__title-wrap">
                    <span class="blx-sr-pane__title-icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuShield width="14px" height="14px" />
                    </span>
                    <h2 class="blx-sr-pane__title">{i18n.tr(I18nKey::TabRules)}</h2>
                </div>
                <div class="blx-sr-pane__actions">
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--primary blx-sr-btn--icon"
                        aria-label=move || i18n.tr(I18nKey::SrNewRule)()
                        title=move || i18n.tr(I18nKey::SrNewRule)()
                        on:click=move |_| {
                            composer_open.set(true);
                            draft_error.set(None);
                        }
                    >
                        <LxIcon icon=icondata::LuPlus width="13px" height="13px" />
                    </button>
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--icon"
                        aria-label=move || i18n.tr(I18nKey::SrRefresh)()
                        title=move || i18n.tr(I18nKey::SrRefresh)()
                        on:click=move |_| svc.refresh_rules(wb)
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="13px" height="13px" />
                    </button>
                </div>
            </header>
            <div class="blx-sr-pane__body">
                {move || error.get().map(|e| view! { <p class="blx-sr-pane__err">{e}</p> })}
                {move || composer_open.get().then(|| {
                    let is_saving = saving.get();
                    view! {
                        <article class="blx-sr-card blx-sr-card--open blx-sr-card--composer">
                            <div class="blx-sr-card__row blx-sr-card__row--static">
                                <span class="blx-sr-card__chevron" aria-hidden="true">
                                    <LxIcon icon=icondata::LuSparkles width="14px" height="14px" />
                                </span>
                                <span class="blx-sr-card__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuShieldPlus width="16px" height="16px" />
                                </span>
                                <span class="blx-sr-card__main">
                                    <span class="blx-sr-card__title-row">
                                        <input
                                            class="blx-sr-card__title-input"
                                            type="text"
                                            placeholder=move || i18n.tr(I18nKey::SrRuleTitlePh)()
                                            prop:value=move || draft_title.get()
                                            on:input=move |ev| {
                                                draft_title.set(input_value(&ev));
                                                draft_error.set(None);
                                            }
                                        />
                                        <span class="blx-sr-card__badge" data-kind="rule">{i18n.tr(I18nKey::SrRuleBadgeNew)}</span>
                                    </span>
                                    <span class="blx-sr-card__summary">{move || preview_name.get()}</span>
                                </span>
                                <span class="blx-switch blx-switch--on" aria-hidden="true">
                                    <span class="blx-switch__thumb" />
                                </span>
                            </div>

                            <section class="blx-sr-card__body">
                                <textarea
                                    class="blx-sr-card__editor"
                                    spellcheck="false"
                                    placeholder=move || i18n.tr(I18nKey::SrRuleBodyPh)()
                                    prop:value=move || draft_body.get()
                                    on:input=move |ev| draft_body.set(textarea_value(&ev))
                                ></textarea>
                                {move || draft_error.get().map(|e| view! {
                                    <p class="blx-sr-card__error">{e}</p>
                                })}
                                <div class="blx-sr-card__actions">
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--ghost"
                                        disabled=is_saving
                                        on:click=move |_| reset_composer()
                                    >
                                        <span>{i18n.tr(I18nKey::SrCancel)}</span>
                                    </button>
                                    <button
                                        type="button"
                                        class="blx-sr-btn blx-sr-btn--primary"
                                        disabled=is_saving
                                        on:click=submit_new_rule
                                    >
                                        <LxIcon icon=icondata::LuSave width="13px" height="13px" />
                                        <span>{i18n.tr(I18nKey::BtnSave)}</span>
                                    </button>
                                </div>
                            </section>
                        </article>
                    }
                })}
                {move || {
                    if loading.get() && rules.with(|r| r.is_empty()) {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrLoading)}</p> }
                            .into_any()
                    } else if rules.with(|r| r.is_empty()) && !composer_open.get() {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrRulesEmpty)}</p> }
                            .into_any()
                    } else {
                        view! {
                            <For
                                each=move || rules.get()
                                key=|r| r.name.clone()
                                children=move |entry| view! { <RuleCard entry=entry /> }
                            />
                        }
                        .into_any()
                    }
                }}
            </div>
        </div>
    }
}

fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

fn textarea_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

fn next_rule_name(title: &str, existing: &[RuleEntry]) -> String {
    let base = slugify_title(title);
    let mut name = format!("rule-{base}.md");
    let mut i = 2;
    while existing.iter().any(|r| r.name == name) {
        name = format!("rule-{base}-{i}.md");
        i += 1;
    }
    name
}

fn slugify_title(title: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in title.trim().chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() || lower == '_' {
            slug.push(lower);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "new-rule".into()
    } else {
        slug.chars().take(80).collect()
    }
}
