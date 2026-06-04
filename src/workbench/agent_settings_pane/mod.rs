//! Settings → **Agent**. Rebuilt as four stacked cards — Personality, Provider,
//! Models, Configuration — backed by a shared [`AgentSettingsCtx`] and per-section
//! auto-save.
//!
//! Decoupling contract: the STT/TTS provider/model controls and the reply
//! voice/gender here drive only the `stt`/`tts` sub-objects of `VoiceSettings`.
//! The Push-to-Talk config (`VoiceSettings.ptt`, owned by the separate Voice
//! settings pane) is preserved verbatim on every save.

mod configuration;
mod data;
mod models;
mod personality;
mod pickers;
mod providers;

pub(crate) use data::{GenderFilter, SpeechKind};

use std::collections::BTreeMap;

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_session_roles_list, agent_settings_get, agent_settings_save,
    agent_web_settings_get, agent_web_settings_save, api_keys_status, image_settings_get,
    image_settings_save, is_tauri_shell, voice_settings_get, voice_settings_save, AgentOrbMode,
    AgentProviderKind, AgentProviderSettingsView, ApiKeysStatus, ImageProviderKind, ImageSettings,
    ProviderModelEntry, ProviderModelsResponse, SessionRoleView, ThinkingLevel, VoiceProviderKind,
    VoiceSettings, WebProviderKind, DEFAULT_AUTO_COMPACT_THRESHOLD_PCT, DEFAULT_TOOL_LOOP_LIMIT,
    MAX_AUTO_COMPACT_THRESHOLD_PCT, MAX_TOOL_LOOP_LIMIT, MIN_AUTO_COMPACT_THRESHOLD_PCT,
    MIN_TOOL_LOOP_LIMIT,
};
use crate::workbench::{SettingsPaneHeader, WorkbenchService};

use data::{choose_model_for_provider, fetch_image_models, fetch_voice_models, provider_cache};

/// All reactive state + save/refresh callbacks for the Agent settings pane,
/// shared with the section components through Leptos context.
#[derive(Clone, Copy)]
pub(crate) struct AgentSettingsCtx {
    // Loaded server views.
    pub settings: RwSignal<Option<AgentProviderSettingsView>>,
    pub voice_settings: RwSignal<Option<VoiceSettings>>,
    pub api_keys: RwSignal<Option<ApiKeysStatus>>,
    pub session_roles: RwSignal<Vec<SessionRoleView>>,

    // Personality / agent core.
    pub nickname: RwSignal<String>,
    pub nickname_error: RwSignal<Option<I18nKey>>,
    pub orb_mode: RwSignal<AgentOrbMode>,
    pub role: RwSignal<Option<String>>,
    pub thinking: RwSignal<ThinkingLevel>,
    pub gender_filter: RwSignal<GenderFilter>,

    // Chat provider / model.
    pub chat_provider: RwSignal<AgentProviderKind>,
    pub chat_model: RwSignal<String>,
    pub chat_models: RwSignal<Vec<ProviderModelEntry>>,
    pub chat_models_loading: RwSignal<bool>,
    pub chat_models_source: RwSignal<String>,
    pub provider_base_urls: RwSignal<BTreeMap<String, String>>,
    pub cloudflare_account_id: RwSignal<String>,

    // Image.
    pub image_provider: RwSignal<ImageProviderKind>,
    pub image_model: RwSignal<String>,
    pub image_models: RwSignal<Vec<ProviderModelEntry>>,
    pub image_models_loading: RwSignal<bool>,
    pub image_quality: RwSignal<crate::tauri_bridge::ImageQualityLevel>,

    // STT / TTS (independent — never PTT).
    pub stt_provider: RwSignal<VoiceProviderKind>,
    pub stt_model: RwSignal<String>,
    pub stt_models: RwSignal<Vec<ProviderModelEntry>>,
    pub stt_models_loading: RwSignal<bool>,
    pub sample_rate_hz: RwSignal<u32>,
    pub tts_provider: RwSignal<VoiceProviderKind>,
    pub tts_model: RwSignal<String>,
    pub tts_models: RwSignal<Vec<ProviderModelEntry>>,
    pub tts_models_loading: RwSignal<bool>,
    pub tts_voice: RwSignal<String>,
    pub tts_enabled: RwSignal<bool>,
    pub post_stt_flow: RwSignal<crate::tauri_bridge::PostSttFlow>,

    // Configuration / misc.
    pub tool_loop_limit: RwSignal<u32>,
    pub auto_compact_enabled: RwSignal<bool>,
    pub auto_compact_threshold: RwSignal<u8>,
    pub web_provider: RwSignal<WebProviderKind>,

    // Status.
    pub status_msg: RwSignal<Option<String>>,
    pub error_msg: RwSignal<Option<String>>,

