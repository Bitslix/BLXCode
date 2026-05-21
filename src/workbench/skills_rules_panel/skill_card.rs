//! Single installed skill shown as a card with toggle / read / remove controls
//! and a source badge.

use leptos::prelude::*;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::{SkillEntry, SkillSourceKind};
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::WorkbenchService;

#[component]
pub fn SkillCard(entry: SkillEntry) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let name_for_toggle = entry.name.clone();
    let name_for_remove = entry.name.clone();
    let enabled = entry.enabled;
    let title = entry.title.clone();
    let summary = entry.summary.clone();
    let missing_skill_md = entry.missing_skill_md;
    let is_core = entry.source.kind == SkillSourceKind::Core;
    let source_badge = match entry.source.kind {
        SkillSourceKind::Core => "core",
        SkillSourceKind::Git => "git",
        SkillSourceKind::Npm => "npm",
        SkillSourceKind::Local => "local",
        SkillSourceKind::AgentCreated => "agent",
    };

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
        svc.set_skill_enabled(wb, name_for_toggle.clone(), !enabled);
    };
    let on_remove = move |_| {
        let msg = i18n.tr(I18nKey::SrConfirmRemove)();
        if confirm_window(&msg) {
            svc.remove_skill(wb, name_for_remove.clone());
        }
    };

    let toggle_label = if enabled {
        i18n.tr(I18nKey::SrDisable)
    } else {
        i18n.tr(I18nKey::SrEnable)
    };

    let badge_class = if is_core {
        "blx-sr-card__badge blx-sr-card__badge--core"
    } else {
        "blx-sr-card__badge"
    };

    view! {
        <article class="blx-sr-card" class:blx-sr-card--off=move || !enabled>
            <header class="blx-sr-card__header">
                <h3 class="blx-sr-card__title">{title}</h3>
                <span class=badge_class>{source_badge}</span>
                <span class=pill_class>{pill_text}</span>
            </header>
            <p class="blx-sr-card__summary">{summary}</p>
            {move || missing_skill_md.then(|| view! {
                <p class="blx-sr-card__warn">{i18n.tr(I18nKey::SrMissingSkillMd)}</p>
            })}
            <footer class="blx-sr-card__footer">
                <button type="button" class="blx-sr-btn" on:click=on_toggle>{toggle_label}</button>
                {(!is_core).then(|| view! {
                    <button type="button" class="blx-sr-btn blx-sr-btn--danger" on:click=on_remove>
                        {i18n.tr(I18nKey::SrRemove)}
                    </button>
                })}
            </footer>
        </article>
    }
}

fn confirm_window(msg: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(msg).ok())
        .unwrap_or(true)
}
