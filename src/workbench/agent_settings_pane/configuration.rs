//! Configuration card — Images, Audio, Misc and Web search sub-groups.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    ImageQualityLevel, PostSttFlow, WebProviderKind, MAX_AUTO_COMPACT_THRESHOLD_PCT,
    MAX_TOOL_LOOP_LIMIT, MIN_AUTO_COMPACT_THRESHOLD_PCT, MIN_TOOL_LOOP_LIMIT,
};

use super::data::{image_quality_icon, image_quality_label, image_quality_levels};
use super::pickers::{OptionPicker, PickerOption};
use super::{debounced, AgentSettingsCtx, CardHead};

fn quality_id(q: ImageQualityLevel) -> String {
    format!("{q:?}").to_ascii_lowercase()
}

fn quality_from_id(id: &str) -> ImageQualityLevel {
    image_quality_levels()
        .into_iter()
        .find(|q| quality_id(*q) == id)
        .unwrap_or(ImageQualityLevel::Medium)
}

#[component]
pub(crate) fn ConfigurationSection() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ctx = expect_context::<AgentSettingsCtx>();
    let loop_gen = RwSignal::new(0u32);
    let pct_gen = RwSignal::new(0u32);

    let quality_options = Signal::derive(move || {
        image_quality_levels()
            .into_iter()
            .map(|q| {
                PickerOption::lucide(
                    quality_id(q),
                    image_quality_label(&i18n, q),
                    image_quality_icon(q),
                )
            })
            .collect::<Vec<_>>()
    });

    view! {
        <section class="harness-subpane agent-settings-card">
            <CardHead icon=icondata::LuSlidersHorizontal label=I18nKey::AgSecConfiguration />

            // --- Images ---
            <div class="agent-subgroup">
                <span class="agent-subgroup__head">
                    <LxIcon icon=icondata::LuImage />
                    <span>{move || i18n.tr(I18nKey::AgGrpImages)()}</span>
                </span>
                <div class="agent-subgroup__row">
                    <div class="agent-field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgImageQualityField)()}</span>
                        </span>
                        <OptionPicker
                            prefix="image-quality"
                            level_style=true
                            options=quality_options
                            selected=Signal::derive(move || quality_id(ctx.image_quality.get()))
                            on_select=Callback::new(move |id: String| {
                                ctx.image_quality.set(quality_from_id(&id));
                                ctx.save_image.run(());
                            })
                        />
                    </div>
                </div>
            </div>

            // --- Audio ---
            <div class="agent-subgroup">
                <span class="agent-subgroup__head">
                    <LxIcon icon=icondata::LuMic />
                    <span>{move || i18n.tr(I18nKey::AgGrpAudio)()}</span>
                </span>
                <div class="agent-field">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::VoiceQualityField)()}</span>
                    </span>
                    <div class="agent-choice-row">
                        <RateChoice rate=16_000 key=I18nKey::VoiceQualityLow />
                        <RateChoice rate=24_000 key=I18nKey::VoiceQualityStandard />
                        <RateChoice rate=48_000 key=I18nKey::VoiceQualityHigh />
                    </div>
                    <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::VoiceQualityHint)()}</small>
                </div>
                <label class="agent-inline-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || ctx.tts_enabled.get()
                        on:change=move |ev| {
                            if let Some(t) = ev.target() {
                                if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                    ctx.tts_enabled.set(inp.checked());
                                    ctx.save_voice.run(());
                                }
                            }
                        }
                    />
                    <span>{move || i18n.tr(I18nKey::VoiceTtsEnabled)()}</span>
                </label>
            </div>

            // --- Misc ---
            <div class="agent-subgroup">
                <span class="agent-subgroup__head">
                    <LxIcon icon=icondata::LuSettings2 />
                    <span>{move || i18n.tr(I18nKey::AgGrpMisc)()}</span>
                </span>
                <div class="agent-subgroup__row">
                    // Tool loop maximum
                    <label class="agent-field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuRepeat width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgToolLoopLimitField)()}</span>
                        </span>
                        <input
                            class="workbench-plain-input"
                            type="number"
                            min=MIN_TOOL_LOOP_LIMIT.to_string()
                            max=MAX_TOOL_LOOP_LIMIT.to_string()
                            step="1"
                            inputmode="numeric"
                            prop:value=move || ctx.tool_loop_limit.get().to_string()
                            on:input=move |ev| {
                                if let Ok(parsed) = event_target_value(&ev).trim().parse::<u32>() {
                                    ctx.tool_loop_limit.set(parsed.clamp(MIN_TOOL_LOOP_LIMIT, MAX_TOOL_LOOP_LIMIT));
                                    debounced(loop_gen, 600, ctx.save_agent_core);
                                }
                            }
                        />
                    </label>

                    // Compaction level %
                    <div class="agent-field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuShrink width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgAutoCompactField)()}</span>
                        </span>
                        <span class="agent-inline-toggle">
                            <input
                                type="checkbox"
                                prop:checked=move || ctx.auto_compact_enabled.get()
                                on:change=move |ev| {
                                    if let Some(t) = ev.target() {
                                        if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                            ctx.auto_compact_enabled.set(inp.checked());
                                            ctx.save_agent_core.run(());
                                        }
                                    }
                                }
                            />
                            <input
                                class="workbench-plain-input agent-pct-input"
                                type="number"
                                min=MIN_AUTO_COMPACT_THRESHOLD_PCT.to_string()
                                max=MAX_AUTO_COMPACT_THRESHOLD_PCT.to_string()
                                step="1"
                                inputmode="numeric"
                                prop:disabled=move || !ctx.auto_compact_enabled.get()
                                prop:value=move || ctx.auto_compact_threshold.get().to_string()
                                on:input=move |ev| {
                                    if let Ok(parsed) = event_target_value(&ev).trim().parse::<u8>() {
                                        ctx.auto_compact_threshold.set(parsed.clamp(MIN_AUTO_COMPACT_THRESHOLD_PCT, MAX_AUTO_COMPACT_THRESHOLD_PCT));
                                        debounced(pct_gen, 600, ctx.save_agent_core);
                                    }
                                }
                            />
                            <span class="harness-muted">"%"</span>
                        </span>
                    </div>

                    // After transcription
                    <div class="agent-field">
                        <span class="harness-field-label">
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuCornerDownLeft width="0.82rem" height="0.82rem" />
                            </span>
                            <span class="harness-field-label__text">{move || i18n.tr(I18nKey::VoicePostSttFlow)()}</span>
                        </span>
                        <div class="agent-choice-row">
                            <FlowChoice target=PostSttFlow::AutoSend key=I18nKey::VoicePostSttAutoSend />
                            <FlowChoice target=PostSttFlow::Draft key=I18nKey::VoicePostSttDraft />
                        </div>
                    </div>
                </div>
            </div>

            // --- Web search ---
            <div class="agent-subgroup">
                <span class="agent-subgroup__head">
                    <LxIcon icon=icondata::LuGlobe />
                    <span>{move || i18n.tr(I18nKey::AgGrpWebSearch)()}</span>
                </span>
                <div class="agent-choice-row">
                    <WebChoice target=WebProviderKind::None key=I18nKey::AgWebProviderNone />
                    <WebChoice target=WebProviderKind::Brave key=I18nKey::AgWebProviderBrave />
                    <WebChoice target=WebProviderKind::Tavily key=I18nKey::AgWebProviderTavily />
                </div>
                <p class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::ApiKeysManageHintWeb)()}</p>
            </div>
        </section>
    }
}