    // Per-section auto-save.
    pub save_agent_core: Callback<()>,
    pub save_image: Callback<()>,
    pub save_voice: Callback<()>,
    pub save_web: Callback<()>,

    // Model refreshes.
    pub refresh_chat_models: Callback<()>,
    pub refresh_image_models: Callback<()>,
    pub refresh_stt_models: Callback<()>,
    pub refresh_tts_models: Callback<()>,
}

fn dispatch_agent_settings_changed() {
    if let Some(window) = web_sys::window() {
        if let Ok(ev) = web_sys::CustomEvent::new("blxcode-agent-settings-changed") {
            let _ = window.dispatch_event(&ev);
        }
    }
}

#[component]
pub fn AgentSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();

    let ctx = AgentSettingsCtx {
        settings: RwSignal::new(None),
        voice_settings: RwSignal::new(None),
        api_keys: RwSignal::new(None),
        session_roles: RwSignal::new(Vec::new()),
        nickname: RwSignal::new(String::new()),
        nickname_error: RwSignal::new(None),
        orb_mode: RwSignal::new(AgentOrbMode::ThreeD),
        role: RwSignal::new(None),
        thinking: RwSignal::new(ThinkingLevel::Medium),
        gender_filter: RwSignal::new(GenderFilter::All),
        chat_provider: RwSignal::new(AgentProviderKind::Openrouter),
        chat_model: RwSignal::new(String::new()),
        chat_models: RwSignal::new(Vec::new()),
        chat_models_loading: RwSignal::new(false),
        chat_models_source: RwSignal::new(String::new()),
        provider_base_urls: RwSignal::new(BTreeMap::new()),
        cloudflare_account_id: RwSignal::new(String::new()),
        image_provider: RwSignal::new(ImageProviderKind::Openai),
        image_model: RwSignal::new(String::new()),
        image_models: RwSignal::new(Vec::new()),
        image_models_loading: RwSignal::new(false),
        image_quality: RwSignal::new(crate::tauri_bridge::ImageQualityLevel::Medium),
        stt_provider: RwSignal::new(VoiceProviderKind::Openai),
        stt_model: RwSignal::new(String::new()),
        stt_models: RwSignal::new(Vec::new()),
        stt_models_loading: RwSignal::new(false),
        sample_rate_hz: RwSignal::new(16_000),
        tts_provider: RwSignal::new(VoiceProviderKind::Openai),
        tts_model: RwSignal::new(String::new()),
        tts_models: RwSignal::new(Vec::new()),
        tts_models_loading: RwSignal::new(false),
        tts_voice: RwSignal::new(String::new()),
        tts_enabled: RwSignal::new(true),
        post_stt_flow: RwSignal::new(crate::tauri_bridge::PostSttFlow::AutoSend),
        tool_loop_limit: RwSignal::new(DEFAULT_TOOL_LOOP_LIMIT),
        auto_compact_enabled: RwSignal::new(true),
        auto_compact_threshold: RwSignal::new(DEFAULT_AUTO_COMPACT_THRESHOLD_PCT),
        web_provider: RwSignal::new(WebProviderKind::None),
        status_msg: RwSignal::new(None),
        error_msg: RwSignal::new(None),
        // Placeholders; replaced below once the closures are built.
        save_agent_core: Callback::new(|_| {}),
        save_image: Callback::new(|_| {}),
        save_voice: Callback::new(|_| {}),
        save_web: Callback::new(|_| {}),
        refresh_chat_models: Callback::new(|_| {}),
        refresh_image_models: Callback::new(|_| {}),
        refresh_stt_models: Callback::new(|_| {}),
        refresh_tts_models: Callback::new(|_| {}),
    };

    // ----- model refreshers -----------------------------------------------
    let refresh_chat_models = move |_: ()| {
        let provider = ctx.chat_provider.get_untracked();
        ctx.chat_models_loading.set(true);
        leptos::task::spawn_local(async move {
            if let Ok(ProviderModelsResponse {
                entries, source, ..
            }) = agent_provider_models(provider).await
            {
                if provider == ctx.chat_provider.get_untracked() {
                    if let Some(model) = choose_model_for_provider(
                        &ctx.chat_model.get_untracked(),
                        &ctx.chat_models.get_untracked(),
                        &entries,
                    ) {
                        ctx.chat_model.set(model);
                    }
                    ctx.chat_models.set(entries);
                    ctx.chat_models_source.set(source);
                }
            }
            ctx.chat_models_loading.set(false);
        });
    };
    let refresh_image_models = move |_: ()| {
        let provider = ctx.image_provider.get_untracked();
        ctx.image_models_loading.set(true);
        leptos::task::spawn_local(async move {
            fetch_image_models(provider, ctx.image_models).await;
            ctx.image_models_loading.set(false);
        });
    };
    let refresh_stt_models = move |_: ()| {
        let provider = ctx.stt_provider.get_untracked();
        ctx.stt_models_loading.set(true);
        leptos::task::spawn_local(async move {
            fetch_voice_models(provider, SpeechKind::Stt, ctx.stt_models).await;
            ctx.stt_models_loading.set(false);
        });
    };
    let refresh_tts_models = move |_: ()| {
        let provider = ctx.tts_provider.get_untracked();
        ctx.tts_models_loading.set(true);
        leptos::task::spawn_local(async move {
            fetch_voice_models(provider, SpeechKind::Tts, ctx.tts_models).await;
            ctx.tts_models_loading.set(false);
        });
    };

    // ----- savers ----------------------------------------------------------
    let apply_agent_view = move |view: AgentProviderSettingsView| {
        ctx.chat_provider.set(view.provider);
        ctx.chat_model.set(view.model_id.clone());
        ctx.thinking.set(view.thinking_level);
        ctx.tool_loop_limit.set(view.tool_loop_limit);
        ctx.auto_compact_enabled.set(view.auto_compact_enabled);
        ctx.auto_compact_threshold
            .set(view.auto_compact_threshold_pct);
        ctx.orb_mode.set(view.orb_mode);
        ctx.nickname.set(view.agent_nickname.clone());
        ctx.role.set(view.default_session_role.clone());
        ctx.provider_base_urls.set(view.provider_base_urls.clone());
        ctx.cloudflare_account_id
            .set(view.cloudflare_account_id.clone());
        ctx.chat_models.set(provider_cache(&view, view.provider));
        ctx.settings.set(Some(view));
    };

    let save_agent_core = move |_: ()| {
        if !is_tauri_shell() || ctx.nickname_error.get_untracked().is_some() {
            return;
        }
        let provider = ctx.chat_provider.get_untracked();
        let model_id = ctx.chat_model.get_untracked();
        let level = ctx.thinking.get_untracked();
        let loop_limit = ctx
            .tool_loop_limit
            .get_untracked()
            .clamp(MIN_TOOL_LOOP_LIMIT, MAX_TOOL_LOOP_LIMIT);
        let ac_enabled = ctx.auto_compact_enabled.get_untracked();
        let ac_threshold = ctx.auto_compact_threshold.get_untracked().clamp(
            MIN_AUTO_COMPACT_THRESHOLD_PCT,
            MAX_AUTO_COMPACT_THRESHOLD_PCT,
        );
        let orb = ctx.orb_mode.get_untracked();
        let nick = ctx.nickname.get_untracked();
        let role = ctx.role.get_untracked();
        let base_urls = ctx.provider_base_urls.get_untracked();
        let cf = ctx.cloudflare_account_id.get_untracked();
        leptos::task::spawn_local(async move {
            match agent_settings_save(
                provider,
                model_id,
                level,
                loop_limit,
                ac_enabled,
                ac_threshold,
                orb,
                nick,
                role,
                base_urls,
                cf,
            )
            .await
            {
                Ok(view) => {
                    apply_agent_view(view);
                    ctx.error_msg.set(None);
                    ctx.status_msg
                        .set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string()));
                    dispatch_agent_settings_changed();
                }
                Err(e) => ctx.error_msg.set(Some(e)),
            }
        });
    };

    let save_image = move |_: ()| {
        if !is_tauri_shell() {
            return;
        }
        let patch = ImageSettings {
            provider: ctx.image_provider.get_untracked(),
            model_id: ctx.image_model.get_untracked(),
            quality: ctx.image_quality.get_untracked(),
        };
        leptos::task::spawn_local(async move {
            match image_settings_save(patch).await {
                Ok(s) => {
                    ctx.image_provider.set(s.provider);
                    ctx.image_quality.set(s.quality);
                    ctx.image_model.set(s.model_id);
                    ctx.status_msg
                        .set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string()));
                }
                Err(e) => ctx.error_msg.set(Some(e)),
            }
        });
    };

    // Builds the voice patch from a loaded base so `ptt` / `ptt_hotkey` /
    // `stt_language` are preserved verbatim — only stt/tts/post_stt_flow change.
    let save_voice = move |_: ()| {
        if !is_tauri_shell() {
            return;
        }
        let Some(mut patch) = ctx.voice_settings.get_untracked() else {
            return;
        };
        patch.stt.provider = ctx.stt_provider.get_untracked();
        patch.stt.model_id = ctx.stt_model.get_untracked();
        patch.stt.sample_rate_hz = ctx.sample_rate_hz.get_untracked();
        patch.tts.provider = ctx.tts_provider.get_untracked();
        patch.tts.model_id = ctx.tts_model.get_untracked();
        patch.tts.voice = ctx.tts_voice.get_untracked();
        patch.tts.enabled = ctx.tts_enabled.get_untracked();
        patch.post_stt_flow = ctx.post_stt_flow.get_untracked();
        leptos::task::spawn_local(async move {
            match voice_settings_save(patch).await {
                Ok(v) => {
                    ctx.voice_settings.set(Some(v));
                    ctx.status_msg
                        .set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string()));
                }
                Err(e) => ctx.error_msg.set(Some(e)),
            }
        });
    };

    let save_web = move |_: ()| {
        if !is_tauri_shell() {
            return;
        }
        let provider = ctx.web_provider.get_untracked();
        leptos::task::spawn_local(async move {
            match agent_web_settings_save(provider).await {
                Ok(_) => ctx
                    .status_msg
                    .set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string())),
                Err(e) => ctx.error_msg.set(Some(e)),
            }
        });
    };

    let ctx = AgentSettingsCtx {
        save_agent_core: Callback::new(save_agent_core),
        save_image: Callback::new(save_image),
        save_voice: Callback::new(save_voice),
        save_web: Callback::new(save_web),
        refresh_chat_models: Callback::new(refresh_chat_models),
        refresh_image_models: Callback::new(refresh_image_models),
        refresh_stt_models: Callback::new(refresh_stt_models),
        refresh_tts_models: Callback::new(refresh_tts_models),
        ..ctx
    };

    // ----- initial load ----------------------------------------------------
    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            if let Ok(list) = agent_session_roles_list().await {
                ctx.session_roles.set(list);
            }
            if let Ok(keys) = api_keys_status().await {
                ctx.api_keys.set(Some(keys));
            }
            match agent_settings_get().await {
                Ok(view) => {
                    ctx.error_msg.set(None);
                    apply_agent_view(view);
                    wb.set_default_session_role(ctx.role.get_untracked());
                }
                Err(e) => ctx.error_msg.set(Some(e)),
            }
            if let Ok(s) = image_settings_get().await {
                ctx.image_provider.set(s.provider);
                ctx.image_quality.set(s.quality);
                ctx.image_model.set(s.model_id);
                fetch_image_models(ctx.image_provider.get_untracked(), ctx.image_models).await;
            }
            if let Ok(v) = voice_settings_get().await {
                ctx.stt_provider.set(v.stt.provider);
                ctx.stt_model.set(v.stt.model_id.clone());
                ctx.sample_rate_hz.set(v.stt.sample_rate_hz);
                ctx.tts_provider.set(v.tts.provider);
                ctx.tts_model.set(v.tts.model_id.clone());
                ctx.tts_voice.set(v.tts.voice.clone());
                ctx.tts_enabled.set(v.tts.enabled);
                ctx.post_stt_flow.set(v.post_stt_flow);
                fetch_voice_models(v.stt.provider, SpeechKind::Stt, ctx.stt_models).await;
                fetch_voice_models(v.tts.provider, SpeechKind::Tts, ctx.tts_models).await;
                ctx.voice_settings.set(Some(v));
            }
            if let Ok(w) = agent_web_settings_get().await {
                ctx.web_provider.set(w.settings.provider);
            }
        });
    });

    provide_context(ctx);

    view! {
        <article class="harness-pane agent-settings-pane">
            <SettingsPaneHeader
                icon=icondata::LuBot
                title=I18nKey::AgProviderHeading
                description=I18nKey::AgProviderDescription
            />

            <personality::PersonalitySection />
            <providers::ProvidersSection />
            <models::ModelsSection />
            <configuration::ConfigurationSection />

            <Show when=move || ctx.status_msg.with(|m| m.is_some())>
                <p class="harness-status">{move || ctx.status_msg.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || ctx.error_msg.with(|m| m.is_some())>
                <p class="harness-error-text">{move || ctx.error_msg.get().unwrap_or_default()}</p>
            </Show>
        </article>
    }
}

/// Helper used by sections: debounce a text/number input save by `ms`.
pub(crate) fn debounced(gen: RwSignal<u32>, ms: u32, run: Callback<()>) {
    let token = gen.get_untracked().wrapping_add(1);
    gen.set(token);
    leptos::task::spawn_local(async move {
        TimeoutFuture::new(ms).await;
        if gen.get_untracked() == token {
            run.run(());
        }
    });
}

/// Shared subhead row used by every card/sub-group.
#[component]
pub(crate) fn CardHead(icon: icondata::Icon, label: I18nKey) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <h4 class="harness-pane-subhead">
            <span class="harness-pane-subhead__icon" aria-hidden="true">
                <LxIcon icon=icon width="0.9rem" height="0.9rem" />
            </span>
            <span class="harness-pane-subhead__text">{move || i18n.tr(label)()}</span>
        </h4>
    }
}
