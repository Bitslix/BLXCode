use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    default_cwd, is_tauri_shell, list_directory, path_nav_invoke, DirEntryBrief, PathNavResult,
};
use crate::workbench::path_nav::path_nav_wasm_string;
use crate::workbench::state::{CreateWorkspaceDraft, WorkbenchService};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

const PRESETS: &[(u8, I18nKey)] = &[
    (1, I18nKey::WzPresetSingle),
    (2, I18nKey::WzPreset2),
    (4, I18nKey::WzPreset4),
    (6, I18nKey::WzPreset6),
    (8, I18nKey::WzPreset8),
    (10, I18nKey::WzPreset10),
    (12, I18nKey::WzPreset12),
    (14, I18nKey::WzPreset14),
    (16, I18nKey::WzPreset16),
];

fn looks_like_cd(s: &str) -> bool {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    lower == "cd" || lower.starts_with("cd ")
}

/// Pfad + Name → neuer Pfad. Behandelt trailing-slash, leeren Base, etc.
fn join_path(base: &str, name: &str) -> String {
    let b = base.trim();
    if b.is_empty() {
        return format!("/{name}");
    }
    if b.ends_with('/') {
        format!("{b}{name}")
    } else {
        format!("{b}/{name}")
    }
}

fn parent_of(path: &str) -> Option<String> {
    let p = std::path::Path::new(path.trim());
    p.parent().map(|x| x.to_string_lossy().into_owned())
}

