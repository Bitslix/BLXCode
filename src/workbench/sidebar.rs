use crate::config::{
    SIDEBAR_EXPLORER_HEIGHT_PCT_DEFAULT, SIDEBAR_EXPLORER_HEIGHT_PCT_KEY,
    SIDEBAR_EXPLORER_HEIGHT_PCT_MAX, SIDEBAR_EXPLORER_HEIGHT_PCT_MIN,
};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{git_is_repository, is_tauri_shell};
use crate::workbench::git_graph::GitGraphSection;
use crate::workbench::project_explorer::ProjectExplorerSection;
use crate::workbench::sidebar_resizer::SidebarResizer;
use crate::workbench::WorkbenchService;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, HtmlInputElement};

const APP_NAME: &str = "blxcode";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn workspace_icon_label(title: &str, fallback_num: u64) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return fallback_num.to_string();
    }
    let mut out = String::new();
    for word in trimmed.split_whitespace() {
        if let Some(ch) = word.chars().find(|c| c.is_alphanumeric()) {
            out.extend(ch.to_uppercase());
        }
        if out.len() >= 2 {
            break;
        }
    }
    if out.is_empty() {
        fallback_num.to_string()
    } else {
        out
    }
}

#[component]
pub fn Sidebar() -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let i18n = expect_context::<I18nService>();

    let collapsed = wb.sidebar_collapsed();
    let workspaces = wb.workspaces();
    let explorer_height_pct = RwSignal::new(read_explorer_height_pct());
    Effect::new(move |_| {
        write_explorer_height_pct(explorer_height_pct.get());
    });
    let context_menu = RwSignal::new(None::<WorkspaceContextMenu>);
    let rename_dialog = RwSignal::new(None::<RenameWorkspaceDialog>);
    let rename_input = RwSignal::new(String::new());
    let drag_from = RwSignal::new(None::<usize>);
    let drag_over = RwSignal::new(None::<usize>);
    let git_repo_available = RwSignal::new(None::<bool>);
    let last_git_cwd = StoredValue::new(None::<String>);

    // Re-check only when workspace selection, harness root, or repo path changes —
    // not on every `workspaces` mutation (agent timeline, compose draft, etc.).
    Effect::new(move |_| {
        let _ = wb.active_id().get();
        let _ = wb.harness_workspace_root().get();
        let _ = wb.sidebar_repo_epoch().get();
        if !is_tauri_shell() {
            git_repo_available.set(Some(false));
            last_git_cwd.set_value(None);
            return;
        }
        let cwd = wb.default_workspace_cwd();
        let Some(cwd) = cwd.filter(|c| !c.trim().is_empty()) else {
            git_repo_available.set(Some(false));
            last_git_cwd.set_value(None);
            return;
        };
        if last_git_cwd.with_value(|prev| prev.as_deref() == Some(cwd.as_str()))
            && git_repo_available.get_untracked().is_some()
        {
            return;
        }
        last_git_cwd.set_value(Some(cwd.clone()));
        let cwd_check = cwd.clone();
        spawn_local(async move {
            let ok = git_is_repository(cwd_check).await.unwrap_or(false);
            git_repo_available.set(Some(ok));
        });
    });

    let close_menu_click = window_event_listener_untyped("click", move |_| {
        context_menu.set(None);
    });
    let close_menu_escape = window_event_listener_untyped("keydown", move |ev| {
        let Some(ev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        if ev.key() == "Escape" {
            context_menu.set(None);
            rename_dialog.set(None);
        }
    });

    on_cleanup(move || {
        close_menu_click.remove();
        close_menu_escape.remove();
    });

    view! {
        <aside
            class=move || {
                let mut c = String::from("workbench-sidebar");
                if collapsed.get() {
                    c.push_str(" workbench-sidebar--collapsed");
                }
                c
            }
            aria-label=move || i18n.tr(I18nKey::SbAria)()
        >
            <header class=move || {
                if collapsed.get() {
                    "workbench-gutter-bar".to_string()
                } else {
                    "workbench-sidebar__header".to_string()
                }
            }>
                <Show
                    when=move || !collapsed.get()
                    fallback=move || view! {
                        <button
                            type="button"
                            class="workbench-icon-btn"
                            aria-expanded="false"
                            aria-label=move || i18n.tr(I18nKey::SbExpand)()
                            on:click=move |_| wb.toggle_sidebar()
                        >
                            "›"
                        </button>
                    }
                >
                    <div class="workbench-sidebar__title-row">
                        <span class="workbench-sidebar__title">{move || i18n.tr(I18nKey::SbHeading)()}</span>
                        <button
                            type="button"
                            class="workbench-sidebar__add-btn"
                            aria-label=move || i18n.tr(I18nKey::SbAddWorkspaceAria)()
                            on:click=move |_| { let _ = wb.start_inline_configure(); }
                        >
                            "+"
                        </button>
                    </div>
                    <button
                        type="button"
                        class="workbench-icon-btn"
                        aria-expanded="true"
                        aria-controls="workbench-workspace-list"
                        aria-label=move || i18n.tr(I18nKey::SbCollapse)()
                        on:click=move |_| wb.toggle_sidebar()
                    >
                        "«"
                    </button>
                </Show>
            </header>

            <nav class="workbench-sidebar__nav">
                <ul id="workbench-workspace-list" class="workbench-sidebar__list">
                    <For
                        each=move || {
                            workspaces
                                .get()
                                .into_iter()
                                .enumerate()
                                .collect::<Vec<_>>()
                        }
                        key=|(_, ws)| ws.id
                        children=move |(idx, entry)| {
                            let id = entry.id;
                            let title_signal = Memo::new(move |_| {
                                workspaces.with(|list| {
                                    list.iter()
                                        .find(|w| w.id == id)
                                        .map(|w| w.title.clone())
                                        .unwrap_or_else(|| format!("Workspace {id}"))
                                })
                            });
                            let title_str = move || title_signal.get();
                            let icon_label = move || {
                                workspace_icon_label(&title_signal.get(), id)
                            };
                            let badge_counts = Memo::new(move |_| {
                                let _ = wb.notifications().get();
                                let _ = wb.workspaces().get();
                                wb.workspace_notification_counts(id)
                            });
                            view! {
                                <li
                                    class=move || {
                                        let mut c = String::from("workbench-sidebar__item");
                                        if drag_from.get() == Some(idx) {
                                            c.push_str(" workbench-sidebar__item--drag-source");
                                        }
                                        if drag_over.get() == Some(idx)
                                            && drag_from.get() != Some(idx)
                                        {
                                            c.push_str(" workbench-sidebar__item--drag-over");
                                        }
                                        c
                                    }
                                    prop:draggable=move || !collapsed.get()
                                    on:dragstart=move |ev| {
                                        if collapsed.get_untracked() {
                                            return;
                                        }
                                        drag_from.set(Some(idx));
                                        drag_over.set(None);
                                        if let Some(de) = ev.dyn_ref::<DragEvent>() {
                                            if let Some(dt) = de.data_transfer() {
                                                let _ = dt.set_data("text/plain", &id.to_string());
                                                let _ = dt.set_effect_allowed("move");
                                            }
                                        }
                                    }
                                    on:dragover=move |ev| {
                                        if collapsed.get_untracked() || drag_from.get_untracked().is_none()
                                        {
                                            return;
                                        }
                                        ev.prevent_default();
                                        if let Some(de) = ev.dyn_ref::<DragEvent>() {
                                            if let Some(dt) = de.data_transfer() {
                                                let _ = dt.set_drop_effect("move");
                                            }
                                        }
                                        drag_over.set(Some(idx));
                                    }
                                    on:drop=move |ev| {
                                        ev.prevent_default();
                                        if collapsed.get_untracked() {
                                            return;
                                        }
                                        if let Some(from) = drag_from.get_untracked() {
                                            wb.reorder_workspaces(from, idx);
                                        }
                                        drag_from.set(None);
                                        drag_over.set(None);
                                    }
                                    on:dragend=move |_| {
                                        drag_from.set(None);
                                        drag_over.set(None);
                                    }
                                >
                                    <button
                                        type="button"
                                        title=title_str
                                        class=move || {
                                            let mut c =
                                                String::from("workbench-sidebar__row");
                                            if wb.active_id().get() == Some(id) {
                                                c.push_str(" workbench-sidebar__row--active");
                                            }
                                            c
                                        }
                                        on:click=move |_| wb.select_workspace(id)
                                        on:contextmenu=move |ev| {
                                            ev.prevent_default();
                                            context_menu.set(Some(WorkspaceContextMenu {
                                                workspace_id: id,
                                                title: title_signal.get(),
                                                x: ev.client_x(),
                                                y: ev.client_y(),
                                            }));
                                        }
                                    >
                                        <span class="workbench-sidebar__icon" aria-hidden="true">
                                            {icon_label}
                                        </span>
                                        <span class="workbench-sidebar__label">
                                            <span class="workbench-sidebar__bullet">"▸ "</span>
                                            {move || title_signal.get()}
                                        </span>
                                        <Show when=move || {
                                            !collapsed.get() && badge_counts.get().total_unread > 0
                                        }>
                                            {move || {
                                                let total = badge_counts.get().total_unread;
                                                let aria = i18n
                                                    .tr(I18nKey::SbBadgeTotalAria)()
                                                    .replace("{n}", &total.to_string());
                                                view! {
                                                    <span class="workbench-sidebar__badges">
                                                        <span
                                                            class="workbench-sidebar__badge workbench-sidebar__badge--total"
                                                            aria-label=aria
                                                            title=move || total.to_string()
                                                        >
                                                            {total.to_string()}
                                                        </span>
                                                    </span>
                                                }
                                            }}
                                        </Show>
                                    </button>
                                    <button
                                        type="button"
                                        class="workbench-sidebar__close"
                                        prop:draggable=false
                                        title=move || format!("Close {}", title_signal.get())
                                        aria-label=move || format!("Close {}", title_signal.get())
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            wb.close_workspace(id);
                                        }
                                    >"×"</button>
                                </li>
                            }
                            .into_any()
                        }
                    />
                </ul>
                <Show when=move || collapsed.get()>
                    <div class="workbench-sidebar__collapsed-actions">
                        <button
                            type="button"
                            class="workbench-sidebar__collapsed-add"
                            aria-label=move || i18n.tr(I18nKey::SbAddWorkspaceAria)()
                            on:click=move |_| { let _ = wb.start_inline_configure(); }
                        >
                            "+"
                        </button>
                    </div>
                </Show>
            </nav>

            <Show when=move || !collapsed.get()>
                <div class="workbench-sidebar__views">
                    <div
                        class="workbench-sidebar__explorer-slot"
                        style=move || {
                            format!(
                                "flex: 0 0 {pct:.2}%; min-height: 0;",
                                pct = explorer_height_pct.get(),
                            )
                        }
                    >
                        <ProjectExplorerSection />
                    </div>
                    <SidebarResizer
                        height_pct=explorer_height_pct
                        container_selector=".workbench-sidebar__views"
                    />
                    <div class="workbench-sidebar__graph-slot">
                        <GitGraphSection git_repo_available=git_repo_available.read_only() />
                    </div>
                </div>
            </Show>

            <div class="workbench-sidebar__footer">
                <div class="sidebar-app-brand" aria-label=APP_NAME>
                    <span class="sidebar-app-brand__name">{APP_NAME}</span>
                    <span class="sidebar-app-brand__version">{format!("v{APP_VERSION}")}</span>
                </div>
            </div>
            <Show when=move || context_menu.get().is_some()>
                {move || {
                    let Some(menu) = context_menu.get() else {
                        return view! { <div></div> }.into_any();
                    };
                    view! {
                        <div
                            class="workspace-context-menu"
                            style=format!("left:{}px;top:{}px;", menu.x, menu.y)
                            role="menu"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            <button
                                type="button"
                                class="workspace-context-menu__item"
                                role="menuitem"
                                on:click=move |_| {
                                    context_menu.set(None);
                                    rename_input.set(menu.title.clone());
                                    rename_dialog.set(Some(RenameWorkspaceDialog {
                                        workspace_id: menu.workspace_id,
                                    }));
                                }
                            >
                                {move || i18n.tr(I18nKey::SbRenameMenu)()}
                            </button>
                            <button
                                type="button"
                                class="workspace-context-menu__item workspace-context-menu__item--danger"
                                role="menuitem"
                                on:click=move |_| {
                                    context_menu.set(None);
                                    wb.close_workspace(menu.workspace_id);
                                }
                            >
                                {move || i18n.tr(I18nKey::SbCloseWorkspaceMenu)()}
                            </button>
                        </div>
                    }
                    .into_any()
                }}
            </Show>
            <Show when=move || rename_dialog.get().is_some()>
                {move || {
                    let Some(dialog) = rename_dialog.get() else {
                        return view! { <div></div> }.into_any();
                    };
                    let save = move || {
                        let next = rename_input.get_untracked();
                        if !next.trim().is_empty() {
                            wb.rename_workspace(dialog.workspace_id, next);
                        }
                        rename_dialog.set(None);
                    };
                    view! {
                        <div class="workspace-rename-backdrop" role="presentation">
                            <section
                                class="workspace-rename-dialog"
                                role="dialog"
                                aria-modal="true"
                                aria-labelledby="workspace-rename-title"
                            >
                                <header class="workspace-rename-dialog__head">
                                    <h2 id="workspace-rename-title">{move || i18n.tr(I18nKey::SbRenameTitle)()}</h2>
                                    <button
                                        type="button"
                                        class="workspace-rename-dialog__close"
                                        aria-label=move || i18n.tr(I18nKey::SbRenameCloseAria)()
                                        on:click=move |_| rename_dialog.set(None)
                                    >
                                        "×"
                                    </button>
                                </header>
                                <div class="workspace-rename-dialog__body">
                                    <label class="workspace-rename-dialog__label" for="workspace-rename-input">
                                        {move || i18n.tr(I18nKey::SbRenameNameLabel)()}
                                    </label>
                                    <input
                                        id="workspace-rename-input"
                                        class="workspace-rename-dialog__input"
                                        type="text"
                                        prop:value=move || rename_input.get()
                                        on:input=move |ev| {
                                            let Some(input) = ev
                                                .target()
                                                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
                                            else {
                                                return;
                                            };
                                            rename_input.set(input.value());
                                        }
                                        on:keydown=move |ev| {
                                            if ev.key() == "Enter" {
                                                ev.prevent_default();
                                                save();
                                            }
                                        }
                                    />
                                </div>
                                <footer class="workspace-rename-dialog__actions">
                                    <button
                                        type="button"
                                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--ghost"
                                        on:click=move |_| rename_dialog.set(None)
                                    >
                                        {move || i18n.tr(I18nKey::MemCancel)()}
                                    </button>
                                    <button
                                        type="button"
                                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                                        on:click=move |_| save()
                                        disabled=move || rename_input.get().trim().is_empty()
                                    >
                                        {move || i18n.tr(I18nKey::SbRenameSubmit)()}
                                    </button>
                                </footer>
                            </section>
                        </div>
                    }
                    .into_any()
                }}
            </Show>
        </aside>
    }
}

#[derive(Clone, Debug)]
struct WorkspaceContextMenu {
    workspace_id: u64,
    title: String,
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug)]
struct RenameWorkspaceDialog {
    workspace_id: u64,
}

fn read_explorer_height_pct() -> f64 {
    let stored = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(SIDEBAR_EXPLORER_HEIGHT_PCT_KEY).ok().flatten())
        .and_then(|raw| raw.parse::<f64>().ok());
    let pct = stored.unwrap_or(SIDEBAR_EXPLORER_HEIGHT_PCT_DEFAULT);
    pct.max(SIDEBAR_EXPLORER_HEIGHT_PCT_MIN)
        .min(SIDEBAR_EXPLORER_HEIGHT_PCT_MAX)
}

fn write_explorer_height_pct(pct: f64) {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(Some(storage)) = window.local_storage() {
        let _ = storage.set_item(SIDEBAR_EXPLORER_HEIGHT_PCT_KEY, &format!("{pct:.2}"));
    }
}
