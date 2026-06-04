//! Plugins settings pane (Settings -> Plugins).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    plugins_install_from_github, plugins_install_progress, plugins_list, plugins_remove,
    plugins_set_enabled, PluginInstallKind, PluginInstallProgress, PluginInstallRequest,
    PluginRegistry, PluginRegistryEntry,
};
use crate::workbench::SettingsPaneHeader;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn PluginsSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let registry = RwSignal::new(PluginRegistry::default());
    let status = RwSignal::new(String::new());
    let install_open = RwSignal::new(false);
    let install_url = RwSignal::new(String::new());
    let install_ref = RwSignal::new(String::new());
    let install_dir = RwSignal::new(String::new());
    let install_progress = RwSignal::new(None::<PluginInstallProgress>);
    let installing = RwSignal::new(false);

    let reload = move || {
        leptos::task::spawn_local(async move {
            match plugins_list().await {
                Ok(next) => {
                    registry.set(next);
                    status.set(String::new());
                }
                Err(err) => status.set(err),
            }
        });
    };
    Effect::new(move |_| reload());

    let on_install = move |_| {
        let request = PluginInstallRequest {
            url: install_url.get_untracked(),
            git_ref: optional_text(install_ref.get_untracked()),
            package_dir: optional_text(install_dir.get_untracked()),
        };
        installing.set(true);
        install_progress.set(None);
        status.set(String::new());

        leptos::task::spawn_local(async move {
            while installing.get_untracked() {
                if let Ok(progress) = plugins_install_progress().await {
                    install_progress.set(Some(progress));
                }
                TimeoutFuture::new(180).await;
            }
            if let Ok(progress) = plugins_install_progress().await {
                install_progress.set(Some(progress));
            }
        });

        leptos::task::spawn_local(async move {
            match plugins_install_from_github(request).await {
                Ok(next) => {
                    registry.set(next);
                    status.set(String::new());
                    install_url.set(String::new());
                    install_ref.set(String::new());
                    install_dir.set(String::new());
                }
                Err(err) => status.set(err),
            }
            installing.set(false);
        });
    };

    view! {
        <article class="harness-pane plugins-settings-pane">
            <SettingsPaneHeader
                icon=icondata::LuPackage
                title=I18nKey::HsCatPlugins
                description=I18nKey::PluginsDescription
            />

            <section class="harness-subpane">
                <div class="plugins-list-head">
                    <h4 class="harness-pane-subhead">
                        <span class="harness-pane-subhead__text">
                            {move || i18n.tr(I18nKey::PluginsHeading)()}
                        </span>
                    </h4>
                    <div class="plugins-list-head__actions">
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--ghost"
                            on:click=move |_| reload()
                        >
                            <LxIcon icon=icondata::LuRefreshCw width="0.85rem" height="0.85rem" />
                            <span>{move || i18n.tr(I18nKey::PluginsRefresh)()}</span>
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            on:click=move |_| install_open.set(true)
                        >
                            <LxIcon icon=icondata::LuPlus width="0.85rem" height="0.85rem" />
                            <span>{move || i18n.tr(I18nKey::PluginsInstall)()}</span>
                        </button>
                    </div>
                </div>

                <Show
                    when=move || !registry.get().plugins.is_empty()
                    fallback=move || view! {
                        <p class="plugins-empty">{move || i18n.tr(I18nKey::PluginsEmpty)()}</p>
                    }
                >
                    <ul class="plugins-list">
                        <For
                            each=move || registry.get().plugins
                            key=|entry| entry.manifest.id.clone()
                            children=move |entry| {
                                view! {
                                    <PluginRow
                                        entry=entry
                                        on_changed=Callback::new(move |next| registry.set(next))
                                        on_error=Callback::new(move |err| status.set(err))
                                    />
                                }
                            }
                        />
                    </ul>
                </Show>

                <Show when=move || !status.get().is_empty()>
                    <p class="plugins-status-error" role="status">{move || status.get()}</p>
                </Show>
            </section>

            <Show when=move || install_open.get()>
                <PluginInstallDialog
                    url=install_url
                    git_ref=install_ref
                    package_dir=install_dir
                    installing=installing
                    progress=install_progress
                    on_install=Callback::new(on_install)
                    on_close=Callback::new(move |_| {
                        if !installing.get_untracked() {
                            install_open.set(false);
                        }
                    })
                />
            </Show>
        </article>
    }
}

