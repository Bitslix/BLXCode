use crate::boot_loading::{BootLoadingScreen, BootPhase};
use crate::config::EULA_STORAGE_KEY;
use crate::i18n::{localized_eula_html, I18nKey};
use crate::open_http::dom_click_http_url_from_mouse_event;
use crate::quit::request_app_quit;
use crate::service::I18nService;
use crate::tauri_bridge::{
    heartbeat_services_list, heartbeat_set_open_workspaces, is_tauri_shell,
    listen_heartbeat_services_changed, HeartbeatServiceStatus, HeartbeatServiceView,
    PopoutPayload,
};
use crate::workbench::AppTitleBar;
use crate::workbench::EditorSettingsService;
use crate::workbench::PlanMigrationService;
use crate::workbench::ThemeService;
use crate::workbench::UpdateCheckSource;
use crate::workbench::UpdateService;
use crate::workbench::UpdateUiStatus;
use crate::workbench::WorkbenchService;
use crate::workbench::WorkbenchShell;
use crate::workbench::PopoutShell;
use crate::workbench::{CoreStatusBarItem, CoreStatusService, VimStatusIndicator};
use crate::workbench::{HookInstallDialogService, HookStatusBarItem, HookStatusService};
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use send_wrapper::SendWrapper;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[component]
pub fn App() -> impl IntoView {
    let i18n = I18nService::new();
    let theme = ThemeService::new();
    // Code editor preferences (Vim toggle, …) — provided at the App root so the
    // editor, the Code Editor settings pane, and the status-bar Vim indicator
    // all read the same signal.
    let editor_settings = EditorSettingsService::new();
    // The workbench service is created (and provided) at the App root rather
    // than inside `WorkbenchShell` so the always-mounted `AppTitleBar` can read
    // its workspace-scoped state via context. Its hydration/auto-save effects
    // still live in `WorkbenchShell`, which only mounts after the EULA gate.
    let wb = WorkbenchService::new();
    // Hook-check + install-dialog services live at the App root (not inside
    // WorkbenchShell) because `AppStatusLine` is a sibling of the shell and
    // must read the hook-check phase via context.
    let hook_status = HookStatusService::new();
    let hook_install = HookInstallDialogService::new();
    let updates = UpdateService::new();
    let plan_migration = PlanMigrationService::new();
    // Provided at the App root so the sibling `AppStatusLine` can show the
    // enabled-rules/skills counts for the active workspace in its centre slot.
    let core_status = CoreStatusService::new();
    let popout_payload = RwSignal::new(read_popout_payload());
    provide_context(i18n);
    provide_context(theme);
    provide_context(editor_settings);
    provide_context(wb);
    provide_context(hook_status);
    provide_context(hook_install);
    provide_context(updates);
    provide_context(plan_migration);
    provide_context(core_status);

    Effect::new(move |_| {
        remove_static_boot_screen();
    });

    Effect::new(move |_| {
        let lang = i18n.locale().get().as_str();
        if let Some(w) = web_sys::window() {
            if let Some(doc) = w.document() {
                if let Some(root) = doc.document_element() {
                    let _ = root.set_attribute("lang", lang);
                }
            }
        }
    });

    let (ui_ready, set_ui_ready) = signal(false);
    let (app_boot_phase, set_app_boot_phase) = signal(BootPhase::Starting);
    let (eula_ok, set_eula_ok) = signal(false);

    Effect::new(move |_| {
        let stored = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(EULA_STORAGE_KEY).ok().flatten());

        set_eula_ok.set(stored.as_deref() == Some("1"));
        set_app_boot_phase.set(BootPhase::OpeningWorkbench);
        set_ui_ready.set(true);
    });

    Effect::new(move |_| {
        if !ui_ready.get() {
            return;
        }
        if eula_ok.get() {
            return;
        }
        let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
            return;
        };
        let doc_click = Closure::wrap(Box::new(move |ev: web_sys::Event| {
            let Some(mouse) = ev.dyn_ref::<web_sys::MouseEvent>() else {
                return;
            };
            let Some(url) = dom_click_http_url_from_mouse_event(mouse) else {
                return;
            };
            ev.prevent_default();
            ev.stop_propagation();
            open_external(&url);
        }) as Box<dyn FnMut(_)>);
        let _ = doc.add_event_listener_with_callback_and_bool(
            "click",
            doc_click.as_ref().unchecked_ref(),
            true,
        );
        let doc_click = SendWrapper::new(doc_click);
        let doc_cleanup = doc.clone();
        on_cleanup(move || {
            let c = doc_click.take();
            let _ = doc_cleanup.remove_event_listener_with_callback_and_bool(
                "click",
                c.as_ref().unchecked_ref(),
                true,
            );
        });
    });

    let accept = move |_| {
        if let Some(w) = web_sys::window() {
            if let Ok(Some(s)) = w.local_storage() {
                let _ = s.set_item(EULA_STORAGE_KEY, "1");
            }
        }
        set_eula_ok.set(true);
    };

    let decline = move |_| {
        request_app_quit();
    };

    let eula_html = Memo::new(move |_prev| {
        let loc = i18n.locale().get();
        localized_eula_html(loc)
    });

    let is_popout = popout_payload.get_untracked().is_some();
    let show_workbench = move || eula_ok.get() || is_popout;
    let show_eula = move || !eula_ok.get() && !is_popout;

    // Workspace-scoped title-bar controls appear only once the UI is ready and
    // the EULA is accepted; during boot/EULA only the brand + window controls
    // (drag, minimize, maximize, close) render.
    let workbench_active = Signal::derive(move || ui_ready.get() && eula_ok.get() && !is_popout);

    view! {
        <div class="app-root">
            <Show when=move || !is_popout>
                <AppTitleBar workbench_active=workbench_active />
            </Show>
            <div class="app-root__body">
                <Show
                    when=move || ui_ready.get()
                    fallback=move || view! { <BootLoadingScreen phase=app_boot_phase.get()/> }
                >
                    <Show when=show_workbench fallback=move || view! {
                        <Show when=show_eula>
                            <div class="eula-root">
                                <div class="eula-scrim" aria-hidden="true"></div>
                                <div
                                    class="eula-sheet"
                                    role="dialog"
                                    aria-modal="true"
                                    aria-labelledby="eula-heading"
                                >
                                    <div class="eula-scroll eula-md" inner_html=eula_html></div>

                                    <footer class="eula-actions">
                                        <button type="button" class="eula-btn eula-btn--ghost" on:click=decline>
                                            {move || i18n.tr(I18nKey::Decline)()}
                                        </button>
                                        <button type="button" class="eula-btn eula-btn--primary" on:click=accept>
                                            {move || i18n.tr(I18nKey::Accept)()}
                                        </button>
                                    </footer>
                                </div>
                            </div>
                        </Show>
                    }>
                        {move || {
                            if let Some(payload) = popout_payload.get() {
                                view! { <PopoutShell payload=payload /> }.into_any()
                            } else {
                                view! { <WorkbenchShell/> }.into_any()
                            }
                        }}
                    </Show>
                </Show>
            </div>
            <Show when=move || workbench_active.get()>
                <AppStatusLine />
            </Show>
        </div>
    }
}

