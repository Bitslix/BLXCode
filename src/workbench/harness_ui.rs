//! Befehlspalette und Harness‑Einstellungen (kategorisiert).
//!
//! Tastenkürzel (tmux-Standard: `Ctrl+b` + zweite Taste; Legacy in App-Einstellungen)
//! sind im Haupt-Webview gebunden ([`HarnessHost`] → [`super::harness_chords`]).
use super::app_prefs::AppPrefsService;
use super::browser_tab::sync_embedded_browser_layer;
use super::harness_chords::handle_harness_keydown;
use super::state::{
    workspace_entry_has_folder, BrowserEmbedSurface, HarnessSettingsCategory, HarnessUiService,
    RecentWorkspaceItem, RightPanelTab, WorkbenchService,
};
use super::update_service::{UpdateService, UpdateUiStatus};
use super::voice_app_controls::{VoicePttControls, VoiceSttLanguageControls};
use crate::i18n::{lookup, I18nKey, Locale, APP_LOCALES};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_hooks_status, install_agent_hooks, is_tauri_shell, uninstall_agent_hooks,
    voice_settings_get, voice_settings_save, AgentHooksReport, VoiceSettings,
};
use gloo_timers::future::TimeoutFuture;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;

#[derive(Clone, Copy)]
enum PaletteAction {
    OpenQuickOpen,
    OpenSettings,
    ToggleRightPanel,
    AgentTab,
    BrowserTab,
    MemoryTab,
    /// Reopen the Terminals tab in the active workspace without spawning
    /// a fresh terminal slot — used after the user closed the tab via
    /// the confirmation dialog (technically that flow closes the whole
    /// workspace, so this is mainly handy after manually closing extra
    /// tabs and wanting to jump back to the grid).
    OpenTerminalsTab,
}

#[derive(Clone, Copy)]
struct PaletteRow {
    title: I18nKey,
    subtitle: I18nKey,
    action: PaletteAction,
    icon: icondata::Icon,
}

const PALETTE_ROWS: &[PaletteRow] = &[
    PaletteRow {
        title: I18nKey::CmdQkTitle,
        subtitle: I18nKey::CmdQkSub,
        action: PaletteAction::OpenQuickOpen,
        icon: icondata::LuFolderSearch,
    },
    PaletteRow {
        title: I18nKey::CmdSetTitle,
        subtitle: I18nKey::CmdSetSub,
        action: PaletteAction::OpenSettings,
        icon: icondata::LuSettings2,
    },
    PaletteRow {
        title: I18nKey::CmdRtpTitle,
        subtitle: I18nKey::CmdRtpSub,
        action: PaletteAction::ToggleRightPanel,
        icon: icondata::LuPanelRight,
    },
    PaletteRow {
        title: I18nKey::CmdAgentTitle,
        subtitle: I18nKey::CmdAgentSub,
        action: PaletteAction::AgentTab,
        icon: icondata::LuSparkles,
    },
    PaletteRow {
        title: I18nKey::CmdBrowseTitle,
        subtitle: I18nKey::CmdBrowseSub,
        action: PaletteAction::BrowserTab,
        icon: icondata::LuGlobe,
    },
    PaletteRow {
        title: I18nKey::CmdMemoryTitle,
        subtitle: I18nKey::CmdMemorySub,
        action: PaletteAction::MemoryTab,
        icon: icondata::LuLayers,
    },
    PaletteRow {
        title: I18nKey::CmdTermTitle,
        subtitle: I18nKey::CmdTermSub,
        action: PaletteAction::OpenTerminalsTab,
        icon: icondata::LuSquareTerminal,
    },
];

fn palette_matches(q_raw: &str, row: &PaletteRow, loc: Locale) -> bool {
    let q = q_raw.trim().to_ascii_lowercase();
    if q.is_empty() {
        return true;
    }
    let title = lookup(loc, row.title).to_ascii_lowercase();
    let sub = lookup(loc, row.subtitle).to_ascii_lowercase();
    title.contains(&q) || sub.contains(&q)
}

#[component]
pub fn HarnessHost() -> impl IntoView {
    let ui = expect_context::<HarnessUiService>();
    let wb = expect_context::<WorkbenchService>();
    let embed = expect_context::<BrowserEmbedSurface>();
    let prefs = expect_context::<AppPrefsService>();

    Effect::new(move |_| {
        let handle = window_event_listener_untyped("keydown", move |ev| {
            let Ok(ke) = ev.dyn_into::<web_sys::KeyboardEvent>() else {
                return;
            };
            let _ = handle_harness_keydown(&ke, prefs, ui, wb, embed);
        });

        on_cleanup(move || handle.remove());
    });

    view! {
        <Show when=move || ui.quick_open_open().get()>
            <QuickOpenChrome ui=ui wb=wb embed=embed />
        </Show>
        <Show when=move || ui.palette_open().get()>
            <PaletteChrome ui=ui wb=wb embed=embed />
        </Show>
    }
}

