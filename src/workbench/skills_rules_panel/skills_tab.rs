//! Right-panel tab body for `.agents/skills/<name>/`. Contains the install
//! button which opens [`SkillInstallDialog`], a category filter row, and live
//! search matching the Rules tab structure.

use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::SkillSourceKind;
use crate::workbench::skills_rules_panel::install_dialog::SkillInstallDialog;
use crate::workbench::skills_rules_panel::skill_card::SkillCard;
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::{RightPanelTab, WorkbenchService};

#[component]
pub fn SkillsTabDock() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let skills = svc.skills();
    let loading = svc.skills_loading();
    let error = svc.skills_error();
    let active_tab = wb.right_active_tab();
    let active_id = wb.active_id();
    let install_open = RwSignal::new(false);
    let search_query = RwSignal::new(String::new());
    let selected_category = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        if active_tab.get() == RightPanelTab::Skills {
            let _ = active_id.get();
            svc.refresh_skills(wb);
        }
    });

    let visible_skills = Memo::new(move |_| {
        skills.with(|list| {
            list.iter()
                .filter(|skill| skill.source.kind != SkillSourceKind::Core)
                .cloned()
                .collect::<Vec<_>>()
        })
    });

    let filtered_skills = Memo::new(move |_| {
        let query = search_query.get().trim().to_lowercase();
        let category_filter = selected_category.get();
        let mut list = visible_skills.get();
        if query.is_empty() && category_filter.is_none() {
            return list;
        }
        list.retain(|skill| {
            let source_kind = format!("{:?}", skill.source.kind).to_lowercase();
            let matches_query = query.is_empty()
                || skill.name.to_lowercase().contains(&query)
                || skill.title.to_lowercase().contains(&query)
                || skill.summary.to_lowercase().contains(&query)
                || source_kind.contains(&query)
                || skill
                    .category
                    .as_ref()
                    .is_some_and(|category| category.to_lowercase().contains(&query));
            let matches_category = category_filter
                .as_ref()
                .is_none_or(|selected| skill.category.as_deref() == Some(selected.as_str()));
            matches_query && matches_category
        });
        list
    });

    let category_counts = Memo::new(move |_| {
        let mut counts = BTreeMap::<String, usize>::new();
        for skill in visible_skills.get() {
            if let Some(category) = skill.category {
                *counts.entry(category).or_insert(0) += 1;
            }
        }
        counts.into_iter().collect::<Vec<_>>()
    });

    view! {
        <div class="blx-sr-pane" role="region" aria-label=move || i18n.tr(I18nKey::TabSkills)()>
            <header class="blx-sr-pane__header">
                <div class="blx-sr-pane__title-wrap">
                    <span class="blx-sr-pane__title-icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuSparkles width="14px" height="14px" />
                    </span>
                    <h2 class="blx-sr-pane__title">{i18n.tr(I18nKey::TabSkills)}</h2>
                </div>
                <div class="blx-sr-pane__actions">
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--primary"
                        on:click=move |_| install_open.set(true)
                    >
                        <LxIcon icon=icondata::LuPlus width="13px" height="13px" />
                        <span>{i18n.tr(I18nKey::SrInstallSkill)}</span>
                    </button>
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--icon"
                        aria-label=move || i18n.tr(I18nKey::SrRefresh)()
                        title=move || i18n.tr(I18nKey::SrRefresh)()
                        on:click=move |_| svc.refresh_skills(wb)
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="13px" height="13px" />
                    </button>
                </div>
            </header>

            <Show when=move || !category_counts.with(|categories| categories.is_empty())>
                <div class="blx-sr-tabs blx-sr-category-filter" role="tablist" aria-label="Filter skills by category">
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
                        <span class="blx-sr-tab__count">{move || visible_skills.with(|s| s.len())}</span>
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
                <label class="blx-sr-search">
                    <span class="blx-sr-search__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuSearch width="14px" height="14px" />
                    </span>
                    <input
                        type="search"
                        class="blx-sr-search__input"
                        placeholder="Search skills..."
                        aria-label="Search skills"
                        prop:value=move || search_query.get()
                        on:input=move |ev| search_query.set(input_value(&ev))
                    />
                </label>
                {move || {
                    let visible = filtered_skills.get();
                    if loading.get() && skills.with(|r| r.is_empty()) {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrLoading)}</p> }
                            .into_any()
                    } else if visible_skills.with(|s| s.is_empty()) {
                        view! {
                            <div class="blx-sr-empty">
                                <span class="blx-sr-empty__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuPackagePlus width="22px" height="22px" />
                                </span>
                                <p class="blx-sr-empty__text">{i18n.tr(I18nKey::SrSkillsEmpty)}</p>
                                <button
                                    type="button"
                                    class="blx-sr-btn blx-sr-btn--primary"
                                    on:click=move |_| install_open.set(true)
                                >
                                    <LxIcon icon=icondata::LuPlus width="13px" height="13px" />
                                    <span>{i18n.tr(I18nKey::SrInstallSkill)}</span>
                                </button>
                            </div>
                        }
                        .into_any()
                    } else if (!search_query.with(|q| q.trim().is_empty())
                        || selected_category.with(|category| category.is_some()))
                        && visible.is_empty()
                    {
                        view! { <p class="blx-sr-pane__hint">"No skills match this search."</p> }
                            .into_any()
                    } else {
                        view! {
                            <For
                                each=move || filtered_skills.get()
                                key=|s| s.name.clone()
                                children=move |entry| view! { <SkillCard entry=entry /> }
                            />
                        }
                        .into_any()
                    }
                }}
            </div>
            <SkillInstallDialog open=install_open />
        </div>
    }
}

fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}