#[component]
pub fn CreateWorkspaceWizardHost() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let open = wb.create_wizard_open();
    let step = wb.create_wizard_step();
    let draft = wb.create_wizard_draft();
    let cwd_err = RwSignal::new(false);

    // Directory-Browser-Popup
    let browser_open = RwSignal::new(false);
    let dir_entries: RwSignal<Vec<DirEntryBrief>> = RwSignal::new(Vec::new());
    let dir_err: RwSignal<String> = RwSignal::new(String::new());
    let show_hidden = RwSignal::new(false);
    // (top, left, width) in viewport-px für `position: fixed`-Popup,
    // damit es nicht vom `overflow: auto` der Dialog-Sheet abgeschnitten wird.
    let popup_rect: RwSignal<(f64, f64, f64)> = RwSignal::new((0.0, 0.0, 0.0));

    // Beim Öffnen des Wizards einen sinnvollen Start-cwd aus dem Backend holen,
    // wenn das Feld leer ist (kein gespeicherter Workspace-Root vorhanden).
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        if !draft.get_untracked().cwd_display.trim().is_empty() {
            return;
        }
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(p) = default_cwd().await {
                if !p.trim().is_empty() {
                    wb.set_wizard_cwd(p);
                }
            }
        });
    });

    fn measure_input_rect(popup_rect: RwSignal<(f64, f64, f64)>) {
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return; };
        let Some(wrap) = doc.get_element_by_id("wz-cwd-wrap") else { return; };
        let Some(input) = wrap
            .query_selector("input")
            .ok()
            .flatten()
        else {
            return;
        };
        let r = input.get_bounding_client_rect();
        popup_rect.set((r.bottom() + 4.0, r.left(), r.width()));
    }

    // Wenn die cwd-Eingabe wie ein `cd ...`-Befehl aussieht, Enter resolved
    // gegen den Workspace-Root und ersetzt das Feld durch den absoluten Pfad.
    let submit_cwd = move || {
        cwd_err.set(false);
        let value = draft.get_untracked().cwd_display.clone();
        let trimmed = value.trim().to_string();
        if !looks_like_cd(&trimmed) {
            return;
        }
        let base = wb.harness_workspace_root().get_untracked();
        spawn_local(async move {
            let r: Result<PathNavResult, String> = if is_tauri_shell() {
                path_nav_invoke(base.clone(), trimmed.clone()).await
            } else {
                path_nav_wasm_string(&base, &trimmed).map(|(cwd, log_line)| PathNavResult {
                    cwd,
                    log_line,
                })
            };
            if let Ok(res) = r {
                wb.set_wizard_cwd(res.cwd);
            }
        });
    };

    // Listing-Refresh bei jeder Änderung von cwd_display (sofern Pfad und Popup offen).
    Effect::new(move |_| {
        let _open = browser_open.get();
        let val = draft.get().cwd_display.trim().to_string();
        if !is_tauri_shell() || !browser_open.get_untracked() {
            return;
        }
        if val.is_empty() || looks_like_cd(&val) {
            return;
        }
        spawn_local(async move {
            match list_directory(val).await {
                Ok(entries) => {
                    dir_entries.set(entries);
                    dir_err.set(String::new());
                }
                Err(e) => {
                    dir_entries.set(Vec::new());
                    dir_err.set(e);
                }
            }
        });
    });

    // Click-outside schließt den Browser. Popup hat eigene ID, weil es per
    // `position: fixed` nicht mehr im wrap-Subtree des Wrappers liegt
    // (Browsers betrachten die Containment-Beziehung im Layout-Tree, aber für
    // `Node::contains` zählt der DOM-Tree — und der popup hängt nach wie vor
    // unter `#wz-cwd-wrap`. Also weiter wrap.contains checken).
    Effect::new(move |_| {
        if !browser_open.get() {
            return;
        }
        // Position initial messen.
        measure_input_rect(popup_rect);

        let h_down = window_event_listener_untyped("mousedown", move |ev| {
            let Some(target) = ev.target() else { return; };
            let Ok(node) = target.dyn_into::<web_sys::Node>() else { return; };
            let Some(doc) = web_sys::window().and_then(|w| w.document()) else { return; };
            let Some(wrap) = doc.get_element_by_id("wz-cwd-wrap") else { return; };
            if !wrap.contains(Some(&node)) {
                browser_open.set(false);
            }
        });
        let h_resize = window_event_listener_untyped("resize", move |_| {
            measure_input_rect(popup_rect);
        });
        let h_scroll = window_event_listener_untyped("scroll", move |_| {
            measure_input_rect(popup_rect);
        });
        on_cleanup(move || {
            h_down.remove();
            h_resize.remove();
            h_scroll.remove();
        });
    });

    view! {
        <Show when=move || open.get()>
            <div class="harness-overlay harness-overlay--modal" role="presentation">
                <section
                    class="harness-sheet harness-sheet--wizard"
                    role="dialog"
                    aria-modal="true"
                    aria-labelledby="wz-title"
                >
                    <header class="wz-head">
                        <h2 id="wz-title" class="wz-title">{move || i18n.tr(I18nKey::WzTitle)()}</h2>
                        <p class="wz-sub">
                            {move || {
                                if step.get() == 0 {
                                    i18n.tr(I18nKey::WzSubLayout)().to_string()
                                } else {
                                    let n = draft.get().terminal_count;
                                    i18n.tr(I18nKey::WzSubFleet)().replace("{n}", &n.to_string())
                                }
                            }}
                        </p>
                    </header>

                    <Show when=move || step.get() == 0>
                        <div class="wz-layout">
                            <div class="wz-layout__left">
                                <label class="wz-label">{move || i18n.tr(I18nKey::WzNameLabel)()}</label>
                                <input
                                    class="workbench-plain-input wz-input"
                                    type="text"
                                    prop:value=move || draft.get().name_input.clone()
                                    placeholder=move || i18n.tr(I18nKey::WzNamePh)()
                                    on:input=move |ev| {
                                        let v = input_value(&ev);
                                        draft.update(|d| d.name_input = v);
                                    }
                                />
                                <label class="wz-label">{move || i18n.tr(I18nKey::WzCwdLabel)()}</label>
                                <div id="wz-cwd-wrap" class="wz-cwd-wrap">
                                    <input
                                        class="workbench-plain-input wz-input"
                                        type="text"
                                        prop:value=move || draft.get().cwd_display.clone()
                                        on:focus=move |_| browser_open.set(true)
                                        on:input=move |ev| {
                                            cwd_err.set(false);
                                            let v = input_value(&ev);
                                            wb.set_wizard_cwd(v);
                                        }
                                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                                            let key = ev.key();
                                            if key == "Enter" {
                                                ev.prevent_default();
                                                submit_cwd();
                                            } else if key == "Escape" {
                                                browser_open.set(false);
                                            }
                                        }
                                    />
                                    <Show when=move || browser_open.get()>
                                        <div
                                            class="wz-cwd-browser"
                                            role="listbox"
                                            style:top=move || format!("{}px", popup_rect.get().0)
                                            style:left=move || format!("{}px", popup_rect.get().1)
                                            style:width=move || format!("{}px", popup_rect.get().2)
                                        >
                                            <div class="wz-cwd-browser__toolbar">
                                                <button
                                                    type="button"
                                                    class="workbench-mini-btn"
                                                    on:mousedown=move |ev| {
                                                        ev.prevent_default();
                                                        let cur = draft.get_untracked().cwd_display;
                                                        if let Some(parent) = parent_of(&cur) {
                                                            wb.set_wizard_cwd(parent);
                                                        }
                                                    }
                                                    aria-label="parent"
                                                >
                                                    "↑ .."
                                                </button>
                                                <span class="wz-cwd-browser__path">
                                                    {move || draft.get().cwd_display.clone()}
                                                </span>
                                                <label class="wz-cwd-browser__hidden">
                                                    <input
                                                        type="checkbox"
                                                        prop:checked=move || show_hidden.get()
                                                        on:change=move |ev| {
                                                            let el = ev.target()
                                                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
                                                            show_hidden.set(el.map(|e| e.checked()).unwrap_or(false));
                                                        }
                                                    />
                                                    " .hidden"
                                                </label>
                                            </div>
                                            <ul class="wz-cwd-browser__list">
                                                {move || {
                                                    let entries = dir_entries.get();
                                                    let show_h = show_hidden.get();
                                                    let visible: Vec<DirEntryBrief> = entries
                                                        .into_iter()
                                                        .filter(|e| show_h || !e.hidden)
                                                        .collect();
                                                    if visible.is_empty() {
                                                        let msg = dir_err.get();
                                                        return view! {
                                                            <li class="wz-cwd-browser__empty">
                                                                {if msg.is_empty() { "— empty —".to_string() } else { msg }}
                                                            </li>
                                                        }.into_any();
                                                    }
                                                    visible.into_iter().map(|e| {
                                                        let name = e.name.clone();
                                                        let label = e.name.clone();
                                                        let hidden = e.hidden;
                                                        view! {
                                                            <li>
                                                                <button
                                                                    type="button"
                                                                    class=move || {
                                                                        let mut c = String::from("wz-cwd-browser__item");
                                                                        if hidden { c.push_str(" wz-cwd-browser__item--hidden"); }
                                                                        c
                                                                    }
                                                                    on:mousedown=move |ev| {
                                                                        // mousedown statt click damit blur des inputs uns nicht abwürgt
                                                                        ev.prevent_default();
                                                                        let cur = draft.get_untracked().cwd_display;
                                                                        wb.set_wizard_cwd(join_path(&cur, &name));
                                                                    }
                                                                >
                                                                    {label}
                                                                </button>
                                                            </li>
                                                        }
                                                    }).collect_view().into_any()
                                                }}
                                            </ul>
                                        </div>
                                    </Show>
                                </div>
                                <Show when=move || cwd_err.get()>
                                    <p class="wz-error">{move || i18n.tr(I18nKey::WzCwdEmpty)()}</p>
                                </Show>
                            </div>
                            <div class="wz-layout__right">
                                <p class="wz-templates-title">{move || i18n.tr(I18nKey::WzTemplatesHeading)()}</p>
                                <div class="wz-template-grid">
                                    {PRESETS
                                        .iter()
                                        .map(|&(n, key)| {
                                            view! {
                                                <button
                                                    type="button"
                                                    class=move || {
                                                        let mut c = String::from("wz-template-card");
                                                        if draft.get().terminal_count == n {
                                                            c.push_str(" wz-template-card--active");
                                                        }
                                                        c
                                                    }
                                                    on:click=move |_| wb.wizard_set_terminal_layout(n)
                                                >
                                                    <span class="wz-template-card__n">{n}</span>
                                                    <span class="wz-template-card__lbl">{move || i18n.tr(key)()}</span>
                                                </button>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        </div>
                    </Show>

                    <Show when=move || step.get() == 1>
                        <div class="wz-fleet">
                            <h3 class="wz-fleet__title">{move || i18n.tr(I18nKey::WzFleetTitle)()}</h3>
                            <div class="wz-fleet__main">
                                <div class="wz-fleet-tools">
                                    <button type="button" class="eula-btn eula-btn--ghost" on:click=move |_| wb.wizard_fleet_select_all()>
                                        {move || i18n.tr(I18nKey::WzFleetSelectAll)()}
                                    </button>
                                    <button type="button" class="eula-btn eula-btn--ghost" on:click=move |_| wb.wizard_fleet_one_each()>
                                        {move || i18n.tr(I18nKey::WzFleetOneEach)()}
                                    </button>
                                    <button type="button" class="eula-btn eula-btn--ghost" on:click=move |_| wb.wizard_fleet_fill_evenly()>
                                        {move || i18n.tr(I18nKey::WzFillEvenly)()}
                                    </button>
                                    <button type="button" class="eula-btn eula-btn--ghost wz-danger" on:click=move |_| wb.wizard_fleet_clear()>
                                        {move || i18n.tr(I18nKey::WzFleetClear)()}
                                    </button>
                                </div>
                                <ul class="wz-agent-list">
                                    {agent_rows(wb, i18n, draft)}
                                </ul>
                            </div>
                            <aside class="wz-fleet__side">
                                <h3 class="wz-util-title">{move || i18n.tr(I18nKey::WzFleetUtil)()}</h3>
                                <p class="wz-util-slots">{move || {
                                    let d = draft.get();
                                    let a: u8 = wb.wizard_fleet_assigned();
                                    format!("{a} / {}", d.terminal_count)
                                }}</p>
                                <p class="wz-util-note">{move || {
                                    let d = draft.get();
                                    let n = d.terminal_count;
                                    let a: u8 = wb.wizard_fleet_assigned();
                                    if d.agents_skipped {
                                        i18n.tr(I18nKey::WzFleetNoAgents)().to_string()
                                    } else if a == n {
                                        i18n.tr(I18nKey::WzFleetOptimal)().to_string()
                                    } else {
                                        i18n.tr(I18nKey::WzFleetSumWrong)().replace("{n}", &n.to_string())
                                    }
                                }}</p>
                            </aside>
                        </div>
                    </Show>

                    <footer class="wz-footer">
                        <button type="button" class="eula-btn eula-btn--ghost" on:click=move |_| wb.close_create_workspace_wizard()>
                            {move || i18n.tr(I18nKey::WzCancel)()}
                        </button>
                        <div class="wz-footer__grow" />
                        <Show when=move || step.get() == 1>
                            <button type="button" class="eula-btn eula-btn--ghost" on:click=move |_| wb.wizard_back_to_layout()>
                                {move || i18n.tr(I18nKey::WzBack)()}
                            </button>
                            <button type="button" class="eula-btn eula-btn--ghost" on:click=move |_| wb.wizard_skip_agents()>
                                {move || i18n.tr(I18nKey::WzSkipAgents)()}
                            </button>
                            <button
                                type="button"
                                class="eula-btn eula-btn--primary"
                                on:click=move |_| wb.commit_create_workspace()
                                prop:disabled=move || {
                                    let d = draft.get();
                                    if d.cwd_display.trim().is_empty() {
                                        return true;
                                    }
                                    if d.agents_skipped {
                                        return false;
                                    }
                                    wb.wizard_fleet_assigned() != d.terminal_count
                                }
                            >
                                {move || i18n.tr(I18nKey::WzLaunch)()}
                            </button>
                        </Show>
                        <Show when=move || step.get() == 0>
                            <button
                                type="button"
                                class="eula-btn eula-btn--primary"
                                on:click=move |_| {
                                    if wb.wizard_go_to_fleet_step().is_err() {
                                        cwd_err.set(true);
                                    }
                                }
                            >
                                {move || i18n.tr(I18nKey::WzNext)()}
                            </button>
                        </Show>
                    </footer>
                </section>
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=move |_| wb.close_create_workspace_wizard()
                ></button>
            </div>
        </Show>
    }
}

fn agent_rows(
    wb: WorkbenchService,
    i18n: I18nService,
    draft: RwSignal<CreateWorkspaceDraft>,
) -> impl IntoView {
    (0..5)
        .map(|idx| {
            let (name_k, sub_k) = match idx {
                0 => (I18nKey::WzAgentClaude, I18nKey::WzAgentSubClaude),
                1 => (I18nKey::WzAgentCodex, I18nKey::WzAgentSubCodex),
                2 => (I18nKey::WzAgentGemini, I18nKey::WzAgentSubGemini),
                3 => (I18nKey::WzAgentOpencode, I18nKey::WzAgentSubOpencode),
                _ => (I18nKey::WzAgentCursor, I18nKey::WzAgentSubCursor),
            };
            view! {
                <li class="wz-agent-row">
                    <div class="wz-agent-row__meta">
                        <strong>{move || i18n.tr(name_k)()}</strong>
                        <span class="wz-agent-row__sub">{move || i18n.tr(sub_k)()}</span>
                    </div>
                    <div class="wz-agent-row__ctl">
                        <button
                            type="button"
                            class="eula-btn eula-btn--ghost"
                            on:click=move |_| wb.wizard_agent_fill_all(idx)
                        >
                            {move || {
                                let n = draft.get().terminal_count;
                                i18n.tr(I18nKey::WzFleetAll)().replace("{n}", &n.to_string())
                            }}
                        </button>
                        <button
                            type="button"
                            class="wz-stepper"
                            on:click=move |_| {
                                let c = draft.get().agent_counts[idx];
                                wb.wizard_set_agent_count(idx, c.saturating_sub(1));
                            }
                        >"−"</button>
                        <span class="wz-stepper-val">{move || draft.get().agent_counts[idx].to_string()}</span>
                        <button
                            type="button"
                            class="wz-stepper"
                            on:click=move |_| {
                                let c = draft.get().agent_counts[idx];
                                wb.wizard_set_agent_count(idx, c.saturating_add(1));
                            }
                        >"+"</button>
                    </div>
                </li>
            }
        })
        .collect_view()
}

fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}
