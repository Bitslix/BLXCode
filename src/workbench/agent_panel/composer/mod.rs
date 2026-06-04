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
use std::collections::HashSet;
use wasm_bindgen::JsCast;

use crate::agent_wire::AgentChatMode;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, agent_settings_save, is_tauri_shell,
    AgentProviderSettingsView, ProviderModelEntry, ThinkingLevel,
};
use crate::workbench::WorkbenchService;

const MODEL_FAVORITES_STORAGE_KEY: &str = "blxcode.agent.model_favorites.v1";

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
    model: ProviderModelEntry,
    active: String,
    favorites: HashSet<String>,
    model_favorites: RwSignal<HashSet<String>>,
    persist: impl Fn(Option<String>, Option<ThinkingLevel>) + Copy + 'static,
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
                        persist(Some(select_id.clone()), None);
                        model_open.set(false);
                    }
                >
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

    let models = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let models_loading = RwSignal::new(false);
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
                settings.set(Some(view));
            }
        });
    }

    // Persist a model / thinking change, preserving every other setting.
    let persist = move |new_model: Option<String>, new_think: Option<ThinkingLevel>| {
        let Some(view) = settings.get_untracked() else {
            return;
        };
        let provider = view.provider;
        let model_id = new_model.clone().unwrap_or_else(|| view.model_id.clone());
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
                settings.set(Some(updated));
            }
        });
    };

    // Lazy-load the provider's model list the first time the picker opens.
    let load_models = move || {
        if !is_tauri_shell() || models_loading.get_untracked() {
            return;
        }
        let Some(provider) = settings.get_untracked().map(|v| v.provider) else {
            return;
        };
        models_loading.set(true);
        leptos::task::spawn_local(async move {
            if let Ok(resp) = agent_provider_models(provider).await {
                models.set(resp.entries);
            }
            models_loading.set(false);
        });
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
                            <ul class="agent-composer__model-list">
                                {move || {
                                    let filter = model_filter.get().to_lowercase();
                                    let active = settings.get().map(|v| v.model_id).unwrap_or_default();
                                    let favorites = model_favorites.get();
                                    let matches_filter = |m: &ProviderModelEntry| {
                                        filter.is_empty()
                                            || m.id.to_lowercase().contains(&filter)
                                            || m.label.to_lowercase().contains(&filter)
                                    };
                                    let mut active_row = None::<ProviderModelEntry>;
                                    let mut rest = Vec::<ProviderModelEntry>::new();
                                    for model in models.get().into_iter().filter(matches_filter) {
                                        if model.id == active && active_row.is_none() {
                                            active_row = Some(model);
                                        } else {
                                            rest.push(model);
                                        }
                                    }
                                    rest.sort_by_key(|m| (!favorites.contains(&m.id), m.label.to_lowercase(), m.id.clone()));
                                    let mut rows = Vec::new();
                                    if let Some(model) = active_row {
                                        rows.push(model_row(
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
                                            model,
                                            active.clone(),
                                            favorites.clone(),
                                            model_favorites,
                                            persist,
                                            model_open,
                                        )
                                    }));
                                    rows.into_iter().collect_view()
                                }}
                            </ul>
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
                                            persist(None, Some(level));
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
