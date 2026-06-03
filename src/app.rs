use crate::boot_loading::{BootLoadingScreen, BootPhase};
use crate::config::EULA_STORAGE_KEY;
use crate::i18n::{localized_eula_html, I18nKey};
use crate::open_http::dom_click_http_url_from_mouse_event;
use crate::quit::request_app_quit;
use crate::service::I18nService;
use crate::workbench::AppTitleBar;
use crate::workbench::ThemeService;
use crate::workbench::UpdateService;
use crate::workbench::UpdateUiStatus;
use crate::workbench::WorkbenchService;
use crate::workbench::WorkbenchShell;
use crate::workbench::{CoreStatusBarItem, CoreStatusService};
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
    // Provided at the App root so the sibling `AppStatusLine` can show the
    // enabled-rules/skills counts for the active workspace in its centre slot.
    let core_status = CoreStatusService::new();
    provide_context(i18n);
    provide_context(theme);
    provide_context(wb);
    provide_context(hook_status);
    provide_context(hook_install);
    provide_context(updates);
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

    let show_workbench = move || eula_ok.get();
    let show_eula = move || !eula_ok.get();

    // Workspace-scoped title-bar controls appear only once the UI is ready and
    // the EULA is accepted; during boot/EULA only the brand + window controls
    // (drag, minimize, maximize, close) render.
    let workbench_active = Signal::derive(move || ui_ready.get() && eula_ok.get());

    view! {
        <div class="app-root">
            <AppTitleBar workbench_active=workbench_active />
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
                        <WorkbenchShell/>
                    </Show>
                </Show>
            </div>
            <Show when=move || workbench_active.get()>
                <AppStatusLine />
            </Show>
        </div>
    }
}

#[component]
fn AppStatusLine() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let updates = expect_context::<UpdateService>();
    let update_visible = RwSignal::new(false);
    let hide_generation = RwSignal::new(0_u64);

    Effect::new(move |_| {
        let status = updates.status().get();
        let manual = updates.manual_check_active().get();
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

    view! {
        <footer class="app-statusline" aria-label="Application status">
            <div class="app-statusline__slot app-statusline__slot--left">
                <Show when=move || update_visible.get()>
                    <span class=move || update_statusline_class(updates.status().get())>
                        <LxIcon
                            icon=move || update_statusline_icon(updates.status().get())
                            width="0.76rem"
                            height="0.76rem"
                        />
                        <span>{move || update_statusline_label(updates, i18n)}</span>
                    </span>
                </Show>
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
