//! Settings → BLXCode Agent — provider, model, thinking, web tools.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, agent_settings_save, agent_web_settings_get,
    agent_web_settings_save, is_tauri_shell, AgentProviderKind, AgentProviderSettingsView,
    AgentWebSettingsView, ProviderModelEntry, ProviderModelsResponse, ThinkingLevel,
    WebProviderKind,
};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq, Eq)]
struct AgentSettingsBaseline {
    provider: AgentProviderKind,
    model_id: String,
    thinking: ThinkingLevel,
    web_provider: WebProviderKind,
}

fn input_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

fn provider_label(i18n: &I18nService, provider: AgentProviderKind) -> String {
    let key = match provider {
        AgentProviderKind::Openrouter => I18nKey::AgProviderOpenrouter,
        AgentProviderKind::Anthropic => I18nKey::AgProviderAnthropic,
        AgentProviderKind::Openai => I18nKey::AgProviderOpenai,
    };
    i18n.tr(key)().to_string()
}

fn provider_icon_url(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        AgentProviderKind::Anthropic => "/public/brand-icons/anthropic.svg",
        AgentProviderKind::Openai => "/public/brand-icons/openai.svg",
    }
}

fn thinking_levels() -> [ThinkingLevel; 5] {
    [
        ThinkingLevel::Off,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Max,
    ]
}

fn thinking_label(i18n: &I18nService, level: ThinkingLevel) -> String {
    let key = match level {
        ThinkingLevel::Off => I18nKey::AgThinkingOff,
        ThinkingLevel::Low => I18nKey::AgThinkingLow,
        ThinkingLevel::Medium => I18nKey::AgThinkingMedium,
        ThinkingLevel::High => I18nKey::AgThinkingHigh,
        ThinkingLevel::Max => I18nKey::AgThinkingMax,
    };
    i18n.tr(key)().to_string()
}

fn thinking_icon(level: ThinkingLevel) -> icondata::Icon {
    match level {
        ThinkingLevel::Off => icondata::LuCircleOff,
        ThinkingLevel::Low => icondata::LuGauge,
        ThinkingLevel::Medium => icondata::LuActivity,
        ThinkingLevel::High => icondata::LuFlame,
        ThinkingLevel::Max => icondata::LuZap,
    }
}

fn provider_key_status_text(
    i18n: &I18nService,
    view: &AgentProviderSettingsView,
    provider: AgentProviderKind,
) -> String {
    let configured = view
        .key_statuses
        .iter()
        .find(|s| s.provider == provider)
        .map(|s| s.configured)
        .unwrap_or(false);
    if configured {
        let mask = view
            .key_statuses
            .iter()
            .find(|s| s.provider == provider)
            .and_then(|s| s.masked_value.clone());
        if let Some(mask) = mask {
            format!("{} ({mask})", i18n.tr(I18nKey::AgApiKeyConfigured)())
        } else {
            i18n.tr(I18nKey::AgApiKeyConfigured)().to_string()
        }
    } else {
        i18n.tr(I18nKey::AgApiKeyMissing)().to_string()
    }
}

fn provider_cache(view: &AgentProviderSettingsView, provider: AgentProviderKind) -> Vec<ProviderModelEntry> {
    match provider {
        AgentProviderKind::Openrouter => view.model_cache_openrouter.clone(),
        AgentProviderKind::Anthropic => view.model_cache_anthropic.clone(),
        AgentProviderKind::Openai => view.model_cache_openai.clone(),
    }
}

fn price_per_million(usd_per_token: f64) -> String {
    format!("${:.2}", usd_per_token * 1_000_000.0)
}

fn model_row_meta(i18n: &I18nService, entry: &ProviderModelEntry) -> Option<String> {
    if let Some(p) = entry.pricing {
        Some(
            i18n.tr(I18nKey::AgModelMetaPricing)()
                .replace("{in}", &price_per_million(p.prompt))
                .replace("{out}", &price_per_million(p.completion)),
        )
    } else {
        entry.description.as_ref().and_then(|d| {
            let t = d.trim();
            if t.is_empty() {
                None
            } else {
                Some(if t.len() > 88 {
                    format!("{}…", &t[..88])
                } else {
                    t.to_string()
                })
            }
        })
    }
}

