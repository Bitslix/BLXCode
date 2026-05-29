//! Banner + dialog for the rules-pointer installer.
//!
//! Mirrors the memory-pointer UI in `workbench/memory_panel.rs` but
//! drives the `rules_*` Tauri commands instead. The visual layout reuses
//! the generic `workbench-pointers-notice` and `pointers-dialog` CSS
//! classes shared with the memory panel.

use std::collections::HashSet;

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::PointerResult;
use crate::workbench::pointer_agents::{PointerAgent, POINTER_AGENTS};
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::WorkbenchService;

#[component]
pub fn RulesPointersNotice() -> impl IntoView {
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="workbench-pointers-notice" role="status">
            <span class="workbench-pointers-notice__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuInfo width="0.9rem" height="0.9rem" />
            </span>
            <p>
                {i18n.tr(I18nKey::SrPointersBannerLead)}
                <button
                    type="button"
                    class="workbench-pointers-notice__setup"
                    on:click=move |_| svc.pointers_open().set(true)
                >
                    {i18n.tr(I18nKey::SrPointersBannerCta)}
                </button>
            </p>
            <button
                type="button"
                class="workbench-pointers-notice__close"
                aria-label=move || i18n.tr(I18nKey::SrPointersClose)()
                on:click=move |_| svc.pointers_notice_dismissed().set(true)
            >
                <LxIcon icon=icondata::LuX width="0.75rem" height="0.75rem" />
            </button>
        </div>
    }
}

#[component]
pub fn RulesPointersDialog() -> impl IntoView {
    let svc = expect_context::<SkillsRulesService>();
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    view! {
        <div
            class="workspace-rename-backdrop"
            on:click=move |_| {
                if !svc.pointers_busy().get_untracked() {
                    svc.pointers_open().set(false);
                }
            }
        >
            <section
                class="workspace-rename-dialog pointers-dialog"
                role="dialog"
                aria-modal="true"
                aria-labelledby="rules-pointers-title"
                on:click=move |ev| ev.stop_propagation()
            >
                <header class="pointers-dialog__head">
                    <div class="pointers-dialog__title-icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuLink2 width="1.1rem" height="1.1rem" />
                    </div>
                    <div>
                        <h2 id="rules-pointers-title">{i18n.tr(I18nKey::SrPointersDialogTitle)}</h2>
                        <p>{i18n.tr(I18nKey::SrPointersDialogLead)}</p>
                    </div>
                    <button
                        type="button"
                        class="workspace-rename-dialog__close"
                        disabled=move || svc.pointers_busy().get()
                        on:click=move |_| svc.pointers_open().set(false)
                    >
                        "×"
                    </button>
                </header>
                <ul class="pointers-dialog__agents">
                    <For
                        each=move || POINTER_AGENTS.to_vec()
                        key=|agent| agent.id
                        children=move |agent: PointerAgent| view! {
                            <RulesPointerAgentRow agent=agent />
                        }
                    />
                </ul>
                <footer class="workspace-rename-dialog__actions pointers-dialog__actions">
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--ghost"
                        disabled=move || {
                            svc.pointers_busy().get()
                                || svc.selected_pointer_agents().with(HashSet::is_empty)
                        }
                        on:click=move |_| run_action(svc, wb, false)
                    >
                        {i18n.tr(I18nKey::SrPointersUninstall)}
                    </button>
                    <button
                        type="button"
                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                        disabled=move || {
                            svc.pointers_busy().get()
                                || svc.selected_pointer_agents().with(HashSet::is_empty)
                        }
                        on:click=move |_| run_action(svc, wb, true)
                    >
                        {i18n.tr(I18nKey::SrPointersInstall)}
                    </button>
                </footer>
            </section>
        </div>
    }
}

#[component]
fn RulesPointerAgentRow(agent: PointerAgent) -> impl IntoView {
    let svc = expect_context::<SkillsRulesService>();
    let input_id = format!("rules-pointer-agent-{}", agent.id);
    let id_for_checked = agent.id;
    let id_for_change = agent.id.to_owned();
    let id_for_status = agent.id;
    view! {
        <li>
            <label class="pointers-dialog__agent" for=input_id.clone()>
                <input
                    id=input_id.clone()
                    type="checkbox"
                    class="pointers-dialog__checkbox"
                    prop:checked=move || {
                        svc.selected_pointer_agents()
                            .with(|selected| selected.contains(id_for_checked))
                    }
                    disabled=move || svc.pointers_busy().get()
                    on:change=move |ev| {
                        let checked = ev
                            .target()
                            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                            .is_some_and(|input| input.checked());
                        svc.selected_pointer_agents().update(|selected| {
                            if checked {
                                selected.insert(id_for_change.clone());
                            } else {
                                selected.remove(&id_for_change);
                            }
                        });
                    }
                />
                <span class="pointers-dialog__brand">
                    <img src=agent.icon alt="" prop:draggable=false />
                </span>
                <span class="pointers-dialog__meta">
                    <span class="pointers-dialog__label">{agent.label}</span>
                    <span class="pointers-dialog__target">{agent.target}</span>
                </span>
                <PointerStatusBadge agent_id=id_for_status />
            </label>
        </li>
    }
}

#[component]
fn PointerStatusBadge(agent_id: &'static str) -> impl IntoView {
    let svc = expect_context::<SkillsRulesService>();
    view! {
        {move || {
            let entry = svc
                .pointer_status()
                .get()
                .and_then(|status| status.into_iter().find(|item| item.agent == agent_id));
            let (class, icon, label) = pointer_status_view(entry.as_ref());
            view! {
                <span class=class>
                    {icon.map(|icon| view! {
                        <LxIcon icon=icon width="0.7rem" height="0.7rem" />
                    })}
                    <span>{label}</span>
                </span>
            }
        }}
    }
}

fn pointer_status_view(
    entry: Option<&PointerResult>,
) -> (&'static str, Option<icondata::Icon>, &'static str) {
    if entry.is_some_and(|r| r.installed) {
        (
            "pointers-dialog__status pointers-dialog__status--installed",
            Some(icondata::LuCircleCheck),
            "Installed",
        )
    } else if entry
        .and_then(|r| r.note.as_deref())
        .is_some_and(|note| note == "file missing")
    {
        (
            "pointers-dialog__status pointers-dialog__status--missing",
            Some(icondata::LuFileWarning),
            "File missing",
        )
    } else {
        (
            "pointers-dialog__status pointers-dialog__status--pending",
            None,
            "Not installed",
        )
    }
}

fn run_action(svc: SkillsRulesService, wb: WorkbenchService, install: bool) {
    let selected = svc.selected_pointer_agents().get_untracked();
    let agents: Vec<String> = POINTER_AGENTS
        .iter()
        .filter(|agent| selected.contains(agent.id))
        .map(|agent| agent.id.to_owned())
        .collect();
    svc.run_pointer_action(wb, install, agents);
}