#[component]
fn PluginRow(
    entry: PluginRegistryEntry,
    on_changed: Callback<PluginRegistry>,
    on_error: Callback<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let id = entry.manifest.id.clone();
    let name = entry.manifest.name.clone();
    let version = entry.manifest.version.clone();
    let description = entry.manifest.description.clone();
    let category = entry.manifest.category.clone();
    let enabled = entry.enabled;
    let removable = entry.removable();
    let description_view = (!description.is_empty()).then(|| {
        let text = description.clone();
        view! { <p class="plugins-row__description">{text}</p> }
    });
    let remove_view = removable.then(|| {
        let id_for_remove = id.clone();
        view! {
            <button
                type="button"
                class="workbench-mini-btn workbench-mini-btn--ghost plugins-danger"
                on:click=move |_| {
                    let id = id_for_remove.clone();
                    leptos::task::spawn_local(async move {
                        match plugins_remove(id).await {
                            Ok(next) => on_changed.run(next),
                            Err(err) => on_error.run(err),
                        }
                    });
                }
            >
                <LxIcon icon=icondata::LuTrash2 width="0.82rem" height="0.82rem" />
                <span>{move || i18n.tr(I18nKey::PluginsRemove)()}</span>
            </button>
        }
    });
    let source_key = if entry.source.kind == PluginInstallKind::BuiltIn {
        I18nKey::PluginsBuiltIn
    } else {
        I18nKey::PluginsInstalled
    };
    let toggle_key = if enabled {
        I18nKey::PluginsDisable
    } else {
        I18nKey::PluginsEnable
    };
    let status_key = if enabled {
        I18nKey::PluginsEnabled
    } else {
        I18nKey::PluginsDisabled
    };

    let id_for_toggle = id.clone();
    let on_toggle = move |_| {
        let id = id_for_toggle.clone();
        leptos::task::spawn_local(async move {
            match plugins_set_enabled(id, !enabled).await {
                Ok(next) => on_changed.run(next),
                Err(err) => on_error.run(err),
            }
        });
    };

    view! {
        <li class="plugins-row" class:plugins-row--off=move || !enabled>
            <div class="plugins-row__main">
                <span class="plugins-row__icon" aria-hidden="true">
                    <LxIcon icon=plugin_category_icon(&category) width="0.95rem" height="0.95rem" />
                </span>
                <div class="plugins-row__copy">
                    <div class="plugins-row__titleline">
                        <span class="plugins-row__name">{name}</span>
                        <span class="plugins-row__version">{version}</span>
                    </div>
                    {description_view}
                    <div class="plugins-row__meta">
                        <span class="plugins-row__badge">{move || i18n.tr(plugin_category_key(&category))()}</span>
                        <span class="plugins-row__badge">{move || i18n.tr(source_key)()}</span>
                        <span class="plugins-row__status">{move || i18n.tr(status_key)()}</span>
                    </div>
                </div>
            </div>
            <div class="plugins-row__actions">
                <button
                    type="button"
                    class="plugins-toggle-btn"
                    on:click=on_toggle
                    title=move || i18n.tr(toggle_key)()
                    aria-label=move || i18n.tr(toggle_key)()
                >
                    <span
                        class="blx-switch"
                        class:blx-switch--on=move || enabled
                        aria-hidden="true"
                    >
                        <span class="blx-switch__thumb" />
                    </span>
                </button>
                {remove_view}
            </div>
        </li>
    }
}

