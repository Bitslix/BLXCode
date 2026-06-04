//! "Install hooks" prompt shown at startup when no agent CLI has blxcode
//! hooks wired up. Mirrors the overlay/sheet pattern of
//! [`crate::workbench::post_update_notes`] and reuses the existing backend
//! command (`install_agent_hooks`). It is shown after the post-update
//! ("What's new") screen via the auto-open effect in `WorkbenchShell`.
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::install_agent_hooks;
use crate::workbench::hook_status::HookStatusService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[derive(Clone, Copy)]
pub struct HookInstallDialogService {
    open: RwSignal<bool>,
    busy: RwSignal<bool>,
    error: RwSignal<Option<String>>,
}

impl Default for HookInstallDialogService {
    fn default() -> Self {
        Self::new()
    }
}

impl HookInstallDialogService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            busy: RwSignal::new(false),
            error: RwSignal::new(None),
        }
    }

    pub fn open(&self) -> RwSignal<bool> {
        self.open
    }

    pub fn busy(&self) -> RwSignal<bool> {
        self.busy
    }

    pub fn error(&self) -> RwSignal<Option<String>> {
        self.error
    }

    pub fn show(&self) {
        self.error.set(None);
        self.open.set(true);
    }

    /// Close without installing. No persistence — the dialog is offered
    /// again on the next launch while hooks remain missing.
    pub fn dismiss(&self) {
        if self.busy.get_untracked() {
            return;
        }
        self.open.set(false);
    }

    /// Install hooks for every supported agent, then refresh the status
    /// indicator and close on success.
    pub fn install(&self, status: HookStatusService) {
        if self.busy.get_untracked() {
            return;
        }
        let open = self.open;
        let busy = self.busy;
        let error = self.error;
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match install_agent_hooks().await {
                Ok(report) => {
                    crate::app_log::info(
                        "hooks",
                        "agent_hooks_installed_startup",
                        serde_json::json!({ "entries": report.entries.len() }),
                    );
                    status.recheck();
                    open.set(false);
                }
                Err(e) => {
                    crate::app_log::error(
                        "hooks",
                        "agent_hooks_install_startup_failed",
                        serde_json::json!({ "error": e.clone() }),
                    );
                    error.set(Some(e));
                }
            }
            busy.set(false);
        });
    }
}

#[component]
pub fn HookInstallDialog() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let dialog = expect_context::<HookInstallDialogService>();
    let status = expect_context::<HookStatusService>();

    let busy = dialog.busy();
    let error = dialog.error();

    view! {
        <Show when=move || dialog.open().get()>
            <div class="harness-overlay harness-overlay--modal harness-overlay--centered" role="presentation">
                <button
                    type="button"
                    class="harness-scrim"
                    tabindex="-1"
                    aria-label=move || i18n.tr(I18nKey::HookInstallLater)()
                    on:click=move |_| dialog.dismiss()
                ></button>
                <section
                    class="harness-sheet harness-sheet--hook-install"
                    role="dialog"
                    aria-modal="true"
                    on:keydown=move |ev: web_sys::KeyboardEvent| {
                        if ev.key() == "Escape" {
                            ev.prevent_default();
                            dialog.dismiss();
                        }
                    }
                >
                    <header class="hook-install-hero">
                        <div class="hook-install-hero__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuTerminal width="1.25rem" height="1.25rem" />
                        </div>
                        <div class="hook-install-hero__copy">
                            <h2>{move || i18n.tr(I18nKey::HookInstallTitle)()}</h2>
                            <p>{move || i18n.tr(I18nKey::HookInstallBody)()}</p>
                        </div>
                    </header>
                    <Show when=move || error.get().is_some()>
                        <p class="hook-install-error" role="alert">{move || error.get().unwrap_or_default()}</p>
                    </Show>
                    <footer class="hook-install-actions">
                        <button
                            type="button"
                            class="workbench-mini-btn"
                            disabled=move || busy.get()
                            on:click=move |_| dialog.dismiss()
                        >
                            {move || i18n.tr(I18nKey::HookInstallLater)()}
                        </button>
                        <button
                            type="button"
                            class="workbench-mini-btn workbench-mini-btn--primary"
                            disabled=move || busy.get()
                            on:click=move |_| dialog.install(status)
                        >
                            <span class="harness-btn-inline">
                                <Show
                                    when=move || busy.get()
                                    fallback=move || view! {
                                        <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
                                    }
                                >
                                    <span class="hook-install-spin" aria-hidden="true">
                                        <LxIcon icon=icondata::LuLoaderCircle width="0.82rem" height="0.82rem" />
                                    </span>
                                </Show>
                                <span>{move || if busy.get() {
                                    i18n.tr(I18nKey::HookInstallBusy)()
                                } else {
                                    i18n.tr(I18nKey::HookInstallCta)()
                                }}</span>
                            </span>
                        </button>
                    </footer>
                </section>
            </div>
        </Show>
    }
}
