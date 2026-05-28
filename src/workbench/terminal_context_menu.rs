//! Right-click context menu for workspace xterm terminals.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{clipboard_read_text_compat, clipboard_write_text_compat};
use crate::workbench::terminal_glue::{
    terminal_clear_selection, terminal_focus, terminal_get_selection, terminal_paste,
    terminal_select_all,
};
use crate::workbench::toast::ToastService;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug)]
pub enum TerminalMenuAction {
    Copy,
    Paste,
    SelectAll,
}

#[derive(Clone, Debug)]
pub struct TerminalContextMenuState {
    pub term_id: f64,
    pub anchor_x: i32,
    pub anchor_y: i32,
    pub has_selection: bool,
}

#[component]
pub fn TerminalContextMenu(
    state: RwSignal<Option<TerminalContextMenuState>>,
    on_action: Callback<TerminalMenuAction>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <Show when=move || state.get().is_some()>
            {move || {
                let Some(s) = state.get() else {
                    return view! {}.into_any();
                };
                let style = format!("left: {}px; top: {}px;", s.anchor_x, s.anchor_y);
                let has_selection = s.has_selection;
                view! {
                    <div
                        class="terminal-context-menu"
                        role="menu"
                        aria-label=move || i18n.tr(I18nKey::WsTermMenuAria)()
                        style=style
                        on:mousedown=|ev| ev.stop_propagation()
                        on:click=|ev| ev.stop_propagation()
                        on:contextmenu=|ev| ev.prevent_default()
                    >
                        <button
                            type="button"
                            class="terminal-context-menu__item"
                            role="menuitem"
                            disabled=move || !has_selection
                            on:click=move |_| on_action.run(TerminalMenuAction::Copy)
                        >
                            {move || i18n.tr(I18nKey::WsTermMenuCopy)()}
                        </button>
                        <button
                            type="button"
                            class="terminal-context-menu__item"
                            role="menuitem"
                            on:click=move |_| on_action.run(TerminalMenuAction::Paste)
                        >
                            {move || i18n.tr(I18nKey::WsTermMenuPaste)()}
                        </button>
                        <button
                            type="button"
                            class="terminal-context-menu__item"
                            role="menuitem"
                            on:click=move |_| on_action.run(TerminalMenuAction::SelectAll)
                        >
                            {move || i18n.tr(I18nKey::WsTermMenuSelectAll)()}
                        </button>
                    </div>
                }
                .into_any()
            }}
        </Show>
    }
}

pub fn paste_terminal_from_clipboard(term_id: f64, i18n: I18nService, toast: ToastService) {
    leptos::task::spawn_local(async move {
        match clipboard_read_text_compat().await {
            Ok(text) if !text.is_empty() => {
                terminal_focus(term_id);
                terminal_paste(term_id, &text);
            }
            Ok(_) => {}
            Err(err) => {
                let msg = i18n.tr(I18nKey::WsTermToastPasteFailed)().replace("{error}", &err);
                toast.error(msg);
            }
        }
    });
}

pub fn dispatch_terminal_menu_action(
    action: TerminalMenuAction,
    term_id: f64,
    i18n: I18nService,
    toast: ToastService,
) {
    match action {
        TerminalMenuAction::Copy => {
            let text = terminal_get_selection(term_id);
            if text.is_empty() {
                return;
            }
            leptos::task::spawn_local(async move {
                match clipboard_write_text_compat(text.clone()).await {
                    Ok(()) => {
                        terminal_clear_selection(term_id);
                        toast.success(i18n.tr(I18nKey::WsTermToastCopied)());
                    }
                    Err(err) => {
                        let msg =
                            i18n.tr(I18nKey::WsTermToastCopyFailed)().replace("{error}", &err);
                        toast.error(msg);
                    }
                }
            });
        }
        TerminalMenuAction::Paste => paste_terminal_from_clipboard(term_id, i18n, toast),
        TerminalMenuAction::SelectAll => {
            terminal_focus(term_id);
            terminal_select_all(term_id);
        }
    }
}

