//! Workspace settings pane — same row/list layout as API Keys (`api-keys-row`).

mod category_colors;

use super::app_prefs::AppPrefsService;
use super::browser_tab::sync_embedded_browser_layer;
use super::state::{BrowserEmbedSurface, WorkbenchService};
use crate::config::HARNESS_BROWSER_DEFAULT_URL;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use category_colors::WorkspaceCategoryColorsSection;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq, Eq)]
struct WorkspaceBaseline {
    project_dir: String,
    sandbox_root: String,
    browser_url: String,
    architecture_llm_prose: bool,
}

fn input_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.checked())
}

fn snapshot_baseline(wb: &WorkbenchService) -> WorkspaceBaseline {
    let architecture_llm_prose = wb
        .active_id()
        .get_untracked()
        .map(|id| wb.architecture_llm_prose_for_workspace_untracked(id))
        .unwrap_or(false);
    WorkspaceBaseline {
        project_dir: wb.default_project_dir().get_untracked(),
        sandbox_root: wb.harness_workspace_root().get_untracked(),
        browser_url: wb.browser_url().get_untracked(),
        architecture_llm_prose,
    }
}

fn apply_browser_url(wb: WorkbenchService, embed: BrowserEmbedSurface, url: String) {
    let mut trimmed = url.trim().to_owned();
    if trimmed.is_empty() {
        trimmed = HARNESS_BROWSER_DEFAULT_URL.into();
    }
    wb.persist_browser_url_from_input(trimmed.clone());
    let aid = wb.embedded_browser_active_id().get_untracked();
    leptos::task::spawn_local(async move {
        let _ = crate::tauri_bridge::browser_navigate(aid, trimmed.as_str()).await;
        TimeoutFuture::new(12).await;
        sync_embedded_browser_layer(wb, embed).await;
    });
}