#[component]
fn PaletteChrome(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    Effect::new(move |_| {
        let _ = i18n.locale().get();
        ui.palette_selection().set(0);
    });

    Effect::new(move |_| {
        leptos::task::spawn_local(async {
            TimeoutFuture::new(32).await;
            let Some(w) = web_sys::window() else {
                return;
            };
            let Some(doc) = w.document() else {
                return;
            };
            let Some(el) = doc.get_element_by_id("harness-palette-filter") else {
                return;
            };
            let Ok(inp) = el.dyn_into::<web_sys::HtmlInputElement>() else {
                return;
            };
            let _ = inp.focus();
        });
    });

    view! {
        <div class="harness-overlay harness-overlay--modal" role="presentation">
            <div class="harness-sheet harness-sheet--palette" role="dialog" aria-modal="true">
                <div class="harness-palette-filter-wrap">
                    <span class="harness-palette-filter__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuSearch width="0.92rem" height="0.92rem" />
                    </span>
                    <input
                        class="workbench-plain-input harness-filter harness-filter--with-icon"
                        id="harness-palette-filter"
                        placeholder=move || i18n.tr(I18nKey::PlFilterPh)()
                        type="text"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || ui.palette_query().get()
                        on:input=move |ev| palette_input(ev, ui)
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            palette_key_nav(ev, ui, wb, embed, i18n);
                        }
                    />
                </div>

                <ul class="harness-cmd-list" role="listbox">
                    <PaletteList ui=ui wb=wb embed=embed i18n=i18n />
                </ul>

                <p class="harness-sheet-hint">
                    {move || i18n.tr(I18nKey::PlHint)()}
                </p>
            </div>
            <button
                type="button"
                class="harness-scrim"
                tabindex="-1"
                aria-label=move || i18n.tr(I18nKey::BtnClose)()
                on:click=move |_| ui.close_command_palette()
            ></button>

        </div>
    }
}

