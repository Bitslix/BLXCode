//! Befehlspalette und Harness‑Einstellungen (kategorisiert).
//!
//! Tastenkürzel (tmux-Standard: `Ctrl+b` + zweite Taste; Legacy in App-Einstellungen)
//! sind im Haupt-Webview gebunden ([`HarnessHost`] → [`super::harness_chords`]).
use super::app_prefs::{AppPrefsService, ShortcutMode};
use super::browser_tab::sync_embedded_browser_layer;
use super::harness_chords::handle_harness_keydown;
use super::state::{
    workspace_entry_has_folder, BrowserEmbedSurface, HarnessSettingsCategory, HarnessUiService,
    MemoryColorPreset, RecentWorkspaceItem, RightPanelTab, WorkbenchService,
};
use super::update_service::{UpdateService, UpdateUiStatus};
use crate::config::HARNESS_BROWSER_DEFAULT_URL;
use crate::i18n::{lookup, I18nKey, Locale, APP_LOCALES};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_api_key_delete, agent_api_key_set, agent_hooks_status, agent_provider_models,
    agent_settings_get, agent_settings_save, agent_web_api_key_delete, agent_web_api_key_set,
    agent_web_settings_get, agent_web_settings_save, install_agent_hooks, is_tauri_shell,
    uninstall_agent_hooks, AgentHooksReport, AgentProviderKind, AgentProviderSettingsView,
    AgentWebSettingsView, ProviderModelEntry, ProviderModelsResponse, ThinkingLevel, WebKeyStatus,
    WebProviderKind,
};
use gloo_timers::future::TimeoutFuture;
use js_sys::Date;
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

