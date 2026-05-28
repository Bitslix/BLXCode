//! Right inspector column: collapsible (default closed); width via splitter when open.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::skills_rules_panel::{RulesTabDock, SkillsTabDock};
use crate::workbench::{
    AgentPanelDock, BrowserTabDock, HarnessSettingsCategory, MemoryPanel, PlansPanel,
    RightPanelTab, WorkbenchService,
};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

const RIGHT_PANEL_MIN_PX: f64 = 320.0;
const RIGHT_PANEL_HARD_MIN_PX: f64 = 220.0;
const WORKSPACE_MIN_PX: f64 = 240.0;

fn viewport_width_px() -> f64 {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(RIGHT_PANEL_MIN_PX + WORKSPACE_MIN_PX)
}

fn right_panel_min_px(viewport_w: f64) -> f64 {
    if viewport_w < 980.0 {
        RIGHT_PANEL_HARD_MIN_PX
    } else {
        RIGHT_PANEL_MIN_PX
    }
}

fn right_panel_max_px(viewport_w: f64) -> f64 {
    let min_width = right_panel_min_px(viewport_w);
    let max_by_ratio = viewport_w * 0.58;
    let max_by_space = viewport_w - WORKSPACE_MIN_PX;
    max_by_ratio.max(min_width).min(max_by_space.max(min_width))
}

#[component]
fn MemoryTabDock() -> impl IntoView {
    view! {
        <div class="workbench-right-memory" role="region">
            <MemoryPanel />
        </div>
    }
}

#[component]
fn PlansTabDock() -> impl IntoView {
    view! {
        <div class="workbench-right-plans" role="region">
            <PlansPanel />
        </div>
    }
}

#[component]
fn RightPanelSettingsButton(#[prop(default = "")] extra_class: &'static str) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    view! {
        <button
            type="button"
            class=move || {
                let mut c = String::from("workbench-icon-btn workbench-right-settings-btn");
                if !extra_class.is_empty() {
                    c.push(' ');
                    c.push_str(extra_class);
                }
                c
            }
            aria-label=move || i18n.tr(I18nKey::CmdSetTitle)()
            title=move || i18n.tr(I18nKey::CmdSetTitle)()
            on:click=move |_| wb.open_center_settings_tab(HarnessSettingsCategory::App)
        >
            <span class="workbench-right-settings-btn__icon" aria-hidden="true">
                <LxIcon icon=icondata::LuSettings width="1rem" height="1rem" />
            </span>
        </button>
    }
}

