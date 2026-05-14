//! Befehlspalette (`Ctrl+Shift+P`) und Harness‑Einstellungen (kategorisiert).
//!
//! Shortcut ist im Haupt-Webview gebunden ([`HarnessHost`]).
use super::browser_tab::sync_embedded_browser_layer;
use super::state::{
    BrowserEmbedSurface, HarnessSettingsCategory, HarnessUiService, RightPanelTab, WorkbenchService,
};
use crate::config::{EULA_STORAGE_KEY, HARNESS_BROWSER_DEFAULT_URL};
use crate::i18n::{lookup, I18nKey, Locale};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_hooks_status, agent_provider_status, install_agent_hooks, is_tauri_shell,
    uninstall_agent_hooks, AgentHooksReport,
};
use gloo_timers::future::TimeoutFuture;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy)]
enum PaletteAction {
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
}

const PALETTE_ROWS: &[PaletteRow] = &[
    PaletteRow {
        title: I18nKey::CmdSetTitle,
        subtitle: I18nKey::CmdSetSub,
        action: PaletteAction::OpenSettings,
    },
    PaletteRow {
        title: I18nKey::CmdRtpTitle,
        subtitle: I18nKey::CmdRtpSub,
        action: PaletteAction::ToggleRightPanel,
    },
    PaletteRow {
        title: I18nKey::CmdAgentTitle,
        subtitle: I18nKey::CmdAgentSub,
        action: PaletteAction::AgentTab,
    },
    PaletteRow {
        title: I18nKey::CmdBrowseTitle,
        subtitle: I18nKey::CmdBrowseSub,
        action: PaletteAction::BrowserTab,
    },
    PaletteRow {
        title: I18nKey::CmdMemoryTitle,
        subtitle: I18nKey::CmdMemorySub,
        action: PaletteAction::MemoryTab,
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

    Effect::new(move |_| {
        let handle = window_event_listener_untyped("keydown", move |ev| {
            let Ok(ke) = ev.dyn_into::<web_sys::KeyboardEvent>() else {
                return;
            };

            let blocked = ui.palette_open().get_untracked() || ui.settings_open().get_untracked();
            let ctrl_or_meta = ke.ctrl_key() || ke.meta_key();
            let key = ke.key();

            if ctrl_or_meta && ke.shift_key() {
                match key.as_str() {
                    "p" | "P" => {
                        ke.prevent_default();
                        ui.toggle_command_palette();
                        return;
                    }
                    "a" | "A" | "b" | "B" | "m" | "M" if !blocked => {
                        ke.prevent_default();
                        let tab = match key.as_str() {
                            "a" | "A" => RightPanelTab::Agent,
                            "b" | "B" => RightPanelTab::Browser,
                            "m" | "M" => RightPanelTab::Memory,
                            _ => return,
                        };
                        if wb.right_collapsed().get_untracked() {
                            wb.toggle_right_panel();
                        }
                        wb.set_right_tab(tab);
                        defer_browser_bounds(wb, embed);
                        return;
                    }
                    _ => {}
                }
            }

            if ctrl_or_meta && !ke.shift_key() && !blocked {
                match key.as_str() {
                    "p" | "P" => {
                        ke.prevent_default();
                        wb.toggle_right_panel();
                        defer_browser_bounds(wb, embed);
                        return;
                    }
                    "o" | "O" => {
                        ke.prevent_default();
                        ui.open_command_palette();
                        return;
                    }
                    _ => {}
                }
            }

            if blocked && key.as_str() == "Escape" {
                ke.prevent_default();
                ui.close_command_palette();
                ui.close_settings();
            }
        });

        on_cleanup(move || handle.remove());
    });

    view! {
        <Show when=move || ui.palette_open().get()>
            <PaletteChrome ui=ui wb=wb embed=embed />
        </Show>
        <Show when=move || ui.settings_open().get()>
            <SettingsChrome ui=ui wb=wb embed=embed />
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
                <input
                    class="workbench-plain-input harness-filter"
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
                                <span class="harness-cmd-title">{move || i18n.tr(meta.title)()}</span>
                                <span class="harness-cmd-sub">{move || i18n.tr(meta.subtitle)()}</span>
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
        PaletteAction::OpenSettings => {
            ui.open_settings(HarnessSettingsCategory::General);
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

#[component]
fn SettingsChrome(
    ui: HarnessUiService,
    wb: WorkbenchService,
    embed: BrowserEmbedSurface,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    view! {
        <div class="harness-overlay harness-overlay--modal" role="presentation">
            <button
                type="button"
                class="harness-scrim"
                aria-label=move || i18n.tr(I18nKey::HsCloseSettingsAria)()
                on:click=move |_| ui.close_settings()
            ></button>

            <section class="harness-sheet harness-sheet--settings" role="dialog" aria-modal="true">
                <header class="harness-settings-head">
                    <h2 class="harness-settings-title">{move || i18n.tr(I18nKey::HsTitle)()}</h2>
                    <button type="button" class="workbench-mini-btn" on:click=move |_| ui.close_settings()>
                        {move || i18n.tr(I18nKey::BtnClose)()}
                    </button>
                </header>
                <div class="harness-settings-grid">
                    <nav class="harness-settings-cats" aria-label=move || i18n.tr(I18nKey::HsAriaCats)()>
                        <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::General label=I18nKey::HsCatGeneral />
                        <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Layout label=I18nKey::HsCatLayout />
                        <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Language label=I18nKey::HsCatLanguage />
                        <HarnessCatBtn ui=ui cat=HarnessSettingsCategory::Agent label=I18nKey::HsCatAgent />
                    </nav>

                    <div class="harness-settings-detail">
                        {move || match ui.settings_category().get() {
                            HarnessSettingsCategory::General => view! {
                                <article class="harness-pane">
                                    <h3>{move || i18n.tr(I18nKey::GenHeading)()}</h3>
                                    <p>{move || i18n.tr(I18nKey::GenApiNote)()}</p>
                                    <label class="harness-stack">
                                        <span>{move || i18n.tr(I18nKey::GenEulaStatus)()}</span>
                                        <input
                                            class="workbench-plain-input"
                                            type="text"
                                            prop:readonly=true
                                            prop:value=move || eula_preview(i18n.locale().get())
                                        />
                                    </label>
                                    <small class="harness-muted">{move || i18n.tr(I18nKey::GenMoreSoon)()}</small>
                                </article>
                            }
                            .into_any(),
                            HarnessSettingsCategory::Layout => view! {
                                <article class="harness-pane">
                                    <h3>{move || i18n.tr(I18nKey::LayHeading)()}</h3>
                                    <label class="harness-stack">
                                        <span>{move || i18n.tr(I18nKey::LayBrowserUrl)()}</span>
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
                                          {move || i18n.tr(I18nKey::BtnApply)()}
                                        </button>
                                        <small class="harness-muted">
                                          {move || format!("{} {}", i18n.tr(I18nKey::LayDefaultIntro)(), HARNESS_BROWSER_DEFAULT_URL)}
                                        </small>
                                    </div>
                                </article>
                            }
                            .into_any(),
                            HarnessSettingsCategory::Language => view! {
                                <article class="harness-pane">
                                    <h3>{move || i18n.tr(I18nKey::LangHeading)()}</h3>
                                    <label class="harness-stack">
                                        <span>{move || i18n.tr(I18nKey::LangUiLang)()}</span>
                                        <select
                                          class="workbench-plain-input"
                                          prop:value=move || i18n.locale().get().as_str().to_owned()
                                          on:change=move |ev| {
                                             if let Some(tag) =
                                               ev.target()
                                                 .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
                                                   .map(|s| s.value())
                                             {
                                                if let Some(loc)=Locale::parse_bcp47(&tag) {
                                                    i18n.set_locale(loc);
                                                }
                                             }
                                          }
                                        >
                                            <option value="de-DE">"Deutsch"</option>
                                            <option value="en-US">"English"</option>
                                        </select>
                                    </label>
                                </article>
                            }
                            .into_any(),
                            HarnessSettingsCategory::Agent => view! {
                                <article class="harness-pane">
                                    <h3>{move || i18n.tr(I18nKey::AgHeading)()}</h3>
                                    <label class="harness-stack">
                                        <span>{move || i18n.tr(I18nKey::AgWsRootLabel)()}</span>
                                        <textarea
                                            class="workbench-plain-textarea"
                                            rows="3"
                                            placeholder=move || i18n.tr(I18nKey::AgWsPlaceholder)()
                                            prop:value=move || wb.harness_workspace_root().get()
                                            on:input=move |ev| {
                                                if let Some(ta)=ev.target()
                                                    .and_then(|t|t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()){
                                                    wb.set_harness_workspace_root_text(ta.value());
                                                }
                                            }
                                        ></textarea>
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
                                        {move || i18n.tr(I18nKey::BtnSave)()}
                                      </button>
                                    </div>
                                    <HarnessProviderPreview />
                                    <small class="harness-muted">{move || i18n.tr(I18nKey::AgReadBuiltin)()}</small>
                                    <AgentHooksPanel />
                                </article>
                            }.into_any(),
                        }}
                    </div>
                </div>
            </section>
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
    view! {
        <button
            type="button"
            class:harness-cat-active=move || ui.settings_category().get() == cat
            class="harness-cat-btn"
            on:click={
                move |_| ui.settings_category().set(cat)
            }
        >
            {move || i18n.tr(label)()}
        </button>
    }
}

#[component]
fn HarnessProviderPreview() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let body = RwSignal::new(String::new());
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            body.set(match agent_provider_status().await {
                Ok(v) => serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
                Err(e) => format!("IPC: {e}"),
            });
        });
    });
    view! {
        <pre class="workbench-plain-pre">{move || {
            let t = body.get();
            if t.is_empty() {
                i18n.tr(I18nKey::HarnessLoading)().into()
            } else {
                t
            }
        }}</pre>
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
            <h4>{move || i18n.tr(I18nKey::AgHooksHeading)()}</h4>
            <p class="harness-muted">{move || i18n.tr(I18nKey::AgHooksDesc)()}</p>
            <ul class="harness-hooks__list">
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
                                view! {
                                    <li class="harness-hooks__item">
                                        <strong>{entry.agent}</strong>
                                        <span class="harness-muted">{format!(" — {status}")}</span>
                                        <Show when=move || has_note>
                                            <small class="harness-muted">{note.clone()}</small>
                                        </Show>
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
                    {move || {
                        if busy.get() {
                            i18n.tr(I18nKey::AgHooksBusy)().to_string()
                        } else {
                            i18n.tr(I18nKey::AgHooksInstall)().to_string()
                        }
                    }}
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    prop:disabled=move || busy.get() || !is_tauri_shell()
                    on:click=on_uninstall
                >
                    {move || i18n.tr(I18nKey::AgHooksUninstall)()}
                </button>
            </div>
            <Show when=move || error.get().is_some()>
                <p class="harness-muted">{move || error.get().unwrap_or_default()}</p>
            </Show>
        </section>
    }
}

fn eula_preview(loc: Locale) -> String {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(EULA_STORAGE_KEY).ok().flatten())
        .map(|v| match v.as_str() {
            "1" => lookup(loc, I18nKey::EulaAccepted).to_string(),
            other => format!("„{other}“"),
        })
        .unwrap_or_else(|| lookup(loc, I18nKey::EulaUnknown).to_string())
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