fn read_popout_payload() -> Option<PopoutPayload> {
    use base64::Engine;

    let search = web_sys::window()?.location().search().ok()?;
    let query = search.strip_prefix('?').unwrap_or(search.as_str());
    let encoded = query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        (key == "popout").then_some(value)
    })?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()?;
    serde_json::from_slice::<PopoutPayload>(&bytes).ok()
}

#[component]
fn AppStatusLine() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let updates = expect_context::<UpdateService>();
    let plan_migration = expect_context::<PlanMigrationService>();
    let update_visible = RwSignal::new(false);
    let hide_generation = RwSignal::new(0_u64);
    let heartbeat_services = RwSignal::new(Vec::<HeartbeatServiceView>::new());
    let left_process_index = RwSignal::new(0_usize);

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        let workspaces = wb
            .workspaces()
            .get()
            .into_iter()
            .map(|workspace| workspace.cwd.trim().to_string())
            .filter(|cwd| !cwd.is_empty())
            .collect::<Vec<_>>();
        spawn_local(async move {
            let _ = heartbeat_set_open_workspaces(workspaces).await;
        });
    });

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(list) = heartbeat_services_list().await {
                heartbeat_services.set(list);
            }
        });
        let listener = listen_heartbeat_services_changed(move |list| {
            heartbeat_services.set(list);
        });
        let listener = SendWrapper::new(listener);
        on_cleanup(move || {
            drop(listener.take());
        });
    });

    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                TimeoutFuture::new(3000).await;
                left_process_index.update(|idx| *idx = idx.wrapping_add(1));
            }
        });
    });

    Effect::new(move |_| {
        let active_id = wb.active_id().get();
        let workspaces = wb.workspaces().get();
        let harness_root = wb.harness_workspace_root().get();
        let cwd = active_id
            .and_then(|id| {
                workspaces
                    .iter()
                    .find(|workspace| workspace.id == id)
                    .map(|workspace| workspace.cwd.trim().to_string())
            })
            .filter(|cwd| !cwd.is_empty())
            .or_else(|| {
                let root = harness_root.trim();
                (!root.is_empty()).then(|| root.to_string())
            });
        if let Some(cwd) = cwd {
            plan_migration.ensure_for_workspace(cwd, wb);
        }
    });

    Effect::new(move |_| {
        let status = updates.status().get();
        let manual = updates.manual_check_active().get();
        let background = updates.check_source().get() == UpdateCheckSource::Background;
        if background {
            match status {
                UpdateUiStatus::Checking => update_visible.set(true),
                _ => update_visible.set(false),
            }
            return;
        }
        if !manual {
            update_visible.set(false);
            return;
        }
        match status {
            UpdateUiStatus::Checking
            | UpdateUiStatus::Available
            | UpdateUiStatus::Downloading
            | UpdateUiStatus::Installing
            | UpdateUiStatus::Done
            | UpdateUiStatus::Error
            | UpdateUiStatus::DevUnavailable => update_visible.set(true),
            UpdateUiStatus::UpToDate => {
                update_visible.set(true);
                hide_generation.update(|generation| *generation = generation.saturating_add(1));
                let generation = hide_generation.get_untracked();
                spawn_local(async move {
                    TimeoutFuture::new(2200).await;
                    if hide_generation.get_untracked() == generation {
                        update_visible.set(false);
                    }
                });
            }
            UpdateUiStatus::Idle => update_visible.set(false),
        }
    });

    let memory_indexer_visible = move || {
        heartbeat_services.with(|services| {
            services.iter().any(|service| {
                service.id == "memory_indexer"
                    && matches!(
                        service.status,
                        HeartbeatServiceStatus::Running | HeartbeatServiceStatus::Stalled
                    )
            })
        })
    };
    let memory_indexer_service = move || {
        heartbeat_services.with(|services| {
            services
                .iter()
                .find(|service| service.id == "memory_indexer")
                .cloned()
        })
    };
    let process_visible_count = move || {
        [
            update_visible.get(),
            plan_migration_statusline_visible(plan_migration),
            memory_indexer_visible(),
        ]
        .into_iter()
        .filter(|visible| *visible)
        .count()
        .max(1)
    };
    let process_slot = move || left_process_index.get() % process_visible_count();
    let process_item_index = move |target: usize| {
        let mut idx = 0usize;
        if update_visible.get() {
            if target == 0 {
                return Some(idx);
            }
            idx += 1;
        }
        if plan_migration_statusline_visible(plan_migration) {
            if target == 1 {
                return Some(idx);
            }
            idx += 1;
        }
        if memory_indexer_visible() && target == 2 {
            return Some(idx);
        }
        None
    };

    view! {
        <footer class="app-statusline" aria-label="Application status">
            <div class="app-statusline__slot app-statusline__slot--left">
                <Show when=move || update_visible.get() && process_item_index(0) == Some(process_slot())>
                    <span class=move || update_statusline_class(updates.status().get())>
                        <LxIcon
                            icon=move || update_statusline_icon(updates.status().get())
                            width="0.76rem"
                            height="0.76rem"
                        />
                        <span>{move || update_statusline_label(updates, i18n)}</span>
                    </span>
                </Show>
                <Show when=move || plan_migration_statusline_visible(plan_migration) && process_item_index(1) == Some(process_slot())>
                    <span class=move || plan_migration_statusline_class(plan_migration)>
                        <LxIcon
                            icon=move || plan_migration_statusline_icon(plan_migration)
                            width="0.76rem"
                            height="0.76rem"
                        />
                        <span>{move || plan_migration_statusline_label(plan_migration, i18n)}</span>
                    </span>
                </Show>
                <Show when=move || memory_indexer_visible() && process_item_index(2) == Some(process_slot())>
                    <HeartbeatStatusBarItem service=memory_indexer_service />
                </Show>
                <VimStatusIndicator />
            </div>
            <div class="app-statusline__slot app-statusline__slot--center">
                <CoreStatusBarItem />
            </div>
            <div class="app-statusline__slot app-statusline__slot--right">
                <HookStatusBarItem />
            </div>
        </footer>
    }
}

