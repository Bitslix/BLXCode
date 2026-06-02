//! Standalone Settings -> Voice pane for push-to-talk transcription.

mod model_manager;
mod ptt_section;

use leptos::prelude::*;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, voice_settings_get, voice_settings_save, VoiceSettings};
use crate::workbench::{voice_app_controls::VoiceSttLanguageControls, SettingsPaneHeader};

/// Full Settings -> Voice pane.
#[component]
pub fn VoiceSettingsPane() -> impl IntoView {
    view! {
        <article class="harness-pane voice-settings-pane">
            <SettingsPaneHeader
                icon=icondata::LuMic
                title=I18nKey::VoicePaneTitle
                description=I18nKey::VoicePaneDescription
            />
            <PttVoiceSettingsContent />
        </article>
    }
}

#[component]
fn PttVoiceSettingsContent() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings = RwSignal::new(Option::<VoiceSettings>::None);
    let status = RwSignal::new(Option::<String>::None);

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(v) = voice_settings_get().await {
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
                    status.set(Some(i18n.tr(I18nKey::VoiceSaveDone)().to_string()));
                }
                Err(e) => status.set(Some(e)),
            }
        });
    };

    view! {
        <div class="voice-settings-pane__ptt-only">
            <Show
                when=move || settings.get().is_some()
                fallback=move || view! {
                    <p class="voice-pane__loading">{move || i18n.tr(I18nKey::BlxLoading)()}</p>
                }
            >
                <section class="harness-subpane voice-settings-pane__stt-language">
                    <VoiceSttLanguageControls settings=settings save=save />
                </section>
                <ptt_section::PushToTalkSection settings=settings save=save />
            </Show>
            <Show when=move || status.get().is_some()>
                <p class="voice-pane__status">{move || status.get().unwrap_or_default()}</p>
            </Show>
        </div>
    }
}