#[component]
fn QuickOpenChrome(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let path_buf = RwSignal::new(String::new());

    let ranked = Memo::new(move |_| {
        let q = ui.quick_open_query().get().trim().to_ascii_lowercase();
        wb.recent_workspaces().with(|list| {
            list.iter()
                .enumerate()
                .filter(|(_, it)| {
                    workspace_entry_has_folder(&it.workspace)
                        && (q.is_empty()
                            || it.workspace.title.to_ascii_lowercase().contains(&q)
                            || it.workspace.cwd.to_ascii_lowercase().contains(&q))
                })
                .map(|(i, it)| (i, it.clone()))
                .collect::<Vec<_>>()
        })
    });

    Effect::new(move |_| {
        let _ = i18n.locale().get();
        ui.quick_open_selection().set(0);
    });

    Effect::new(move |_| {
        leptos::task::spawn_local(async {
            TimeoutFuture::new(32).await;
            let Some(w) = web_sys::window() else {
                return;
            };
            let Some(doc) = w.document() else {
                return;
            };
            let Some(el) = doc.get_element_by_id("harness-quickopen-filter") else {
                return;
            };
            let Ok(inp) = el.dyn_into::<web_sys::HtmlInputElement>() else {
                return;
            };
            let _ = inp.focus();
        });
    });

    view! {
        <div class="harness-overlay harness-overlay--modal" role="presentation">
            <div
                class="harness-sheet harness-sheet--palette harness-sheet--quickopen"
                role="dialog"
                aria-modal="true"
            >
                <h2 class="harness-quickopen-title">{move || i18n.tr(I18nKey::QkTitle)()}</h2>
                <div class="harness-palette-filter-wrap">
                    <span class="harness-palette-filter__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuSearch width="0.92rem" height="0.92rem" />
                    </span>
                    <input
                        class="workbench-plain-input harness-filter harness-filter--with-icon"
                        id="harness-quickopen-filter"
                        placeholder=move || i18n.tr(I18nKey::QkFilterPh)()
                        type="text"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || ui.quick_open_query().get()
                        on:input=move |ev| quick_open_filter_input(ev, ui)
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            quick_open_key_nav(ev, ui, wb, embed, ranked);
                        }
                    />
                </div>

                <p class="harness-quickopen-section">{move || i18n.tr(I18nKey::QkRecentHeading)()}</p>
                <ul class="harness-cmd-list" role="listbox">
                    {move || {
                        let rows = ranked.get();
                        if rows.is_empty() {
                            return view! {
                                <li class="harness-muted">{move || i18n.tr(I18nKey::QkEmptyRecent)()}</li>
                            }
                            .into_any();
                        }
                        rows.into_iter()
                            .enumerate()
                            .map(|(rank, (orig_idx, item))| {
                                let title = item.workspace.title.clone();
                                let cwd = item.workspace.cwd.clone();
                                let sel = ui.quick_open_selection();
                                view! {
                                    <li class="harness-cmd-li workbench-recent-row">
                                        <button
                                            type="button"
                                            class="harness-cmd-btn"
                                            class:harness-cmd-btn--active=move || sel.get() == rank
                                            on:click=move |_| {
                                                wb.reopen_recent_workspace(orig_idx);
                                                ui.close_quick_open();
                                                defer_browser_bounds(wb, embed);
                                            }
                                        >
                                            <span class="harness-cmd-btn__icon" aria-hidden="true">
                                                <LxIcon icon=icondata::LuFolder width="1rem" height="1rem" />
                                            </span>
                                            <span class="harness-cmd-btn__text">
                                                <span class="harness-cmd-title">{title}</span>
                                                <span class="harness-cmd-sub">{cwd}</span>
                                            </span>
                                        </button>
                                        <button
                                            type="button"
                                            class="workbench-recent-remove"
                                            aria-label=move || i18n.tr(I18nKey::QkRecentRemoveAria)()
                                            on:click=move |ev: MouseEvent| {
                                                ev.stop_propagation();
                                                ev.prevent_default();
                                                wb.remove_recent_workspace(orig_idx);
                                                clamp_quick_open_selection_after_recent_change(
                                                    ui, wb,
                                                );
                                            }
                                        >
                                            <span aria-hidden="true">
                                                <LxIcon icon=icondata::LuX width="0.85rem" height="0.85rem" />
                                            </span>
                                        </button>
                                    </li>
                                }
                            })
                            .collect_view()
                            .into_any()
                    }}
                </ul>

                <div class="harness-quickopen-path">
                    <input
                        class="workbench-plain-input harness-filter"
                        type="text"
                        spellcheck="false"
                        placeholder=move || i18n.tr(I18nKey::QkPathPh)()
                        prop:value=move || path_buf.get()
                        on:input=move |ev| {
                            if let Some(s) = input_str(&ev) {
                                path_buf.set(s);
                            }
                        }
                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                            if ev.key() == "Enter" {
                                ev.prevent_default();
                                let p = path_buf.get_untracked();
                                if wb.open_workspace_from_path_quick(p).is_ok() {
                                    path_buf.set(String::new());
                                    ui.close_quick_open();
                                    defer_browser_bounds(wb, embed);
                                }
                            }
                        }
                    />
                    <button
                        type="button"
                        class="harness-quickopen-path__btn"
                        on:click=move |_| {
                            let p = path_buf.get_untracked();
                            if wb.open_workspace_from_path_quick(p).is_ok() {
                                path_buf.set(String::new());
                                ui.close_quick_open();
                                defer_browser_bounds(wb, embed);
                            }
                        }
                    >
                        {move || i18n.tr(I18nKey::QkPathOpen)()}
                    </button>
                </div>

                <button
                    type="button"
                    class="harness-quickopen-wizard"
                    on:click=move |_| {
                        let _ = wb.start_inline_configure();
                        ui.close_quick_open();
                    }
                >
                    {move || i18n.tr(I18nKey::QkNewWorkspace)()}
                </button>

                <p class="harness-sheet-hint">{move || i18n.tr(I18nKey::QkHint)()}</p>
            </div>
            <button
                type="button"
                class="harness-scrim"
                tabindex="-1"
                aria-label=move || i18n.tr(I18nKey::BtnClose)()
                on:click=move |_| ui.close_quick_open()
            ></button>
        </div>
    }
}

fn quick_open_filter_input(ev: web_sys::Event, ui: HarnessUiService) {
    if let Some(s) = input_str(&ev) {
        ui.quick_open_query().set(s);
    }
    ui.quick_open_selection().set(0);
}

fn clamp_quick_open_selection_after_recent_change(ui: HarnessUiService, wb: WorkbenchService) {
    let n = wb.recent_workspaces().with(|list| {
        list.iter()
            .filter(|it| workspace_entry_has_folder(&it.workspace))
            .count()
    });
    ui.quick_open_selection().update(|s| {
        if n == 0 {
            *s = 0;
        } else {
            *s = (*s).min(n - 1);
        }
    });
}

