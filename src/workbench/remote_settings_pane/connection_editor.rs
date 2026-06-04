//! The create/edit form for a single SSH connection preset. Opened from the
//! list view (new) or by clicking a card (edit). Reports back to the pane via
//! `on_close` so the list can refresh; secrets are write-only here.

use super::{
    auth_from_str, auth_to_str, input_value, resume_from_str, resume_to_str, select_value,
    EditorOutcome,
};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, ssh_remote_delete, ssh_remote_list_dirs, ssh_remote_save, ssh_remote_test,
    RemoteAuthKind, RemoteConnection, RemoteConnectionView, RemoteDirEntry, RemoteResume,
};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn RemoteConnectionEditor(
    initial: RemoteConnectionView,
    on_close: Callback<EditorOutcome>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    let is_new = initial.connection.id.trim().is_empty();
    let id = RwSignal::new(initial.connection.id.clone());
    let label = RwSignal::new(initial.connection.label.clone());
    let host = RwSignal::new(initial.connection.host.clone());
    let port = RwSignal::new(initial.connection.port.to_string());
    let username = RwSignal::new(initial.connection.username.clone());
    let auth_kind = RwSignal::new(initial.connection.auth_kind);
    let key_path = RwSignal::new(initial.connection.key_path.clone().unwrap_or_default());
    let resume = RwSignal::new(initial.connection.resume);
    let remote_dir = RwSignal::new(
        initial
            .connection
            .default_remote_dir
            .clone()
            .unwrap_or_default(),
    );
    let password_input = RwSignal::new(String::new());
    let passphrase_input = RwSignal::new(String::new());
    let has_password = RwSignal::new(initial.has_password);
    let has_passphrase = RwSignal::new(initial.has_passphrase);

    let busy = RwSignal::new(false);
    let testing = RwSignal::new(false);
    let status_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let browser_open = RwSignal::new(false);
    let browser_loading = RwSignal::new(false);
    let browser_path = RwSignal::new(String::new());
    let browser_parent = RwSignal::new(Option::<String>::None);
    let browser_entries = RwSignal::new(Vec::<RemoteDirEntry>::new());
    let browser_error = RwSignal::new(Option::<String>::None);

    // Assemble the current preset (no secrets) from the form fields.
    let build_connection = move || RemoteConnection {
        id: id.get_untracked(),
        label: label.get_untracked().trim().to_string(),
        host: host.get_untracked().trim().to_string(),
        port: port.get_untracked().trim().parse::<u16>().unwrap_or(22),
        username: username.get_untracked().trim().to_string(),
        auth_kind: auth_kind.get_untracked(),
        key_path: {
            let k = key_path.get_untracked().trim().to_string();
            if k.is_empty() {
                None
            } else {
                Some(k)
            }
        },
        resume: resume.get_untracked(),
        default_remote_dir: {
            let d = remote_dir.get_untracked().trim().to_string();
            if d.is_empty() {
                None
            } else {
                Some(d)
            }
        },
    };
    // Secrets are sent only when the user typed a new value.
    let current_password = move || {
        let v = password_input.get_untracked();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    };
    let current_passphrase = move || {
        let v = passphrase_input.get_untracked();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    };

    let on_save = move |_| {
        let conn = build_connection();
        let pw = current_password();
        let pp = current_passphrase();
        busy.set(true);
        status_msg.set(None);
        error_msg.set(None);
        leptos::task::spawn_local(async move {
            match ssh_remote_save(conn, pw, pp).await {
                Ok(_view) => {
                    busy.set(false);
                    on_close.run(EditorOutcome::Saved);
                }
                Err(err) => {
                    error_msg.set(Some(format!(
                        "{}: {err}",
                        i18n.tr(I18nKey::RemoteSaveError)()
                    )));
                    busy.set(false);
                }
            }
        });
    };

    let on_test = move |_| {
        let conn = build_connection();
        let pw = current_password();
        let pp = current_passphrase();
        testing.set(true);
        status_msg.set(None);
        error_msg.set(None);
        leptos::task::spawn_local(async move {
            match ssh_remote_test(conn, pw, pp).await {
                Ok(()) => status_msg.set(Some(i18n.tr(I18nKey::RemoteTestOk)().to_string())),
                Err(err) => error_msg.set(Some(format!(
                    "{}: {err}",
                    i18n.tr(I18nKey::RemoteTestFailed)()
                ))),
            }
            testing.set(false);
        });
    };

    let on_delete = move |_| {
        let saved_id = id.get_untracked();
        if saved_id.is_empty() {
            // Unsaved draft — nothing persisted, so just go back.
            on_close.run(EditorOutcome::Cancelled);
            return;
        }
        if let Some(win) = web_sys::window() {
            let confirmed = win
                .confirm_with_message(&i18n.tr(I18nKey::RemoteDeleteConfirm)())
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            match ssh_remote_delete(saved_id).await {
                Ok(()) => on_close.run(EditorOutcome::Deleted),
                Err(err) => {
                    error_msg.set(Some(err));
                    busy.set(false);
                }
            }
        });
    };

    let secret_placeholder = move || {
        if has_password.get() {
            i18n.tr(I18nKey::RemoteSecretKeepHint)()
        } else {
            i18n.tr(I18nKey::RemoteSecretNotSet)()
        }
    };
    let passphrase_placeholder = move || {
        if has_passphrase.get() {
            i18n.tr(I18nKey::RemoteSecretKeepHint)()
        } else {
            i18n.tr(I18nKey::RemoteSecretNotSet)()
        }
    };

    let load_remote_dir = move |path: String| {
        let connection_id = id.get_untracked();
        if connection_id.trim().is_empty() {
            browser_open.set(true);
            browser_error.set(Some(i18n.tr(I18nKey::RemoteDirSaveFirst)().to_string()));
            browser_entries.set(Vec::new());
            browser_parent.set(None);
            return;
        }
        browser_loading.set(true);
        browser_error.set(None);
        leptos::task::spawn_local(async move {
            match ssh_remote_list_dirs(connection_id, path).await {
                Ok(listing) => {
                    browser_path.set(listing.path);
                    browser_parent.set(listing.parent);
                    browser_entries.set(listing.entries);
                }
                Err(err) => browser_error.set(Some(err)),
            }
            browser_loading.set(false);
        });
    };

    let open_browser = move |_| {
        browser_open.set(true);
        let start = remote_dir.get_untracked();
        load_remote_dir(start);
    };

    view! {
        <section class="remote-conn-editor settings-field-card">
            <header class="remote-conn-editor__head">
                <button
                    type="button"
                    class="workbench-mini-btn"
                    on:click=move |_| on_close.run(EditorOutcome::Cancelled)
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuArrowLeft width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::RemoteBackToList)()}</span>
                    </span>
                </button>
                <h4 class="remote-conn-editor__title">
                    {move || if is_new {
                        i18n.tr(I18nKey::RemoteNewConnection)()
                    } else {
                        i18n.tr(I18nKey::RemoteEditConnection)()
                    }}
                </h4>
            </header>

            <div class="remote-conn-grid">
                <label class="remote-field">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteName)()}</span>
                    <input
                        class="workbench-plain-input"
                        type="text"
                        prop:value=move || label.get()
                        on:input=move |ev| label.set(input_value(&ev))
                    />
                </label>
                <label class="remote-field">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteHost)()}</span>
                    <input
                        class="workbench-plain-input"
                        type="text"
                        autocapitalize="off"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || host.get()
                        on:input=move |ev| host.set(input_value(&ev))
                    />
                </label>
                <label class="remote-field remote-field--port">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemotePort)()}</span>
                    <input
                        class="workbench-plain-input"
                        type="number"
                        min="1"
                        max="65535"
                        prop:value=move || port.get()
                        on:input=move |ev| port.set(input_value(&ev))
                    />
                </label>
                <label class="remote-field">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteUser)()}</span>
                    <input
                        class="workbench-plain-input"
                        type="text"
                        autocapitalize="off"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || username.get()
                        on:input=move |ev| username.set(input_value(&ev))
                    />
                </label>
                <label class="remote-field">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteAuthMethod)()}</span>
                    <select
                        class="workbench-plain-input"
                        prop:value=move || auth_to_str(auth_kind.get())
                        on:change=move |ev| auth_kind.set(auth_from_str(&select_value(&ev)))
                    >
                        <option value="password">{move || i18n.tr(I18nKey::RemoteAuthPassword)()}</option>
                        <option value="key">{move || i18n.tr(I18nKey::RemoteAuthKey)()}</option>
                        <option value="agent">{move || i18n.tr(I18nKey::RemoteAuthAgent)()}</option>
                    </select>
                </label>
                <label class="remote-field">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteResumeModel)()}</span>
                    <select
                        class="workbench-plain-input"
                        prop:value=move || resume_to_str(resume.get())
                        on:change=move |ev| resume.set(resume_from_str(&select_value(&ev)))
                    >
                        <option value="keepalive">{move || i18n.tr(I18nKey::RemoteResumeKeepalive)()}</option>
                        <option value="tmux">{move || i18n.tr(I18nKey::RemoteResumeTmux)()}</option>
                    </select>
                </label>
            </div>

            <p class="harness-muted remote-conn-row__hint">
                {move || match resume.get() {
                    RemoteResume::Tmux => i18n.tr(I18nKey::RemoteResumeTmuxHint)(),
                    RemoteResume::KeepaliveOnly => i18n.tr(I18nKey::RemoteResumeKeepaliveHint)(),
                }}
            </p>

            // Auth-specific secret inputs.
            <Show when=move || matches!(auth_kind.get(), RemoteAuthKind::Password)>
                <label class="remote-field">
                    <span class="remote-field__label">{move || i18n.tr(I18nKey::RemotePassword)()}</span>
                    <input
                        class="workbench-plain-input"
                        type="password"
                        autocomplete="off"
                        prop:value=move || password_input.get()
                        prop:placeholder=secret_placeholder
                        on:input=move |ev| password_input.set(input_value(&ev))
                    />
                </label>
            </Show>
            <Show when=move || matches!(auth_kind.get(), RemoteAuthKind::Key)>
                <div class="remote-conn-grid">
                    <label class="remote-field remote-field--wide">
                        <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteKeyPath)()}</span>
                        <input
                            class="workbench-plain-input"
                            type="text"
                            autocomplete="off"
                            spellcheck="false"
                            prop:value=move || key_path.get()
                            on:input=move |ev| key_path.set(input_value(&ev))
                        />
                    </label>
                    <label class="remote-field">
                        <span class="remote-field__label">{move || i18n.tr(I18nKey::RemotePassphrase)()}</span>
                        <input
                            class="workbench-plain-input"
                            type="password"
                            autocomplete="off"
                            prop:value=move || passphrase_input.get()
                            prop:placeholder=passphrase_placeholder
                            on:input=move |ev| passphrase_input.set(input_value(&ev))
                        />
                    </label>
                </div>
            </Show>

            <label class="remote-field remote-field--wide">
                <span class="remote-field__label">{move || i18n.tr(I18nKey::RemoteDefaultDir)()}</span>
                <span class="remote-dir-input">
                    <input
                        class="workbench-plain-input remote-dir-input__field"
                        type="text"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || remote_dir.get()
                        on:input=move |ev| remote_dir.set(input_value(&ev))
                    />
                    <button
                        type="button"
                        class="remote-dir-input__browse"
                        title=move || i18n.tr(I18nKey::RemoteBrowseDirs)()
                        aria-label=move || i18n.tr(I18nKey::RemoteBrowseDirs)()
                        disabled=move || !is_tauri_shell() || busy.get() || testing.get()
                        on:click=open_browser
                    >
                        <LxIcon icon=icondata::LuFolderSearch width="0.9rem" height="0.9rem" />
                    </button>
                </span>
            </label>

            <Show when=move || browser_open.get()>
                <div class="remote-dir-dialog__scrim" role="presentation">
                    <section
                        class="remote-dir-dialog"
                        role="dialog"
                        aria-modal="true"
                        aria-label=move || i18n.tr(I18nKey::RemoteSelectDir)()
                    >
                        <header class="remote-dir-dialog__head">
                            <div class="remote-dir-dialog__title">
                                <LxIcon icon=icondata::LuFolderOpen width="0.95rem" height="0.95rem" />
                                <span>{move || i18n.tr(I18nKey::RemoteDefaultDir)()}</span>
                            </div>
                            <button
                                type="button"
                                class="remote-dir-dialog__icon-btn"
                                aria-label=move || i18n.tr(I18nKey::BtnClose)()
                                on:click=move |_| browser_open.set(false)
                            >
                                <LxIcon icon=icondata::LuX width="0.85rem" height="0.85rem" />
                            </button>
                        </header>
                        <div class="remote-dir-dialog__path">
                            <button
                                type="button"
                                class="workbench-mini-btn"
                                disabled=move || browser_loading.get() || browser_parent.get().is_none()
                                on:click=move |_| {
                                    if let Some(parent) = browser_parent.get_untracked() {
                                        load_remote_dir(parent);
                                    }
                                }
                            >
                                <span class="harness-btn-inline">
                                    <LxIcon icon=icondata::LuArrowUp width="0.78rem" height="0.78rem" />
                                    <span>{move || i18n.tr(I18nKey::RemoteDirUp)()}</span>
                                </span>
                            </button>
                            <code>{move || browser_path.get()}</code>
                        </div>

                        <Show when=move || browser_error.get().is_some()>
                            <p class="harness-error remote-dir-dialog__message">{move || browser_error.get().unwrap_or_default()}</p>
                        </Show>
                        <Show when=move || browser_loading.get()>
                            <p class="harness-muted remote-dir-dialog__message">{move || i18n.tr(I18nKey::BlxLoading)()}</p>
                        </Show>

                        <ul class="remote-dir-dialog__list">
                            <For
                                each=move || browser_entries.get()
                                key=|entry| entry.path.clone()
                                children=move |entry: RemoteDirEntry| {
                                    let path_for_open = entry.path.clone();
                                    let path_for_pick = entry.path.clone();
                                    view! {
                                        <li>
                                            <button
                                                type="button"
                                                class="remote-dir-dialog__item"
                                                on:click=move |_| load_remote_dir(path_for_open.clone())
                                                on:dblclick=move |_| {
                                                    remote_dir.set(path_for_pick.clone());
                                                    browser_open.set(false);
                                                }
                                            >
                                                <LxIcon icon=icondata::LuFolder width="0.9rem" height="0.9rem" />
                                                <span>{entry.name}</span>
                                            </button>
                                        </li>
                                    }
                                }
                            />
                        </ul>

                        <footer class="remote-dir-dialog__actions">
                            <button
                                type="button"
                                class="workbench-mini-btn"
                                on:click=move |_| browser_open.set(false)
                            >
                                {move || i18n.tr(I18nKey::MemCancel)()}
                            </button>
                            <button
                                type="button"
                                class="workbench-mini-btn workbench-mini-btn--primary"
                                disabled=move || browser_path.get().is_empty()
                                on:click=move |_| {
                                    remote_dir.set(browser_path.get_untracked());
                                    browser_open.set(false);
                                }
                            >
                                {move || i18n.tr(I18nKey::Accept)()}
                            </button>
                        </footer>
                    </section>
                </div>
            </Show>

            <Show when=move || status_msg.with(|m| m.is_some())>
                <p class="harness-status">{move || status_msg.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || error_msg.with(|m| m.is_some())>
                <p class="harness-error">{move || error_msg.get().unwrap_or_default()}</p>
            </Show>

            <div class="remote-conn-row__actions harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    disabled=move || busy.get()
                    on:click=on_save
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuSave width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::RemoteSave)()}</span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn"
                    disabled=move || testing.get() || busy.get()
                    on:click=on_test
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuPlugZap width="0.78rem" height="0.78rem" />
                        <span>{move || if testing.get() {
                            i18n.tr(I18nKey::RemoteTesting)()
                        } else {
                            i18n.tr(I18nKey::RemoteTest)()
                        }}</span>
                    </span>
                </button>
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--danger"
                    disabled=move || busy.get()
                    on:click=on_delete
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuTrash2 width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::RemoteDelete)()}</span>
                    </span>
                </button>
            </div>
        </section>
    }
}
