//! Interactive question card rendered when the agent invokes the
//! `harness.ask_user` client-tool. Submits the user's choice back via
//! `agent_submit_tool_result` and updates the matching `TimelineItem::AskUser`
//! row state so the bubble stays in the chat with disabled controls.

use crate::i18n::{lookup, I18nKey};
use crate::service::I18nService;
use crate::tauri_bridge::agent_submit_tool_result;
use crate::workbench::agent_panel::timeline::TimelineItem;
use crate::workbench::agent_timeline::{AskUserOption, AskUserState};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use serde_json::json;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

#[component]
pub fn AskUserCard(
    call_id: String,
    question: String,
    header: Option<String>,
    options: Vec<AskUserOption>,
    multi_select: bool,
    allow_other: bool,
    state: AskUserState,
    timeline: RwSignal<Vec<TimelineItem>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let loc = i18n.locale().get_untracked();

    let selected = RwSignal::new(HashSet::<String>::new());
    let other_text = RwSignal::new(String::new());
    let is_open = matches!(state, AskUserState::Open);

    let render_state = state.clone();

    let pick_single = {
        let call_id = call_id.clone();
        move |label: String| {
            let cid = call_id.clone();
            let payload = json!({
                "selected": [label.clone()],
                "other": "",
                "cancelled": false,
            });
            mark_answered(timeline, &cid, vec![label.clone()], None);
            submit_async(cid, true, "user answered".into(), Some(payload));
        }
    };

    let toggle_multi = move |label: String| {
        selected.update(|s| {
            if s.contains(&label) {
                s.remove(&label);
            } else {
                s.insert(label);
            }
        });
    };

    let submit_multi = {
        let call_id = call_id.clone();
        move || {
            let picked: Vec<String> = selected.get().into_iter().collect();
            let other = {
                let s = other_text.get();
                let s = s.trim().to_string();
                (!s.is_empty()).then_some(s)
            };
            if picked.is_empty() && other.is_none() {
                return;
            }
            let cid = call_id.clone();
            let payload = json!({
                "selected": picked.clone(),
                "other": other.clone().unwrap_or_default(),
                "cancelled": false,
            });
            mark_answered(timeline, &cid, picked, other);
            submit_async(cid, true, "user answered".into(), Some(payload));
        }
    };

    let cancel = {
        let call_id = call_id.clone();
        move || {
            let cid = call_id.clone();
            mark_cancelled(timeline, &cid);
            submit_async(
                cid,
                false,
                "user dismissed".into(),
                Some(json!({ "cancelled": true })),
            );
        }
    };

    let answered_summary = match &render_state {
        AskUserState::Answered { selected, other } => {
            let mut parts = selected.clone();
            if let Some(o) = other.as_ref().filter(|s| !s.trim().is_empty()) {
                parts.push(format!("\"{}\"", o.trim()));
            }
            Some(parts.join(", "))
        }
        _ => None,
    };

    let cancelled = matches!(render_state, AskUserState::Cancelled);
    let answered = matches!(render_state, AskUserState::Answered { .. });
    let header_text = header.clone();
    let header_present = header.is_some();
    let show_send = is_open && (multi_select || allow_other);

    view! {
        <div class="ask-user-card" data-open=move || if is_open { "true" } else { "false" }>
            <div class="ask-user-card__head">
                <Show when=move || header_present>
                    <span class="ask-user-card__chip">{header_text.clone().unwrap_or_default()}</span>
                </Show>
                <span class="ask-user-card__title">{lookup(loc, I18nKey::AgAskUserTitle)}</span>
                <Show when=move || is_open>
                    <button
                        type="button"
                        class="ask-user-card__cancel"
                        title=lookup(loc, I18nKey::AgAskUserCancel)
                        aria-label=lookup(loc, I18nKey::AgAskUserCancel)
                        on:click={
                            let cancel = cancel.clone();
                            move |_| cancel()
                        }
                    >
                        <LxIcon icon=icondata::LuX width="0.78rem" height="0.78rem" />
                    </button>
                </Show>
            </div>

            <p class="ask-user-card__question">{question}</p>

            <p class="ask-user-card__hint">{
                if multi_select {
                    lookup(loc, I18nKey::AgAskUserChooseMultiple)
                } else {
                    lookup(loc, I18nKey::AgAskUserChooseOne)
                }
            }</p>

            <ul class="ask-user-card__options">
                {options.iter().enumerate().map(|(i, opt)| {
                    let label = opt.label.clone();
                    let label_for_pick = label.clone();
                    let label_for_toggle = label.clone();
                    let description = opt.description.clone();
                    let desc_for_show = description.clone();
                    let desc_present = description.is_some();
                    let number = i + 1;
                    let pick_single = pick_single.clone();
                    let toggle_multi = toggle_multi.clone();
                    let label_check = label.clone();
                    let is_checked = move || selected.with(|s| s.contains(&label_check));
                    let is_checked_for_class = is_checked.clone();
                    view! {
                        <li class="ask-user-card__option">
                            <button
                                type="button"
                                class="ask-user-card__option-btn"
                                class:is-selected=is_checked_for_class
                                disabled=!is_open
                                on:click=move |_| {
                                    if multi_select {
                                        toggle_multi(label_for_toggle.clone());
                                    } else {
                                        pick_single(label_for_pick.clone());
                                    }
                                }
                            >
                                <span class="ask-user-card__option-num">{number}</span>
                                <span class="ask-user-card__option-body">
                                    <span class="ask-user-card__option-label">{label}</span>
                                    <Show when=move || desc_present>
                                        <span class="ask-user-card__option-desc">
                                            {desc_for_show.clone().unwrap_or_default()}
                                        </span>
                                    </Show>
                                </span>
                                <Show when=move || multi_select>
                                    <span class="ask-user-card__check" aria-hidden="true">
                                        {
                                            let is_checked = is_checked.clone();
                                            move || if is_checked() { "✓" } else { "" }
                                        }
                                    </span>
                                </Show>
                            </button>
                        </li>
                    }
                }).collect_view()}
            </ul>

            <Show when=move || allow_other && is_open>
                <input
                    type="text"
                    class="ask-user-card__other"
                    placeholder=lookup(loc, I18nKey::AgAskUserOtherPlaceholder)
                    prop:value=move || other_text.get()
                    on:input=move |ev| {
                        if let Some(input) = ev.target()
                            .and_then(|t| t.dyn_into::<HtmlInputElement>().ok())
                        {
                            other_text.set(input.value());
                        }
                    }
                    disabled=!is_open
                />
            </Show>

            <Show when=move || show_send>
                <div class="ask-user-card__actions">
                    <button
                        type="button"
                        class="ask-user-card__send"
                        on:click={
                            let submit_multi = submit_multi.clone();
                            move |_| submit_multi()
                        }
                    >
                        {lookup(loc, I18nKey::AgAskUserSend)}
                    </button>
                </div>
            </Show>

            <Show when=move || answered>
                {
                    let summary = answered_summary.clone();
                    let has_summary = summary.as_ref().is_some_and(|s| !s.is_empty());
                    view! {
                        <p class="ask-user-card__status ask-user-card__status--answered">
                            <span>{lookup(loc, I18nKey::AgAskUserAnswered)}</span>
                            <Show when=move || has_summary>
                                <span class="ask-user-card__status-summary">
                                    {format!(" — {}", summary.clone().unwrap_or_default())}
                                </span>
                            </Show>
                        </p>
                    }
                }
            </Show>

            <Show when=move || cancelled>
                <p class="ask-user-card__status ask-user-card__status--cancelled">
                    {lookup(loc, I18nKey::AgAskUserCancelled)}
                </p>
            </Show>
        </div>
    }
}

fn mark_answered(
    timeline: RwSignal<Vec<TimelineItem>>,
    call_id: &str,
    selected: Vec<String>,
    other: Option<String>,
) {
    timeline.update(|rows| {
        for row in rows.iter_mut() {
            if let TimelineItem::AskUser {
                call_id: cid,
                state,
                ..
            } = row
            {
                if cid == call_id {
                    *state = AskUserState::Answered {
                        selected: selected.clone(),
                        other: other.clone(),
                    };
                    break;
                }
            }
        }
    });
}

fn mark_cancelled(timeline: RwSignal<Vec<TimelineItem>>, call_id: &str) {
    timeline.update(|rows| {
        for row in rows.iter_mut() {
            if let TimelineItem::AskUser {
                call_id: cid,
                state,
                ..
            } = row
            {
                if cid == call_id {
                    *state = AskUserState::Cancelled;
                    break;
                }
            }
        }
    });
}

fn submit_async(call_id: String, ok: bool, message: String, data: Option<serde_json::Value>) {
    leptos::task::spawn_local(async move {
        let _ = agent_submit_tool_result(call_id, ok, Some(message), data).await;
    });
}