fn quick_open_key_nav(
    ev: web_sys::KeyboardEvent,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
    ranked: Memo<Vec<(usize, RecentWorkspaceItem)>>,
) {
    let rows = ranked.get_untracked();
    let n = rows.len();
    match ev.key().as_str() {
        "ArrowDown" => {
            ev.prevent_default();
            if n == 0 {
                return;
            }
            let next = (ui.quick_open_selection().get_untracked().saturating_add(1)).min(n - 1);
            ui.quick_open_selection().set(next);
        }
        "ArrowUp" => {
            ev.prevent_default();
            if n == 0 {
                return;
            }
            let sel = ui.quick_open_selection().get_untracked().saturating_sub(1);
            ui.quick_open_selection().set(sel);
        }
        "Enter" => {
            ev.prevent_default();
            if n > 0 {
                let sel = ui.quick_open_selection().get_untracked().min(n - 1);
                if let Some((orig_idx, _)) = rows.get(sel) {
                    wb.reopen_recent_workspace(*orig_idx);
                    ui.close_quick_open();
                    defer_browser_bounds(wb, embed);
                }
                return;
            }
            let p = ui.quick_open_query().get_untracked().trim().to_string();
            if !p.is_empty() && wb.open_workspace_from_path_quick(p).is_ok() {
                ui.quick_open_query().set(String::new());
                ui.close_quick_open();
                defer_browser_bounds(wb, embed);
            }
        }
        _ => {}
    }
}

fn palette_input(ev: web_sys::Event, ui: HarnessUiService) {
    if let Some(s) = input_str(&ev) {
        ui.palette_query().set(s);
    }
    ui.palette_selection().set(0);
}

fn input_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.value())
}

fn checkbox_checked(ev: &web_sys::Event) -> Option<bool> {
    ev.target()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
        .map(|i| i.checked())
}

#[component]
fn PaletteList(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
    i18n: I18nService,
) -> impl IntoView {
    view! {
        {move || {
            let needle = ui.palette_query().get();
            let loc = i18n.locale().get();
            let filtered: Vec<usize> = PALETTE_ROWS
                .iter()
                .enumerate()
                .filter(|(_, row)| palette_matches(&needle, row, loc))
                .map(|(i, _)| i)
                .collect();

            if filtered.is_empty() {
                return view! {
                    <li class="harness-muted">{move || i18n.tr(I18nKey::PlNoHits)()}</li>
                }
                .into_any();
            }

            filtered
                .into_iter()
                .enumerate()
                .map(|(rank, row_idx)| {
                    let meta = PALETTE_ROWS[row_idx];
                    let palette_sel = ui.palette_selection();

                    view! {
                        <li class="harness-cmd-li">
                            <button
                                type="button"
                                class="harness-cmd-btn"
                                class:harness-cmd-btn--active=move || palette_sel.get() == rank
                                on:click=move |_| palette_run(ui, wb, embed, meta.action)
                            >
                                <span class="harness-cmd-btn__icon" aria-hidden="true">
                                    <LxIcon icon=meta.icon width="1rem" height="1rem" />
                                </span>
                                <span class="harness-cmd-btn__text">
                                    <span class="harness-cmd-title">{move || i18n.tr(meta.title)()}</span>
                                    <span class="harness-cmd-sub">{move || i18n.tr(meta.subtitle)()}</span>
                                </span>
                            </button>
                        </li>
                    }
                })
                .collect_view()
                .into_any()
        }}
    }
}

fn palette_key_nav(
    ev: web_sys::KeyboardEvent,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
    i18n: I18nService,
) {
    let loc = i18n.locale().get_untracked();
    match ev.key().as_str() {
        "ArrowDown" => {
            ev.prevent_default();
            let filtered = filtered_with_query(ui.palette_query().get_untracked(), loc);
            if filtered.is_empty() {
                return;
            }
            let next =
                (ui.palette_selection().get_untracked().saturating_add(1)).min(filtered.len() - 1);
            ui.palette_selection().set(next);
        }
        "ArrowUp" => {
            ev.prevent_default();
            let filtered_len = filtered_with_query(ui.palette_query().get_untracked(), loc).len();
            if filtered_len == 0 {
                return;
            }
            let sel = ui.palette_selection().get_untracked().saturating_sub(1);
            ui.palette_selection().set(sel);
        }
        "Enter" => {
            ev.prevent_default();
            let filtered = filtered_with_query(ui.palette_query().get_untracked(), loc);
            if filtered.is_empty() {
                return;
            }
            let idx = filtered
                .get(
                    ui.palette_selection()
                        .get_untracked()
                        .min(filtered.len().saturating_sub(1)),
                )
                .copied();
            let Some(pi) = idx else {
                return;
            };
            palette_run(ui, wb, embed, PALETTE_ROWS[pi].action);
        }
        _ => {}
    }
}

fn filtered_with_query(needle: String, loc: Locale) -> Vec<usize> {
    PALETTE_ROWS
        .iter()
        .enumerate()
        .filter(|(_, row)| palette_matches(&needle, row, loc))
        .map(|(i, _)| i)
        .collect()
}

