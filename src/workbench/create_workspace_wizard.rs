use crate::i18n::I18nKey;

use crate::service::I18nService;
use crate::tauri_bridge::{
    agent_session_roles_list, create_directory, default_cwd, is_tauri_shell, list_directory,
    path_nav_invoke, ssh_remotes_list, workspace_presets_delete, workspace_presets_list,
    workspace_presets_save, DirEntryBrief, PathNavResult, RemoteConnectionView, SessionRoleView,
    WorkspacePresetView,
};
use crate::workbench::path_nav::path_nav_wasm_string;
use crate::workbench::state::{
    CreateWorkspaceDraft, HarnessSettingsCategory, HarnessUiService, WorkbenchService,
    WORKSPACE_FLEET_AGENT_SLUGS,
};
use crate::workbench::terminal_agent_profiles::{terminal_agent_efforts, terminal_agent_models};
use leptos::leptos_dom::helpers::window_event_listener_untyped;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

/// Terminal-count presets shown in the layout step. Only counts that fill a
/// clean rectangular grid (e.g. 4 = 2×2, 9 = 3×3) — no 10/14-style strips.
const PRESETS: &[u8] = &[1, 2, 4, 6, 8, 9, 12, 16];

fn looks_like_cd(s: &str) -> bool {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    lower == "cd" || lower.starts_with("cd ")
}

fn join_path(base: &str, name: &str) -> String {
    let b = base.trim();
    if b.is_empty() {
        return format!("/{name}");
    }
    if b.ends_with('/') {
        format!("{b}{name}")
    } else {
        format!("{b}/{name}")
    }
}

fn parent_of(path: &str) -> Option<String> {
    let p = std::path::Path::new(path.trim());
    p.parent().map(|x| x.to_string_lossy().into_owned())
}

