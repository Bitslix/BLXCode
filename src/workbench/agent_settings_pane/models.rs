//! Models card — Chat / Image / STT / TTS model pickers + per-channel refresh.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, ProviderModelEntry};
use crate::workbench::agent_model_picker::AgentModelPicker;

use super::{AgentSettingsCtx, CardHead};

#[component]
pub(crate) fn ModelsSection() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ctx = expect_context::<AgentSettingsCtx>();

    let chat_source = Signal::derive(move || match ctx.chat_models_source.get().as_str() {
        "live" => i18n.tr(I18nKey::AgModelsSourceLive)().to_string(),
        "cache" => i18n.tr(I18nKey::AgModelsSourceCache)().to_string(),
        "curated" | "fallback" => i18n.tr(I18nKey::AgModelsSourceCurated)().to_string(),
        _ => String::new(),
    });
    let empty_source = Signal::derive(String::new);

    view! {
        <section class="harness-subpane agent-settings-card">
            <CardHead icon=icondata::LuBoxes label=I18nKey::AgSecModels />

            <div class="agent-grid">
                <ChannelModel
                    label=I18nKey::AgChannelChat
                    prefix="agent-chat-model"
                    model_id=ctx.chat_model
                    entries=ctx.chat_models
                    loading=ctx.chat_models_loading
                    show_custom=true
                    on_change=Callback::new(move |_| ctx.save_agent_core.run(()))
                    refresh=ctx.refresh_chat_models
                    source=chat_source
                />
                <ChannelModel
                    label=I18nKey::AgChannelImage
                    prefix="agent-image-model"
                    model_id=ctx.image_model
                    entries=ctx.image_models
                    loading=ctx.image_models_loading
                    show_custom=true
                    on_change=Callback::new(move |_| ctx.save_image.run(()))
                    refresh=ctx.refresh_image_models
                    source=empty_source
                />
                <ChannelModel
                    label=I18nKey::AgChannelStt
                    prefix="agent-stt-model"
                    model_id=ctx.stt_model
                    entries=ctx.stt_models
                    loading=ctx.stt_models_loading
                    show_custom=false
                    on_change=Callback::new(move |_| ctx.save_voice.run(()))
                    refresh=ctx.refresh_stt_models
                    source=empty_source
                />
                <ChannelModel
                    label=I18nKey::AgChannelTts
                    prefix="agent-tts-model"
                    model_id=ctx.tts_model
                    entries=ctx.tts_models
                    loading=ctx.tts_models_loading
                    show_custom=false
                    on_change=Callback::new(move |_| ctx.save_voice.run(()))
                    refresh=ctx.refresh_tts_models
                    source=empty_source
                />
            </div>
        </section>
    }
}

#[component]
fn ChannelModel(
    label: I18nKey,
    prefix: &'static str,
    model_id: RwSignal<String>,
    entries: RwSignal<Vec<ProviderModelEntry>>,
    loading: RwSignal<bool>,
    show_custom: bool,
    on_change: Callback<String>,
    refresh: Callback<()>,
    source: Signal<String>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <div class="agent-field">
            <span class="harness-field-label">
                <span class="harness-field-label__text">{move || i18n.tr(label)()}</span>
            </span>
            <AgentModelPicker
                model_id=model_id
                model_entries=entries
                loading_models=loading
                option_id_prefix=prefix
                show_custom_field=show_custom
                on_change=on_change
            />
            <div class="agent-actions">
                <button
                    type="button"
                    class="workbench-mini-btn"
                    disabled=move || loading.get() || !is_tauri_shell()
                    on:click=move |_| refresh.run(())
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
                <Show when=move || !source.get().is_empty()>
                    <small class="harness-muted">{move || source.get()}</small>
                </Show>
            </div>
        </div>
    }
}