#[component]
fn PluginInstallDialog(
    url: RwSignal<String>,
    git_ref: RwSignal<String>,
    package_dir: RwSignal<String>,
    installing: RwSignal<bool>,
    progress: RwSignal<Option<PluginInstallProgress>>,
    on_install: Callback<()>,
    on_close: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <div class="plugins-install-overlay" role="presentation">
            <div class="plugins-install" role="dialog" aria-modal="true">
                <header class="plugins-install__head">
                    <h4>{move || i18n.tr(I18nKey::PluginsInstallTitle)()}</h4>
                    <button
                        type="button"
                        class="workbench-icon-btn"
                        disabled=move || installing.get()
                        aria-label=move || i18n.tr(I18nKey::BtnClose)()
                        on:click=move |_| on_close.run(())
                    >
                        <LxIcon icon=icondata::LuX width="0.9rem" height="0.9rem" />
                    </button>
                </header>

                <label class="plugins-field">
                    <span>{move || i18n.tr(I18nKey::PluginsGithubUrl)()}</span>
                    <input
                        class="workbench-plain-input"
                        type="url"
                        autocomplete="off"
                        spellcheck="false"
                        placeholder=move || i18n.tr(I18nKey::PluginsGithubUrlPlaceholder)()
                        prop:value=move || url.get()
                        on:input=move |ev| url.set(event_target_value(&ev))
                    />
                </label>

                <div class="plugins-install__grid">
                    <label class="plugins-field">
                        <span>{move || i18n.tr(I18nKey::PluginsGitRef)()}</span>
                        <input
                            class="workbench-plain-input"
                            type="text"
                            autocomplete="off"
                            spellcheck="false"
                            prop:value=move || git_ref.get()
                            on:input=move |ev| git_ref.set(event_target_value(&ev))
                        />
                    </label>
                    <label class="plugins-field">
                        <span>{move || i18n.tr(I18nKey::PluginsPackageDir)()}</span>
                        <input
                            class="workbench-plain-input"
                            type="text"
                            autocomplete="off"
                            spellcheck="false"
                            prop:value=move || package_dir.get()
                            on:input=move |ev| package_dir.set(event_target_value(&ev))
                        />
                    </label>
                </div>

                <Show when=move || installing.get() || progress.get().is_some()>
                    <div class="plugins-progress" role="status">
                        <span class="plugins-progress__bar">
                            <span class="plugins-progress__fill" />
                        </span>
                        <span>
                            {move || {
                                progress.get()
                                    .and_then(|p| {
                                        if p.error.is_some() {
                                            p.error
                                        } else if p.phase == "done" {
                                            Some(i18n.tr(I18nKey::PluginsInstallDone)().to_string())
                                        } else {
                                            Some(i18n.tr(I18nKey::PluginsInstallProgress)().to_string())
                                        }
                                    })
                                    .unwrap_or_else(|| i18n.tr(I18nKey::PluginsInstallProgress)().to_string())
                            }}
                        </span>
                    </div>
                </Show>

                <div class="plugins-install__actions">
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--ghost"
                        disabled=move || installing.get()
                        on:click=move |_| on_close.run(())
                    >
                        {move || i18n.tr(I18nKey::BtnClose)()}
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        disabled=move || installing.get()
                        on:click=move |_| on_install.run(())
                    >
                        <LxIcon icon=icondata::LuDownload width="0.85rem" height="0.85rem" />
                        <span>{move || i18n.tr(I18nKey::PluginsInstall)()}</span>
                    </button>
                </div>
            </div>
            <button
                type="button"
                class="plugins-install-scrim"
                tabindex="-1"
                disabled=move || installing.get()
                aria-label=move || i18n.tr(I18nKey::BtnClose)()
                on:click=move |_| on_close.run(())
            ></button>
        </div>
    }
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn plugin_category_key(category: &str) -> I18nKey {
    match category {
        "runtime" => I18nKey::PluginsCategoryRuntime,
        _ => I18nKey::PluginsCategoryOther,
    }
}

fn plugin_category_icon(category: &str) -> icondata::Icon {
    match category {
        "runtime" => icondata::LuSquareTerminal,
        _ => icondata::LuPackage,
    }
}
