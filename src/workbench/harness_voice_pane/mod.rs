//! Voice settings tab: STT/TTS provider+model, voice with gender filter,
//! recording quality, post-STT behaviour. STT language + PTT live under App.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, api_keys_status, is_tauri_shell,
    voice_provider_voices, voice_settings_get, voice_settings_save, voice_tts_preview,
    AgentProviderKind, AgentProviderSettingsView, ApiKeyEntry, ApiKeysStatus, PostSttFlow,
    ProviderModelEntry, SttSettings, TtsSettings, VoiceEntry, VoiceGender, VoiceProviderKind,
    VoiceSettings,
};
use crate::workbench::agent_model_picker::AgentModelPicker;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gloo_timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelKind {
    Stt,
    Tts,
}

impl ModelKind {
    fn matches(self, id: &str) -> bool {
        let lower = id.to_ascii_lowercase();
        match self {
            Self::Stt => lower.contains("transcribe") || lower.contains("whisper"),
            Self::Tts => lower.contains("tts") || lower.contains("speech"),
        }
    }
}

fn voice_to_agent_provider(v: VoiceProviderKind) -> Option<AgentProviderKind> {
    match v {
        VoiceProviderKind::Openai => Some(AgentProviderKind::Openai),
        VoiceProviderKind::Openrouter => Some(AgentProviderKind::Openrouter),
        VoiceProviderKind::Aws => None,
    }
}

fn voice_model_entry(id: &str, label: &str, description: &str) -> ProviderModelEntry {
    ProviderModelEntry {
        id: id.into(),
        label: label.into(),
        description: Some(description.into()),
        pricing: None,
    }
}

fn curated_voice_models(provider: VoiceProviderKind, kind: ModelKind) -> Vec<ProviderModelEntry> {
    if provider != VoiceProviderKind::Aws {
        return Vec::new();
    }
    match kind {
        ModelKind::Stt => vec![voice_model_entry(
            "amazon-transcribe",
            "Amazon Transcribe",
            "AWS speech-to-text (curated list).",
        )],
        ModelKind::Tts => vec![
            voice_model_entry("neural", "Polly Neural", "Neural TTS engine."),
            voice_model_entry("standard", "Polly Standard", "Standard TTS engine."),
        ],
    }
}

fn media_key_entry<'a>(api_keys: &'a ApiKeysStatus, kind: &str) -> Option<&'a ApiKeyEntry> {
    api_keys.entries.iter().find(|e| e.kind == kind)
}

fn voice_provider_key_status(
    i18n: &I18nService,
    agent_settings: Option<&AgentProviderSettingsView>,
    api_keys: Option<&ApiKeysStatus>,
    provider: VoiceProviderKind,
) -> String {
    if provider == VoiceProviderKind::Aws {
        let Some(entry) = api_keys.and_then(|s| media_key_entry(s, "aws_polly")) else {
            return i18n.tr(I18nKey::AgApiKeyMissing)().to_string();
        };
        if entry.configured {
            if let Some(mask) = entry.masked_value.as_ref() {
                format!("{} ({mask})", i18n.tr(I18nKey::AgApiKeyConfigured)())
            } else {
                i18n.tr(I18nKey::AgApiKeyConfigured)().to_string()
            }
        } else {
            i18n.tr(I18nKey::AgApiKeyMissing)().to_string()
        }
    } else if let (Some(view), Some(agent)) = (agent_settings, voice_to_agent_provider(provider)) {
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
    } else {
        i18n.tr(I18nKey::AgApiKeyMissing)().to_string()
    }
}