#[component]
pub fn RightPanel() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let collapsed = wb.right_collapsed();

    let resizing = RwSignal::new(false);
    let drag_anchor_x = RwSignal::new(0.0_f64);
    let drag_anchor_w = RwSignal::new(0.0_f64);

    let on_splitter_down = move |ev: web_sys::MouseEvent| {
        if collapsed.get_untracked() {
            return;
        }
        ev.prevent_default();
        drag_anchor_x.set(ev.client_x() as f64);
        drag_anchor_w.set(wb.right_width_px().get_untracked());
        resizing.set(true);
    };

    Effect::new(move |_| {
        if !resizing.get() {
            return;
        }

        let width_sig = wb.right_width_px();
        let ax = drag_anchor_x;
        let aw = drag_anchor_w;
        let resizing_sig = resizing;

        let move_h = window_event_listener_untyped("mousemove", move |ev| {
            let me = match ev.dyn_into::<web_sys::MouseEvent>() {
                Ok(m) => m,
                Err(_) => return,
            };
            let dx = f64::from(me.client_x()) - ax.get_untracked();
            let viewport_w = viewport_width_px();
            let min_width = right_panel_min_px(viewport_w);
            let max_width = right_panel_max_px(viewport_w);
            let next = (aw.get_untracked() - dx).clamp(min_width, max_width);
            width_sig.set(next);
        });

        let up_h = window_event_listener_untyped("mouseup", move |_| {
            resizing_sig.set(false);
        });

        on_cleanup(move || {
            move_h.remove();
            up_h.remove();
        });
    });

    let width_style = Memo::new(move |_| {
        let viewport_w = viewport_width_px();
        let width = wb.right_width_px().get().clamp(
            right_panel_min_px(viewport_w),
            right_panel_max_px(viewport_w),
        );
        format!("{width:.0}px")
    });
    let active_tab = Memo::new(move |_| wb.right_active_tab().get());
    let browser_dock_mounted = RwSignal::new(false);

    Effect::new(move |_| {
        if active_tab.get() == RightPanelTab::Browser {
            browser_dock_mounted.set(true);
        }
    });

    view! {
        <div
            class="workbench-right-slot"
            class:workbench-right-slot--collapsed=move || collapsed.get()
            class:workbench-right-slot--resizing=move || resizing.get()
        >
            <div
                class="workbench-right-rail"
                class:workbench-right-rail--hidden=move || !collapsed.get()
                role="toolbar"
                aria-label=move || i18n.tr(I18nKey::RpRailAria)()
            >
                <header class="workbench-gutter-bar">
                    <button
                        type="button"
                        class="workbench-icon-btn workbench-right-panel-toggle"
                        aria-expanded=move || (!collapsed.get()).to_string()
                        aria-label=move || if collapsed.get() { i18n.tr(I18nKey::RpExpand)() } else { i18n.tr(I18nKey::RpCollapse)() }
                        title=move || if collapsed.get() { i18n.tr(I18nKey::RpExpand)() } else { i18n.tr(I18nKey::RpCollapse)() }
                        on:click=move |_| wb.toggle_right_panel()
                    >
                        <span class="workbench-right-panel-toggle__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuPanelRight width="1rem" height="1rem" />
                        </span>
                    </button>
                </header>
                <div
                    class="workbench-right-rail__tabs"
                    role="tablist"
                    aria-orientation="vertical"
                    aria-label=move || i18n.tr(I18nKey::RpTabsAria)()
                >
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || active_tab.get() == RightPanelTab::Agent
                        class="workbench-right-rail-tab"
                        class:workbench-right-rail-tab--active=move || active_tab.get() == RightPanelTab::Agent
                        aria-label=move || i18n.tr(I18nKey::TabAgent)()
                        title=move || i18n.tr(I18nKey::TabAgent)()
                        on:click=move |_| {
                            wb.set_right_tab(RightPanelTab::Agent);
                            if wb.right_collapsed().get_untracked() {
                                wb.toggle_right_panel();
                            }
                        }
                    >
                        <span class="workbench-right-rail-tab__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuSparkles width="1rem" height="1rem" />
                        </span>
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || active_tab.get() == RightPanelTab::Browser
                        class="workbench-right-rail-tab"
                        class:workbench-right-rail-tab--active=move || active_tab.get() == RightPanelTab::Browser
                        aria-label=move || i18n.tr(I18nKey::TabBrowser)()
                        title=move || i18n.tr(I18nKey::TabBrowser)()
                        on:click=move |_| {
                            wb.set_right_tab(RightPanelTab::Browser);
                            if wb.right_collapsed().get_untracked() {
                                wb.toggle_right_panel();
                            }
                        }
                    >
                        <span class="workbench-right-rail-tab__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuGlobe width="1rem" height="1rem" />
                        </span>
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || active_tab.get() == RightPanelTab::Plans
                        class="workbench-right-rail-tab"
                        class:workbench-right-rail-tab--active=move || active_tab.get() == RightPanelTab::Plans
                        aria-label=move || i18n.tr(I18nKey::TabPlans)()
                        title=move || i18n.tr(I18nKey::TabPlans)()
                        on:click=move |_| {
                            wb.set_right_tab(RightPanelTab::Plans);
                            if wb.right_collapsed().get_untracked() {
                                wb.toggle_right_panel();
                            }
                        }
                    >
                        <span class="workbench-right-rail-tab__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuClipboardList width="1rem" height="1rem" />
                        </span>
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || active_tab.get() == RightPanelTab::Memory
                        class="workbench-right-rail-tab"
                        class:workbench-right-rail-tab--active=move || active_tab.get() == RightPanelTab::Memory
                        aria-label=move || i18n.tr(I18nKey::TabMemory)()
                        title=move || i18n.tr(I18nKey::TabMemory)()
                        on:click=move |_| {
                            wb.set_right_tab(RightPanelTab::Memory);
                            if wb.right_collapsed().get_untracked() {
                                wb.toggle_right_panel();
                            }
                        }
                    >
                        <span class="workbench-right-rail-tab__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuLayers width="1rem" height="1rem" />
                        </span>
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || active_tab.get() == RightPanelTab::Rules
                        class="workbench-right-rail-tab"
                        class:workbench-right-rail-tab--active=move || active_tab.get() == RightPanelTab::Rules
                        aria-label=move || i18n.tr(I18nKey::TabRules)()
                        title=move || i18n.tr(I18nKey::TabRules)()
                        on:click=move |_| {
                            wb.set_right_tab(RightPanelTab::Rules);
                            if wb.right_collapsed().get_untracked() {
                                wb.toggle_right_panel();
                            }
                        }
                    >
                        <span class="workbench-right-rail-tab__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuShield width="1rem" height="1rem" />
                        </span>
                    </button>
                    <button
                        type="button"
                        role="tab"
                        aria-selected=move || active_tab.get() == RightPanelTab::Skills
                        class="workbench-right-rail-tab"
                        class:workbench-right-rail-tab--active=move || active_tab.get() == RightPanelTab::Skills
                        aria-label=move || i18n.tr(I18nKey::TabSkills)()
                        title=move || i18n.tr(I18nKey::TabSkills)()
                        on:click=move |_| {
                            wb.set_right_tab(RightPanelTab::Skills);
                            if wb.right_collapsed().get_untracked() {
                                wb.toggle_right_panel();
                            }
                        }
                    >
                        <span class="workbench-right-rail-tab__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuPuzzle width="1rem" height="1rem" />
                        </span>
                    </button>
                </div>
                <footer class="workbench-right-rail__footer">
                    <RightPanelSettingsButton extra_class="workbench-right-rail-settings-btn" />
                </footer>
            </div>
            <div
                class="workbench-splitter"
                class:workbench-splitter--hidden=move || collapsed.get()
                role="separator"
                aria-orientation="vertical"
                aria-label=move || i18n.tr(I18nKey::RpSplitterAria)()
                on:mousedown=on_splitter_down
            >
            </div>
            <Show when=move || resizing.get()>
                <div class="workbench-resize-shield" aria-hidden="true"></div>
            </Show>
            <Show when=move || !collapsed.get()>
                <aside
                    class="workbench-right"
                    style:width=move || width_style.get()
                >
                    <header class="workbench-right__header">
                        <div class="workbench-right__toolbar">
                            <button
                                type="button"
                                class="workbench-icon-btn workbench-right-panel-toggle"
                                aria-expanded="true"
                                aria-label=move || i18n.tr(I18nKey::RpCollapse)()
                                title=move || i18n.tr(I18nKey::RpCollapse)()
                                on:click=move |_| wb.toggle_right_panel()
                            >
                                <span class="workbench-right-panel-toggle__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuPanelRight width="1rem" height="1rem" />
                                </span>
                            </button>
                            <div class="workbench-right-tabstrip" role="tablist" aria-label=move || i18n.tr(I18nKey::RpTabsAria)()>
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || active_tab.get() == RightPanelTab::Agent
                                    class="workbench-right-tab"
                                    class:workbench-right-tab--active=move || active_tab.get() == RightPanelTab::Agent
                                    aria-label=move || i18n.tr(I18nKey::TabAgent)()
                                    title=move || i18n.tr(I18nKey::TabAgent)()
                                    on:click=move |_| wb.set_right_tab(RightPanelTab::Agent)
                                >
                                    <span class="workbench-right-tab__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuSparkles width="14px" height="14px" />
                                    </span>
                                    <span class="workbench-right-tab__label">{move || i18n.tr(I18nKey::TabAgent)()}</span>
                                </button>
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || active_tab.get() == RightPanelTab::Browser
                                    class="workbench-right-tab"
                                    class:workbench-right-tab--active=move || active_tab.get() == RightPanelTab::Browser
                                    aria-label=move || i18n.tr(I18nKey::TabBrowser)()
                                    title=move || i18n.tr(I18nKey::TabBrowser)()
                                    on:click=move |_| wb.set_right_tab(RightPanelTab::Browser)
                                >
                                    <span class="workbench-right-tab__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuGlobe width="14px" height="14px" />
                                    </span>
                                    <span class="workbench-right-tab__label">{move || i18n.tr(I18nKey::TabBrowser)()}</span>
                                </button>
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || active_tab.get() == RightPanelTab::Plans
                                    class="workbench-right-tab"
                                    class:workbench-right-tab--active=move || active_tab.get() == RightPanelTab::Plans
                                    aria-label=move || i18n.tr(I18nKey::TabPlans)()
                                    title=move || i18n.tr(I18nKey::TabPlans)()
                                    on:click=move |_| wb.set_right_tab(RightPanelTab::Plans)
                                >
                                    <span class="workbench-right-tab__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuClipboardList width="14px" height="14px" />
                                    </span>
                                    <span class="workbench-right-tab__label">{move || i18n.tr(I18nKey::TabPlans)()}</span>
                                </button>
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || active_tab.get() == RightPanelTab::Memory
                                    class="workbench-right-tab"
                                    class:workbench-right-tab--active=move || active_tab.get() == RightPanelTab::Memory
                                    aria-label=move || i18n.tr(I18nKey::TabMemory)()
                                    title=move || i18n.tr(I18nKey::TabMemory)()
                                    on:click=move |_| wb.set_right_tab(RightPanelTab::Memory)
                                >
                                    <span class="workbench-right-tab__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuLayers width="14px" height="14px" />
                                    </span>
                                    <span class="workbench-right-tab__label">{move || i18n.tr(I18nKey::TabMemory)()}</span>
                                </button>
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || active_tab.get() == RightPanelTab::Rules
                                    class="workbench-right-tab"
                                    class:workbench-right-tab--active=move || active_tab.get() == RightPanelTab::Rules
                                    aria-label=move || i18n.tr(I18nKey::TabRules)()
                                    title=move || i18n.tr(I18nKey::TabRules)()
                                    on:click=move |_| wb.set_right_tab(RightPanelTab::Rules)
                                >
                                    <span class="workbench-right-tab__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuShield width="14px" height="14px" />
                                    </span>
                                    <span class="workbench-right-tab__label">{move || i18n.tr(I18nKey::TabRules)()}</span>
                                </button>
                                <button
                                    type="button"
                                    role="tab"
                                    aria-selected=move || active_tab.get() == RightPanelTab::Skills
                                    class="workbench-right-tab"
                                    class:workbench-right-tab--active=move || active_tab.get() == RightPanelTab::Skills
                                    aria-label=move || i18n.tr(I18nKey::TabSkills)()
                                    title=move || i18n.tr(I18nKey::TabSkills)()
                                    on:click=move |_| wb.set_right_tab(RightPanelTab::Skills)
                                >
                                    <span class="workbench-right-tab__icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuPuzzle width="14px" height="14px" />
                                    </span>
                                    <span class="workbench-right-tab__label">{move || i18n.tr(I18nKey::TabSkills)()}</span>
                                </button>

                            </div>
                            <RightPanelSettingsButton />
                        </div>
                    </header>
                    <div id="blx-right-panel-body" class="workbench-right__body">
                        <div class="workbench-right-tab-panel" class:workbench-right-tab-panel--hidden=move || active_tab.get() != RightPanelTab::Agent>
                            <AgentPanelDock />
                        </div>
                        <Show when=move || browser_dock_mounted.get()>
                            <div class="workbench-right-tab-panel" class:workbench-right-tab-panel--hidden=move || active_tab.get() != RightPanelTab::Browser>
                                <BrowserTabDock />
                            </div>
                        </Show>
                        <div class="workbench-right-tab-panel" class:workbench-right-tab-panel--hidden=move || active_tab.get() != RightPanelTab::Plans>
                            <PlansTabDock />
                        </div>
                        <div class="workbench-right-tab-panel" class:workbench-right-tab-panel--hidden=move || active_tab.get() != RightPanelTab::Memory>
                            <MemoryTabDock />
                        </div>
                        <div class="workbench-right-tab-panel" class:workbench-right-tab-panel--hidden=move || active_tab.get() != RightPanelTab::Rules>
                            <RulesTabDock />
                        </div>
                        <div class="workbench-right-tab-panel" class:workbench-right-tab-panel--hidden=move || active_tab.get() != RightPanelTab::Skills>
                            <SkillsTabDock />
                        </div>
                    </div>
                </aside>
            </Show>
        </div>
    }
}
