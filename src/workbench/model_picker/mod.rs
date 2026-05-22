//! Shared model picker (datalist input + refresh button).
//!
//! Originally inlined in `harness_voice_pane`; lifted here so the image
//! settings pane (and any future provider-bound picker) can reuse the same
//! UX without duplicating logic.
//!
//! Component contract:
//! - `label_key`  – i18n key for the field label.
//! - `datalist_id` – DOM id for the inline `<datalist>` (must be unique per
//!   picker on the page; both panes set their own id).
//! - `current`    – the currently-persisted model id (displayed in the input).
//! - `models`     – reactive list of suggestions.
//! - `on_change`  – fired on every keystroke; caller persists.
//! - `on_refresh` – fired when the user clicks the refresh button.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::ProviderModelEntry;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[allow(dead_code)]
#[component]
pub fn ModelPicker<F, R>(
    label_key: I18nKey,
    datalist_id: &'static str,
    current: String,
    models: RwSignal<Vec<ProviderModelEntry>>,
    on_change: F,
    on_refresh: R,
) -> impl IntoView
where
    F: Fn(String) + Send + Sync + 'static + Clone,
    R: Fn() + Send + Sync + 'static + Copy,
{
    let i18n = expect_context::<I18nService>();
    let buf = RwSignal::new(current.clone());
    let loading = RwSignal::new(false);

    // Keep the buffer aligned with the persisted value when settings reload.
    Effect::new({
        let current = current.clone();
        move |_| {
            let _ = models.get();
            buf.set(current.clone());
        }
    });

    let on_input = {
        let on_change = on_change.clone();
        move |ev: web_sys::Event| {
            if let Some(t) = ev.target() {
                if let Ok(inp) = t.dyn_into::<web_sys::HtmlInputElement>() {
                    let v = inp.value();
                    buf.set(v.clone());
                    on_change(v);
                }
            }
        }
    };

    let on_refresh_click = move |_| {
        if loading.get_untracked() {
            return;
        }
        loading.set(true);
        on_refresh();
    };

    // Reset the loading flag whenever the model list arrives.
    Effect::new(move |_| {
        let _ = models.get();
        loading.set(false);
    });

    view! {
        <div class="model-picker">
            <label class="model-picker__label">{move || i18n.tr(label_key)()}</label>
            <input
                class="model-picker__input workbench-plain-input"
                type="text"
                list=datalist_id
                prop:value=move || buf.get()
                on:input=on_input
            />
            <datalist id=datalist_id>
                {move || {
                    models.get()
                        .into_iter()
                        .map(|m| view! { <option value=m.id.clone()></option> })
                        .collect_view()
                }}
            </datalist>
            <div class="model-picker__row">
                <button
                    type="button"
                    class="model-picker__refresh workbench-mini-btn"
                    prop:disabled=move || loading.get()
                    on:click=on_refresh_click
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
                <small class="model-picker__count harness-muted">
                    {move || format!("{} entries", models.get().len())}
                </small>
            </div>
        </div>
    }
}
