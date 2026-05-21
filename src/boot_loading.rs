use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootPhase {
    Starting,
    RestoringWorkspace,
    OpeningWorkbench,
}

impl BootPhase {
    fn eyebrow(self) -> &'static str {
        match self {
            Self::Starting => "Starting BLXCode",
            Self::RestoringWorkspace => "Restoring workspace",
            Self::OpeningWorkbench => "Opening workbench",
        }
    }

    fn status(self) -> &'static str {
        match self {
            Self::Starting => "Preparing the interface",
            Self::RestoringWorkspace => "Loading sidebar, sessions, and workspace state",
            Self::OpeningWorkbench => "Bringing the panels online",
        }
    }
}

#[component]
pub fn BootLoadingScreen(phase: BootPhase) -> impl IntoView {
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
                        <p class="blx-boot__eyebrow">{phase.eyebrow()}</p>
                        <h1 class="blx-boot__title">"BLXCode"</h1>
                        <p class="blx-boot__status">{phase.status()}</p>
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
