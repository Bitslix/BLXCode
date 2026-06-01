//! BLXCode Agent voice settings: cloud STT/TTS and reply voice selection.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gloo_timers::future::TimeoutFuture;
use js_sys::Uint8Array;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement};

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, api_keys_status, is_tauri_shell, voice_settings_get,
    voice_settings_save, voice_tts_preview, AgentProviderKind, AgentProviderSettingsView,
    ApiKeyEntry, ApiKeysStatus, PostSttFlow, ProviderModelEntry, VoiceEntry, VoiceGender,
    VoiceProviderKind, VoiceSettings,
};
use crate::workbench::agent_model_picker::AgentModelPicker;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelKind {
    Stt,
    Tts,
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
        matches!(
            (self, g),
            (Self::All, _)
                | (Self::Male, VoiceGender::Male)
                | (Self::Female, VoiceGender::Female)
                | (Self::Neutral, VoiceGender::Neutral)
        )
    }
}

#[component]
pub fn AgentVoiceSettings() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings = RwSignal::new(Option::<VoiceSettings>::None);
    let agent_settings = RwSignal::new(Option::<AgentProviderSettingsView>::None);
    let api_keys = RwSignal::new(Option::<ApiKeysStatus>::None);
    let status = RwSignal::new(Option::<String>::None);
    let provider = RwSignal::new(VoiceProviderKind::Openai);
    let stt_model_id = RwSignal::new(String::new());
    let tts_model_id = RwSignal::new(String::new());
    let stt_models = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let tts_models = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let stt_loading = RwSignal::new(false);
    let tts_loading = RwSignal::new(false);
    let gender_filter = RwSignal::new(GenderFilter::All);

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                agent_settings.set(Some(view));
            }
            if let Ok(keys) = api_keys_status().await {
                api_keys.set(Some(keys));
            }
            if let Ok(v) = voice_settings_get().await {
                provider.set(v.tts.provider);
                stt_model_id.set(v.stt.model_id.clone());
                tts_model_id.set(v.tts.model_id.clone());
                fetch_models_for(v.stt.provider, ModelKind::Stt, stt_models).await;
                fetch_models_for(v.tts.provider, ModelKind::Tts, tts_models).await;
                settings.set(Some(v));
            }
        });
    }

    let save = move |patch: VoiceSettings| {
        provider.set(patch.tts.provider);
        stt_model_id.set(patch.stt.model_id.clone());
        tts_model_id.set(patch.tts.model_id.clone());
        if !is_tauri_shell() {
            settings.set(Some(patch));
            return;
        }
        leptos::task::spawn_local(async move {
            match voice_settings_save(patch).await {
                Ok(v) => {
                    provider.set(v.tts.provider);
                    stt_model_id.set(v.stt.model_id.clone());
                    tts_model_id.set(v.tts.model_id.clone());
                    settings.set(Some(v));
                    status.set(Some(i18n.tr(I18nKey::VoiceSaveDone)().to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let reload_stt = move |p: VoiceProviderKind| {
        stt_loading.set(true);
        leptos::task::spawn_local(async move {
            fetch_models_for(p, ModelKind::Stt, stt_models).await;
            stt_loading.set(false);
        });
    };
    let reload_tts = move |p: VoiceProviderKind| {
        tts_loading.set(true);
        leptos::task::spawn_local(async move {
            fetch_models_for(p, ModelKind::Tts, tts_models).await;
            tts_loading.set(false);
        });
    };
    let reload_all = move |p: VoiceProviderKind| {
        reload_stt(p);
        reload_tts(p);
    };

    view! {
        <div class="agent-provider-pane__col agent-provider-pane__col--span-2 agent-provider-pane__voice-col">
            <h4 class="harness-pane-subhead agent-provider-pane__col-title">
                <span class="harness-pane-subhead__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuVolume2 width="0.82rem" height="0.82rem" />
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
                    let post_flow = current.post_stt_flow;
                    let voice_id = current.tts.voice.clone();
                    let tts_enabled = current.tts.enabled;

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
                                        selected_provider=provider
                                        on_select=Callback::new(move |p| {
                                            let mut next = current.clone();
                                            provider.set(p);
                                            next.stt.provider = p;
                                            next.tts.provider = p;
                                            apply_provider_defaults(&mut next, p);
                                            save(next);
                                            reload_all(p);
                                        })
                                    />
                                </label>
                                <div class="agent-provider-pane__key-row">
                                    <span>{move || i18n.tr(I18nKey::ApiKeysManageHint)()}</span>
                                    <span class="agent-provider-pane__key-status">
                                        {move || key_status(
                                            &i18n,
                                            agent_settings.get().as_ref(),
                                            api_keys.get().as_ref(),
                                            provider.get(),
                                        )}
                                    </span>
                                </div>
                            </div>

                            <SpeechModels
                                settings=settings
                                provider=provider
                                stt_model_id=stt_model_id
                                tts_model_id=tts_model_id
                                sample_rate=sample_rate
                                stt_models=stt_models
                                tts_models=tts_models
                                stt_loading=stt_loading
                                tts_loading=tts_loading
                                save=save
                                reload_stt=reload_stt
                                reload_tts=reload_tts
                            />
                            <VoiceBehavior
                                settings=settings
                                provider=provider
                                post_flow=post_flow
                                voice_id=voice_id
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
        </div>
    }
}

#[component]
fn SpeechModels<F, RS, RT>(
    settings: RwSignal<Option<VoiceSettings>>,
    provider: RwSignal<VoiceProviderKind>,
    stt_model_id: RwSignal<String>,
    tts_model_id: RwSignal<String>,
    sample_rate: u32,
    stt_models: RwSignal<Vec<ProviderModelEntry>>,
    tts_models: RwSignal<Vec<ProviderModelEntry>>,
    stt_loading: RwSignal<bool>,
    tts_loading: RwSignal<bool>,
    save: F,
    reload_stt: RS,
    reload_tts: RT,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
    RS: Fn(VoiceProviderKind) + Send + Sync + 'static + Copy,
    RT: Fn(VoiceProviderKind) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let set_stt = Callback::new(move |model: String| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        stt_model_id.set(model.clone());
        next.stt.model_id = model;
        save(next);
    });
    let set_tts = Callback::new(move |model: String| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        tts_model_id.set(model.clone());
        next.tts.model_id = model;
        save(next);
    });
    let set_rate = move |rate: u32| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.stt.sample_rate_hz = rate;
        save(next);
    };
    let source_hint = move || {
        if provider.get() == VoiceProviderKind::Aws {
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
                        {move || format!("{} — {}", i18n.tr(I18nKey::AgModelField)(), i18n.tr(I18nKey::VoiceSttSection)())}
                    </span>
                </span>
                <AgentModelPicker
                    model_id=stt_model_id
                    model_entries=stt_models
                    loading_models=stt_loading
                    option_id_prefix="agent-voice-stt-model"
                    show_custom_field=false
                    on_change=set_stt
                />
            </label>
            <RefreshRow loading=stt_loading source=Signal::derive(source_hint) on_refresh=move || reload_stt(provider.get_untracked()) />

            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__text">
                        {move || format!("{} — {}", i18n.tr(I18nKey::AgModelField)(), i18n.tr(I18nKey::VoiceTtsSection)())}
                    </span>
                </span>
                <AgentModelPicker
                    model_id=tts_model_id
                    model_entries=tts_models
                    loading_models=tts_loading
                    option_id_prefix="agent-voice-tts-model"
                    show_custom_field=false
                    on_change=set_tts
                />
            </label>
            <RefreshRow loading=tts_loading source=Signal::derive(source_hint) on_refresh=move || reload_tts(provider.get_untracked()) />

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceQualityField)()}</label>
                <div class="voice-pane__quality-row">
                    <button type="button" class="voice-pane__choice" class:voice-pane__choice--active=move || sample_rate == 16_000 on:click=move |_| set_rate(16_000)>
                        {move || i18n.tr(I18nKey::VoiceQualityLow)()}
                    </button>
                    <button type="button" class="voice-pane__choice" class:voice-pane__choice--active=move || sample_rate == 24_000 on:click=move |_| set_rate(24_000)>
                        {move || i18n.tr(I18nKey::VoiceQualityStandard)()}
                    </button>
                    <button type="button" class="voice-pane__choice" class:voice-pane__choice--active=move || sample_rate == 48_000 on:click=move |_| set_rate(48_000)>
                        {move || i18n.tr(I18nKey::VoiceQualityHigh)()}
                    </button>
                </div>
                <p class="voice-pane__hint">{move || i18n.tr(I18nKey::VoiceQualityHint)()}</p>
            </div>
        </section>
    }
}

#[component]
fn RefreshRow<R>(loading: RwSignal<bool>, source: Signal<String>, on_refresh: R) -> impl IntoView
where
    R: Fn() + Send + Sync + Copy + 'static,
{
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="agent-provider-pane__actions">
            <button
                type="button"
                class="workbench-mini-btn"
                disabled=move || loading.get() || !is_tauri_shell()
                on:click=move |_| on_refresh()
            >
                <span class="harness-btn-inline">
                    <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                    <span>{move || if loading.get() {
                        i18n.tr(I18nKey::AgModelsLoading)().to_string()
                    } else {
                        i18n.tr(I18nKey::AgModelsRefresh)().to_string()
                    }}</span>
                </span>
            </button>
            <small class="harness-muted">{move || source.get()}</small>
        </div>
    }
}

#[component]
fn VoiceBehavior<F>(
    settings: RwSignal<Option<VoiceSettings>>,
    provider: RwSignal<VoiceProviderKind>,
    post_flow: PostSttFlow,
    voice_id: String,
    gender_filter: RwSignal<GenderFilter>,
    tts_enabled: bool,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let set_flow = move |flow: PostSttFlow| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.post_stt_flow = flow;
        save(next);
    };
    let set_voice = Callback::new(move |voice: String| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.tts.voice = voice;
        save(next);
    });
    let set_enabled = move |enabled: bool| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.tts.enabled = enabled;
        save(next);
    };

    view! {
        <section class="voice-pane__section">
            <h3>{move || i18n.tr(I18nKey::VoiceBehaviorSection)()}</h3>
            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoicePostSttFlow)()}</label>
                <div class="voice-pane__radio-row">
                    <button type="button" class="voice-pane__choice" class:voice-pane__choice--active=move || post_flow == PostSttFlow::AutoSend on:click=move |_| set_flow(PostSttFlow::AutoSend)>
                        {move || i18n.tr(I18nKey::VoicePostSttAutoSend)()}
                    </button>
                    <button type="button" class="voice-pane__choice" class:voice-pane__choice--active=move || post_flow == PostSttFlow::Draft on:click=move |_| set_flow(PostSttFlow::Draft)>
                        {move || i18n.tr(I18nKey::VoicePostSttDraft)()}
                    </button>
                </div>
            </div>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceVoiceField)()}</label>
                <div class="voice-pane__gender-row">
                    <GenderButton target=GenderFilter::All key=I18nKey::VoiceGenderAll filter=gender_filter />
                    <GenderButton target=GenderFilter::Male key=I18nKey::VoiceGenderMale filter=gender_filter />
                    <GenderButton target=GenderFilter::Female key=I18nKey::VoiceGenderFemale filter=gender_filter />
                    <GenderButton target=GenderFilter::Neutral key=I18nKey::VoiceGenderNeutral filter=gender_filter />
                </div>
                <p
                    class="voice-pane__hint voice-pane__hint--aws-only"
                    class:voice-pane__hint--visible=move || !voices_pick_enabled(provider.get())
                >
                    {move || i18n.tr(I18nKey::VoiceVoicesAwsOnly)()}
                </p>
                <div
                    class="voice-pane__voice-grid voice-pane__voice-grid--six"
                    class:voice-pane__voice-grid--disabled=move || !voices_pick_enabled(provider.get())
                    aria-disabled=move || if voices_pick_enabled(provider.get()) { "false" } else { "true" }
                >
                    {move || {
                        let active = voice_id.clone();
                        let disabled = !voices_pick_enabled(provider.get());
                        let filter = gender_filter.get();
                        voice_catalog_for(provider.get())
                            .into_iter()
                            .filter(|entry| filter.matches(entry.gender))
                            .map(|entry| {
                                view! {
                                    <VoiceCard
                                        entry=entry.clone()
                                        active=entry.id == active
                                        disabled=disabled
                                        settings=settings
                                        on_pick=set_voice
                                    />
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </div>

            <div class="voice-pane__field">
                <label class="voice-pane__toggle">
                    <input
                        type="checkbox"
                        prop:checked=tts_enabled
                        on:change=move |ev| set_enabled(event_target_checked(&ev))
                    />
                    <span>{move || i18n.tr(I18nKey::VoiceTtsEnabled)()}</span>
                </label>
                <p class="voice-pane__hint">{move || i18n.tr(I18nKey::VoiceTtsAutoplayHint)()}</p>
                <p class="voice-pane__hint voice-pane__hint--small">{move || i18n.tr(I18nKey::VoiceTtsLangAutoNote)()}</p>
            </div>
        </section>
    }
}

#[component]
fn GenderButton(
    target: GenderFilter,
    key: I18nKey,
    filter: RwSignal<GenderFilter>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="voice-pane__choice"
            class:voice-pane__choice--active=move || filter.get() == target
            on:click=move |_| filter.set(target)
        >
            {move || i18n.tr(key)()}
        </button>
    }
}

#[component]
fn VoiceCard(
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
                on:click=move |_| if !disabled { on_pick.run(id_pick.clone()) }
            >
                <strong>{entry.label.clone()}</strong>
                <span class="voice-pane__voice-gender">{gender_icon(entry.gender)}</span>
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
                        let text = expect_context::<I18nService>().tr(I18nKey::VoicePreviewText)().to_string();
                        if let Ok(resp) = voice_tts_preview(provider, model, voice, text).await {
                            play_b64(&resp.audio_b64, &resp.mime);
                        }
                    });
                }
            >
                <LxIcon icon=icondata::LuPlay width="0.85rem" height="0.85rem" />
            </button>
        </div>
    }
}

