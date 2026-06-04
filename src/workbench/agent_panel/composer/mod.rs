//! Modern agent chat composer: an auto-growing textarea plus a footer bar with
//! a model picker popover, a click-cycling mode/access pill (mapped onto the
//! existing [`AgentChatMode`] values), a thinking-level popover and the
//! send/stop orb.
//! Replaces the old single-line input + separate mode toolbar. Styling lives in
//! `composer.css`; only theme tokens are used.

use leptos::either::Either;
use leptos::ev;
use leptos::html;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::{BTreeMap, HashSet};
use wasm_bindgen::JsCast;

use crate::agent_wire::AgentChatMode;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, agent_settings_save, is_tauri_shell,
    AgentProviderKind, AgentProviderSettingsView, ProviderModelEntry, ThinkingLevel,
};
use crate::workbench::WorkbenchService;

const MODEL_FAVORITES_STORAGE_KEY: &str = "blxcode.agent.model_favorites.v1";

const COMPOSER_MODEL_PROVIDERS: [AgentProviderKind; 3] = [
    AgentProviderKind::Openrouter,
    AgentProviderKind::Openai,
    AgentProviderKind::Anthropic,
];

fn provider_cache_key(provider: AgentProviderKind) -> String {
    provider.as_str().to_string()
}

fn provider_display_name(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "OpenRouter",
        AgentProviderKind::Openai => "OpenAI",
        AgentProviderKind::Anthropic => "Anthropic",
        _ => "Provider",
    }
}

fn provider_icon_url(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        AgentProviderKind::Openai => "/public/brand-icons/openai.svg",
        AgentProviderKind::Anthropic => "/public/brand-icons/anthropic.svg",
        _ => "/public/brand-icons/provider.svg",
    }
}

fn openrouter_owner_slug(model_id: &str) -> Option<String> {
    model_id
        .split_once('/')
        .map(|(owner, _)| owner.trim().to_ascii_lowercase())
        .filter(|owner| !owner.is_empty())
}

fn owner_logo_url(owner_slug: &str) -> Option<&'static str> {
    match owner_slug {
        "openai" => Some("/public/brand-icons/openai.svg"),
        "anthropic" => Some("/public/brand-icons/anthropic.svg"),
        "google" => Some("/public/brand-icons/google.svg"),
        "mistral" => Some("/public/brand-icons/mistral.svg"),
        "x-ai" | "xai" => Some("/public/brand-icons/grok.svg"),
        "amazon" | "aws" => Some("/public/brand-icons/aws.svg"),
        _ => None,
    }
}

fn owner_initials(owner_slug: Option<&str>, model_id: &str) -> String {
    let source = owner_slug
        .filter(|owner| !owner.trim().is_empty())
        .unwrap_or(model_id);
    source
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn initial_provider_model_cache(
    view: &AgentProviderSettingsView,
) -> BTreeMap<String, Vec<ProviderModelEntry>> {
    let mut cache = view.model_caches.clone();
    cache
        .entry(provider_cache_key(AgentProviderKind::Openrouter))
        .or_insert_with(|| view.model_cache_openrouter.clone());
    cache
        .entry(provider_cache_key(AgentProviderKind::Openai))
        .or_insert_with(|| view.model_cache_openai.clone());
    cache
        .entry(provider_cache_key(AgentProviderKind::Anthropic))
        .or_insert_with(|| view.model_cache_anthropic.clone());
    cache
}

fn thinking_levels() -> [ThinkingLevel; 5] {
    [
        ThinkingLevel::Off,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Max,
    ]
}

fn thinking_key(level: ThinkingLevel) -> I18nKey {
    match level {
        ThinkingLevel::Off => I18nKey::AgThinkingOff,
        ThinkingLevel::Low => I18nKey::AgThinkingLow,
        ThinkingLevel::Medium => I18nKey::AgThinkingMedium,
        ThinkingLevel::High => I18nKey::AgThinkingHigh,
        ThinkingLevel::Max => I18nKey::AgThinkingMax,
    }
}

fn mode_label_key(mode: AgentChatMode) -> I18nKey {
    match mode {
        AgentChatMode::AskEdits => I18nKey::AgModeSupervised,
        AgentChatMode::AllowAll => I18nKey::AgModeFullAccess,
        AgentChatMode::Plan => I18nKey::AgModePlan,
    }
}

fn mode_icon(mode: AgentChatMode) -> icondata::Icon {
    match mode {
        AgentChatMode::AskEdits => icondata::LuShieldCheck,
        AgentChatMode::AllowAll => icondata::LuLockOpen,
        AgentChatMode::Plan => icondata::LuClipboardList,
    }
}

fn next_mode(mode: AgentChatMode) -> AgentChatMode {
    match mode {
        AgentChatMode::AskEdits => AgentChatMode::AllowAll,
        AgentChatMode::AllowAll => AgentChatMode::Plan,
        AgentChatMode::Plan => AgentChatMode::AskEdits,
    }
}

fn format_context_length(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M ctx", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{}K ctx", tokens / 1_000)
    } else {
        format!("{tokens} ctx")
    }
}

