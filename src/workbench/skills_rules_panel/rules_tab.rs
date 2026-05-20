//! Right-panel tab body for `.agents/rules/*.md`.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
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
                <h2 class="blx-sr-pane__title">{i18n.tr(I18nKey::TabRules)}</h2>
                <button
                    type="button"
                    class="blx-sr-btn blx-sr-btn--ghost"
                    aria-label=move || i18n.tr(I18nKey::SrRefresh)()
                    on:click=move |_| svc.refresh_rules(wb)
                >
                    <LxIcon icon=icondata::LuRefreshCw width="14px" height="14px" />
                </button>
            </header>
            <div class="blx-sr-pane__body">
                {move || error.get().map(|e| view! { <p class="blx-sr-pane__err">{e}</p> })}
                {move || {
                    if loading.get() && rules.with(|r| r.is_empty()) {
                        view! { <p class="blx-sr-pane__hint">{i18n.tr(I18nKey::SrLoading)}</p> }
                            .into_any()
                    } else if rules.with(|r| r.is_empty()) {
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
