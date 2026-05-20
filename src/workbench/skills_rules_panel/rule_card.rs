//! Single Markdown rule shown as a card with toggle / read / remove controls.

use leptos::prelude::*;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::RuleEntry;
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::WorkbenchService;

#[component]
pub fn RuleCard(entry: RuleEntry) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let name_for_toggle = entry.name.clone();
    let name_for_remove = entry.name.clone();
    let name_for_read = entry.name.clone();
    let enabled = entry.enabled;
    let title = entry.title.clone();
    let summary = entry.summary.clone();
    let pill_class = if enabled {
        "blx-sr-card__status blx-sr-card__status--on"
    } else {
        "blx-sr-card__status blx-sr-card__status--off"
    };
    let pill_text = if enabled {
        i18n.tr(I18nKey::SrStatusEnabled)
    } else {
        i18n.tr(I18nKey::SrStatusDisabled)
    };

    let on_toggle = move |_| {
        svc.set_rule_enabled(wb, name_for_toggle.clone(), !enabled);
    };
    let on_remove = move |_| {
        let confirm_msg = i18n.tr(I18nKey::SrConfirmRemove)();
        if confirm_window(&confirm_msg) {
            svc.remove_rule(wb, name_for_remove.clone());
        }
    };
    let on_read = move |_| {
        // The agent panel reads rules via the agent tool; in the UI we keep
        // the action minimal and reuse browser-style "open in editor" later.
        // For now, log to the console so wiring is obvious during smoke.
        let _ = name_for_read;
    };

    let toggle_label = if enabled {
        i18n.tr(I18nKey::SrDisable)
    } else {
        i18n.tr(I18nKey::SrEnable)
    };

    view! {
        <article class="blx-sr-card" class:blx-sr-card--off=move || !enabled>
            <header class="blx-sr-card__header">
                <h3 class="blx-sr-card__title">{title}</h3>
                <span class=pill_class>{pill_text}</span>
            </header>
            <p class="blx-sr-card__summary">{summary}</p>
            <footer class="blx-sr-card__footer">
                <button type="button" class="blx-sr-btn" on:click=on_toggle>{toggle_label}</button>
                <button type="button" class="blx-sr-btn blx-sr-btn--ghost" on:click=on_read>
                    {i18n.tr(I18nKey::SrRead)}
                </button>
                <button type="button" class="blx-sr-btn blx-sr-btn--danger" on:click=on_remove>
                    {i18n.tr(I18nKey::SrRemove)}
                </button>
            </footer>
        </article>
    }
}

fn confirm_window(msg: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(msg).ok())
        .unwrap_or(true)
}