fn find_model_entry(entries: &[ProviderModelEntry], id: &str) -> Option<ProviderModelEntry> {
    entries.iter().find(|e| e.id == id).cloned()
}

fn model_trigger_label(entries: &[ProviderModelEntry], id: &str) -> String {
    find_model_entry(entries, id)
        .map(|e| {
            if e.label.trim().is_empty() {
                e.id
            } else {
                e.label
            }
        })
        .unwrap_or_else(|| id.to_string())
}

fn focus_provider_option(provider: AgentProviderKind) {
    let id = format!("provider-option-{}", provider.as_str());
    focus_by_id(&id);
}

fn focus_thinking_option(level: ThinkingLevel) {
    let id = format!("thinking-option-{:?}", level).to_ascii_lowercase();
    focus_by_id(&id);
}

fn focus_model_option(model_id: &str) {
    let id = model_option_dom_id(model_id);
    focus_by_id(&id);
}

fn focus_by_id(id: &str) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(el) = doc.get_element_by_id(id) else {
        return;
    };
    let Ok(button) = el.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let _ = button.focus();
}

fn model_option_dom_id(model_id: &str) -> String {
    let slug: String = model_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("model-option-{slug}")
}

fn next_provider(provider: AgentProviderKind) -> AgentProviderKind {
    match provider {
        AgentProviderKind::Openrouter => AgentProviderKind::Anthropic,
        AgentProviderKind::Anthropic => AgentProviderKind::Openai,
        AgentProviderKind::Openai => AgentProviderKind::Openrouter,
    }
}

fn prev_provider(provider: AgentProviderKind) -> AgentProviderKind {
    match provider {
        AgentProviderKind::Openrouter => AgentProviderKind::Openai,
        AgentProviderKind::Anthropic => AgentProviderKind::Openrouter,
        AgentProviderKind::Openai => AgentProviderKind::Anthropic,
    }
}

fn next_thinking(level: ThinkingLevel) -> ThinkingLevel {
    let levels = thinking_levels();
    let idx = levels.iter().position(|l| *l == level).unwrap_or(2);
    levels[(idx + 1) % levels.len()]
}

fn prev_thinking(level: ThinkingLevel) -> ThinkingLevel {
    let levels = thinking_levels();
    let idx = levels.iter().position(|l| *l == level).unwrap_or(2);
    levels[(idx + levels.len() - 1) % levels.len()]
}

