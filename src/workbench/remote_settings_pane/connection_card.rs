//! A single saved SSH connection rendered as a clickable card in the list
//! view. Shows only non-secret metadata plus a "stored / not set" badge for
//! the relevant secret — clicking the card opens the editor.

use super::{auth_label_key, resume_label_key};
use crate::i18n::I18nKey;
use crate::service::I18nService;
use crate::tauri_bridge::{RemoteAuthKind, RemoteConnectionView};
use leptos::prelude::*;
use leptos_icons::Icon as LxIcon;

#[component]
pub fn RemoteConnectionCard(
    view: RemoteConnectionView,
    on_edit: Callback<()>,
) -> impl IntoView {
    let i18n = expect_context::<I18nService>();
    let conn = view.connection.clone();

    let title = if conn.label.trim().is_empty() {
        conn.host.clone()
    } else {
        conn.label.clone()
    };
    // `user@host:port`, skipping any empty parts gracefully.
    let endpoint = {
        let host_port = if conn.host.is_empty() {
            String::new()
        } else {
            format!("{}:{}", conn.host, conn.port)
        };
        if conn.username.is_empty() {
            host_port
        } else if host_port.is_empty() {
            conn.username.clone()
        } else {
            format!("{}@{}", conn.username, host_port)
        }
    };

    let auth_key = auth_label_key(conn.auth_kind);
    let resume_key = resume_label_key(conn.resume);
    let remote_dir = conn.default_remote_dir.clone();

    // Secret indicator: passwords use `has_password`, key auth uses
    // `has_passphrase`. SSH-agent auth carries no stored secret.
    let secret_stored = match conn.auth_kind {
        RemoteAuthKind::Password => Some(view.has_password),
        RemoteAuthKind::Key => Some(view.has_passphrase),
        RemoteAuthKind::Agent => None,
    };

    view! {
        <li class="remote-conn-card-item">
            <button
                type="button"
                class="settings-field-card remote-conn-card"
                title=move || i18n.tr(I18nKey::RemoteCardEditHint)()
                aria-label=move || i18n.tr(I18nKey::RemoteEditConnection)()
                on:click=move |_| on_edit.run(())
            >
                <div class="remote-conn-card__head">
                    <span class="remote-conn-card__title">
                        <span class="remote-conn-card__icon" aria-hidden="true">
                            <LxIcon icon=icondata::LuServer width="0.85rem" height="0.85rem" />
                        </span>
                        <span class="remote-conn-card__name">{title}</span>
                    </span>
                    <span class="remote-conn-card__chevron" aria-hidden="true">
                        <LxIcon icon=icondata::LuChevronRight width="0.9rem" height="0.9rem" />
                    </span>
                </div>

                <Show when={ let ep = endpoint.clone(); move || !ep.is_empty() }>
                    <code class="remote-conn-card__endpoint">{endpoint.clone()}</code>
                </Show>

                <div class="remote-conn-card__meta">
                    <span class="remote-conn-card__badge">{move || i18n.tr(auth_key)()}</span>
                    <span class="remote-conn-card__badge">{move || i18n.tr(resume_key)()}</span>
                    {move || {
                        secret_stored
                            .map(|stored| {
                                let label = if stored {
                                    i18n.tr(I18nKey::RemoteSecretStored)()
                                } else {
                                    i18n.tr(I18nKey::RemoteSecretNotSet)()
                                };
                                view! {
                                    <span
                                        class="remote-conn-card__badge"
                                        class:remote-conn-card__badge--muted=move || !stored
                                    >
                                        {label}
                                    </span>
                                }
                            })
                    }}
                </div>

                <Show when={ let d = remote_dir.clone(); move || d.is_some() }>
                    <code class="remote-conn-card__dir">
                        {remote_dir.clone().unwrap_or_default()}
                    </code>
                </Show>
            </button>
        </li>
    }
}
