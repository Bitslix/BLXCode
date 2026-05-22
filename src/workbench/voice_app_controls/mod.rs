//! Voice STT language + push-to-talk controls styled for Settings → App.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{PttHotkey, SttLanguageMode, VoiceSettings};
use leptos::prelude::*;
use wasm_bindgen::JsCast;

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

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.checked())
}

/// STT input language (nested under App → Language).
#[component]
pub fn VoiceSttLanguageControls<F>(settings: RwSignal<Option<VoiceSettings>>, save: F) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();

    view! {
        <Show
            when=move || settings.get().is_some()
            fallback=|| ()
        >
            {move || {
                let Some(current) = settings.get() else {
                    return ().into_any();
                };
                let stt_lang = current.stt_language.clone();
                let is_follow = matches!(stt_lang, SttLanguageMode::FollowApp);
                let is_auto = matches!(stt_lang, SttLanguageMode::AutoDetect);
                let is_manual = matches!(stt_lang, SttLanguageMode::Manual { .. });
                let manual_code = if let SttLanguageMode::Manual { ref code } = stt_lang {
                    code.clone()
                } else {
                    String::new()
                };

                view! {
                    <label class="harness-stack app-voice-stt-lang">
                        <span class="harness-field-label">
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::VoiceSttLangMode)()}</span>
                        </span>
                        <div class="app-prefs-toggle-grid app-prefs-toggle-grid--triple">
                            <div class="app-prefs-toggle-cell">
                                <label class="app-prefs-radio">
                                    <input
                                        type="radio"
                                        name="stt-lang-mode"
                                        prop:checked=move || is_follow
                                        on:change=move |_| {
                                            let Some(mut next) = settings.get_untracked() else { return };
                                            next.stt_language = SttLanguageMode::FollowApp;
                                            save(next);
                                        }
                                    />
                                    <span>{move || i18n.tr(I18nKey::VoiceSttLangFollowApp)()}</span>
                                </label>
                            </div>
                            <div class="app-prefs-toggle-cell">
                                <label class="app-prefs-radio">
                                    <input
                                        type="radio"
                                        name="stt-lang-mode"
                                        prop:checked=move || is_auto
                                        on:change=move |_| {
                                            let Some(mut next) = settings.get_untracked() else { return };
                                            next.stt_language = SttLanguageMode::AutoDetect;
                                            save(next);
                                        }
                                    />
                                    <span>{move || i18n.tr(I18nKey::VoiceSttLangAutoDetect)()}</span>
                                </label>
                            </div>
                            <div class="app-prefs-toggle-cell">
                                <label class="app-prefs-radio">
                                    <input
                                        type="radio"
                                        name="stt-lang-mode"
                                        prop:checked=move || is_manual
                                        on:change={
                                            let manual_code = manual_code.clone();
                                            move |_| {
                                                let Some(mut next) = settings.get_untracked() else { return };
                                                next.stt_language =
                                                    SttLanguageMode::Manual { code: manual_code.clone() };
                                                save(next);
                                            }
                                        }
                                    />
                                    <span>{move || i18n.tr(I18nKey::VoiceSttLangManual)()}</span>
                                </label>
                            </div>
                        </div>
                        <Show when=move || is_manual>
                            <input
                                type="text"
                                class="workbench-plain-input"
                                placeholder="ISO-639-1 (e.g. de, en, ja)"
                                prop:value=manual_code.clone()
                                on:change=move |ev| {
                                    if let Some(t) = ev.target() {
                                        if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                            let Some(mut next) = settings.get_untracked() else { return };
                                            next.stt_language =
                                                SttLanguageMode::Manual { code: inp.value() };
                                            save(next);
                                        }
                                    }
                                }
                            />
                        </Show>
                    </label>
                }
                .into_any()
            }}
        </Show>
    }
}

/// Push-to-talk hotkey (nested under App → Keyboard shortcuts).
#[component]
pub fn VoicePttControls<F>(
    settings: RwSignal<Option<VoiceSettings>>,
    recording: RwSignal<bool>,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();

    view! {
        <Show
            when=move || settings.get().is_some()
            fallback=|| ()
        >
            {move || {
                let Some(current) = settings.get() else {
                    return ().into_any();
                };
                let ptt = current.ptt_hotkey.clone();
                let begin_capture = move || recording.set(true);
                let capture_keydown = move |ev: web_sys::KeyboardEvent| {
                    if !recording.get_untracked() {
                        return;
                    }
                    ev.prevent_default();
                    if ev.key() == "Escape" {
                        recording.set(false);
                        return;
                    }
                    if matches!(
                        ev.code().as_str(),
                        "ControlLeft"
                            | "ControlRight"
                            | "ShiftLeft"
                            | "ShiftRight"
                            | "AltLeft"
                            | "AltRight"
                            | "MetaLeft"
                            | "MetaRight"
                    ) {
                        return;
                    }
                    let Some(mut next) = settings.get_untracked() else {
                        return;
                    };
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
                };
                let display = format_hotkey(&ptt);
                let enabled = ptt.enabled;

                view! {
                    <div class="app-voice-ptt">
                        <label class="app-prefs-toggle">
                            <input
                                type="checkbox"
                                prop:checked=enabled
                                on:change=move |ev| {
                                    if let Some(checked) = checkbox_checked(&ev) {
                                        let Some(mut next) = settings.get_untracked() else { return };
                                        next.ptt_hotkey.enabled = checked;
                                        save(next);
                                    }
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::VoicePttEnabled)()}</span>
                        </label>
                        <label class="harness-stack">
                            <span class="harness-field-label">
                                <span class="harness-field-label__text">{move || i18n.tr(I18nKey::VoicePttHotkey)()}</span>
                            </span>
                            <button
                                type="button"
                                class="workbench-plain-input app-prefs-hotkey-capture"
                                class:app-prefs-hotkey-capture--recording=move || recording.get()
                                on:click=move |_| begin_capture()
                                on:keydown=capture_keydown
                            >
                                {move || if recording.get() {
                                    i18n.tr(I18nKey::VoicePttRecorderHint)().to_string()
                                } else {
                                    display.clone()
                                }}
                            </button>
                        </label>
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}
