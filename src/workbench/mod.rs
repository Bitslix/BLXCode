//! Three-pane editor shell: collapsible sidebar, workspace, resizable right column.
mod agent_accent;
mod agent_context_handoff;
mod agent_model_picker;
mod agent_onboarding_dialog;
mod agent_panel;
mod agent_settings_pane;
mod agent_timeline;
mod api_keys_pane;
mod app_prefs;
mod app_titlebar;
mod appearance_settings_pane;
mod browser_tab;
mod chat_markdown;
mod close_terminals_tab_dialog;
mod code_editor_settings_pane;
mod commit_dialog;
mod confirm_dialog;
pub mod context_drag;
mod context_drag_overlay;
mod core_status;
mod create_workspace_wizard;
mod diagram_gallery;
mod diagram_render;
mod editor_settings_service;
mod editor_shortcut_config;
mod file_diff;
mod file_diff_section;
mod file_preview;
mod fuzzy;
mod git_graph;
mod git_sync_controls;
mod harness_chords;
mod harness_ui;
mod harness_voice_pane;
mod heartbeat_settings_pane;
mod hook_install_dialog;
mod hook_status;
mod kanban_dnd;
mod kanban_drag_overlay;
mod mcp_settings_pane;
mod memory_graph;
mod memory_panel;
mod memory_settings_pane;
mod notification_sound;
mod path_nav;
mod plan_migration_service;
mod plans_panel;
mod popout_shell;
mod plugins_settings_pane;
pub(crate) mod pointer_agents;
mod post_update_notes;
mod project_explorer;
mod ptt_runtime;
mod remote_settings_pane;
mod right_panel;
mod session_role_picker;
mod settings_pane_header;
mod shortcut_config;
mod shortcuts_settings_pane;
mod sidebar;
mod sidebar_resizer;
mod sidebar_view_section;
pub mod skills_rules_panel;
pub mod state;
pub(crate) mod terminal_agent_profiles;
mod terminal_cell;
mod terminal_context_menu;
mod terminal_glue;
mod terminal_naming;
mod terminal_slot_dnd;
mod terminal_slot_drag_overlay;
mod terminal_usage;
mod theme_service;
mod toast;
mod update_dialog;
mod update_service;
mod voice_app_controls;
mod workspace_kanban;
mod workspace_panel;
mod workspace_settings_pane;

pub use agent_onboarding_dialog::AgentOnboardingDialog;
pub use agent_panel::AgentPanelDock;
pub use agent_settings_pane::AgentSettingsPane;
pub use api_keys_pane::ApiKeysPane;
pub use app_titlebar::AppTitleBar;
pub use appearance_settings_pane::AppearanceSettingsPane;
pub use browser_tab::{BrowserTabDock, EmbeddedBrowserGlue};
pub use code_editor_settings_pane::CodeEditorSettingsPane;
pub use core_status::{CoreStatusBarItem, CoreStatusService, VimStatusIndicator};
pub use editor_settings_service::EditorSettingsService;
pub use heartbeat_settings_pane::HeartbeatSettingsPane;
pub use hook_install_dialog::{HookInstallDialog, HookInstallDialogService};
pub use hook_status::{HookStatusBarItem, HookStatusService};
pub use mcp_settings_pane::McpSettingsPane;
pub use memory_panel::MemoryPanel;
pub use memory_settings_pane::MemorySettingsPane;
pub use plan_migration_service::PlanMigrationService;
pub use plans_panel::PlansPanel;
pub use popout_shell::PopoutShell;
pub use plugins_settings_pane::PluginsSettingsPane;
pub use remote_settings_pane::RemoteSettingsPane;
pub use right_panel::RightPanel;
pub use session_role_picker::SessionRolePicker;
pub use settings_pane_header::SettingsPaneHeader;
pub use shortcuts_settings_pane::ShortcutsSettingsPane;
pub use sidebar::Sidebar;
pub use skills_rules_panel::SkillsRulesService;
pub use state::{
    AgentChatSessionStatus, AgentImageContextStatus, BrowserEmbedSurface, HarnessSettingsCategory,
    HarnessUiService, LegacyStorageMigration, RightPanelTab, WorkbenchService, WorkbenchSnapshot,
    WorkspaceAgentImage,
};
pub use theme_service::ThemeService;
pub use update_service::{UpdateCheckSource, UpdateService, UpdateUiStatus};
pub use workspace_kanban::WorkspaceKanban;
pub use workspace_panel::WorkspacePanel;
pub use workspace_settings_pane::WorkspaceSettingsPane;