fn palette_run(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
    action: PaletteAction,
) {
    match action {
        PaletteAction::OpenQuickOpen => {
            ui.close_command_palette();
            ui.open_quick_open();
        }
        PaletteAction::OpenSettings => {
            ui.close_command_palette();
            ui.settings_category().set(HarnessSettingsCategory::App);
            wb.open_center_settings_tab(HarnessSettingsCategory::App);
        }
        PaletteAction::ToggleRightPanel => {
            wb.toggle_right_panel();
            ui.close_command_palette();
            defer_browser_bounds(wb, embed);
        }
        PaletteAction::AgentTab => {
            reveal_tab(RightPanelTab::Agent, ui, wb, embed);
        }
        PaletteAction::BrowserTab => {
            reveal_tab(RightPanelTab::Browser, ui, wb, embed);
        }
        PaletteAction::MemoryTab => {
            reveal_tab(RightPanelTab::Memory, ui, wb, embed);
        }
        PaletteAction::OpenTerminalsTab => {
            if let Some(workspace_id) = wb.active_id().get_untracked() {
                wb.open_center_terminals_tab(workspace_id);
            }
        }
    }
}

fn reveal_tab(
    tab: RightPanelTab,
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) {
    if wb.right_collapsed().get_untracked() {
        wb.toggle_right_panel();
    }
    wb.set_right_tab(tab);
    ui.close_command_palette();
    defer_browser_bounds(wb, embed);
}

fn defer_browser_bounds(wb: WorkbenchService, embed: BrowserEmbedSurface) {
    leptos::task::spawn_local(async move {
        TimeoutFuture::new(48).await;
        sync_embedded_browser_layer(wb, embed).await;
    });
}

fn harness_settings_cat_icon(cat: HarnessSettingsCategory) -> icondata::Icon {
    match cat {
        HarnessSettingsCategory::App => icondata::LuLayoutDashboard,
        HarnessSettingsCategory::Appearance => icondata::LuSunMoon,
        HarnessSettingsCategory::Shortcuts => icondata::LuKeyboard,
        HarnessSettingsCategory::ApiKeys => icondata::LuKeyRound,
        HarnessSettingsCategory::Workspace => icondata::LuFolderOpen,
        HarnessSettingsCategory::AgentProvider => icondata::LuCpu,
        HarnessSettingsCategory::Memory => icondata::LuPalette,
        HarnessSettingsCategory::Voice => icondata::LuMic,
        HarnessSettingsCategory::Image => icondata::LuImage,
    }
}

