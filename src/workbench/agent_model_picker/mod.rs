//! Shared agent model dropdown (Text + Image columns in BLXCode Agent settings).

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::ProviderModelEntry;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

fn input_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

fn price_per_million(usd_per_token: f64) -> String {
    format!("${:.2}", usd_per_token * 1_000_000.0)
}

fn model_row_meta(i18n: &I18nService, entry: &ProviderModelEntry) -> Option<String> {
    if let Some(p) = entry.pricing {
        Some(
            i18n.tr(I18nKey::AgModelMetaPricing)()
                .replace("{in}", &price_per_million(p.prompt))
                .replace("{out}", &price_per_million(p.completion)),
        )
    } else {
        entry.description.as_ref().and_then(|d| {
            let t = d.trim();
            if t.is_empty() {
                None
            } else {
                Some(if t.len() > 88 {
                    format!("{}…", &t[..88])
                } else {
                    t.to_string()
                })
            }
        })
    }
}

fn find_model_entry(entries: &[ProviderModelEntry], id: &str) -> Option<ProviderModelEntry> {
    entries.iter().find(|e| e.id == id).cloned()
}

fn model_trigger_label(entries: &[ProviderModelEntry], id: &str) -> String {
    find_model_entry(entries, id)
        .map(|e| {
            if e.label.trim().is_empty() {
                e.id
            } else {
                e.label
            }
        })
        .unwrap_or_else(|| id.to_string())
}

fn focus_model_option(prefix: &str, model_id: &str) {
    focus_by_id(&model_option_dom_id(prefix, model_id));
}

fn focus_by_id(id: &str) {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(el) = doc.get_element_by_id(id) else {
        return;
    };
    let Ok(button) = el.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let _ = button.focus();
}

fn model_option_dom_id(prefix: &str, model_id: &str) -> String {
    let slug: String = model_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{prefix}-option-{slug}")
}

#[component]
pub fn AgentModelPicker(
    model_id: RwSignal<String>,
    model_entries: RwSignal<Vec<ProviderModelEntry>>,
    loading_models: RwSignal<bool>,
    #[prop(default = "agent-model")]
    option_id_prefix: &'static str,
    #[prop(optional)] on_change: Option<Callback<String>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);
    let prefix = option_id_prefix;

    let selected_entry =
        Signal::derive(move || find_model_entry(&model_entries.get(), &model_id.get()));

    view! {
        <div class="agent-model-picker">
            <div class="harness-provider-picker">
                <button
                    type="button"
                    class="harness-provider-trigger"
                    aria-haspopup="listbox"
                    aria-expanded=move || if open.get() { "true" } else { "false" }
                    disabled=move || loading_models.get()
                    on:click=move |_| {
                        let next = !open.get_untracked();
                        open.set(next);
                        if next {
                            let id = model_id.get_untracked();
                            let prefix = prefix;
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_model_option(prefix, &id);
                            });
                        }
                    }
                >
                    <span class="harness-provider-trigger__main">
                        <span class="harness-provider-trigger__brand">
                            <LxIcon icon=icondata::LuPackage2 width="0.76rem" height="0.76rem" />
                        </span>
                        <span class="agent-model-picker__trigger-label">
                            {move || model_trigger_label(&model_entries.get(), &model_id.get())}
                        </span>
                    </span>
                    <span class="harness-provider-trigger__caret">"▾"</span>
                </button>

                <Show when=move || open.get()>
                    <div class="harness-provider-menu agent-model-picker__menu" role="listbox">
                        {move || {
                            let entries = model_entries.get();
                            if entries.is_empty() {
                                view! {
                                    <p class="harness-muted" style="padding:0.45rem 0.5rem;margin:0;">
                                        {move || i18n.tr(I18nKey::AgModelsUnavailable)()}
                                    </p>
                                }
                                .into_any()
                            } else {
                                entries
                                    .into_iter()
                                    .map(|entry| {
                                        let entry_id = entry.id.clone();
                                        let id_active = entry_id.clone();
                                        let id_aria = entry_id.clone();
                                        let id_click = entry_id.clone();
                                        let id_key = entry_id.clone();
                                        let meta = model_row_meta(&i18n, &entry);
                                        let title = if entry.label.trim().is_empty() {
                                            entry.id.clone()
                                        } else {
                                            entry.label.clone()
                                        };
                                        let dom_id = model_option_dom_id(prefix, &entry_id);
                                        view! {
                                            <button
                                                id=dom_id
                                                type="button"
                                                role="option"
                                                class="harness-provider-option agent-model-option"
                                                class:harness-provider-option--active=move || model_id.get() == id_active
                                                aria-selected=move || if model_id.get() == id_aria {
                                                    "true"
                                                } else {
                                                    "false"
                                                }
                                                on:click=move |_| {
                                                    let id = id_click.clone();
                                                    model_id.set(id.clone());
                                                    if let Some(cb) = on_change.as_ref() {
                                                        cb.run(id);
                                                    }
                                                    open.set(false);
                                                }
                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                    match ev.key().as_str() {
                                                        "Enter" | " " => {
                                                            ev.prevent_default();
                                                            let id = id_key.clone();
                                                            model_id.set(id.clone());
                                                            if let Some(cb) = on_change.as_ref() {
                                                                cb.run(id);
                                                            }
                                                            open.set(false);
                                                        }
                                                        "Escape" => {
                                                            ev.prevent_default();
                                                            open.set(false);
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            >
                                                <span class="agent-model-option__title">{title}</span>
                                                {meta.map(|line| view! {
                                                    <span class="agent-model-option__meta">{line}</span>
                                                })}
                                            </button>
                                        }
                                        .into_any()
                                    })
                                    .collect_view()
                                    .into_any()
                            }
                        }}
                    </div>
                </Show>
            </div>

            <input
                class="workbench-plain-input"
                type="text"
                placeholder=move || i18n.tr(I18nKey::AgModelCustomField)()
                prop:value=move || model_id.get()
                on:input=move |ev| {
                    if let Some(value) = input_str(&ev) {
                        model_id.set(value.clone());
                        if let Some(cb) = on_change.as_ref() {
                            cb.run(value);
                        }
                    }
                }
            />

            {move || {
                let Some(entry) = selected_entry.get() else {
                    return ().into_any();
                };
                let mut lines = Vec::new();
                if entry.id != entry.label && !entry.label.trim().is_empty() {
                    lines.push(entry.id.clone());
                }
                if let Some(desc) = entry.description.as_ref().filter(|d| !d.trim().is_empty()) {
                    lines.push(desc.trim().to_string());
                }
                if let Some(p) = entry.pricing {
                    lines.push(
                        i18n.tr(I18nKey::AgModelMetaPricing)()
                            .replace("{in}", &price_per_million(p.prompt))
                            .replace("{out}", &price_per_million(p.completion)),
                    );
                }
                if lines.is_empty() {
                    return ().into_any();
                }
                let title = if entry.label.trim().is_empty() {
                    entry.id.clone()
                } else {
                    entry.label.clone()
                };
                view! {
                    <div class="agent-model-picker__detail harness-muted" aria-live="polite">
                        <span class="agent-model-picker__detail-title">{title}</span>
                        {lines.into_iter().map(|line| view! {
                            <span class="agent-model-picker__detail-line">{line}</span>
                        }).collect_view()}
                    </div>
                }
                .into_any()
            }}
        </div>
    }
}
