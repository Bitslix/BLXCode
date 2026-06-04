//! Provider card — Chat / Image / STT / TTS provider pickers, conditional
//! endpoint fields (Ollama/LM-Studio URL, Portkey base URL, Cloudflare account),
//! and per-channel API-key status (keys are managed in the API-Keys tab).

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{AgentProviderKind, ImageProviderKind, VoiceProviderKind};

use super::data::{
    agent_key_status_text, apply_voice_provider_defaults, image_key_status_text,
    image_provider_icon_url, image_provider_label, image_providers, local_provider_default_url,
    provider_icon_url, provider_label, voice_key_status_text, voice_provider_icon_url,
    voice_provider_label, voice_providers, AGENT_PROVIDERS,
};
use super::pickers::{OptionPicker, PickerOption};
use super::{debounced, AgentSettingsCtx, CardHead};

fn agent_provider_from_id(id: &str) -> AgentProviderKind {
    AGENT_PROVIDERS
        .iter()
        .copied()
        .find(|p| p.as_str() == id)
        .unwrap_or(AgentProviderKind::Openrouter)
}

fn image_provider_from_id(id: &str) -> ImageProviderKind {
    image_providers()
        .into_iter()
        .find(|p| p.as_str() == id)
        .unwrap_or(ImageProviderKind::Openai)
}

fn voice_provider_from_id(id: &str) -> VoiceProviderKind {
    voice_providers()
        .into_iter()
        .find(|p| p.as_str() == id)
        .unwrap_or(VoiceProviderKind::Openai)
}

