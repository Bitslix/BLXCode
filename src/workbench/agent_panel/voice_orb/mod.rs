//! Voice orb — hybrid (click toggle / hold PTT) microphone button with
//! audio-playback support for TTS replies streamed via `AgentEvent::VoiceReady`.

mod state;

use crate::agent_wire::AgentEvent;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_settings_get, api_keys_status, is_tauri_shell, voice_cancel_recording,
    voice_settings_get, voice_start_recording, voice_stop_and_transcribe, voice_tts_preview,
    AgentProviderKind, AgentProviderSettingsView, ApiKeysStatus, PostSttFlow, SttLanguageMode,
    VoiceProviderKind, VoiceSettings,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use js_sys::Uint8Array;
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{Blob, BlobPropertyBag, HtmlAudioElement, KeyboardEvent, MouseEvent};

pub use state::{focus_in_editable, hotkey_matches, VoiceOrbState};

/// Public handle the agent panel uses to read the orb's state and drive
/// playback when `AgentEvent::VoiceReady` arrives.
#[derive(Clone, Copy)]
pub struct VoiceOrbHandle {
    pub state: RwSignal<VoiceOrbState>,
    pub voice_pending: RwSignal<bool>,
    pub settings: RwSignal<Option<VoiceSettings>>,
    /// True only when TTS is enabled *and* the configured TTS provider has an
    /// API key set in settings — gates the per-line "Play" button.
    pub tts_ready: RwSignal<bool>,
    pub audio_ref: NodeRef<html::Audio>,
}

impl VoiceOrbHandle {
    pub fn new() -> Self {
        Self {
            state: RwSignal::new(VoiceOrbState::Idle),
            voice_pending: RwSignal::new(false),
            settings: RwSignal::new(None),
            tts_ready: RwSignal::new(false),
            audio_ref: NodeRef::<html::Audio>::new(),
        }
    }
}

/// True when the TTS provider selected in `settings` has a configured API key.
fn tts_provider_key_configured(
    settings: &VoiceSettings,
    agent_settings: Option<&AgentProviderSettingsView>,
    api_keys: Option<&ApiKeysStatus>,
) -> bool {
    if !settings.tts.enabled {
        return false;
    }
    match settings.tts.provider {
        VoiceProviderKind::Aws => api_keys.is_some_and(|s| {
            s.entries
                .iter()
                .any(|e| e.kind == "aws_polly" && e.configured)
        }),
        VoiceProviderKind::Openai => {
            agent_provider_key_configured(agent_settings, AgentProviderKind::Openai)
        }
        VoiceProviderKind::Openrouter => {
            agent_provider_key_configured(agent_settings, AgentProviderKind::Openrouter)
        }
    }
}

fn agent_provider_key_configured(
    view: Option<&AgentProviderSettingsView>,
    provider: AgentProviderKind,
) -> bool {
    view.is_some_and(|v| {
        v.key_statuses
            .iter()
            .any(|s| s.provider == provider && s.configured)
    })
}

/// Fetch voice settings + API-key status and update `handle.tts_ready` so the
/// per-line Play button only appears once voice chat is actually configured.
pub async fn refresh_tts_ready(handle: VoiceOrbHandle) {
    if !is_tauri_shell() {
        return;
    }
    let settings = match handle.settings.get_untracked() {
        Some(s) => s,
        None => match voice_settings_get().await {
            Ok(s) => {
                handle.settings.set(Some(s.clone()));
                s
            }
            Err(_) => {
                handle.tts_ready.set(false);
                return;
            }
        },
    };
    let agent_settings = agent_settings_get().await.ok();
    let api_keys = api_keys_status().await.ok();
    handle.tts_ready.set(tts_provider_key_configured(
        &settings,
        agent_settings.as_ref(),
        api_keys.as_ref(),
    ));
}

