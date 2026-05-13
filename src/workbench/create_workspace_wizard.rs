use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{is_tauri_shell, path_nav_invoke, PathNavResult};
use crate::workbench::path_nav::path_nav_wasm_string;
use crate::workbench::state::{CreateWorkspaceDraft, WorkbenchService};
use leptos::callback::Callback;
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

#[component]
pub fn CreateWorkspaceWizardHost() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let open = wb.create_wizard_open();
    let step = wb.create_wizard_step();
    let draft = wb.create_wizard_draft();
    let cwd_err = RwSignal::new(false);

    let apply_nav = Callback::new({
        let wb = wb;
        let i18n = i18n.clone();
        move |()| {
            cwd_err.set(false);
            let line = draft.get_untracked().nav_line.clone();
            let base = draft.get_untracked().cwd_display.clone();
            spawn_local(async move {
                let r: Result<PathNavResult, String> = if is_tauri_shell() {
                    path_nav_invoke(base, line.clone()).await
                } else {
                    path_nav_wasm_string(&base, &line).map(|(cwd, log_line)| PathNavResult {
                        cwd,
                        log_line,
                    })
                };
                match r {
                    Ok(res) => {
                        wb.set_wizard_cwd(res.cwd.clone());
                        wb.append_nav_log(res.log_line);
                        draft.update(|d| {
                            d.nav_line.clear();
                        });
                    }
                    Err(e) => {
                        wb.append_nav_log(format!("{}: {e}", i18n.tr(I18nKey::WzCdErr)()));
                    }
                }
            });
        }
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
                                <div class="wz-cwd-row">
                                    <input
                                        class="workbench-plain-input wz-input"
                                        type="text"
                                        prop:value=move || draft.get().cwd_display.clone()
                                        on:input=move |ev| {
                                            cwd_err.set(false);
                                            let v = input_value(&ev);
                                            wb.set_wizard_cwd(v);
                                        }
                                    />
                                    <button
                                        type="button"
                                        class="eula-btn eula-btn--primary wz-go"
                                        on:click=move |_| apply_nav.run(())
                                    >
                                        {move || i18n.tr(I18nKey::WzGo)()}
                                    </button>
                                </div>
                                <div class="wz-nav-row">
                                    <span class="wz-prompt">{"\u{003e}_$"}</span>
                                    <input
                                        class="workbench-plain-input wz-input wz-nav-input"
                                        type="text"
                                        prop:value=move || draft.get().nav_line.clone()
                                        placeholder=move || i18n.tr(I18nKey::WzNavPh)()
                                        on:input=move |ev| {
                                            let v = input_value(&ev);
                                            draft.update(|d| d.nav_line = v);
                                        }
                                        on:keydown=move |ev: web_sys::KeyboardEvent| {
                                            if ev.key() == "Enter" {
                                                apply_nav.run(());
                                            }
                                        }
                                    />
                                </div>
                                <p class="wz-hint">{move || i18n.tr(I18nKey::WzNavHint)()}</p>
                                <Show when=move || cwd_err.get()>
                                    <p class="wz-error">{move || i18n.tr(I18nKey::WzCwdEmpty)()}</p>
                                </Show>
                                <pre class="wz-log">{move || draft.get().nav_log.join("\n")}</pre>
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
                                <label class="wz-label">"1–16"</label>
                                <input
                                    class="workbench-plain-input wz-input"
                                    type="number"
                                    min="1"
                                    max="16"
                                    prop:value=move || draft.get().terminal_count.to_string()
                                    on:change=move |ev| {
                                        let v = input_value(&ev).parse::<u8>().unwrap_or(1).clamp(1, 16);
                                        wb.wizard_set_terminal_layout(v);
                                    }
                                />
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