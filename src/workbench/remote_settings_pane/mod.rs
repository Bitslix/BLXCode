//! Settings → Remote: manage SSH connection presets used by remote
//! workspaces. Non-secret metadata is persisted by the backend
//! (`remote_connections.json`); passwords / key passphrases go to the OS
//! keychain via `ssh_secrets`. Secret inputs here are write-only — the pane
//! only ever learns whether a secret is *stored*, never its value.

use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{
    is_tauri_shell, ssh_remote_delete, ssh_remote_save, ssh_remote_test, ssh_remotes_list,
    RemoteAuthKind, RemoteConnection, RemoteConnectionView, RemoteResume,
};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;
use wasm_bindgen::JsCast;

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

fn auth_to_str(kind: RemoteAuthKind) -> &'static str {
    match kind {
        RemoteAuthKind::Password => "password",
        RemoteAuthKind::Key => "key",
        RemoteAuthKind::Agent => "agent",
    }
}

fn auth_from_str(s: &str) -> RemoteAuthKind {
    match s {
        "key" => RemoteAuthKind::Key,
        "agent" => RemoteAuthKind::Agent,
        _ => RemoteAuthKind::Password,
    }
}

fn resume_to_str(r: RemoteResume) -> &'static str {
    match r {
        RemoteResume::Tmux => "tmux",
        RemoteResume::KeepaliveOnly => "keepalive",
    }
}

fn resume_from_str(s: &str) -> RemoteResume {
    match s {
        "tmux" => RemoteResume::Tmux,
        _ => RemoteResume::KeepaliveOnly,
    }
}

#[derive(Clone)]
struct RowModel {
    uid: u64,
    view: RemoteConnectionView,
}

fn blank_view() -> RemoteConnectionView {
    RemoteConnectionView {
        connection: RemoteConnection {
            id: String::new(),
            label: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            auth_kind: RemoteAuthKind::Password,
            key_path: None,
            resume: RemoteResume::KeepaliveOnly,
            default_remote_dir: None,
        },
        has_password: false,
        has_passphrase: false,
    }
}

#[component]
pub fn RemoteSettingsPane() -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let rows: RwSignal<Vec<RowModel>> = RwSignal::new(Vec::new());
    let next_uid = StoredValue::new(1u64);
    let load_error: RwSignal<Option<String>> = RwSignal::new(None);

    let alloc_uid = move || {
        let id = next_uid.get_value();
        next_uid.set_value(id + 1);
        id
    };

    let load = move || {
        if !is_tauri_shell() {
            return;
        }
        leptos::task::spawn_local(async move {
            match ssh_remotes_list().await {
                Ok(list) => {
                    let models = list
                        .into_iter()
                        .map(|view| RowModel {
                            uid: alloc_uid(),
                            view,
                        })
                        .collect::<Vec<_>>();
                    rows.set(models);
                    load_error.set(None);
                }
                Err(err) => load_error.set(Some(err)),
            }
        });
    };

    Effect::new(move |_| {
        load();
    });

    let add_row = move |_| {
        rows.update(|v| {
            v.push(RowModel {
                uid: alloc_uid(),
                view: blank_view(),
            });
        });
    };

    view! {
        <article class="harness-pane remote-pane">
            <h3 class="harness-pane-title">
                <span class="harness-pane-title__icon" aria-hidden="true">
                    <LxIcon icon=icondata::LuServer width="1.02rem" height="1.02rem" />
                </span>
                <span class="harness-pane-title__text">{move || i18n.tr(I18nKey::RemoteHeading)()}</span>
            </h3>
            <p class="harness-muted">{move || i18n.tr(I18nKey::RemoteSubtitle)()}</p>

            <Show when=move || !is_tauri_shell()>
                <p class="harness-error">{move || i18n.tr(I18nKey::RemoteRequiresTauri)()}</p>
            </Show>
            <Show when=move || load_error.with(|m| m.is_some())>
                <p class="harness-error">{move || load_error.get().unwrap_or_default()}</p>
            </Show>
            <Show when=move || is_tauri_shell() && rows.with(|v| v.is_empty()) && load_error.with(|m| m.is_none())>
                <p class="harness-muted">{move || i18n.tr(I18nKey::RemoteEmpty)()}</p>
            </Show>

            <ul class="remote-conn-list">
                <For
                    each=move || rows.get()
                    key=|r| r.uid
                    children=move |row: RowModel| {
                        view! { <RemoteConnectionRow uid=row.uid initial=row.view rows=rows /> }
                    }
                />
            </ul>

            <footer class="settings-pane-footer harness-row-gap">
                <button
                    type="button"
                    class="workbench-mini-btn workbench-mini-btn--primary"
                    disabled=move || !is_tauri_shell()
                    on:click=add_row
                >
                    <span class="harness-btn-inline">
                        <LxIcon icon=icondata::LuPlus width="0.78rem" height="0.78rem" />
                        <span>{move || i18n.tr(I18nKey::RemoteAddConnection)()}</span>
                    </span>
                </button>
            </footer>
        </article>
    }
}

