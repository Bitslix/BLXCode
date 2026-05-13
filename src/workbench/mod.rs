//! Three-pane editor shell: collapsible sidebar, workspace, resizable right column.
mod agent_panel;
mod browser_tab;
mod create_workspace_wizard;
mod harness_ui;
mod path_nav;
mod right_panel;
mod sidebar;
pub mod state;
mod terminal_cell;
mod terminal_glue;
mod workspace_panel;

pub use agent_panel::AgentPanelDock;
pub use browser_tab::{BrowserTabDock, EmbeddedBrowserGlue};
pub use create_workspace_wizard::CreateWorkspaceWizardHost;
pub use right_panel::RightPanel;
pub use sidebar::Sidebar;
pub use state::{BrowserEmbedSurface, HarnessUiService, RightPanelTab, WorkbenchService};
pub use workspace_panel::WorkspacePanel;

use crate::tauri_bridge::{browser_embedding_kind, is_tauri_shell};
use harness_ui::HarnessHost;
use leptos::prelude::*;
use leptos::task::spawn_local;

#[component]
pub fn WorkbenchShell() -> impl IntoView {
    let wb = WorkbenchService::new();
    let harness = HarnessUiService::new();
    let embed_surface = BrowserEmbedSurface(RwSignal::new(None));

    provide_context(wb);
    provide_context(harness);
    provide_context(embed_surface);

    Effect::new(move |_| {
        if !is_tauri_shell() {
            embed_surface.0.set(Some("iframe_embed".into()));
            return;
        }
        spawn_local(async move {
            let k = browser_embedding_kind()
                .await
                .unwrap_or_else(|_| "iframe_embed".into());
            embed_surface.0.set(Some(k));
        });
    });

    view! {
        <main class="container app-shell workbench-root">
            <Sidebar />
            <div class="workbench-main">
                <WorkspacePanel />
                <RightPanel />
            </div>
        </main>
        <EmbeddedBrowserGlue />
        <HarnessHost />
        <CreateWorkspaceWizardHost />
    }
}
