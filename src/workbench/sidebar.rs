use crate::auth::{AuthEnv, AuthUserBrief};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::workbench::WorkbenchService;
use leptos::callback::Callable;
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;

fn initials_for(profile: Option<&AuthUserBrief>) -> String {
    let Some(b) = profile else {
        return "?".into();
    };
    let base = b.name.trim();
    let mut words = base.split_whitespace().filter_map(|w| w.chars().next());
    let Some(a) = words.next() else {
        return "?".into();
    };
    let mut s: String = a.to_uppercase().collect();
    if let Some(b) = words.next() {
        s.extend(b.to_uppercase());
    }
    s.truncate(3);
    s
}

fn avatar_img_url(profile: Option<&AuthUserBrief>) -> Option<String> {
    profile?
        .avatar_url
        .as_ref()
        .filter(|u| !u.trim().is_empty())
        .cloned()
}

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
    let auth = expect_context::<AuthEnv>();

    let collapsed = wb.sidebar_collapsed();
    let workspaces = wb.workspaces();
    let context_menu = RwSignal::new(None::<WorkspaceContextMenu>);
    let rename_dialog = RwSignal::new(None::<RenameWorkspaceDialog>);
    let rename_input = RwSignal::new(String::new());

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
                        each=move || workspaces.get()
                        key=|ws| ws.id
                        children=move |entry| {
                            let id = entry.id;
                            let title = entry.title;
                            let icon_label = workspace_icon_label(&title, id);
                            let title_ctx = title.clone();
                            view! {
                                <li class="workbench-sidebar__item">
                                    <button
                                        type="button"
                                        title=title.clone()
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
                                                title: title_ctx.clone(),
                                                x: ev.client_x(),
                                                y: ev.client_y(),
                                            }));
                                        }
                                    >
                                        <span class="workbench-sidebar__icon" aria-hidden="true">
                                            {icon_label.clone()}
                                        </span>
                                        <span class="workbench-sidebar__label">
                                            <span class="workbench-sidebar__bullet">"▸ "</span>
                                            {title.clone()}
                                        </span>
                                    </button>
                                    <button
                                        type="button"
                                        class="workbench-sidebar__close"
                                        title=format!("Close {title}")
                                        aria-label=format!("Close {title}")
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
            </nav>

            <div class="workbench-sidebar__footer">
                <details class="sidebar-user-menu">
                    <summary
                        class="sidebar-user-menu__trigger"
                        aria-label=move || i18n.tr(I18nKey::SbUserMenuAria)()
                    >
                        <span class="sidebar-user-menu__avatar-slot" aria-hidden="true">
                            {move || {
                                let p = auth.profile.get();
                                match avatar_img_url(p.as_ref()) {
                                    Some(url) => view! {
                                        <img
                                            class="sidebar-user-menu__avatar-img"
                                            src=url
                                            alt=""
                                            referrerpolicy="no-referrer"
                                            loading="lazy"
                                        />
                                    }
                                    .into_any(),
                                    None => {
                                        let ini = initials_for(p.as_ref());
                                        view! {
                                            <span class="sidebar-user-menu__avatar-initials">{ini}</span>
                                        }
                                        .into_any()
                                    }
                                }
                            }}
                        </span>
                        <span class="sidebar-user-menu__identity">
                            <span class="sidebar-user-menu__name">
                                {move || {
                                    auth.profile.with(|p| {
                                        p.as_ref()
                                            .map(|x| x.name.clone())
                                            .unwrap_or_else(|| i18n.tr(I18nKey::SbAccount)().to_string())
                                    })
                                }}
                            </span>
                            <span class="sidebar-user-menu__caret" aria-hidden="true">
                                "▾"
                            </span>
                        </span>
                    </summary>
                    <div class="sidebar-user-menu__dropdown" role="group">
                        <p class="sidebar-user-menu__email">
                            {move || {
                                auth.profile.with(|p| {
                                    p.as_ref().and_then(|x| x.email.clone()).unwrap_or_default()
                                })
                            }}
                        </p>
                        <button
                            type="button"
                            class="sidebar-user-menu__signout workbench-sidebar__logout eula-btn eula-btn--ghost"
                            on:click=move |ev| {
                                ev.prevent_default();
                                auth.logout.run(());
                            }
                        >
                            {move || i18n.tr(I18nKey::SbSignOut)()}
                        </button>
                    </div>
                </details>
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
                                "Rename Workspace"
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
                                "Close Workspace"
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
                                    <h2 id="workspace-rename-title">"Rename Workspace"</h2>
                                    <button
                                        type="button"
                                        class="workspace-rename-dialog__close"
                                        aria-label="Close rename dialog"
                                        on:click=move |_| rename_dialog.set(None)
                                    >
                                        "×"
                                    </button>
                                </header>
                                <div class="workspace-rename-dialog__body">
                                    <label class="workspace-rename-dialog__label" for="workspace-rename-input">
                                        "Workspace name"
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
                                        "Cancel"
                                    </button>
                                    <button
                                        type="button"
                                        class="workspace-rename-dialog__btn workspace-rename-dialog__btn--primary"
                                        on:click=move |_| save()
                                        disabled=move || rename_input.get().trim().is_empty()
                                    >
                                        "Rename"
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
