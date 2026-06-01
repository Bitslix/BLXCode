//! Push-to-talk settings section for the Voice pane.
//!
//! Renders enable/mode/quality/target/behaviour controls plus the whisper
//! model manager (local mode). The hotkey itself is configured in
//! Settings → Shortcuts, so this section only links there. All strings come
//! from i18n; all colours from theme tokens (`ptt_section.css`).

use leptos::prelude::*;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    PttInsertTarget, PttMode, PttTargetMode, TtsCollision, VoiceProviderKind, VoiceSettings,
    WhisperQuality,
};
use crate::workbench::ptt_runtime::refresh_ptt_settings_cache;

use super::model_manager::ModelManager;

/// `save` mirrors the pattern used by the other voice sections: a `Copy`
/// closure that persists the full `VoiceSettings`.
#[component]
pub fn PushToTalkSection<F>(settings: RwSignal<Option<VoiceSettings>>, save: F) -> impl IntoView
where
    F: Fn(VoiceSettings) + Copy + Send + Sync + 'static,
{
    let i18n = expect_context::<I18nService>();

    // Persist a mutation of the `ptt` sub-object, then refresh the runtime cache.
    let patch = move |mutate: &dyn Fn(&mut crate::tauri_bridge::PttSettings)| {
        if let Some(mut cur) = settings.get_untracked() {
            mutate(&mut cur.ptt);
            save(cur);
            refresh_ptt_settings_cache();
        }
    };

    // Reactive accessors into the current ptt settings.
    let ptt = move || settings.get().map(|s| s.ptt);
    let enabled = move || ptt().map(|p| p.enabled).unwrap_or(false);
    let mode = move || ptt().map(|p| p.mode).unwrap_or(PttMode::Local);

    view! {
        <section class="ptt-section">
            <h5 class="ptt-section__head">
                {move || i18n.tr(I18nKey::VoicePttSection)()}
            </h5>

            // Enable
            <label class="ptt-row ptt-row--switch">
                <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttEnabled)()}</span>
                <input
                    type="checkbox"
                    class="ptt-switch"
                    prop:checked=enabled
                    on:change=move |ev| {
                        let v = event_target_checked(&ev);
                        patch(&move |p| p.enabled = v);
                    }
                />
            </label>

            // Hotkey hint → Settings → Shortcuts
            <p class="ptt-hint">{move || i18n.tr(I18nKey::VoicePttHotkeyHint)()}</p>

            <Show when=move || enabled()>
                <div class="ptt-body">
                    // Mode
                    <div class="ptt-row">
                        <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttMode)()}</span>
                        <div class="ptt-seg" role="group">
                            <button
                                type="button"
                                class="ptt-seg__btn"
                                class:ptt-seg__btn--on=move || mode() == PttMode::Local
                                on:click=move |_| patch(&|p| p.mode = PttMode::Local)
                            >
                                {move || i18n.tr(I18nKey::VoicePttModeLocal)()}
                            </button>
                            <button
                                type="button"
                                class="ptt-seg__btn"
                                class:ptt-seg__btn--on=move || mode() == PttMode::Cloud
                                on:click=move |_| patch(&|p| p.mode = PttMode::Cloud)
                            >
                                {move || i18n.tr(I18nKey::VoicePttModeCloud)()}
                            </button>
                        </div>
                    </div>

                    // Local: quality + model manager
                    <Show when=move || mode() == PttMode::Local>
                        <div class="ptt-row">
                            <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttQuality)()}</span>
                            <QualitySeg ptt_quality=Signal::derive(move || {
                                ptt().map(|p| p.local_quality).unwrap_or(WhisperQuality::Balanced)
                            }) on_pick=move |q| patch(&move |p| p.local_quality = q) />
                        </div>
                        <ModelManager
                            selected=Signal::derive(move || {
                                ptt().and_then(|p| p.local_model_path)
                            })
                            on_use=move |id: String| {
                                patch(&move |p| p.local_model_path = Some(id.clone()));
                            }
                        />
                    </Show>

                    // Cloud: provider + model
                    <Show when=move || mode() == PttMode::Cloud>
                        <div class="ptt-row">
                            <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttCloudProvider)()}</span>
                            <select
                                class="ptt-select"
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    let prov = if v == "openrouter" {
                                        VoiceProviderKind::Openrouter
                                    } else {
                                        VoiceProviderKind::Openai
                                    };
                                    patch(&move |p| p.cloud_provider = prov);
                                }
                            >
                                // AWS is intentionally absent: Polly is TTS-only.
                                <option
                                    value="openai"
                                    selected=move || {
                                        ptt().map(|p| p.cloud_provider == VoiceProviderKind::Openai).unwrap_or(true)
                                    }
                                >"OpenAI"</option>
                                <option
                                    value="openrouter"
                                    selected=move || {
                                        ptt().map(|p| p.cloud_provider == VoiceProviderKind::Openrouter).unwrap_or(false)
                                    }
                                >"OpenRouter"</option>
                            </select>
                        </div>
                        <label class="ptt-row">
                            <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttCloudModel)()}</span>
                            <input
                                type="text"
                                class="ptt-input"
                                prop:value=move || ptt().map(|p| p.cloud_model_id).unwrap_or_default()
                                on:change=move |ev| {
                                    let v = event_target_value(&ev);
                                    patch(&move |p| p.cloud_model_id = v.clone());
                                }
                            />
                        </label>
                    </Show>

                    // Insert target
                    <div class="ptt-row">
                        <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttInsertTarget)()}</span>
                        <select
                            class="ptt-select"
                            on:change=move |ev| {
                                let v = event_target_value(&ev);
                                let t = match v.as_str() {
                                    "terminal" => PttInsertTarget::Terminal,
                                    "activeInput" => PttInsertTarget::ActiveInput,
                                    "clipboard" => PttInsertTarget::Clipboard,
                                    _ => PttInsertTarget::Agent,
                                };
                                patch(&move |p| p.insert_target = t);
                            }
                        >
                            <TargetOption value="agent" key=I18nKey::VoicePttTargetAgent current=Signal::derive(move || ptt().map(|p| p.insert_target)) want=PttInsertTarget::Agent />
                            <TargetOption value="terminal" key=I18nKey::VoicePttTargetTerminal current=Signal::derive(move || ptt().map(|p| p.insert_target)) want=PttInsertTarget::Terminal />
                            <TargetOption value="activeInput" key=I18nKey::VoicePttTargetActiveInput current=Signal::derive(move || ptt().map(|p| p.insert_target)) want=PttInsertTarget::ActiveInput />
                            <TargetOption value="clipboard" key=I18nKey::VoicePttTargetClipboard current=Signal::derive(move || ptt().map(|p| p.insert_target)) want=PttInsertTarget::Clipboard />
                        </select>
                    </div>

                    // Target mode
                    <div class="ptt-row">
                        <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttTargetMode)()}</span>
                        <div class="ptt-seg" role="group">
                            <button
                                type="button"
                                class="ptt-seg__btn"
                                class:ptt-seg__btn--on=move || ptt().map(|p| p.target_mode == PttTargetMode::CurrentFocus).unwrap_or(true)
                                on:click=move |_| patch(&|p| p.target_mode = PttTargetMode::CurrentFocus)
                            >
                                {move || i18n.tr(I18nKey::VoicePttTargetModeCurrentFocus)()}
                            </button>
                            <button
                                type="button"
                                class="ptt-seg__btn"
                                class:ptt-seg__btn--on=move || ptt().map(|p| p.target_mode == PttTargetMode::RememberStart).unwrap_or(false)
                                on:click=move |_| patch(&|p| p.target_mode = PttTargetMode::RememberStart)
                            >
                                {move || i18n.tr(I18nKey::VoicePttTargetModeRememberStart)()}
                            </button>
                        </div>
                    </div>

                    // Auto-submit
                    <label class="ptt-row ptt-row--switch">
                        <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttAutoSubmit)()}</span>
                        <input
                            type="checkbox"
                            class="ptt-switch"
                            prop:checked=move || ptt().map(|p| p.auto_submit).unwrap_or(false)
                            on:change=move |ev| {
                                let v = event_target_checked(&ev);
                                patch(&move |p| p.auto_submit = v);
                            }
                        />
                    </label>

                    // Partial transcript
                    <label class="ptt-row ptt-row--switch">
                        <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttPartialTranscript)()}</span>
                        <input
                            type="checkbox"
                            class="ptt-switch"
                            prop:checked=move || ptt().map(|p| p.partial_transcript).unwrap_or(true)
                            on:change=move |ev| {
                                let v = event_target_checked(&ev);
                                patch(&move |p| p.partial_transcript = v);
                            }
                        />
                    </label>

                    // TTS collision
                    <div class="ptt-row">
                        <span class="ptt-row__label">{move || i18n.tr(I18nKey::VoicePttTtsCollision)()}</span>
                        <div class="ptt-seg" role="group">
                            <CollisionBtn current=Signal::derive(move || ptt().map(|p| p.tts_collision)) want=TtsCollision::Stop key=I18nKey::VoicePttTtsStop on_pick=move || patch(&|p| p.tts_collision = TtsCollision::Stop) />
                            <CollisionBtn current=Signal::derive(move || ptt().map(|p| p.tts_collision)) want=TtsCollision::Pause key=I18nKey::VoicePttTtsPause on_pick=move || patch(&|p| p.tts_collision = TtsCollision::Pause) />
                            <CollisionBtn current=Signal::derive(move || ptt().map(|p| p.tts_collision)) want=TtsCollision::Block key=I18nKey::VoicePttTtsBlock on_pick=move || patch(&|p| p.tts_collision = TtsCollision::Block) />
                        </div>
                    </div>
                </div>
            </Show>
        </section>
    }
}

