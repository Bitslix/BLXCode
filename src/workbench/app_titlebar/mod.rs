//! Custom cross-platform application title bar (`decorations:false`).
//!
//! Mounted once at the App root so the window stays draggable and closable
//! during boot and the EULA gate. The workspace-scoped cluster (sidebar
//! toggles, breadcrumb, NAVIGATE, Notifications, Settings) renders only when
//! `workbench_active` is true; the brand, drag region, and window controls are
//! always present.
//!
//! Per `rule-no-monolith-structure` / `rule-reusable-components`, each piece
//! lives in its own file with one co-located stylesheet (`app-titlebar.css`).
//! Styling is tokens-only (`rule-theme-tokens`).
mod brand;
mod navigate_menu;
mod notifications_menu;
mod window_controls;

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::WorkbenchService;
use brand::TitleBarBrand;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use navigate_menu::NavigateMenu;
use notifications_menu::NotificationsMenu;
use window_controls::WindowControls;

/// One entry in the title-bar notification feed. v1 is always empty; the
/// follow-up wiring (agent-done / toast) pushes into [`TitleBarFeed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TitleBarFeedItem {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// Shared, future-facing store backing the Notifications popover. Provided at
/// the bar root so external producers can populate it without restructuring
/// the component tree.
#[derive(Clone, Copy)]
pub struct TitleBarFeed {
    pub items: RwSignal<Vec<TitleBarFeedItem>>,
}

impl TitleBarFeed {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: RwSignal::new(Vec::new()),
        }
    }
}

impl Default for TitleBarFeed {
    fn default() -> Self {
        Self::new()
    }
}

#[component]
pub fn AppTitleBar(#[prop(into)] workbench_active: Signal<bool>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();

    // Notification feed store, shared for future producers.
    let feed = TitleBarFeed::new();
    provide_context(feed);

    let sidebar_collapsed = wb.sidebar_collapsed();
    let right_collapsed = wb.right_collapsed();

    // Breadcrumb: active workspace title › active center tab title.
    let breadcrumb = Memo::new(move |_| {
        let active = wb.active_id().get()?;
        wb.workspaces().with(|list| {
            let ws = list.iter().find(|w| w.id == active)?;
            let title = ws.title.trim().to_string();
            if title.is_empty() {
                return None;
            }
            let tab = ws
                .center_tabs
                .iter()
                .find(|t| t.id == ws.center_active_tab_id)
                .map(|t| t.title.trim().to_string())
                .filter(|t| !t.is_empty());
            Some((title, tab))
        })
    });

    // When the active center tab is the Terminals grid, append the focused
    // terminal's live title (auto/OSC title, else its slot number/name).
    let terminal_crumb = Memo::new(move |_| wb.active_terminal_breadcrumb_title());

    view! {
        <header class="app-titlebar" data-tauri-drag-region="">
            <div class="app-titlebar__cluster app-titlebar__cluster--left" data-tauri-drag-region="">
                <Show when=move || workbench_active.get()>
                    <button
                        type="button"
                        class="app-titlebar__icon-btn"
                        aria-label=move || {
                            if sidebar_collapsed.get() {
                                i18n.tr(I18nKey::SbExpand)()
                            } else {
                                i18n.tr(I18nKey::SbCollapse)()
                            }
                        }
                        title=move || {
                            if sidebar_collapsed.get() {
                                i18n.tr(I18nKey::SbExpand)()
                            } else {
                                i18n.tr(I18nKey::SbCollapse)()
                            }
                        }
                        on:click=move |_| wb.toggle_sidebar()
                    >
                        {move || {
                            if sidebar_collapsed.get() {
                                view! { <LxIcon icon=icondata::LuPanelLeftOpen width="1rem" height="1rem" /> }.into_any()
                            } else {
                                view! { <LxIcon icon=icondata::LuPanelLeftClose width="1rem" height="1rem" /> }.into_any()
                            }
                        }}
                    </button>
                </Show>
                <TitleBarBrand />
            </div>

            <div class="app-titlebar__cluster app-titlebar__cluster--center" data-tauri-drag-region="">
                <Show when=move || workbench_active.get()>
                    {move || {
                        breadcrumb.get().map(|(ws_title, tab_title)| {
                            let term = terminal_crumb.get();
                            view! {
                                <nav class="app-titlebar__breadcrumb" aria-label="Breadcrumb">
                                    <span class="app-titlebar__crumb app-titlebar__crumb--workspace">{ws_title}</span>
                                    {tab_title.map(|t| view! {
                                        <span class="app-titlebar__crumb-sep" aria-hidden="true">
                                            <LxIcon icon=icondata::LuChevronRight width="0.8rem" height="0.8rem" />
                                        </span>
                                        <span class="app-titlebar__crumb app-titlebar__crumb--context">{t}</span>
                                    })}
                                    {term.map(|(slot, text)| view! {
                                        <span class="app-titlebar__crumb-sep" aria-hidden="true">
                                            <LxIcon icon=icondata::LuChevronRight width="0.8rem" height="0.8rem" />
                                        </span>
                                        <span class="app-titlebar__crumb app-titlebar__crumb--terminal">
                                            {slot.map(|n| view! {
                                                <span class="app-titlebar__crumb-slot">{format!("({n})")}</span>
                                            })}
                                            <span class="app-titlebar__crumb-term-text">{text}</span>
                                        </span>
                                    })}
                                </nav>
                            }
                        })
                    }}
                </Show>
            </div>

            <div class="app-titlebar__cluster app-titlebar__cluster--right" data-tauri-drag-region="">
                <Show when=move || workbench_active.get()>
                    <div class="app-titlebar__actions">
                        <NavigateMenu />
                        <NotificationsMenu />
                        <button
                            type="button"
                            class="app-titlebar__icon-btn"
                            aria-label=move || i18n.tr(I18nKey::CmdSetTitle)()
                            title=move || i18n.tr(I18nKey::CmdSetTitle)()
                            on:click=move |_| wb.open_center_settings_tab(
                                crate::workbench::state::HarnessSettingsCategory::App,
                            )
                        >
                            <LxIcon icon=icondata::LuSettings width="1rem" height="1rem" />
                        </button>
                        <button
                            type="button"
                            class="app-titlebar__icon-btn"
                            aria-label=move || {
                                if right_collapsed.get() {
                                    i18n.tr(I18nKey::RpExpand)()
                                } else {
                                    i18n.tr(I18nKey::RpCollapse)()
                                }
                            }
                            title=move || {
                                if right_collapsed.get() {
                                    i18n.tr(I18nKey::RpExpand)()
                                } else {
                                    i18n.tr(I18nKey::RpCollapse)()
                                }
                            }
                            on:click=move |_| wb.toggle_right_panel()
                        >
                            <LxIcon icon=icondata::LuPanelRight width="1rem" height="1rem" />
                        </button>
                        <div class="app-titlebar__divider" aria-hidden="true"></div>
                    </div>
                </Show>
                <WindowControls />
            </div>
        </header>
    }
}
