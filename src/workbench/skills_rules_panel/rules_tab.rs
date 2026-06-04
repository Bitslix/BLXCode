//! Right-panel tab body for `.agents/rules/*.md`.

use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::RuleEntry;
use crate::workbench::skills_rules_panel::rule_card::RuleCard;
use crate::workbench::skills_rules_panel::rules_pointers::{
    RulesPointersDialog, RulesPointersNotice,
};
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
    let search_query = RwSignal::new(String::new());
    let selected_category = RwSignal::<Option<String>>::new(None);
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

    let filtered_rules = Memo::new(move |_| {
        let query = search_query.get().trim().to_lowercase();
        let category_filter = selected_category.get();
        let mut list = rules.get();
        if query.is_empty() && category_filter.is_none() {
            return list;
        }
        list.retain(|rule| {
            let matches_query = query.is_empty()
                || rule.name.to_lowercase().contains(&query)
                || rule.title.to_lowercase().contains(&query)
                || rule.summary.to_lowercase().contains(&query)
                || rule
                    .category
                    .as_ref()
                    .is_some_and(|category| category.to_lowercase().contains(&query));
            let matches_category = category_filter
                .as_ref()
                .is_none_or(|selected| rule.category.as_deref() == Some(selected.as_str()));
            matches_query && matches_category
        });
        list
    });

    let category_counts = Memo::new(move |_| {
        let mut counts = BTreeMap::<String, usize>::new();
        for rule in rules.get() {
            if let Some(category) = rule.category {
                *counts.entry(category).or_insert(0) += 1;
            }
        }
        counts.into_iter().collect::<Vec<_>>()
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
            svc.refresh_pointer_status(wb);
        }
    });

    // Reset pointer UI when the workspace itself changes so a stale
    // "dismissed" flag from the previous workspace doesn't suppress the
    // banner in the new one.
    Effect::new(move |prev: Option<Option<u64>>| {
        let id = active_id.get();
        if prev.is_some() && prev.unwrap() != id {
            svc.reset_pointer_ui();
            svc.refresh_pointer_status(wb);
        }
        id
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
            <Show when=move || !category_counts.with(|categories| categories.is_empty())>
                <div class="blx-sr-tabs blx-sr-category-filter" role="tablist" aria-label="Filter rules by category">
                    <button
                        type="button"
                        role="tab"
                        class="blx-sr-tab"
                        class:blx-sr-tab--active=move || selected_category.get().is_none()
                        aria-selected=move || if selected_category.get().is_none() { "true" } else { "false" }
                        on:click=move |_| selected_category.set(None)
                    >
                        <LxIcon icon=icondata::LuLayers width="13px" height="13px" />
                        <span>"All"</span>
                        <span class="blx-sr-tab__count">{move || rules.with(|r| r.len())}</span>
                    </button>
                    <For
                        each=move || category_counts.get()
                        key=|(category, _)| category.clone()
                        children=move |(category, count)| {
                            let category_for_active = category.clone();
                            let category_for_aria = category.clone();
                            let category_for_click = category.clone();
                            view! {
                                <button
                                    type="button"
                                    role="tab"
                                    class="blx-sr-tab"
                                    class:blx-sr-tab--active=move || {
                                        selected_category.with(|selected| selected.as_deref() == Some(category_for_active.as_str()))
                                    }
                                    aria-selected=move || {
                                        if selected_category.with(|selected| selected.as_deref() == Some(category_for_aria.as_str())) {
                                            "true"
                                        } else {
                                            "false"
                                        }
                                    }
                                    on:click=move |_| {
                                        selected_category.update(|selected| {
                                            if selected.as_deref() == Some(category_for_click.as_str()) {
                                                *selected = None;
                                            } else {
                                                *selected = Some(category_for_click.clone());
                                            }
                                        });
                                    }
                                >
                                    <span>{category}</span>
                                    <span class="blx-sr-tab__count">{count}</span>
                                </button>
                            }
                        }
                    />
                </div>
            </Show>
            <div class="blx-sr-pane__body">
                {move || error.get().map(|e| view! { <p class="blx-sr-pane__err">{e}</p> })}
                <Show when=move || {
                    active_id.get().is_some()
                        && !svc.pointers_notice_dismissed().get()
                        && svc
                            .pointer_status()
                            .get()
                            .is_some_and(|status| !status.iter().any(|e| e.installed))
                }>
                    <RulesPointersNotice />
                </Show>
                <label class="blx-sr-search">
                    <span class="blx-sr-search__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuSearch width="14px" height="14px" />
                    </span>
                    <input
                        type="search"
                        class="blx-sr-search__input"
                        placeholder=move || i18n.tr(I18nKey::SrRulesSearchRules)()
                        aria-label=move || i18n.tr(I18nKey::SrRulesSearchRulesAria)()
                        prop:value=move || search_query.get()
                        on:input=move |ev| search_query.set(input_value(&ev))
                    />
                </label>
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
                    } else if (!search_query.with(|q| q.trim().is_empty())
                        || selected_category.with(|category| category.is_some()))
                        && filtered_rules.with(|r| r.is_empty())
                    {
                        view! { <p class="blx-sr-pane__hint">"No rules match this search."</p> }
                            .into_any()
                    } else {
                        view! {
                            <For
                                each=move || filtered_rules.get()
                                key=|r| r.name.clone()
                                children=move |entry| view! { <RuleCard entry=entry /> }
                            />
                        }
                        .into_any()
                    }
                }}
            </div>
            <Show when=move || svc.pointers_open().get()>
                <RulesPointersDialog />
            </Show>
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
