//! Commit dialog for the File Diff section.
//!
//! Lets the user write a commit message by hand or generate one with AI
//! (reusing the agent tab's configured provider via the
//! `git_generate_commit_message` Tauri command), then runs `git commit`.
//! Driven by an `open` signal owned by the caller; closes itself on success,
//! cancel, scrim click, or Escape.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{git_commit, git_generate_commit_message};
use crate::workbench::toast::ToastService;
use crate::workbench::WorkbenchService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn CommitDialog(
    /// Controls visibility; the dialog flips it to `false` when it closes.
    open: RwSignal<bool>,
    /// Called after a successful commit so the section can refresh.
    after: Callback<()>,
) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let toast = expect_context::<ToastService>();

    let message = RwSignal::new(String::new());
    let generating = RwSignal::new(false);
    let committing = RwSignal::new(false);

    // Clear transient state whenever the dialog is (re)opened.
    Effect::new(move |_| {
        if open.get() {
            message.set(String::new());
            generating.set(false);
            committing.set(false);
        }
    });

    let close = move || open.set(false);

    let busy = move || generating.get() || committing.get();

    let on_generate = move |_| {
        if busy() {
            return;
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        generating.set(true);
        spawn_local(async move {
            match git_generate_commit_message(cwd).await {
                Ok(msg) => {
                    message.set(msg);
                }
                Err(e) if e == "nothing staged" => {
                    toast.error(i18n.tr(I18nKey::SbDiffCommitNothingStaged)());
                }
                Err(_) => {
                    toast.error(i18n.tr(I18nKey::SbDiffCommitGenFailed)());
                }
            }
            generating.set(false);
        });
    };

    let on_commit = move |_| {
        if busy() {
            return;
        }
        let text = message.get().trim().to_string();
        if text.is_empty() {
            toast.error(i18n.tr(I18nKey::SbDiffCommitEmptyMsg)());
            return;
        }
        let Some(cwd) = wb.default_workspace_cwd() else {
            return;
        };
        committing.set(true);
        spawn_local(async move {
            match git_commit(cwd, text).await {
                Ok(()) => {
                    toast.success(i18n.tr(I18nKey::SbDiffCommitSuccess)());
                    committing.set(false);
                    open.set(false);
                    after.run(());
                }
                Err(_) => {
                    toast.error(i18n.tr(I18nKey::SbDiffCommitFailed)());
                    committing.set(false);
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
                    class="harness-sheet blx-commit"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" {
                            ev.prevent_default();
                            close();
                        }
                    }
                >
                    <header class="blx-commit__head">
                        <h2 class="harness-settings-title">
                            <span class="harness-settings-title__icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuGitCommitHorizontal width="1.05rem" height="1.05rem" />
                            </span>
                            <span>{move || i18n.tr(I18nKey::SbDiffCommitTitle)()}</span>
                        </h2>
                    </header>

                    <textarea
                        class="blx-commit__message"
                        rows="5"
                        placeholder=move || i18n.tr(I18nKey::SbDiffCommitPlaceholder)()
                        prop:value=move || message.get()
                        prop:disabled=move || committing.get()
                        on:input=move |ev| message.set(event_target_value(&ev))
                    ></textarea>

                    <footer class="blx-commit__actions">
                        <button
                            type="button"
                            class="workbench-mini-btn blx-commit__ai"
                            prop:disabled=busy
                            on:click=on_generate
                        >
                            <Show
                                when=move || generating.get()
                                fallback=move || view! {
                                    <span class="blx-commit__ai-icon" aria-hidden="true">
                                        <LxIcon icon=icondata::LuSparkles width="0.8rem" height="0.8rem" />
                                    </span>
                                    <span>{move || i18n.tr(I18nKey::SbDiffCommitAi)()}</span>
                                }
                            >
                                <span class="blx-commit__spin" aria-hidden="true">
                                    <LxIcon icon=icondata::LuLoaderCircle width="0.8rem" height="0.8rem" />
                                </span>
                                <span>{move || i18n.tr(I18nKey::SbDiffCommitGenerating)()}</span>
                            </Show>
                        </button>
                        <span class="blx-commit__spacer"></span>
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            prop:disabled=move || committing.get()
                            on:click=move |_| close()
                        >
                            {move || i18n.tr(I18nKey::SrCancel)()}
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            prop:disabled=busy
                            on:click=on_commit
                        >
                            {move || i18n.tr(I18nKey::SbDiffCommit)()}
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}
