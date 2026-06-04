//! Memory settings pane.

use super::app_prefs::AppPrefsService;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, is_tauri_shell, memory_index_settings_get, memory_index_settings_save,
    memory_index_stats, AgentProviderKind, MemoryIndexSettings, ProviderModelEntry,
};
use crate::workbench::agent_model_picker::AgentModelPicker;
use crate::workbench::SettingsPaneHeader;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|input| input.checked())
}

fn select_value(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlSelectElement>()
        .ok()
        .map(|input| input.value())
}

fn provider_from_str(raw: &str) -> Option<AgentProviderKind> {
    Some(match raw {
        "openrouter" => AgentProviderKind::Openrouter,
        "anthropic" => AgentProviderKind::Anthropic,
        "openai" => AgentProviderKind::Openai,
        "ollama" => AgentProviderKind::Ollama,
        "lmStudio" => AgentProviderKind::LmStudio,
        "huggingFace" => AgentProviderKind::HuggingFace,
        "cloudflare" => AgentProviderKind::Cloudflare,
        "together" => AgentProviderKind::Together,
        "portkey" => AgentProviderKind::Portkey,
        _ => return None,
    })
}

fn provider_label(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "OpenRouter",
        AgentProviderKind::Anthropic => "Anthropic",
        AgentProviderKind::Openai => "OpenAI",
        AgentProviderKind::Ollama => "Ollama",
        AgentProviderKind::LmStudio => "LM Studio",
        AgentProviderKind::HuggingFace => "Hugging Face",
        AgentProviderKind::Cloudflare => "Cloudflare",
        AgentProviderKind::Together => "Together",
        AgentProviderKind::Portkey => "Portkey",
    }
}

fn providers() -> [AgentProviderKind; 9] {
    [
        AgentProviderKind::Ollama,
        AgentProviderKind::LmStudio,
        AgentProviderKind::Openrouter,
        AgentProviderKind::Anthropic,
        AgentProviderKind::Openai,
        AgentProviderKind::HuggingFace,
        AgentProviderKind::Cloudflare,
        AgentProviderKind::Together,
        AgentProviderKind::Portkey,
    ]
}

