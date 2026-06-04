//! Notifications bell + popover for persistent BLXCode Agent notifications.
use super::TitleBarFeed;
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, workbench_list_agent_notifications, workbench_mark_agent_notifications_read,
    workbench_remove_agent_notification,
};
use crate::workbench::{HarnessSettingsCategory, RightPanelTab, UpdateService, WorkbenchService};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

#[component]
pub fn NotificationsMenu() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let feed = expect_context::<TitleBarFeed>();
    let wb = expect_context::<WorkbenchService>();
    let updates = expect_context::<UpdateService>();
    let open = RwSignal::new(false);

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            if let Ok(items) = workbench_list_agent_notifications(true, 200).await {
                wb.set_agent_notifications(items);
            }
        });
    });

    let close_click = window_event_listener_untyped("click", move |ev| {
        if !open.get_untracked() {
            return;
        }
        let inside = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".app-titlebar__menu-wrap").ok().flatten())
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

    let has_unread = move || feed.items.with(|items| items.iter().any(|item| !item.read));
    let has_items = move || feed.items.with(|items| !items.is_empty());
    let unread_count = move || {
        feed.items
            .with(|items| items.iter().filter(|item| !item.read).count())
    };

    view! {
        <div class="app-titlebar__menu-wrap">
            <button
                type="button"
                class="app-titlebar__icon-btn"
                class:app-titlebar__icon-btn--active=move || open.get()
                aria-haspopup="menu"
                aria-expanded=move || open.get().to_string()
                aria-label=move || i18n.tr(I18nKey::TbNotifications)()
                title=move || i18n.tr(I18nKey::TbNotifications)()
                on:click=move |ev| {
                    ev.stop_propagation();
                    open.update(|o| *o = !*o);
                }
            >
                <LxIcon icon=icondata::LuBell width="1rem" height="1rem" />
                <Show when=move || has_unread()>
                    <span class="app-titlebar__bell-dot" aria-hidden="true">
                        {move || unread_count().min(9).to_string()}
                    </span>
                </Show>
            </button>
            <Show when=move || open.get()>
                <div class="app-titlebar__popover app-titlebar__popover--notifications" role="menu">
                    <div class="app-titlebar__popover-head-row">
                        <p class="app-titlebar__popover-head">{move || i18n.tr(I18nKey::TbNotifications)()}</p>
                        <Show when=move || has_unread()>
                            <button
                                type="button"
                                class="app-titlebar__notif-action"
                                title=move || i18n.tr(I18nKey::NotificationsMenuMarkAllRead)()
                                aria-label=move || i18n.tr(I18nKey::NotificationsMenuMarkAllRead)()
                                on:click=move |ev| {
                                    ev.stop_propagation();
                                    wb.mark_all_agent_notifications_read();
                                    leptos::task::spawn_local(async move {
                                        let _ = workbench_mark_agent_notifications_read(None, true).await;
                                    });
                                }
                            >
                                <LxIcon icon=icondata::LuCheckCheck width="0.85rem" height="0.85rem" />
                            </button>
                        </Show>
                    </div>
                    <Show
                        when=move || has_items()
                        fallback=move || view! {
                            <div class="app-titlebar__notif-empty">
                                <LxIcon icon=icondata::LuBell width="1.4rem" height="1.4rem" />
                                <p class="app-titlebar__notif-empty-title">
                                    {move || i18n.tr(I18nKey::TbNotificationsEmptyTitle)()}
                                </p>
                                <p class="app-titlebar__notif-empty-body">
                                    {move || i18n.tr(I18nKey::TbNotificationsEmptyBody)()}
                                </p>
                            </div>
                        }
                    >
                        <ul class="app-titlebar__notif-list">
                            <For
                                each=move || feed.items.get()
                                key=|item| item.id.clone()
                                children=move |item| {
                                    let item_id = item.id.clone();
                                    let remove_id = item.id.clone();
                                    let target = item.target.clone();
                                    let title = item.title.clone();
                                    let kind = item.kind.clone();
                                    let body = item.body.clone().unwrap_or_default();
                                    let has_body = !body.is_empty();
                                    let unread = !item.read;
                                    let wb_open = wb;
                                    let wb_remove = wb;
                                    let updates_open = updates;
                                    view! {
                                        <li
                                            class="app-titlebar__notif-item"
                                            class:app-titlebar__notif-item--unread=move || unread
                                        >
                                            <button
                                                type="button"
                                                class="app-titlebar__notif-main"
                                                on:click=move |_| {
                                                    open_notification_target(wb_open, updates_open, target.clone());
                                                    wb_open.mark_agent_notification_read(&item_id);
                                                    let id = item_id.clone();
                                                    leptos::task::spawn_local(async move {
                                                        let _ = workbench_mark_agent_notifications_read(Some(id), false).await;
                                                    });
                                                }
                                            >
                                                <span class="app-titlebar__notif-icon" aria-hidden="true">
                                                    {notification_icon(&kind)}
                                                </span>
                                                <span class="app-titlebar__notif-copy">
                                                    <span class="app-titlebar__notif-item-title">{title}</span>
                                                    <Show when=move || has_body>
                                                        <span class="app-titlebar__notif-item-body">
                                                            {body.clone()}
                                                        </span>
                                                    </Show>
                                                </span>
                                            </button>
                                            <button
                                                type="button"
                                                class="app-titlebar__notif-remove"
                                                title=move || i18n.tr(I18nKey::SrRemove)()
                                                aria-label=move || i18n.tr(I18nKey::CommonRemoveNotification)()
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    wb_remove.remove_agent_notification(&remove_id);
                                                    let id = remove_id.clone();
                                                    leptos::task::spawn_local(async move {
                                                        let _ = workbench_remove_agent_notification(id).await;
                                                    });
                                                }
                                            >
                                                <LxIcon icon=icondata::LuX width="0.78rem" height="0.78rem" />
                                            </button>
                                        </li>
                                    }
                                }
                            />
                        </ul>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