#[component]
fn VoiceProviderPicker(
    selected_provider: RwSignal<VoiceProviderKind>,
    on_select: Callback<VoiceProviderKind>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);
    let choose = move |p: VoiceProviderKind| {
        selected_provider.set(p);
        open.set(false);
        on_select.run(p);
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
                        let p = selected_provider.get_untracked();
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(0).await;
                            focus_provider_option(p);
                        });
                    }
                }
            >
                <span class="harness-provider-trigger__main">
                    <span class="harness-provider-trigger__brand">
                        <img class="harness-provider-trigger__img" src=move || provider_icon_url(selected_provider.get()) alt="" />
                    </span>
                    <span>{move || provider_label(&i18n, selected_provider.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || voice_providers()
                        .into_iter()
                        .map(|p| view! {
                            <button
                                id=format!("agent-voice-provider-option-{}", p.as_str())
                                type="button"
                                role="option"
                                class="harness-provider-option"
                                class:harness-provider-option--active=move || selected_provider.get() == p
                                on:click=move |_| choose(p)
                            >
                                <span class="harness-provider-option__brand">
                                    <img class="harness-provider-option__img" src=provider_icon_url(p) alt="" />
                                </span>
                                <span>{provider_label(&i18n, p)}</span>
                            </button>
                        })
                        .collect_view()
                    }
                </div>
            </Show>
        </div>
    }
}

