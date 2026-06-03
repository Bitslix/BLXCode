use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::SessionRoleView;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

const DEFAULT_ROLE_DESC: &str = "Default BLXCode Agent without a specialized harness session role.";

#[component]
pub fn SessionRolePicker(
    id: String,
    roles: Signal<Vec<SessionRoleView>>,
    selected: Signal<Option<String>>,
    on_select: Callback<Option<String>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);
    let picker_id = id.clone();

    Effect::new({
        let outside_id = picker_id.clone();
        move |_| {
            if !open.get() {
                return;
            }

            let outside_id_inner = outside_id.clone();
            let h_down = window_event_listener_untyped("mousedown", move |ev| {
                let Some(target) = ev.target() else {
                    return;
                };
                let Ok(node) = target.dyn_into::<web_sys::Node>() else {
                    return;
                };
                let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                    return;
                };
                let Some(wrap) = doc.get_element_by_id(&outside_id_inner) else {
                    return;
                };
                if !wrap.contains(Some(&node)) {
                    open.set(false);
                }
            });
            on_cleanup(move || h_down.remove());
        }
    });

    let enabled_roles = Memo::new(move |_| {
        roles
            .get()
            .into_iter()
            .filter(|role| role.enabled)
            .collect::<Vec<_>>()
    });

    view! {
        <div id=picker_id class="ws-config__role-picker">
            <button
                type="button"
                class="ws-config__role-trigger"
                class:ws-config__role-trigger--open=move || open.get()
                aria-haspopup="listbox"
                aria-expanded=move || open.get().to_string()
                on:click=move |_| open.update(|value| *value = !*value)
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "Escape" => {
                            open.set(false);
                            ev.prevent_default();
                        }
                        "ArrowDown" | "Enter" | " " => {
                            open.set(true);
                            ev.prevent_default();
                        }
                        _ => {}
                    }
                }
            >
                <span class="ws-config__role-trigger-copy">
                    <span class="ws-config__role-title">
                        {move || {
                            let Some(slug) = selected.get() else {
                                return i18n.tr(I18nKey::WzSessionRoleNone)().to_string();
                            };
                            enabled_roles
                                .get()
                                .into_iter()
                                .find(|role| role.slug == slug)
                                .map(|role| role.title)
                                .unwrap_or_else(|| i18n.tr(I18nKey::WzSessionRoleNone)().to_string())
                        }}
                    </span>
                    <span class="ws-config__role-desc">
                        {move || {
                            let Some(slug) = selected.get() else {
                                return DEFAULT_ROLE_DESC.to_string();
                            };
                            enabled_roles
                                .get()
                                .into_iter()
                                .find(|role| role.slug == slug)
                                .map(|role| role.description)
                                .unwrap_or_else(|| DEFAULT_ROLE_DESC.to_string())
                        }}
                    </span>
                </span>
                <LxIcon icon=icondata::LuChevronDown width="0.85rem" height="0.85rem" />
            </button>
            <Show when=move || open.get()>
                <div class="ws-config__role-menu" role="listbox">
                    <button
                        type="button"
                        class=move || {
                            let mut class = String::from("ws-config__role-option");
                            if selected.get().is_none() {
                                class.push_str(" ws-config__role-option--active");
                            }
                            class
                        }
                        role="option"
                        aria-selected=move || selected.get().is_none().to_string()
                        on:click=move |_| {
                            on_select.run(None);
                            open.set(false);
                        }
                    >
                        <span class="ws-config__role-option-main">
                            <span class="ws-config__role-title">
                                {move || i18n.tr(I18nKey::WzSessionRoleNone)()}
                            </span>
                            <span class="ws-config__role-desc">{DEFAULT_ROLE_DESC}</span>
                        </span>
                        <Show when=move || selected.get().is_none()>
                            <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
                        </Show>
                    </button>
                    {move || {
                        enabled_roles
                            .get()
                            .into_iter()
                            .map(|role| {
                                let slug = role.slug.clone();
                                let slug_for_class = slug.clone();
                                let slug_for_aria = slug.clone();
                                let slug_for_click = slug.clone();
                                let title = role.title.clone();
                                let desc = role.description.clone();
                                let mut meta: Vec<String> = Vec::new();
                                if !role.tools.is_empty() {
                                    meta.push(format!(
                                        "{}: {}",
                                        i18n.tr(I18nKey::WzSessionRoleTools)(),
                                        role.tools.join(", ")
                                    ));
                                }
                                if !role.models.is_empty() {
                                    meta.push(format!(
                                        "{}: {}",
                                        i18n.tr(I18nKey::WzAgentModelLabel)(),
                                        role.models.join(", ")
                                    ));
                                }
                                let meta = meta.join("  ·  ");
                                let meta_for_when = meta.clone();
                                let meta_for_view = meta.clone();
                                view! {
                                    <button
                                        type="button"
                                        class=move || {
                                            let mut class = String::from("ws-config__role-option");
                                            if selected.get().as_deref() == Some(slug_for_class.as_str()) {
                                                class.push_str(" ws-config__role-option--active");
                                            }
                                            class
                                        }
                                        role="option"
                                        aria-selected=move || {
                                            (selected.get().as_deref() == Some(slug_for_aria.as_str())).to_string()
                                        }
                                        on:click=move |_| {
                                            on_select.run(Some(slug_for_click.clone()));
                                            open.set(false);
                                        }
                                    >
                                        <span class="ws-config__role-option-main">
                                            <span class="ws-config__role-title">{title.clone()}</span>
                                            <span class="ws-config__role-desc">{desc.clone()}</span>
                                            <Show when=move || !meta_for_when.is_empty()>
                                                <span class="ws-config__role-meta">{meta_for_view.clone()}</span>
                                            </Show>
                                        </span>
                                        <Show when=move || selected.get().as_deref() == Some(slug.as_str())>
                                            <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
                                        </Show>
                                    </button>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </Show>
        </div>
    }
}