#[component]
pub fn SettingsDock(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <div class="harness-settings-grid harness-settings-grid--docked">
            <nav class="harness-settings-cats" aria-label=move || i18n.tr(I18nKey::HsAriaCats)()>
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::App label=I18nKey::HsCatApp />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Appearance label=I18nKey::HsCatAppearance />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Shortcuts label=I18nKey::HsCatShortcuts />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::ApiKeys label=I18nKey::HsCatApiKeys />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Workspace label=I18nKey::HsCatWorkspace />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::AgentProvider label=I18nKey::HsCatProvider />
            </nav>

            <div class="harness-settings-detail">
                {move || match ui.settings_category().get() {
                    HarnessSettingsCategory::App => view! {
                        <AppSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::Appearance => view! {
                        <crate::workbench::AppearanceSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::Shortcuts => view! {
                        <crate::workbench::ShortcutsSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::ApiKeys => view! {
                        <ApiKeysSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::Workspace => view! {
                        <crate::workbench::WorkspaceSettingsPane wb=wb embed=embed />
                    }.into_any(),
                    HarnessSettingsCategory::AgentProvider => view! {
                        <crate::workbench::AgentProviderPane />
                    }.into_any(),
                    HarnessSettingsCategory::Memory => view! {
                        <crate::workbench::WorkspaceSettingsPane wb=wb embed=embed />
                    }.into_any(), // legacy category → Workspace
                    HarnessSettingsCategory::Voice => view! {
                        <crate::workbench::AgentProviderPane />
                    }.into_any(), // legacy category → BLXCode Agent
                    HarnessSettingsCategory::Image => view! {
                        <crate::workbench::AgentProviderPane />
                    }.into_any(), // legacy category → BLXCode Agent
                }}
            </div>
        </div>
    }
}

#[component]
fn HarnessCatBtn(
    ui: HarnessUiService,
    cat: HarnessSettingsCategory,
    label: I18nKey,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let icon = harness_settings_cat_icon(cat);
    view! {
        <button
            type="button"
            class:harness-cat-active=move || ui.settings_category().get() == cat
            class="harness-cat-btn"
            on:click={
                move |_| ui.settings_category().set(cat)
            }
        >
            <span class="harness-cat-btn__icon" aria-hidden="true">
                <LxIcon icon=icon width="0.92rem" height="0.92rem" />
            </span>
            <span class="harness-cat-btn__label">{move || i18n.tr(label)()}</span>
        </button>
    }
}

fn focus_locale_option(loc: Locale) {
    let id = format!("locale-option-{}", loc.as_str());
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(el) = doc.get_element_by_id(&id) else {
        return;
    };
    let Ok(button) = el.dyn_into::<web_sys::HtmlElement>() else {
        return;
    };
    let _ = button.focus();
}

fn next_app_locale(loc: Locale) -> Locale {
    let n = APP_LOCALES.len();
    let idx = APP_LOCALES.iter().position(|(l, _)| *l == loc).unwrap_or(0);
    APP_LOCALES[(idx + 1) % n].0
}

fn prev_app_locale(loc: Locale) -> Locale {
    let n = APP_LOCALES.len();
    let idx = APP_LOCALES.iter().position(|(l, _)| *l == loc).unwrap_or(0);
    APP_LOCALES[(idx + n - 1) % n].0
}

fn app_locale_native_label(loc: Locale) -> &'static str {
    APP_LOCALES
        .iter()
        .find(|(l, _)| *l == loc)
        .map(|(_, label)| *label)
        .unwrap_or("?")
}

#[component]
fn LocalePicker() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |loc: Locale| {
        i18n.set_locale(loc);
        open.set(false);
    };

    view! {
        <div class="harness-provider-picker harness-locale-picker">
            <button
                type="button"
                class="harness-provider-trigger"
                aria-haspopup="listbox"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |_| {
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        let loc = i18n.locale().get_untracked();
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(0).await;
                            focus_locale_option(loc);
                        });
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " => {
                            ev.prevent_default();
                            open.set(true);
                            let loc = i18n.locale().get_untracked();
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_locale_option(loc);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let loc = prev_app_locale(i18n.locale().get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_locale_option(loc);
                            });
                        }
                        "Escape" => open.set(false),
                        _ => {}
                    }
                }
            >
                <span class="harness-provider-trigger__main">
                    <span class="harness-provider-trigger__brand">
                        <img
                            class="harness-provider-trigger__img"
                            src=move || i18n.locale().get().flag_icon_url()
                            alt=""
                        />
                    </span>
                    <span>{move || app_locale_native_label(i18n.locale().get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    <For
                        each={move || APP_LOCALES.iter().copied().collect::<Vec<_>>()}
                        key=|&(loc, _)| loc
                        children={move |(loc, label)| {
                            view! {
                                <button
                                    id=format!("locale-option-{}", loc.as_str())
                                    type="button"
                                    role="option"
                                    class="harness-provider-option"
                                    class:harness-provider-option--active=move || i18n.locale().get() == loc
                                    aria-selected=move || if i18n.locale().get() == loc { "true" } else { "false" }
                                    on:click=move |_| choose(loc)
                                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                                        match ev.key().as_str() {
                                            "ArrowDown" => {
                                                ev.prevent_default();
                                                focus_locale_option(next_app_locale(loc));
                                            }
                                            "ArrowUp" => {
                                                ev.prevent_default();
                                                focus_locale_option(prev_app_locale(loc));
                                            }
                                            "Enter" | " " => {
                                                ev.prevent_default();
                                                choose(loc);
                                            }
                                            "Escape" => {
                                                ev.prevent_default();
                                                open.set(false);
                                            }
                                            _ => {}
                                        }
                                    }
                                >
                                    <span class="harness-provider-option__brand">
                                        <img
                                            class="harness-provider-option__img"
                                            src=loc.flag_icon_url()
                                            alt=""
                                            loading="lazy"
                                        />
                                    </span>
                                    <span>{label}</span>
                                </button>
                            }
                        }}
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn AppSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let prefs = expect_context::<AppPrefsService>();
    let updates = expect_context::<UpdateService>();
    let voice_settings = RwSignal::new(Option::<VoiceSettings>::None);
    let ptt_recording = RwSignal::new(false);

    if is_tauri_shell() {
        leptos::task::spawn_local(async move {
            if let Ok(v) = voice_settings_get().await {
                voice_settings.set(Some(v));
            }
        });
    }

    let save_voice = move |patch: VoiceSettings| {
        if !is_tauri_shell() {
            voice_settings.set(Some(patch));
            return;
        }
        leptos::task::spawn_local(async move {
            if let Ok(v) = voice_settings_save(patch).await {
                voice_settings.set(Some(v));
            }
        });
    };

    view! {
        <article class="harness-pane app-settings-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuLayoutDashboard width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::AppHeading)()}</span>
            </h3>
            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuLanguages width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AppLanguage)()}</span>
                </span>
                <LocalePicker />
                <VoiceSttLanguageControls settings=voice_settings save=save_voice />
            </label>
            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuKeyboard width="0.82rem" height="0.82rem" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::AppShortcutHeading)()}</span>
                </h4>
                <VoicePttControls
                    settings=voice_settings
                    recording=ptt_recording
                    save=save_voice
                />
            </section>
            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuBell width="0.82rem" height="0.82rem" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::AppNotifHeading)()}</span>
                </h4>
                <div class="app-prefs-toggle-grid">
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-toggle">
                            <input
                                type="checkbox"
                                prop:checked=move || prefs.success_toast_enabled().get()
                                on:change=move |ev| {
                                    if let Some(checked) = checkbox_checked(&ev) {
                                        prefs.set_success_toast(checked);
                                    }
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::AppNotifToasts)()}</span>
                        </label>
                        <p class="app-prefs-hint">{move || i18n.tr(I18nKey::AppNotifToastsHint)()}</p>
                    </div>
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-toggle">
                            <input
                                type="checkbox"
                                prop:checked=move || prefs.success_sound_enabled().get()
                                on:change=move |ev| {
                                    if let Some(checked) = checkbox_checked(&ev) {
                                        prefs.set_success_sound(checked);
                                    }
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::AppNotifSound)()}</span>
                        </label>
                        <p class="app-prefs-hint">{move || i18n.tr(I18nKey::AppNotifSoundHint)()}</p>
                    </div>
                </div>
            </section>
            <section class="harness-subpane">
                <AgentHooksPanel />
            </section>
            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuRefreshCw width="0.82rem" height="0.82rem" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::AppUpdateHeading)()}</span>
                </h4>
                <label class="app-prefs-toggle">
                    <input
                        type="checkbox"
                        prop:checked=move || prefs.update_auto_check_enabled().get()
                        on:change=move |ev| {
                            if let Some(checked) = checkbox_checked(&ev) {
                                prefs.set_update_auto_check(checked);
                            }
                        }
                    />
                    <span>{move || i18n.tr(I18nKey::AppUpdateAutoCheck)()}</span>
                </label>
                <p class="app-prefs-hint">{move || i18n.tr(I18nKey::AppUpdateAutoCheckHint)()}</p>
                <dl class="app-prefs-version">
                    <div class="app-prefs-version__row">
                        <dt>
                            <span class="harness-field-label__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuBadgeInfo width="0.82rem" height="0.82rem" />
                            </span>
                            <span>{move || i18n.tr(I18nKey::AppUpdateCurrentVersion)()}</span>
                        </dt>
                        <dd>{move || {
                            let v = updates.current_version().get();
                            if v.is_empty() { "—".to_string() } else { v }
                        }}</dd>
                    </div>
                    <Show when=move || updates.available_version().get().is_some()>
                        <div class="app-prefs-version__row app-prefs-version__row--available">
                            <dt>
                                <span class="harness-field-label__icon" aria-hidden="true">
                                    <LxIcon icon=icondata::LuCircleArrowUp width="0.82rem" height="0.82rem" />
                                </span>
                                <span>{move || i18n.tr(I18nKey::AppUpdateAvailableVersion)()}</span>
                            </dt>
                            <dd>{move || updates.available_version().get().unwrap_or_default()}</dd>
                        </div>
                    </Show>
                </dl>
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    on:click=move |_| updates.check_manual()
                    prop:disabled=move || updates.status().get() == UpdateUiStatus::Checking
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSearch width="0.82rem" height="0.82rem" />
                        <span>{move || {
                            if updates.status().get() == UpdateUiStatus::Checking {
                                i18n.tr(I18nKey::AppUpdateChecking)()
                            } else {
                                i18n.tr(I18nKey::AppUpdateCheck)()
                            }
                        }}</span>
                    </span>
                </button>
                <p class="app-prefs-hint">
                    {move || match updates.status().get() {
                        UpdateUiStatus::UpToDate => i18n.tr(I18nKey::AppUpdateUpToDate)().to_string(),
                        UpdateUiStatus::DevUnavailable => i18n.tr(I18nKey::AppUpdateDevUnavailable)().to_string(),
                        UpdateUiStatus::Error => updates.message().get().unwrap_or_default(),
                        _ => String::new(),
                    }}
                </p>
            </section>
        </article>
    }
}

