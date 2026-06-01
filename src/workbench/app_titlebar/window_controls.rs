//! Minimize / maximize-restore / close controls for the custom title bar.
//!
//! The privileged window operations live in the Rust backend
//! (`window_controls.rs`); here we only invoke the thin bridge wrappers and
//! keep the maximize/restore icon in sync. We re-query `window_is_maximized`
//! on mount and on every DOM `resize` (maximize, unmaximize, OS snap, and the
//! double-click drag-region toggle all resize the webview), so the icon stays
//! correct without widening JS window permissions to the event API.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, window_close, window_is_maximized, window_minimize, window_toggle_maximize,
};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn WindowControls() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let maximized = RwSignal::new(false);

    let refresh_maximized = move || {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(state) = window_is_maximized().await {
                maximized.set(state);
            }
        });
    };

    // Initial state + keep in sync as the window geometry changes.
    Effect::new(move |_| {
        refresh_maximized();
    });
    let resize_handle = window_event_listener_untyped("resize", move |_| {
        refresh_maximized();
    });
    on_cleanup(move || resize_handle.remove());

    let on_minimize = move |_| {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            let _ = window_minimize().await;
        });
    };

    let on_toggle_maximize = move |_| {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(state) = window_toggle_maximize().await {
                maximized.set(state);
            }
        });
    };

    let on_close = move |_| {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            let _ = window_close().await;
        });
    };

    view! {
        <div class="app-titlebar__window-controls" role="group">
            <button
                type="button"
                class="app-titlebar__win-btn"
                aria-label=move || i18n.tr(I18nKey::TbWinMinimize)()
                title=move || i18n.tr(I18nKey::TbWinMinimize)()
                on:click=on_minimize
            >
                <span class="app-titlebar__win-glyph app-titlebar__win-glyph--min" aria-hidden="true"></span>
            </button>
            <button
                type="button"
                class="app-titlebar__win-btn"
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
                <span class="app-titlebar__win-glyph" aria-hidden="true">
                    {move || {
                        if maximized.get() {
                            view! { <LxIcon icon=icondata::LuCopy width="0.78rem" height="0.78rem" /> }.into_any()
                        } else {
                            view! { <LxIcon icon=icondata::LuSquare width="0.78rem" height="0.78rem" /> }.into_any()
                        }
                    }}
                </span>
            </button>
            <button
                type="button"
                class="app-titlebar__win-btn app-titlebar__win-btn--close"
                aria-label=move || i18n.tr(I18nKey::BtnClose)()
                title=move || i18n.tr(I18nKey::BtnClose)()
                on:click=on_close
            >
                <span class="app-titlebar__win-glyph" aria-hidden="true">
                    <LxIcon icon=icondata::LuX width="0.9rem" height="0.9rem" />
                </span>
            </button>
        </div>
    }
}
