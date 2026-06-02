//! Modern agent chat composer: an auto-growing textarea plus a footer bar with
//! a model picker popover, a mode/access popover (mapped onto the existing
//! [`AgentChatMode`] values), a thinking-level popover and the send/stop orb.
//! Replaces the old single-line input + separate mode toolbar. Styling lives in
//! `composer.css`; only theme tokens are used.

use leptos::either::Either;
use leptos::ev;
use leptos::html;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

use crate::agent_wire::AgentChatMode;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_provider_models, agent_settings_get, agent_settings_save, is_tauri_shell,
    AgentProviderSettingsView, ProviderModelEntry, ThinkingLevel,
};
use crate::workbench::WorkbenchService;

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

/// The three chat modes shown in the access popover, mapped onto the existing
/// [`AgentChatMode`] values (label, description).
fn mode_entries() -> [(AgentChatMode, I18nKey, I18nKey); 3] {
    [
        (
            AgentChatMode::AskEdits,
            I18nKey::AgModeSupervised,
            I18nKey::AgModeSupervisedDesc,
        ),
        (
            AgentChatMode::AllowAll,
            I18nKey::AgModeFullAccess,
            I18nKey::AgModeFullAccessDesc,
        ),
        (
            AgentChatMode::Plan,
            I18nKey::AgModePlan,
            I18nKey::AgModePlanDesc,
        ),
    ]
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

#[allow(clippy::too_many_arguments)]
#[component]
pub fn Composer(
    draft: RwSignal<String>,
    chat_mode: RwSignal<AgentChatMode>,
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
    let mode_open = RwSignal::new(false);
    let think_open = RwSignal::new(false);

    let models = RwSignal::new(Vec::<ProviderModelEntry>::new());
    let models_loading = RwSignal::new(false);
    let model_filter = RwSignal::new(String::new());

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
            )
            .await
            {
                model_label.set(format!("{}/{}", updated.provider.as_str(), updated.model_id));
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
        mode_open.set(false);
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
                    rows="1"
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
                            mode_open.set(false);
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
                                    models.get().into_iter().filter(move |m| {
                                        filter.is_empty()
                                            || m.id.to_lowercase().contains(&filter)
                                            || m.label.to_lowercase().contains(&filter)
                                    }).map(move |m| {
                                        let id = m.id.clone();
                                        let is_active = id == active;
                                        let label = if m.label.is_empty() { m.id.clone() } else { m.label.clone() };
                                        view! {
                                            <li>
                                                <button
                                                    type="button"
                                                    class="agent-composer__model-item"
                                                    class:agent-composer__model-item--active=move || is_active
                                                    on:click=move |_| {
                                                        persist(Some(id.clone()), None);
                                                        model_open.set(false);
                                                    }
                                                >
                                                    <span class="agent-composer__model-name">{label}</span>
                                                    <Show when=move || is_active>
                                                        <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" />
                                                    </Show>
                                                </button>
                                            </li>
                                        }
                                    }).collect_view()
                                }}
                            </ul>
                        </div>
                    </Show>
                </div>

                // ---- Mode / access popover ----
                <div class="agent-composer__menu">
                    <button
                        type="button"
                        class="agent-composer__pill"
                        prop:disabled=move || busy.get()
                        on:click=move |_| {
                            let next = !mode_open.get_untracked();
                            mode_open.set(next);
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
                        <LxIcon icon=icondata::LuChevronDown width="0.78rem" height="0.78rem" />
                    </button>
                    <Show when=move || mode_open.get()>
                        <div class="agent-composer__popover">
                            {mode_entries().into_iter().map(|(mode, label_key, desc_key)| {
                                let is_active = Memo::new(move |_| chat_mode.get() == mode);
                                view! {
                                    <button
                                        type="button"
                                        class="agent-composer__option"
                                        class:agent-composer__option--active=move || is_active.get()
                                        on:click=move |_| {
                                            chat_mode.set(mode);
                                            if let Some(ws_id) = wb.active_id().get_untracked() {
                                                wb.set_workspace_agent_chat_mode(ws_id, mode);
                                            }
                                            mode_open.set(false);
                                        }
                                    >
                                        <span class="agent-composer__option-head">
                                            <LxIcon icon=mode_icon(mode) width="0.82rem" height="0.82rem" />
                                            <span class="agent-composer__option-title">
                                                {move || i18n.tr(label_key)()}
                                            </span>
                                            <Show when=move || is_active.get()>
                                                <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" />
                                            </Show>
                                        </span>
                                        <span class="agent-composer__option-desc">
                                            {move || i18n.tr(desc_key)()}
                                        </span>
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </Show>
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
                            mode_open.set(false);
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