fn plan_migration_statusline_visible(service: PlanMigrationService) -> bool {
    let progress = service.progress().get();
    progress.busy || progress.phase == "error"
}

#[component]
fn HeartbeatStatusBarItem(
    service: impl Fn() -> Option<HeartbeatServiceView> + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <span class=move || heartbeat_statusline_class(service().as_ref())>
            <LxIcon
                icon=move || heartbeat_statusline_icon(service().as_ref())
                width="0.76rem"
                height="0.76rem"
            />
            <span>{move || heartbeat_statusline_label(service(), i18n)}</span>
        </span>
    }
}

fn heartbeat_statusline_class(service: Option<&HeartbeatServiceView>) -> String {
    let modifier = match service.map(|s| s.status) {
        Some(HeartbeatServiceStatus::Stalled | HeartbeatServiceStatus::Error) => {
            " app-statusline__item--heartbeat-warn"
        }
        Some(HeartbeatServiceStatus::Running) => " app-statusline__item--heartbeat-busy",
        _ => " app-statusline__item--quiet",
    };
    format!("app-statusline__item app-statusline__item--heartbeat{modifier}")
}

fn heartbeat_statusline_icon(service: Option<&HeartbeatServiceView>) -> icondata::Icon {
    match service.map(|s| s.status) {
        Some(HeartbeatServiceStatus::Stalled | HeartbeatServiceStatus::Error) => {
            icondata::LuCircleAlert
        }
        Some(HeartbeatServiceStatus::Running) => icondata::LuDatabaseZap,
        _ => icondata::LuHeartPulse,
    }
}