async fn fetch_models_for(
    provider: VoiceProviderKind,
    kind: ModelKind,
    out: RwSignal<Vec<ProviderModelEntry>>,
) {
    if provider == VoiceProviderKind::Aws {
        out.set(match kind {
            ModelKind::Stt => vec![model_entry(
                "amazon-transcribe",
                "Amazon Transcribe",
                "AWS speech-to-text.",
            )],
            ModelKind::Tts => vec![
                model_entry("neural", "Polly Neural", "Neural TTS engine."),
                model_entry("standard", "Polly Standard", "Standard TTS engine."),
            ],
        });
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
    let filtered: Vec<_> = all
        .iter()
        .filter(|m| model_kind_matches(kind, &m.id))
        .cloned()
        .collect();
    let mut entries = if filtered.is_empty() { all } else { filtered };
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    out.set(entries);
}

fn model_kind_matches(kind: ModelKind, id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    match kind {
        ModelKind::Stt => lower.contains("transcribe") || lower.contains("whisper"),
        ModelKind::Tts => lower.contains("tts") || lower.contains("speech"),
    }
}

fn model_entry(id: &str, label: &str, description: &str) -> ProviderModelEntry {
    ProviderModelEntry {
        id: id.into(),
        label: label.into(),
        description: Some(description.into()),
        pricing: None,
        context_length: None,
    }
}

fn voice_to_agent_provider(v: VoiceProviderKind) -> Option<AgentProviderKind> {
    match v {
        VoiceProviderKind::Openai => Some(AgentProviderKind::Openai),
        VoiceProviderKind::Openrouter => Some(AgentProviderKind::Openrouter),
        VoiceProviderKind::Aws => None,
    }
}

fn media_key_entry<'a>(api_keys: &'a ApiKeysStatus, kind: &str) -> Option<&'a ApiKeyEntry> {
    api_keys.entries.iter().find(|e| e.kind == kind)
}

