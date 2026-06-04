//! Personality card — Name, 2D|3D orb, Role, Intelligence, Gender, reply Voice.

use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_validate_nickname, voice_tts_preview, AgentOrbMode, ThinkingLevel, VoiceEntry,
    VoiceGender,
};
use crate::workbench::{SessionRolePicker, WorkbenchService};

use super::data::{
    gender_icon, nickname_err_key, play_b64, thinking_icon, thinking_label, thinking_levels,
    voice_catalog_for, voices_pick_enabled, GenderFilter,
};
use super::pickers::{OptionPicker, PickerOption};
use super::{debounced, AgentSettingsCtx, CardHead};

fn thinking_id(level: ThinkingLevel) -> String {
    format!("{level:?}").to_ascii_lowercase()
}

fn thinking_from_id(id: &str) -> ThinkingLevel {
    thinking_levels()
        .into_iter()
        .find(|l| thinking_id(*l) == id)
        .unwrap_or(ThinkingLevel::Medium)
}

#[component]
pub(crate) fn PersonalitySection() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let ctx = expect_context::<AgentSettingsCtx>();
    let name_gen = RwSignal::new(0u32);

    let thinking_options = Signal::derive(move || {
        thinking_levels()
            .into_iter()
            .map(|l| PickerOption::lucide(thinking_id(l), thinking_label(&i18n, l), thinking_icon(l)))
            .collect::<Vec<_>>()
    });

    view! {
        <section class="harness-subpane agent-settings-card">
            <CardHead icon=icondata::LuUser label=I18nKey::AgSecPersonality />

            <div class="agent-grid">
                // --- Name ---
                <label class="agent-field">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuUser width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgFieldName)()}</span>
                    </span>
                    <input
                        class="workbench-plain-input"
                        class:agent-input--error=move || ctx.nickname_error.get().is_some()
                        type="text"
                        maxlength="32"
                        prop:value=move || ctx.nickname.get()
                        placeholder=move || i18n.tr(I18nKey::AgNicknamePlaceholder)()
                        on:input=move |ev| {
                            let val = event_target_value(&ev);
                            ctx.nickname.set(val.clone());
                            let val2 = val.clone();
                            leptos::task::spawn_local(async move {
                                let res = agent_validate_nickname(val2.clone()).await;
                                if ctx.nickname.get_untracked() == val2 {
                                    match res {
                                        Ok(()) => ctx.nickname_error.set(None),
                                        Err(code) => ctx.nickname_error.set(Some(nickname_err_key(&code))),
                                    }
                                }
                            });
                            debounced(name_gen, 600, ctx.save_agent_core);
                        }
                    />
                    <Show
                        when=move || ctx.nickname_error.get().is_some()
                        fallback=move || view! {
                            <small class="harness-muted agent-field__hint">
                                {move || i18n.tr(I18nKey::AgNicknameHelp)()}
                            </small>
                        }
                    >
                        <small class="agent-field__error">
                            {move || ctx.nickname_error.get().map(|k| i18n.tr(k)()).unwrap_or_default()}
                        </small>
                    </Show>
                </label>

                // --- 2D | 3D orb ---
                <div class="agent-field agent-field--orb">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuBot width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgOrbField)()}</span>
                    </span>
                    <span class="agent-switch-row">
                        <input
                            class="agent-switch-input"
                            type="checkbox"
                            prop:checked=move || ctx.orb_mode.get() == AgentOrbMode::ThreeD
                            on:change=move |ev| {
                                if let Some(t) = ev.target() {
                                    if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                        ctx.orb_mode.set(if inp.checked() {
                                            AgentOrbMode::ThreeD
                                        } else {
                                            AgentOrbMode::TwoD
                                        });
                                        ctx.save_agent_core.run(());
                                    }
                                }
                            }
                        />
                        <span
                            class="blx-switch"
                            class:blx-switch--on=move || ctx.orb_mode.get() == AgentOrbMode::ThreeD
                            aria-hidden="true"
                        >
                            <span class="blx-switch__thumb" />
                        </span>
                        <span>{move || if ctx.orb_mode.get() == AgentOrbMode::ThreeD {
                            i18n.tr(I18nKey::AgOrb3d)()
                        } else {
                            i18n.tr(I18nKey::AgOrb2d)()
                        }}</span>
                    </span>
                    <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::AgOrbHint)()}</small>
                </div>

                // --- Role ---
                <div class="agent-field">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuSparkles width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::WzSessionRoleLabel)()}</span>
                    </span>
                    <SessionRolePicker
                        id="agent-settings-default-role-picker".to_string()
                        roles=Signal::derive(move || ctx.session_roles.get())
                        selected=Signal::derive(move || ctx.role.get())
                        on_select=Callback::new(move |role: Option<String>| {
                            ctx.role.set(role.clone());
                            wb.set_default_session_role(role);
                            ctx.save_agent_core.run(());
                        })
                    />
                    <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::AgRoleHint)()}</small>
                </div>

                // --- Intelligence (thinking level) ---
                <div class="agent-field">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuBrain width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgFieldIntelligence)()}</span>
                    </span>
                    <OptionPicker
                        prefix="thinking"
                        level_style=true
                        options=thinking_options
                        selected=Signal::derive(move || thinking_id(ctx.thinking.get()))
                        on_select=Callback::new(move |id: String| {
                            ctx.thinking.set(thinking_from_id(&id));
                            ctx.save_agent_core.run(());
                        })
                    />
                </div>
            </div>

            // --- Gender filter ---
            <div class="agent-field">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuVenetianMask width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgFieldGender)()}</span>
                </span>
                <div class="agent-choice-row">
                    <GenderButton target=GenderFilter::All key=I18nKey::VoiceGenderAll filter=ctx.gender_filter />
                    <GenderButton target=GenderFilter::Male key=I18nKey::VoiceGenderMale filter=ctx.gender_filter />
                    <GenderButton target=GenderFilter::Female key=I18nKey::VoiceGenderFemale filter=ctx.gender_filter />
                    <GenderButton target=GenderFilter::Neutral key=I18nKey::VoiceGenderNeutral filter=ctx.gender_filter />
                </div>
            </div>

            // --- Reply voice ---
            <div class="agent-field">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuVolume2 width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgColumnVoice)()}</span>
                </span>
                <Show
                    when=move || !voices_pick_enabled(ctx.tts_provider.get())
                    fallback=|| ()
                >
                    <small class="harness-muted agent-field__hint">{move || i18n.tr(I18nKey::VoiceVoicesAwsOnly)()}</small>
                </Show>
                <div
                    class="agent-voice-grid"
                    class:agent-voice-grid--disabled=move || !voices_pick_enabled(ctx.tts_provider.get())
                >
                    {move || {
                        let active = ctx.tts_voice.get();
                        let filter = ctx.gender_filter.get();
                        let disabled = !voices_pick_enabled(ctx.tts_provider.get());
                        voice_catalog_for(ctx.tts_provider.get())
                            .into_iter()
                            .filter(|e| filter.matches(e.gender))
                            .map(|entry| view! {
                                <VoiceCard entry=entry.clone() active=entry.id == active disabled=disabled />
                            })
                            .collect_view()
                    }}
                </div>
            </div>
        </section>
    }
}

