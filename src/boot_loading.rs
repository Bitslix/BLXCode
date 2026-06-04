use crate::i18n::I18nKey;
use crate::service::I18nService;
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootPhase {
    Starting,
    RestoringWorkspace,
    OpeningWorkbench,
}

impl BootPhase {
    fn eyebrow_key(self) -> I18nKey {
        match self {
            Self::Starting => I18nKey::BootStartingBLXCode,
            Self::RestoringWorkspace => I18nKey::BootRestoringWorkspace,
            Self::OpeningWorkbench => I18nKey::BootOpeningWorkbench,
        }
    }

    fn status_key(self) -> I18nKey {
        match self {
            Self::Starting => I18nKey::BootPreparingTheInterface,
            Self::RestoringWorkspace => I18nKey::BootLoadingSidebarSessionsAndWorkspaceState,
            Self::OpeningWorkbench => I18nKey::BootBringingThePanelsOnline,
        }
    }
}

#[component]
pub fn BootLoadingScreen(phase: BootPhase) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <section
            class="blx-boot app-shell app-shell--boot"
            role="status"
            aria-live="polite"
            aria-busy="true"
        >
            <div class="blx-boot__grid" aria-hidden="true"></div>
            <div class="blx-boot__frame">
                <div class="blx-boot__brand">
                    <div class="blx-boot__mark-wrap" aria-hidden="true">
                        <img
                            class="blx-boot__mark"
                            src="/public/blxcode.png"
                            alt=""
                            width="128"
                            height="128"
                            decoding="async"
                        />
                    </div>
                    <div class="blx-boot__copy">
                        <p class="blx-boot__eyebrow">{move || i18n.tr(phase.eyebrow_key())()}</p>
                        <h1 class="blx-boot__title">"BLXCode"</h1>
                        <p class="blx-boot__status">{move || i18n.tr(phase.status_key())()}</p>
                    </div>
                </div>

                <div class="blx-boot__preview" aria-hidden="true">
                    <div class="blx-boot__preview-sidebar">
                        <span></span>
                        <span></span>
                        <span></span>
                        <span></span>
                    </div>
                    <div class="blx-boot__preview-main">
                        <div class="blx-boot__preview-toolbar"></div>
                        <div class="blx-boot__preview-workspace">
                            <span></span>
                            <span></span>
                            <span></span>
                            <span></span>
                        </div>
                    </div>
                    <div class="blx-boot__preview-panel">
                        <span></span>
                        <span></span>
                        <span></span>
                    </div>
                </div>

                <div class="blx-boot__rail" aria-hidden="true">
                    <span></span>
                </div>
            </div>
        </section>
    }
}