#[component]
fn QualitySeg<P>(ptt_quality: Signal<WhisperQuality>, on_pick: P) -> impl IntoView
where
    P: Fn(WhisperQuality) + Copy + 'static,
{
    let i18n = expect_context::<I18nService>();
    let btn = move |want: WhisperQuality, key: I18nKey| {
        view! {
            <button
                type="button"
                class="ptt-seg__btn"
                class:ptt-seg__btn--on=move || ptt_quality.get() == want
                on:click=move |_| on_pick(want)
            >
                {move || i18n.tr(key)()}
            </button>
        }
    };
    view! {
        <div class="ptt-seg" role="group">
            {btn(WhisperQuality::Fast, I18nKey::VoicePttQualityFast)}
            {btn(WhisperQuality::Balanced, I18nKey::VoicePttQualityBalanced)}
            {btn(WhisperQuality::Best, I18nKey::VoicePttQualityBest)}
        </div>
    }
}

#[component]
fn TargetOption(
    value: &'static str,
    key: I18nKey,
    current: Signal<Option<PttInsertTarget>>,
    want: PttInsertTarget,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <option value=value selected=move || current.get() == Some(want)>
            {move || i18n.tr(key)()}
        </option>
    }
}

#[component]
fn CollisionBtn<P>(
    current: Signal<Option<TtsCollision>>,
    want: TtsCollision,
    key: I18nKey,
    on_pick: P,
) -> impl IntoView
where
    P: Fn() + Copy + 'static,
{
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="ptt-seg__btn"
            class:ptt-seg__btn--on=move || current.get() == Some(want)
            on:click=move |_| on_pick()
        >
            {move || i18n.tr(key)()}
        </button>
    }
}