fn notification_icon(kind: &str) -> AnyView {
    match kind {
        "error" => view! { <LxIcon icon=icondata::LuCircleAlert width="0.9rem" height="0.9rem" /> }
            .into_any(),
        "question" => {
            view! { <LxIcon icon=icondata::LuCircleHelp width="0.9rem" height="0.9rem" /> }
                .into_any()
        }
        "plan_completed" | "task_completed" => {
            view! { <LxIcon icon=icondata::LuCircleCheck width="0.9rem" height="0.9rem" /> }
                .into_any()
        }
        "cli_agent_response" => {
            view! { <LxIcon icon=icondata::LuMessagesSquare width="0.9rem" height="0.9rem" /> }
                .into_any()
        }
        "update" => {
            view! { <LxIcon icon=icondata::LuCircleArrowUp width="0.9rem" height="0.9rem" /> }
                .into_any()
        }
        _ => view! { <LxIcon icon=icondata::LuBell width="0.9rem" height="0.9rem" /> }.into_any(),
    }
}

fn open_notification_target(
    wb: WorkbenchService,
    updates: UpdateService,
    target: Option<serde_json::Value>,
) {
    let Some(target) = target else {
        return;
    };
    let view = target
        .get("view")
        .or_else(|| target.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match view {
        "agent" => {
            wb.set_right_tab(RightPanelTab::Agent);
            if wb.right_collapsed().get_untracked() {
                wb.toggle_right_panel();
            }
        }
        "plans" | "plan" => {
            wb.set_right_tab(RightPanelTab::Plans);
            if wb.right_collapsed().get_untracked() {
                wb.toggle_right_panel();
            }
        }
        "kanban" => {
            if let Some(ws_id) = wb.active_id().get_untracked() {
                wb.open_center_kanban_tab(ws_id);
            }
        }
        "memory" => {
            wb.set_right_tab(RightPanelTab::Memory);
            if wb.right_collapsed().get_untracked() {
                wb.toggle_right_panel();
            }
        }
        "settings" => wb.open_center_settings_tab(HarnessSettingsCategory::App),
        "update" => updates.open_dialog(),
        "file" => {
            if let (Some(ws_id), Some(path)) = (
                wb.active_id().get_untracked(),
                target.get("path").and_then(|v| v.as_str()),
            ) {
                wb.open_center_file_tab(ws_id, path.to_string());
            }
        }
        "diff" => {
            if let (Some(ws_id), Some(path)) = (
                wb.active_id().get_untracked(),
                target.get("path").and_then(|v| v.as_str()),
            ) {
                let staged = target
                    .get("staged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                wb.open_center_diff_tab(ws_id, path.to_string(), staged);
            }
        }
        _ => {}
    }
}