#[component]
pub fn WorkspaceSettingsPane(wb: WorkbenchService, embed: BrowserEmbedSurface) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let baseline = RwSignal::new(snapshot_baseline(&wb));
    let status_msg = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    let dirty = Memo::new(move |_| {
        let b = baseline.get();
        let architecture_llm_prose = wb
            .active_id()
            .get()
            .map(|id| wb.architecture_llm_prose_for_workspace(id))
            .unwrap_or(false);
        wb.default_project_dir().get() != b.project_dir
            || wb.harness_workspace_root().get() != b.sandbox_root
            || wb.browser_url().get() != b.browser_url
            || architecture_llm_prose != b.architecture_llm_prose
    });

    let save = move || {
        if !dirty.get_untracked() {
            return;
        }
        busy.set(true);
        status_msg.set(None);
        let b = baseline.get_untracked();
        let project = wb.default_project_dir().get_untracked().trim().to_owned();
        let sandbox = wb
            .harness_workspace_root()
            .get_untracked()
            .trim()
            .to_owned();
        let browser = wb.browser_url().get_untracked();

        if project != b.project_dir {
            wb.persist_default_project_dir(project);
        }
        let sandbox_changed = sandbox != b.sandbox_root;
        if sandbox_changed {
            wb.persist_harness_workspace_root(sandbox);
        }
        if browser != b.browser_url {
            apply_browser_url(wb, embed, browser);
        } else if sandbox_changed {
            let w = wb;
            leptos::task::spawn_local(async move {
                TimeoutFuture::new(8).await;
                sync_embedded_browser_layer(w, embed).await;
            });
        }
        if let Some(id) = wb.active_id().get_untracked() {
            let enabled = wb.architecture_llm_prose_for_workspace_untracked(id);
            if enabled != b.architecture_llm_prose {
                wb.set_workspace_architecture_llm_prose(id, enabled);
            }
        }

        baseline.set(snapshot_baseline(&wb));
        status_msg.set(Some(i18n.tr(I18nKey::ApiKeysSaved)().to_string()));
        busy.set(false);
    };

    let discard = move || {
        if !dirty.get_untracked() {
            return;
        }
        let b = baseline.get_untracked();
        wb.set_default_project_dir_text(b.project_dir);
        wb.set_harness_workspace_root_text(b.sandbox_root);
        wb.set_browser_url_text(b.browser_url);
        if let Some(id) = wb.active_id().get_untracked() {
            wb.set_workspace_architecture_llm_prose(id, b.architecture_llm_prose);
        }
        status_msg.set(None);
    };

    let on_project_input = move |ev: web_sys::Event| {
        if let Some(txt) = input_str(&ev) {
            wb.set_default_project_dir_text(txt);
        }
    };
    let on_sandbox_input = move |ev: web_sys::Event| {
        if let Some(txt) = input_str(&ev) {
            wb.set_harness_workspace_root_text(txt);
        }
    };
    let on_browser_input = move |ev: web_sys::Event| {
        if let Some(txt) = input_str(&ev) {
            wb.set_browser_url_text(txt);
        }
    };
    let on_architecture_prose_change = move |ev: web_sys::Event| {
        let Some(id) = wb.active_id().get_untracked() else {
            return;
        };
        if let Some(checked) = checkbox_checked(&ev) {
            wb.set_workspace_architecture_llm_prose(id, checked);
        }
    };

    view! {
        <article class="harness-pane workspace-settings-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuFolderOpen width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::WsHeading)()}</span>
            </h3>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuFolderTree width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::WsSectionPaths)()}</span>
                </h4>
                <ul class="api-keys-list">
                    <li class="settings-field-card api-keys-row">
                        <div class="api-keys-row__head">
                            <span class="api-keys-row__label">{move || i18n.tr(I18nKey::WsDefaultProjectDirLabel)()}</span>
                        </div>
                        <div class="api-keys-row__body harness-row-gap">
                            <input
                                class="workbench-plain-input api-keys-row__input workspace-field-row__input"
                                type="text"
                                placeholder=move || i18n.tr(I18nKey::WsDefaultProjectDirPlaceholder)()
                                prop:value=move || wb.default_project_dir().get()
                                on:input=on_project_input
                            />
                            <span></span>
                        </div>
                        <p class="harness-muted api-keys-row__hint">{move || i18n.tr(I18nKey::WsDefaultProjectDirHint)()}</p>
                    </li>
                    <li class="settings-field-card api-keys-row">
                        <div class="api-keys-row__head">
                            <span class="api-keys-row__label">{move || i18n.tr(I18nKey::WsRootLabel)()}</span>
                        </div>
                        <div class="api-keys-row__body harness-row-gap">
                            <input
                                class="workbench-plain-input api-keys-row__input workspace-field-row__input"
                                type="text"
                                placeholder=move || i18n.tr(I18nKey::WsRootPlaceholder)()
                                prop:value=move || wb.harness_workspace_root().get()
                                on:input=on_sandbox_input
                            />
                            <span></span>
                        </div>
                        <p class="harness-muted api-keys-row__hint">{move || i18n.tr(I18nKey::WsRootHint)()}</p>
                    </li>
                </ul>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuGlobe width="0.9rem" height="0.9rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::WsSectionBrowser)()}</span>
                </h4>
                <ul class="api-keys-list">
                    <li class="settings-field-card api-keys-row">
                        <div class="api-keys-row__head">
                            <span class="api-keys-row__label">{move || i18n.tr(I18nKey::LayBrowserUrl)()}</span>
                        </div>
                        <div class="api-keys-row__body harness-row-gap">
                            <input
                                class="workbench-plain-input api-keys-row__input"
                                type="url"
                                prop:value=move || wb.browser_url().get()
                                on:input=on_browser_input
                            />
                            <span></span>
                        </div>
                        <p class="harness-muted api-keys-row__hint">
                            {move || {
                                format!(
                                    "{} {}",
                                    i18n.tr(I18nKey::WsBrowserDefault)(),
                                    HARNESS_BROWSER_DEFAULT_URL
                                )
                            }}
                        </p>
                    </li>
                </ul>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuTriangleAlert width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::WsSectionConfirm)()}</span>
                </h4>
                <label class="app-prefs-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || prefs.confirm_close_workspace_enabled().get()
                        on:change=move |ev| {
                            if let Some(checked) = checkbox_checked(&ev) {
                                prefs.set_confirm_close_workspace(checked);
                            }
                        }
                    />
                    <span>{move || i18n.tr(I18nKey::WsConfirmCloseLabel)()}</span>
                </label>
                <p class="app-prefs-hint">{move || i18n.tr(I18nKey::WsConfirmCloseHint)()}</p>
            </section>

            <WorkspaceCategoryColorsSection wb=wb />

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuNetwork width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">"Architecture map"</span>
                </h4>
                <label class="app-prefs-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || {
                            wb.active_id()
                                .get()
                                .map(|id| wb.architecture_llm_prose_for_workspace(id))
                                .unwrap_or(false)
                        }
                        on:change=on_architecture_prose_change
                    />
                    <span>"LLM prose ingest"</span>
                </label>
                <p class="app-prefs-hint">
                    "Default off. Rebuilds stay deterministic; enabling this only permits future explicit prose synthesis into manual architecture sections."
                </p>
            </section>

            <Show when=move || status_msg.with(|m| m.is_some())>
                <p class="harness-status">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>

            <footer class="settings-pane-footer harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    disabled=move || busy.get() || !dirty.get()
                    on:click=move |_| save()
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::BtnSave)()}</span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    disabled=move || busy.get() || !dirty.get()
                    on:click=move |_| discard()
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuUndo2 width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::ApiKeysDiscard)()}</span>
                    </span>
                </button>
                <Show when=move || dirty.get()>
                    <span class="settings-pane-dirty harness-muted">
                        {move || i18n.tr(I18nKey::ApiKeysUnsaved)()}
                    </span>
                </Show>
            </footer>
        </article>
    }
}
