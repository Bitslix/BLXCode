//! Startup hook-installation check + VSCode-style status-bar indicator.
//!
//! The actual filesystem/JSON inspection happens in the Tauri backend
//! (`agent_hooks_status`), so the check never blocks the UI thread — the
//! frontend only awaits the `invoke` via `spawn_local`. The result drives
//! a small reactive status-bar item and (via the auto-open effect in
//! `WorkbenchShell`) the [`crate::workbench::hook_install_dialog`].
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{agent_hooks_status, is_tauri_shell};
use crate::workbench::hook_install_dialog::HookInstallDialogService;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookCheckPhase {
    /// No check has run yet (e.g. non-Tauri shell).
    Idle,
    /// `agent_hooks_status` is in flight.
    Checking,
    /// At least one agent has hooks wired up.
    Installed,
    /// No agent has hooks installed — prompt the user.
    Missing,
    /// The status command failed.
    Error,
}

#[derive(Clone, Copy)]
pub struct HookStatusService {
    phase: RwSignal<HookCheckPhase>,
    checked: RwSignal<bool>,
}

impl Default for HookStatusService {
    fn default() -> Self {
        Self::new()
    }
}

impl HookStatusService {
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: RwSignal::new(HookCheckPhase::Idle),
            checked: RwSignal::new(false),
        }
    }

    pub fn phase(&self) -> RwSignal<HookCheckPhase> {
        self.phase
    }

    /// True once a check resolved to "no agent has hooks".
    pub fn needs_install(&self) -> bool {
        self.phase.get() == HookCheckPhase::Missing
    }

    /// Run once per session, after workbench hydration. No-op outside the
    /// Tauri shell or if already run.
    pub fn check_after_start(&self) {
        if self.checked.get_untracked() || !is_tauri_shell() {
            return;
        }
        self.checked.set(true);
        self.run_check();
    }

    /// Re-run the check on demand (e.g. right after a successful install) so
    /// the status-bar item reflects the new state.
    pub fn recheck(&self) {
        if !is_tauri_shell() {
            return;
        }
        self.run_check();
    }

    fn run_check(&self) {
        let phase = self.phase;
        phase.set(HookCheckPhase::Checking);
        spawn_local(async move {
            match agent_hooks_status().await {
                Ok(report) => {
                    let none_installed = report.entries.iter().all(|e| !e.installed);
                    phase.set(if none_installed {
                        HookCheckPhase::Missing
                    } else {
                        HookCheckPhase::Installed
                    });
                }
                Err(e) => {
                    leptos::logging::warn!("agent_hooks_status: {e}");
                    phase.set(HookCheckPhase::Error);
                }
            }
        });
    }
}

/// VSCode-style status-bar entry: icon + text reflecting the hook-check
/// phase. Hidden while `Idle`. Clicking (when missing/errored) opens the
/// install dialog.
#[component]
pub fn HookStatusBarItem() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let status = expect_context::<HookStatusService>();
    let dialog = expect_context::<HookInstallDialogService>();
    let phase = status.phase();

    view! {
        <Show when=move || phase.get() != HookCheckPhase::Idle>
            <button
                type="button"
                class=move || {
                    let modifier = match phase.get() {
                        HookCheckPhase::Missing | HookCheckPhase::Error => " app-statusline__item--warn",
                        HookCheckPhase::Installed => " app-statusline__item--quiet",
                        _ => "",
                    };
                    format!("app-statusline__item app-statusline__item--button hook-status-item{modifier}")
                }
                disabled=move || matches!(phase.get(), HookCheckPhase::Checking | HookCheckPhase::Installed)
                on:click=move |_| {
                    if matches!(phase.get(), HookCheckPhase::Missing | HookCheckPhase::Error) {
                        dialog.show();
                    }
                }
            >
                {move || match phase.get() {
                    HookCheckPhase::Checking => view! {
                        <span class="hook-status-item__spin" aria-hidden="true">
                            <LxIcon icon=icondata::LuLoaderCircle width="0.78rem" height="0.78rem" />
                        </span>
                    }.into_any(),
                    HookCheckPhase::Installed => view! {
                        <LxIcon icon=icondata::LuCircleCheck width="0.78rem" height="0.78rem" />
                    }.into_any(),
                    _ => view! {
                        <LxIcon icon=icondata::LuCircleAlert width="0.78rem" height="0.78rem" />
                    }.into_any(),
                }}
                <span>{move || match phase.get() {
                    HookCheckPhase::Checking => i18n.tr(I18nKey::HookCheckChecking)(),
                    HookCheckPhase::Installed => i18n.tr(I18nKey::HookCheckReady)(),
                    HookCheckPhase::Error => i18n.tr(I18nKey::HookCheckFailed)(),
                    _ => i18n.tr(I18nKey::HookCheckMissing)(),
                }}</span>
            </button>
        </Show>
    }
}
