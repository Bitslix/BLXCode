//! Three-pane editor shell: collapsible sidebar, workspace, resizable right column.
mod agent_timeline;
mod agent_panel;
mod browser_tab;
mod chat_markdown;
mod create_workspace_wizard;
mod harness_ui;
mod harness_voice_pane;
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

use crate::open_http::{dom_click_nav_href, DomNavHref};
use crate::tauri_bridge::{
    browser_embedding_kind, harness_ensure_default_sandbox, is_tauri_shell, workbench_load_state,
    workbench_save_state,
};
use gloo_timers::future::TimeoutFuture;
use harness_ui::HarnessHost;
use js_sys;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use send_wrapper::SendWrapper;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

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

    // HTTP(S) links from Markdown / DOM: capture clicks; PTY uses `blxcode-open-http` from terminal_bootstrap.mjs.
    Effect::new(move |_| {
        let Some(win) = web_sys::window() else {
            return;
        };
        let Some(doc) = win.document() else {
            return;
        };

        let doc_click = Closure::wrap(Box::new({
            let wb = wb;
            let surface = embed_surface;
            move |ev: web_sys::Event| {
                let Some(mouse) = ev.dyn_ref::<web_sys::MouseEvent>() else {
                    return;
                };
                match dom_click_nav_href(mouse) {
                    Some(DomNavHref::Http(url)) => {
                        ev.prevent_default();
                        ev.stop_propagation();
                        browser_tab::open_http_in_embedded_browser(wb, surface, &url);
                    }
                    Some(DomNavHref::Memory(path)) => {
                        ev.prevent_default();
                        ev.stop_propagation();
                        wb.request_open_memory_note(path);
                    }
                    None => {}
                }
            }
        }) as Box<dyn FnMut(_)>);

        let _ = doc.add_event_listener_with_callback_and_bool(
            "click",
            doc_click.as_ref().unchecked_ref(),
            true,
        );

        let win_http = Closure::wrap(Box::new({
            let wb = wb;
            let surface = embed_surface;
            move |ev: web_sys::Event| {
                let Some(ce) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let detail = ce.detail();
                let url = js_sys::Reflect::get(&detail, &wasm_bindgen::JsValue::from_str("url"))
                    .ok()
                    .and_then(|v| v.as_string());
                let Some(url) = url else {
                    return;
                };
                let url = url.trim();
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return;
                }
                browser_tab::open_http_in_embedded_browser(wb, surface, url);
            }
        }) as Box<dyn FnMut(_)>);

        let _ = win.add_event_listener_with_callback(
            browser_tab::BLXCODE_OPEN_HTTP_EVENT,
            win_http.as_ref().unchecked_ref(),
        );

        let doc_click = SendWrapper::new(doc_click);
        let win_http = SendWrapper::new(win_http);
        let doc_cleanup = doc.clone();
        let win_cleanup = win.clone();
        on_cleanup(move || {
            let dc = doc_click.take();
            let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                "click",
                dc.as_ref().unchecked_ref(),
                true,
            );
            let wh = win_http.take();
            let _ = win_cleanup.remove_event_listener_with_callback(
                browser_tab::BLXCODE_OPEN_HTTP_EVENT,
                wh.as_ref().unchecked_ref(),
            );
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
