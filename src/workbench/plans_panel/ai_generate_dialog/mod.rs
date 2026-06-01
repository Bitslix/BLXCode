//! AI-driven plan/task generation dialog for the Plans panel.
//!
//! Opened from the panel header's "AI Plan" / "AI Tasks" buttons. The user
//! types a prompt; the configured agent provider generates a Skill-conformant
//! Markdown plan (via the `plan_generate_ai` Tauri command) which is shown as a
//! scrollable preview. Saving persists it with the existing `plan_create`, and
//! — when tasks are requested — syncs the `## Tasks` section via `plan_load`.
//! Reuses the one-shot provider path that powers AI commit messages; no new
//! write path to `.agents/plans` is introduced here.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{plan_create, plan_load, plan_generate_ai, GeneratedPlan, PlanMeta};
use crate::workbench::toast::ToastService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

use super::{next_plan_name, PlansState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiGenMode {
    Plan,
    Tasks,
}

impl AiGenMode {
    fn with_tasks(self) -> bool {
        matches!(self, Self::Tasks)
    }

    fn title_key(self) -> I18nKey {
        match self {
            Self::Plan => I18nKey::PlansAiDialogTitlePlan,
            Self::Tasks => I18nKey::PlansAiDialogTitleTasks,
        }
    }
}

#[component]
pub fn AiGenerateDialog(
    /// Controls visibility; the dialog flips it to `false` when it closes.
    open: RwSignal<bool>,
    /// Selects plan-only vs. plan+tasks generation.
    mode: Signal<AiGenMode>,
    /// Shared plans-panel state (workspace cwd + existing plans + list reload).
    state: PlansState,
    /// Called after a successful save so the panel can refresh its list.
    on_saved: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();

    let prompt = RwSignal::new(String::new());
    let result = RwSignal::<Option<GeneratedPlan>>::new(None);
    let with_tasks = RwSignal::new(false);
    let generating = RwSignal::new(false);
    let saving = RwSignal::new(false);
    let error = RwSignal::<Option<String>>::new(None);

    // Reset transient state on (re)open and seed the toggle from the mode.
    Effect::new(move |_| {
        if open.get() {
            prompt.set(String::new());
            result.set(None);
            generating.set(false);
            saving.set(false);
            error.set(None);
            with_tasks.set(mode.get_untracked().with_tasks());
        }
    });

    let busy = move || generating.get() || saving.get();
    let close = move || open.set(false);

    // In Tasks mode the toggle is forced on (and disabled).
    let toggle_locked = move || mode.get() == AiGenMode::Tasks;

    let on_generate = move |_| {
        if busy() {
            return;
        }
        let text = prompt.get().trim().to_string();
        if text.is_empty() {
            error.set(Some(i18n.tr(I18nKey::PlansAiEmptyPrompt)().to_string()));
            return;
        }
        let want_tasks = with_tasks.get();
        error.set(None);
        generating.set(true);
        spawn_local(async move {
            match plan_generate_ai(text, want_tasks).await {
                Ok(plan) => {
                    result.set(Some(plan));
                }
                Err(e) => {
                    error.set(Some(e));
                    toast.error(i18n.tr(I18nKey::PlansAiFailed)());
                }
            }
            generating.set(false);
        });
    };

    let on_save = move |_| {
        if busy() {
            return;
        }
        let Some(plan) = result.get() else {
            return;
        };
        let Some(ws) = state.workspace_cwd.get_untracked() else {
            error.set(Some(i18n.tr(I18nKey::SrNoWorkspace)().to_string()));
            return;
        };
        let want_tasks = with_tasks.get();
        let existing: Vec<PlanMeta> = state.plans.get_untracked();
        let path = next_plan_name(&plan.title, &existing);
        let markdown = plan.markdown.clone();
        saving.set(true);
        spawn_local(async move {
            match plan_create(&ws, &path, Some(&markdown)).await {
                Ok(_) => {
                    if want_tasks {
                        let _ = plan_load(&ws, &path).await;
                    }
                    toast.success(i18n.tr(I18nKey::PlansAiSaved)());
                    saving.set(false);
                    open.set(false);
                    on_saved.run(());
                }
                Err(e) => {
                    error.set(Some(e));
                    toast.error(i18n.tr(I18nKey::PlansAiFailed)());
                    saving.set(false);
                }
            }
        });
    };

    view! {
        <Show when=move || open.get()>
            <div class="harness-overlay harness-overlay--modal" role="presentation">
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::BtnClose)()
                    on:click=move |_| close()
                ></button>
                <section
                    class="harness-sheet blx-ai-gen"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" && !busy() {
                            ev.prevent_default();
                            close();
                        }
                    }
                >
                    <header class="blx-ai-gen__head">
                        <h2 class="harness-settings-title">
                            <span class="harness-settings-title__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuSparkles width="1.05rem" height="1.05rem" />
                            </span>
                            <span>{move || i18n.tr(mode.get().title_key())()}</span>
                        </h2>
                    </header>

                    <div class="blx-ai-gen__field">
                        {move || if result.get().is_some() {
                            view! {
                                <div class="blx-ai-gen__preview" tabindex="0">
                                    <pre class="blx-ai-gen__preview-text">
                                        {move || result.get().map(|p| p.markdown).unwrap_or_default()}
                                    </pre>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div
                                    class="blx-ai-gen__textwrap"
                                    class:blx-ai-gen__textwrap--loading=move || generating.get()
                                >
                                    <textarea
                                        class="blx-ai-gen__prompt"
                                        rows="6"
                                        placeholder=move || i18n.tr(I18nKey::PlansAiPromptPh)()
                                        prop:value=move || prompt.get()
                                        prop:disabled=move || generating.get()
                                        on:input=move |ev| prompt.set(textarea_value(&ev))
                                    ></textarea>
                                    <Show when=move || generating.get()>
                                        <span class="blx-ai-gen__shimmer" aria-hidden="true"></span>
                                    </Show>
                                </div>
                            }.into_any()
                        }}
                    </div>

                    <label
                        class="app-prefs-toggle blx-ai-gen__toggle"
                        class:blx-ai-gen__toggle--locked=toggle_locked
                    >
                        <input
                            type="checkbox"
                            prop:checked=move || with_tasks.get()
                            prop:disabled=move || toggle_locked() || busy()
                            on:change=move |ev| {
                                if !toggle_locked() {
                                    with_tasks.set(checkbox_checked(&ev));
                                }
                            }
                        />
                        <span>{move || i18n.tr(I18nKey::PlansAiWithTasksToggle)()}</span>
                    </label>

                    {move || error.get().map(|e| view! {
                        <p class="blx-ai-gen__error">{e}</p>
                    })}
                    {move || result.get().is_some().then(|| view! {
                        <p class="blx-ai-gen__hint">{move || i18n.tr(I18nKey::PlansAiPreviewHint)()}</p>
                    })}

                    <footer class="blx-ai-gen__actions">
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            prop:disabled=busy
                            on:click=move |_| close()
                        >
                            {move || i18n.tr(I18nKey::SrCancel)()}
                        </button>
                        <span class="blx-ai-gen__spacer"></span>
                        {move || if result.get().is_some() {
                            view! {
                                <button
                                    type="button"
                                    class="workbench-mini-btn"
                                    prop:disabled=busy
                                    on:click=on_generate
                                >
                                    <span class="blx-ai-gen__btn-icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuRefreshCw width="0.8rem" height="0.8rem" />
                                    </span>
                                    <span>{move || i18n.tr(I18nKey::PlansAiRegenerate)()}</span>
                                </button>
                                <button
                                    type="button"
                                    class="workbench-mini-btn workbench-mini-btn--primary"
                                    prop:disabled=busy
                                    on:click=on_save
                                >
                                    <span class="blx-ai-gen__btn-icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuSave width="0.8rem" height="0.8rem" />
                                    </span>
                                    <span>{move || i18n.tr(I18nKey::PlansAiSave)()}</span>
                                </button>
                            }.into_any()
                        } else {
                            view! {
                                <button
                                    type="button"
                                    class="workbench-mini-btn workbench-mini-btn--primary"
                                    prop:disabled=busy
                                    on:click=on_generate
                                >
                                    <Show
                                        when=move || generating.get()
                                        fallback=move || view! {
                                            <span class="blx-ai-gen__btn-icon" aria-hidden="true">
                                                <LxIcon icon=icondata::LuSparkles width="0.8rem" height="0.8rem" />
                                            </span>
                                            <span>{move || i18n.tr(I18nKey::PlansAiGenerate)()}</span>
                                        }
                                    >
                                        <span class="blx-ai-gen__spin" aria-hidden="true">
                                            <LxIcon icon=icondata::LuLoaderCircle width="0.8rem" height="0.8rem" />
                                        </span>
                                        <span>{move || i18n.tr(I18nKey::PlansAiGenerating)()}</span>
                                    </Show>
                                </button>
                            }.into_any()
                        }}
                    </footer>
                </section>
            </div>
        </Show>
    }
}

fn textarea_value(ev: &web_sys::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

fn checkbox_checked(ev: &web_sys::Event) -> bool {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.checked())
        .unwrap_or(false)
}