async fn fetch_models_for(
    provider: VoiceProviderKind,
    kind: ModelKind,
    out: RwSignal<Vec<ProviderModelEntry>>,
) {
    if provider == VoiceProviderKind::Aws {
        out.set(curated_voice_models(provider, kind));
        return;
    }
    let Some(agent_provider) = voice_to_agent_provider(provider) else {
        out.set(Vec::new());
        return;
    };
    let all = match agent_provider_models(agent_provider).await {
        Ok(resp) => resp.entries,
        Err(_) => Vec::new(),
    };
    // Prefer audio-shaped models if the provider returns any; otherwise
    // surface the full list so the user can still pick something (especially
    // OpenRouter, whose /models endpoint does not flag transcription/TTS).
    let filtered: Vec<_> = all
        .iter()
        .filter(|m| kind.matches(&m.id))
        .cloned()
        .collect();
    let mut entries = if filtered.is_empty() { all } else { filtered };
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    out.set(entries);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GenderFilter {
    All,
    Male,
    Female,
    Neutral,
}

impl GenderFilter {
    fn matches(self, g: VoiceGender) -> bool {
        match (self, g) {
            (Self::All, _) => true,
            (Self::Male, VoiceGender::Male) => true,
            (Self::Female, VoiceGender::Female) => true,
            (Self::Neutral, VoiceGender::Neutral) => true,
            _ => false,
        }
    }
}

fn voice_providers() -> [VoiceProviderKind; 3] {
    [
        VoiceProviderKind::Openai,
        VoiceProviderKind::Openrouter,
        VoiceProviderKind::Aws,
    ]
}

fn voice_provider_icon_url(provider: VoiceProviderKind) -> &'static str {
    match provider {
        VoiceProviderKind::Openai => "/public/brand-icons/openai.svg",
        VoiceProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        VoiceProviderKind::Aws => "/public/brand-icons/aws.svg",
    }
}

fn voice_provider_label(i18n: &I18nService, provider: VoiceProviderKind) -> String {
    let key = match provider {
        VoiceProviderKind::Openai => I18nKey::AgProviderOpenai,
        VoiceProviderKind::Openrouter => I18nKey::AgProviderOpenrouter,
        VoiceProviderKind::Aws => I18nKey::AgProviderAws,
    };
    i18n.tr(key)().to_string()
}

fn apply_voice_provider_defaults(next: &mut VoiceSettings, provider: VoiceProviderKind) {
    if provider == VoiceProviderKind::Aws {
        if !next.stt.model_id.contains("transcribe") {
            next.stt.model_id = "amazon-transcribe".into();
        }
        if !matches!(next.tts.model_id.as_str(), "neural" | "standard") {
            next.tts.model_id = "neural".into();
        }
        if next.tts.voice.is_empty() || openai_voice_ids().contains(&next.tts.voice.as_str()) {
            next.tts.voice = "Joanna".into();
        }
    }
}

fn openai_voice_ids() -> &'static [&'static str] {
    &[
        "alloy", "ash", "ballad", "coral", "echo", "fable", "nova", "onyx", "sage", "shimmer",
    ]
}

fn focus_voice_provider_option(provider: VoiceProviderKind) {
    let id = format!("voice-provider-option-{}", provider.as_str());
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if let Some(el) = doc.get_element_by_id(&id) {
            let _ = el.dyn_into::<web_sys::HtmlElement>().map(|e| e.focus());
        }
    }
}

fn next_voice_provider(provider: VoiceProviderKind) -> VoiceProviderKind {
    let list = voice_providers();
    let i = list.iter().position(|&p| p == provider).unwrap_or(0);
    list[(i + 1) % list.len()]
}

fn prev_voice_provider(provider: VoiceProviderKind) -> VoiceProviderKind {
    let list = voice_providers();
    let i = list.iter().position(|&p| p == provider).unwrap_or(0);
    list[(i + list.len() - 1) % list.len()]
}

