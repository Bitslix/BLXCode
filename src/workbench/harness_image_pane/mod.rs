//! Image-mode settings: provider dropdown + shared `AgentModelPicker`.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, image_settings_get, image_settings_save,
    is_tauri_shell, AgentProviderKind, AgentProviderSettingsView, ImageProviderKind,
    ImageQualityLevel, ImageSettings, ProviderModelEntry,
};
use crate::workbench::agent_model_picker::AgentModelPicker;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

fn image_providers() -> [ImageProviderKind; 2] {
    [ImageProviderKind::Openrouter, ImageProviderKind::Openai]
}

fn image_to_agent_provider(p: ImageProviderKind) -> AgentProviderKind {
    match p {
        ImageProviderKind::Openai => AgentProviderKind::Openai,
        ImageProviderKind::Openrouter => AgentProviderKind::Openrouter,
    }
}

fn image_provider_label(i18n: &I18nService, provider: ImageProviderKind) -> String {
    let key = match provider {
        ImageProviderKind::Openrouter => I18nKey::AgProviderOpenrouter,
        ImageProviderKind::Openai => I18nKey::AgProviderOpenai,
    };
    i18n.tr(key)().to_string()
}

fn image_provider_icon_url(provider: ImageProviderKind) -> &'static str {
    match provider {
        ImageProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        ImageProviderKind::Openai => "/public/brand-icons/openai.svg",
    }
}

