//! Three-pane editor shell: collapsible sidebar, workspace, resizable right column.
mod agent_panel;
mod browser_tab;
mod create_workspace_wizard;
mod harness_ui;
mod memory_panel;
mod path_nav;
mod right_panel;
mod sidebar;
pub mod state;
mod terminal_cell;
mod terminal_glue;
mod workspace_panel;

pub use agent_panel::AgentPanelDock;
pub use browser_tab::{BrowserTabDock, EmbeddedBrowserGlue};
pub use memory_panel::MemoryPanel;
pub use right_panel::RightPanel;
pub use sidebar::Sidebar;
pub use state::{
    BrowserEmbedSurface, HarnessUiService, RightPanelTab, WorkbenchService, WorkbenchSnapshot,
};
pub use workspace_panel::WorkspacePanel;

use crate::tauri_bridge::{
    browser_embedding_kind, harness_ensure_default_sandbox, is_tauri_shell, workbench_load_state,
    workbench_save_state,
};
use gloo_timers::future::TimeoutFuture;
use harness_ui::HarnessHost;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Debounce window before a dirty workbench gets flushed to disk. Short
/// enough to feel "live", long enough to coalesce a burst of mutations
/// (typing in name field, dragging splitter, etc.) into one IPC call.
const AUTO_SAVE_DEBOUNCE_MS: u32 = 500;

#[component]
pub fn WorkbenchShell() -> impl IntoView {
    let wb = WorkbenchService::new();
    let harness = HarnessUiService::new();
    let embed_surface = BrowserEmbedSurface(RwSignal::new(None));

    provide_context(wb);
    provide_context(harness);
    provide_context(embed_surface);

    // Hydrate from persisted snapshot before auto-save kicks in.
    let hydrated = RwSignal::new(false);
    let persistence_enabled = RwSignal::new(!is_tauri_shell());
    Effect::new(move |_| {
        if !is_tauri_shell() {
            hydrated.set(true);
            persistence_enabled.set(true);
            return;
        }
        spawn_local(async move {
            let mut allow_save = true;
            match workbench_load_state().await {
                Err(_) => allow_save = false,
                Ok(None) => {}
                Ok(Some(json)) => match serde_json::from_str::<WorkbenchSnapshot>(&json) {
                    Err(_) => allow_save = false,
                    Ok(snap) => {
                        allow_save = wb.hydrate(snap);
                    }
                },
            }
            if wb
                .harness_workspace_root()
                .get_untracked()
                .trim()
                .is_empty()
            {
                if let Ok(path) = harness_ensure_default_sandbox().await {
                    wb.persist_harness_workspace_root(path);
                }
            }
            persistence_enabled.set(allow_save);
            hydrated.set(true);
        });
    });

    // Debounced auto-save. Tracks every persisted signal; a token guards
    // against firing a stale save when a newer tick is already scheduled.
    let save_token: Arc<AtomicU32> = Arc::new(AtomicU32::new(0));
    Effect::new(move |_| {
        // Subscribe to every persisted signal.
        let _ = wb.workspaces().get();
        let _ = wb.active_id().get();
        let _ = wb.recent_workspaces().get();
        let _ = wb.sidebar_collapsed().get();
        let _ = wb.right_collapsed().get();
        let _ = wb.right_width_px().get();
        let _ = wb.right_active_tab().get();
        let _ = wb.embedded_browser_tabs().get();
        let _ = wb.embedded_browser_active_id().get();

        if !hydrated.get() || !is_tauri_shell() || !persistence_enabled.get() {
            return;
        }
        let token = save_token.fetch_add(1, Ordering::Relaxed) + 1;
        let save_token = save_token.clone();
        spawn_local(async move {
            TimeoutFuture::new(AUTO_SAVE_DEBOUNCE_MS).await;
            if save_token.load(Ordering::Relaxed) != token {
                return; // newer tick superseded us
            }
            let snap = wb.snapshot();
            if let Ok(json) = serde_json::to_string(&snap) {
                let _ = workbench_save_state(json).await;
            }
        });
    });

    // Best-effort flush when the window is closing — the debounce timer
    // may not fire if the OS terminates the process first.
    let beforeunload_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("beforeunload", move |_| {
            if !hydrated.get_untracked() || !is_tauri_shell() || !persistence_enabled.get_untracked() {
                return;
            }
            let snap = wb.snapshot();
            if let Ok(json) = serde_json::to_string(&snap) {
                spawn_local(async move {
                    let _ = workbench_save_state(json).await;
                });
            }
        });
    on_cleanup(move || drop(beforeunload_handle));

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
    }
}