fn select_str(ev: &web_sys::Event) -> Option<String> {
    ev.target()?
        .dyn_into::<web_sys::HtmlSelectElement>()
        .ok()
        .map(|i| i.value())
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
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::ApiKeys label=I18nKey::HsCatApiKeys />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Workspace label=I18nKey::HsCatWorkspace />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::AgentProvider label=I18nKey::HsCatProvider />
                <HarnessCatStaticBtn ui=ui cat=HarnessSettingsCategory::Memory label="Memory" />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Voice label=I18nKey::HsCatVoice />
                <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Image label=I18nKey::HsCatImage />
            </nav>

            <div class="harness-settings-detail">
                {move || match ui.settings_category().get() {
                    HarnessSettingsCategory::App => view! {
                        <AppSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::Appearance => view! {
                        <AppearanceSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::ApiKeys => view! {
                        <ApiKeysSettingsPane />
                    }.into_any(),
                    HarnessSettingsCategory::Workspace => view! {
                        <WorkspaceSettingsPane ui=ui wb=wb embed=embed />
                    }.into_any(),
                    HarnessSettingsCategory::AgentProvider => view! {
                        <AgentProviderPane />
                    }.into_any(),
                    HarnessSettingsCategory::Memory => view! {
                        <MemorySettingsPane wb=wb />
                    }.into_any(),
                    HarnessSettingsCategory::Voice => view! {
                        <crate::workbench::harness_voice_pane::VoicePane />
                    }.into_any(),
                    HarnessSettingsCategory::Image => view! {
                        <crate::workbench::harness_image_pane::ImagePane />
                    }.into_any(),
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

#[component]
fn HarnessCatStaticBtn(
    ui: HarnessUiService,
    cat: HarnessSettingsCategory,
    label: &'static str,
) -> impl IntoView {
    let icon = harness_settings_cat_icon(cat);
    view! {
        <button
            type="button"
            class:harness-cat-active=move || ui.settings_category().get() == cat
            class="harness-cat-btn"
            on:click=move |_| ui.settings_category().set(cat)
        >
            <span class="harness-cat-btn__icon" aria-hidden="true">
                <LxIcon icon=icon width="0.92rem" height="0.92rem" />
            </span>
            <span class="harness-cat-btn__label">{label}</span>
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
    let ui = expect_context::<HarnessUiService>();
    let updates = expect_context::<UpdateService>();
    view! {
        <article class="harness-pane">
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
            </label>
            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuKeyboard width="0.82rem" height="0.82rem" />
                    </span>
                    <span>{move || i18n.tr(I18nKey::AppShortcutHeading)()}</span>
                </h4>
                <div class="app-prefs-toggle-grid">
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="shortcut-mode"
                                prop:checked=move || prefs.shortcut_mode().get() == ShortcutMode::Tmux
                                on:change=move |_| {
                                    prefs.set_shortcut_mode(ShortcutMode::Tmux);
                                    ui.clear_prefix();
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::AppShortcutModeTmux)()}</span>
                        </label>
                    </div>
                    <div class="app-prefs-toggle-cell">
                        <label class="app-prefs-radio">
                            <input
                                type="radio"
                                name="shortcut-mode"
                                prop:checked=move || prefs.shortcut_mode().get() == ShortcutMode::Legacy
                                on:change=move |_| {
                                    prefs.set_shortcut_mode(ShortcutMode::Legacy);
                                    ui.clear_prefix();
                                }
                            />
                            <span>{move || i18n.tr(I18nKey::AppShortcutModeLegacy)()}</span>
                        </label>
                    </div>
                </div>
                <p class="app-prefs-hint">{move || i18n.tr(I18nKey::AppShortcutModeHint)()}</p>
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
fn AppearanceSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <article class="harness-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuSunMoon width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::AppearanceHeading)()}</span>
            </h3>
        </article>
    }
}

#[component]
fn ApiKeysSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <article class="harness-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuKeyRound width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::ApiKeysHeading)()}</span>
            </h3>
        </article>
    }
}

#[component]
fn MemorySettingsPane(wb: WorkbenchService) -> impl IntoView {
    let new_label = RwSignal::new(String::new());
    let new_color = RwSignal::new("#7dd3fc".to_string());

    view! {
        <article class="harness-pane memory-settings-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuPalette width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">"Memory"</span>
            </h3>
            <section class="harness-subpane memory-presets">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuPalette width="0.82rem" height="0.82rem" />
                    </span>
                    <span>"Color presets"</span>
                </h4>
                <ol class="memory-preset-list">
                    <For
                        each=move || wb.memory_color_presets().get()
                        key=|preset| preset.id.clone()
                        children=move |preset: MemoryColorPreset| {
                            let id_for_label = preset.id.clone();
                            let id_for_color = preset.id.clone();
                            let id_for_delete = preset.id.clone();
                            view! {
                                <li class="memory-preset-row">
                                    <input
                                        type="color"
                                        class="memory-preset-row__color"
                                        prop:value=preset.color.clone()
                                        on:input=move |ev| {
                                            if let Some(value) = harness_input_value(ev) {
                                                update_memory_preset(wb, &id_for_color, None, Some(value));
                                            }
                                        }
                                    />
                                    <input
                                        type="text"
                                        class="workbench-plain-input memory-preset-row__name"
                                        prop:value=preset.label.clone()
                                        on:input=move |ev| {
                                            if let Some(value) = harness_input_value(ev) {
                                                update_memory_preset(wb, &id_for_label, Some(value), None);
                                            }
                                        }
                                    />
                                    <button
                                        type="button"
                                        class="workbench-mini-btn memory-preset-row__delete"
                                        title="Delete preset"
                                        aria-label="Delete preset"
                                        on:click=move |_| {
                                            let mut presets = wb.memory_color_presets().get_untracked();
                                            presets.retain(|preset| preset.id != id_for_delete);
                                            wb.set_memory_color_presets(presets);
                                        }
                                    >
                                        <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
                                    </button>
                                </li>
                            }
                        }
                    />
                </ol>
                <form
                    class="memory-preset-add"
                    on:submit=move |ev: web_sys::SubmitEvent| {
                        ev.prevent_default();
                        let name = new_label.get_untracked().trim().to_string();
                        if name.is_empty() {
                            return;
                        }
                        let mut presets = wb.memory_color_presets().get_untracked();
                        presets.push(MemoryColorPreset {
                            id: format!("custom-{}", Date::now() as i64),
                            label: name,
                            color: normalize_settings_color(&new_color.get_untracked()),
                        });
                        wb.set_memory_color_presets(presets);
                        new_label.set(String::new());
                    }
                >
                    <input
                        type="color"
                        class="memory-preset-row__color"
                        prop:value=move || new_color.get()
                        on:input=move |ev| {
                            if let Some(value) = harness_input_value(ev) {
                                new_color.set(value);
                            }
                        }
                    />
                    <input
                        type="text"
                        class="workbench-plain-input"
                        placeholder="Preset name"
                        prop:value=move || new_label.get()
                        on:input=move |ev| {
                            if let Some(value) = harness_input_value(ev) {
                                new_label.set(value);
                            }
                        }
                    />
                    <button type="submit" class="workbench-mini-btn workbench-mini-btn--primary">
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuPlus width="0.78rem" height="0.78rem" />
                            <span>"Add"</span>
                        </span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        on:click=move |_| wb.reset_memory_color_presets()
                    >"Reset"</button>
                </form>
            </section>
        </article>
    }
}

fn harness_input_value(ev: web_sys::Event) -> Option<String> {
    ev.target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
}

fn update_memory_preset(
    wb: WorkbenchService,
    id: &str,
    label: Option<String>,
    color: Option<String>,
) {
    let mut presets = wb.memory_color_presets().get_untracked();
    if let Some(preset) = presets.iter_mut().find(|preset| preset.id == id) {
        if let Some(label) = label {
            preset.label = label;
        }
        if let Some(color) = color {
            preset.color = normalize_settings_color(&color);
        }
    }
    wb.set_memory_color_presets(presets);
}

fn normalize_settings_color(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
    {
        trimmed.to_ascii_lowercase()
    } else {
        "#7dd3fc".into()
    }
}

#[component]
fn WorkspaceSettingsPane(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <article class="harness-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuFolderOpen width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::WsHeading)()}</span>
            </h3>
            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuFolderGit2 width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::WsDefaultProjectDirLabel)()}</span>
                </span>
                <input
                    class="workbench-plain-input"
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::WsDefaultProjectDirPlaceholder)()
                    prop:value=move || wb.default_project_dir().get()
                    on:input=move |ev| {
                        if let Some(txt) = input_str(&ev) {
                            wb.set_default_project_dir_text(txt);
                        }
                    }
                />
                <small class="harness-muted">
                    {move || i18n.tr(I18nKey::WsDefaultProjectDirHint)()}
                </small>
            </label>
            <div class="harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    on:click=move |_| {
                        let trimmed = wb.default_project_dir().get_untracked().trim().to_owned();
                        wb.persist_default_project_dir(trimmed);
                    }
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::BtnSave)()}</span>
                    </span>
                </button>
            </div>
            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuFolderTree width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::WsRootLabel)()}</span>
                </span>
                <input
                    class="workbench-plain-input"
                    type="text"
                    placeholder=move || i18n.tr(I18nKey::WsRootPlaceholder)()
                    prop:value=move || wb.harness_workspace_root().get()
                    on:input=move |ev| {
                        if let Some(txt) = input_str(&ev) {
                            wb.set_harness_workspace_root_text(txt);
                        }
                    }
                />
                <small class="harness-muted">
                    {move || i18n.tr(I18nKey::WsRootHint)()}
                </small>
            </label>
            <div class="harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    on:click=move |_| {
                        let trimmed = wb.harness_workspace_root().get_untracked().trim().to_owned();
                        wb.persist_harness_workspace_root(trimmed);
                        let w = wb;
                        let surf = embed;
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(8).await;
                            sync_embedded_browser_layer(w, surf).await;
                        });
                    }
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::BtnSave)()}</span>
                    </span>
                </button>
            </div>
            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuGlobe width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::LayBrowserUrl)()}</span>
                </span>
                <input
                    class="workbench-plain-input"
                    type="url"
                    prop:value=move || wb.browser_url().get()
                    on:input=move |ev| {
                        if let Some(txt) = input_str(&ev) {
                            wb.set_browser_url_text(txt);
                        }
                    }
                />
            </label>
            <div class="harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    on:click=move |_| persist_browser_defaults(wb, ui, embed)
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::BtnApply)()}</span>
                    </span>
                </button>
                <small class="harness-muted">
                    {move || format!("{} {}", i18n.tr(I18nKey::WsBrowserDefault)(), HARNESS_BROWSER_DEFAULT_URL)}
                </small>
            </div>
        </article>
    }
}