fn heartbeat_statusline_label(service: Option<HeartbeatServiceView>, i18n: I18nService) -> String {
    let Some(service) = service else {
        return "HeartBeat".into();
    };
    match service.status {
        HeartbeatServiceStatus::Stalled => i18n.tr(I18nKey::AppMemoryIndexStalled)().to_string(),
        HeartbeatServiceStatus::Error => service
            .last_response
            .unwrap_or_else(|| i18n.tr(I18nKey::AppMemoryIndexError)().to_string()),
        HeartbeatServiceStatus::Running => i18n.tr(I18nKey::AppMemoryIndexing)().to_string(),
        _ => service.name,
    }
}

fn plan_migration_statusline_class(service: PlanMigrationService) -> String {
    let progress = service.progress().get();
    let modifier = if progress.phase == "error" {
        " app-statusline__item--plan-migration-warn"
    } else {
        " app-statusline__item--plan-migration-busy"
    };
    format!("app-statusline__item app-statusline__item--plan-migration{modifier}")
}

fn plan_migration_statusline_icon(service: PlanMigrationService) -> icondata::Icon {
    if service.progress().get().phase == "error" {
        icondata::LuCircleAlert
    } else {
        icondata::LuFolderSync
    }
}

fn plan_migration_statusline_label(service: PlanMigrationService, i18n: I18nService) -> String {
    let progress = service.progress().get();
    if progress.phase == "error" {
        return progress
            .error
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| i18n.tr(I18nKey::AppPlanMigrationFailed)().to_string());
    }
    if progress.total > 0 {
        format!("Plans {}/{}", progress.processed, progress.total)
    } else {
        "Plans".into()
    }
}