fn key_status(
    i18n: &I18nService,
    agent_settings: Option<&AgentProviderSettingsView>,
    api_keys: Option<&ApiKeysStatus>,
    provider: VoiceProviderKind,
) -> String {
    if provider == VoiceProviderKind::Aws {
        let Some(entry) = api_keys.and_then(|s| media_key_entry(s, "aws_polly")) else {
            return i18n.tr(I18nKey::AgApiKeyMissing)().to_string();
        };
        return if entry.configured {
            entry
                .masked_value
                .as_ref()
                .map(|mask| format!("{} ({mask})", i18n.tr(I18nKey::AgApiKeyConfigured)()))
                .unwrap_or_else(|| i18n.tr(I18nKey::AgApiKeyConfigured)().to_string())
        } else {
            i18n.tr(I18nKey::AgApiKeyMissing)().to_string()
        };
    }

    let Some(agent) = voice_to_agent_provider(provider) else {
        return i18n.tr(I18nKey::AgApiKeyMissing)().to_string();
    };
    let Some(view) = agent_settings else {
        return i18n.tr(I18nKey::AgApiKeyMissing)().to_string();
    };
    let Some(status) = view.key_statuses.iter().find(|s| s.provider == agent) else {
        return i18n.tr(I18nKey::AgApiKeyMissing)().to_string();
    };
    if status.configured {
        status
            .masked_value
            .as_ref()
            .map(|mask| format!("{} ({mask})", i18n.tr(I18nKey::AgApiKeyConfigured)()))
            .unwrap_or_else(|| i18n.tr(I18nKey::AgApiKeyConfigured)().to_string())
    } else {
        i18n.tr(I18nKey::AgApiKeyMissing)().to_string()
    }
}

