use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, window_current_close, window_current_is_maximized, window_current_minimize,
    window_current_toggle_maximize, PopoutPayload,
};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn PopoutShell(payload: PopoutPayload) -> impl IntoView {
    let title = payload.fallback_title();
    view! {
        <div class="workbench-popout-shell">
            <PopoutTitleBar title=title.clone() />
            <main class="workbench-popout-shell__body">
                <div class="workbench-popout-shell__placeholder">
                    <span class="workbench-popout-shell__eyebrow">"BLXCode Popout"</span>
                    <h1>{title}</h1>
                </div>
            </main>
        </div>
    }
}

#[component]
fn PopoutTitleBar(title: String) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let maximized = RwSignal::new(false);

    let refresh_maximized = move || {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(state) = window_current_is_maximized().await {
                maximized.set(state);
            }
        });
    };

    Effect::new(move |_| refresh_maximized());
    let resize_handle = window_event_listener_untyped("resize", move |_| refresh_maximized());
    on_cleanup(move || resize_handle.remove());

    let on_minimize = move |_| {
        if is_tauri_shell() {
            spawn_local(async move {
                let _ = window_current_minimize().await;
            });
        }
    };
    let on_toggle_maximize = move |_| {
        if is_tauri_shell() {
            spawn_local(async move {
                if let Ok(state) = window_current_toggle_maximize().await {
                    maximized.set(state);
                }
            });
        }
    };
    let on_close = move |_| {
        if is_tauri_shell() {
            spawn_local(async move {
                let _ = window_current_close().await;
            });
        }
    };

    view! {
        <header class="workbench-popout-titlebar" data-tauri-drag-region="">
            <div class="workbench-popout-titlebar__brand" data-tauri-drag-region="">
                <img
                    class="workbench-popout-titlebar__logo"
                    src="/blxcode.png"
                    alt=""
                    draggable="false"
                />
                <span class="workbench-popout-titlebar__app">"BLXCode"</span>
                <span class="workbench-popout-titlebar__sep" aria-hidden="true"></span>
                <span class="workbench-popout-titlebar__title" title=title.clone()>{title.clone()}</span>
            </div>
            <div class="workbench-popout-titlebar__controls" role="group">
                <button
                    type="button"
                    class="workbench-popout-titlebar__btn"
                    aria-label=move || i18n.tr(I18nKey::TbWinMinimize)()
                    title=move || i18n.tr(I18nKey::TbWinMinimize)()
                    on:click=on_minimize
                >
                    <span class="workbench-popout-titlebar__min" aria-hidden="true"></span>
                </button>
                <button
                    type="button"
                    class="workbench-popout-titlebar__btn"
                    aria-label=move || {
                        if maximized.get() {
                            i18n.tr(I18nKey::TbWinRestore)()
                        } else {
                            i18n.tr(I18nKey::TbWinMaximize)()
                        }
                    }
                    title=move || {
                        if maximized.get() {
                            i18n.tr(I18nKey::TbWinRestore)()
                        } else {
                            i18n.tr(I18nKey::TbWinMaximize)()
                        }
                    }
                    on:click=on_toggle_maximize
                >
                    {move || {
                        if maximized.get() {
                            view! { <LxIcon icon=icondata::LuCopy width="0.72rem" height="0.72rem" /> }.into_any()
                        } else {
                            view! { <LxIcon icon=icondata::LuSquare width="0.72rem" height="0.72rem" /> }.into_any()
                        }
                    }}
                </button>
                <button
                    type="button"
                    class="workbench-popout-titlebar__btn workbench-popout-titlebar__btn--close"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    title=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=on_close
                >
                    <LxIcon icon=icondata::LuX width="0.84rem" height="0.84rem" />
                </button>
            </div>
        </header>
    }
}
