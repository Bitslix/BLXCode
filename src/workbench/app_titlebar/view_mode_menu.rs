use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::state::WorkspaceViewMode;
use crate::workbench::WorkbenchService;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[component]
pub fn ViewModeMenu() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let close_click = window_event_listener_untyped("click", move |ev| {
        if !open.get_untracked() {
            return;
        }
        let inside = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".app-titlebar__view-mode-wrap").ok().flatten())
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

    let active_mode = Signal::derive(move || {
        wb.active_workspace_view_mode()
            .unwrap_or(WorkspaceViewMode::Grid)
    });
    let choose = move |mode: WorkspaceViewMode| {
        if let Some(id) = wb.active_id().get_untracked() {
            match mode {
                WorkspaceViewMode::Grid => wb.open_center_terminals_tab(id),
                WorkspaceViewMode::Canvas => wb.open_center_canvas_tab(id),
                WorkspaceViewMode::Swarm => wb.open_center_swarm_tab(id),
            }
        }
        open.set(false);
    };

    view! {
        <div class="app-titlebar__menu-wrap app-titlebar__view-mode-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label=move || i18n.tr(I18nKey::TbViewMode)()
                title=move || i18n.tr(I18nKey::TbViewMode)()
                on:click=move |ev| {
                    ev.stop_propagation();
                    open.update(|value| *value = !*value);
                }
            >
                {move || view_mode_icon(active_mode.get(), "1rem")}
            </button>
            <Show when=move || open.get()>
                <div
                    class="app-titlebar__popover app-titlebar__popover--view-mode"
                    role="menu"
                    aria-label=move || i18n.tr(I18nKey::TbViewMode)()
                >
                    <p class="app-titlebar__popover-head">{move || i18n.tr(I18nKey::TbViewModeHead)()}</p>
                    <ViewModeItem mode=WorkspaceViewMode::Grid active=active_mode on_choose=choose />
                    <ViewModeItem mode=WorkspaceViewMode::Canvas active=active_mode on_choose=choose />
                    <ViewModeItem mode=WorkspaceViewMode::Swarm active=active_mode on_choose=choose />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn ViewModeItem(
    mode: WorkspaceViewMode,
    active: Signal<WorkspaceViewMode>,
    on_choose: impl Fn(WorkspaceViewMode) + Copy + 'static,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class="app-titlebar__menu-item app-titlebar__view-mode-item"
            class:app-titlebar__view-mode-item--active=move || active.get() == mode
            role="menuitemradio"
            aria-checked=move || (active.get() == mode).to_string()
            on:click=move |_| on_choose(mode)
        >
            {view_mode_icon(mode, "0.95rem")}
            <span class="app-titlebar__menu-item-label">{move || i18n.tr(view_mode_label_key(mode))()}</span>
            <span class="app-titlebar__submenu-check" aria-hidden="true">
                <Show when=move || active.get() == mode>
                    <LxIcon icon=icondata::LuCheck width="0.8rem" height="0.8rem" />
                </Show>
            </span>
        </button>
    }
}

fn view_mode_icon(mode: WorkspaceViewMode, size: &'static str) -> impl IntoView {
    let icon = match mode {
        WorkspaceViewMode::Grid => icondata::LuLayoutGrid,
        WorkspaceViewMode::Canvas => icondata::LuWorkflow,
        WorkspaceViewMode::Swarm => icondata::LuNetwork,
    };
    view! { <LxIcon icon=icon width=size height=size /> }
}

fn view_mode_label_key(mode: WorkspaceViewMode) -> I18nKey {
    match mode {
        WorkspaceViewMode::Grid => I18nKey::TbViewModeGrid,
        WorkspaceViewMode::Canvas => I18nKey::TbViewModeCanvas,
        WorkspaceViewMode::Swarm => I18nKey::TbViewModeSwarm,
    }
}