fn image_provider_key_status(
    i18n: &I18nService,
    view: &AgentProviderSettingsView,
    provider: ImageProviderKind,
) -> String {
    let agent = image_to_agent_provider(provider);
    let configured = view
        .key_statuses
        .iter()
        .find(|s| s.provider == agent)
        .map(|s| s.configured)
        .unwrap_or(false);
    if configured {
        let mask = view
            .key_statuses
            .iter()
            .find(|s| s.provider == agent)
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

fn focus_image_provider_option(provider: ImageProviderKind) {
    focus_by_id(&format!("image-provider-option-{}", provider.as_str()));
}

fn next_image_provider(provider: ImageProviderKind) -> ImageProviderKind {
    match provider {
        ImageProviderKind::Openrouter => ImageProviderKind::Openai,
        ImageProviderKind::Openai => ImageProviderKind::Openrouter,
    }
}

fn prev_image_provider(provider: ImageProviderKind) -> ImageProviderKind {
    next_image_provider(provider)
}

fn image_quality_levels() -> [ImageQualityLevel; 4] {
    [
        ImageQualityLevel::Low,
        ImageQualityLevel::Medium,
        ImageQualityLevel::High,
        ImageQualityLevel::Max,
    ]
}

fn image_quality_label(i18n: &I18nService, level: ImageQualityLevel) -> String {
    let key = match level {
        ImageQualityLevel::Low => I18nKey::AgImageQualityLow,
        ImageQualityLevel::Medium => I18nKey::AgImageQualityMedium,
        ImageQualityLevel::High => I18nKey::AgImageQualityHigh,
        ImageQualityLevel::Max => I18nKey::AgImageQualityMax,
    };
    i18n.tr(key)().to_string()
}

fn image_quality_icon(level: ImageQualityLevel) -> icondata::Icon {
    match level {
        ImageQualityLevel::Low => icondata::LuGauge,
        ImageQualityLevel::Medium => icondata::LuImage,
        ImageQualityLevel::High => icondata::LuSparkles,
        ImageQualityLevel::Max => icondata::LuZap,
    }
}

fn focus_image_quality_option(level: ImageQualityLevel) {
    let id = format!("image-quality-option-{level:?}").to_ascii_lowercase();
    focus_by_id(&id);
}

fn next_image_quality(level: ImageQualityLevel) -> ImageQualityLevel {
    let levels = image_quality_levels();
    let idx = levels.iter().position(|l| *l == level).unwrap_or(1);
    levels[(idx + 1) % levels.len()]
}

fn prev_image_quality(level: ImageQualityLevel) -> ImageQualityLevel {
    let levels = image_quality_levels();
    let idx = levels.iter().position(|l| *l == level).unwrap_or(1);
    levels[(idx + levels.len() - 1) % levels.len()]
}

fn looks_like_image_model(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("image")
        || l.contains("dall-e")
        || l.contains("dalle")
        || l.contains("gpt-image")
        || l.contains("flux")
        || l.contains("stable-diffusion")
        || l.contains("sdxl")
        || l.contains("imagen")
}

async fn fetch_image_models(
    provider: ImageProviderKind,
    out: RwSignal<Vec<ProviderModelEntry>>,
) {
    let agent_provider = image_to_agent_provider(provider);
    let all = match agent_provider_models(agent_provider).await {
        Ok(resp) => resp.entries,
        Err(_) => Vec::new(),
    };
    let filtered: Vec<_> = all
        .iter()
        .filter(|m| looks_like_image_model(&m.id))
        .cloned()
        .collect();
    let mut entries = if filtered.is_empty() { all } else { filtered };
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    out.set(entries);
}

#[component]
fn ImageProviderPicker(
    selected_provider: RwSignal<ImageProviderKind>,
    on_select: Callback<ImageProviderKind>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |provider: ImageProviderKind| {
        selected_provider.set(provider);
        open.set(false);
        on_select.run(provider);
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
                            focus_image_provider_option(provider);
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
                                focus_image_provider_option(provider);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let provider = prev_image_provider(selected_provider.get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_image_provider_option(provider);
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
                            src=move || image_provider_icon_url(selected_provider.get())
                            alt=""
                        />
                    </span>
                    <span>{move || image_provider_label(&i18n, selected_provider.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        image_providers()
                            .into_iter()
                            .map(|provider| {
                                view! {
                                    <button
                                        id=format!("image-provider-option-{}", provider.as_str())
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
                                                    focus_image_provider_option(next_image_provider(provider));
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    focus_image_provider_option(prev_image_provider(provider));
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
                                            <img
                                                class="harness-provider-option__img"
                                                src=image_provider_icon_url(provider)
                                                alt=""
                                            />
                                        </span>
                                        <span>{image_provider_label(&i18n, provider)}</span>
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
fn ImageQualityLevelPicker(
    selected: RwSignal<ImageQualityLevel>,
    on_select: Callback<ImageQualityLevel>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |level: ImageQualityLevel| {
        selected.set(level);
        open.set(false);
        on_select.run(level);
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
                            focus_image_quality_option(level);
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
                                focus_image_quality_option(level);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let level = prev_image_quality(selected.get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_image_quality_option(level);
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
                            icon=move || image_quality_icon(selected.get())
                            width="0.76rem"
                            height="0.76rem"
                        />
                    </span>
                    <span>{move || image_quality_label(&i18n, selected.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        image_quality_levels()
                            .into_iter()
                            .map(|level| {
                                view! {
                                    <button
                                        id=format!("image-quality-option-{level:?}").to_ascii_lowercase()
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
                                                    focus_image_quality_option(next_image_quality(level));
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    focus_image_quality_option(prev_image_quality(level));
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
                                            <LxIcon icon=image_quality_icon(level) width="0.76rem" height="0.76rem" />
                                        </span>
                                        <span>{image_quality_label(&i18n, level)}</span>
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

/// Image model settings column (BLXCode Agent grid, middle).
#[component]
pub fn AgentImageColumn() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings = RwSignal::new(Option::<ImageSettings>::None);
    let agent_settings = RwSignal::new(Option::<AgentProviderSettingsView>::None);
    let status = RwSignal::new(Option::<String>::None);
    let selected_provider = RwSignal::new(ImageProviderKind::Openai);
    let quality_level = RwSignal::new(ImageQualityLevel::Medium);
    let model_entries = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let model_id = RwSignal::new(String::new());
    let loading_models = RwSignal::new(false);

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                agent_settings.set(Some(view));
            }
            if let Ok(s) = image_settings_get().await {
                selected_provider.set(s.provider);
                quality_level.set(s.quality);
                model_id.set(s.model_id.clone());
                fetch_image_models(s.provider, model_entries).await;
                settings.set(Some(s));
            }
        });
    }

    let save = move |patch: ImageSettings| {
        if !is_tauri_shell() {
            selected_provider.set(patch.provider);
            quality_level.set(patch.quality);
            model_id.set(patch.model_id.clone());
            settings.set(Some(patch));
            return;
        }
        leptos::task::spawn_local(async move {
            match image_settings_save(patch).await {
                Ok(s) => {
                    selected_provider.set(s.provider);
                    quality_level.set(s.quality);
                    model_id.set(s.model_id.clone());
                    settings.set(Some(s));
                    status.set(Some(i18n.tr(I18nKey::ApiKeysSaved)().to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let reload_models = move |provider: ImageProviderKind| {
        loading_models.set(true);
        leptos::task::spawn_local(async move {
            fetch_image_models(provider, model_entries).await;
            loading_models.set(false);
        });
    };

    let on_provider_select = Callback::new(move |p: ImageProviderKind| {
        let mut next = settings.get_untracked().unwrap_or_default();
        next.provider = p;
        next.quality = quality_level.get_untracked();
        save(next);
        reload_models(p);
    });

    let on_quality_select = Callback::new(move |q: ImageQualityLevel| {
        let mut next = settings.get_untracked().unwrap_or_default();
        next.quality = q;
        save(next);
    });

    let on_model_change = Callback::new(move |m: String| {
        let mut next = settings.get_untracked().unwrap_or_default();
        next.model_id = m;
        next.quality = quality_level.get_untracked();
        save(next);
    });

    view! {
        <>
            <h4 class="harness-pane-subhead agent-provider-pane__col-title">
                <span class="harness-pane-subhead__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuImage width="0.82rem" height="0.82rem" />
                </span>
                <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgColumnImage)()}</span>
            </h4>

            <Show
                when=move || settings.get().is_some()
                fallback=move || view! {
                    <p class="image-pane__loading">{move || i18n.tr(I18nKey::BlxLoading)()}</p>
                }
            >
                <div class="agent-provider-pane__picker-grid agent-provider-pane__picker-grid--stacked">
                    <label class="agent-provider-pane__field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuPlug width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderField)()}</span>
                        </span>
                        <ImageProviderPicker
                            selected_provider=selected_provider
                            on_select=on_provider_select
                        />
                    </label>
                    <label class="agent-provider-pane__field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuSlidersHorizontal width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgImageQualityField)()}</span>
                        </span>
                        <ImageQualityLevelPicker
                            selected=quality_level
                            on_select=on_quality_select
                        />
                    </label>
                </div>
                <div class="agent-provider-pane__key-row harness-muted">
                    <span>{move || i18n.tr(I18nKey::ApiKeysManageHint)()}</span>
                    <span class="agent-provider-pane__key-status">
                        {move || {
                            agent_settings
                                .get()
                                .map(|view| {
                                    image_provider_key_status(&i18n, &view, selected_provider.get())
                                })
                                .unwrap_or_else(|| i18n.tr(I18nKey::AgApiKeyMissing)().to_string())
                        }}
                    </span>
                </div>
                <label class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgModelField)()}</span>
                    </span>
                    <AgentModelPicker
                        model_id=model_id
                        model_entries=model_entries
                        loading_models=loading_models
                        option_id_prefix="agent-image-model"
                        on_change=on_model_change
                    />
                </label>
                <div class="agent-provider-pane__actions">
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        disabled=move || loading_models.get() || !is_tauri_shell()
                        on:click=move |_| reload_models(selected_provider.get_untracked())
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
                </div>
            </Show>

            <Show when=move || status.get().is_some()>
                <p class="image-pane__status">{move || status.get().unwrap_or_default()}</p>
            </Show>
        </>
    }
}

#[component]
pub fn ImagePane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <article class="harness-pane image-pane-standalone">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuImage width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::ImagePaneTitle)()}</span>
            </h3>
            <div class="agent-provider-pane__col agent-provider-pane__col--standalone">
                <AgentImageColumn />
            </div>
        </article>
    }
}