/// Voice settings column (BLXCode Agent grid, bottom row spanning both columns).
#[component]
pub fn AgentVoiceColumn() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings = RwSignal::new(Option::<VoiceSettings>::None);
    let agent_settings = RwSignal::new(Option::<AgentProviderSettingsView>::None);
    let api_keys = RwSignal::new(Option::<ApiKeysStatus>::None);
    let voices = RwSignal::new(Vec::<VoiceEntry>::new());
    let gender_filter = RwSignal::new(GenderFilter::All);
    let status = RwSignal::new(Option::<String>::None);
    let stt_models = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let tts_models = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let voice_provider = RwSignal::new(VoiceProviderKind::Openai);
    let stt_model_id = RwSignal::new(String::new());
    let tts_model_id = RwSignal::new(String::new());
    let stt_loading_models = RwSignal::new(false);
    let tts_loading_models = RwSignal::new(false);

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                agent_settings.set(Some(view));
            }
            if let Ok(keys) = api_keys_status().await {
                api_keys.set(Some(keys));
            }
            if let Ok(v) = voice_settings_get().await {
                voice_provider.set(v.stt.provider);
                stt_model_id.set(v.stt.model_id.clone());
                tts_model_id.set(v.tts.model_id.clone());
                if let Ok(catalog) = voice_provider_voices(v.tts.provider).await {
                    voices.set(catalog.voices);
                }
                fetch_models_for(v.stt.provider, ModelKind::Stt, stt_models).await;
                fetch_models_for(v.tts.provider, ModelKind::Tts, tts_models).await;
                settings.set(Some(v));
            }
        });
    }

    let save = move |patch: VoiceSettings| {
        voice_provider.set(patch.stt.provider);
        stt_model_id.set(patch.stt.model_id.clone());
        tts_model_id.set(patch.tts.model_id.clone());
        if !is_tauri_shell() {
            settings.set(Some(patch));
            return;
        }
        leptos::task::spawn_local(async move {
            match voice_settings_save(patch).await {
                Ok(v) => {
                    voice_provider.set(v.stt.provider);
                    stt_model_id.set(v.stt.model_id.clone());
                    tts_model_id.set(v.tts.model_id.clone());
                    settings.set(Some(v));
                    status.set(Some(i18n.tr(I18nKey::ApiKeysSaved)().to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let reload_tts_models = move |provider: VoiceProviderKind| {
        tts_loading_models.set(true);
        leptos::task::spawn_local(async move {
            if let Ok(catalog) = voice_provider_voices(provider).await {
                voices.set(catalog.voices);
            }
            fetch_models_for(provider, ModelKind::Tts, tts_models).await;
            tts_loading_models.set(false);
        });
    };

    let reload_stt_models = move |provider: VoiceProviderKind| {
        stt_loading_models.set(true);
        leptos::task::spawn_local(async move {
            fetch_models_for(provider, ModelKind::Stt, stt_models).await;
            stt_loading_models.set(false);
        });
    };

    let reload_all_for_provider = move |provider: VoiceProviderKind| {
        reload_tts_models(provider);
        reload_stt_models(provider);
    };

    view! {
        <>
            <h4 class="harness-pane-subhead agent-provider-pane__col-title">
                <span class="harness-pane-subhead__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuMic width="0.82rem" height="0.82rem" />
                </span>
                <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgColumnVoice)()}</span>
            </h4>

            <Show
                when=move || settings.get().is_some()
                fallback=move || view! {
                    <p class="voice-pane__loading">{move || i18n.tr(I18nKey::BlxLoading)()}</p>
                }
            >
                {move || {
                    let Some(current) = settings.get() else {
                        return view! { <></> }.into_any();
                    };

                    let sample_rate = current.stt.sample_rate_hz;
                    let voice_id = current.tts.voice.clone();
                    let post_flow = current.post_stt_flow;
                    let tts_enabled = current.tts.enabled;

                    let on_voice_provider = {
                        let current = current.clone();
                        move |p: VoiceProviderKind| {
                            voice_provider.set(p);
                            let mut next = current.clone();
                            next.stt.provider = p;
                            next.tts.provider = p;
                            apply_voice_provider_defaults(&mut next, p);
                            stt_model_id.set(next.stt.model_id.clone());
                            tts_model_id.set(next.tts.model_id.clone());
                            save(next);
                            reload_all_for_provider(p);
                        }
                    };

                    view! {
                        <div class="agent-provider-pane__voice-inner">
                            <div class="agent-provider-pane__voice-provider">
                                <label class="agent-provider-pane__field">
                                    <span class="harness-field-label">
                                        <span class="harness-field-label__icon" aria-hidden="true">
                                            <LxIcon icon=icondata::LuPlug width="0.82rem" height="0.82rem" />
                                        </span>
                                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderField)()}</span>
                                    </span>
                                    <VoiceProviderPicker
                                        selected_provider=voice_provider
                                        on_select=Callback::new(on_voice_provider)
                                    />
                                </label>
                                <div class="agent-provider-pane__key-row harness-muted">
                                    <span>{move || i18n.tr(I18nKey::ApiKeysManageHint)()}</span>
                                    <span class="agent-provider-pane__key-status">
                                        {move || {
                                            voice_provider_key_status(
                                                &i18n,
                                                agent_settings.get().as_ref(),
                                                api_keys.get().as_ref(),
                                                voice_provider.get(),
                                            )
                                        }}
                                    </span>
                                </div>
                            </div>

                            <SpeechSection
                                settings=settings
                                voice_provider=voice_provider
                                stt_model_id=stt_model_id
                                tts_model_id=tts_model_id
                                sample_rate=sample_rate
                                stt_models=stt_models
                                tts_models=tts_models
                                stt_loading_models=stt_loading_models
                                tts_loading_models=tts_loading_models
                                save=save
                                reload_stt_models=reload_stt_models
                                reload_tts_models=reload_tts_models
                            />
                            <BehaviorSection
                                settings=settings
                                voice_provider=voice_provider
                                post_flow=post_flow
                                voice_id=voice_id.clone()
                                voices=voices
                                gender_filter=gender_filter
                                tts_enabled=tts_enabled
                                save=save
                            />
                        </div>
                    }.into_any()
                }}
            </Show>

            <Show when=move || status.get().is_some()>
                <p class="voice-pane__status">{move || status.get().unwrap_or_default()}</p>
            </Show>
        </>
    }
}

#[component]
pub fn VoicePane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <section class="harness-settings-pane voice-pane" aria-labelledby="voice-pane-title">
            <header class="harness-settings-pane__head">
                <h2 id="voice-pane-title">
                    <LxIcon icon=icondata::LuMic width="1.1rem" height="1.1rem" />
                    <span>{move || i18n.tr(I18nKey::VoicePaneTitle)()}</span>
                </h2>
            </header>
            <div class="agent-provider-pane__col agent-provider-pane__col--standalone voice-pane-standalone">
                <AgentVoiceColumn />
            </div>
        </section>
    }
}

