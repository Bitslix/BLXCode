use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{pty_write, run_commands_discover, RunCommand, RunCommandKind};
use crate::workbench::state::SlotPaneState;
use crate::workbench::WorkbenchService;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use gloo_timers::future::TimeoutFuture;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActiveRunScope {
    workspace_id: u64,
    storage_key: String,
    cwd: String,
    connection_id: Option<String>,
}

#[component]
pub fn RunMenu() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let wb = expect_context::<WorkbenchService>();
    let open = RwSignal::new(false);
    let loading = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let commands = RwSignal::new(Vec::<RunCommand>::new());

    let active_scope = Memo::new(move |_| {
        let active = wb.active_id().get()?;
        wb.workspaces().with(|list| {
            let workspace = list.iter().find(|workspace| workspace.id == active)?;
            let cwd = workspace.cwd.trim().to_string();
            if cwd.is_empty() {
                return None;
            }
            Some(ActiveRunScope {
                workspace_id: workspace.id,
                storage_key: workspace.storage_key.clone(),
                cwd,
                connection_id: workspace.remote_connection_id.clone(),
            })
        })
    });

    let close_click = window_event_listener_untyped("click", move |ev| {
        if !open.get_untracked() {
            return;
        }
        let inside = ev
            .target()
            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
            .and_then(|element| element.closest(".app-titlebar__run-wrap").ok().flatten())
            .is_some();
        if !inside {
            open.set(false);
        }
    });
    let close_esc = window_event_listener_untyped("keydown", move |ev| {
        let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if ev.key() == "Escape" {
            open.set(false);
        }
    });
    on_cleanup(move || {
        close_click.remove();
        close_esc.remove();
    });

    let refresh = move || {
        let Some(scope) = active_scope.get_untracked() else {
            commands.set(Vec::new());
            return;
        };
        loading.set(true);
        error.set(None);
        spawn_local(async move {
            match run_commands_discover(scope.cwd, scope.connection_id).await {
                Ok(next) => {
                    commands.set(next);
                    error.set(None);
                }
                Err(err) => {
                    commands.set(Vec::new());
                    error.set(Some(err));
                }
            }
            loading.set(false);
        });
    };

    let launch = move |command: RunCommand| {
        let Some(scope) = active_scope.get_untracked() else {
            return;
        };
        wb.open_center_terminals_tab(scope.workspace_id);
        let Ok(slot_id) = wb.append_terminal_slot(scope.workspace_id, None) else {
            return;
        };
        let pane_id = SlotPaneState::default_for_slot(slot_id).pane_ids[0];
        let terminal_key = format!("{}:{slot_id}:{pane_id}", scope.storage_key);
        wb.focus_terminal(terminal_key.clone());
        wb.bump_terminal_layout();
        open.set(false);

        spawn_local(async move {
            let Some(session_id) = wait_for_terminal_session(wb, &terminal_key).await else {
                return;
            };
            let payload = shell_payload(&command);
            let encoded = BASE64.encode(payload.as_bytes());
            let _ = pty_write(session_id, encoded).await;
        });
    };

    view! {
        <div class="app-titlebar__menu-wrap app-titlebar__run-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label=move || i18n.tr(I18nKey::TbRunMenu)()
                title=move || i18n.tr(I18nKey::TbRunMenu)()
                disabled=move || active_scope.get().is_none()
                on:click=move |ev| {
                    ev.stop_propagation();
                    let next = !open.get_untracked();
                    open.set(next);
                    if next {
                        refresh();
                    }
                }
            >
                <LxIcon icon=icondata::LuPlay width="1rem" height="1rem" />
            </button>

            <Show when=move || open.get()>
                <div class="app-titlebar__popover app-titlebar__popover--run" role="menu">
                    <div class="app-titlebar__popover-head-row">
                        <p class="app-titlebar__popover-head">{move || i18n.tr(I18nKey::TbRunMenu)()}</p>
                        <button
                            type="button"
                            class="app-titlebar__notif-action"
                            aria-label=move || i18n.tr(I18nKey::TbRunRefresh)()
                            title=move || i18n.tr(I18nKey::TbRunRefresh)()
                            on:click=move |ev| {
                                ev.stop_propagation();
                                refresh();
                            }
                        >
                            <LxIcon icon=icondata::LuRefreshCw width="0.9rem" height="0.9rem" />
                        </button>
                    </div>

                    <Show when=move || loading.get()>
                        <p class="app-titlebar__menu-note">{move || i18n.tr(I18nKey::TbRunLoading)()}</p>
                    </Show>
                    <Show when=move || error.get().is_some()>
                        <p class="app-titlebar__menu-note app-titlebar__menu-note--error">
                            {move || error.get().unwrap_or_default()}
                        </p>
                    </Show>

                    <Show
                        when=move || !commands.get().is_empty()
                        fallback=move || view! {
                            <Show when=move || !loading.get() && error.get().is_none()>
                                <p class="app-titlebar__menu-note">{move || i18n.tr(I18nKey::TbRunEmpty)()}</p>
                            </Show>
                        }
                    >
                        <div class="app-titlebar__run-list">
                            <For
                                each=move || commands.get()
                                key=|command| command.id.clone()
                                children=move |command| {
                                    let cmd_for_launch = command.clone();
                                    let kind_key = run_kind_key(command.kind.clone());
                                    view! {
                                        <button
                                            type="button"
                                            class="app-titlebar__menu-item app-titlebar__run-item"
                                            role="menuitem"
                                            title=command.command.clone()
                                            on:click=move |_| launch(cmd_for_launch.clone())
                                        >
                                            <LxIcon icon=run_kind_icon(&command.kind) width="0.95rem" height="0.95rem" />
                                            <span class="app-titlebar__menu-item-label app-titlebar__run-label">
                                                <span>{command.label.clone()}</span>
                                                <span class="app-titlebar__menu-item-workspace">{command_display_path(&command)}</span>
                                            </span>
                                            <span class="app-titlebar__run-kind">
                                                {move || i18n.tr(kind_key)()}
                                            </span>
                                        </button>
                                    }
                                }
                            />
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

async fn wait_for_terminal_session(wb: WorkbenchService, terminal_key: &str) -> Option<u64> {
    for _ in 0..80 {
        if let Some(session_id) = wb
            .pty_sessions_signal()
            .get_untracked()
            .get(terminal_key)
            .copied()
        {
            return Some(session_id);
        }
        TimeoutFuture::new(100).await;
    }
    None
}

fn shell_payload(command: &RunCommand) -> String {
    let cmd = command.command.trim();
    let cwd = command.cwd_rel.trim();
    if cwd.is_empty() {
        format!("{cmd}\n")
    } else {
        format!("cd -- {} && {cmd}\n", shell_quote(cwd))
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn command_display_path(command: &RunCommand) -> String {
    if command.cwd_rel.is_empty() {
        command.command.clone()
    } else {
        format!("{} · {}", command.cwd_rel, command.command)
    }
}

fn run_kind_key(kind: RunCommandKind) -> I18nKey {
    match kind {
        RunCommandKind::Dev => I18nKey::TbRunDev,
        RunCommandKind::Run => I18nKey::TbRunRun,
        RunCommandKind::Debug => I18nKey::TbRunDebug,
        RunCommandKind::Test => I18nKey::TbRunTest,
        RunCommandKind::Build => I18nKey::TbRunBuild,
        RunCommandKind::Other => I18nKey::TbRunOther,
    }
}

fn run_kind_icon(kind: &RunCommandKind) -> icondata::Icon {
    match kind {
        RunCommandKind::Dev => icondata::LuWrench,
        RunCommandKind::Run => icondata::LuPlay,
        RunCommandKind::Debug => icondata::LuBug,
        RunCommandKind::Test => icondata::LuBadgeCheck,
        RunCommandKind::Build => icondata::LuHammer,
        RunCommandKind::Other => icondata::LuTerminal,
    }
}
