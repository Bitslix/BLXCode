//! Modal dialog for installing a new skill from `git`, `npm`, or a local
//! workspace path. Submits to [`SkillsRulesService::install_skill`].

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::skills_rules_wire::{SkillSourceInput, SkillSourceKind};
use crate::workbench::skills_rules_panel::SkillsRulesService;
use crate::workbench::WorkbenchService;

#[component]
pub fn SkillInstallDialog(open: RwSignal<bool>) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let svc = expect_context::<SkillsRulesService>();
    let i18n = expect_context::<I18nService>();

    let name = RwSignal::new(String::new());
    let kind = RwSignal::new(SkillSourceKind::Git);
    let git_url = RwSignal::new(String::new());
    let git_ref = RwSignal::new(String::new());
    let npm_package = RwSignal::new(String::new());
    let npm_version = RwSignal::new(String::new());
    let local_path = RwSignal::new(String::new());

    let busy = svc.install_busy();
    let err = svc.install_error();

    let close = move || {
        open.set(false);
        err.set(None);
    };

    let submit = move |_| {
        let n = name.get().trim().to_owned();
        if n.is_empty() {
            err.set(Some(i18n.tr(I18nKey::SrSkillNameLabel)().to_string()));
            return;
        }
        let source = match kind.get() {
            SkillSourceKind::Git => SkillSourceInput {
                kind: SkillSourceKind::Git,
                url: Some(git_url.get().trim().to_owned()),
                git_ref: Some(git_ref.get().trim().to_owned()).filter(|s| !s.is_empty()),
                package: None,
                version: None,
                path: None,
            },
            SkillSourceKind::Npm => SkillSourceInput {
                kind: SkillSourceKind::Npm,
                url: None,
                git_ref: None,
                package: Some(npm_package.get().trim().to_owned()),
                version: Some(npm_version.get().trim().to_owned()).filter(|s| !s.is_empty()),
                path: None,
            },
            SkillSourceKind::Local => SkillSourceInput {
                kind: SkillSourceKind::Local,
                url: None,
                git_ref: None,
                package: None,
                version: None,
                path: Some(local_path.get().trim().to_owned()),
            },
            SkillSourceKind::AgentCreated | SkillSourceKind::Core => return,
        };
        svc.install_skill(wb, n, source, move |result| {
            if result.is_ok() {
                open.set(false);
            }
        });
    };

    let dynamic_fields = move || match kind.get() {
        SkillSourceKind::Git => view! {
            <div class="blx-sr-dialog__row">
                <label>{i18n.tr(I18nKey::SrGitUrlLabel)}</label>
                <input
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::SrGitUrlPh)()
                    prop:value=move || git_url.get()
                    on:input=move |ev| git_url.set(read_input(&ev))
                />
            </div>
            <div class="blx-sr-dialog__row">
                <label>{i18n.tr(I18nKey::SrGitRefLabel)}</label>
                <input
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::SrGitRefPh)()
                    prop:value=move || git_ref.get()
                    on:input=move |ev| git_ref.set(read_input(&ev))
                />
            </div>
        }
        .into_any(),
        SkillSourceKind::Npm => view! {
            <div class="blx-sr-dialog__row">
                <label>{i18n.tr(I18nKey::SrNpmPackageLabel)}</label>
                <input
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::SrNpmPackagePh)()
                    prop:value=move || npm_package.get()
                    on:input=move |ev| npm_package.set(read_input(&ev))
                />
            </div>
            <div class="blx-sr-dialog__row">
                <label>{i18n.tr(I18nKey::SrNpmVersionLabel)}</label>
                <input
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::SrNpmVersionPh)()
                    prop:value=move || npm_version.get()
                    on:input=move |ev| npm_version.set(read_input(&ev))
                />
            </div>
        }
        .into_any(),
        SkillSourceKind::Local => view! {
            <div class="blx-sr-dialog__row">
                <label>{i18n.tr(I18nKey::SrLocalPathLabel)}</label>
                <input
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::SrLocalPathPh)()
                    prop:value=move || local_path.get()
                    on:input=move |ev| local_path.set(read_input(&ev))
                />
            </div>
        }
        .into_any(),
        SkillSourceKind::AgentCreated | SkillSourceKind::Core => view! { <></> }.into_any(),
    };

    view! {
        <Show when=move || open.get() fallback=|| ()>
            <div class="blx-sr-dialog__backdrop" on:click=move |_| close()></div>
            <div class="blx-sr-dialog" role="dialog" aria-modal="true">
                <header class="blx-sr-dialog__header">
                    <h2>{i18n.tr(I18nKey::SrInstallSkill)}</h2>
                </header>
                <div class="blx-sr-dialog__body">
                    <div class="blx-sr-dialog__row">
                        <label>{i18n.tr(I18nKey::SrSkillNameLabel)}</label>
                        <input
                            type="text"
                            placeholder=move || i18n.tr(I18nKey::SrSkillNamePh)()
                            prop:value=move || name.get()
                            on:input=move |ev| name.set(read_input(&ev))
                        />
                    </div>
                    <div class="blx-sr-dialog__row">
                        <label>{i18n.tr(I18nKey::SrSourceKind)}</label>
                        <div class="blx-sr-dialog__segmented" role="tablist">
                            <button
                                type="button"
                                class="blx-sr-seg"
                                class:blx-sr-seg--active=move || kind.get() == SkillSourceKind::Git
                                on:click=move |_| kind.set(SkillSourceKind::Git)
                            >
                                {i18n.tr(I18nKey::SrSourceGit)}
                            </button>
                            <button
                                type="button"
                                class="blx-sr-seg"
                                class:blx-sr-seg--active=move || kind.get() == SkillSourceKind::Npm
                                on:click=move |_| kind.set(SkillSourceKind::Npm)
                            >
                                {i18n.tr(I18nKey::SrSourceNpm)}
                            </button>
                            <button
                                type="button"
                                class="blx-sr-seg"
                                class:blx-sr-seg--active=move || kind.get() == SkillSourceKind::Local
                                on:click=move |_| kind.set(SkillSourceKind::Local)
                            >
                                {i18n.tr(I18nKey::SrSourceLocal)}
                            </button>
                        </div>
                    </div>
                    {dynamic_fields}
                    {move || err.get().map(|e| view! { <p class="blx-sr-dialog__err">{e}</p> })}
                </div>
                <footer class="blx-sr-dialog__footer">
                    <button
                        type="button"
                        class="blx-sr-btn blx-sr-btn--ghost"
                        on:click=move |_| close()
                        disabled=move || busy.get()
                    >
                        {i18n.tr(I18nKey::SrCancel)}
                    </button>
                    <button
                        type="button"
                        class="blx-sr-btn"
                        on:click=submit
                        disabled=move || busy.get()
                    >
                        {move || if busy.get() {
                            i18n.tr(I18nKey::SrInstalling)()
                        } else {
                            i18n.tr(I18nKey::SrInstall)()
                        }}
                    </button>
                </footer>
            </div>
        </Show>
    }
}

fn read_input(ev: &leptos::ev::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}
