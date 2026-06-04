//! One reusable dropdown primitive ([`OptionPicker`]) shared by every
//! provider/level selector in the Agent settings pane. Sections map their enum
//! variants to [`PickerOption`]s and react via the `on_select` id callback —
//! replacing the five near-identical bespoke pickers the old pane carried
//! (rule-reusable-components).

use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

/// The leading glyph for an option: a brand SVG URL or a Lucide icon.
#[derive(Clone, PartialEq)]
pub(crate) enum PickerIcon {
    Brand(&'static str),
    Lucide(icondata::Icon),
}

#[derive(Clone, PartialEq)]
pub(crate) struct PickerOption {
    pub id: String,
    pub label: String,
    pub icon: PickerIcon,
}

impl PickerOption {
    pub fn brand(id: impl Into<String>, label: impl Into<String>, url: &'static str) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: PickerIcon::Brand(url),
        }
    }
    pub fn lucide(id: impl Into<String>, label: impl Into<String>, icon: icondata::Icon) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: PickerIcon::Lucide(icon),
        }
    }
}

fn dom_id(prefix: &str, id: &str) -> String {
    let slug: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("agent-picker-{prefix}-{slug}")
}

fn focus_dom(id: &str) {
    let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(id))
    else {
        return;
    };
    if let Ok(html) = el.dyn_into::<web_sys::HtmlElement>() {
        let _ = html.focus();
    }
}

#[component]
fn OptionGlyph(icon: PickerIcon) -> impl IntoView {
    match icon {
        PickerIcon::Brand(url) => view! {
            <span class="harness-provider-option__brand">
                <img class="harness-provider-option__img" src=url alt="" />
            </span>
        }
        .into_any(),
        PickerIcon::Lucide(ic) => view! {
            <span class="harness-provider-option__brand">
                <LxIcon icon=ic width="0.78rem" height="0.78rem" />
            </span>
        }
        .into_any(),
    }
}

/// Accessible single-select dropdown. `prefix` must be unique per picker
/// instance so option DOM ids don't collide on a page with several pickers.
#[component]
pub(crate) fn OptionPicker(
    prefix: &'static str,
    options: Signal<Vec<PickerOption>>,
    selected: Signal<String>,
    on_select: Callback<String>,
    #[prop(default = false)] level_style: bool,
) -> impl IntoView {
    let open = RwSignal::new(false);

    let focus_selected = move || {
        let sel = selected.get_untracked();
        let prefix = prefix;
        leptos::task::spawn_local(async move {
            TimeoutFuture::new(0).await;
            focus_dom(&dom_id(prefix, &sel));
        });
    };

    let neighbor = move |delta: i32| -> Option<String> {
        let opts = options.get_untracked();
        if opts.is_empty() {
            return None;
        }
        let cur = selected.get_untracked();
        let idx = opts.iter().position(|o| o.id == cur).unwrap_or(0) as i32;
        let len = opts.len() as i32;
        let next = ((idx + delta) % len + len) % len;
        Some(opts[next as usize].id.clone())
    };

    let choose = move |id: String| {
        on_select.run(id);
        open.set(false);
    };

    view! {
        <div class="harness-provider-picker" class:harness-level-picker=level_style>
            <button
                type="button"
                class="harness-provider-trigger"
                aria-haspopup="listbox"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |_| {
                    let next = !open.get_untracked();
                    open.set(next);
                    if next { focus_selected(); }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " => {
                            ev.prevent_default();
                            open.set(true);
                            focus_selected();
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            if let Some(id) = neighbor(-1) {
                                let prefix = prefix;
                                leptos::task::spawn_local(async move {
                                    TimeoutFuture::new(0).await;
                                    focus_dom(&dom_id(prefix, &id));
                                });
                            }
                        }
                        "Escape" => open.set(false),
                        _ => {}
                    }
                }
            >
                <span class="harness-provider-trigger__main">
                    {move || {
                        let sel = selected.get();
                        options.get().into_iter().find(|o| o.id == sel).map(|o| view! {
                            <OptionGlyph icon=o.icon.clone() />
                            <span>{o.label.clone()}</span>
                        })
                    }}
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        options.get().into_iter().map(|o| {
                            let id_click = o.id.clone();
                            let id_self = o.id.clone();
                            let id_active = o.id.clone();
                            let id_aria = o.id.clone();
                            view! {
                                <button
                                    id=dom_id(prefix, &o.id)
                                    type="button"
                                    role="option"
                                    class="harness-provider-option"
                                    class:harness-provider-option--active=move || selected.get() == id_active
                                    aria-selected=move || if selected.get() == id_aria { "true" } else { "false" }
                                    on:click=move |_| choose(id_click.clone())
                                    on:keydown={
                                        let id_enter = id_self.clone();
                                        move |ev: web_sys::KeyboardEvent| {
                                            match ev.key().as_str() {
                                                "ArrowDown" => {
                                                    ev.prevent_default();
                                                    if let Some(id) = neighbor(1) { focus_dom(&dom_id(prefix, &id)); }
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    if let Some(id) = neighbor(-1) { focus_dom(&dom_id(prefix, &id)); }
                                                }
                                                "Enter" | " " => {
                                                    ev.prevent_default();
                                                    choose(id_enter.clone());
                                                }
                                                "Escape" => {
                                                    ev.prevent_default();
                                                    open.set(false);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                >
                                    <OptionGlyph icon=o.icon.clone() />
                                    <span>{o.label.clone()}</span>
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>
            </Show>
        </div>
    }
}
