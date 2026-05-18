//! Voice settings tab: STT/TTS provider+model, voice with gender filter,
//! recording quality, post-STT behaviour, STT language, push-to-talk hotkey.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, voice_provider_voices, voice_settings_get, voice_settings_save,
    voice_tts_preview, PostSttFlow, PttHotkey, SttLanguageMode, SttSettings, TtsSettings,
    VoiceEntry, VoiceGender, VoiceProviderKind, VoiceSettings,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use js_sys::Uint8Array;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement};

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

#[component]
pub fn VoicePane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings = RwSignal::new(Option::<VoiceSettings>::None);
    let voices = RwSignal::new(Vec::<VoiceEntry>::new());
    let gender_filter = RwSignal::new(GenderFilter::All);
    let status = RwSignal::new(Option::<String>::None);
    let recording_hotkey = RwSignal::new(false);

    // Load current settings + initial voice catalogue.
    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(v) = voice_settings_get().await {
                if let Ok(catalog) = voice_provider_voices(v.tts.provider).await {
                    voices.set(catalog.voices);
                }
                settings.set(Some(v));
            }
        });
    }

    let save = move |patch: VoiceSettings| {
        if !is_tauri_shell() {
            settings.set(Some(patch));
            return;
        }
        leptos::task::spawn_local(async move {
            match voice_settings_save(patch).await {
                Ok(v) => {
                    settings.set(Some(v));
                    status.set(Some("saved".into()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    let reload_voices = move |provider: VoiceProviderKind| {
        leptos::task::spawn_local(async move {
            if let Ok(catalog) = voice_provider_voices(provider).await {
                voices.set(catalog.voices);
            }
        });
    };

    view! {
        <section class="harness-settings-pane voice-pane" aria-labelledby="voice-pane-title">
            <header class="harness-settings-pane__head">
                <h2 id="voice-pane-title">
                    <LxIcon icon=icondata::LuMic width="1.1rem" height="1.1rem" />
                    <span>{move || i18n.tr(I18nKey::VoicePaneTitle)()}</span>
                </h2>
            </header>

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

                    let stt_provider = current.stt.provider;
                    let tts_provider = current.tts.provider;
                    let stt_model = current.stt.model_id.clone();
                    let tts_model = current.tts.model_id.clone();
                    let sample_rate = current.stt.sample_rate_hz;
                    let voice_id = current.tts.voice.clone();
                    let post_flow = current.post_stt_flow;
                    let stt_lang = current.stt_language.clone();
                    let ptt = current.ptt_hotkey.clone();
                    let tts_enabled = current.tts.enabled;

                    view! {
                        <SttSection
                            current=current.clone()
                            stt_provider=stt_provider
                            stt_model=stt_model.clone()
                            sample_rate=sample_rate
                            save=save
                        />
                        <TtsSection
                            current=current.clone()
                            tts_provider=tts_provider
                            tts_model=tts_model.clone()
                            voice_id=voice_id.clone()
                            voices=voices
                            gender_filter=gender_filter
                            tts_enabled=tts_enabled
                            save=save
                            reload_voices=reload_voices
                        />
                        <BehaviorSection
                            current=current.clone()
                            post_flow=post_flow
                            save=save
                        />
                        <LanguageSection
                            current=current.clone()
                            stt_lang=stt_lang.clone()
                            save=save
                        />
                        <PttSection
                            current=current.clone()
                            ptt=ptt.clone()
                            recording=recording_hotkey
                            save=save
                        />
                    }.into_any()
                }}
            </Show>

            <Show when=move || status.get().is_some()>
                <p class="voice-pane__status">{move || status.get().unwrap_or_default()}</p>
            </Show>
        </section>
    }
}

// ---------------------------------------------------------------------------
// STT section
// ---------------------------------------------------------------------------

#[component]
fn SttSection<F>(
    current: VoiceSettings,
    stt_provider: VoiceProviderKind,
    stt_model: String,
    sample_rate: u32,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let on_provider = {
        let current = current.clone();
        move |p: VoiceProviderKind| {
            let mut next = current.clone();
            next.stt.provider = p;
            save(next);
        }
    };
    let on_model = {
        let current = current.clone();
        move |m: String| {
            let mut next = current.clone();
            next.stt.model_id = m;
            save(next);
        }
    };
    let on_rate = {
        let current = current.clone();
        move |r: u32| {
            let mut next = current.clone();
            next.stt.sample_rate_hz = r;
            save(next);
        }
    };

    view! {
        <section class="voice-pane__section">
            <h3>{move || i18n.tr(I18nKey::VoiceSttSection)()}</h3>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceProviderField)()}</label>
                <div class="voice-pane__provider-row">
                    <ProviderBtn label="OpenAI" target=VoiceProviderKind::Openai active=stt_provider on:click={
                        let on_provider = on_provider.clone();
                        move |_| on_provider(VoiceProviderKind::Openai)
                    } />
                    <ProviderBtn label="OpenRouter" target=VoiceProviderKind::Openrouter active=stt_provider on:click={
                        let on_provider = on_provider.clone();
                        move |_| on_provider(VoiceProviderKind::Openrouter)
                    } />
                </div>
            </div>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceModelField)()}</label>
                <input
                    type="text"
                    class="voice-pane__input"
                    prop:value=stt_model.clone()
                    on:change={
                        let on_model = on_model.clone();
                        move |ev| {
                            if let Some(t) = ev.target() {
                                if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                    on_model(inp.value());
                                }
                            }
                        }
                    }
                />
            </div>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceQualityField)()}</label>
                <div class="voice-pane__quality-row">
                    <QualityBtn rate=16000 label_key=I18nKey::VoiceQualityLow active=sample_rate on:click={
                        let on_rate = on_rate.clone();
                        move |_| on_rate(16_000)
                    } />
                    <QualityBtn rate=24000 label_key=I18nKey::VoiceQualityStandard active=sample_rate on:click={
                        let on_rate = on_rate.clone();
                        move |_| on_rate(24_000)
                    } />
                    <QualityBtn rate=48000 label_key=I18nKey::VoiceQualityHigh active=sample_rate on:click={
                        let on_rate = on_rate.clone();
                        move |_| on_rate(48_000)
                    } />
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
fn ProviderBtn(label: &'static str, target: VoiceProviderKind, active: VoiceProviderKind) -> impl IntoView {
    let is_active = move || target == active;
    view! {
        <button
            type="button"
            class="voice-pane__choice"
            class:voice-pane__choice--active=is_active
        >
            {label}
        </button>
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

// ---------------------------------------------------------------------------
// TTS section
// ---------------------------------------------------------------------------

#[component]
fn TtsSection<F, RV>(
    current: VoiceSettings,
    tts_provider: VoiceProviderKind,
    tts_model: String,
    voice_id: String,
    voices: RwSignal<Vec<VoiceEntry>>,
    gender_filter: RwSignal<GenderFilter>,
    tts_enabled: bool,
    save: F,
    reload_voices: RV,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
    RV: Fn(VoiceProviderKind) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let audio_ref = NodeRef::<leptos::html::Audio>::new();

    let on_provider = {
        let current = current.clone();
        move |p: VoiceProviderKind| {
            let mut next = current.clone();
            next.tts.provider = p;
            save(next);
            reload_voices(p);
        }
    };
    let on_model = {
        let current = current.clone();
        move |m: String| {
            let mut next = current.clone();
            next.tts.model_id = m;
            save(next);
        }
    };
    let on_voice = {
        let current = current.clone();
        move |id: String| {
            let mut next = current.clone();
            next.tts.voice = id;
            save(next);
        }
    };
    let on_enabled = {
        let current = current.clone();
        move |enabled: bool| {
            let mut next = current.clone();
            next.tts.enabled = enabled;
            save(next);
        }
    };

    let preview_voice = {
        let current = current.clone();
        move |voice: String| {
            let model = current.tts.model_id.clone();
            let provider = current.tts.provider;
            let text = i18n.tr(I18nKey::VoicePreviewText)().to_string();
            leptos::task::spawn_local(async move {
                if let Ok(resp) = voice_tts_preview(provider, model, voice, text).await {
                    play_b64(audio_ref, &resp.audio_b64, &resp.mime);
                }
            });
        }
    };

    view! {
        <section class="voice-pane__section">
            <h3>{move || i18n.tr(I18nKey::VoiceTtsSection)()}</h3>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceProviderField)()}</label>
                <div class="voice-pane__provider-row">
                    <ProviderBtn label="OpenAI" target=VoiceProviderKind::Openai active=tts_provider on:click={
                        let on_provider = on_provider.clone();
                        move |_| on_provider(VoiceProviderKind::Openai)
                    } />
                </div>
            </div>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceModelField)()}</label>
                <input
                    type="text"
                    class="voice-pane__input"
                    prop:value=tts_model.clone()
                    on:change={
                        let on_model = on_model.clone();
                        move |ev| {
                            if let Some(t) = ev.target() {
                                if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                    on_model(inp.value());
                                }
                            }
                        }
                    }
                />
            </div>

            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceVoiceField)()}</label>
                <div class="voice-pane__gender-row">
                    <GenderBtn target=GenderFilter::All label_key=I18nKey::VoiceGenderAll filter=gender_filter />
                    <GenderBtn target=GenderFilter::Male label_key=I18nKey::VoiceGenderMale filter=gender_filter />
                    <GenderBtn target=GenderFilter::Female label_key=I18nKey::VoiceGenderFemale filter=gender_filter />
                    <GenderBtn target=GenderFilter::Neutral label_key=I18nKey::VoiceGenderNeutral filter=gender_filter />
                </div>
                <div class="voice-pane__voice-grid">
                    {move || {
                        let active = voice_id.clone();
                        let filter = gender_filter.get();
                        let on_voice = on_voice.clone();
                        let preview_voice = preview_voice.clone();
                        voices.get()
                            .into_iter()
                            .filter(|v| filter.matches(v.gender))
                            .map(|v| {
                                let is_active = v.id == active;
                                let id_choose = v.id.clone();
                                let id_preview = v.id.clone();
                                let on_voice = on_voice.clone();
                                let preview_voice = preview_voice.clone();
                                view! {
                                    <div
                                        class="voice-pane__voice-card"
                                        class:voice-pane__voice-card--active=is_active
                                    >
                                        <button
                                            type="button"
                                            class="voice-pane__voice-pick"
                                            on:click=move |_| on_voice(id_choose.clone())
                                        >
                                            <strong>{v.label.clone()}</strong>
                                            <span class="voice-pane__voice-gender">
                                                {gender_label_for(v.gender)}
                                            </span>
                                        </button>
                                        <button
                                            type="button"
                                            class="voice-pane__voice-preview"
                                            on:click=move |_| preview_voice(id_preview.clone())
                                            aria-label="Sample"
                                        >
                                            <LxIcon icon=icondata::LuPlay width="0.85rem" height="0.85rem" />
                                        </button>
                                    </div>
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
                        on:change={
                            let on_enabled = on_enabled.clone();
                            move |ev| {
                                if let Some(t) = ev.target() {
                                    if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                        on_enabled(inp.checked());
                                    }
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

            <audio node_ref=audio_ref preload="none" />
        </section>
    }
}

#[component]
fn GenderBtn(target: GenderFilter, label_key: I18nKey, filter: RwSignal<GenderFilter>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let is_active = move || filter.get() == target;
    view! {
        <button
            type="button"
            class="voice-pane__choice voice-pane__choice--gender"
            class:voice-pane__choice--active=is_active
            on:click=move |_| filter.set(target)
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

fn play_b64(audio_ref: NodeRef<leptos::html::Audio>, b64: &str, mime: &str) {
    let Ok(bytes) = BASE64.decode(b64) else {
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
    if let Some(audio) = audio_ref.get_untracked() {
        let el: HtmlAudioElement = audio.unchecked_into();
        let old = el.src();
        if old.starts_with("blob:") {
            let _ = web_sys::Url::revoke_object_url(&old);
        }
        el.set_src(&url);
        let _ = el.play();
    }
}

// ---------------------------------------------------------------------------
// Behavior section
// ---------------------------------------------------------------------------

#[component]
fn BehaviorSection<F>(current: VoiceSettings, post_flow: PostSttFlow, save: F) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let on_flow = {
        let current = current.clone();
        move |flow: PostSttFlow| {
            let mut next = current.clone();
            next.post_stt_flow = flow;
            save(next);
        }
    };
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
                        on:click={
                            let on_flow = on_flow.clone();
                            move |_| on_flow(PostSttFlow::AutoSend)
                        }
                    >
                        {move || i18n.tr(I18nKey::VoicePostSttAutoSend)()}
                    </button>
                    <button
                        type="button"
                        class="voice-pane__choice"
                        class:voice-pane__choice--active=move || matches!(post_flow, PostSttFlow::Draft)
                        on:click={
                            let on_flow = on_flow.clone();
                            move |_| on_flow(PostSttFlow::Draft)
                        }
                    >
                        {move || i18n.tr(I18nKey::VoicePostSttDraft)()}
                    </button>
                </div>
            </div>
        </section>
    }
}

// ---------------------------------------------------------------------------
// Language section
// ---------------------------------------------------------------------------

#[component]
fn LanguageSection<F>(
    current: VoiceSettings,
    stt_lang: SttLanguageMode,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let on_mode = {
        let current = current.clone();
        move |mode: SttLanguageMode| {
            let mut next = current.clone();
            next.stt_language = mode;
            save(next);
        }
    };

    let is_follow = matches!(stt_lang, SttLanguageMode::FollowApp);
    let is_auto = matches!(stt_lang, SttLanguageMode::AutoDetect);
    let is_manual = matches!(stt_lang, SttLanguageMode::Manual { .. });
    let manual_code = if let SttLanguageMode::Manual { ref code } = stt_lang {
        code.clone()
    } else {
        String::new()
    };

    view! {
        <section class="voice-pane__section">
            <h3>{move || i18n.tr(I18nKey::VoiceLanguageSection)()}</h3>
            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoiceSttLangMode)()}</label>
                <div class="voice-pane__radio-row">
                    <button
                        type="button"
                        class="voice-pane__choice"
                        class:voice-pane__choice--active=move || is_follow
                        on:click={
                            let on_mode = on_mode.clone();
                            move |_| on_mode(SttLanguageMode::FollowApp)
                        }
                    >
                        {move || i18n.tr(I18nKey::VoiceSttLangFollowApp)()}
                    </button>
                    <button
                        type="button"
                        class="voice-pane__choice"
                        class:voice-pane__choice--active=move || is_auto
                        on:click={
                            let on_mode = on_mode.clone();
                            move |_| on_mode(SttLanguageMode::AutoDetect)
                        }
                    >
                        {move || i18n.tr(I18nKey::VoiceSttLangAutoDetect)()}
                    </button>
                    <button
                        type="button"
                        class="voice-pane__choice"
                        class:voice-pane__choice--active=move || is_manual
                        on:click={
                            let on_mode = on_mode.clone();
                            let manual_code = manual_code.clone();
                            move |_| on_mode(SttLanguageMode::Manual { code: manual_code.clone() })
                        }
                    >
                        {move || i18n.tr(I18nKey::VoiceSttLangManual)()}
                    </button>
                </div>
                <Show when=move || is_manual>
                    <input
                        type="text"
                        class="voice-pane__input"
                        placeholder="ISO-639-1 (e.g. de, en, ja)"
                        prop:value=manual_code.clone()
                        on:change={
                            let on_mode = on_mode.clone();
                            move |ev| {
                                if let Some(t) = ev.target() {
                                    if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                        on_mode(SttLanguageMode::Manual { code: inp.value() });
                                    }
                                }
                            }
                        }
                    />
                </Show>
            </div>
        </section>
    }
}

// ---------------------------------------------------------------------------
// PTT hotkey section
// ---------------------------------------------------------------------------

#[component]
fn PttSection<F>(
    current: VoiceSettings,
    ptt: PttHotkey,
    recording: RwSignal<bool>,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let on_enabled = {
        let current = current.clone();
        move |v: bool| {
            let mut next = current.clone();
            next.ptt_hotkey.enabled = v;
            save(next);
        }
    };
    let begin_capture = move || recording.set(true);
    let capture_keydown = {
        let current = current.clone();
        move |ev: web_sys::KeyboardEvent| {
            if !recording.get_untracked() {
                return;
            }
            ev.prevent_default();
            if ev.key() == "Escape" {
                recording.set(false);
                return;
            }
            if matches!(ev.code().as_str(), "ControlLeft" | "ControlRight" | "ShiftLeft" | "ShiftRight" | "AltLeft" | "AltRight" | "MetaLeft" | "MetaRight") {
                return;
            }
            let mut next = current.clone();
            next.ptt_hotkey = PttHotkey {
                enabled: next.ptt_hotkey.enabled,
                code: ev.code(),
                ctrl: ev.ctrl_key(),
                shift: ev.shift_key(),
                alt: ev.alt_key(),
                meta: ev.meta_key(),
            };
            save(next);
            recording.set(false);
        }
    };

    let display = format_hotkey(&ptt);
    let enabled = ptt.enabled;

    view! {
        <section class="voice-pane__section">
            <h3>{move || i18n.tr(I18nKey::VoicePttSection)()}</h3>
            <div class="voice-pane__field">
                <label class="voice-pane__toggle">
                    <input
                        type="checkbox"
                        prop:checked=enabled
                        on:change={
                            let on_enabled = on_enabled.clone();
                            move |ev| {
                                if let Some(t) = ev.target() {
                                    if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                        on_enabled(inp.checked());
                                    }
                                }
                            }
                        }
                    />
                    <span>{move || i18n.tr(I18nKey::VoicePttEnabled)()}</span>
                </label>
            </div>
            <div class="voice-pane__field">
                <label>{move || i18n.tr(I18nKey::VoicePttHotkey)()}</label>
                <button
                    type="button"
                    class="voice-pane__hotkey-capture"
                    class:voice-pane__hotkey-capture--recording=move || recording.get()
                    on:click=move |_| begin_capture()
                    on:keydown=capture_keydown
                    tabindex="0"
                >
                    {move || if recording.get() {
                        i18n.tr(I18nKey::VoicePttRecorderHint)().to_string()
                    } else {
                        display.clone()
                    }}
                </button>
            </div>
        </section>
    }
}

fn format_hotkey(spec: &PttHotkey) -> String {
    let mut parts: Vec<&'static str> = Vec::new();
    if spec.ctrl {
        parts.push("Ctrl");
    }
    if spec.shift {
        parts.push("Shift");
    }
    if spec.alt {
        parts.push("Alt");
    }
    if spec.meta {
        parts.push("Meta");
    }
    let mut out = parts.join("+");
    if !out.is_empty() {
        out.push('+');
    }
    let key = spec.code.strip_prefix("Key").unwrap_or(&spec.code);
    out.push_str(key);
    out
}

// Convince the compiler we still need these types (referenced via trait bounds only).
#[allow(dead_code)]
fn _ensure_types(_s: SttSettings, _t: TtsSettings) {}