#[component]
pub fn VoiceOrb<F>(handle: VoiceOrbHandle, on_transcript: F) -> impl IntoView
where
    F: Fn(String, bool) + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let active_turn_id = RwSignal::new(Option::<String>::None);
    let mousedown_at = RwSignal::new(0.0_f64);

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(v) = voice_settings_get().await {
                handle.settings.set(Some(v));
            }
            refresh_tts_ready(handle).await;
        });
    }

    let start_recording = move || {
        if !is_tauri_shell() {
            return;
        }
        let rate = handle
            .settings
            .get_untracked()
            .map(|v| v.stt.sample_rate_hz)
            .unwrap_or(16_000);
        leptos::task::spawn_local(async move {
            match voice_start_recording(rate).await {
                Ok(r) => active_turn_id.set(Some(r.turn_id)),
                Err(_) => handle.state.set(VoiceOrbState::Idle),
            }
        });
    };

    let stop_recording = move || {
        let Some(turn_id) = active_turn_id.get_untracked() else {
            handle.state.set(VoiceOrbState::Idle);
            return;
        };
        handle.state.set(VoiceOrbState::Transcribing);
        let locale_hint = build_locale_hint(handle.settings.get_untracked().as_ref(), &i18n);
        leptos::task::spawn_local(async move {
            let result = voice_stop_and_transcribe(turn_id, locale_hint).await;
            active_turn_id.set(None);
            handle.state.set(VoiceOrbState::Idle);
            if let Ok(resp) = result {
                let auto_send = handle
                    .settings
                    .get_untracked()
                    .map(|s| matches!(s.post_stt_flow, PostSttFlow::AutoSend))
                    .unwrap_or(true);
                if !resp.text.trim().is_empty() {
                    handle.voice_pending.set(true);
                    on_transcript(resp.text, auto_send);
                }
            }
        });
    };

    let cancel_recording = move || {
        let Some(turn_id) = active_turn_id.get_untracked() else {
            handle.state.set(VoiceOrbState::Idle);
            return;
        };
        active_turn_id.set(None);
        handle.state.set(VoiceOrbState::Idle);
        leptos::task::spawn_local(async move {
            let _ = voice_cancel_recording(turn_id).await;
        });
    };

    let on_mousedown = move |ev: MouseEvent| {
        ev.prevent_default();
        mousedown_at.set(performance_now());
        match handle.state.get_untracked() {
            VoiceOrbState::Idle => {
                handle.state.set(VoiceOrbState::RecordingHold);
                start_recording();
            }
            _ => {}
        }
    };

    let on_mouseup = move |_: MouseEvent| {
        let held = performance_now() - mousedown_at.get_untracked();
        match handle.state.get_untracked() {
            VoiceOrbState::RecordingHold => {
                if held < state::HOLD_THRESHOLD_MS {
                    handle.state.set(VoiceOrbState::RecordingToggle);
                } else {
                    stop_recording();
                }
            }
            VoiceOrbState::RecordingToggle => stop_recording(),
            _ => {}
        }
    };

    let on_mouseleave = move |_: MouseEvent| {
        if matches!(handle.state.get_untracked(), VoiceOrbState::RecordingHold) {
            let held = performance_now() - mousedown_at.get_untracked();
            if held >= state::HOLD_THRESHOLD_MS {
                stop_recording();
            }
        }
    };

    let on_keydown = move |ev: KeyboardEvent| {
        if ev.repeat() {
            return;
        }
        let key = ev.key();
        if key == "Escape" && handle.state.get_untracked().is_recording() {
            ev.prevent_default();
            cancel_recording();
            return;
        }
        if (key == " " || key == "Enter")
            && matches!(handle.state.get_untracked(), VoiceOrbState::Idle)
        {
            ev.prevent_default();
            handle.state.set(VoiceOrbState::RecordingToggle);
            start_recording();
        } else if (key == " " || key == "Enter") && handle.state.get_untracked().is_recording() {
            ev.prevent_default();
            stop_recording();
        }
    };

    let aria_label = move || match handle.state.get() {
        VoiceOrbState::Idle => i18n.tr(I18nKey::VoiceOrbAriaIdle)().to_string(),
        VoiceOrbState::RecordingHold | VoiceOrbState::RecordingToggle => {
            i18n.tr(I18nKey::VoiceOrbAriaRecording)().to_string()
        }
        VoiceOrbState::Transcribing => i18n.tr(I18nKey::VoiceOrbAriaTranscribing)().to_string(),
    };

    view! {
        <>
            <button
                type="button"
                class="agent-hero__orb voice-orb"
                class:agent-hero__orb--active=move || handle.state.get().is_recording()
                class:voice-orb--transcribing=move || matches!(handle.state.get(), VoiceOrbState::Transcribing)
                aria-pressed=move || handle.state.get().is_recording().to_string()
                aria-label=aria_label
                on:mousedown=on_mousedown
                on:mouseup=on_mouseup
                on:mouseleave=on_mouseleave
                on:keydown=on_keydown
            >
                <Show
                    when=move || matches!(handle.state.get(), VoiceOrbState::Transcribing)
                    fallback=move || view! {
                        <Show
                            when=move || handle.state.get().is_recording()
                            fallback=move || view! { <span class="agent-hero__logo">"B"</span> }.into_any()
                        >
                            <LxIcon icon=icondata::LuMic width="1.5rem" height="1.5rem" />
                        </Show>
                    }.into_any()
                >
                    <LxIcon icon=icondata::LuLoader width="1.4rem" height="1.4rem" />
                </Show>
            </button>
            <audio node_ref=handle.audio_ref class="voice-orb__audio" preload="none" />
        </>
    }
}

pub fn play_audio_b64(audio_ref: NodeRef<html::Audio>, b64: &str, mime: &str) {
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
        let old_src = el.src();
        if !old_src.is_empty() && old_src.starts_with("blob:") {
            let _ = web_sys::Url::revoke_object_url(&old_src);
        }
        el.set_src(&url);
        let _ = el.play();
    }
}

pub fn handle_voice_event(audio_ref: NodeRef<html::Audio>, ev: &AgentEvent) {
    if let AgentEvent::VoiceReady { audio_b64, mime } = ev {
        play_audio_b64(audio_ref, audio_b64, mime);
    }
}

/// True when App voice settings have a non-empty TTS model (provider is always set).
#[must_use]
pub fn tts_line_playback_available(settings: Option<&VoiceSettings>) -> bool {
    settings.is_some_and(|s| !s.tts.model_id.trim().is_empty())
}