fn update_statusline_class(status: UpdateUiStatus) -> String {
    let modifier = match status {
        UpdateUiStatus::Checking | UpdateUiStatus::Downloading | UpdateUiStatus::Installing => {
            " app-statusline__item--update-busy"
        }
        UpdateUiStatus::Available | UpdateUiStatus::Done => {
            " app-statusline__item--update-available"
        }
        UpdateUiStatus::UpToDate => " app-statusline__item--update-ok",
        UpdateUiStatus::Error | UpdateUiStatus::DevUnavailable => {
            " app-statusline__item--update-warn"
        }
        UpdateUiStatus::Idle => "",
    };
    format!("app-statusline__item app-statusline__item--update{modifier}")
}

fn update_statusline_icon(status: UpdateUiStatus) -> icondata::Icon {
    match status {
        UpdateUiStatus::Checking | UpdateUiStatus::Downloading | UpdateUiStatus::Installing => {
            icondata::LuRefreshCw
        }
        UpdateUiStatus::Available | UpdateUiStatus::Done => icondata::LuCircleArrowUp,
        UpdateUiStatus::UpToDate => icondata::LuCircleCheck,
        UpdateUiStatus::Error | UpdateUiStatus::DevUnavailable => icondata::LuCircleAlert,
        UpdateUiStatus::Idle => icondata::LuRefreshCw,
    }
}

fn update_statusline_label(updates: UpdateService, i18n: I18nService) -> String {
    match updates.status().get() {
        UpdateUiStatus::Checking => i18n.tr(I18nKey::AppUpdateChecking)().to_string(),
        UpdateUiStatus::Available => {
            let version = updates.available_version().get().unwrap_or_default();
            if version.trim().is_empty() {
                i18n.tr(I18nKey::UpdateBannerTitle)().to_string()
            } else {
                format!("{} {version}", i18n.tr(I18nKey::UpdateBannerTitle)())
            }
        }
        UpdateUiStatus::UpToDate => i18n.tr(I18nKey::AppUpdateUpToDate)().to_string(),
        UpdateUiStatus::Downloading => {
            let progress = updates
                .progress_pct()
                .get()
                .map(|pct| format!(" {pct:.0}%"))
                .unwrap_or_default();
            format!(
                "{}{}",
                i18n.tr(I18nKey::UpdateDialogDownloading)(),
                progress
            )
        }
        UpdateUiStatus::Installing => i18n.tr(I18nKey::UpdateDialogInstalling)().to_string(),
        UpdateUiStatus::Done => i18n.tr(I18nKey::UpdateDialogDone)().to_string(),
        UpdateUiStatus::Error => updates
            .message()
            .get()
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| i18n.tr(I18nKey::UpdateDialogError)().to_string()),
        UpdateUiStatus::DevUnavailable => i18n.tr(I18nKey::AppUpdateDevUnavailable)().to_string(),
        UpdateUiStatus::Idle => String::new(),
    }
}

fn remove_static_boot_screen() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if let Some(el) = doc.get_element_by_id("blx-static-boot") {
        el.remove();
    }
}

fn open_external(url: &str) {
    if crate::tauri_bridge::is_tauri_shell() {
        let owned = url.to_string();
        spawn_local(async move {
            if crate::tauri_bridge::open_external_url(&owned)
                .await
                .is_err()
            {
                open_via_dom_window(&owned);
            }
        });
        return;
    }
    open_via_dom_window(url);
}

fn open_via_dom_window(url: &str) {
    let Some(win) = web_sys::window() else {
        return;
    };
    let opened = win.open_with_url_and_target(url, "_blank").ok().flatten();
    if opened.is_none() {
        let _ = win.location().set_href(url);
    }
}