pub fn copy_terminal_selection(
    term_id: f64,
    selection: Option<String>,
    i18n: I18nService,
    toast: ToastService,
) {
    let text = selection.unwrap_or_else(|| terminal_get_selection(term_id));
    if text.is_empty() {
        return;
    }
    leptos::task::spawn_local(async move {
        match clipboard_write_text_compat(text.clone()).await {
            Ok(()) => {
                terminal_clear_selection(term_id);
                toast.success(i18n.tr(I18nKey::WsTermToastCopied)());
            }
            Err(err) => {
                let msg = i18n.tr(I18nKey::WsTermToastCopyFailed)().replace("{error}", &err);
                toast.error(msg);
            }
        }
    });
}

fn detail_f64(detail: &wasm_bindgen::JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(detail, &wasm_bindgen::JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_f64())
}

fn detail_i32(detail: &wasm_bindgen::JsValue, key: &str) -> Option<i32> {
    detail_f64(detail, key).map(|v| v as i32)
}

fn detail_bool(detail: &wasm_bindgen::JsValue, key: &str) -> bool {
    js_sys::Reflect::get(detail, &wasm_bindgen::JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn detail_string(detail: &wasm_bindgen::JsValue, key: &str) -> Option<String> {
    js_sys::Reflect::get(detail, &wasm_bindgen::JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

pub fn install_terminal_clipboard_listeners(
    menu_state: RwSignal<Option<TerminalContextMenuState>>,
    i18n: I18nService,
    toast: ToastService,
) -> impl FnOnce() + 'static {
    let contextmenu_handle = leptos::leptos_dom::helpers::window_event_listener_untyped(
        "blxcode-terminal-contextmenu",
        {
            move |ev| {
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let Some(term_id) = detail_f64(&detail, "termId") else {
                    return;
                };
                let Some(anchor_x) = detail_i32(&detail, "clientX") else {
                    return;
                };
                let Some(anchor_y) = detail_i32(&detail, "clientY") else {
                    return;
                };
                let has_selection = detail_bool(&detail, "hasSelection")
                    || detail_string(&detail, "selection").is_some_and(|s| !s.is_empty());
                menu_state.set(Some(TerminalContextMenuState {
                    term_id,
                    anchor_x,
                    anchor_y,
                    has_selection,
                }));
            }
        },
    );

    let paste_handle = leptos::leptos_dom::helpers::window_event_listener_untyped(
        "blxcode-terminal-paste-request",
        {
            let i18n = i18n.clone();
            let toast = toast.clone();
            move |ev| {
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let Some(term_id) = detail_f64(&detail, "termId") else {
                    return;
                };
                paste_terminal_from_clipboard(term_id, i18n.clone(), toast.clone());
            }
        },
    );

    let copy_handle = leptos::leptos_dom::helpers::window_event_listener_untyped(
        "blxcode-terminal-copy-request",
        {
            let i18n = i18n.clone();
            let toast = toast.clone();
            move |ev| {
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let Some(term_id) = detail_f64(&detail, "termId") else {
                    return;
                };
                let selection = detail_string(&detail, "selection");
                copy_terminal_selection(term_id, selection, i18n.clone(), toast.clone());
            }
        },
    );

    let click_close_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("mousedown", {
            let menu_state = menu_state;
            move |_| {
                if menu_state.get_untracked().is_some() {
                    menu_state.set(None);
                }
            }
        });

    let escape_handle = leptos::leptos_dom::helpers::window_event_listener_untyped("keydown", {
        let menu_state = menu_state;
        move |ev| {
            let Some(kev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            if kev.key() == "Escape" && menu_state.get_untracked().is_some() {
                menu_state.set(None);
            }
        }
    });

    move || {
        drop(contextmenu_handle);
        drop(paste_handle);
        drop(copy_handle);
        drop(click_close_handle);
        drop(escape_handle);
    }
}