/// Synthesize and play one chat line via configured TTS (same API as voice settings preview).
pub fn play_line_tts(audio_ref: NodeRef<html::Audio>, settings: VoiceSettings, text: String) {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return;
    }
    let provider = settings.tts.provider;
    let model_id = settings.tts.model_id.clone();
    let voice = settings.tts.voice.clone();
    leptos::task::spawn_local(async move {
        if let Ok(resp) = voice_tts_preview(provider, model_id, voice, text).await {
            play_audio_b64(audio_ref, &resp.audio_b64, &resp.mime);
        }
    });
}

fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

fn build_locale_hint(settings: Option<&VoiceSettings>, i18n: &I18nService) -> Option<String> {
    let mode = settings.map(|s| s.stt_language.clone());
    match mode {
        Some(SttLanguageMode::FollowApp) | None => {
            Some(i18n.locale().get_untracked().iso639_1().to_string())
        }
        Some(SttLanguageMode::AutoDetect) => None,
        Some(SttLanguageMode::Manual { code }) => {
            if code.trim().is_empty() {
                None
            } else {
                Some(code)
            }
        }
    }
}

/// Window-level keyboard listener for the configured PTT hotkey. Registers
/// listeners on `window` and tears them down via `on_cleanup` when the
/// caller's reactive scope is dropped.
///
/// Legacy: superseded by `workbench::ptt_runtime`, which reads the key from
/// Settings → Shortcuts and routes to all targets. Retained for reference.
#[allow(dead_code)]
pub fn install_ptt_hotkey(
    handle: VoiceOrbHandle,
    i18n: I18nService,
    on_transcript: impl Fn(String, bool) + 'static + Copy,
) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let active_turn_id = std::rc::Rc::new(std::cell::RefCell::new(Option::<String>::None));
    let active_down = active_turn_id.clone();
    let active_up = active_turn_id.clone();

    let down: Closure<dyn FnMut(KeyboardEvent)> = Closure::new(move |ev: KeyboardEvent| {
        let Some(settings) = handle.settings.get_untracked() else {
            return;
        };
        let spec = settings.ptt_hotkey.clone();
        if ev.repeat() || !hotkey_matches(&spec, &ev) {
            return;
        }
        let has_modifier = spec.ctrl || spec.shift || spec.alt || spec.meta;
        if !has_modifier && focus_in_editable() {
            return;
        }
        if handle.state.get_untracked().is_recording() {
            return;
        }
        ev.prevent_default();
        handle.state.set(VoiceOrbState::RecordingHold);
        let rate = settings.stt.sample_rate_hz;
        let active = active_down.clone();
        leptos::task::spawn_local(async move {
            match voice_start_recording(rate).await {
                Ok(r) => *active.borrow_mut() = Some(r.turn_id),
                Err(_) => handle.state.set(VoiceOrbState::Idle),
            }
        });
    });

    let up: Closure<dyn FnMut(KeyboardEvent)> = Closure::new(move |ev: KeyboardEvent| {
        let Some(settings) = handle.settings.get_untracked() else {
            return;
        };
        let spec = settings.ptt_hotkey.clone();
        if !hotkey_matches(&spec, &ev) {
            return;
        }
        if !matches!(handle.state.get_untracked(), VoiceOrbState::RecordingHold) {
            return;
        }
        let id = active_up.borrow_mut().take();
        let Some(turn_id) = id else {
            handle.state.set(VoiceOrbState::Idle);
            return;
        };
        handle.state.set(VoiceOrbState::Transcribing);
        let locale_hint = build_locale_hint(Some(&settings), &i18n);
        leptos::task::spawn_local(async move {
            let result = voice_stop_and_transcribe(turn_id, locale_hint).await;
            handle.state.set(VoiceOrbState::Idle);
            if let Ok(resp) = result {
                let auto_send = handle
                    .settings
                    .get_untracked()
                    .map(|s| matches!(s.post_stt_flow, PostSttFlow::AutoSend))
                    .unwrap_or(true);
                if !resp.text.trim().is_empty() {
                    handle.voice_pending.set(true);
                    on_transcript(resp.text, auto_send);
                }
            }
        });
    });

    let _ = window.add_event_listener_with_callback("keydown", down.as_ref().unchecked_ref());
    let _ = window.add_event_listener_with_callback("keyup", up.as_ref().unchecked_ref());

    // Park the closures behind `send_wrapper` so they can live in a
    // `Send + Sync` cleanup callback even though `Closure` itself is `!Send`.
    let down = send_wrapper::SendWrapper::new(down);
    let up = send_wrapper::SendWrapper::new(up);
    let window_clone = send_wrapper::SendWrapper::new(window);
    leptos::prelude::on_cleanup(move || {
        let d = down.take();
        let u = up.take();
        let w = window_clone.take();
        let _ = w.remove_event_listener_with_callback("keydown", d.as_ref().unchecked_ref());
        let _ = w.remove_event_listener_with_callback("keyup", u.as_ref().unchecked_ref());
    });
}