fn apply_provider_defaults(next: &mut VoiceSettings, provider: VoiceProviderKind) {
    match provider {
        VoiceProviderKind::Aws => {
            if !next.stt.model_id.contains("transcribe") {
                next.stt.model_id = "amazon-transcribe".into();
            }
            if !matches!(next.tts.model_id.as_str(), "neural" | "standard") {
                next.tts.model_id = "neural".into();
            }
            if next.tts.voice.is_empty() || is_openai_voice_id(&next.tts.voice) {
                next.tts.voice = "Joanna".into();
            }
        }
        VoiceProviderKind::Openai => {
            if next.tts.voice.is_empty() || is_aws_voice_id(&next.tts.voice) {
                next.tts.voice = "nova".into();
            }
        }
        VoiceProviderKind::Openrouter => {}
    }
}

fn voice_entry(id: &str, label: &str, gender: VoiceGender) -> VoiceEntry {
    VoiceEntry {
        id: id.into(),
        label: label.into(),
        gender,
    }
}

fn voice_catalog_for(provider: VoiceProviderKind) -> Vec<VoiceEntry> {
    match provider {
        VoiceProviderKind::Openai | VoiceProviderKind::Openrouter => vec![
            voice_entry("alloy", "Alloy", VoiceGender::Neutral),
            voice_entry("nova", "Nova", VoiceGender::Female),
            voice_entry("echo", "Echo", VoiceGender::Male),
            voice_entry("shimmer", "Shimmer", VoiceGender::Female),
            voice_entry("onyx", "Onyx", VoiceGender::Male),
            voice_entry("coral", "Coral", VoiceGender::Female),
        ],
        VoiceProviderKind::Aws => vec![
            voice_entry("Joanna", "Joanna", VoiceGender::Female),
            voice_entry("Matthew", "Matthew", VoiceGender::Male),
            voice_entry("Amy", "Amy", VoiceGender::Female),
            voice_entry("Brian", "Brian", VoiceGender::Male),
            voice_entry("Emma", "Emma", VoiceGender::Female),
            voice_entry("Arthur", "Arthur", VoiceGender::Male),
        ],
    }
}