#[component]
fn ApiKeysSettingsPane() -> impl IntoView {
    view! { <crate::workbench::ApiKeysPane /> }
}

fn hook_brand_icon(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("/public/brand-icons/anthropic.svg"),
        "codex" => Some("/public/brand-icons/openai.svg"),
        "gemini" => Some("/public/brand-icons/gemini.svg"),
        "cursor" => Some("/public/brand-icons/cursor.svg"),
        _ => None,
    }
}

#[component]
fn AgentHooksPanel() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let report: RwSignal<Option<AgentHooksReport>> = RwSignal::new(None);
    let busy = RwSignal::new(false);
    let error: RwSignal<Option<String>> = RwSignal::new(None);

    let refresh = move || {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match agent_hooks_status().await {
                Ok(r) => {
                    report.set(Some(r));
                    error.set(None);
                }
                Err(e) => error.set(Some(e)),
            }
        });
    };

    Effect::new(move |_| refresh());

    let on_install = move |_| {
        if busy.get_untracked() || !is_tauri_shell() {
            return;
        }
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match install_agent_hooks().await {
                Ok(r) => report.set(Some(r)),
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    let on_uninstall = move |_| {
        if busy.get_untracked() || !is_tauri_shell() {
            return;
        }
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match uninstall_agent_hooks().await {
                Ok(r) => report.set(Some(r)),
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };

    view! {
        <section class="harness-hooks">
            <h4 class="harness-pane-subhead">
                <span class="harness-pane-subhead__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuTerminal width="0.9rem" height="0.9rem" />
                </span>
                <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgHooksHeading)()}</span>
            </h4>
            <p class="harness-muted">{move || i18n.tr(I18nKey::AgHooksDesc)()}</p>
            <ul class="harness-hooks__list harness-hooks__list--grid" role="list">
                {move || {
                    let rendered = report.get();
                    let installed_label = i18n.tr(I18nKey::AgHooksStatusInstalled)().to_string();
                    let missing_label = i18n.tr(I18nKey::AgHooksStatusMissing)().to_string();
                    let unknown_label = i18n.tr(I18nKey::AgHooksStatusUnknown)().to_string();
                    match rendered {
                        Some(r) if !r.entries.is_empty() => r
                            .entries
                            .into_iter()
                            .map(|entry| {
                                let status = if entry.installed {
                                    installed_label.clone()
                                } else {
                                    missing_label.clone()
                                };
                                let note = entry.note.unwrap_or_default();
                                let has_note = !note.is_empty();
                                let icon_url = hook_brand_icon(&entry.agent);
                                let status_tip = status.clone();
                                view! {
                                    <li class="harness-hooks__item">
                                        <div class="harness-hooks__main">
                                            <span class="harness-hooks__brand">
                                                <Show
                                                    when=move || icon_url.is_some()
                                                    fallback=move || view! {
                                                        <span class="harness-hooks__fallback">
                                                            <LxIcon icon=icondata::LuTerminal width="0.9rem" height="0.9rem" />
                                                        </span>
                                                    }
                                                >
                                                    <img
                                                        class="harness-hooks__img"
                                                        src=move || icon_url.unwrap_or("")
                                                        alt=""
                                                    />
                                                </Show>
                                            </span>
                                            <div class="harness-hooks__copy">
                                                <div class="harness-hooks__title-row">
                                                    <strong class="harness-hooks__name">{entry.agent}</strong>
                                                    <span
                                                        class="harness-hooks__status"
                                                        class:harness-hooks__status--ok=entry.installed
                                                        title=status_tip.clone()
                                                        aria-label=status_tip
                                                    >
                                                        <LxIcon
                                                            icon=if entry.installed {
                                                                icondata::LuCheck
                                                            } else {
                                                                icondata::LuX
                                                            }
                                                            width="0.82rem"
                                                            height="0.82rem"
                                                        />
                                                    </span>
                                                </div>
                                                <Show when=move || has_note>
                                                    <small class="harness-muted">{note.clone()}</small>
                                                </Show>
                                            </div>
                                        </div>
                                    </li>
                                }
                                .into_any()
                            })
                            .collect::<Vec<_>>()
                            .into_any(),
                        _ => view! {
                            <li class="harness-hooks__item harness-muted">{unknown_label}</li>
                        }
                        .into_any(),
                    }
                }}
            </ul>
            <div class="harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    prop:disabled=move || busy.get() || !is_tauri_shell()
                    on:click=on_install
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuDownload width="0.78rem" height="0.78rem" />
                        <span>{move || {
                            if busy.get() {
                                i18n.tr(I18nKey::AgHooksBusy)().to_string()
                            } else {
                                i18n.tr(I18nKey::AgHooksInstall)().to_string()
                            }
                        }}</span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    prop:disabled=move || busy.get() || !is_tauri_shell()
                    on:click=on_uninstall
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::AgHooksUninstall)()}</span>
                    </span>
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="harness-muted">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </section>
    }
}
