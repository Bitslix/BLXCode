//! Three-pane editor shell: collapsible sidebar, workspace, resizable right column.
mod agent_accent;
mod agent_context_handoff;
mod agent_panel;
mod agent_timeline;
mod agent_model_picker;
mod agent_provider_pane;
mod api_keys_pane;
mod workspace_settings_pane;
mod appearance_settings_pane;
mod app_prefs;
mod browser_tab;
mod chat_markdown;
mod close_terminals_tab_dialog;
mod create_workspace_wizard;
mod file_preview;
mod git_graph;
mod harness_chords;
mod harness_image_pane;
mod harness_ui;
mod harness_voice_pane;
mod memory_graph;
mod memory_panel;
mod notification_sound;
mod path_nav;
mod plans_panel;
mod project_explorer;
mod right_panel;
mod sidebar;
mod sidebar_resizer;
mod sidebar_view_section;
pub mod skills_rules_panel;
pub mod state;
mod voice_app_controls;
mod terminal_cell;
mod terminal_glue;
mod theme_service;
mod toast;
mod update_dialog;
mod update_service;
mod workspace_panel;

pub use agent_panel::AgentPanelDock;
pub use appearance_settings_pane::AppearanceSettingsPane;
pub use theme_service::ThemeService;
pub use agent_provider_pane::AgentProviderPane;
pub use api_keys_pane::ApiKeysPane;
pub use workspace_settings_pane::WorkspaceSettingsPane;
pub use browser_tab::{BrowserTabDock, EmbeddedBrowserGlue};
pub use memory_panel::MemoryPanel;
pub use plans_panel::PlansPanel;
pub use right_panel::RightPanel;
pub use sidebar::Sidebar;
pub use skills_rules_panel::SkillsRulesService;
pub use state::{
    AgentImageContextStatus, BrowserEmbedSurface, HarnessUiService, LegacyStorageMigration,
    RightPanelTab, WorkbenchService, WorkbenchSnapshot, WorkspaceAgentImage,
};
pub use workspace_panel::WorkspacePanel;

use crate::boot_loading::{BootLoadingScreen, BootPhase};
use crate::config::{SIDEBAR_WIDTH_PX_KEY, SIDEBAR_WIDTH_PX_MIN};
use crate::i18n::I18nKey;
use crate::open_http::{dom_click_nav_href, DomNavHref};
use crate::service::I18nService;
use crate::tauri_bridge::{
    browser_embedding_kind, harness_ensure_default_sandbox, harness_user_home_dir, is_tauri_shell,
    workbench_extract_sessions_prefix, workbench_load_state, workbench_merge_sessions_workspace,
    workbench_prune_notifications, workbench_prune_sessions, workbench_save_state,
};
use app_prefs::AppPrefsService;
use gloo_timers::future::TimeoutFuture;
use harness_ui::HarnessHost;
use js_sys;
use leptos::prelude::*;
use leptos::task::spawn_local;
use send_wrapper::SendWrapper;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use toast::{ToastHost, ToastService};
use close_terminals_tab_dialog::CloseTerminalsTabDialog;
use update_dialog::{UpdateBanner, UpdateDialog};
use update_service::UpdateService;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

const SIDEBAR_WORKSPACE_MIN_PX: f64 = 240.0;

fn viewport_width_px() -> f64 {
    web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0)
}

fn sidebar_width_max_px(viewport_w: f64) -> f64 {
    let max_by_ratio = viewport_w * 0.4;
    let max_by_space = viewport_w - SIDEBAR_WORKSPACE_MIN_PX;
    max_by_ratio
        .max(SIDEBAR_WIDTH_PX_MIN)
        .min(max_by_space.max(SIDEBAR_WIDTH_PX_MIN))
}

/// Debounce window before a dirty workbench gets flushed to disk. Short
/// enough to feel "live", long enough to coalesce a burst of mutations
/// (typing in name field, dragging splitter, etc.) into one IPC call.
const AUTO_SAVE_DEBOUNCE_MS: u32 = 500;

async fn migrate_legacy_sessions(migrations: Vec<LegacyStorageMigration>) {
    for migration in migrations {
        let old_prefix = format!("{}:", migration.old_workspace_key);
        let blob = match workbench_extract_sessions_prefix(old_prefix).await {
            Ok(blob) => blob,
            Err(err) => {
                leptos::logging::warn!("workbench_extract_sessions_prefix: {err}");
                continue;
            }
        };
        let trimmed = blob.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            continue;
        }
        if let Err(err) = workbench_merge_sessions_workspace(
            migration.old_workspace_key,
            migration.new_workspace_key,
            blob,
        )
        .await
        {
            leptos::logging::warn!("workbench_merge_sessions_workspace: {err}");
        }
    }
}