fn voices_pick_enabled(provider: VoiceProviderKind) -> bool {
    matches!(provider, VoiceProviderKind::Openai | VoiceProviderKind::Aws)
}

fn is_openai_voice_id(id: &str) -> bool {
    matches!(id, "alloy" | "nova" | "echo" | "shimmer" | "onyx" | "coral")
}

fn is_aws_voice_id(id: &str) -> bool {
    matches!(
        id,
        "Joanna" | "Matthew" | "Amy" | "Brian" | "Emma" | "Arthur"
    )
}

fn voice_providers() -> [VoiceProviderKind; 3] {
    [
        VoiceProviderKind::Openai,
        VoiceProviderKind::Openrouter,
        VoiceProviderKind::Aws,
    ]
}

fn provider_icon_url(provider: VoiceProviderKind) -> &'static str {
    match provider {
        VoiceProviderKind::Openai => "/public/brand-icons/openai.svg",
        VoiceProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        VoiceProviderKind::Aws => "/public/brand-icons/aws.svg",
    }
}

fn provider_label(i18n: &I18nService, provider: VoiceProviderKind) -> String {
    match provider {
        VoiceProviderKind::Openai => i18n.tr(I18nKey::AgProviderOpenai)().to_string(),
        VoiceProviderKind::Openrouter => i18n.tr(I18nKey::AgProviderOpenrouter)().to_string(),
        VoiceProviderKind::Aws => i18n.tr(I18nKey::AgProviderAws)().to_string(),
    }
}

fn focus_provider_option(provider: VoiceProviderKind) {
    let id = format!("agent-voice-provider-option-{}", provider.as_str());
    if let Some(el) = document().get_element_by_id(&id) {
        if let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() {
            let _ = html.focus();
        }
    }
}

fn gender_icon(g: VoiceGender) -> &'static str {
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
    el.set_src(&url);
    let _ = el.play();
}