fn format_price_per_million(value: f64) -> String {
    let per_million = value * 1_000_000.0;
    if per_million >= 100.0 {
        format!("${per_million:.0}/M")
    } else if per_million >= 10.0 {
        format!("${per_million:.1}/M")
    } else {
        format!("${per_million:.2}/M")
    }
}

fn model_detail_line(model: &ProviderModelEntry) -> String {
    let mut parts = Vec::new();
    if let Some(context) = model.context_length {
        parts.push(format_context_length(context));
    }
    if let Some(pricing) = model.pricing {
        parts.push(format!(
            "in {} · out {}",
            format_price_per_million(pricing.prompt),
            format_price_per_million(pricing.completion)
        ));
    }
    if parts.is_empty() {
        model
            .description
            .clone()
            .unwrap_or_else(|| model.id.clone())
    } else {
        parts.join(" · ")
    }
}

fn model_matches_filter(model: &ProviderModelEntry, filter: &str) -> bool {
    filter.is_empty()
        || model.id.to_lowercase().contains(filter)
        || model.label.to_lowercase().contains(filter)
}

fn matching_provider_keys(
    cache: &BTreeMap<String, Vec<ProviderModelEntry>>,
    filter: &str,
) -> Vec<String> {
    COMPOSER_MODEL_PROVIDERS
        .into_iter()
        .filter_map(|provider| {
            let key = provider_cache_key(provider);
            if filter.is_empty()
                || cache.get(&key).is_some_and(|models| {
                    models
                        .iter()
                        .any(|model| model_matches_filter(model, filter))
                })
            {
                Some(key)
            } else {
                None
            }
        })
        .collect()
}