// ---------------------------------------------------------------------------
// Shared voice provider dropdown (STT + TTS)
// ---------------------------------------------------------------------------

#[component]
fn VoiceProviderPicker(
    selected_provider: RwSignal<VoiceProviderKind>,
    on_select: Callback<VoiceProviderKind>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |provider: VoiceProviderKind| {
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
                            focus_voice_provider_option(provider);
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
                                focus_voice_provider_option(provider);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let provider = prev_voice_provider(selected_provider.get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_voice_provider_option(provider);
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
                            src=move || voice_provider_icon_url(selected_provider.get())
                            alt=""
                        />
                    </span>
                    <span>{move || voice_provider_label(&i18n, selected_provider.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        voice_providers()
                            .into_iter()
                            .map(|provider| {
                                view! {
                                    <button
                                        id=format!("voice-provider-option-{}", provider.as_str())
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
                                                    focus_voice_provider_option(next_voice_provider(provider));
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    focus_voice_provider_option(prev_voice_provider(provider));
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
                                                src=voice_provider_icon_url(provider)
                                                alt=""
                                            />
                                        </span>
                                        <span>{voice_provider_label(&i18n, provider)}</span>
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

// ---------------------------------------------------------------------------
// Speech column (STT + TTS models, then recording quality)
// ---------------------------------------------------------------------------

#[component]
fn SpeechSection<F, RS, RT>(
    settings: RwSignal<Option<VoiceSettings>>,
    voice_provider: RwSignal<VoiceProviderKind>,
    stt_model_id: RwSignal<String>,
    tts_model_id: RwSignal<String>,
    sample_rate: u32,
    stt_models: RwSignal<Vec<ProviderModelEntry>>,
    tts_models: RwSignal<Vec<ProviderModelEntry>>,
    stt_loading_models: RwSignal<bool>,
    tts_loading_models: RwSignal<bool>,
    save: F,
    reload_stt_models: RS,
    reload_tts_models: RT,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
    RS: Fn(VoiceProviderKind) + Send + Sync + 'static + Copy,
    RT: Fn(VoiceProviderKind) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();

    let on_stt_model = Callback::new(move |m: String| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        stt_model_id.set(m.clone());
        next.stt.model_id = m;
        save(next);
    });

    let on_tts_model = Callback::new(move |m: String| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        tts_model_id.set(m.clone());
        next.tts.model_id = m;
        save(next);
    });

    let on_rate = move |r: u32| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.stt.sample_rate_hz = r;
        save(next);
    };

    let models_source_hint = move || {
        if voice_provider.get() == VoiceProviderKind::Aws {
            i18n.tr(I18nKey::AgModelsSourceCurated)().to_string()
        } else {
            i18n.tr(I18nKey::AgModelsSourceLive)().to_string()
        }
    };

    view! {
        <section class="voice-pane__section voice-pane__section--speech">
            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__text">
                        {move || format!(
                            "{} — {}",
                            i18n.tr(I18nKey::AgModelField)(),
                            i18n.tr(I18nKey::VoiceSttSection)()
                        )}
                    </span>
                </span>
                <AgentModelPicker
                    model_id=stt_model_id
                    model_entries=stt_models
                    loading_models=stt_loading_models
                    option_id_prefix="voice-stt-model"
                    show_custom_field=false
                    on_change=on_stt_model
                />
            </label>
            <div class="agent-provider-pane__actions">
                <button
                    type="button"
                    class="workbench-mini-btn"
                    disabled=move || stt_loading_models.get() || !is_tauri_shell()
                    on:click=move |_| reload_stt_models(voice_provider.get_untracked())
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                        <span>{move || if stt_loading_models.get() {
                            i18n.tr(I18nKey::AgModelsLoading)().to_string()
                        } else {
                            i18n.tr(I18nKey::AgModelsRefresh)().to_string()
                        }}</span>
                    </span>
                </button>
                <small class="harness-muted">{models_source_hint}</small>
            </div>

            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__text">
                        {move || format!(
                            "{} — {}",
                            i18n.tr(I18nKey::AgModelField)(),
                            i18n.tr(I18nKey::VoiceTtsSection)()
                        )}
                    </span>
                </span>
                <AgentModelPicker
                    model_id=tts_model_id
                    model_entries=tts_models
                    loading_models=tts_loading_models
                    option_id_prefix="voice-tts-model"
                    show_custom_field=false
                    on_change=on_tts_model
                />
            </label>
            <div class="agent-provider-pane__actions">
                <button
                    type="button"
                    class="workbench-mini-btn"
                    disabled=move || tts_loading_models.get() || !is_tauri_shell()
                    on:click=move |_| reload_tts_models(voice_provider.get_untracked())
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                        <span>{move || if tts_loading_models.get() {
                            i18n.tr(I18nKey::AgModelsLoading)().to_string()
                        } else {
                            i18n.tr(I18nKey::AgModelsRefresh)().to_string()
                        }}</span>
                    </span>
                </button>
                <small class="harness-muted">{models_source_hint}</small>
            </div>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceQualityField)()}</label>
                <div class="voice-pane__quality-row">
                    <QualityBtn rate=16000 label_key=I18nKey::VoiceQualityLow active=sample_rate on:click=move |_| on_rate(16_000) />
                    <QualityBtn rate=24000 label_key=I18nKey::VoiceQualityStandard active=sample_rate on:click=move |_| on_rate(24_000) />
                    <QualityBtn rate=48000 label_key=I18nKey::VoiceQualityHigh active=sample_rate on:click=move |_| on_rate(48_000) />
                </div>
                <p class="voice-pane__hint">{move || i18n.tr(I18nKey::VoiceQualityHint)()}</p>
                <p class="voice-pane__hint voice-pane__hint--small">
                    {move || format!(
                        "{}",
                        i18n.tr(I18nKey::VoiceQualitySizeEstimate)()
                            .to_string()
                            .replace("{kb}", &format!("{}", sample_rate * 2 * 10 / 1024))
                    )}
                </p>
            </div>
        </section>
    }
}

#[component]
fn QualityBtn(rate: u32, label_key: I18nKey, active: u32) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let is_active = move || rate == active;
    view! {
        <button
            type="button"
            class="voice-pane__choice"
            class:voice-pane__choice--active=is_active
        >
            {move || i18n.tr(label_key)()}
        </button>
    }
}

#[component]
fn GenderBtn(
    target: GenderFilter,
    label_key: I18nKey,
    filter: RwSignal<GenderFilter>,
    #[prop(default = false)] disabled: bool,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let is_active = move || filter.get() == target;
    view! {
        <button
            type="button"
            class="voice-pane__choice voice-pane__choice--gender"
            class:voice-pane__choice--active=is_active
            disabled=disabled
            on:click=move |_| {
                if !disabled {
                    filter.set(target);
                }
            }
        >
            {move || i18n.tr(label_key)()}
        </button>
    }
}

fn gender_label_for(g: VoiceGender) -> &'static str {
    match g {
        VoiceGender::Male => "♂",
        VoiceGender::Female => "♀",
        VoiceGender::Neutral => "○",
    }
}

fn play_b64(b64: &str, mime: &str) {
    let Ok(bytes) = BASE64.decode(b64) else {
        return;
    };
    let Ok(el) = HtmlAudioElement::new() else {
        return;
    };
    let arr = Uint8Array::new_with_length(bytes.len() as u32);
    arr.copy_from(&bytes);
    let parts = js_sys::Array::new();
    parts.push(&arr.buffer());
    let opts = BlobPropertyBag::new();
    opts.set_type(mime);
    let Ok(blob) = Blob::new_with_u8_array_sequence_and_options(&parts, &opts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let old = el.src();
    if old.starts_with("blob:") {
        let _ = web_sys::Url::revoke_object_url(&old);
    }
    el.set_src(&url);
    let _ = el.play();
}

#[component]
fn VoicePickCard(
    entry: VoiceEntry,
    active: bool,
    disabled: bool,
    settings: RwSignal<Option<VoiceSettings>>,
    on_pick: Callback<String>,
) -> impl IntoView {
    let id_pick = entry.id.clone();
    let id_preview = entry.id.clone();
    view! {
        <div
            class="voice-pane__voice-card"
            class:voice-pane__voice-card--active=move || active && !disabled
            class:voice-pane__voice-card--disabled=move || disabled
        >
            <button
                type="button"
                class="voice-pane__voice-pick"
                disabled=disabled
                on:click=move |_| {
                    if !disabled {
                        on_pick.run(id_pick.clone());
                    }
                }
            >
                <strong>{entry.label.clone()}</strong>
                <span class="voice-pane__voice-gender">{gender_label_for(entry.gender)}</span>
            </button>
            <button
                type="button"
                class="voice-pane__voice-preview"
                disabled=disabled
                on:click=move |_| {
                    if disabled {
                        return;
                    }
                    let Some(s) = settings.get_untracked() else {
                        return;
                    };
                    let model = s.tts.model_id.clone();
                    let provider = s.tts.provider;
                    let voice = id_preview.clone();
                    leptos::task::spawn_local(async move {
                        let text = expect_context::<I18nService>()
                            .tr(I18nKey::VoicePreviewText)()
                            .to_string();
                        if let Ok(resp) = voice_tts_preview(provider, model, voice, text).await {
                            play_b64(&resp.audio_b64, &resp.mime);
                        }
                    });
                }
                aria-label="Sample"
            >
                <LxIcon icon=icondata::LuPlay width="0.85rem" height="0.85rem" />
            </button>
        </div>
    }
}

#[component]
fn VoicePicksGrid(
    settings: RwSignal<Option<VoiceSettings>>,
    voice_id: String,
    voices: RwSignal<Vec<VoiceEntry>>,
    gender_filter: RwSignal<GenderFilter>,
    voices_pick_enabled: Memo<bool>,
    on_pick: Callback<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="voice-pane__voice-picks">
            <div class="voice-pane__gender-row">
                <GenderBtn target=GenderFilter::All label_key=I18nKey::VoiceGenderAll filter=gender_filter />
                <GenderBtn target=GenderFilter::Male label_key=I18nKey::VoiceGenderMale filter=gender_filter />
                <GenderBtn target=GenderFilter::Female label_key=I18nKey::VoiceGenderFemale filter=gender_filter />
                <GenderBtn target=GenderFilter::Neutral label_key=I18nKey::VoiceGenderNeutral filter=gender_filter />
            </div>
            <p
                class="voice-pane__hint voice-pane__hint--aws-only"
                class:voice-pane__hint--visible=move || !voices_pick_enabled.get()
            >
                {move || i18n.tr(I18nKey::VoiceVoicesAwsOnly)()}
            </p>
            <div
                class="voice-pane__voice-grid voice-pane__voice-grid--six"
                class:voice-pane__voice-grid--disabled=move || !voices_pick_enabled.get()
                aria-disabled=move || if voices_pick_enabled.get() { "false" } else { "true" }
            >
                {move || {
                    let active = voice_id.clone();
                    let filter = gender_filter.get();
                    let picks_disabled = !voices_pick_enabled.get();
                    voices.get()
                        .into_iter()
                        .filter(|v| filter.matches(v.gender))
                        .map(|v| {
                            view! {
                                <VoicePickCard
                                    entry=v.clone()
                                    active=v.id == active
                                    disabled=picks_disabled
                                    settings=settings
                                    on_pick=on_pick
                                />
                            }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn VoicePickerBlock(
    settings: RwSignal<Option<VoiceSettings>>,
    voice_provider: RwSignal<VoiceProviderKind>,
    voice_id: String,
    voices: RwSignal<Vec<VoiceEntry>>,
    gender_filter: RwSignal<GenderFilter>,
    on_pick: Callback<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let voices_pick_enabled =
        Memo::new(move |_| voice_provider.get() == VoiceProviderKind::Aws);

    view! {
        <div class="voice-pane__field">
            <label>{move || i18n.tr(I18nKey::VoiceVoiceField)()}</label>
            <VoicePicksGrid
                settings=settings
                voice_id=voice_id.clone()
                voices=voices
                gender_filter=gender_filter
                voices_pick_enabled=voices_pick_enabled
                on_pick=on_pick
            />
        </div>
    }
}

// ---------------------------------------------------------------------------
// Behavior section
// ---------------------------------------------------------------------------

#[component]
fn BehaviorSection<F>(
    settings: RwSignal<Option<VoiceSettings>>,
    voice_provider: RwSignal<VoiceProviderKind>,
    post_flow: PostSttFlow,
    voice_id: String,
    voices: RwSignal<Vec<VoiceEntry>>,
    gender_filter: RwSignal<GenderFilter>,
    tts_enabled: bool,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();

    let on_flow = Callback::new(move |flow: PostSttFlow| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.post_stt_flow = flow;
        save(next);
    });
    let on_voice = Callback::new(move |id: String| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.tts.voice = id;
        save(next);
    });
    let on_enabled = Callback::new(move |enabled: bool| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.tts.enabled = enabled;
        save(next);
    });

    view! {
        <section class="voice-pane__section">
            <h3>{move || i18n.tr(I18nKey::VoiceBehaviorSection)()}</h3>
            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoicePostSttFlow)()}</label>
                <div class="voice-pane__radio-row">
                    <button
                        type="button"
                        class="voice-pane__choice"
                        class:voice-pane__choice--active=move || matches!(post_flow, PostSttFlow::AutoSend)
                        on:click=move |_| on_flow.run(PostSttFlow::AutoSend)
                    >
                        {move || i18n.tr(I18nKey::VoicePostSttAutoSend)()}
                    </button>
                    <button
                        type="button"
                        class="voice-pane__choice"
                        class:voice-pane__choice--active=move || matches!(post_flow, PostSttFlow::Draft)
                        on:click=move |_| on_flow.run(PostSttFlow::Draft)
                    >
                        {move || i18n.tr(I18nKey::VoicePostSttDraft)()}
                    </button>
                </div>
            </div>

            <VoicePickerBlock
                settings=settings
                voice_provider=voice_provider
                voice_id=voice_id
                voices=voices
                gender_filter=gender_filter
                on_pick=on_voice
            />

            <div class="voice-pane__field">
                <label class="voice-pane__toggle">
                    <input
                        type="checkbox"
                        prop:checked=tts_enabled
                        on:change=move |ev| {
                            if let Some(t) = ev.target() {
                                if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                    on_enabled.run(inp.checked());
                                }
                            }
                        }
                    />
                    <span>{move || i18n.tr(I18nKey::VoiceTtsEnabled)()}</span>
                </label>
                <p class="voice-pane__hint">{move || i18n.tr(I18nKey::VoiceTtsAutoplayHint)()}</p>
                <p class="voice-pane__hint voice-pane__hint--small">
                    {move || i18n.tr(I18nKey::VoiceTtsLangAutoNote)()}
                </p>
            </div>

        </section>
    }
}

// Convince the compiler we still need these types (referenced via trait bounds only).
#[allow(dead_code)]
fn _ensure_types(_s: SttSettings, _t: TtsSettings) {}
