//! Right-panel tab body for `.agents/skills/<name>/`. Contains the install
//! button which opens [`SkillInstallDialog`], and a Core/User segmented tab
//! strip styled as a modern pill switcher.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::SkillSourceKind;
use crate::workbench::skills_rules_panel::install_dialog::SkillInstallDialog;
use crate::workbench::skills_rules_panel::skill_card::SkillCard;
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::{RightPanelTab, WorkbenchService};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SkillsView {
    Core,
    User,
}

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
    let view_mode = RwSignal::new(SkillsView::Core);

    Effect::new(move |_| {
        if active_tab.get() == RightPanelTab::Skills {
            let _ = active_id.get();
            svc.refresh_skills(wb);
        }
    });

    let filtered_skills = Signal::derive(move || {
        let mode = view_mode.get();
        skills.with(|list| {
            list.iter()
                .filter(|s| match mode {
                    SkillsView::Core => s.source.kind == SkillSourceKind::Core,
                    SkillsView::User => s.source.kind != SkillSourceKind::Core,
                })
                .cloned()
                .collect::<Vec<_>>()
        })
    });

    let core_count = Signal::derive(move || {
        skills.with(|list| {
            list.iter()
                .filter(|s| s.source.kind == SkillSourceKind::Core)
                .count()
        })
    });
    let user_count = Signal::derive(move || {
        skills.with(|list| {
            list.iter()
                .filter(|s| s.source.kind != SkillSourceKind::Core)
                .count()
        })
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
                    {move || (view_mode.get() == SkillsView::User).then(|| view! {
                        <button
                            type="button"
                            class="blx-sr-btn blx-sr-btn--primary"
                            on:click=move |_| install_open.set(true)
                        >
                            <LxIcon icon=icondata::LuPlus width="13px" height="13px" />
                            <span>{i18n.tr(I18nKey::SrInstallSkill)}</span>
                        </button>
                    })}
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

            <div class="blx-sr-tabs" role="tablist">
                <button
                    type="button"
                    role="tab"
                    class="blx-sr-tab"
                    class:blx-sr-tab--active=move || view_mode.get() == SkillsView::Core
                    aria-selected=move || if view_mode.get() == SkillsView::Core { "true" } else { "false" }
                    on:click=move |_| view_mode.set(SkillsView::Core)
                >
                    <LxIcon icon=icondata::LuBox width="13px" height="13px" />
                    <span>{i18n.tr(I18nKey::SrSkillsTabCore)}</span>
                    <span class="blx-sr-tab__count">{move || core_count.get()}</span>
                </button>
                <button
                    type="button"
                    role="tab"
                    class="blx-sr-tab"
                    class:blx-sr-tab--active=move || view_mode.get() == SkillsView::User
                    aria-selected=move || if view_mode.get() == SkillsView::User { "true" } else { "false" }
                    on:click=move |_| view_mode.set(SkillsView::User)
                >
                    <LxIcon icon=icondata::LuUser width="13px" height="13px" />
                    <span>{i18n.tr(I18nKey::SrSkillsTabUser)}</span>
                    <span class="blx-sr-tab__count">{move || user_count.get()}</span>
                </button>
            </div>

            <div class="blx-sr-pane__body">
                {move || error.get().map(|e| view! { <p class="blx-sr-pane__err">{e}</p> })}
                {move || {
                    let visible = filtered_skills.get();
                    if loading.get() && skills.with(|r| r.is_empty()) {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrLoading)}</p> }
                            .into_any()
                    } else if visible.is_empty() && view_mode.get() == SkillsView::User {
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