#[component]
fn RateChoice(rate: u32, key: I18nKey) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ctx = expect_context::<AgentSettingsCtx>();
    view! {
        <button
            type="button"
            class="agent-choice"
            class:agent-choice--active=move || ctx.sample_rate_hz.get() == rate
            on:click=move |_| {
                ctx.sample_rate_hz.set(rate);
                ctx.save_voice.run(());
            }
        >
            {move || i18n.tr(key)()}
        </button>
    }
}

#[component]
fn FlowChoice(target: PostSttFlow, key: I18nKey) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ctx = expect_context::<AgentSettingsCtx>();
    view! {
        <button
            type="button"
            class="agent-choice"
            class:agent-choice--active=move || ctx.post_stt_flow.get() == target
            on:click=move |_| {
                ctx.post_stt_flow.set(target);
                ctx.save_voice.run(());
            }
        >
            {move || i18n.tr(key)()}
        </button>
    }
}

#[component]
fn WebChoice(target: WebProviderKind, key: I18nKey) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let ctx = expect_context::<AgentSettingsCtx>();
    view! {
        <button
            type="button"
            class="agent-choice"
            class:agent-choice--active=move || ctx.web_provider.get() == target
            on:click=move |_| {
                ctx.web_provider.set(target);
                ctx.save_web.run(());
            }
        >
            {move || i18n.tr(key)()}
        </button>
    }
}
