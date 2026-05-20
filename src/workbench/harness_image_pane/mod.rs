//! Image-mode settings tab: provider buttons + shared `ModelPicker`.
//!
//! Mirrors the smaller flow of `harness_voice_pane` (no voice catalogue,
//! no PTT). Persists via the `image_settings_save` Tauri command.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, image_settings_get, image_settings_save, is_tauri_shell,
    AgentProviderKind, ImageProviderKind, ImageSettings, ProviderModelEntry,
};
use crate::workbench::model_picker::ModelPicker;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

fn image_to_agent_provider(p: ImageProviderKind) -> AgentProviderKind {
    match p {
        ImageProviderKind::Openai => AgentProviderKind::Openai,
        ImageProviderKind::Openrouter => AgentProviderKind::Openrouter,
    }
}

/// Cheap heuristic mirroring `looks_like_image_model` in the backend —
/// catches `dall-e`, `gpt-image`, `flux`, `stable-diffusion`, `imagen`,
/// `gemini-2.5-flash-image` and anything that just says "image".
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
pub fn ImagePane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings = RwSignal::new(Option::<ImageSettings>::None);
    let status = RwSignal::new(Option::<String>::None);
    let models = RwSignal::new(Vec::<ProviderModelEntry>::new());

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(s) = image_settings_get().await {
                fetch_image_models(s.provider, models).await;
                settings.set(Some(s));
            }
        });
    }

    let save = move |patch: ImageSettings| {
        if !is_tauri_shell() {
            settings.set(Some(patch));
            return;
        }
        leptos::task::spawn_local(async move {
            match image_settings_save(patch).await {
                Ok(s) => {
                    settings.set(Some(s));
                    status.set(Some("saved".into()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let reload_models = move |provider: ImageProviderKind| {
        leptos::task::spawn_local(async move {
            fetch_image_models(provider, models).await;
        });
    };

    view! {
        <section class="harness-settings-pane image-pane" aria-labelledby="image-pane-title">
            <header class="harness-settings-pane__head">
                <h2 id="image-pane-title">
                    <LxIcon icon=icondata::LuImage width="1.1rem" height="1.1rem" />
                    <span>{move || i18n.tr(I18nKey::ImagePaneTitle)()}</span>
                </h2>
                <p class="image-pane__lede">{move || i18n.tr(I18nKey::ImagePaneDescription)()}</p>
            </header>

            <Show
                when=move || settings.get().is_some()
                fallback=move || view! {
                    <p class="image-pane__loading">{move || i18n.tr(I18nKey::BlxLoading)()}</p>
                }
            >
                {move || {
                    let Some(current) = settings.get() else {
                        return view! { <></> }.into_any();
                    };
                    let provider = current.provider;
                    let model_id = current.model_id.clone();

                    let on_provider = {
                        let current = current.clone();
                        move |p: ImageProviderKind| {
                            let mut next = current.clone();
                            next.provider = p;
                            save(next);
                            reload_models(p);
                        }
                    };
                    let on_model = {
                        let current = current.clone();
                        move |m: String| {
                            let mut next = current.clone();
                            next.model_id = m;
                            save(next);
                        }
                    };

                    view! {
                        <div class="image-pane__field">
                            <label>{move || i18n.tr(I18nKey::ImageProviderField)()}</label>
                            <div class="image-pane__provider-row">
                                <ProviderBtn
                                    label="OpenAI"
                                    target=ImageProviderKind::Openai
                                    active=provider
                                    on:click={
                                        let on_provider = on_provider.clone();
                                        move |_| on_provider(ImageProviderKind::Openai)
                                    }
                                />
                                <ProviderBtn
                                    label="OpenRouter"
                                    target=ImageProviderKind::Openrouter
                                    active=provider
                                    on:click={
                                        let on_provider = on_provider.clone();
                                        move |_| on_provider(ImageProviderKind::Openrouter)
                                    }
                                />
                            </div>
                        </div>

                        <ModelPicker
                            label_key=I18nKey::ImageModelField
                            datalist_id="blxcode-image-models"
                            current=model_id.clone()
                            models=models
                            on_change=on_model.clone()
                            on_refresh={
                                let provider = provider;
                                move || reload_models(provider)
                            }
                        />
                    }.into_any()
                }}
            </Show>

            <Show when=move || status.get().is_some()>
                <p class="image-pane__status">{move || status.get().unwrap_or_default()}</p>
            </Show>
        </section>
    }
}

#[component]
fn ProviderBtn(
    label: &'static str,
    target: ImageProviderKind,
    active: ImageProviderKind,
) -> impl IntoView {
    let is_active = move || target == active;
    view! {
        <button
            type="button"
            class="image-pane__choice"
            class:image-pane__choice--active=is_active
        >
            {label}
        </button>
    }
}
