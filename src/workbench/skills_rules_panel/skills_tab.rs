//! Right-panel tab body for `.agents/skills/<name>/`. Contains the install
//! button which opens [`SkillInstallDialog`].

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
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

    Effect::new(move |_| {
        if active_tab.get() == RightPanelTab::Skills {
            let _ = active_id.get();
            svc.refresh_skills(wb);
        }
    });

    view! {
        <div class="blx-sr-pane" role="region" aria-label=move || i18n.tr(I18nKey::TabSkills)()>
            <header class="blx-sr-pane__header">
                <h2 class="blx-sr-pane__title">{i18n.tr(I18nKey::TabSkills)}</h2>
                <div class="blx-sr-pane__actions">
                    <button
                        type="button"
                        class="blx-sr-btn"
                        on:click=move |_| install_open.set(true)
                    >
                        <LxIcon icon=icondata::LuPlus width="14px" height="14px" />
                        <span>{i18n.tr(I18nKey::SrInstallSkill)}</span>
                    </button>
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--ghost"
                        aria-label=move || i18n.tr(I18nKey::SrRefresh)()
                        on:click=move |_| svc.refresh_skills(wb)
                    >
                        <LxIcon icon=icondata::LuRefreshCw width="14px" height="14px" />
                    </button>
                </div>
            </header>
            <div class="blx-sr-pane__body">
                {move || error.get().map(|e| view! { <p class="blx-sr-pane__err">{e}</p> })}
                {move || {
                    if loading.get() && skills.with(|r| r.is_empty()) {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrLoading)}</p> }
                            .into_any()
                    } else if skills.with(|r| r.is_empty()) {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrSkillsEmpty)}</p> }
                            .into_any()
                    } else {
                        view! {
                            <For
                                each=move || skills.get()
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
