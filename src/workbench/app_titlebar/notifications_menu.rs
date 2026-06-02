//! Notifications bell + popover. v1 renders the empty state; entries are read
//! from the shared [`TitleBarFeed`] store so the agent-done / toast feed can
//! populate it later (follow-up task `titlebar-notifications-feed`) without
//! touching this component.
use super::TitleBarFeed;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[component]
pub fn NotificationsMenu() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let feed = expect_context::<TitleBarFeed>();
    let open = RwSignal::new(false);

    let close_click = window_event_listener_untyped("click", move |ev| {
        if !open.get_untracked() {
            return;
        }
        let inside = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".app-titlebar__menu-wrap").ok().flatten())
            .is_some();
        if !inside {
            open.set(false);
        }
    });
    let close_esc = window_event_listener_untyped("keydown", move |ev| {
        let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if ev.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        close_click.remove();
        close_esc.remove();
    });

    let has_unread = move || feed.items.with(|items| !items.is_empty());

    view! {
        <div class="app-titlebar__menu-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label=move || i18n.tr(I18nKey::TbNotifications)()
                title=move || i18n.tr(I18nKey::TbNotifications)()
                on:click=move |ev| {
                    ev.stop_propagation();
                    open.update(|o| *o = !*o);
                }
            >
                <LxIcon icon=icondata::LuBell width="1rem" height="1rem" />
                <Show when=move || has_unread()>
                    <span class="app-titlebar__bell-dot" aria-hidden="true"></span>
                </Show>
            </button>
            <Show when=move || open.get()>
                <div class="app-titlebar__popover app-titlebar__popover--notifications" role="menu">
                    <p class="app-titlebar__popover-head">{move || i18n.tr(I18nKey::TbNotifications)()}</p>
                    <Show
                        when=move || has_unread()
                        fallback=move || view! {
                            <div class="app-titlebar__notif-empty">
                                <LxIcon icon=icondata::LuBell width="1.4rem" height="1.4rem" />
                                <p class="app-titlebar__notif-empty-title">
                                    {move || i18n.tr(I18nKey::TbNotificationsEmptyTitle)()}
                                </p>
                                <p class="app-titlebar__notif-empty-body">
                                    {move || i18n.tr(I18nKey::TbNotificationsEmptyBody)()}
                                </p>
                            </div>
                        }
                    >
                        <ul class="app-titlebar__notif-list">
                            <For
                                each=move || feed.items.get()
                                key=|item| item.id.clone()
                                children=move |item| {
                                    view! {
                                        <li class="app-titlebar__notif-item">
                                            <span class="app-titlebar__notif-item-title">{item.title}</span>
                                            <span class="app-titlebar__notif-item-body">{item.body}</span>
                                        </li>
                                    }
                                }
                            />
                        </ul>
                    </Show>
                </div>
            </Show>
        </div>
    }
}