fn provider_label(i18n: &I18nService, provider: AgentProviderKind) -> String {
    let key = match provider {
        AgentProviderKind::Openrouter => I18nKey::AgProviderOpenrouter,
        AgentProviderKind::Anthropic => I18nKey::AgProviderAnthropic,
        AgentProviderKind::Openai => I18nKey::AgProviderOpenai,
    };
    i18n.tr(key)().to_string()
}

fn provider_icon_url(provider: AgentProviderKind) -> &'static str {
    match provider {
        AgentProviderKind::Openrouter => "/public/brand-icons/openrouter.svg",
        AgentProviderKind::Anthropic => "/public/brand-icons/anthropic.svg",
        AgentProviderKind::Openai => "/public/brand-icons/openai.svg",
    }
}

fn thinking_levels() -> [ThinkingLevel; 5] {
    [
        ThinkingLevel::Off,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Max,
    ]
}

fn thinking_label(i18n: &I18nService, level: ThinkingLevel) -> String {
    let key = match level {
        ThinkingLevel::Off => I18nKey::AgThinkingOff,
        ThinkingLevel::Low => I18nKey::AgThinkingLow,
        ThinkingLevel::Medium => I18nKey::AgThinkingMedium,
        ThinkingLevel::High => I18nKey::AgThinkingHigh,
        ThinkingLevel::Max => I18nKey::AgThinkingMax,
    };
    i18n.tr(key)().to_string()
}

fn provider_key_configured(view: &AgentProviderSettingsView, provider: AgentProviderKind) -> bool {
    view.key_statuses
        .iter()
        .find(|status| status.provider == provider)
        .map(|status| status.configured)
        .unwrap_or(false)
}

fn provider_key_mask(
    view: &AgentProviderSettingsView,
    provider: AgentProviderKind,
) -> Option<String> {
    view.key_statuses
        .iter()
        .find(|status| status.provider == provider)
        .and_then(|status| status.masked_value.clone())
}