#[component]
fn GenderButton(target: GenderFilter, key: I18nKey, filter: RwSignal<GenderFilter>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="agent-choice"
            class:agent-choice--active=move || filter.get() == target
            on:click=move |_| filter.set(target)
        >
            {move || i18n.tr(key)()}
        </button>
    }
}

#[component]
fn VoiceCard(entry: VoiceEntry, active: bool, disabled: bool) -> impl IntoView {
    let ctx = expect_context::<AgentSettingsCtx>();
    let id_pick = entry.id.clone();
    let id_preview = entry.id.clone();
    view! {
        <div
            class="agent-voice-card"
            class:agent-voice-card--active=move || active && !disabled
        >
            <button
                type="button"
                class="agent-voice-pick"
                disabled=disabled
                on:click=move |_| {
                    if !disabled {
                        ctx.tts_voice.set(id_pick.clone());
                        ctx.save_voice.run(());
                    }
                }
            >
                <strong>{entry.label.clone()}</strong>
                <span class="agent-voice-gender">{gender_label(entry.gender)}</span>
            </button>
            <button
                type="button"
                class="agent-voice-preview"
                disabled=disabled
                on:click=move |_| {
                    if disabled {
                        return;
                    }
                    let Some(s) = ctx.voice_settings.get_untracked() else {
                        return;
                    };
                    let provider = ctx.tts_provider.get_untracked();
                    let model = s.tts.model_id.clone();
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

fn gender_label(g: VoiceGender) -> &'static str {
    gender_icon(g)
}
