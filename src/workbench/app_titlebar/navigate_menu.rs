//! NAVIGATE quick popover: jump between views, spin up a terminal in the
//! active workspace, or toggle fullscreen. Closes on outside-click and `Esc`.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, window_toggle_fullscreen};
use crate::workbench::state::{HarnessSettingsCategory, RightPanelTab};
use crate::workbench::WorkbenchService;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[component]
pub fn NavigateMenu() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    // Close on outside-click / Esc while the popover is open.
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

    let has_workspace = move || wb.active_id().get().is_some();

    let go_tab = move |tab: RightPanelTab| {
        wb.set_right_tab(tab);
        if wb.right_collapsed().get_untracked() {
            wb.toggle_right_panel();
        }
        open.set(false);
    };

    let open_terminals = move |_| {
        if let Some(id) = wb.active_id().get_untracked() {
            wb.open_center_terminals_tab(id);
        }
        open.set(false);
    };
    let new_terminal = move |_| {
        if let Some(id) = wb.active_id().get_untracked() {
            let _ = wb.append_terminal_slot(id, None);
            wb.open_center_terminals_tab(id);
        }
        open.set(false);
    };
    let open_settings = move |_| {
        wb.open_center_settings_tab(HarnessSettingsCategory::App);
        open.set(false);
    };
    let toggle_fullscreen = move |_| {
        open.set(false);
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            let _ = window_toggle_fullscreen().await;
        });
    };

    view! {
        <div class="app-titlebar__menu-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label=move || i18n.tr(I18nKey::TbNavigate)()
                title=move || i18n.tr(I18nKey::TbNavigate)()
                on:click=move |ev| {
                    ev.stop_propagation();
                    open.update(|o| *o = !*o);
                }
            >
                <LxIcon icon=icondata::LuLayoutGrid width="1rem" height="1rem" />
            </button>
            <Show when=move || open.get()>
                <div class="app-titlebar__popover app-titlebar__popover--navigate" role="menu">
                    <p class="app-titlebar__popover-head">{move || i18n.tr(I18nKey::TbNavigate)()}</p>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        disabled=move || !has_workspace()
                        on:click=open_terminals
                    >
                        <LxIcon icon=icondata::LuTerminal width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::WsKwTerminal)()}</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        disabled=move || !has_workspace()
                        on:click=new_terminal
                    >
                        <LxIcon icon=icondata::LuPlus width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::TbNavNewTerminal)()}</span>
                    </button>
                    <div class="app-titlebar__menu-sep" role="separator"></div>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=move |_| go_tab(RightPanelTab::Plans)
                    >
                        <LxIcon icon=icondata::LuClipboardList width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::TabPlans)()}</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=move |_| go_tab(RightPanelTab::Memory)
                    >
                        <LxIcon icon=icondata::LuLayers width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::TabMemory)()}</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=move |_| go_tab(RightPanelTab::Skills)
                    >
                        <LxIcon icon=icondata::LuPuzzle width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::TabSkills)()}</span>
                    </button>
                    <div class="app-titlebar__menu-sep" role="separator"></div>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=open_settings
                    >
                        <LxIcon icon=icondata::LuSettings width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::CmdSetTitle)()}</span>
                    </button>
                    <button
                        type="button"
                        class="app-titlebar__menu-item"
                        role="menuitem"
                        on:click=toggle_fullscreen
                    >
                        <LxIcon icon=icondata::LuMaximize2 width="0.95rem" height="0.95rem" />
                        <span class="app-titlebar__menu-item-label">{move || i18n.tr(I18nKey::TbFullscreenEnter)()}</span>
                    </button>
                </div>
            </Show>
        </div>
    }
}