#[component]
fn ProviderPicker(
    selected_provider: RwSignal<AgentProviderKind>,
    settings: RwSignal<Option<AgentProviderSettingsView>>,
    model_entries: RwSignal<Vec<ProviderModelEntry>>,
    provider_refresh_request: RwSignal<Option<AgentProviderKind>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |provider: AgentProviderKind| {
        selected_provider.set(provider);
        if let Some(view) = settings.get_untracked() {
            model_entries.set(provider_cache(&view, provider));
        }
        open.set(false);
        provider_refresh_request.set(Some(provider));
    };

    view! {
        <div class="harness-provider-picker">
            <button
                type="button"
                class="harness-provider-trigger"
                aria-haspopup="listbox"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |_| {
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        let provider = selected_provider.get_untracked();
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(0).await;
                            focus_provider_option(provider);
                        });
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " => {
                            ev.prevent_default();
                            open.set(true);
                            let provider = selected_provider.get_untracked();
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_provider_option(provider);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let provider = prev_provider(selected_provider.get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_provider_option(provider);
                            });
                        }
                        "Escape" => open.set(false),
                        _ => {}
                    }
                }
            >
                <span class="harness-provider-trigger__main">
                    <span class="harness-provider-trigger__brand">
                        <img
                            class="harness-provider-trigger__img"
                            src=move || provider_icon_url(selected_provider.get())
                            alt=""
                        />
                    </span>
                    <span>{move || provider_label(&i18n, selected_provider.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        [
                            AgentProviderKind::Openrouter,
                            AgentProviderKind::Anthropic,
                            AgentProviderKind::Openai,
                        ]
                        .into_iter()
                        .map(|provider| {
                            view! {
                                <button
                                    id=format!("provider-option-{}", provider.as_str())
                                    type="button"
                                    role="option"
                                    class="harness-provider-option"
                                    class:harness-provider-option--active=move || selected_provider.get() == provider
                                    aria-selected=move || if selected_provider.get() == provider {
                                        "true"
                                    } else {
                                        "false"
                                    }
                                    on:click=move |_| choose(provider)
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        match ev.key().as_str() {
                                            "ArrowDown" => {
                                                ev.prevent_default();
                                                focus_provider_option(next_provider(provider));
                                            }
                                            "ArrowUp" => {
                                                ev.prevent_default();
                                                focus_provider_option(prev_provider(provider));
                                            }
                                            "Enter" | " " => {
                                                ev.prevent_default();
                                                choose(provider);
                                            }
                                            "Escape" => {
                                                ev.prevent_default();
                                                open.set(false);
                                            }
                                            _ => {}
                                        }
                                    }
                                >
                                    <span class="harness-provider-option__brand">
                                        <img class="harness-provider-option__img" src=provider_icon_url(provider) alt="" />
                                    </span>
                                    <span>{provider_label(&i18n, provider)}</span>
                                </button>
                            }
                        })
                        .collect_view()
                    }}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn ThinkingLevelPicker(selected: RwSignal<ThinkingLevel>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |level: ThinkingLevel| {
        selected.set(level);
        open.set(false);
    };

    view! {
        <div class="harness-provider-picker">
            <button
                type="button"
                class="harness-provider-trigger"
                aria-haspopup="listbox"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |_| {
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        let level = selected.get_untracked();
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(0).await;
                            focus_thinking_option(level);
                        });
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " => {
                            ev.prevent_default();
                            open.set(true);
                            let level = selected.get_untracked();
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_thinking_option(level);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let level = prev_thinking(selected.get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_thinking_option(level);
                            });
                        }
                        "Escape" => open.set(false),
                        _ => {}
                    }
                }
            >
                <span class="harness-provider-trigger__main">
                    <span class="harness-provider-trigger__brand">
                        <LxIcon
                            icon=move || thinking_icon(selected.get())
                            width="0.76rem"
                            height="0.76rem"
                        />
                    </span>
                    <span>{move || thinking_label(&i18n, selected.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        thinking_levels()
                            .into_iter()
                            .map(|level| {
                                view! {
                                    <button
                                        id=format!("thinking-option-{:?}", level).to_ascii_lowercase()
                                        type="button"
                                        role="option"
                                        class="harness-provider-option"
                                        class:harness-provider-option--active=move || selected.get() == level
                                        aria-selected=move || if selected.get() == level {
                                            "true"
                                        } else {
                                            "false"
                                        }
                                        on:click=move |_| choose(level)
                                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                                            match ev.key().as_str() {
                                                "ArrowDown" => {
                                                    ev.prevent_default();
                                                    focus_thinking_option(next_thinking(level));
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    focus_thinking_option(prev_thinking(level));
                                                }
                                                "Enter" | " " => {
                                                    ev.prevent_default();
                                                    choose(level);
                                                }
                                                "Escape" => {
                                                    ev.prevent_default();
                                                    open.set(false);
                                                }
                                                _ => {}
                                            }
                                        }
                                    >
                                        <span class="harness-provider-option__brand">
                                            <LxIcon icon=thinking_icon(level) width="0.76rem" height="0.76rem" />
                                        </span>
                                        <span>{thinking_label(&i18n, level)}</span>
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn AgentModelPicker(
    model_id: RwSignal<String>,
    model_entries: RwSignal<Vec<ProviderModelEntry>>,
    loading_models: RwSignal<bool>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let selected_entry =
        Signal::derive(move || find_model_entry(&model_entries.get(), &model_id.get()));

    let choose = move |id: String| {
        model_id.set(id);
        open.set(false);
    };

    view! {
        <div class="agent-model-picker">
            <div class="harness-provider-picker">
                <button
                    type="button"
                    class="harness-provider-trigger"
                    aria-haspopup="listbox"
                    aria-expanded=move || if open.get() { "true" } else { "false" }
                    prop:disabled=move || loading_models.get()
                    on:click=move |_| {
                        let next = !open.get_untracked();
                        open.set(next);
                        if next {
                            let id = model_id.get_untracked();
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_model_option(&id);
                            });
                        }
                    }
                >
                    <span class="harness-provider-trigger__main">
                        <span class="harness-provider-trigger__brand">
                            <LxIcon icon=icondata::LuPackage2 width="0.76rem" height="0.76rem" />
                        </span>
                        <span class="agent-model-picker__trigger-label">
                            {move || model_trigger_label(&model_entries.get(), &model_id.get())}
                        </span>
                    </span>
                    <span class="harness-provider-trigger__caret">"▾"</span>
                </button>

                <Show when=move || open.get()>
                    <div class="harness-provider-menu agent-model-picker__menu" role="listbox">
                        {move || {
                            let entries = model_entries.get();
                            if entries.is_empty() {
                                view! {
                                    <p class="harness-muted" style="padding:0.45rem 0.5rem;margin:0;">
                                        {move || i18n.tr(I18nKey::AgModelsUnavailable)()}
                                    </p>
                                }
                                .into_any()
                            } else {
                                entries
                                    .into_iter()
                                    .map(|entry| {
                                        let entry_id = entry.id.clone();
                                        let meta = model_row_meta(&i18n, &entry);
                                        let title = if entry.label.trim().is_empty() {
                                            entry.id.clone()
                                        } else {
                                            entry.label.clone()
                                        };
                                        view! {
                                            <button
                                                id=model_option_dom_id(&entry_id)
                                                type="button"
                                                role="option"
                                                class="harness-provider-option agent-model-option"
                                                class:harness-provider-option--active={
                                                    let v = entry_id.clone();
                                                    move || model_id.get() == v
                                                }
                                                aria-selected={
                                                    let v = entry_id.clone();
                                                    move || if model_id.get() == v {
                                                        "true"
                                                    } else {
                                                        "false"
                                                    }
                                                }
                                                on:click={
                                                    let v = entry_id.clone();
                                                    move |_| choose(v)
                                                }
                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                    let v = entry_id.clone();
                                                    match ev.key().as_str() {
                                                        "Enter" | " " => {
                                                            ev.prevent_default();
                                                            choose(v);
                                                        }
                                                        "Escape" => {
                                                            ev.prevent_default();
                                                            open.set(false);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            >
                                                <span class="agent-model-option__title">{title}</span>
                                                {meta.map(|line| view! {
                                                    <span class="agent-model-option__meta">{line}</span>
                                                })}
                                            </button>
                                        }
                                        .into_any()
                                    })
                                    .collect_view()
                                    .into_any()
                            }
                        }}
                    </div>
                </Show>
            </div>

            <input
                class="workbench-plain-input"
                type="text"
                placeholder=move || i18n.tr(I18nKey::AgModelCustomField)()
                prop:value=move || model_id.get()
                on:input=move |ev| {
                    if let Some(value) = input_str(&ev) {
                        model_id.set(value);
                    }
                }
            />

            {move || {
                let Some(entry) = selected_entry.get() else {
                    return ().into_any();
                };
                let mut lines = Vec::new();
                if entry.id != entry.label && !entry.label.trim().is_empty() {
                    lines.push(entry.id.clone());
                }
                if let Some(desc) = entry.description.as_ref().filter(|d| !d.trim().is_empty()) {
                    lines.push(desc.trim().to_string());
                }
                if let Some(p) = entry.pricing {
                    lines.push(
                        i18n.tr(I18nKey::AgModelMetaPricing)()
                            .replace("{in}", &price_per_million(p.prompt))
                            .replace("{out}", &price_per_million(p.completion)),
                    );
                }
                if lines.is_empty() {
                    return ().into_any();
                }
                let title = if entry.label.trim().is_empty() {
                    entry.id.clone()
                } else {
                    entry.label.clone()
                };
                view! {
                    <div class="agent-model-picker__detail harness-muted" aria-live="polite">
                        <span class="agent-model-picker__detail-title">{title}</span>
                        {lines.into_iter().map(|line| view! {
                            <span class="agent-model-picker__detail-line">{line}</span>
                        }).collect_view()}
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}

#[component]
pub fn AgentProviderPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings: RwSignal<Option<AgentProviderSettingsView>> = RwSignal::new(None);
    let selected_provider = RwSignal::new(AgentProviderKind::Openrouter);
    let custom_model = RwSignal::new(String::new());
    let thinking_level = RwSignal::new(ThinkingLevel::Medium);
    let model_entries: RwSignal<Vec<ProviderModelEntry>> = RwSignal::new(Vec::new());
    let models_source = RwSignal::new(String::new());
    let models_message: RwSignal<Option<String>> = RwSignal::new(None);
    let busy = RwSignal::new(false);
    let loading_models = RwSignal::new(false);
    let status_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let provider_refresh_request: RwSignal<Option<AgentProviderKind>> = RwSignal::new(None);
    let web_provider = RwSignal::new(WebProviderKind::None);
    let baseline = RwSignal::new(AgentSettingsBaseline {
        provider: AgentProviderKind::Openrouter,
        model_id: String::new(),
        thinking: ThinkingLevel::Medium,
        web_provider: WebProviderKind::None,
    });

    let dirty = Memo::new(move |_| {
        let b = baseline.get();
        selected_provider.get() != b.provider
            || custom_model.get() != b.model_id
            || thinking_level.get() != b.thinking
            || web_provider.get() != b.web_provider
    });

    let snapshot_baseline = move || AgentSettingsBaseline {
        provider: selected_provider.get_untracked(),
        model_id: custom_model.get_untracked(),
        thinking: thinking_level.get_untracked(),
        web_provider: web_provider.get_untracked(),
    };

    let apply_settings = move |view: AgentProviderSettingsView| {
        selected_provider.set(view.provider);
        custom_model.set(view.model_id.clone());
        thinking_level.set(view.thinking_level);
        model_entries.set(provider_cache(&view, view.provider));
        settings.set(Some(view));
        baseline.update(|b| {
            b.provider = selected_provider.get_untracked();
            b.model_id = custom_model.get_untracked();
            b.thinking = thinking_level.get_untracked();
        });
    };

    let apply_web = move |view: AgentWebSettingsView| {
        web_provider.set(view.settings.provider);
        baseline.update(|b| {
            b.web_provider = web_provider.get_untracked();
        });
    };

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match agent_settings_get().await {
                Ok(view) => {
                    error_msg.set(None);
                    apply_settings(view);
                }
                Err(err) => error_msg.set(Some(err)),
            }
            match agent_web_settings_get().await {
                Ok(view) => apply_web(view),
                Err(err) => error_msg.set(Some(err)),
            }
        });
    });

    let refresh_models = move |provider: AgentProviderKind| {
        loading_models.set(true);
        models_message.set(None);
        leptos::task::spawn_local(async move {
            match agent_provider_models(provider).await {
                Ok(ProviderModelsResponse {
                    entries,
                    source,
                    used_fallback,
                    message,
                    ..
                }) => {
                    model_entries.set(entries);
                    models_source.set(source);
                    models_message.set(message.or_else(|| {
                        if used_fallback {
                            Some(i18n.tr(I18nKey::AgModelsFallback)().to_string())
                        } else {
                            None
                        }
                    }));
                }
                Err(err) => error_msg.set(Some(err)),
            }
            loading_models.set(false);
        });
    };

    Effect::new(move |_| {
        let Some(provider) = provider_refresh_request.get() else {
            return;
        };
        provider_refresh_request.set(None);
        refresh_models(provider);
    });

    let save = move || {
        if !dirty.get_untracked() || busy.get_untracked() {
            return;
        }
        busy.set(true);
        error_msg.set(None);
        status_msg.set(None);
        let provider = selected_provider.get_untracked();
        let model_id = custom_model.get_untracked();
        let level = thinking_level.get_untracked();
        let web = web_provider.get_untracked();
        leptos::task::spawn_local(async move {
            let mut err: Option<String> = None;
            match agent_settings_save(provider, model_id, level).await {
                Ok(view) => apply_settings(view),
                Err(e) => err = Some(e),
            }
            match agent_web_settings_save(web).await {
                Ok(view) => apply_web(view),
                Err(e) => err = err.or(Some(e)),
            }
            if let Some(e) = err {
                error_msg.set(Some(e));
            } else {
                baseline.set(snapshot_baseline());
                status_msg.set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string()));
            }
            busy.set(false);
        });
    };

    view! {
        <article class="harness-pane agent-provider-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuCpu width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::AgProviderHeading)()}</span>
            </h3>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuPlug width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgSectionInference)()}</span>
                </h4>
                <div class="agent-provider-pane__picker-grid">
                    <label class="agent-provider-pane__field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuPlug width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderField)()}</span>
                        </span>
                        <ProviderPicker
                            selected_provider=selected_provider
                            settings=settings
                            model_entries=model_entries
                            provider_refresh_request=provider_refresh_request
                        />
                    </label>
                    <label class="agent-provider-pane__field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuGauge width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgThinkingField)()}</span>
                        </span>
                        <ThinkingLevelPicker selected=thinking_level />
                    </label>
                </div>
                <div class="agent-provider-pane__key-row harness-muted">
                    <span>{move || i18n.tr(I18nKey::ApiKeysManageHint)()}</span>
                    <span class="agent-provider-pane__key-status">
                        {move || {
                            settings
                                .get()
                                .map(|view| provider_key_status_text(&i18n, &view, selected_provider.get()))
                                .unwrap_or_else(|| i18n.tr(I18nKey::AgApiKeyMissing)().to_string())
                        }}
                    </span>
                </div>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuPackage2 width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgSectionModel)()}</span>
                </h4>
                <label class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgModelField)()}</span>
                    </span>
                    <AgentModelPicker
                        model_id=custom_model
                        model_entries=model_entries
                        loading_models=loading_models
                    />
                </label>
                <div class="agent-provider-pane__actions">
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        disabled=move || loading_models.get() || !is_tauri_shell()
                        on:click=move |_| refresh_models(selected_provider.get_untracked())
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                            <span>{move || if loading_models.get() {
                                i18n.tr(I18nKey::AgModelsLoading)().to_string()
                            } else {
                                i18n.tr(I18nKey::AgModelsRefresh)().to_string()
                            }}</span>
                        </span>
                    </button>
                    <small class="harness-muted">
                        {move || match models_source.get().as_str() {
                            "live" => i18n.tr(I18nKey::AgModelsSourceLive)().to_string(),
                            "cache" => i18n.tr(I18nKey::AgModelsSourceCache)().to_string(),
                            "curated" | "fallback" => i18n.tr(I18nKey::AgModelsSourceCurated)().to_string(),
                            _ => String::new(),
                        }}
                    </small>
                </div>
                <Show when=move || models_message.get().is_some()>
                    <p class="harness-muted">{move || models_message.get().unwrap_or_default()}</p>
                </Show>
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuGlobe width="0.9rem" height="0.9rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgWebToolsHeading)()}</span>
                </h4>
                <div class="app-prefs-toggle-grid">
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="agent-web-provider"
                                prop:checked=move || web_provider.get() == WebProviderKind::None
                                on:change=move |_| web_provider.set(WebProviderKind::None)
                            />
                            <span>{move || i18n.tr(I18nKey::AgWebProviderNone)()}</span>
                        </label>
                    </div>
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="agent-web-provider"
                                prop:checked=move || web_provider.get() == WebProviderKind::Tavily
                                on:change=move |_| web_provider.set(WebProviderKind::Tavily)
                            />
                            <span>{move || i18n.tr(I18nKey::AgWebProviderTavily)()}</span>
                        </label>
                    </div>
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="agent-web-provider"
                                prop:checked=move || web_provider.get() == WebProviderKind::Brave
                                on:change=move |_| web_provider.set(WebProviderKind::Brave)
                            />
                            <span>{move || i18n.tr(I18nKey::AgWebProviderBrave)()}</span>
                        </label>
                    </div>
                </div>
                <p class="app-prefs-hint">{move || i18n.tr(I18nKey::ApiKeysManageHintWeb)()}</p>
            </section>

            <Show when=move || status_msg.with(|m| m.is_some())>
                <p class="harness-status">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || error_msg.with(|m| m.is_some())>
                <p class="harness-error-text">{move || error_msg.get().unwrap_or_default()}</p>
            </Show>

            <footer class="settings-pane-footer harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    disabled=move || busy.get() || !dirty.get() || !is_tauri_shell()
                    on:click=move |_| save()
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::BtnSave)()}</span>
                    </span>
                </button>
            </footer>
        </article>
    }
}