#[component]
pub fn WorkbenchShell() -> impl IntoView {
    let wb = WorkbenchService::new();
    let harness = HarnessUiService::new();
    let embed_surface = BrowserEmbedSurface(RwSignal::new(None));
    let skills_rules = SkillsRulesService::new();
    let app_prefs = AppPrefsService::new();
    let toast = ToastService::new();
    let updates = UpdateService::new();

    provide_context(wb);
    provide_context(harness);
    provide_context(embed_surface);
    provide_context(skills_rules);
    provide_context(app_prefs);
    provide_context(toast);
    provide_context(updates);

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
                    Ok(mut snap) => {
                        let migrations = snap.backfill_storage_keys();
                        if !migrations.is_empty() {
                            migrate_legacy_sessions(migrations).await;
                        }
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
            if wb.default_project_dir().get_untracked().trim().is_empty() {
                if let Ok(home) = harness_user_home_dir().await {
                    wb.persist_default_project_dir(home);
                }
            }
            persistence_enabled.set(allow_save);
            hydrated.set(true);
        });
    });

    // One-shot prune right after hydration: removes notifications.json
    // and sessions.json entries whose terminal_key has no matching slot
    // in the freshly-loaded workspace state. This drops legacy numeric
    // notification keys and any closed-slot session references after
    // pre-UUID session entries have been migrated.
    Effect::new(move |_| {
        if !hydrated.get() {
            return;
        }
        if !is_tauri_shell() {
            return;
        }
        let live_keys = wb.live_terminal_keys();
        let live_keys_for_sessions = live_keys.clone();
        spawn_local(async move {
            let _ = workbench_prune_notifications(live_keys).await;
        });
        spawn_local(async move {
            let _ = workbench_prune_sessions(live_keys_for_sessions).await;
        });
    });

    Effect::new(move |_| {
        if hydrated.get() && app_prefs.update_auto_check_enabled().get() {
            updates.check_silent();
        }
    });

    let notification_poller_started = RwSignal::new(false);
    Effect::new(move |_| {
        if !hydrated.get() || notification_poller_started.get_untracked() {
            return;
        }
        notification_poller_started.set(true);
        notification_sound::spawn_notification_poller(wb);
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
        let _ = wb.sidebar_width_px().get();
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
            let live_keys = wb.live_terminal_keys();
            if let Ok(json) = serde_json::to_string(&snap) {
                let _ = workbench_save_state(json).await;
            }
            let _ = workbench_prune_notifications(live_keys).await;
        });
    });

    // Best-effort flush when the window is closing — the debounce timer
    // may not fire if the OS terminates the process first.
    let beforeunload_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("beforeunload", move |_| {
            if !hydrated.get_untracked()
                || !is_tauri_shell()
                || !persistence_enabled.get_untracked()
            {
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

    let contextmenu_handle =
        leptos::leptos_dom::helpers::window_event_listener_untyped("contextmenu", move |ev| {
            ev.prevent_default();
        });
    on_cleanup(move || drop(contextmenu_handle));

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

    let i18n = expect_context::<I18nService>();
    let sidebar_resizing = RwSignal::new(false);
    let sidebar_drag_anchor_x = RwSignal::new(0.0_f64);
    let sidebar_drag_anchor_w = RwSignal::new(0.0_f64);

    Effect::new(move |_| {
        let w = wb.sidebar_width_px().get();
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(SIDEBAR_WIDTH_PX_KEY, &format!("{w:.0}"));
        }
    });

    Effect::new(move |_| {
        if !sidebar_resizing.get() {
            return;
        }
        let width_sig = wb.sidebar_width_px();
        let ax = sidebar_drag_anchor_x;
        let aw = sidebar_drag_anchor_w;
        let resizing_sig = sidebar_resizing;

        let move_h = window_event_listener_untyped("mousemove", move |ev| {
            let me = match ev.dyn_into::<web_sys::MouseEvent>() {
                Ok(m) => m,
                Err(_) => return,
            };
            let dx = f64::from(me.client_x()) - ax.get_untracked();
            let viewport_w = viewport_width_px();
            let next = (aw.get_untracked() + dx)
                .clamp(SIDEBAR_WIDTH_PX_MIN, sidebar_width_max_px(viewport_w));
            width_sig.set(next);
        });

        let up_h = window_event_listener_untyped("mouseup", move |_| {
            resizing_sig.set(false);
        });

        on_cleanup(move || {
            move_h.remove();
            up_h.remove();
        });
    });

    let on_sidebar_splitter_down = move |ev: web_sys::MouseEvent| {
        if wb.sidebar_collapsed().get_untracked() {
            return;
        }
        ev.prevent_default();
        sidebar_drag_anchor_x.set(ev.client_x() as f64);
        sidebar_drag_anchor_w.set(wb.sidebar_width_px().get_untracked());
        sidebar_resizing.set(true);
    };

    view! {
        <Show
            when=move || hydrated.get()
            fallback=|| view! { <BootLoadingScreen phase=BootPhase::RestoringWorkspace/> }
        >
            <main class="container app-shell workbench-root">
                <div
                    class=move || {
                        let mut c = String::from("workbench-left-slot");
                        if sidebar_resizing.get() {
                            c.push_str(" workbench-left-slot--resizing");
                        }
                        c
                    }
                >
                    <Sidebar />
                    <Show when=move || !wb.sidebar_collapsed().get()>
                        <div
                            class="workbench-splitter workbench-splitter--sidebar"
                            role="separator"
                            aria-orientation="vertical"
                            aria-label=move || i18n.tr(I18nKey::SbWidthSplitterAria)()
                            on:mousedown=on_sidebar_splitter_down
                        >
                        </div>
                    </Show>
                </div>
                <Show when=move || sidebar_resizing.get()>
                    <div class="workbench-resize-shield workbench-resize-shield--col" aria-hidden="true"></div>
                </Show>
                <div class="workbench-main">
                    <WorkspacePanel />
                    <RightPanel />
                </div>
            </main>
            <EmbeddedBrowserGlue />
            <UpdateBanner />
            <UpdateDialog />
            <CloseTerminalsTabDialog />
            <HarnessHost />
            <ToastHost />
        </Show>
    }
}
