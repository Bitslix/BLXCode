//! Centralized API-keys pane (Settings → API Keys).
//!
//! Draft model: every text edit and per-row remove writes into a local
//! draft map. The single footer Save dispatches the full batch via
//! `api_keys_apply`; Discard rolls back without IPC. A window-level
//! `beforeunload` listener nudges the user when the draft is dirty so
//! reloads/quits don't silently drop pending keys.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    api_keys_apply, api_keys_status, is_tauri_shell, ApiKeyAction, ApiKeyCategory, ApiKeyEntry,
    ApiKeysStatus,
};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use std::collections::BTreeMap;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug)]
enum DraftAction {
    Set(String),
    Delete,
}

type DraftMap = BTreeMap<String, DraftAction>;

fn input_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

fn draft_to_actions(drafts: &DraftMap) -> Vec<ApiKeyAction> {
    drafts
        .iter()
        .filter_map(|(kind, action)| match action {
            DraftAction::Set(value) if !value.trim().is_empty() => Some(ApiKeyAction::Set {
                kind: kind.clone(),
                value: value.trim().to_string(),
            }),
            DraftAction::Set(_) => None,
            DraftAction::Delete => Some(ApiKeyAction::Delete { kind: kind.clone() }),
        })
        .collect()
}

#[component]
pub fn ApiKeysPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let status: RwSignal<Option<ApiKeysStatus>> = RwSignal::new(None);
    let drafts: RwSignal<DraftMap> = RwSignal::new(DraftMap::new());
    let busy = RwSignal::new(false);
    let status_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);

    let dirty = Memo::new(move |_| !drafts.with(|m| m.is_empty()));

    let load = move || {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match api_keys_status().await {
                Ok(s) => {
                    status.set(Some(s));
                    error_msg.set(None);
                }
                Err(err) => error_msg.set(Some(err)),
            }
        });
    };

    Effect::new(move |_| {
        load();
    });

    // `beforeunload` warning while the draft is dirty.
    Effect::new(move |_| {
        let handler = window_event_listener_untyped("beforeunload", move |ev| {
            if !dirty.get_untracked() {
                return;
            }
            if let Ok(be) = ev.dyn_into::<web_sys::BeforeUnloadEvent>() {
                be.prevent_default();
                be.set_return_value("");
            }
        });
        on_cleanup(move || handler.remove());
    });

    let save = move || {
        let actions = draft_to_actions(&drafts.get_untracked());
        if actions.is_empty() {
            drafts.set(DraftMap::new());
            return;
        }
        busy.set(true);
        error_msg.set(None);
        status_msg.set(None);
        leptos::task::spawn_local(async move {
            match api_keys_apply(actions).await {
                Ok(s) => {
                    status.set(Some(s));
                    drafts.set(DraftMap::new());
                    status_msg.set(Some(i18n.tr(I18nKey::ApiKeysSaved)()));
                }
                Err(err) => error_msg.set(Some(err)),
            }
            busy.set(false);
        });
    };

    let discard = move || {
        drafts.set(DraftMap::new());
        status_msg.set(None);
        error_msg.set(None);
    };

    let llm_entries = Memo::new(move |_| {
        status
            .get()
            .map(|s| {
                s.entries
                    .into_iter()
                    .filter(|e| matches!(e.category, ApiKeyCategory::Llm))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let search_entries = Memo::new(move |_| {
        status
            .get()
            .map(|s| {
                s.entries
                    .into_iter()
                    .filter(|e| matches!(e.category, ApiKeyCategory::Search))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });

    view! {
        <article class="harness-pane api-keys-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuKeyRound width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::ApiKeysHeading)()}</span>
            </h3>

            <p class="harness-muted">{move || i18n.tr(I18nKey::AgApiKeyHint)()}</p>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuCpu width="0.9rem" height="0.9rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::ApiKeysLlmSubhead)()}</span>
                </h4>
                <ApiKeyRows entries=llm_entries drafts=drafts />
            </section>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuGlobe width="0.9rem" height="0.9rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::ApiKeysSearchSubhead)()}</span>
                </h4>
                <ApiKeyRows entries=search_entries drafts=drafts />
            </section>

            <Show when=move || status_msg.with(|m| m.is_some())>
                <p class="harness-status">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || error_msg.with(|m| m.is_some())>
                <p class="harness-error">{move || error_msg.get().unwrap_or_default()}</p>
            </Show>

            <footer class="api-keys-footer harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    prop:disabled=move || busy.get() || !dirty.get() || !is_tauri_shell()
                    on:click=move |_| save()
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::BtnSave)()}</span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    prop:disabled=move || busy.get() || !dirty.get()
                    on:click=move |_| discard()
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuUndo2 width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::ApiKeysDiscard)()}</span>
                    </span>
                </button>
                <Show when=move || dirty.get()>
                    <span class="api-keys-dirty harness-muted">{move || i18n.tr(I18nKey::ApiKeysUnsaved)()}</span>
                </Show>
            </footer>
        </article>
    }
}

#[component]
fn ApiKeyRows(entries: Memo<Vec<ApiKeyEntry>>, drafts: RwSignal<DraftMap>) -> impl IntoView {
    view! {
        <ul class="api-keys-list">
            <For
                each=move || entries.get()
                key=|e| e.kind.clone()
                children=move |entry: ApiKeyEntry| {
                    view! { <ApiKeyRow entry=entry drafts=drafts /> }
                }
            />
        </ul>
    }
}

#[component]
fn ApiKeyRow(entry: ApiKeyEntry, drafts: RwSignal<DraftMap>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let kind: std::sync::Arc<str> = std::sync::Arc::from(entry.kind.as_str());
    let coming_soon = entry.coming_soon;
    let configured = entry.configured;
    let via_env = entry.via_env;

    let label = StoredValue::new(entry.label.clone());
    let masked = StoredValue::new(entry.masked_value.clone().unwrap_or_default());
    let env_var = StoredValue::new(entry.env_var.clone());
    let show_env_hint = !configured || via_env;

    let kind_for_draft = kind.clone();
    let draft_value = Signal::derive(move || {
        drafts.with(|m| match m.get(kind_for_draft.as_ref()) {
            Some(DraftAction::Set(v)) => v.clone(),
            _ => String::new(),
        })
    });

    let kind_for_delete_check = kind.clone();
    let marked_delete = Signal::derive(move || {
        drafts.with(|m| matches!(m.get(kind_for_delete_check.as_ref()), Some(DraftAction::Delete)))
    });

    let kind_for_dirty = kind.clone();
    let row_dirty = Signal::derive(move || {
        drafts.with(|m| m.contains_key(kind_for_dirty.as_ref()))
    });

    let kind_for_input = kind.clone();
    let on_input = move |ev: web_sys::Event| {
        let Some(value) = input_str(&ev) else { return };
        let k = kind_for_input.to_string();
        drafts.update(|m| {
            if value.is_empty() {
                m.remove(&k);
            } else {
                m.insert(k, DraftAction::Set(value));
            }
        });
    };

    let kind_for_undo = kind.clone();
    let on_undo = move |_| {
        let k = kind_for_undo.to_string();
        drafts.update(|m| {
            m.remove(&k);
        });
    };

    let kind_for_remove = kind.clone();
    let on_remove = move |_| {
        let k = kind_for_remove.to_string();
        drafts.update(|m| {
            m.insert(k, DraftAction::Delete);
        });
    };

    view! {
        <li
            class="api-keys-row"
            class:api-keys-row--coming-soon=move || coming_soon
            class:api-keys-row--marked-delete=move || marked_delete.get()
        >
            <div class="api-keys-row__head">
                <span class="api-keys-row__label">{move || label.get_value()}</span>
                {move || {
                    if coming_soon {
                        view! {
                            <span class="api-keys-row__badge">{move || i18n.tr(I18nKey::ApiKeysComingSoon)()}</span>
                        }.into_any()
                    } else if marked_delete.get() {
                        view! {
                            <span class="api-keys-row__badge api-keys-row__badge--warn">
                                {move || i18n.tr(I18nKey::ApiKeysWillBeRemoved)()}
                            </span>
                        }.into_any()
                    } else if configured && via_env {
                        view! {
                            <span class="api-keys-row__badge">{move || i18n.tr(I18nKey::ApiKeysViaEnv)()}</span>
                        }.into_any()
                    } else if configured {
                        view! { <span class="api-keys-row__masked">{move || masked.get_value()}</span> }.into_any()
                    } else {
                        view! {
                            <span class="api-keys-row__badge api-keys-row__badge--muted">
                                {move || i18n.tr(I18nKey::ApiKeysNotSet)()}
                            </span>
                        }.into_any()
                    }
                }}
            </div>
            <Show when=move || !coming_soon>
                <div class="api-keys-row__body harness-row-gap">
                    <input
                        class="workbench-plain-input api-keys-row__input"
                        type="password"
                        autocomplete="off"
                        prop:value=move || draft_value.get()
                        prop:disabled=move || marked_delete.get()
                        on:input=on_input.clone()
                    />
                    {
                        let on_undo = on_undo.clone();
                        let on_remove = on_remove.clone();
                        move || {
                            if marked_delete.get() {
                                let on_undo = on_undo.clone();
                                view! {
                                    <button type="button" class="workbench-mini-btn" on:click=on_undo>
                                        <span class="harness-btn-inline">
                                            <LxIcon icon=icondata::LuUndo2 width="0.78rem" height="0.78rem" />
                                            <span>{move || i18n.tr(I18nKey::ApiKeysUndo)()}</span>
                                        </span>
                                    </button>
                                }.into_any()
                            } else if configured {
                                let on_remove = on_remove.clone();
                                view! {
                                    <button type="button" class="workbench-mini-btn" on:click=on_remove>
                                        <span class="harness-btn-inline">
                                            <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
                                            <span>{move || i18n.tr(I18nKey::ApiKeysRemove)()}</span>
                                        </span>
                                    </button>
                                }.into_any()
                            } else {
                                view! { <span></span> }.into_any()
                            }
                        }
                    }
                </div>
                <Show when=move || row_dirty.get() && !marked_delete.get()>
                    <p class="harness-muted api-keys-row__hint">{move || i18n.tr(I18nKey::ApiKeysPending)()}</p>
                </Show>
                <Show when=move || show_env_hint && env_var.with_value(|v| v.is_some())>
                    <p class="harness-muted api-keys-row__envhint">
                        {move || {
                            env_var
                                .with_value(|v| {
                                    v.as_ref().map(|name| {
                                        i18n.tr(I18nKey::ApiKeysEnvFallback)().replace("{var}", name)
                                    })
                                })
                                .flatten()
                                .unwrap_or_default()
                        }}
                    </p>
                </Show>
            </Show>
        </li>
    }
}