#[component]
fn RemoteConnectionRow(
    uid: u64,
    initial: RemoteConnectionView,
    rows: RwSignal<Vec<RowModel>>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();

    let id = RwSignal::new(initial.connection.id.clone());
    let label = RwSignal::new(initial.connection.label.clone());
    let host = RwSignal::new(initial.connection.host.clone());
    let port = RwSignal::new(initial.connection.port.to_string());
    let username = RwSignal::new(initial.connection.username.clone());
    let auth_kind = RwSignal::new(initial.connection.auth_kind);
    let key_path = RwSignal::new(initial.connection.key_path.clone().unwrap_or_default());
    let resume = RwSignal::new(initial.connection.resume);
    let remote_dir = RwSignal::new(initial.connection.default_remote_dir.clone().unwrap_or_default());
    let password_input = RwSignal::new(String::new());
    let passphrase_input = RwSignal::new(String::new());
    let has_password = RwSignal::new(initial.has_password);
    let has_passphrase = RwSignal::new(initial.has_passphrase);

    let busy = RwSignal::new(false);
    let testing = RwSignal::new(false);
    let status_msg: RwSignal<Option<String>> = RwSignal::new(None);
    let error_msg: RwSignal<Option<String>> = RwSignal::new(None);

    // Assemble the current preset (no secrets) from the row's fields.
    let build_connection = move || RemoteConnection {
        id: id.get_untracked(),
        label: label.get_untracked().trim().to_string(),
        host: host.get_untracked().trim().to_string(),
        port: port.get_untracked().trim().parse::<u16>().unwrap_or(22),
        username: username.get_untracked().trim().to_string(),
        auth_kind: auth_kind.get_untracked(),
        key_path: {
            let k = key_path.get_untracked().trim().to_string();
            if k.is_empty() { None } else { Some(k) }
        },
        resume: resume.get_untracked(),
        default_remote_dir: {
            let d = remote_dir.get_untracked().trim().to_string();
            if d.is_empty() { None } else { Some(d) }
        },
    };
    // Secrets are sent only when the user typed a new value.
    let current_password = move || {
        let v = password_input.get_untracked();
        if v.is_empty() { None } else { Some(v) }
    };
    let current_passphrase = move || {
        let v = passphrase_input.get_untracked();
        if v.is_empty() { None } else { Some(v) }
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
                Ok(view) => {
                    id.set(view.connection.id.clone());
                    has_password.set(view.has_password);
                    has_passphrase.set(view.has_passphrase);
                    password_input.set(String::new());
                    passphrase_input.set(String::new());
                    status_msg.set(Some(i18n.tr(I18nKey::RemoteSaved)().to_string()));
                }
                Err(err) => error_msg.set(Some(format!(
                    "{}: {err}",
                    i18n.tr(I18nKey::RemoteSaveError)()
                ))),
            }
            busy.set(false);
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
            rows.update(|v| v.retain(|r| r.uid != uid));
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
                Ok(()) => rows.update(|v| v.retain(|r| r.uid != uid)),
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

    view! {
        <li class="settings-field-card remote-conn-row">
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
                <input
                    class="workbench-plain-input"
                    type="text"
                    autocomplete="off"
                    spellcheck="false"
                    prop:value=move || remote_dir.get()
                    on:input=move |ev| remote_dir.set(input_value(&ev))
                />
            </label>

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
        </li>
    }
}