#[component]
pub fn MemorySettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let index_provider = RwSignal::new(AgentProviderKind::Openrouter);
    let index_model = RwSignal::new(String::new());
    let model_entries = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let loading_models = RwSignal::new(false);
    let stats_line = RwSignal::new(String::from("No index run yet."));
    let status = RwSignal::new(None::<String>);
    let error = RwSignal::new(None::<String>);

    let load_models = move |provider: AgentProviderKind| {
        loading_models.set(true);
        leptos::task::spawn_local(async move {
            match agent_provider_models(provider).await {
                Ok(resp) => {
                    if index_model.get_untracked().trim().is_empty() {
                        if let Some(first) = resp.entries.first() {
                            index_model.set(first.id.clone());
                        }
                    }
                    model_entries.set(resp.entries);
                }
                Err(err) => error.set(Some(err)),
            }
            loading_models.set(false);
        });
    };

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match memory_index_settings_get().await {
                Ok(settings) => {
                    index_provider.set(settings.provider);
                    index_model.set(settings.model_id);
                    load_models(settings.provider);
                }
                Err(err) => error.set(Some(err)),
            }
            match memory_index_stats().await {
                Ok(stats) => {
                    let warning_suffix = if stats.warnings.is_empty() {
                        String::new()
                    } else {
                        format!(", {} warning(s)", stats.warnings.len())
                    };
                    let last = stats
                        .last_indexed_at
                        .map(|ts| ts.to_string())
                        .unwrap_or_else(|| "never".into());
                    stats_line.set(format!(
                        "{} workspace notes, {} global notes, {} generated files, last indexed: {}{}",
                        stats.workspace_count,
                        stats.global_count,
                        stats.generated_files.len(),
                        last,
                        warning_suffix
                    ));
                }
                Err(err) => error.set(Some(err)),
            }
        });
    });

    let save_index_settings = move || {
        status.set(None);
        error.set(None);
        let settings = MemoryIndexSettings {
            provider: index_provider.get_untracked(),
            model_id: index_model.get_untracked(),
        };
        leptos::task::spawn_local(async move {
            match memory_index_settings_save(settings).await {
                Ok(saved) => {
                    index_provider.set(saved.provider);
                    index_model.set(saved.model_id);
                    status.set(Some("Saved.".into()));
                }
                Err(err) => error.set(Some(err)),
            }
        });
    };

    view! {
        <article class="harness-pane memory-settings-pane">
            <SettingsPaneHeader
                icon=icondata::LuLayers
                title=I18nKey::TabMemory
                description=I18nKey::MemorySettingsDescription
            />

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuPanelRight width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">
                        {move || i18n.tr(I18nKey::MemorySettingsSidePanelSection)()}
                    </span>
                </h4>
                <label class="app-prefs-toggle memory-settings-toggle">
                    <input
                        class="memory-settings-toggle__input"
                        type="checkbox"
                        prop:checked=move || prefs.memory_right_panel_enabled().get()
                        on:change=move |ev| {
                            if let Some(checked) = checkbox_checked(&ev) {
                                prefs.set_memory_right_panel_enabled(checked);
                            }
                        }
                    />
                    <span
                        class="blx-switch"
                        class:blx-switch--on=move || prefs.memory_right_panel_enabled().get()
                        aria-hidden="true"
                    >
                        <span class="blx-switch__thumb" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::MemorySettingsRightPanelToggle)()}</span>
                </label>
                <p class="app-prefs-hint">
                    {move || i18n.tr(I18nKey::MemorySettingsRightPanelHint)()}
                </p>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuDatabaseZap width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">"Memory Indexer"</span>
                </h4>
                <p class="app-prefs-hint">{move || stats_line.get()}</p>
                <div class="agent-provider-pane__grid">
                    <label class="harness-stack">
                        <span class="harness-field-label">
                            <span class="harness-field-label__text">"Provider"</span>
                        </span>
                        <select
                            class="harness-input"
                            prop:value=move || index_provider.get().as_str()
                            on:change=move |ev| {
                                if let Some(provider) = select_value(&ev).and_then(|v| provider_from_str(&v)) {
                                    index_provider.set(provider);
                                    load_models(provider);
                                }
                            }
                        >
                            <For
                                each=move || providers().to_vec()
                                key=|provider| provider.as_str().to_string()
                                children=move |provider| view! {
                                    <option value=provider.as_str()>{provider_label(provider)}</option>
                                }
                            />
                        </select>
                    </label>
                    <label class="harness-stack">
                        <span class="harness-field-label">
                            <span class="harness-field-label__text">"Indexing model"</span>
                        </span>
                        <AgentModelPicker
                            model_id=index_model
                            model_entries=model_entries
                            loading_models=loading_models
                            option_id_prefix="memory-index-model"
                        />
                    </label>
                </div>
                <div class="agent-provider-pane__actions">
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        disabled=move || !is_tauri_shell()
                        on:click=move |_| save_index_settings()
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                            <span>{move || i18n.tr(I18nKey::MemorySettingsSaveIndexingModel)()}</span>
                        </span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        disabled=move || loading_models.get() || !is_tauri_shell()
                        on:click=move |_| load_models(index_provider.get_untracked())
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                            <span>{move || {
                                if loading_models.get() {
                                    i18n.tr(I18nKey::CommonLoading)().to_string()
                                } else {
                                    i18n.tr(I18nKey::AgModelsRefresh)().to_string()
                                }
                            }}</span>
                        </span>
                    </button>
                </div>
            </section>

            <Show when=move || status.get().is_some()>
                <p class="app-prefs-hint">{move || status.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || error.get().is_some()>
                <p class="mcp-status-error">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </article>
    }
}