fn provider_key_status_text(
    i18n: &I18nService,
    view: &AgentProviderSettingsView,
    provider: AgentProviderKind,
) -> String {
    if provider_key_configured(view, provider) {
        if let Some(mask) = provider_key_mask(view, provider) {
            format!("{} ({mask})", i18n.tr(I18nKey::AgApiKeyConfigured)())
        } else {
            i18n.tr(I18nKey::AgApiKeyConfigured)().to_string()
        }
    } else {
        i18n.tr(I18nKey::AgApiKeyMissing)().to_string()
    }
}

fn provider_cache(
    view: &AgentProviderSettingsView,
    provider: AgentProviderKind,
) -> Vec<ProviderModelEntry> {
    match provider {
        AgentProviderKind::Openrouter => view.model_cache_openrouter.clone(),
        AgentProviderKind::Anthropic => view.model_cache_anthropic.clone(),
        AgentProviderKind::Openai => view.model_cache_openai.clone(),
    }
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

fn focus_provider_option(provider: AgentProviderKind) {
    let id = format!("provider-option-{}", provider.as_str());
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

fn next_provider(provider: AgentProviderKind) -> AgentProviderKind {
    match provider {
        AgentProviderKind::Openrouter => AgentProviderKind::Anthropic,
        AgentProviderKind::Anthropic => AgentProviderKind::Openai,
        AgentProviderKind::Openai => AgentProviderKind::Openrouter,
    }
}

fn prev_provider(provider: AgentProviderKind) -> AgentProviderKind {
    match provider {
        AgentProviderKind::Openrouter => AgentProviderKind::Openai,
        AgentProviderKind::Anthropic => AgentProviderKind::Openrouter,
        AgentProviderKind::Openai => AgentProviderKind::Anthropic,
    }
}

#[component]
fn ProviderPicker(
    selected_provider: RwSignal<AgentProviderKind>,
    settings: RwSignal<Option<AgentProviderSettingsView>>,
    model_entries: RwSignal<Vec<ProviderModelEntry>>,
    provider_refresh_request: RwSignal<Option<AgentProviderKind>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let open = RwSignal::new(false);

    let choose = move |provider: AgentProviderKind| {
        selected_provider.set(provider);
        if let Some(view) = settings.get_untracked() {
            model_entries.set(provider_cache(&view, provider));
        }
        open.set(false);
        provider_refresh_request.set(Some(provider));
    };

    view! {
        <div class="harness-provider-picker">
            <button
                type="button"
                class="harness-provider-trigger"
                aria-haspopup="listbox"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                on:click=move |_| {
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        let provider = selected_provider.get_untracked();
                        leptos::task::spawn_local(async move {
                            TimeoutFuture::new(0).await;
                            focus_provider_option(provider);
                        });
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    match ev.key().as_str() {
                        "ArrowDown" | "Enter" | " " => {
                            ev.prevent_default();
                            open.set(true);
                            let provider = selected_provider.get_untracked();
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_provider_option(provider);
                            });
                        }
                        "ArrowUp" => {
                            ev.prevent_default();
                            open.set(true);
                            let provider = prev_provider(selected_provider.get_untracked());
                            leptos::task::spawn_local(async move {
                                TimeoutFuture::new(0).await;
                                focus_provider_option(provider);
                            });
                        }
                        "Escape" => open.set(false),
                        _ => {}
                    }
                }
            >
                <span class="harness-provider-trigger__main">
                    <span class="harness-provider-trigger__brand">
                        <img class="harness-provider-trigger__img" src=move || provider_icon_url(selected_provider.get()) alt="" />
                    </span>
                    <span>{move || provider_label(&i18n, selected_provider.get())}</span>
                </span>
                <span class="harness-provider-trigger__caret">"▾"</span>
            </button>

            <Show when=move || open.get()>
                <div class="harness-provider-menu" role="listbox">
                    {move || {
                        [AgentProviderKind::Openrouter, AgentProviderKind::Anthropic, AgentProviderKind::Openai]
                            .into_iter()
                            .map(|provider| {
                                view! {
                                    <button
                                        id=format!("provider-option-{}", provider.as_str())
                                        type="button"
                                        role="option"
                                        class="harness-provider-option"
                                        class:harness-provider-option--active=move || selected_provider.get() == provider
                                        aria-selected=move || if selected_provider.get() == provider { "true" } else { "false" }
                                        on:click=move |_| choose(provider)
                                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                                            match ev.key().as_str() {
                                                "ArrowDown" => {
                                                    ev.prevent_default();
                                                    focus_provider_option(next_provider(provider));
                                                }
                                                "ArrowUp" => {
                                                    ev.prevent_default();
                                                    focus_provider_option(prev_provider(provider));
                                                }
                                                "Enter" | " " => {
                                                    ev.prevent_default();
                                                    choose(provider);
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
                                            <img class="harness-provider-option__img" src=provider_icon_url(provider) alt="" />
                                        </span>
                                        <span>{provider_label(&i18n, provider)}</span>
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

#[component]
fn AgentProviderPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let settings: RwSignal<Option<AgentProviderSettingsView>> = RwSignal::new(None);
    let selected_provider = RwSignal::new(AgentProviderKind::Openrouter);
    let custom_model = RwSignal::new(String::new());
    let thinking_level = RwSignal::new(ThinkingLevel::Medium);
    let api_key_input = RwSignal::new(String::new());
    let model_entries: RwSignal<Vec<ProviderModelEntry>> = RwSignal::new(Vec::new());
    let models_source = RwSignal::new(String::new());
    let models_message: RwSignal<Option<String>> = RwSignal::new(None);
    let busy = RwSignal::new(false);
    let loading_models = RwSignal::new(false);
    let status_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let provider_refresh_request: RwSignal<Option<AgentProviderKind>> = RwSignal::new(None);
    let web_settings: RwSignal<Option<AgentWebSettingsView>> = RwSignal::new(None);
    let web_provider = RwSignal::new(WebProviderKind::None);
    let web_tavily_key = RwSignal::new(String::new());
    let web_brave_key = RwSignal::new(String::new());
    let web_status_msg: RwSignal<Option<String>> = RwSignal::new(None);

    let apply_web_settings = move |view: AgentWebSettingsView| {
        web_provider.set(view.settings.provider);
        web_settings.set(Some(view));
    };

    let apply_settings = move |view: AgentProviderSettingsView| {
        selected_provider.set(view.provider);
        custom_model.set(view.model_id.clone());
        thinking_level.set(view.thinking_level);
        model_entries.set(provider_cache(&view, view.provider));
        settings.set(Some(view));
    };

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match agent_settings_get().await {
                Ok(view) => {
                    error_msg.set(None);
                    status_msg.set(None);
                    apply_settings(view);
                }
                Err(err) => error_msg.set(Some(err)),
            }
            match agent_web_settings_get().await {
                Ok(view) => {
                    web_status_msg.set(None);
                    apply_web_settings(view);
                }
                Err(err) => error_msg.set(Some(err)),
            }
        });
    });

    let refresh_models = move |provider: AgentProviderKind| {
        loading_models.set(true);
        models_message.set(None);
        error_msg.set(None);
        leptos::task::spawn_local(async move {
            match agent_provider_models(provider).await {
                Ok(ProviderModelsResponse {
                    provider: _,
                    entries,
                    source,
                    used_fallback,
                    message,
                }) => {
                    model_entries.set(entries);
                    models_source.set(source);
                    models_message.set(message.or_else(|| {
                        if used_fallback {
                            Some(i18n.tr(I18nKey::AgModelsFallback)().to_string())
                        } else {
                            None
                        }
                    }));
                }
                Err(err) => error_msg.set(Some(err)),
            }
            loading_models.set(false);
        });
    };

    Effect::new(move |_| {
        let Some(provider) = provider_refresh_request.get() else {
            return;
        };
        provider_refresh_request.set(None);
        refresh_models(provider);
    });

    view! {
        <article class="harness-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuCpu width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::AgProviderHeading)()}</span>
            </h3>
            <div class="harness-provider-grid">
                <div class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuPlug width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgProviderField)()}</span>
                    </span>
                    <ProviderPicker
                        selected_provider=selected_provider
                        settings=settings
                        model_entries=model_entries
                        provider_refresh_request=provider_refresh_request
                    />
                </div>

                <label class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuGauge width="0.82rem" height="0.82rem" />
                        </span>
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgThinkingField)()}</span>
                    </span>
                    <select
                        class="workbench-plain-input"
                        prop:value=move || format!("{:?}", thinking_level.get()).to_ascii_lowercase()
                        on:change=move |ev| {
                            if let Some(value) = select_str(&ev) {
                                let level = match value.as_str() {
                                    "off" => ThinkingLevel::Off,
                                    "low" => ThinkingLevel::Low,
                                    "high" => ThinkingLevel::High,
                                    "max" => ThinkingLevel::Max,
                                    _ => ThinkingLevel::Medium,
                                };
                                thinking_level.set(level);
                            }
                        }
                    >
                        {move || {
                            thinking_levels()
                                .into_iter()
                                .map(|level| {
                                    let value = format!("{:?}", level).to_ascii_lowercase();
                                    view! { <option value=value>{thinking_label(&i18n, level)}</option> }
                                })
                                .collect_view()
                        }}
                    </select>
                </label>
            </div>

            <label class="harness-stack">
                <span class="harness-field-label">
                    <span class="harness-field-label__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuPackage2 width="0.82rem" height="0.82rem" />
                    </span>
                    <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgModelField)()}</span>
                </span>
                <input
                    class="workbench-plain-input"
                    type="text"
                    list="blxcode-agent-models"
                    prop:value=move || custom_model.get()
                    on:input=move |ev| {
                        if let Some(value) = input_str(&ev) {
                            custom_model.set(value);
                        }
                    }
                />
                <datalist id="blxcode-agent-models">
                    {move || {
                        model_entries
                            .get()
                            .into_iter()
                            .map(|entry| view! { <option value=entry.id.clone()></option> })
                            .collect_view()
                    }}
                </datalist>
            </label>

            <div class="harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn"
                    prop:disabled=move || loading_models.get() || !is_tauri_shell()
                    on:click=move |_| refresh_models(selected_provider.get_untracked())
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuRefreshCw width="0.78rem" height="0.78rem" />
                        <span>{move || if loading_models.get() {
                            i18n.tr(I18nKey::AgModelsLoading)().to_string()
                        } else {
                            i18n.tr(I18nKey::AgModelsRefresh)().to_string()
                        }}</span>
                    </span>
                </button>
                <small class="harness-muted">
                    {move || match models_source.get().as_str() {
                        "live" => i18n.tr(I18nKey::AgModelsSourceLive)().to_string(),
                        "cache" => i18n.tr(I18nKey::AgModelsSourceCache)().to_string(),
                        "curated" | "fallback" => i18n.tr(I18nKey::AgModelsSourceCurated)().to_string(),
                        _ => String::new(),
                    }}
                </small>
            </div>

            <Show when=move || models_message.get().is_some()>
                <p class="harness-muted">{move || models_message.get().unwrap_or_default()}</p>
            </Show>

            <section class="harness-subpane">
                <div class="harness-row-gap harness-row-gap--space">
                    <h4 class="harness-pane-subhead">
                        <span class="harness-pane-subhead__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuKeyRound width="0.9rem" height="0.9rem" />
                        </span>
                        <span class="harness-pane-subhead__text">
                            {move || format!("{} {}", i18n.tr(I18nKey::AgApiKeyField)(), provider_label(&i18n, selected_provider.get()))}
                        </span>
                    </h4>
                    <small class="harness-muted">
                        {move || {
                            settings
                                .get()
                                .map(|view| provider_key_status_text(&i18n, &view, selected_provider.get()))
                                .unwrap_or_else(|| i18n.tr(I18nKey::AgApiKeyMissing)().to_string())
                        }}
                    </small>
                </div>
                <p class="harness-muted">{move || i18n.tr(I18nKey::AgApiKeyHint)()}</p>
                <label class="harness-stack">
                    <input
                        class="workbench-plain-input"
                        type="password"
                        autocomplete="off"
                        placeholder=move || {
                            settings
                                .get()
                                .and_then(|view| provider_key_mask(&view, selected_provider.get()))
                                .unwrap_or_default()
                        }
                        prop:value=move || api_key_input.get()
                        on:input=move |ev| {
                            if let Some(value) = input_str(&ev) {
                                api_key_input.set(value);
                            }
                        }
                    />
                </label>
                <div class="harness-row-gap">
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            let provider = selected_provider.get_untracked();
                            let api_key = api_key_input.get_untracked();
                            if api_key.trim().is_empty() {
                                status_msg.set(None);
                                error_msg.set(Some("API key is empty".into()));
                                return;
                            }
                            busy.set(true);
                            error_msg.set(None);
                            leptos::task::spawn_local(async move {
                                match agent_api_key_set(provider, api_key).await {
                                    Ok(view) => {
                                        if !provider_key_configured(&view, provider) {
                                            status_msg.set(None);
                                            error_msg.set(Some(format!(
                                                "API-Key speichern fehlgeschlagen: provider state for {} is still missing after save",
                                                provider.as_str()
                                            )));
                                            busy.set(false);
                                            return;
                                        }
                                        api_key_input.set(String::new());
                                        status_msg.set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string()));
                                        error_msg.set(None);
                                        apply_settings(view);
                                        refresh_models(provider);
                                    }
                                    Err(err) => {
                                        status_msg.set(None);
                                        error_msg.set(Some(format!("API-Key speichern fehlgeschlagen: {err}")));
                                    }
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuCheck width="0.78rem" height="0.78rem" />
                            <span>{move || i18n.tr(I18nKey::AgApiKeySet)()}</span>
                        </span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            let provider = selected_provider.get_untracked();
                            busy.set(true);
                            error_msg.set(None);
                            leptos::task::spawn_local(async move {
                                match agent_api_key_delete(provider).await {
                                    Ok(view) => {
                                        status_msg.set(None);
                                        error_msg.set(None);
                                        apply_settings(view);
                                    }
                                    Err(err) => {
                                        status_msg.set(None);
                                        error_msg.set(Some(format!("API-Key löschen fehlgeschlagen: {err}")));
                                    }
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
                            <span>{move || i18n.tr(I18nKey::AgApiKeyDelete)()}</span>
                        </span>
                    </button>
                </div>
            </section>

            <div class="harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    prop:disabled=move || busy.get() || !is_tauri_shell()
                    on:click=move |_| {
                        let provider = selected_provider.get_untracked();
                        let model_id = custom_model.get_untracked();
                        let level = thinking_level.get_untracked();
                        busy.set(true);
                        error_msg.set(None);
                        leptos::task::spawn_local(async move {
                            match agent_settings_save(provider, model_id, level).await {
                                Ok(view) => {
                                    status_msg.set(Some(i18n.tr(I18nKey::AgSaveProviderDone)().to_string()));
                                    apply_settings(view);
                                }
                                Err(err) => error_msg.set(Some(err)),
                            }
                            busy.set(false);
                        });
                    }
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::AgSaveProvider)()}</span>
                    </span>
                </button>
            </div>

            <section class="harness-subpane">
                <h4 class="harness-pane-subhead">
                    <span class="harness-pane-subhead__icon" aria-hidden="true">
                        <LxIcon icon=icondata::LuGlobe width="0.9rem" height="0.9rem" />
                    </span>
                    <span class="harness-pane-subhead__text">{move || i18n.tr(I18nKey::AgWebToolsHeading)()}</span>
                </h4>
                <p class="harness-muted">{move || i18n.tr(I18nKey::AgWebToolsDescription)()}</p>
                <label class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgWebProviderField)()}</span>
                    </span>
                    <select
                        class="workbench-plain-input"
                        prop:value=move || web_provider_kind_value(web_provider.get())
                        on:change=move |ev| {
                            if let Some(value) = select_str(&ev) {
                                web_provider.set(web_provider_from_value(&value));
                            }
                        }
                    >
                        <option value="none">{move || i18n.tr(I18nKey::AgWebProviderNone)()}</option>
                        <option value="tavily">{move || i18n.tr(I18nKey::AgWebProviderTavily)()}</option>
                        <option value="brave">{move || i18n.tr(I18nKey::AgWebProviderBrave)()}</option>
                    </select>
                </label>
                <p class="harness-muted">{move || i18n.tr(I18nKey::AgWebKeyHint)()}</p>
                <label class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgWebTavilyKey)()}</span>
                    </span>
                    <small class="harness-muted">
                        {move || web_key_status_text(&i18n, web_settings.get(), "tavily")}
                    </small>
                    <input
                        class="workbench-plain-input"
                        type="password"
                        autocomplete="off"
                        placeholder=move || web_key_mask(web_settings.get(), "tavily").unwrap_or_default()
                        prop:value=move || web_tavily_key.get()
                        on:input=move |ev| {
                            if let Some(value) = input_str(&ev) {
                                web_tavily_key.set(value);
                            }
                        }
                    />
                </label>
                <div class="harness-row-gap">
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            let key = web_tavily_key.get_untracked();
                            if key.trim().is_empty() {
                                error_msg.set(Some("API key is empty".into()));
                                return;
                            }
                            busy.set(true);
                            leptos::task::spawn_local(async move {
                                match agent_web_api_key_set("tavily", key).await {
                                    Ok(view) => {
                                        web_tavily_key.set(String::new());
                                        web_status_msg.set(Some(i18n.tr(I18nKey::AgWebSaveDone)().to_string()));
                                        apply_web_settings(view);
                                    }
                                    Err(err) => error_msg.set(Some(err)),
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span>{move || i18n.tr(I18nKey::AgWebKeySet)()}</span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            busy.set(true);
                            leptos::task::spawn_local(async move {
                                match agent_web_api_key_delete("tavily").await {
                                    Ok(view) => apply_web_settings(view),
                                    Err(err) => error_msg.set(Some(err)),
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span>{move || i18n.tr(I18nKey::AgWebKeyDelete)()}</span>
                    </button>
                </div>
                <label class="harness-stack">
                    <span class="harness-field-label">
                        <span class="harness-field-label__text">{move || i18n.tr(I18nKey::AgWebBraveKey)()}</span>
                    </span>
                    <small class="harness-muted">
                        {move || web_key_status_text(&i18n, web_settings.get(), "brave")}
                    </small>
                    <input
                        class="workbench-plain-input"
                        type="password"
                        autocomplete="off"
                        placeholder=move || web_key_mask(web_settings.get(), "brave").unwrap_or_default()
                        prop:value=move || web_brave_key.get()
                        on:input=move |ev| {
                            if let Some(value) = input_str(&ev) {
                                web_brave_key.set(value);
                            }
                        }
                    />
                </label>
                <div class="harness-row-gap">
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            let key = web_brave_key.get_untracked();
                            if key.trim().is_empty() {
                                error_msg.set(Some("API key is empty".into()));
                                return;
                            }
                            busy.set(true);
                            leptos::task::spawn_local(async move {
                                match agent_web_api_key_set("brave", key).await {
                                    Ok(view) => {
                                        web_brave_key.set(String::new());
                                        web_status_msg.set(Some(i18n.tr(I18nKey::AgWebSaveDone)().to_string()));
                                        apply_web_settings(view);
                                    }
                                    Err(err) => error_msg.set(Some(err)),
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span>{move || i18n.tr(I18nKey::AgWebKeySet)()}</span>
                    </button>
                    <button
                        type="button"
                        class="workbench-mini-btn"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            busy.set(true);
                            leptos::task::spawn_local(async move {
                                match agent_web_api_key_delete("brave").await {
                                    Ok(view) => apply_web_settings(view),
                                    Err(err) => error_msg.set(Some(err)),
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span>{move || i18n.tr(I18nKey::AgWebKeyDelete)()}</span>
                    </button>
                </div>
                <div class="harness-row-gap">
                    <button
                        type="button"
                        class="workbench-mini-btn workbench-mini-btn--primary"
                        prop:disabled=move || busy.get() || !is_tauri_shell()
                        on:click=move |_| {
                            let provider = web_provider.get_untracked();
                            busy.set(true);
                            leptos::task::spawn_local(async move {
                                match agent_web_settings_save(provider).await {
                                    Ok(view) => {
                                        web_status_msg.set(Some(i18n.tr(I18nKey::AgWebSaveDone)().to_string()));
                                        apply_web_settings(view);
                                    }
                                    Err(err) => error_msg.set(Some(err)),
                                }
                                busy.set(false);
                            });
                        }
                    >
                        <span class="harness-btn-inline">
                            <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                            <span>{move || i18n.tr(I18nKey::AgSaveProvider)()}</span>
                        </span>
                    </button>
                </div>
            </section>

            <Show when=move || web_status_msg.get().is_some()>
                <p class="harness-muted">{move || web_status_msg.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || status_msg.get().is_some()>
                <p class="harness-muted">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || error_msg.get().is_some()>
                <p class="harness-error-text">{move || error_msg.get().unwrap_or_default()}</p>
            </Show>
        </article>
    }
}

fn web_provider_kind_value(kind: WebProviderKind) -> &'static str {
    match kind {
        WebProviderKind::None => "none",
        WebProviderKind::Tavily => "tavily",
        WebProviderKind::Brave => "brave",
    }
}

fn web_provider_from_value(value: &str) -> WebProviderKind {
    match value {
        "tavily" => WebProviderKind::Tavily,
        "brave" => WebProviderKind::Brave,
        _ => WebProviderKind::None,
    }
}

fn web_key_entry<'a>(
    view: Option<&'a AgentWebSettingsView>,
    kind: &str,
) -> Option<&'a WebKeyStatus> {
    view?.key_statuses.iter().find(|k| k.kind == kind)
}

fn web_key_mask(view: Option<AgentWebSettingsView>, kind: &str) -> Option<String> {
    web_key_entry(view.as_ref(), kind).and_then(|k| k.masked_value.clone())
}

fn web_key_status_text(
    i18n: &I18nService,
    view: Option<AgentWebSettingsView>,
    kind: &str,
) -> String {
    match web_key_entry(view.as_ref(), kind) {
        Some(k) if k.configured => k
            .masked_value
            .clone()
            .unwrap_or_else(|| i18n.tr(I18nKey::AgApiKeyConfigured)().to_string()),
        _ => i18n.tr(I18nKey::AgApiKeyMissing)().to_string(),
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
                                                <strong class="harness-hooks__name">{entry.agent}</strong>
                                                <Show when=move || has_note>
                                                    <small class="harness-muted">{note.clone()}</small>
                                                </Show>
                                            </div>
                                        </div>
                                        <span
                                            class="harness-hooks__status"
                                            class:harness-hooks__status--ok=entry.installed
                                        >
                                            <LxIcon
                                                icon=if entry.installed { icondata::LuCheck } else { icondata::LuX }
                                                width="0.82rem"
                                                height="0.82rem"
                                            />
                                            <span>{status}</span>
                                        </span>
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

fn persist_browser_defaults(
    wb: WorkbenchService,
    ui: HarnessUiService,
    embed: BrowserEmbedSurface,
) {
    let mut trimmed = wb.browser_url().get_untracked().trim().to_owned();
    if trimmed.is_empty() {
        trimmed = HARNESS_BROWSER_DEFAULT_URL.into();
    }
    wb.persist_browser_url_from_input(trimmed.clone());
    let wclone = wb;
    let aid = wb.embedded_browser_active_id().get_untracked();
    leptos::task::spawn_local(async move {
        let _ = crate::tauri_bridge::browser_navigate(aid, trimmed.as_str()).await;
        TimeoutFuture::new(12).await;
        sync_embedded_browser_layer(wclone, embed).await;
    });
    ui.close_settings();
}