fn read_model_favorites() -> HashSet<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(MODEL_FAVORITES_STORAGE_KEY).ok().flatten())
        .map(|raw| {
            raw.split('\n')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn write_model_favorites(favorites: &HashSet<String>) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let mut ids: Vec<&str> = favorites.iter().map(String::as_str).collect();
    ids.sort_unstable();
    let _ = storage.set_item(MODEL_FAVORITES_STORAGE_KEY, &ids.join("\n"));
}

fn model_row(
    provider: AgentProviderKind,
    model: ProviderModelEntry,
    active: String,
    favorites: HashSet<String>,
    model_favorites: RwSignal<HashSet<String>>,
    persist: impl Fn(AgentProviderKind, Option<String>, Option<ThinkingLevel>) + Copy + 'static,
    model_open: RwSignal<bool>,
) -> AnyView {
    let i18n = expect_context::<I18nService>();
    let id = model.id.clone();
    let select_id = id.clone();
    let favorite_id = id.clone();
    let favorite_id_for_aria = id.clone();
    let favorite_id_for_class = id.clone();
    let favorite_id_for_click = id.clone();
    let label = if model.label.is_empty() {
        model.id.clone()
    } else {
        model.label.clone()
    };
    let detail = model_detail_line(&model);
    let is_active = id == active;
    let is_favorite = favorites.contains(&id);
    let owner_slug = if provider == AgentProviderKind::Openrouter {
        openrouter_owner_slug(&id)
    } else {
        Some(provider_cache_key(provider))
    };
    let owner_logo = owner_slug.as_deref().and_then(owner_logo_url);
    let owner_initials = owner_initials(owner_slug.as_deref(), &id);

    view! {
        <li>
            <div
                class="agent-composer__model-item"
                class:agent-composer__model-item--active=move || is_active
                class:agent-composer__model-item--favorite=move || model_favorites.with(|set| set.contains(&favorite_id_for_class))
            >
                <button
                    type="button"
                    class="agent-composer__model-main"
                    on:click=move |_| {
                        persist(provider, Some(select_id.clone()), None);
                        model_open.set(false);
                    }
                >
                    <span class="agent-composer__model-logo" aria-hidden="true">
                        {if let Some(url) = owner_logo {
                            Either::Left(view! {
                                <img class="agent-composer__model-logo-img" src=url alt="" />
                            })
                        } else {
                            Either::Right(view! {
                                <span class="agent-composer__model-logo-fallback">{owner_initials}</span>
                            })
                        }}
                    </span>
                    <span class="agent-composer__model-copy">
                        <span class="agent-composer__model-name">{label}</span>
                        <span class="agent-composer__model-meta">{detail}</span>
                    </span>
                    <Show when=move || is_active>
                        <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" />
                    </Show>
                </button>
                <button
                    type="button"
                    class="agent-composer__model-fav"
                    class:agent-composer__model-fav--active=move || model_favorites.with(|set| set.contains(&favorite_id))
                    aria-pressed=move || model_favorites.with(|set| set.contains(&favorite_id_for_aria)).to_string()
                    aria-label=move || {
                        if is_favorite {
                            i18n.tr(I18nKey::AgentComposerRemoveFavorite)()
                        } else {
                            i18n.tr(I18nKey::AgentComposerAddFavorite)()
                        }
                    }
                    title=move || {
                        if is_favorite {
                            i18n.tr(I18nKey::AgentComposerRemoveFavorite)()
                        } else {
                            i18n.tr(I18nKey::AgentComposerAddFavorite)()
                        }
                    }
                    on:click=move |_| {
                        model_favorites.update(|set| {
                            if !set.remove(&favorite_id_for_click) {
                                set.insert(favorite_id_for_click.clone());
                            }
                            write_model_favorites(set);
                        });
                    }
                >
                    <LxIcon icon=icondata::LuStar width="0.78rem" height="0.78rem" />
                </button>
            </div>
        </li>
    }
    .into_any()
}

#[allow(clippy::too_many_arguments)]
#[component]
pub fn Composer(
    draft: RwSignal<String>,
    chat_mode: RwSignal<AgentChatMode>,
    enhance_prompt: RwSignal<bool>,
    busy: RwSignal<bool>,
    model_label: RwSignal<String>,
    input_ref: NodeRef<html::Textarea>,
    wb: WorkbenchService,
    i18n: I18nService,
    on_submit: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    // Authoritative provider settings cache (loaded on mount). Saving a model
    // or thinking change rewrites the full patch from this snapshot so the
    // other fields are preserved.
    let settings = RwSignal::new(None::<AgentProviderSettingsView>);
    let thinking = RwSignal::new(ThinkingLevel::Medium);

    let model_open = RwSignal::new(false);
    let think_open = RwSignal::new(false);
    let open_provider_group = RwSignal::new(provider_cache_key(AgentProviderKind::Openrouter));
    let provider_group_manually_changed = RwSignal::new(false);

    let provider_models = RwSignal::new(BTreeMap::<String, Vec<ProviderModelEntry>>::new());
    let models_loading = RwSignal::new(HashSet::<String>::new());
    let model_filter = RwSignal::new(String::new());
    let model_favorites = RwSignal::new(read_model_favorites());

    let close_popovers = move || {
        model_open.set(false);
        think_open.set(false);
    };

    let close_on_outside = window_event_listener_untyped("mousedown", move |ev| {
        if !model_open.get_untracked() && !think_open.get_untracked() {
            return;
        }
        let inside_menu = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".agent-composer__menu").ok().flatten())
            .is_some();
        if !inside_menu {
            close_popovers();
        }
    });
    let close_on_escape = window_event_listener_untyped("keydown", move |ev| {
        let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if ev.key() == "Escape" {
            close_popovers();
        }
    });
    on_cleanup(move || {
        close_on_outside.remove();
        close_on_escape.remove();
    });

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(view) = agent_settings_get().await {
                thinking.set(view.thinking_level);
                provider_group_manually_changed.set(false);
                open_provider_group.set(provider_cache_key(view.provider));
                provider_models.set(initial_provider_model_cache(&view));
                settings.set(Some(view));
            }
        });
    }

    // Persist a model / thinking change, preserving every other setting.
    let persist = move |provider: AgentProviderKind,
                        new_model: Option<String>,
                        new_think: Option<ThinkingLevel>| {
        let Some(view) = settings.get_untracked() else {
            return;
        };
        let model_id = new_model.clone().unwrap_or_else(|| {
            if provider == view.provider {
                view.model_id.clone()
            } else {
                String::new()
            }
        });
        if model_id.is_empty() {
            return;
        }
        let level = new_think.unwrap_or(view.thinking_level);
        leptos::task::spawn_local(async move {
            if let Ok(updated) = agent_settings_save(
                provider,
                model_id.clone(),
                level,
                view.tool_loop_limit,
                view.auto_compact_enabled,
                view.auto_compact_threshold_pct,
                view.orb_mode,
                view.agent_nickname.clone(),
                view.default_session_role.clone(),
                view.provider_base_urls.clone(),
                view.cloudflare_account_id.clone(),
            )
            .await
            {
                model_label.set(format!(
                    "{}/{}",
                    updated.provider.as_str(),
                    updated.model_id
                ));
                thinking.set(updated.thinking_level);
                provider_group_manually_changed.set(false);
                open_provider_group.set(provider_cache_key(updated.provider));
                settings.set(Some(updated));
            }
        });
    };

    Effect::new(move |_| {
        let filter = model_filter.get().trim().to_ascii_lowercase();
        let Some(active_provider) = settings.get().map(|view| view.provider) else {
            return;
        };
        if filter.is_empty() {
            if !provider_group_manually_changed.get() {
                open_provider_group.set(provider_cache_key(active_provider));
            }
            return;
        }
        let cache = provider_models.get();
        let matching = matching_provider_keys(&cache, &filter);
        let current = open_provider_group.get();
        if !matching.iter().any(|key| key == &current) {
            if let Some(first) = matching.into_iter().next() {
                open_provider_group.set(first);
            }
        }
    });

    // Lazy-load each provider's model list the first time the picker opens.
    let load_models = move || {
        if !is_tauri_shell() {
            return;
        }
        for provider in COMPOSER_MODEL_PROVIDERS {
            let key = provider_cache_key(provider);
            let has_models = provider_models
                .with_untracked(|cache| cache.get(&key).is_some_and(|entries| !entries.is_empty()));
            let is_loading = models_loading.with_untracked(|loading| loading.contains(&key));
            if has_models || is_loading {
                continue;
            }
            models_loading.update(|loading| {
                loading.insert(key.clone());
            });
            leptos::task::spawn_local(async move {
                if let Ok(resp) = agent_provider_models(provider).await {
                    provider_models.update(|cache| {
                        cache.insert(provider_cache_key(resp.provider), resp.entries);
                    });
                }
                models_loading.update(|loading| {
                    loading.remove(&key);
                });
            });
        }
    };

    let submit = move || {
        model_open.set(false);
        think_open.set(false);
        on_submit.run(());
    };

    // Auto-grow the textarea to fit its content (capped by CSS max-height).
    let autosize = move || {
        if let Some(el) = input_ref.get_untracked() {
            el.style(("height", "auto"));
            let h = el.scroll_height();
            el.style(("height", format!("{h}px")));
        }
    };

    view! {
        <div class="agent-composer">
            <div class="agent-composer__field">
                <textarea
                    node_ref=input_ref
                    class="agent-composer__textarea"
                    rows="2"
                    placeholder=move || i18n.tr(I18nKey::AgComposerPh)()
                    prop:value=move || draft.get()
                    prop:disabled=move || busy.get()
                    on:input=move |ev| {
                        if let Some(t) = ev.target() {
                            if let Ok(area) = t.dyn_into::<web_sys::HtmlTextAreaElement>() {
                                let v = area.value();
                                draft.set(v.clone());
                                if let Some(id) = wb.active_id().get_untracked() {
                                    wb.set_workspace_agent_compose_draft(id, v);
                                }
                            }
                        }
                        autosize();
                    }
                    on:keydown=move |ev: ev::KeyboardEvent| {
                        if ev.key() == "Enter" && !ev.shift_key() && !ev.ctrl_key() && !ev.meta_key() {
                            ev.prevent_default();
                            submit();
                        }
                    }
                ></textarea>
            </div>

            <div class="agent-composer__footer">
                // ---- Model picker ----
                <div class="agent-composer__menu">
                    <button
                        type="button"
                        class="agent-composer__pill"
                        prop:disabled=move || busy.get()
                        on:click=move |_| {
                            let next = !model_open.get_untracked();
                            model_open.set(next);
                            think_open.set(false);
                            if next {
                                load_models();
                            }
                        }
                    >
                        <LxIcon icon=icondata::LuBox width="0.82rem" height="0.82rem" />
                        <span class="agent-composer__pill-label">{move || model_label.get()}</span>
                        <LxIcon icon=icondata::LuChevronDown width="0.78rem" height="0.78rem" />
                    </button>
                    <Show when=move || model_open.get()>
                        <div class="agent-composer__popover agent-composer__popover--model">
                            <input
                                type="text"
                                class="agent-composer__search"
                                placeholder=move || i18n.tr(I18nKey::AgComposerModelSearch)()
                                prop:value=move || model_filter.get()
                                on:input=move |ev| {
                                    if let Some(t) = ev.target() {
                                        if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                                            model_filter.set(inp.value());
                                        }
                                    }
                                }
                            />
                            <div class="agent-composer__provider-groups">
                                {move || {
                                    let filter = model_filter.get().to_lowercase();
                                    let filter_is_empty = filter.trim().is_empty();
                                    let active_settings = settings.get();
                                    let active = active_settings
                                        .as_ref()
                                        .map(|v| v.model_id.clone())
                                        .unwrap_or_default();
                                    let active_provider = active_settings
                                        .as_ref()
                                        .map(|v| v.provider)
                                        .unwrap_or(AgentProviderKind::Openrouter);
                                    let active_provider_key = provider_cache_key(active_provider);
                                    let favorites = model_favorites.get();
                                    let model_cache = provider_models.get();
                                    let loading_providers = models_loading.get();
                                    COMPOSER_MODEL_PROVIDERS
                                        .into_iter()
                                        .filter_map(|provider| {
                                            let provider_key = provider_cache_key(provider);
                                            let provider_label = provider_display_name(provider);
                                            let provider_icon = provider_icon_url(provider);
                                            let is_open = provider_key == open_provider_group.get();
                                            let is_active_provider = provider_key == active_provider_key;
                                            let is_loading = loading_providers.contains(&provider_key);
                                            let models = model_cache
                                                .get(&provider_key)
                                                .cloned()
                                                .unwrap_or_default();
                                            let mut active_row = None::<ProviderModelEntry>;
                                            let mut rest = Vec::<ProviderModelEntry>::new();
                                            for model in models
                                                .into_iter()
                                                .filter(|model| model_matches_filter(model, &filter))
                                            {
                                                if is_active_provider && model.id == active && active_row.is_none() {
                                                    active_row = Some(model);
                                                } else {
                                                    rest.push(model);
                                                }
                                            }
                                            let match_count = active_row.iter().count() + rest.len();
                                            if !filter_is_empty && match_count == 0 && !is_loading {
                                                return None;
                                            }
                                            rest.sort_by_key(|m| (!favorites.contains(&m.id), m.label.to_lowercase(), m.id.clone()));
                                            let click_provider_key = provider_key.clone();
                                            let group_class = if is_open {
                                                "agent-composer__provider-group agent-composer__provider-group--open"
                                            } else {
                                                "agent-composer__provider-group"
                                            };
                                            let count = if is_loading {
                                                "...".to_string()
                                            } else {
                                                match_count.to_string()
                                            };
                                            let active_badge = if is_active_provider {
                                                Either::Left(view! {
                                                    <span class="agent-composer__provider-active">"Active"</span>
                                                })
                                            } else {
                                                Either::Right(())
                                            };
                                            let panel = if is_open {
                                                let content = if is_loading && match_count == 0 {
                                                    Either::Left(view! {
                                                        <div class="agent-composer__model-empty">"Loading models..."</div>
                                                    })
                                                } else if match_count == 0 {
                                                    Either::Left(view! {
                                                        <div class="agent-composer__model-empty">"No models"</div>
                                                    })
                                                } else {
                                                    let mut rows = Vec::new();
                                                    if let Some(model) = active_row {
                                                        rows.push(model_row(
                                                            provider,
                                                            model,
                                                            active.clone(),
                                                            favorites.clone(),
                                                            model_favorites,
                                                            persist,
                                                            model_open,
                                                        ));
                                                        if !rest.is_empty() {
                                                            rows.push(view! { <li class="agent-composer__model-separator" aria-hidden="true"></li> }.into_any());
                                                        }
                                                    }
                                                    rows.extend(rest.into_iter().map(|model| {
                                                        model_row(
                                                            provider,
                                                            model,
                                                            active.clone(),
                                                            favorites.clone(),
                                                            model_favorites,
                                                            persist,
                                                            model_open,
                                                        )
                                                    }));
                                                    Either::Right(view! {
                                                        <ul class="agent-composer__model-list">
                                                            {rows.into_iter().collect_view()}
                                                        </ul>
                                                    })
                                                };
                                                Either::Left(view! {
                                                    <div class="agent-composer__provider-panel">{content}</div>
                                                })
                                            } else {
                                                Either::Right(())
                                            };
                                            view! {
                                                <section class=group_class>
                                                    <button
                                                        type="button"
                                                        class="agent-composer__provider-header"
                                                        aria-expanded=is_open.to_string()
                                                        on:click=move |_| {
                                                            provider_group_manually_changed.set(true);
                                                            open_provider_group.set(click_provider_key.clone());
                                                        }
                                                    >
                                                        <span class="agent-composer__provider-logo" aria-hidden="true">
                                                            <img class="agent-composer__provider-logo-img" src=provider_icon alt="" />
                                                        </span>
                                                        <span class="agent-composer__provider-headline">
                                                            <span class="agent-composer__provider-name">{provider_label}</span>
                                                            {active_badge}
                                                        </span>
                                                        <span class="agent-composer__provider-count">{count}</span>
                                                        <LxIcon icon=icondata::LuChevronDown width="0.78rem" height="0.78rem" />
                                                    </button>
                                                    {panel}
                                                </section>
                                            }
                                            .into_any()
                                            .into()
                                        })
                                        .collect_view()
                                }}
                            </div>
                        </div>
                    </Show>
                </div>

                // ---- Mode / access cycle ----
                <div class="agent-composer__menu">
                    <button
                        type="button"
                        class="agent-composer__pill agent-composer__pill--mode"
                        prop:disabled=move || busy.get()
                        on:click=move |_| {
                            let next = next_mode(chat_mode.get_untracked());
                            chat_mode.set(next);
                            if let Some(ws_id) = wb.active_id().get_untracked() {
                                wb.set_workspace_agent_chat_mode(ws_id, next);
                            }
                            model_open.set(false);
                            think_open.set(false);
                        }
                    >
                        {move || {
                            let m = chat_mode.get();
                            view! { <LxIcon icon=mode_icon(m) width="0.82rem" height="0.82rem" /> }
                        }}
                        <span class="agent-composer__pill-label">
                            {move || i18n.tr(mode_label_key(chat_mode.get()))()}
                        </span>
                    </button>
                </div>

                // ---- Thinking level ----
                <div class="agent-composer__menu">
                    <button
                        type="button"
                        class="agent-composer__pill"
                        prop:disabled=move || busy.get()
                        on:click=move |_| {
                            let next = !think_open.get_untracked();
                            think_open.set(next);
                            model_open.set(false);
                        }
                    >
                        <LxIcon icon=icondata::LuBrain width="0.82rem" height="0.82rem" />
                        <span class="agent-composer__pill-label">
                            {move || i18n.tr(thinking_key(thinking.get()))()}
                        </span>
                        <LxIcon icon=icondata::LuChevronDown width="0.78rem" height="0.78rem" />
                    </button>
                    <Show when=move || think_open.get()>
                        <div class="agent-composer__popover">
                            {thinking_levels().into_iter().map(|level| {
                                let is_active = Memo::new(move |_| thinking.get() == level);
                                view! {
                                    <button
                                        type="button"
                                        class="agent-composer__option"
                                        class:agent-composer__option--active=move || is_active.get()
                                        on:click=move |_| {
                                            let provider = settings
                                                .get_untracked()
                                                .map(|view| view.provider)
                                                .unwrap_or(AgentProviderKind::Openrouter);
                                            persist(provider, None, Some(level));
                                            think_open.set(false);
                                        }
                                    >
                                        <span class="agent-composer__option-head">
                                            <span class="agent-composer__option-title">
                                                {move || i18n.tr(thinking_key(level))()}
                                            </span>
                                            <Show when=move || is_active.get()>
                                                <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" />
                                            </Show>
                                        </span>
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </Show>
                </div>

                <span class="blx-tip-anchor blx-tip-anchor--top agent-composer__enhance-tip">
                    <button
                        type="button"
                        class=move || if enhance_prompt.get() {
                            "agent-composer__enhance agent-composer__enhance--active"
                        } else {
                            "agent-composer__enhance"
                        }
                        prop:disabled=move || busy.get()
                        aria-pressed=move || enhance_prompt.get().to_string()
                        aria-describedby="agent-composer-enhance-tooltip"
                        aria-label=move || i18n.tr(I18nKey::AgComposerEnhancePrompt)()
                        on:click=move |_| {
                            let next = !enhance_prompt.get_untracked();
                            enhance_prompt.set(next);
                            if let Some(ws_id) = wb.active_id().get_untracked() {
                                wb.set_workspace_agent_enhance_prompt(ws_id, next);
                            }
                        }
                    >
                        <LxIcon icon=icondata::LuSparkles width="0.9rem" height="0.9rem" />
                    </button>
                    <span id="agent-composer-enhance-tooltip" class="blx-tooltip agent-composer__tooltip" role="tooltip">
                        <span class="blx-tooltip__eyebrow">
                            <span class="blx-tooltip__spark" aria-hidden="true"></span>
                            "Prompt"
                        </span>
                        <span class="blx-tooltip__main">{move || i18n.tr(I18nKey::AgComposerEnhancePrompt)()}</span>
                        <span class="blx-tooltip__hint">"Rewrite before sending"</span>
                    </span>
                </span>

                <span class="agent-composer__spacer"></span>

                // ---- Send / stop orb ----
                <button
                    type="button"
                    class=move || if busy.get() {
                        "agent-composer__send agent-composer__send--stop"
                    } else {
                        "agent-composer__send"
                    }
                    aria-label=move || if busy.get() {
                        i18n.tr(I18nKey::AgCancel)()
                    } else {
                        i18n.tr(I18nKey::AgSend)()
                    }
                    on:mousedown=|ev| ev.prevent_default()
                    on:click=move |_| {
                        if busy.get_untracked() {
                            on_cancel.run(());
                        } else {
                            submit();
                        }
                    }
                >
                    {move || if busy.get() {
                        Either::Left(view! { <LxIcon icon=icondata::LuSquare width="0.95rem" height="0.95rem" /> })
                    } else {
                        Either::Right(view! { <LxIcon icon=icondata::LuArrowUp width="1rem" height="1rem" /> })
                    }}
                </button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, label: &str) -> ProviderModelEntry {
        ProviderModelEntry {
            id: id.to_string(),
            label: label.to_string(),
            description: None,
            pricing: None,
            context_length: None,
        }
    }

    #[test]
    fn openrouter_owner_slug_should_extract_prefix() {
        assert_eq!(
            openrouter_owner_slug("anthropic/claude-4-sonnet"),
            Some("anthropic".to_string())
        );
        assert_eq!(openrouter_owner_slug("gpt-4.1"), None);
    }

    #[test]
    fn owner_logo_url_should_map_common_owners() {
        assert_eq!(
            owner_logo_url("openai"),
            Some("/public/brand-icons/openai.svg")
        );
        assert_eq!(owner_logo_url("x-ai"), Some("/public/brand-icons/grok.svg"));
        assert_eq!(
            owner_logo_url("amazon"),
            Some("/public/brand-icons/aws.svg")
        );
        assert_eq!(owner_logo_url("unknown-lab"), None);
    }

    #[test]
    fn owner_initials_should_fallback_from_owner_or_model() {
        assert_eq!(
            owner_initials(Some("unknown-lab"), "unknown-lab/model"),
            "UL"
        );
        assert_eq!(owner_initials(None, "solo-model"), "SM");
    }

    #[test]
    fn matching_provider_keys_should_hide_groups_without_search_hits() {
        let mut cache = BTreeMap::new();
        cache.insert(
            provider_cache_key(AgentProviderKind::Openrouter),
            vec![model("anthropic/claude-sonnet-4", "Claude Sonnet 4")],
        );
        cache.insert(
            provider_cache_key(AgentProviderKind::Openai),
            vec![model("gpt-4.1", "GPT-4.1")],
        );
        cache.insert(
            provider_cache_key(AgentProviderKind::Anthropic),
            vec![model("claude-opus-4", "Claude Opus 4")],
        );

        assert_eq!(
            matching_provider_keys(&cache, "gpt"),
            vec![provider_cache_key(AgentProviderKind::Openai)]
        );
        assert_eq!(
            matching_provider_keys(&cache, "claude"),
            vec![
                provider_cache_key(AgentProviderKind::Openrouter),
                provider_cache_key(AgentProviderKind::Anthropic),
            ]
        );
    }

    #[test]
    fn matching_provider_keys_should_show_all_groups_without_filter() {
        let cache = BTreeMap::new();
        assert_eq!(
            matching_provider_keys(&cache, ""),
            vec![
                provider_cache_key(AgentProviderKind::Openrouter),
                provider_cache_key(AgentProviderKind::Openai),
                provider_cache_key(AgentProviderKind::Anthropic),
            ]
        );
    }
}