use crate::boot_loading::{BootLoadingScreen, BootPhase};
use crate::config::{AGENTS_BOOTSTRAP_CHOICE_KEY, SIDEBAR_WIDTH_PX_KEY, SIDEBAR_WIDTH_PX_MIN};
use crate::i18n::I18nKey;
use crate::open_http::{dom_click_nav_href, DomNavHref};
use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_settings_get, browser_embedding_kind, harness_ensure_default_sandbox,
    harness_user_home_dir, is_tauri_shell, skills_rules_bootstrap,
    workbench_extract_sessions_prefix, workbench_load_state, workbench_merge_sessions_workspace,
    workbench_prune_notifications, workbench_prune_sessions, workbench_save_state,
    workbench_upsert_agent_notification, workspace_agents_layout_status, workspace_ensure_agents,
    AgentNotificationInput, AgentProviderSettingsView,
};
use app_prefs::AppPrefsService;
use close_terminals_tab_dialog::CloseTerminalsTabDialog;
use confirm_dialog::ConfirmDialog;
use context_drag::ContextDragService;
use context_drag_overlay::ContextDragOverlay;
use gloo_timers::future::TimeoutFuture;
use harness_ui::HarnessHost;
use kanban_dnd::KanbanDragService;
use kanban_drag_overlay::KanbanDragOverlay;
use leptos::prelude::*;
use leptos::task::spawn_local;
use post_update_notes::{PostUpdateNotesDialog, PostUpdateNotesService};
use send_wrapper::SendWrapper;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use terminal_slot_dnd::TerminalSlotDragService;
use terminal_slot_drag_overlay::TerminalSlotDragOverlay;
use toast::{ToastHost, ToastKind, ToastService};
use update_dialog::{UpdateBanner, UpdateDialog};
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

const AGENTS_BOOTSTRAP_AUTO: &str = "auto";
const AGENTS_BOOTSTRAP_SKIP: &str = "skip";

fn read_agents_bootstrap_choice() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(AGENTS_BOOTSTRAP_CHOICE_KEY)
        .ok()
        .flatten()
}

fn write_agents_bootstrap_choice(value: &str) {
    let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) else {
        return;
    };
    let _ = storage.set_item(AGENTS_BOOTSTRAP_CHOICE_KEY, value);
}

async fn run_agents_layout_bootstrap(cwd: String, toast: ToastService, i18n: I18nService) {
    let progress = toast.loading("Creating BLXCode workspace files…");
    let result = async {
        workspace_ensure_agents(&cwd).await?;
        skills_rules_bootstrap(cwd.clone()).await?;
        workspace_agents_layout_status(&cwd).await
    }
    .await;

    match result {
        Ok(status) if status.is_complete() => {
            toast.resolve(
                progress,
                ToastKind::Success,
                i18n.tr(I18nKey::WorkbenchWorkspaceFilesReady)(),
            );
        }
        Ok(status) => {
            let count = status.missing_dirs.len() + status.missing_files.len();
            toast.resolve(
                progress,
                ToastKind::Error,
                format!("Workspace bootstrap incomplete: {count} item(s) still missing."),
            );
        }
        Err(err) => {
            toast.resolve(
                progress,
                ToastKind::Error,
                format!("Workspace bootstrap failed: {err}"),
            );
        }
    }
}

fn missing_agents_layout_body(missing_dirs: &[String], missing_files: &[String]) -> String {
    let mut missing = Vec::with_capacity(missing_dirs.len() + missing_files.len());
    missing.extend(missing_dirs.iter().cloned());
    missing.extend(missing_files.iter().cloned());
    let shown = missing.join(", ");
    format!(
        "This workspace is missing BLXCode agent files needed for memory, learnings, plans, and rules: {shown}. Create them now? Your choice will be remembered for the next app launch."
    )
}

