//! Shared Voice settings controls reused by categorized Settings panes.

use crate::i18n::I18nKey;
use crate::i18n::{Locale, APP_LOCALES};
use crate::service::I18nService;
use crate::tauri_bridge::{SttLanguageMode, VoiceSettings};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

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

fn focus_stt_locale_option(loc: Locale) {
    focus_by_id(&format!("stt-locale-option-{}", loc.as_str()));
}

fn next_stt_locale(loc: Locale) -> Locale {
    let n = APP_LOCALES.len();
    let idx = APP_LOCALES.iter().position(|(l, _)| *l == loc).unwrap_or(0);
    APP_LOCALES[(idx + 1) % n].0
}

fn prev_stt_locale(loc: Locale) -> Locale {
    let n = APP_LOCALES.len();
    let idx = APP_LOCALES.iter().position(|(l, _)| *l == loc).unwrap_or(0);
    APP_LOCALES[(idx + n - 1) % n].0
}

fn app_locale_native_label(loc: Locale) -> &'static str {
    APP_LOCALES
        .iter()
        .find(|(l, _)| *l == loc)
        .map(|(_, label)| *label)
        .unwrap_or("?")
}

fn default_manual_iso639_1(i18n: &I18nService) -> String {
    i18n.locale().get_untracked().iso639_1().to_string()
}

/// Built-in locale dropdown for manual STT language (same chrome as UI language picker).
#[component]
fn SttManualLocalePicker<F>(
    settings: RwSignal<Option<VoiceSettings>>,
    selected: Locale,
    save: F,
) -> impl IntoView
where
    F: Fn(VoiceSettings) + Send + Sync + 'static + Copy,
{
    let open = RwSignal::new(false);

    let choose = move |loc: Locale| {
        let Some(mut next) = settings.get_untracked() else {
            return;
        };
        next.stt_language = SttLanguageMode::Manual {
            code: loc.iso639_1().to_string(),
        };
        save(next);
        open.set(false);
    };

    view! {
        <div class="harness-provider-picker harness-locale-picker">
            <button
                type="button"
                class="harness-provider-trigger"
                aria-haspopup="listbox"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |_| {
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(0).await;
                            focus_stt_locale_option(selected);
                        });
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " => {
                            ev.prevent_default();
                            open.set(true);
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_stt_locale_option(selected);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let loc = prev_stt_locale(selected);
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_stt_locale_option(loc);
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
                            src=selected.flag_icon_url()
                            alt=""
                        />
                    </span>
                    <span>{app_locale_native_label(selected)}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        let active = selected;
                        APP_LOCALES
                            .iter()
                            .copied()
                            .map(|(loc, label)| {
                                view! {
                                    <button
                                        id=format!("stt-locale-option-{}", loc.as_str())
                                        type="button"
                                        role="option"
                                        class="harness-provider-option"
                                        class:harness-provider-option--active=move || active == loc
                                        aria-selected=move || if active == loc { "true" } else { "false" }
                                        on:click=move |_| choose(loc)
                                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                                            match ev.key().as_str() {
                                                "ArrowDown" => {
                                                    ev.prevent_default();
                                                    focus_stt_locale_option(next_stt_locale(loc));
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    focus_stt_locale_option(prev_stt_locale(loc));
                                                }
                                                "Enter" | " " => {
                                                    ev.prevent_default();
                                                    choose(loc);
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
                                                src=loc.flag_icon_url()
                                                alt=""
                                                loading="lazy"
                                            />
                                        </span>
                                        <span>{label}</span>
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

/// STT input language (Settings → Voice).
#[component]
pub fn VoiceSttLanguageControls<F>(
    settings: RwSignal<Option<VoiceSettings>>,
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
                let stt_lang = current.stt_language.clone();
                let is_follow = matches!(stt_lang, SttLanguageMode::FollowApp);
                let is_auto = matches!(stt_lang, SttLanguageMode::AutoDetect);
                let is_manual = matches!(stt_lang, SttLanguageMode::Manual { .. });
                let manual_code = if let SttLanguageMode::Manual { ref code } = stt_lang {
                    code.clone()
                } else {
                    String::new()
                };
                let manual_locale = Locale::from_iso639_1(&manual_code);

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
                                        on:change=move |_| {
                                            let Some(mut next) = settings.get_untracked() else { return };
                                            let code = if manual_code.trim().is_empty() {
                                                default_manual_iso639_1(&i18n)
                                            } else {
                                                manual_code.clone()
                                            };
                                            next.stt_language = SttLanguageMode::Manual { code };
                                            save(next);
                                        }
                                    />
                                    <span>{move || i18n.tr(I18nKey::VoiceSttLangManual)()}</span>
                                </label>
                            </div>
                        </div>
                        <Show when=move || is_manual>
                            <SttManualLocalePicker
                                settings=settings
                                selected=manual_locale
                                save=save
                            />
                        </Show>
                    </label>
                }
                .into_any()
            }}
        </Show>
    }
}
