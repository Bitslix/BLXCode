//! Workspace settings pane — same harness layout as App / API Keys.

use super::browser_tab::sync_embedded_browser_layer;
use super::state::{BrowserEmbedSurface, WorkbenchService};
use crate::config::HARNESS_BROWSER_DEFAULT_URL;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq, Eq)]
struct WorkspaceBaseline {
    project_dir: String,
    sandbox_root: String,
    browser_url: String,
}

fn input_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

fn snapshot_baseline(wb: &WorkbenchService) -> WorkspaceBaseline {
    WorkspaceBaseline {
        project_dir: wb.default_project_dir().get_untracked(),
        sandbox_root: wb.harness_workspace_root().get_untracked(),
        browser_url: wb.browser_url().get_untracked(),
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
pub fn WorkspaceSettingsPane(
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let baseline = RwSignal::new(snapshot_baseline(&wb));
    let status_msg = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);

    let dirty = Memo::new(move |_| {
        let b = baseline.get();
        wb.default_project_dir().get() != b.project_dir
            || wb.harness_workspace_root().get() != b.sandbox_root
            || wb.browser_url().get() != b.browser_url
    });

    let save = move || {
        if !dirty.get_untracked() {
            return;
        }
        busy.set(true);
        status_msg.set(None);
        let b = baseline.get_untracked();
        let project = wb.default_project_dir().get_untracked().trim().to_owned();
        let sandbox = wb.harness_workspace_root().get_untracked().trim().to_owned();
        let browser = wb.browser_url().get_untracked();

        if project != b.project_dir {
            wb.persist_default_project_dir(project);
        }
        if sandbox != b.sandbox_root {
            wb.persist_harness_workspace_root(sandbox);
        }
        if browser != b.browser_url {
            apply_browser_url(wb, embed, browser);
        } else if sandbox != b.sandbox_root {
            let w = wb;
            leptos::task::spawn_local(async move {
                TimeoutFuture::new(8).await;
                sync_embedded_browser_layer(w, embed).await;
            });
        }

        baseline.set(snapshot_baseline(&wb));
        status_msg.set(Some(i18n.tr(I18nKey::ApiKeysSaved)().to_string()));
        busy.set(false);
    };

    let discard = move || {
        let b = baseline.get_untracked();
        wb.set_default_project_dir_text(b.project_dir);
        wb.set_harness_workspace_root_text(b.sandbox_root);
        wb.set_browser_url_text(b.browser_url);
        status_msg.set(None);
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
                <div class="settings-field-list">
                    <div class="settings-field-card">
                        <label class="harness-stack">
                            <span class="harness-field-label">
                                <span class="harness-field-label__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuFolderGit2 width="0.82rem" height="0.82rem" />
                                </span>
                                <span class="harness-field-label__text">
                                    {move || i18n.tr(I18nKey::WsDefaultProjectDirLabel)()}
                                </span>
                            </span>
                            <input
                                class="workbench-plain-input settings-field-card__input"
                                type="text"
                                placeholder=move || i18n.tr(I18nKey::WsDefaultProjectDirPlaceholder)()
                                prop:value=move || wb.default_project_dir().get()
                                on:input=move |ev| {
                                    if let Some(txt) = input_str(&ev) {
                                        wb.set_default_project_dir_text(txt);
                                    }
                                }
                            />
                            <small class="harness-muted">
                                {move || i18n.tr(I18nKey::WsDefaultProjectDirHint)()}
                            </small>
                        </label>
                    </div>
                    <div class="settings-field-card">
                        <label class="harness-stack">
                            <span class="harness-field-label">
                                <span class="harness-field-label__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuShield width="0.82rem" height="0.82rem" />
                                </span>
                                <span class="harness-field-label__text">
                                    {move || i18n.tr(I18nKey::WsRootLabel)()}
                                </span>
                            </span>
                            <input
                                class="workbench-plain-input settings-field-card__input"
                                type="text"
                                placeholder=move || i18n.tr(I18nKey::WsRootPlaceholder)()
                                prop:value=move || wb.harness_workspace_root().get()
                                on:input=move |ev| {
                                    if let Some(txt) = input_str(&ev) {
                                        wb.set_harness_workspace_root_text(txt);
                                    }
                                }
                            />
                            <small class="harness-muted">{move || i18n.tr(I18nKey::WsRootHint)()}</small>
                        </label>
                    </div>
                </div>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuGlobe width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::WsSectionBrowser)()}</span>
                </h4>
                <div class="settings-field-list">
                    <div class="settings-field-card">
                        <label class="harness-stack">
                            <span class="harness-field-label">
                                <span class="harness-field-label__text">
                                    {move || i18n.tr(I18nKey::LayBrowserUrl)()}
                                </span>
                            </span>
                            <input
                                class="workbench-plain-input settings-field-card__input"
                                type="url"
                                prop:value=move || wb.browser_url().get()
                                on:input=move |ev| {
                                    if let Some(txt) = input_str(&ev) {
                                        wb.set_browser_url_text(txt);
                                    }
                                }
                            />
                            <small class="harness-muted">
                                {move || format!(
                                    "{} {}",
                                    i18n.tr(I18nKey::WsBrowserDefault)(),
                                    HARNESS_BROWSER_DEFAULT_URL
                                )}
                            </small>
                        </label>
                    </div>
                </div>
            </section>

            <Show when=move || status_msg.with(|m| m.is_some())>
                <p class="harness-status">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>

            <footer class="settings-pane-footer harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    prop:disabled=move || busy.get() || !dirty.get()
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
                    prop:disabled=move || busy.get() || !dirty.get()
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