/// Inline workspace configuration view. Single-column, centered layout that
/// matches the rest of the workbench chrome (no more modal-style sheet).
#[component]
pub fn WorkspaceConfigurator(workspace_id: u64) -> impl IntoView {
    let wb = expect_context::<WorkbenchService>();
    let ui = expect_context::<HarnessUiService>();
    let i18n = expect_context::<I18nService>();
    let drafts = wb.workspace_drafts();
    let draft_memo =
        Memo::new(move |_| drafts.with(|m| m.get(&workspace_id).cloned().unwrap_or_default()));
    let steps_sig = wb.workspace_config_steps();
    let step_memo =
        Memo::new(move |_| steps_sig.with(|m| m.get(&workspace_id).copied().unwrap_or(0)));

    let cwd_err = RwSignal::new(false);
    let browser_open = RwSignal::new(false);
    let dir_entries: RwSignal<Vec<DirEntryBrief>> = RwSignal::new(Vec::new());
    let dir_err: RwSignal<String> = RwSignal::new(String::new());
    let show_hidden = RwSignal::new(false);
    let popup_rect: RwSignal<(f64, f64, f64)> = RwSignal::new((0.0, 0.0, 0.0));
    let new_folder_open = RwSignal::new(false);
    let new_folder_name = RwSignal::new(String::new());
    let new_folder_err: RwSignal<String> = RwSignal::new(String::new());
    let refresh_token = RwSignal::new(0u64);
    // Saved SSH remote connections (for the Local/Remote selector).
    let remote_conns: RwSignal<Vec<RemoteConnectionView>> = RwSignal::new(Vec::new());
    // Built-in harness session roles (specialized skills) for the mode picker.
    let session_roles: RwSignal<Vec<SessionRoleView>> = RwSignal::new(Vec::new());
    let role_open = RwSignal::new(false);
    // Persisted workspace presets (app-data; one-click launch).
    let presets: RwSignal<Vec<WorkspacePresetView>> = RwSignal::new(Vec::new());
    // Inline "new preset" editor state.
    let preset_new_open = RwSignal::new(false);
    let preset_new_name = RwSignal::new(String::new());

    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(list) = ssh_remotes_list().await {
                remote_conns.set(list);
            }
        });
    });

    // Load session roles + presets once on mount.
    Effect::new(move |_| {
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(list) = agent_session_roles_list().await {
                session_roles.set(list);
            }
            if let Ok(list) = workspace_presets_list().await {
                presets.set(list);
            }
        });
    });

    // Save the current draft as a named preset, then refresh the list.
    let save_preset = move || {
        let name = preset_new_name.get_untracked().trim().to_string();
        if name.is_empty() {
            return;
        }
        let d = draft_memo.get_untracked();
        let preset = WorkspacePresetView {
            id: String::new(),
            name,
            terminal_count: d.terminal_count,
            agent_counts: d.agent_counts,
            agent_models: d.agent_models.clone(),
            agent_efforts: d.agent_efforts.clone(),
            slot_names: d.slot_names.clone(),
            session_role: d.session_role.clone(),
        };
        spawn_local(async move {
            if let Ok(list) = workspace_presets_save(preset).await {
                presets.set(list);
            }
        });
        preset_new_open.set(false);
        preset_new_name.set(String::new());
    };

    // True when the draft targets an SSH remote connection.
    let is_remote = move || draft_memo.get().remote_connection_id.is_some();

    let wrap_id = format!("wz-cwd-wrap-{workspace_id}");
    let wrap_id_for_measure = wrap_id.clone();
    let wrap_id_for_outside = wrap_id.clone();
    let role_wrap_id = format!("wz-role-wrap-{workspace_id}");
    let role_wrap_id_for_outside = role_wrap_id.clone();

    Effect::new(move |_| {
        let d = draft_memo.get();
        if !d.cwd_display.trim().is_empty() {
            return;
        }
        if !is_tauri_shell() {
            return;
        }
        spawn_local(async move {
            if let Ok(p) = default_cwd().await {
                if !p.trim().is_empty() {
                    // Bypass `set_workspace_cwd` so the auto-populated HOME
                    // path doesn't derive a workspace name (would otherwise
                    // turn into the username — we want "Workspace N").
                    wb.update_workspace_draft(workspace_id, |d| d.cwd_display = p);
                }
            }
        });
    });

    let measure_input_rect = {
        let id = wrap_id_for_measure;
        move || {
            let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                return;
            };
            let Some(wrap) = doc.get_element_by_id(&id) else {
                return;
            };
            let Some(input) = wrap.query_selector("input").ok().flatten() else {
                return;
            };
            let r = input.get_bounding_client_rect();
            popup_rect.set((r.bottom() + 4.0, r.left(), r.width()));
        }
    };

    let submit_cwd = move || {
        cwd_err.set(false);
        let value = draft_memo.get_untracked().cwd_display.clone();
        let trimmed = value.trim().to_string();
        if !looks_like_cd(&trimmed) {
            return;
        }
        let base = wb.harness_workspace_root().get_untracked();
        spawn_local(async move {
            let r: Result<PathNavResult, String> = if is_tauri_shell() {
                path_nav_invoke(base.clone(), trimmed.clone()).await
            } else {
                path_nav_wasm_string(&base, &trimmed)
                    .map(|(cwd, log_line)| PathNavResult { cwd, log_line })
            };
            if let Ok(res) = r {
                wb.set_workspace_cwd(workspace_id, res.cwd);
            }
        });
    };

    // Listing refresh whenever cwd / popup / refresh-token changes.
    Effect::new(move |_| {
        let _open = browser_open.get();
        let _ = refresh_token.get();
        let val = draft_memo.get().cwd_display.trim().to_string();
        if !is_tauri_shell() || !browser_open.get_untracked() {
            return;
        }
        if val.is_empty() || looks_like_cd(&val) {
            return;
        }
        spawn_local(async move {
            match list_directory(val).await {
                Ok(entries) => {
                    dir_entries.set(entries);
                    dir_err.set(String::new());
                }
                Err(e) => {
                    dir_entries.set(Vec::new());
                    dir_err.set(e);
                }
            }
        });
    });

    Effect::new({
        let measure = measure_input_rect.clone();
        let outside_id = wrap_id_for_outside;
        move |_| {
            if !browser_open.get() {
                return;
            }
            measure();

            let outside_id_inner = outside_id.clone();
            let h_down = window_event_listener_untyped("mousedown", move |ev| {
                let Some(target) = ev.target() else {
                    return;
                };
                let Ok(node) = target.dyn_into::<web_sys::Node>() else {
                    return;
                };
                let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                    return;
                };
                let Some(wrap) = doc.get_element_by_id(&outside_id_inner) else {
                    return;
                };
                if !wrap.contains(Some(&node)) {
                    browser_open.set(false);
                    new_folder_open.set(false);
                }
            });
            let m1 = measure.clone();
            let h_resize = window_event_listener_untyped("resize", move |_| m1());
            let m2 = measure.clone();
            let h_scroll = window_event_listener_untyped("scroll", move |_| m2());
            on_cleanup(move || {
                h_down.remove();
                h_resize.remove();
                h_scroll.remove();
            });
        }
    });

    Effect::new({
        let outside_id = role_wrap_id_for_outside;
        move |_| {
            if !role_open.get() {
                return;
            }

            let outside_id_inner = outside_id.clone();
            let h_down = window_event_listener_untyped("mousedown", move |ev| {
                let Some(target) = ev.target() else {
                    return;
                };
                let Ok(node) = target.dyn_into::<web_sys::Node>() else {
                    return;
                };
                let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
                    return;
                };
                let Some(wrap) = doc.get_element_by_id(&outside_id_inner) else {
                    return;
                };
                if !wrap.contains(Some(&node)) {
                    role_open.set(false);
                }
            });
            on_cleanup(move || h_down.remove());
        }
    });

    let create_folder = move || {
        let name = new_folder_name.get_untracked().trim().to_string();
        if name.is_empty() {
            new_folder_err.set("Name darf nicht leer sein".into());
            return;
        }
        let parent = draft_memo.get_untracked().cwd_display.trim().to_string();
        if parent.is_empty() {
            new_folder_err.set("Kein Verzeichnis ausgewählt".into());
            return;
        }
        if !is_tauri_shell() {
            new_folder_err.set("Nicht in Tauri-Shell".into());
            return;
        }
        spawn_local(async move {
            match create_directory(parent, name).await {
                Ok(_) => {
                    new_folder_open.set(false);
                    new_folder_name.set(String::new());
                    new_folder_err.set(String::new());
                    refresh_token.update(|n| *n += 1);
                }
                Err(e) => new_folder_err.set(e),
            }
        });
    };

    let step0 = move || step_memo.get() == 0;
    let step1 = move || step_memo.get() == 1;

    let cwd_val = move || draft_memo.get().cwd_display.clone();
    let name_val = move || draft_memo.get().name_input.clone();
    let enabled_session_roles = Memo::new(move |_| {
        session_roles
            .get()
            .into_iter()
            .filter(|role| role.enabled)
            .collect::<Vec<_>>()
    });

    // Distinct working directories from previously opened workspaces, so the
    // user can refill the cwd field with one click instead of re-typing or
    // re-browsing. Deduplicated by normalized path, newest first.
    let recent_dirs = Memo::new(move |_| {
        let mut seen = std::collections::HashSet::new();
        let mut out: Vec<(String, String, u8)> = Vec::new();
        for it in wb.recent_workspaces().get() {
            if !crate::workbench::state::workspace_entry_has_folder(&it.workspace) {
                continue;
            }
            let cwd = it.workspace.cwd.clone();
            let key = cwd.trim().trim_end_matches(['/', '\\']).to_string();
            if key.is_empty() || !seen.insert(key) {
                continue;
            }
            out.push((it.workspace.title.clone(), cwd, it.workspace.terminal_count));
        }
        out
    });

    view! {
        <section class="ws-config" aria-label=move || i18n.tr(I18nKey::WzWizardAria)()>
            <div class="ws-config__shell">
                <ol class="ws-config__steps" aria-label=move || i18n.tr(I18nKey::WzWizardStepsAria)()>
                    <StepChip idx=0 label=I18nKey::WzSubLayout step=step_memo />
                    <li class="ws-config__step-sep" aria-hidden="true"></li>
                    <StepChip idx=1 label=I18nKey::WzFleetTitle step=step_memo />
                </ol>

                <h2 class="ws-config__title">{move || i18n.tr(I18nKey::WzTitle)()}</h2>
                <p class="ws-config__sub">{move || {
                    if step_memo.get() == 0 {
                        i18n.tr(I18nKey::WzSubLayout)().to_string()
                    } else {
                        let n = draft_memo.get().terminal_count;
                        i18n.tr(I18nKey::WzSubFleet)().replace("{n}", &n.to_string())
                    }
                }}</p>

                <Show when=step0.clone()>
                    <div class="ws-config__group">
                        <label class="ws-config__label">
                            <LxIcon icon=icondata::LuCable width="0.8rem" height="0.8rem" />
                            <span>{move || i18n.tr(I18nKey::WsConnectionType)()}</span>
                        </label>
                        <select
                            class="ws-config__field"
                            prop:value=move || draft_memo.get().remote_connection_id.unwrap_or_default()
                            on:change=move |ev| {
                                let v = select_value(&ev);
                                // Sentinel: jump to Settings → Remote to create a
                                // connection. The draft is left unchanged, so the
                                // select snaps back to its bound value; returning to
                                // the Terminals tab remounts the wizard and reloads
                                // the connection list.
                                if v == "__add_remote__" {
                                    ui.settings_category().set(HarnessSettingsCategory::Remote);
                                    wb.open_center_settings_tab(HarnessSettingsCategory::Remote);
                                    return;
                                }
                                wb.set_workspace_remote_connection(
                                    workspace_id,
                                    if v.is_empty() { None } else { Some(v) },
                                );
                            }
                        >
                            <option value="">{move || i18n.tr(I18nKey::WsConnectionLocal)()}</option>
                            {move || {
                                remote_conns
                                    .get()
                                    .into_iter()
                                    .map(|c| {
                                        let id = c.connection.id.clone();
                                        let label = format!("{} · SSH", c.connection.label);
                                        view! { <option value=id>{label}</option> }
                                    })
                                    .collect_view()
                            }}
                            <option value="__add_remote__">
                                {move || format!("+ {}", i18n.tr(I18nKey::RemoteAddConnection)())}
                            </option>
                        </select>
                        <Show when=move || is_tauri_shell() && remote_conns.get().is_empty()>
                            <p class="ws-config__hint">{move || i18n.tr(I18nKey::WsRemoteNoPresets)()}</p>
                        </Show>
                    </div>

                    <div class="ws-config__group">
                        <label class="ws-config__label">
                            <LxIcon icon=icondata::LuTag width="0.8rem" height="0.8rem" />
                            <span>{move || i18n.tr(I18nKey::WzNameLabel)()}</span>
                        </label>
                        <input
                            class="ws-config__field"
                            type="text"
                            prop:value=name_val
                            placeholder=move || i18n.tr(I18nKey::WzNamePh)()
                            on:input=move |ev| {
                                let v = input_value(&ev);
                                wb.update_workspace_draft(workspace_id, |d| d.name_input = v);
                            }
                        />
                    </div>

                    <div class="ws-config__group">
                        <label class="ws-config__label">
                            <LxIcon icon=icondata::LuFolderOpen width="0.8rem" height="0.8rem" />
                            <span>{move || {
                                if is_remote() {
                                    i18n.tr(I18nKey::WsRemoteDir)()
                                } else {
                                    i18n.tr(I18nKey::WzCwdLabel)()
                                }
                            }}</span>
                        </label>
                        <div id=wrap_id.clone() class="ws-config__cwd">
                            <span class="ws-config__cwd-icon" aria-hidden="true">
                                <LxIcon icon=icondata::LuFolder width="1rem" height="1rem" />
                            </span>
                            <input
                                class="ws-config__field ws-config__field--cwd"
                                type="text"
                                prop:value=cwd_val
                                placeholder=move || i18n.tr(I18nKey::WzCwdExamplePh)()
                                // Local directory browser is meaningless over SSH.
                                on:focus=move |_| { if !is_remote() { browser_open.set(true); } }
                                on:input=move |ev| {
                                    cwd_err.set(false);
                                    let v = input_value(&ev);
                                    wb.set_workspace_cwd(workspace_id, v);
                                }
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    let key = ev.key();
                                    if key == "Enter" {
                                        ev.prevent_default();
                                        submit_cwd();
                                    } else if key == "Escape" {
                                        browser_open.set(false);
                                    }
                                }
                            />
                            <Show when=move || browser_open.get()>
                                <div
                                    class="wz-cwd-browser"
                                    role="listbox"
                                    style:top=move || format!("{}px", popup_rect.get().0)
                                    style:left=move || format!("{}px", popup_rect.get().1)
                                    style:width=move || format!("{}px", popup_rect.get().2)
                                >
                                    <div class="wz-cwd-browser__toolbar">
                                        <button
                                            type="button"
                                            class="wz-cwd-browser__tool"
                                            on:mousedown=move |ev| {
                                                ev.prevent_default();
                                                let cur = draft_memo.get_untracked().cwd_display;
                                                if let Some(parent) = parent_of(&cur) {
                                                    wb.set_workspace_cwd(workspace_id, parent);
                                                }
                                            }
                                            aria-label=move || i18n.tr(I18nKey::WzNavParentAria)()
                                            title=move || i18n.tr(I18nKey::WzNavParentTitle)()
                                        >
                                            <LxIcon icon=icondata::LuArrowUp width="0.85rem" height="0.85rem" />
                                        </button>
                                        <span class="wz-cwd-browser__path" title=move || draft_memo.get().cwd_display.clone()>
                                            {move || draft_memo.get().cwd_display.clone()}
                                        </span>
                                        <button
                                            type="button"
                                            class="wz-cwd-browser__tool"
                                            on:mousedown=move |ev| {
                                                ev.prevent_default();
                                                new_folder_open.update(|v| *v = !*v);
                                                new_folder_err.set(String::new());
                                            }
                                            aria-label=move || i18n.tr(I18nKey::WzNewFolderAria)()
                                            title=move || i18n.tr(I18nKey::WzNewFolderTitle)()
                                        >
                                            <LxIcon icon=icondata::LuFolderPlus width="0.85rem" height="0.85rem" />
                                        </button>
                                        <label class="wz-cwd-browser__hidden" title=move || i18n.tr(I18nKey::WzShowHiddenTitle)()>
                                            <input
                                                type="checkbox"
                                                prop:checked=move || show_hidden.get()
                                                on:change=move |ev| {
                                                    let el = ev.target()
                                                        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
                                                    show_hidden.set(el.map(|e| e.checked()).unwrap_or(false));
                                                }
                                            />
                                            <span>{move || i18n.tr(I18nKey::WzDotHiddenLabel)()}</span>
                                        </label>
                                    </div>
                                    <Show when=move || new_folder_open.get()>
                                        <div class="wz-cwd-browser__newrow">
                                            <input
                                                class="wz-cwd-browser__newinput"
                                                type="text"
                                                placeholder=move || i18n.tr(I18nKey::WzFolderNamePh)()
                                                prop:value=move || new_folder_name.get()
                                                on:input=move |ev| {
                                                    new_folder_err.set(String::new());
                                                    new_folder_name.set(input_value(&ev));
                                                }
                                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                                    if ev.key() == "Enter" {
                                                        ev.prevent_default();
                                                        create_folder();
                                                    } else if ev.key() == "Escape" {
                                                        new_folder_open.set(false);
                                                    }
                                                }
                                                on:mousedown=move |ev| ev.stop_propagation()
                                            />
                                            <button
                                                type="button"
                                                class="wz-cwd-browser__newbtn"
                                                on:mousedown=move |ev| { ev.prevent_default(); create_folder(); }
                                            >{move || i18n.tr(I18nKey::WzFolderCreate)()}</button>
                                            <button
                                                type="button"
                                                class="wz-cwd-browser__newbtn wz-cwd-browser__newbtn--ghost"
                                                on:mousedown=move |ev| { ev.prevent_default(); new_folder_open.set(false); }
                                            >"✕"</button>
                                        </div>
                                        <Show when=move || !new_folder_err.get().is_empty()>
                                            <p class="wz-cwd-browser__err">{move || new_folder_err.get()}</p>
                                        </Show>
                                    </Show>
                                    <ul class="wz-cwd-browser__list">
                                        {move || {
                                            let entries = dir_entries.get();
                                            let show_h = show_hidden.get();
                                            let visible: Vec<DirEntryBrief> = entries
                                                .into_iter()
                                                .filter(|e| show_h || !e.hidden)
                                                .collect();
                                            if visible.is_empty() {
                                                let msg = dir_err.get();
                                                let empty = i18n.tr(I18nKey::WzCwdListEmpty)().to_string();
                                                return view! {
                                                    <li class="wz-cwd-browser__empty">
                                                        {if msg.is_empty() { empty } else { msg }}
                                                    </li>
                                                }.into_any();
                                            }
                                            visible.into_iter().map(|e| {
                                                let name = e.name.clone();
                                                let label = e.name.clone();
                                                let hidden = e.hidden;
                                                view! {
                                                    <li>
                                                        <button
                                                            type="button"
                                                            class=move || {
                                                                let mut c = String::from("wz-cwd-browser__item");
                                                                if hidden { c.push_str(" wz-cwd-browser__item--hidden"); }
                                                                c
                                                            }
                                                            on:mousedown=move |ev| {
                                                                ev.prevent_default();
                                                                let cur = draft_memo.get_untracked().cwd_display;
                                                                wb.set_workspace_cwd(workspace_id, join_path(&cur, &name));
                                                            }
                                                        >
                                                            <span class="wz-cwd-browser__item-icon" aria-hidden="true">
                                                                <LxIcon icon=icondata::LuFolder width="0.85rem" height="0.85rem" />
                                                            </span>
                                                            {label}
                                                        </button>
                                                    </li>
                                                }
                                            }).collect_view().into_any()
                                        }}
                                    </ul>
                                </div>
                            </Show>
                        </div>
                        <Show when=move || !recent_dirs.get().is_empty()>
                            <div class="ws-config__recent">
                                <div class="ws-config__recent-head">
                                    <span class="ws-config__recent-label">
                                        <LxIcon icon=icondata::LuClock width="0.82rem" height="0.82rem" />
                                        <span>{move || i18n.tr(I18nKey::QkRecentHeading)()}</span>
                                        <span class="ws-config__recent-count">
                                            {move || recent_dirs.get().len()}
                                        </span>
                                    </span>
                                    <span class="ws-config__recent-note">"Last opened workspaces"</span>
                                </div>
                                <ul class="ws-config__recent-list" role="list">
                                    {move || {
                                        recent_dirs
                                            .get()
                                            .into_iter()
                                            .map(|(title, cwd, terminal_count)| {
                                                let cwd_set = cwd.clone();
                                                let base = cwd
                                                    .trim_end_matches(['/', '\\'])
                                                    .rsplit(['/', '\\'])
                                                    .next()
                                                    .unwrap_or(cwd.as_str())
                                                    .to_string();
                                                let label = if title.trim().is_empty() {
                                                    base
                                                } else {
                                                    title
                                                };
                                                view! {
                                                    <li class="ws-config__recent-item">
                                                        <button
                                                            type="button"
                                                            class="ws-config__recent-card"
                                                            title=cwd.clone()
                                                            on:click=move |_| {
                                                                wb.set_workspace_cwd(workspace_id, cwd_set.clone());
                                                            }
                                                        >
                                                            <span class="ws-config__recent-icon" aria-hidden="true">
                                                                <LxIcon icon=icondata::LuFolderClock width="0.9rem" height="0.9rem" />
                                                            </span>
                                                            <span class="ws-config__recent-text">
                                                                <span class="ws-config__recent-title">{label}</span>
                                                                <span class="ws-config__recent-path">{cwd.clone()}</span>
                                                            </span>
                                                            <span class="ws-config__recent-terminals">{terminal_count}</span>
                                                        </button>
                                                    </li>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </ul>
                            </div>
                        </Show>
                        <Show when=move || cwd_err.get()>
                            <p class="ws-config__error">{move || i18n.tr(I18nKey::WzCwdEmpty)()}</p>
                        </Show>
                    </div>

                    <div class="ws-config__group">
                        <label class="ws-config__label">
                            <LxIcon icon=icondata::LuLayoutGrid width="0.8rem" height="0.8rem" />
                            <span>{move || i18n.tr(I18nKey::WzTemplatesHeading)()}</span>
                        </label>
                        <div class="ws-config__layout-row">
                            {PRESETS
                                .iter()
                                .copied()
                                .map(|n| {
                                    let (rows, cols) = crate::workbench::state::WorkspaceEntry::grid_dims_for_count(n);
                                    let cols_val = format!("repeat({cols},1fr)");
                                    let rows_val = format!("repeat({rows},1fr)");
                                    view! {
                                        <button
                                            type="button"
                                            class=move || {
                                                let mut c = String::from("ws-config__layout-card");
                                                if draft_memo.get().terminal_count == n {
                                                    c.push_str(" ws-config__layout-card--active");
                                                }
                                                c
                                            }
                                            on:click=move |_| wb.set_workspace_terminal_layout(workspace_id, n)
                                            title=format!("{n} terminals · {rows}×{cols}")
                                        >
                                            <span
                                                class="ws-config__layout-mini"
                                                style:grid-template-columns=cols_val
                                                style:grid-template-rows=rows_val
                                                aria-hidden="true"
                                            >
                                                {(0..(rows as usize * cols as usize)).map(|_| view! {
                                                    <span class="ws-config__layout-cell"></span>
                                                }).collect_view()}
                                            </span>
                                            <span class="ws-config__layout-num">{n}</span>
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </div>
                        <p class="ws-config__hint">
                            {move || {
                                let d = draft_memo.get();
                                format!("{} terminal{} · {}×{} grid",
                                    d.terminal_count,
                                    if d.terminal_count == 1 { "" } else { "s" },
                                    d.grid_rows, d.grid_cols)
                            }}
                        </p>
                    </div>

                    <div class="ws-config__group">
                        <label class="ws-config__label">
                            <LxIcon icon=icondata::LuBot width="0.8rem" height="0.8rem" />
                            <span>{move || i18n.tr(I18nKey::WzSessionRoleLabel)()}</span>
                        </label>
                        <div id=role_wrap_id.clone() class="ws-config__role-picker">
                            <button
                                type="button"
                                class="ws-config__role-trigger"
                                class:ws-config__role-trigger--open=move || role_open.get()
                                aria-haspopup="listbox"
                                aria-expanded=move || role_open.get().to_string()
                                on:click=move |_| role_open.update(|open| *open = !*open)
                                on:keydown=move |ev: web_sys::KeyboardEvent| {
                                    match ev.key().as_str() {
                                        "Escape" => {
                                            role_open.set(false);
                                            ev.prevent_default();
                                        }
                                        "ArrowDown" | "Enter" | " " => {
                                            role_open.set(true);
                                            ev.prevent_default();
                                        }
                                        _ => {}
                                    }
                                }
                            >
                                <span class="ws-config__role-trigger-copy">
                                    <span class="ws-config__role-title">
                                        {move || {
                                            let cur = draft_memo.get().session_role;
                                            let Some(slug) = cur else {
                                                return i18n.tr(I18nKey::WzSessionRoleNone)().to_string();
                                            };
                                            enabled_session_roles
                                                .get()
                                                .into_iter()
                                                .find(|r| r.slug == slug)
                                                .map(|r| r.title)
                                                .unwrap_or_else(|| i18n.tr(I18nKey::WzSessionRoleNone)().to_string())
                                        }}
                                    </span>
                                    <span class="ws-config__role-desc">
                                        {move || {
                                            let cur = draft_memo.get().session_role;
                                            let Some(slug) = cur else {
                                                return "Default BLXCode Agent without a specialized harness session role.".to_string();
                                            };
                                            enabled_session_roles
                                                .get()
                                                .into_iter()
                                                .find(|r| r.slug == slug)
                                                .map(|r| r.description)
                                                .unwrap_or_else(|| "Default BLXCode Agent without a specialized harness session role.".to_string())
                                        }}
                                    </span>
                                </span>
                                <LxIcon icon=icondata::LuChevronDown width="0.85rem" height="0.85rem" />
                            </button>
                            <Show when=move || role_open.get()>
                                <div class="ws-config__role-menu" role="listbox">
                                    <button
                                        type="button"
                                        class=move || {
                                            let mut c = String::from("ws-config__role-option");
                                            if draft_memo.get().session_role.is_none() {
                                                c.push_str(" ws-config__role-option--active");
                                            }
                                            c
                                        }
                                        role="option"
                                        aria-selected=move || draft_memo.get().session_role.is_none().to_string()
                                        on:click=move |_| {
                                            wb.set_workspace_session_role(workspace_id, None);
                                            role_open.set(false);
                                        }
                                    >
                                        <span class="ws-config__role-option-main">
                                            <span class="ws-config__role-title">{move || i18n.tr(I18nKey::WzSessionRoleNone)()}</span>
                                            <span class="ws-config__role-desc">"Default BLXCode Agent without a specialized harness session role."</span>
                                        </span>
                                        <Show when=move || draft_memo.get().session_role.is_none()>
                                            <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
                                        </Show>
                                    </button>
                                    {move || {
                                        enabled_session_roles
                                            .get()
                                            .into_iter()
                                            .map(|r| {
                                                let slug = r.slug.clone();
                                                let slug_for_class = slug.clone();
                                                let slug_for_aria = slug.clone();
                                                let slug_for_click = slug.clone();
                                                let title = r.title.clone();
                                                let desc = r.description.clone();
                                                let mut meta: Vec<String> = Vec::new();
                                                if !r.tools.is_empty() {
                                                    meta.push(format!(
                                                        "{}: {}",
                                                        i18n.tr(I18nKey::WzSessionRoleTools)(),
                                                        r.tools.join(", ")
                                                    ));
                                                }
                                                if !r.models.is_empty() {
                                                    meta.push(format!(
                                                        "{}: {}",
                                                        i18n.tr(I18nKey::WzAgentModelLabel)(),
                                                        r.models.join(", ")
                                                    ));
                                                }
                                                let meta = meta.join("  ·  ");
                                                let meta_for_when = meta.clone();
                                                let meta_for_view = meta.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class=move || {
                                                            let mut c = String::from("ws-config__role-option");
                                                            if draft_memo.get().session_role.as_deref() == Some(slug_for_class.as_str()) {
                                                                c.push_str(" ws-config__role-option--active");
                                                            }
                                                            c
                                                        }
                                                        role="option"
                                                        aria-selected=move || (draft_memo.get().session_role.as_deref() == Some(slug_for_aria.as_str())).to_string()
                                                        on:click=move |_| {
                                                            wb.set_workspace_session_role(workspace_id, Some(slug_for_click.clone()));
                                                            role_open.set(false);
                                                        }
                                                    >
                                                        <span class="ws-config__role-option-main">
                                                            <span class="ws-config__role-title">{title.clone()}</span>
                                                            <span class="ws-config__role-desc">{desc.clone()}</span>
                                                            <Show when=move || !meta_for_when.is_empty()>
                                                                <span class="ws-config__role-meta">{meta_for_view.clone()}</span>
                                                            </Show>
                                                        </span>
                                                        <Show when=move || draft_memo.get().session_role.as_deref() == Some(slug.as_str())>
                                                            <LxIcon icon=icondata::LuCheck width="0.82rem" height="0.82rem" />
                                                        </Show>
                                                    </button>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </div>
                            </Show>
                        </div>
                    </div>

                    <div class="ws-config__group">
                        <div class="ws-config__presets-head">
                            <label class="ws-config__label">
                                <LxIcon icon=icondata::LuRocket width="0.8rem" height="0.8rem" />
                                <span>{move || i18n.tr(I18nKey::WzPresetsHeading)()}</span>
                            </label>
                            <span class="ws-config__presets-sub">{move || i18n.tr(I18nKey::WzPresetsSubline)()}</span>
                        </div>
                        <div class="ws-config__presets-row">
                            {move || {
                                let items = presets.get();
                                if items.is_empty() {
                                    return view! {
                                        <div class="ws-config__preset-empty">
                                            <span class="ws-config__preset-empty-icon" aria-hidden="true">
                                                <LxIcon icon=icondata::LuFolderPlus width="1rem" height="1rem" />
                                            </span>
                                            <span class="ws-config__preset-empty-title">
                                                {i18n.tr(I18nKey::WzPresetEmpty)()}
                                            </span>
                                            <button
                                                type="button"
                                                class="ws-config__preset-empty-btn"
                                                on:click=move |_| preset_new_open.set(true)
                                            >
                                                {move || i18n.tr(I18nKey::WzPresetNew)()}
                                            </button>
                                        </div>
                                    }.into_any();
                                }
                                items.into_iter().map(|p| {
                                    let pid = p.id.clone();
                                    let count = p.terminal_count;
                                    let counts = p.agent_counts;
                                    let amodels = p.agent_models.clone();
                                    let aefforts = p.agent_efforts.clone();
                                    let names = p.slot_names.clone();
                                    let role = p.session_role.clone();
                                    view! {
                                        <span class="ws-config__preset">
                                            <button
                                                type="button"
                                                class="ws-config__preset-launch"
                                                title=format!("{count} terminals")
                                                on:click=move |_| {
                                                    wb.apply_preset_to_draft(
                                                        workspace_id,
                                                        count,
                                                        counts,
                                                        amodels.clone(),
                                                        aefforts.clone(),
                                                        names.clone(),
                                                        role.clone(),
                                                    );
                                                    wb.commit_inline_configure(workspace_id);
                                                }
                                            >
                                                <span class="ws-config__preset-name">{p.name.clone()}</span>
                                                <span class="ws-config__preset-meta">{format!("{count}×")}</span>
                                            </button>
                                            <button
                                                type="button"
                                                class="ws-config__preset-del"
                                                aria-label=move || i18n.tr(I18nKey::WzPresetDeleteAria)()
                                                title=move || i18n.tr(I18nKey::WzPresetDeleteAria)()
                                                on:click=move |_| {
                                                    let pid = pid.clone();
                                                    spawn_local(async move {
                                                        if let Ok(list) = workspace_presets_delete(pid).await {
                                                            presets.set(list);
                                                        }
                                                    });
                                                }
                                            >"✕"</button>
                                        </span>
                                    }
                                }).collect_view().into_any()
                            }}
                            <Show when=move || !preset_new_open.get() && !presets.get().is_empty()>
                                <button
                                    type="button"
                                    class="ws-config__preset ws-config__preset--new"
                                    on:click=move |_| preset_new_open.set(true)
                                >
                                    {move || i18n.tr(I18nKey::WzPresetNew)()}
                                </button>
                            </Show>
                        </div>
                        <Show when=move || preset_new_open.get()>
                            <div class="ws-config__preset-newrow">
                                <input
                                    class="ws-config__field ws-config__preset-newinput"
                                    type="text"
                                    placeholder=move || i18n.tr(I18nKey::WzPresetSavePrompt)()
                                    prop:value=move || preset_new_name.get()
                                    on:input=move |ev| preset_new_name.set(input_value(&ev))
                                    on:keydown={
                                        let save_preset = save_preset.clone();
                                        move |ev: web_sys::KeyboardEvent| {
                                            if ev.key() == "Enter" {
                                                ev.prevent_default();
                                                save_preset();
                                            } else if ev.key() == "Escape" {
                                                preset_new_open.set(false);
                                            }
                                        }
                                    }
                                />
                                <button
                                    type="button"
                                    class="ws-config__btn ws-config__btn--primary"
                                    on:click={
                                        let save_preset = save_preset.clone();
                                        move |_| save_preset()
                                    }
                                >
                                    {move || i18n.tr(I18nKey::WzFolderCreate)()}
                                </button>
                                <button
                                    type="button"
                                    class="ws-config__btn ws-config__btn--ghost"
                                    on:click=move |_| preset_new_open.set(false)
                                >"✕"</button>
                            </div>
                        </Show>
                    </div>
                </Show>

                <Show when=step1.clone()>
                    <div class="ws-config__fleet">
                        <div class="ws-config__fleet-tools">
                            <button type="button" class="ws-config__chip" on:click=move |_| wb.workspace_fleet_select_all(workspace_id)>
                                {move || i18n.tr(I18nKey::WzFleetSelectAll)()}
                            </button>
                            <button type="button" class="ws-config__chip" on:click=move |_| wb.workspace_fleet_one_each(workspace_id)>
                                {move || i18n.tr(I18nKey::WzFleetOneEach)()}
                            </button>
                            <button type="button" class="ws-config__chip" on:click=move |_| wb.workspace_fleet_fill_evenly(workspace_id)>
                                {move || i18n.tr(I18nKey::WzFillEvenly)()}
                            </button>
                            <button type="button" class="ws-config__chip ws-config__chip--danger" on:click=move |_| wb.workspace_fleet_clear(workspace_id)>
                                {move || i18n.tr(I18nKey::WzFleetClear)()}
                            </button>
                            <span class="ws-config__fleet-spacer"></span>
                            <span class="ws-config__fleet-meter">{move || {
                                let d = draft_memo.get();
                                let a: u8 = wb.workspace_fleet_assigned(workspace_id);
                                format!("{a} / {}", d.terminal_count)
                            }}</span>
                        </div>
                        <ul class="ws-config__agent-list">
                            {agent_rows(wb, i18n, workspace_id, draft_memo)}
                        </ul>
                        <div class="ws-config__slot-names">
                            <label class="ws-config__label">{move || i18n.tr(I18nKey::WzSlotNamesLabel)()}</label>
                            <div class="ws-config__slot-names-grid">
                                {move || {
                                    let d = draft_memo.get();
                                    let n = d.terminal_count as usize;
                                    (0..n).map(|i| {
                                        let ph = i18n.tr(I18nKey::WzSlotNamePh)().replace("{n}", &(i + 1).to_string());
                                        let val = d.slot_names.get(i).cloned().unwrap_or_default();
                                        view! {
                                            <input
                                                class="ws-config__field ws-config__slot-name-input"
                                                type="text"
                                                placeholder=ph
                                                prop:value=val
                                                on:input=move |ev| {
                                                    wb.set_workspace_slot_name(workspace_id, i, input_value(&ev));
                                                }
                                            />
                                        }
                                    }).collect_view()
                                }}
                            </div>
                        </div>
                        <p class="ws-config__hint">{move || {
                            let d = draft_memo.get();
                            let n = d.terminal_count;
                            let a: u8 = wb.workspace_fleet_assigned(workspace_id);
                            if d.agents_skipped {
                                i18n.tr(I18nKey::WzFleetNoAgents)().to_string()
                            } else if a == n {
                                i18n.tr(I18nKey::WzFleetOptimal)().to_string()
                            } else {
                                i18n.tr(I18nKey::WzFleetSumWrong)().replace("{n}", &n.to_string())
                            }
                        }}</p>
                    </div>
                </Show>

                <div class="ws-config__actions">
                    <button type="button" class="ws-config__btn ws-config__btn--ghost" on:click=move |_| wb.cancel_inline_configure(workspace_id)>
                        {move || i18n.tr(I18nKey::WzCancel)()}
                    </button>
                    <span class="ws-config__actions-spacer"></span>
                    <Show when=step1.clone()>
                        <button type="button" class="ws-config__btn ws-config__btn--ghost" on:click=move |_| wb.workspace_back_to_layout(workspace_id)>
                            {move || i18n.tr(I18nKey::WzBack)()}
                        </button>
                        <button
                            type="button"
                            class="ws-config__btn ws-config__btn--ghost"
                            on:click=move |_| {
                                wb.workspace_skip_agents(workspace_id);
                                wb.commit_inline_configure(workspace_id);
                            }
                        >
                            {move || i18n.tr(I18nKey::WzSkipAgents)()}
                        </button>
                        <button
                            type="button"
                            class="ws-config__btn ws-config__btn--primary"
                            on:click=move |_| wb.commit_inline_configure(workspace_id)
                            prop:disabled=move || {
                                let d = draft_memo.get();
                                // Local needs a cwd; remote may omit it.
                                if d.remote_connection_id.is_none() && d.cwd_display.trim().is_empty() {
                                    return true;
                                }
                                if d.agents_skipped {
                                    return false;
                                }
                                wb.workspace_fleet_assigned(workspace_id) != d.terminal_count
                            }
                        >
                            {move || i18n.tr(I18nKey::WzLaunch)()}
                        </button>
                    </Show>
                    <Show when=step0.clone()>
                        <button
                            type="button"
                            class="ws-config__btn ws-config__btn--primary"
                            on:click=move |_| {
                                if wb.workspace_go_to_fleet_step(workspace_id).is_err() {
                                    cwd_err.set(true);
                                }
                            }
                        >
                            {move || i18n.tr(I18nKey::WzNext)()}
                        </button>
                    </Show>
                </div>
            </div>
        </section>
    }
}

#[component]
fn StepChip(idx: u8, label: I18nKey, step: Memo<u8>) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    view! {
        <li class=move || {
            let cur = step.get();
            let mut c = String::from("ws-config__step");
            if idx == cur {
                c.push_str(" ws-config__step--active");
            } else if idx < cur {
                c.push_str(" ws-config__step--done");
            }
            c
        }>
            <span class="ws-config__step-num">{(idx + 1).to_string()}</span>
            <span class="ws-config__step-lbl">{move || i18n.tr(label)()}</span>
        </li>
    }
}

fn agent_rows(
    wb: WorkbenchService,
    i18n: I18nService,
    workspace_id: u64,
    draft: Memo<CreateWorkspaceDraft>,
) -> impl IntoView {
    (0..5)
        .map(|idx| {
            let (name_k, sub_k) = match idx {
                0 => (I18nKey::WzAgentClaude, I18nKey::WzAgentSubClaude),
                1 => (I18nKey::WzAgentCodex, I18nKey::WzAgentSubCodex),
                2 => (I18nKey::WzAgentGemini, I18nKey::WzAgentSubGemini),
                3 => (I18nKey::WzAgentOpencode, I18nKey::WzAgentSubOpencode),
                _ => (I18nKey::WzAgentCursor, I18nKey::WzAgentSubCursor),
            };
            let has_agents = move || draft.get().agent_counts[idx] != 0;
            view! {
                <li class="ws-config__agent-row">
                    <div class="ws-config__agent-meta">
                        <strong>{move || i18n.tr(name_k)()}</strong>
                        <span class="ws-config__agent-sub">{move || i18n.tr(sub_k)()}</span>
                    </div>
                    <div class="ws-config__agent-ctl">
                        <button
                            type="button"
                            class="ws-config__chip"
                            on:click=move |_| wb.workspace_agent_fill_all(workspace_id, idx)
                        >
                            {move || {
                                let n = draft.get().terminal_count;
                                i18n.tr(I18nKey::WzFleetAll)().replace("{n}", &n.to_string())
                            }}
                        </button>
                        <button
                            type="button"
                            class="ws-config__stepper"
                            on:click=move |_| {
                                let c = draft.get().agent_counts[idx];
                                wb.set_workspace_agent_count(workspace_id, idx, c.saturating_sub(1));
                            }
                        >"−"</button>
                        <span class="ws-config__stepper-val">{move || draft.get().agent_counts[idx].to_string()}</span>
                        <button
                            type="button"
                            class="ws-config__stepper"
                            on:click=move |_| {
                                let c = draft.get().agent_counts[idx];
                                wb.set_workspace_agent_count(workspace_id, idx, c.saturating_add(1));
                            }
                        >"+"</button>
                        <Show when=has_agents>
                            <select
                                class="ws-config__field ws-config__agent-model"
                                title=move || i18n.tr(I18nKey::WzAgentModelLabel)()
                                aria-label=move || i18n.tr(I18nKey::WzAgentModelLabel)()
                                prop:value=move || draft.get().agent_models[idx].clone()
                                on:change=move |ev| {
                                    wb.set_workspace_agent_model(workspace_id, idx, select_value(&ev));
                                }
                            >
                                <option value="">{move || i18n.tr(I18nKey::WzAgentModelDefault)()}</option>
                                {
                                    let slug = WORKSPACE_FLEET_AGENT_SLUGS[idx];
                                    terminal_agent_models(slug)
                                        .iter()
                                        .map(|m| {
                                            let m = m.to_string();
                                            view! { <option value=m.clone()>{m.clone()}</option> }
                                        })
                                    .collect_view()
                                }
                            </select>
                            <Show when=move || !terminal_agent_efforts(WORKSPACE_FLEET_AGENT_SLUGS[idx]).is_empty()>
                                <select
                                    class="ws-config__field ws-config__agent-model"
                                    title=move || i18n.tr(I18nKey::WzAgentEffortLabel)()
                                    aria-label=move || i18n.tr(I18nKey::WzAgentEffortLabel)()
                                    prop:value=move || draft.get().agent_efforts[idx].clone()
                                    on:change=move |ev| {
                                        wb.set_workspace_agent_effort(workspace_id, idx, select_value(&ev));
                                    }
                                >
                                    <option value="">{move || i18n.tr(I18nKey::WzAgentEffortDefault)()}</option>
                                    {
                                        let slug = WORKSPACE_FLEET_AGENT_SLUGS[idx];
                                        terminal_agent_efforts(slug)
                                            .iter()
                                            .map(|effort| {
                                                let effort = effort.to_string();
                                                view! { <option value=effort.clone()>{effort.clone()}</option> }
                                            })
                                            .collect_view()
                                    }
                                </select>
                            </Show>
                        </Show>
                    </div>
                </li>
            }
        })
        .collect_view()
}

fn input_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|i| i.value())
        .unwrap_or_default()
}

fn select_value(ev: &web_sys::Event) -> String {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|s| s.value())
        .unwrap_or_default()
}