#[component]
pub fn WorkbenchShell() -> impl IntoView {
    // Provided at the App root so the always-mounted `AppTitleBar` shares it.
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();
    let harness = HarnessUiService::new();
    let embed_surface = BrowserEmbedSurface(RwSignal::new(None));
    let skills_rules = SkillsRulesService::new();
    let app_prefs = AppPrefsService::new();
    let toast = ToastService::new(app_prefs);
    let updates = expect_context::<UpdateService>();
    let post_update_notes = PostUpdateNotesService::new();
    let agent_onboarding_open = RwSignal::new(false);
    let agent_onboarding_settings: RwSignal<Option<AgentProviderSettingsView>> =
        RwSignal::new(None);
    let agent_onboarding_checked = RwSignal::new(false);
    // Provided at the App root (app.rs); read here to sequence the startup
    // hook check + install prompt after the post-update screen.
    let hook_status = expect_context::<HookStatusService>();
    let hook_install = expect_context::<HookInstallDialogService>();
    let slot_dnd = TerminalSlotDragService::new();
    let context_dnd = ContextDragService::new();
    let kanban_dnd = KanbanDragService::new();
    let git_sync = git_sync_controls::GitSyncControls::new();

    provide_context(harness);
    provide_context(embed_surface);
    provide_context(skills_rules);
    provide_context(app_prefs);
    provide_context(toast);
    provide_context(post_update_notes);
    provide_context(slot_dnd);
    provide_context(context_dnd);
    provide_context(kanban_dnd);
    provide_context(git_sync);

    Effect::new(move |_| {
        crate::app_log::info("workbench", "mounted", serde_json::json!({}));
    });

    let ptt_bus = ptt_runtime::PttBus::default();
    provide_context(ptt_bus);

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
                Err(err) => {
                    leptos::logging::error!("failed to load workbench state: {err}");
                    crate::app_log::error(
                        "workbench",
                        "state_load_failed",
                        serde_json::json!({ "error": err }),
                    );
                    allow_save = false;
                }
                Ok(None) => {}
                Ok(Some(json)) => match serde_json::from_str::<WorkbenchSnapshot>(&json) {
                    Err(err) => {
                        leptos::logging::error!("failed to parse workbench state: {err}");
                        crate::app_log::error(
                            "workbench",
                            "state_parse_failed",
                            serde_json::json!({ "error": err.to_string() }),
                        );
                        allow_save = false;
                    }
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
            crate::app_log::info(
                "workbench",
                "hydrated",
                serde_json::json!({
                    "persistenceEnabled": allow_save,
                    "workspaces": wb.workspaces().with_untracked(|items| items.len()),
                }),
            );
            persistence_enabled.set(allow_save);
            hydrated.set(true);
        });
    });

    let agents_layout_checked = RwSignal::new(HashSet::<String>::new());
    Effect::new(move |_| {
        if !hydrated.get() || !is_tauri_shell() {
            return;
        }
        let active_id = wb.active_id().get();
        let Some((cwd, is_remote)) = active_id.and_then(|id| {
            wb.workspaces().with(|workspaces| {
                workspaces
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .map(|w| {
                        (
                            w.cwd.trim().trim_end_matches(['/', '\\']).to_string(),
                            w.remote_connection_id.is_some(),
                        )
                    })
            })
        }) else {
            return;
        };
        if cwd.is_empty() || is_remote {
            return;
        }
        if agents_layout_checked.with_untracked(|seen| seen.contains(&cwd)) {
            return;
        }
        agents_layout_checked.update(|seen| {
            seen.insert(cwd.clone());
        });

        let ui = harness;
        let toast = toast;
        spawn_local(async move {
            let progress = toast.loading("Checking BLXCode workspace files…");
            let status = match workspace_agents_layout_status(&cwd).await {
                Ok(status) => status,
                Err(err) => {
                    toast.resolve(
                        progress,
                        ToastKind::Error,
                        format!("Workspace file check failed: {err}"),
                    );
                    return;
                }
            };
            if status.is_complete() {
                toast.dismiss(progress);
                return;
            }

            match read_agents_bootstrap_choice().as_deref() {
                Some(AGENTS_BOOTSTRAP_AUTO) => {
                    toast.dismiss(progress);
                    run_agents_layout_bootstrap(cwd, toast, i18n).await;
                }
                Some(AGENTS_BOOTSTRAP_SKIP) => {
                    toast.resolve(
                        progress,
                        ToastKind::Info,
                        i18n.tr(I18nKey::WorkbenchWorkspaceFilesMissingBootstrapSkipped)(),
                    );
                }
                _ => {
                    toast.dismiss(progress);
                    let body =
                        missing_agents_layout_body(&status.missing_dirs, &status.missing_files);
                    let cwd_for_create = cwd.clone();
                    ui.request_confirm(state::ConfirmRequest {
                        title: i18n.tr(I18nKey::WorkbenchCreateWorkspaceFilesPrompt)().to_string(),
                        body,
                        confirm_label: i18n.tr(I18nKey::WorkbenchCreateAutomatically)().to_string(),
                        cancel_label: i18n.tr(I18nKey::WorkbenchNotNow)().to_string(),
                        danger: false,
                        on_confirm: Callback::new(move |_| {
                            write_agents_bootstrap_choice(AGENTS_BOOTSTRAP_AUTO);
                            let cwd = cwd_for_create.clone();
                            spawn_local(async move {
                                run_agents_layout_bootstrap(cwd, toast, i18n).await;
                            });
                        }),
                        on_cancel: Some(Callback::new(move |_| {
                            write_agents_bootstrap_choice(AGENTS_BOOTSTRAP_SKIP);
                            toast.info(i18n.tr(I18nKey::WorkbenchWorkspaceBootstrapSkippedSaved)());
                        })),
                    });
                }
            }
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

    let update_background_started = RwSignal::new(false);
    let update_settings_loaded = RwSignal::new(false);
    Effect::new(move |_| {
        if !hydrated.get() || update_background_started.get_untracked() {
            return;
        }
        update_background_started.set(true);
        if !update_settings_loaded.get_untracked() {
            update_settings_loaded.set(true);
            updates.load_settings();
        }
        spawn_local(async move {
            let mut first = true;
            loop {
                if app_prefs.update_auto_check_enabled().get_untracked() {
                    if first {
                        updates.check_silent();
                    } else {
                        updates.check_background();
                    }
                }
                first = false;
                TimeoutFuture::new(10 * 60 * 1000).await;
            }
        });
    });

    let update_notification_version = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if updates.check_source().get() != UpdateCheckSource::Background
            || updates.status().get() != UpdateUiStatus::Available
        {
            return;
        }
        let Some(version) = updates
            .available_version()
            .get()
            .filter(|version| !version.trim().is_empty())
        else {
            return;
        };
        if update_notification_version.get_untracked().as_deref() == Some(version.as_str()) {
            return;
        }
        update_notification_version.set(Some(version.clone()));
        let title = format!("{} {version}", i18n.tr(I18nKey::UpdateBannerTitle)());
        let body = i18n.tr(I18nKey::UpdateBannerAction)().to_string();
        let input = AgentNotificationInput {
            id: None,
            title: title.clone(),
            body: Some(body.clone()),
            kind: "update".into(),
            severity: Some("info".into()),
            source: Some("updates".into()),
            target: Some(serde_json::json!({
                "view": "update",
                "version": version,
            })),
            dedupe_key: Some("app-update-available".into()),
            read: Some(false),
            sent: Some(true),
        };
        spawn_local(async move {
            if let Ok(item) = workbench_upsert_agent_notification(input).await {
                wb.upsert_agent_notification(item);
                notification_sound::send_native_notification_best_effort(&title, &body);
                notification_sound::play_notification_beep();
            }
        });
    });

    Effect::new(move |_| {
        if hydrated.get() {
            post_update_notes.check_after_start();
            hook_status.check_after_start();
        }
    });

    Effect::new(move |_| {
        if !hydrated.get()
            || !is_tauri_shell()
            || post_update_notes.open().get()
            || agent_onboarding_checked.get_untracked()
        {
            return;
        }
        agent_onboarding_checked.set(true);
        spawn_local(async move {
            match agent_settings_get().await {
                Ok(view) => {
                    let should_open = !view.onboarding_seen;
                    wb.set_default_session_role(view.default_session_role.clone());
                    agent_onboarding_settings.set(Some(view));
                    if should_open {
                        agent_onboarding_open.set(true);
                    }
                }
                Err(err) => {
                    leptos::logging::warn!("agent_settings_get onboarding: {err}");
                }
            }
        });
    });

    // Open the install prompt once the check resolved to "no hooks", but only
    // after the post-update ("What's new") screen is dismissed, so the two
    // modal overlays never stack. No persistence: it reappears each launch
    // while hooks remain missing.
    Effect::new(move |_| {
        if hook_status.needs_install()
            && !post_update_notes.open().get()
            && !agent_onboarding_open.get()
            && !hook_install.open().get_untracked()
        {
            hook_install.show();
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
            if event_target_in_terminal_xterm(&ev) {
                return;
            }
            ev.prevent_default();
        });
    on_cleanup(move || drop(contextmenu_handle));

    // Apply EB's `document.body.cursor = grabbing` while a terminal slot
    // is mid-drag. Restored when the active signal flips back to None.
    Effect::new(move |_| {
        let dragging = slot_dnd.active.get().is_some();
        let Some(body) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.body())
        else {
            return;
        };
        let class_list = body.class_list();
        if dragging {
            let _ = class_list.add_1("blxcode-dragging-terminal");
        } else {
            let _ = class_list.remove_1("blxcode-dragging-terminal");
        }
    });

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

    Effect::new(move |_| {
        let Some(win) = web_sys::window() else {
            return;
        };
        let check_update = Closure::wrap(Box::new({
            let updates = updates;
            move |_ev: web_sys::Event| {
                updates.check_manual();
            }
        }) as Box<dyn FnMut(_)>);
        let _ = win.add_event_listener_with_callback(
            app_titlebar::help_menu::BLXCODE_CHECK_UPDATE_EVENT,
            check_update.as_ref().unchecked_ref(),
        );
        let check_update = SendWrapper::new(check_update);
        let win_cleanup = win.clone();
        on_cleanup(move || {
            let c = check_update.take();
            let _ = win_cleanup.remove_event_listener_with_callback(
                app_titlebar::help_menu::BLXCODE_CHECK_UPDATE_EVENT,
                c.as_ref().unchecked_ref(),
            );
        });
    });

    let previous_update_status = RwSignal::new(UpdateUiStatus::Idle);
    Effect::new(move |_| {
        let status = updates.status().get();
        let manual = updates.manual_check_active().get();
        let previous = previous_update_status.get_untracked();
        if manual && status == UpdateUiStatus::UpToDate && previous != UpdateUiStatus::UpToDate {
            let message = i18n.tr(I18nKey::AppUpdateUpToDate)();
            spawn_local(async move {
                TimeoutFuture::new(2200).await;
                toast.info(message);
            });
        }
        previous_update_status.set(status);
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

    // Push-to-talk: cache settings + install the window-level hold handler.
    ptt_runtime::refresh_ptt_settings_cache();
    ptt_runtime::install_ptt_runtime(app_prefs, wb, i18n, ptt_bus);

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
                <ptt_runtime::PttIndicator/>
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
                            <span class="workbench-splitter__grip" aria-hidden="true"></span>
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
            <PostUpdateNotesDialog />
            <AgentOnboardingDialog
                open=agent_onboarding_open
                settings=agent_onboarding_settings
                on_complete=Callback::new(move |view: AgentProviderSettingsView| {
                    wb.set_default_session_role(view.default_session_role.clone());
                    agent_onboarding_settings.set(Some(view));
                })
            />
            <HookInstallDialog />
            <CloseTerminalsTabDialog />
            <ConfirmDialog />
            <HarnessHost />
            <ToastHost />
            <TerminalSlotDragOverlay />
            <ContextDragOverlay />
            <KanbanDragOverlay />
        </Show>
    }
}

fn event_target_in_terminal_xterm(ev: &web_sys::Event) -> bool {
    let Some(target) = ev
        .target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    else {
        return false;
    };
    target
        .closest(".ws-term-cell__xterm")
        .ok()
        .flatten()
        .is_some()
        || target.closest(".xterm").ok().flatten().is_some()
}