#[component]
pub(crate) fn ProvidersSection() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ctx = expect_context::<AgentSettingsCtx>();
    let url_gen = RwSignal::new(0u32);
    let portkey_gen = RwSignal::new(0u32);
    let cf_gen = RwSignal::new(0u32);

    let chat_options = Signal::derive(move || {
        AGENT_PROVIDERS
            .iter()
            .copied()
            .map(|p| PickerOption::brand(p.as_str(), provider_label(&i18n, p), provider_icon_url(p)))
            .collect::<Vec<_>>()
    });
    let image_options = Signal::derive(move || {
        image_providers()
            .into_iter()
            .map(|p| {
                PickerOption::brand(p.as_str(), image_provider_label(&i18n, p), image_provider_icon_url(p))
            })
            .collect::<Vec<_>>()
    });
    let voice_options = Signal::derive(move || {
        voice_providers()
            .into_iter()
            .map(|p| {
                PickerOption::brand(p.as_str(), voice_provider_label(&i18n, p), voice_provider_icon_url(p))
            })
            .collect::<Vec<_>>()
    });

    let chat_status = Signal::derive(move || {
        agent_key_status_text(&i18n, ctx.settings.get().as_ref(), ctx.chat_provider.get())
    });
    let image_status = Signal::derive(move || {
        image_key_status_text(
            &i18n,
            ctx.settings.get().as_ref(),
            ctx.api_keys.get().as_ref(),
            ctx.image_provider.get(),
        )
    });
    let stt_status = Signal::derive(move || {
        voice_key_status_text(
            &i18n,
            ctx.settings.get().as_ref(),
            ctx.api_keys.get().as_ref(),
            ctx.stt_provider.get(),
        )
    });
    let tts_status = Signal::derive(move || {
        voice_key_status_text(
            &i18n,
            ctx.settings.get().as_ref(),
            ctx.api_keys.get().as_ref(),
            ctx.tts_provider.get(),
        )
    });

    view! {
        <section class="harness-subpane agent-settings-card">
            <CardHead icon=icondata::LuPlug label=I18nKey::AgSecProvider />

            <div class="agent-grid">
                <ChannelProvider
                    label=I18nKey::AgChannelChat
                    prefix="chat-provider"
                    options=chat_options
                    selected=Signal::derive(move || ctx.chat_provider.get().as_str().to_string())
                    on_select=Callback::new(move |id: String| {
                        ctx.chat_provider.set(agent_provider_from_id(&id));
                        ctx.refresh_chat_models.run(());
                        ctx.save_agent_core.run(());
                    })
                    key_status=chat_status
                />
                <ChannelProvider
                    label=I18nKey::AgChannelImage
                    prefix="image-provider"
                    options=image_options
                    selected=Signal::derive(move || ctx.image_provider.get().as_str().to_string())
                    on_select=Callback::new(move |id: String| {
                        let p = image_provider_from_id(&id);
                        ctx.image_provider.set(p);
                        // keep model id compatible with the chosen image provider
                        let cur = ctx.image_model.get_untracked();
                        if p == ImageProviderKind::Fal && !cur.contains("fal-ai/") {
                            ctx.image_model.set("fal-ai/flux/schnell".into());
                        } else if p != ImageProviderKind::Fal && cur.contains("fal-ai/") {
                            ctx.image_model.set("gpt-image-1".into());
                        }
                        ctx.refresh_image_models.run(());
                        ctx.save_image.run(());
                    })
                    key_status=image_status
                />
                <ChannelProvider
                    label=I18nKey::AgChannelStt
                    prefix="stt-provider"
                    options=voice_options
                    selected=Signal::derive(move || ctx.stt_provider.get().as_str().to_string())
                    on_select=Callback::new(move |id: String| {
                        let p = voice_provider_from_id(&id);
                        ctx.stt_provider.set(p);
                        let mut stt = ctx.stt_model.get_untracked();
                        let mut tts = ctx.tts_model.get_untracked();
                        let mut voice = ctx.tts_voice.get_untracked();
                        apply_voice_provider_defaults(&mut stt, &mut tts, &mut voice, p);
                        ctx.stt_model.set(stt);
                        ctx.refresh_stt_models.run(());
                        ctx.save_voice.run(());
                    })
                    key_status=stt_status
                />
                <ChannelProvider
                    label=I18nKey::AgChannelTts
                    prefix="tts-provider"
                    options=voice_options
                    selected=Signal::derive(move || ctx.tts_provider.get().as_str().to_string())
                    on_select=Callback::new(move |id: String| {
                        let p = voice_provider_from_id(&id);
                        ctx.tts_provider.set(p);
                        let mut stt = ctx.stt_model.get_untracked();
                        let mut tts = ctx.tts_model.get_untracked();
                        let mut voice = ctx.tts_voice.get_untracked();
                        apply_voice_provider_defaults(&mut stt, &mut tts, &mut voice, p);
                        ctx.tts_model.set(tts);
                        ctx.tts_voice.set(voice);
                        ctx.refresh_tts_models.run(());
                        ctx.save_voice.run(());
                    })
                    key_status=tts_status
                />
            </div>

            // --- Conditional endpoint fields for the chat provider ---
            <Show when=move || local_provider_default_url(ctx.chat_provider.get()).is_some()>
                <div class="agent-endpoint">
                    <label class="agent-field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuServer width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderLocalServerUrlField)()}</span>
                        </span>
                        <input
                            class="workbench-plain-input agent-endpoint__input"
                            type="url"
                            placeholder=move || local_provider_default_url(ctx.chat_provider.get()).unwrap_or_default()
                            prop:value=move || {
                                ctx.provider_base_urls.get()
                                    .get(ctx.chat_provider.get().as_str())
                                    .cloned()
                                    .unwrap_or_else(|| local_provider_default_url(ctx.chat_provider.get()).unwrap_or_default().to_string())
                            }
                            on:input=move |ev| {
                                let provider = ctx.chat_provider.get_untracked();
                                let value = event_target_value(&ev);
                                ctx.provider_base_urls.update(|m| { m.insert(provider.as_str().to_string(), value); });
                                debounced(url_gen, 600, ctx.save_agent_core);
                            }
                        />
                        <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::AgProviderLocalServerUrlHint)()}</small>
                    </label>
                    <span class="agent-endpoint__chip">
                        <LxIcon icon=icondata::LuKeyRound width="0.76rem" height="0.76rem" />
                        <span>{move || i18n.tr(I18nKey::AgProviderNoApiKeyRequired)()}</span>
                    </span>
                </div>
            </Show>

            <Show when=move || ctx.chat_provider.get() == AgentProviderKind::Portkey>
                <label class="agent-field">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuLink width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderBaseUrlField)()}</span>
                    </span>
                    <input
                        class="workbench-plain-input"
                        type="url"
                        prop:value=move || {
                            ctx.provider_base_urls.get().get(ctx.chat_provider.get().as_str()).cloned().unwrap_or_default()
                        }
                        on:input=move |ev| {
                            let provider = ctx.chat_provider.get_untracked();
                            let value = event_target_value(&ev);
                            ctx.provider_base_urls.update(|m| { m.insert(provider.as_str().to_string(), value); });
                            debounced(portkey_gen, 600, ctx.save_agent_core);
                        }
                    />
                    <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::AgProviderBaseUrlHint)()}</small>
                </label>
            </Show>

            <Show when=move || ctx.chat_provider.get() == AgentProviderKind::Cloudflare>
                <label class="agent-field">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuCloud width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderCloudflareAccountField)()}</span>
                    </span>
                    <input
                        class="workbench-plain-input"
                        type="text"
                        prop:value=move || ctx.cloudflare_account_id.get()
                        on:input=move |ev| {
                            ctx.cloudflare_account_id.set(event_target_value(&ev));
                            debounced(cf_gen, 600, ctx.save_agent_core);
                        }
                    />
                    <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::AgProviderCloudflareAccountHint)()}</small>
                </label>
            </Show>
        </section>
    }
}

#[component]
fn ChannelProvider(
    label: I18nKey,
    prefix: &'static str,
    options: Signal<Vec<PickerOption>>,
    selected: Signal<String>,
    on_select: Callback<String>,
    key_status: Signal<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="agent-field">
            <span class="harness-field-label">
                <span class="harness-field-label__text">{move || i18n.tr(label)()}</span>
            </span>
            <OptionPicker prefix=prefix options=options selected=selected on_select=on_select />
            <div class="agent-key-row">
                <span>{move || i18n.tr(I18nKey::ApiKeysManageHint)()}</span>
                <span class="agent-key-row__status">{move || key_status.get()}</span>
            </div>
        </div>
    }
}
